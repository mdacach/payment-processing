use std::collections::BTreeMap;

use crate::{
    error::ProcessorError,
    event::Event,
    types::{ClientId, Money, TxId},
};

mod account;
pub use account::Account;
pub(crate) use account::AccountError;
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
    pub fn on_event(&mut self, event: Event) -> Result<(), ProcessorError> {
        let client_id = *event.client_id();
        let tx_id = event.transaction_id();
        let Self { accounts, deposits } = self;

        let mut account = accounts
            .entry(client_id)
            .or_default()
            .try_active()
            .map_err(|error| ProcessorError::from_account_error(error, client_id, tx_id))?;

        match event {
            Event::Deposit {
                client_id,
                tx_id,
                amount,
            } => Self::handle_deposit(&mut account, deposits, client_id, tx_id, amount),
            Event::Withdrawal {
                client_id,
                tx_id,
                amount,
            } => Self::handle_withdrawal(&mut account, client_id, tx_id, amount),
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
    ) -> Result<(), ProcessorError> {
        account
            .deposit(amount)
            .map_err(|error| ProcessorError::from_account_error(error, client_id, tx_id))?;

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
    /// funds are insufficient. The withdrawal's transaction ID is used for
    /// error context; only deposits can be disputed.
    fn handle_withdrawal(
        account: &mut ActiveAccountGuard<'_>,
        client_id: ClientId,
        tx_id: TxId,
        amount: Money,
    ) -> Result<(), ProcessorError> {
        account
            .withdrawal(amount)
            .map_err(|error| ProcessorError::from_account_error(error, client_id, tx_id))?;

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
    ) -> Result<(), ProcessorError> {
        let deposit_info = deposits
            .get_mut(&referred_tx_id)
            .filter(|info| {
                info.client_id == client_id && matches!(info.status, DepositStatus::Processed)
            })
            .ok_or(ProcessorError::DisputableDepositNotFound {
                client_id,
                referred_tx_id,
            })?;

        account.dispute(deposit_info.amount).map_err(|error| {
            ProcessorError::from_account_error(error, client_id, referred_tx_id)
        })?;
        deposit_info.status = DepositStatus::Disputed;

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
    ) -> Result<(), ProcessorError> {
        let deposit_info = deposits
            .get_mut(&referred_tx_id)
            .filter(|info| {
                info.client_id == client_id && matches!(info.status, DepositStatus::Disputed)
            })
            .ok_or(ProcessorError::EligibleDepositNotFound {
                client_id,
                referred_tx_id,
            })?;

        account.resolve(deposit_info.amount).map_err(|error| {
            ProcessorError::from_account_error(error, client_id, referred_tx_id)
        })?;
        deposit_info.status = DepositStatus::Resolved;

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
    ) -> Result<(), ProcessorError> {
        let deposit_info = deposits
            .get_mut(&referred_tx_id)
            .filter(|info| {
                info.client_id == client_id && matches!(info.status, DepositStatus::Disputed)
            })
            .ok_or(ProcessorError::EligibleDepositNotFound {
                client_id,
                referred_tx_id,
            })?;

        account.chargeback(deposit_info.amount).map_err(|error| {
            ProcessorError::from_account_error(error, client_id, referred_tx_id)
        })?;
        deposit_info.status = DepositStatus::Chargedback;

        Ok(())
    }
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
