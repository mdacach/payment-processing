use std::collections::BTreeMap;

use crate::event::Event;

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
    pub(crate) fn on_event(&mut self, event: Event) -> anyhow::Result<()> {
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
            } => todo!(),
            Event::Resolve {
                client_id,
                referred_tx_id,
            } => todo!(),
            Event::Chargeback {
                client_id,
                referred_tx_id,
            } => todo!(),
        }
    }

    fn handle_deposit(&mut self, client_id: u16, tx_id: u32, amount: i64) -> anyhow::Result<()> {
        let account = self.accounts.entry(client_id).or_default();
        account.total += amount;
        account.available += amount;

        Ok(())
    }

    fn handle_withdrawal(&mut self, client_id: u16, tx_id: u32, amount: i64) -> anyhow::Result<()> {
        let account = self.accounts.entry(client_id).or_default();
        if account.available < amount {
            anyhow::bail!("insufficient funds");
        }

        account.total -= amount;
        account.available -= amount;

        Ok(())
    }
}

impl PaymentProcessor {
    pub(crate) fn account(&self, client_id: u16) -> Option<&Account> {
        self.accounts.get(&client_id)
    }
}
