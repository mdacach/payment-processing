use crate::{
    payment_processor::AccountError,
    types::{Balance, ClientId, TransactionAmount, TxId},
};

/// Error with the reason why an event could not be successfully processed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ProcessorError {
    /// The client account is locked.
    ///
    /// Locked accounts are prevented from all operations, including further
    /// disputes and settlements.
    #[error("account for client {client_id} is locked")]
    AccountLocked { client_id: ClientId },

    /// There are insufficient funds for the withdrawal.
    #[error(
        "insufficient funds for client {client_id}, transaction {tx_id}: requested {requested}, available {available}"
    )]
    InsufficientFunds {
        client_id: ClientId,
        tx_id: TxId,
        available: Balance,
        requested: TransactionAmount,
    },

    /// No deposit with referred transaction identifier was found.
    #[error("disputable deposit {referred_tx_id} not found for client {client_id}")]
    DisputableDepositNotFound {
        client_id: ClientId,
        referred_tx_id: TxId,
    },

    /// No disputed deposit with referred transaction identifier was found.
    #[error("no disputed deposit {referred_tx_id} eligible for settlement for client {client_id}")]
    EligibleDepositNotFound {
        client_id: ClientId,
        referred_tx_id: TxId,
    },

    /// An arithmetic overflow ocurred while updating the balance.
    ///
    /// Note that `tx_id` is the event's identifier. In the case of disputes,
    /// resolves, or chargebacks, it's the identifier for the deposit instead of
    /// the failed processed transaction.
    #[error("{balance} balance overflow for client {client_id}, transaction {tx_id}")]
    BalanceOverflow {
        client_id: ClientId,
        tx_id: TxId,
        balance: BalanceField,
    },
}

impl ProcessorError {
    pub(crate) fn from_account_error(
        error: AccountError,
        client_id: ClientId,
        tx_id: TxId,
    ) -> Self {
        match error {
            AccountError::Locked => Self::AccountLocked { client_id },
            AccountError::InsufficientFunds {
                available,
                requested,
            } => Self::InsufficientFunds {
                client_id,
                tx_id,
                available,
                requested,
            },
            AccountError::Overflow { balance } => Self::BalanceOverflow {
                client_id,
                tx_id,
                balance,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceField {
    Available,
    Held,
    Total,
}

impl std::fmt::Display for BalanceField {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}",
            match self {
                Self::Available => "available",
                Self::Held => "held",
                Self::Total => "total",
            }
        )
    }
}
