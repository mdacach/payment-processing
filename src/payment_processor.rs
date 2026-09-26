use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub(crate) struct Account {
    total: i64,
    available: i64,
    held: i64,
    is_locked: bool,
}

#[derive(Debug, Default)]
pub(crate) struct PaymentProcessor {
    accounts: BTreeMap<u16, Account>,
}

impl PaymentProcessor {
    pub(crate) fn account(&self, client_id: u16) -> Option<&Account> {
        self.accounts.get(&client_id)
    }
}
