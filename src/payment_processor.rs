use std::collections::BTreeMap;

use crate::{
    event::Event,
    types::{ClientId, Money, TxId},
};

mod account;
pub use account::Account;
use account::ActiveAccountGuard;

// TODO: consider a more comprehensive state machine pattern here, instead of status.
#[derive(Debug, Clone)]
pub(crate) struct DepositInfo {
    client_id: ClientId,
    amount: Money,
    status: DepositStatus,
}

#[derive(Debug, Clone)]
enum DepositStatus {
    Processed,
    Disputed,
    Resolved,
    Chargedback,
}

/// Processes events in order and tracks each client's account and deposits.
#[derive(Debug, Clone, Default)]
pub struct PaymentProcessor {
    accounts: BTreeMap<ClientId, Account>,
    deposits: BTreeMap<TxId, DepositInfo>,
}

impl PaymentProcessor {
    /// Applies an event to the client's account and returns an error if it is rejected.
    ///
    /// A locked account rejects every further operation, including disputes,
    /// resolves, and chargebacks.
    ///
    /// # Errors
    ///
    /// Returns an error if the account is locked or the event cannot be applied.
    pub fn on_event(&mut self, event: Event) -> anyhow::Result<()> {
        let client_id = *event.client_id();
        let Self { accounts, deposits } = self;

        let mut account = accounts.entry(client_id).or_default().try_active()?;

        match event {
            Event::Deposit {
                client_id,
                tx_id,
                amount,
            } => Self::handle_deposit(&mut account, deposits, client_id, tx_id, amount),
            Event::Withdrawal {
                client_id: _,
                tx_id: _,
                amount,
            } => Self::handle_withdrawal(&mut account, amount),
            Event::Dispute {
                client_id,
                referred_tx_id,
            } => Self::handle_dispute(&mut account, deposits, client_id, referred_tx_id),
            Event::Resolve {
                client_id,
                referred_tx_id,
            } => Self::handle_resolve(&mut account, deposits, client_id, referred_tx_id),
            Event::Chargeback {
                client_id,
                referred_tx_id,
            } => Self::handle_chargeback(account, deposits, client_id, referred_tx_id),
        }
    }

    /// Credits `amount` to the client's available and total funds and records
    /// the deposit under `tx_id` so it can be disputed later.
    fn handle_deposit(
        account: &mut ActiveAccountGuard<'_>,
        deposits: &mut BTreeMap<TxId, DepositInfo>,
        client_id: ClientId,
        tx_id: TxId,
        amount: Money,
    ) -> anyhow::Result<()> {
        validate_movement_amount(amount)?;
        account.deposit(amount)?;

        let info = DepositInfo {
            client_id,
            amount,
            status: DepositStatus::Processed,
        };
        deposits.insert(tx_id, info);

        Ok(())
    }

    /// Debits `amount` from the client's available and total funds.
    ///
    /// Rejects the withdrawal without changing either balance if available
    /// funds are insufficient. The withdrawal's transaction ID is not needed
    /// after dispatch because only deposits can be disputed.
    fn handle_withdrawal(
        account: &mut ActiveAccountGuard<'_>,
        amount: Money,
    ) -> anyhow::Result<()> {
        validate_movement_amount(amount)?;
        account.withdrawal(amount)?;

        Ok(())
    }

    /// Holds the amount of the deposit identified by `referred_tx_id`.
    ///
    /// Moves that amount from available to held funds without changing total
    /// funds. Rejects a missing deposit, a deposit owned by another client, or
    /// one that has already been disputed. Available funds may become negative
    /// if the client has already spent the deposit.
    fn handle_dispute(
        account: &mut ActiveAccountGuard<'_>,
        deposits: &mut BTreeMap<TxId, DepositInfo>,
        client_id: ClientId,
        referred_tx_id: TxId,
    ) -> anyhow::Result<()> {
        match deposits.get_mut(&referred_tx_id) {
            Some(deposit_info) => {
                if deposit_info.client_id != client_id {
                    // Very weird, huh!
                    anyhow::bail!("disputed transaction with wrong client id!");
                }

                let DepositStatus::Processed = deposit_info.status else {
                    // A deposit can only be disputed once, so extra requests of
                    // disputing it are considered an error and ignored.
                    anyhow::bail!("deposit has already been disputed");
                };

                // TODO: double-check whether to allow negative available funds,
                //       in the case where a disputed deposit has already been
                //       withdrawn.

                // A disputed deposit freezes associated funds.
                account.dispute(deposit_info.amount)?;
                deposit_info.status = DepositStatus::Disputed;

                // This dispute should eventually be resolved either through a
                // [`Resolve`] or a [`Chargeback`].
            }
            None => {
                // TODO: might be worthwhile to differentiate between
                //       no-tx-at-all and no-deposit.
                anyhow::bail!("only deposits can be disputed");
            }
        }

        Ok(())
    }

