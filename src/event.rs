use crate::types::{ClientId, Money, TxId};

// TODO: consider creating newtypes for values used here.
// TODO: add documentation.
#[derive(Debug, Clone, Copy)]
pub enum Event {
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

// TODO: is there a better way of extracting this information? maybe only with macros?
impl Event {
    pub(crate) fn client_id(&self) -> &ClientId {
        match self {
            Event::Deposit {
                client_id,
                tx_id: _,
                amount: _,
            } => client_id,
            Event::Withdrawal {
                client_id,
                tx_id: _,
                amount: _,
            } => client_id,
            Event::Dispute {
                client_id,
                referred_tx_id: _,
            } => client_id,
            Event::Resolve {
                client_id,
                referred_tx_id: _,
            }
            | Event::Chargeback {
                client_id,
                referred_tx_id: _,
            } => client_id,
        }
    }
}
