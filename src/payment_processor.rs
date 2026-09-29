use std::collections::BTreeMap;

use crate::{
    event::Event,
    types::{ClientId, Money, TxId},
};

// TODO: consider a state machine design here, where a locked account is a final state.
#[derive(Debug, Clone, Default)]
pub struct Account {
    // TODO: decide whether to allow negative totals. a possible scenario that would
    //       create a negative total is a deposit that is withdrawn and then disputed.
    total: Money,
    available: Money,
    held: Money,
    is_locked: bool,
}

// TODO: consider creating specific functions to atomically update an account
//       such as "deposit" or "withdraw". that way we have more control over
//       how the balances are updated, and can better assure that the transitions
//       make sense.
impl Account {
    pub fn total(&self) -> Money {
        self.total
    }

    pub fn available(&self) -> Money {
        self.available
    }

    pub fn held(&self) -> Money {
        self.held
    }

    pub fn is_locked(&self) -> bool {
        self.is_locked
    }
}

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

// TODO: not sure how I feel about this being Clone, but anyway it's
//       not too bad here.
#[derive(Debug, Clone, Default)]
pub struct PaymentProcessor {
    accounts: BTreeMap<ClientId, Account>,
    deposits: BTreeMap<TxId, DepositInfo>,
}

impl PaymentProcessor {
    // The system attempt at handling an event.
    //
    // Events are dispatched to specific handlers.
    pub fn on_event(&mut self, event: Event) -> anyhow::Result<()> {
        // TODO: add witness pattern instead of this if.
        self.maybe_prevent_locked_account(event)?;

        match event {
            Event::Deposit {
                client_id,
                tx_id,
                amount,
            } => self.handle_deposit(client_id, tx_id, amount),
            Event::Withdrawal {
                client_id,
                tx_id,
                amount,
            } => self.handle_withdrawal(client_id, tx_id, amount),
            Event::Dispute {
                client_id,
                referred_tx_id,
            } => self.handle_dispute(client_id, referred_tx_id),
            Event::Resolve {
                client_id,
                referred_tx_id,
            } => self.handle_resolve(client_id, referred_tx_id),
            Event::Chargeback {
                client_id,
                referred_tx_id,
            } => self.handle_chargeback(client_id, referred_tx_id),
        }
    }

    /// Credits `amount` to the client's available and total funds and records
    /// the deposit under `tx_id` so it can be disputed later.
    fn handle_deposit(
        &mut self,
        client_id: ClientId,
        tx_id: TxId,
        amount: Money,
    ) -> anyhow::Result<()> {
        let account = self.accounts.entry(client_id).or_default();
        account.total += amount;
        account.available += amount;

        let info = DepositInfo {
            client_id,
            amount,
            status: DepositStatus::Processed,
        };
        self.deposits.insert(tx_id, info);

        Ok(())
    }

    /// Debits `amount` from the client's available and total funds.
    ///
    /// Rejects the withdrawal without changing either balance if available
    /// funds are insufficient. The withdrawal's transaction ID is not needed
    /// after dispatch because only deposits can be disputed.
    fn handle_withdrawal(
        &mut self,
        client_id: ClientId,
        _tx_id: TxId,
        amount: Money,
    ) -> anyhow::Result<()> {
        let account = self.accounts.entry(client_id).or_default();
        if account.available < amount {
            anyhow::bail!("insufficient funds");
        }

        account.total -= amount;
        account.available -= amount;

        Ok(())
    }

    /// Holds the amount of the deposit identified by `referred_tx_id`.
    ///
    /// Moves that amount from available to held funds without changing total
    /// funds. Rejects a missing deposit, a deposit owned by another client, or
    /// one that has already been disputed. Available funds may become negative
    /// if the client has already spent the deposit.
    fn handle_dispute(&mut self, client_id: ClientId, referred_tx_id: TxId) -> anyhow::Result<()> {
        let account = self.accounts.entry(client_id).or_default();

        match self.deposits.get_mut(&referred_tx_id) {
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

                deposit_info.status = DepositStatus::Disputed;
                // A disputed deposit freezes associated funds.
                account.available -= deposit_info.amount;
                account.held += deposit_info.amount;

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
    fn handle_resolve(&mut self, client_id: ClientId, referred_tx_id: TxId) -> anyhow::Result<()> {
        let account = self.accounts.entry(client_id).or_default();

        match self.deposits.get_mut(&referred_tx_id) {
            Some(deposit_info) => {
                if deposit_info.client_id != client_id {
                    // Very weird, huh!
                    anyhow::bail!("resolve transaction with wrong client id!");
                }

                let DepositStatus::Disputed = deposit_info.status else {
                    // A resolve must only refer to a deposit that is currently being disputed.
                    anyhow::bail!("attemt to resolve not-currently-disputed deposit");
                };

                deposit_info.status = DepositStatus::Resolved;
                // The dispute had previously frozen the associated funds for this deposit,
                // but now that it has been resolved, the funds are released.
                account.available += deposit_info.amount;
                account.held -= deposit_info.amount;
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
        &mut self,
        client_id: ClientId,
        referred_tx_id: TxId,
    ) -> anyhow::Result<()> {
        let account = self.accounts.entry(client_id).or_default();

        match self.deposits.get_mut(&referred_tx_id) {
            Some(deposit_info) => {
                if deposit_info.client_id != client_id {
                    // Very weird, huh!
                    anyhow::bail!("chargeback transaction with wrong client id!");
                }

                let DepositStatus::Disputed = deposit_info.status else {
                    // A chargeback must only refer to a deposit that is currently being disputed.
                    anyhow::bail!("attemt to chargeback not-currently-disputed deposit");
                };

                deposit_info.status = DepositStatus::Chargedback;
                // In the case of a chargeback, the frozen funds have been withdrawn.
                account.held -= deposit_info.amount;
                account.total -= deposit_info.amount;

                // As part of fraud detection, a chargeback causes a client's account to be locked.
                account.is_locked = true;
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

impl PaymentProcessor {
    pub fn account(&self, client_id: ClientId) -> Option<&Account> {
        self.accounts.get(&client_id)
    }

    pub fn accounts(&self) -> impl Iterator<Item = (ClientId, &Account)> {
        self.accounts
            .iter()
            .map(|(&client_id, account)| (client_id, account))
    }

    // TODO: investigate how to make this more secure. there are some patterns that could come in handy,
    //       like witness: https://arxiv.org/pdf/2307.07069
    fn maybe_prevent_locked_account(&self, event: Event) -> anyhow::Result<()> {
        // In this system, a client only has a single account and that account
        // is identifiable by the client's id.
        let account_id = event.client_id();
        if let Some(account) = self.account(*account_id) {
            if account.is_locked {
                // TODO: when reworking errors, make sure to add relevant context, like account ids.
                anyhow::bail!("account referred by event is locked");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod property_tests;

#[cfg(test)]
mod regression_tests;
