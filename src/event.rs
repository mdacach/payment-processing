// TODO: create newtype for money unit, in order to allow four points after the
// decimal of precision.
// TODO: consider creating newtypes for values used here.
// TODO: add documentation.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Event {
    Deposit {
        client_id: u16,
        tx_id: u32,
        amount: i64,
    },
    Withdrawal {
        client_id: u16,
        tx_id: u32,
        amount: i64,
    },
    Dispute {
        client_id: u16,
        referred_tx_id: u32,
    },
    Resolve {
        client_id: u16,
        referred_tx_id: u32,
    },
    Chargeback {
        client_id: u16,
        referred_tx_id: u32,
    },
}
