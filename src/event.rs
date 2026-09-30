use crate::types::{ClientId, TransactionAmount, TxId};

/// A transaction event processed against a client's account.
///
/// Deposits and withdrawals have their own transaction identifiers, while
/// disputes, resolves and chargebacks always refer to another transaction
/// (which should be a deposit).
#[derive(Debug, Clone, Copy)]
pub enum Event {
    /// Credits the client's available and total funds.
    ///
    /// A deposit may later be disputed, holding its funds until a resolve or
    /// chargeback.
    Deposit {
        /// The client whose account receives the funds.
        client_id: ClientId,
        /// This deposit's transaction ID.
        tx_id: TxId,
        /// The amount credited to the account.
        amount: TransactionAmount,
    },
    /// Debits the client's available and total funds.
    ///
    /// A withdrawal is rejected if the account has insufficient available
    /// funds.
    Withdrawal {
        /// The client whose account is debited.
        client_id: ClientId,
        /// This withdrawal's transaction ID.
        tx_id: TxId,
        /// The amount withdrawn from the account.
        amount: TransactionAmount,
    },
    /// Holds the funds associated with an earlier deposit.
    ///
    /// The deposit's amount moves from available to held funds while the
    /// dispute is pending. Only deposits can be disputed, and each deposit can
    /// be disputed at most once.
    Dispute {
        /// The client who owns the disputed deposit.
        client_id: ClientId,
        /// The transaction ID of the deposit under dispute. The CSV amount is empty.
        referred_tx_id: TxId,
    },
    /// Ends a dispute by releasing the deposit's held funds back to available.
    ///
    /// A resolve must refer to a deposit that is currently disputed.
    Resolve {
        /// The client who owns the disputed deposit.
        client_id: ClientId,
        /// The transaction ID of the disputed deposit. The CSV amount is empty.
        referred_tx_id: TxId,
    },
    /// Ends a dispute by removing the held funds from the account.
    ///
    /// A chargeback reverses the deposit and immediately locks the client's
    /// account against further operations. It may leave a negative balance if
    /// the deposited funds were already spent.
    Chargeback {
        /// The client who owns the disputed deposit.
        client_id: ClientId,
        /// The transaction ID of the disputed deposit. The CSV amount is empty.
        referred_tx_id: TxId,
    },
}

impl Event {
    // The affected client by the event. Client identification is specially
    // relevant because locked accounts must be prevented from operating.
    pub(crate) fn client_id(&self) -> &ClientId {
        match self {
            Event::Deposit { client_id, .. }
            | Event::Withdrawal { client_id, .. }
            | Event::Dispute { client_id, .. }
            | Event::Resolve { client_id, .. }
            | Event::Chargeback { client_id, .. } => client_id,
        }
    }

    /// This event's transaction ID, or the referred deposit ID for a dispute operation.
    pub(crate) fn transaction_id(&self) -> TxId {
        match self {
            Event::Deposit { tx_id, .. }
            | Event::Withdrawal { tx_id, .. }
            | Event::Dispute {
                referred_tx_id: tx_id,
                ..
            }
            | Event::Resolve {
                referred_tx_id: tx_id,
                ..
            }
            | Event::Chargeback {
                referred_tx_id: tx_id,
                ..
            } => *tx_id,
        }
    }
}
