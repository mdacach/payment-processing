use crate::types::{ClientId, Money, TxId};

// TODO: create newtype for money unit, in order to allow four points after the
// decimal of precision.
// TODO: consider creating newtypes for values used here.
// TODO: add documentation.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Event {
    Deposit {
        client_id: ClientId,
        tx_id: TxId,
        amount: Money,
    },
    Withdrawal {
        client_id: ClientId,
        tx_id: TxId,
        amount: Money,
    },
    Dispute {
        client_id: ClientId,
        referred_tx_id: TxId,
    },
    Resolve {
        client_id: ClientId,
        referred_tx_id: TxId,
    },
    Chargeback {
        client_id: ClientId,
        referred_tx_id: TxId,
    },
}