    /// Resolves the dispute for the deposit identified by `referred_tx_id`.
    ///
    /// Returns the deposit's amount from held to available funds without
    /// changing total funds. Rejects a missing deposit, a deposit owned by
    /// another client, or one that is not currently disputed.
    fn handle_resolve(
        account: &mut ActiveAccountGuard<'_>,
        deposits: &mut BTreeMap<TxId, DepositInfo>,
        client_id: ClientId,
        referred_tx_id: TxId,
    ) -> anyhow::Result<()> {
        match deposits.get_mut(&referred_tx_id) {
            Some(deposit_info) => {
                if deposit_info.client_id != client_id {
                    // Very weird, huh!
                    anyhow::bail!("resolve transaction with wrong client id!");
                }

                let DepositStatus::Disputed = deposit_info.status else {
                    // A resolve must only refer to a deposit that is currently being disputed.
                    anyhow::bail!("attemt to resolve not-currently-disputed deposit");
                };

                // The dispute had previously frozen the associated funds for this deposit,
                // but now that it has been resolved, the funds are released.
                account.resolve(deposit_info.amount)?;
                deposit_info.status = DepositStatus::Resolved;
            }
            None => {
                // TODO: might be worthwhile to differentiate between
                //       no-tx-at-all and no-deposit.
                anyhow::bail!("only deposits can be disputed");
            }
        }

        Ok(())
    }

    /// Reverses a disputed deposit and locks the client's account.
    ///
    /// Removes the deposit's amount from held and total funds. The resulting
    /// total may be negative if the deposited funds were already spent. Rejects
    /// a missing deposit, a deposit owned by another client, or one that is not
    /// currently disputed. The lock prevents subsequent events for this client.
    fn handle_chargeback(
        account: ActiveAccountGuard<'_>,
        deposits: &mut BTreeMap<TxId, DepositInfo>,
        client_id: ClientId,
        referred_tx_id: TxId,
    ) -> anyhow::Result<()> {
        match deposits.get_mut(&referred_tx_id) {
            Some(deposit_info) => {
                if deposit_info.client_id != client_id {
                    // Very weird, huh!
                    anyhow::bail!("chargeback transaction with wrong client id!");
                }

                let DepositStatus::Disputed = deposit_info.status else {
                    // A chargeback must only refer to a deposit that is currently being disputed.
                    anyhow::bail!("attemt to chargeback not-currently-disputed deposit");
                };

                // In the case of a chargeback, the frozen funds have been withdrawn.
                account.chargeback(deposit_info.amount)?;
                deposit_info.status = DepositStatus::Chargedback;
            }
            None => {
                // TODO: might be worthwhile to differentiate between
                //       no-tx-at-all and no-deposit.
                anyhow::bail!("only deposits can be disputed");
            }
        }

        Ok(())
    }
}

/// Deposits and withdrawals must be between 0.0001 and 10,000,000.0000.
fn validate_movement_amount(amount: Money) -> anyhow::Result<()> {
    let minimum = Money::from_mantissa(1);
    let maximum = Money::from_mantissa(100_000_000_000);
    if amount < minimum || amount > maximum {
        anyhow::bail!("amount must be between 0.0001 and 10000000.0000");
    }
    Ok(())
}

impl PaymentProcessor {
    /// Returns the account for a client, if one has been created.
    pub fn account(&self, client_id: ClientId) -> Option<&Account> {
        self.accounts.get(&client_id)
    }

    /// Iterates over client accounts in client ID order.
    pub fn accounts(&self) -> impl Iterator<Item = (ClientId, &Account)> {
        self.accounts
            .iter()
            .map(|(&client_id, account)| (client_id, account))
    }
}

#[cfg(test)]
mod property_tests;

#[cfg(test)]
mod scenario_tests;
