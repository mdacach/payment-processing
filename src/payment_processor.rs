use std::collections::BTreeMap;

use crate::{
    event::Event,
    types::{ClientId, Money, TxId},
};

#[derive(Debug, Default)]
pub(crate) struct Account {
    // TODO: decide whether to allow negative totals. a possible scenario that would
    //       create a negative total is a deposit that is withdrawn and then disputed.
    total: Money,
    available: Money,
    held: Money,
    is_locked: bool,
}

#[derive(Debug, Default)]
pub(crate) struct DepositInfo {
    client_id: ClientId,
    disputed_count: u16,
    amount: Money,
}

#[derive(Debug, Default)]
pub(crate) struct PaymentProcessor {
    accounts: BTreeMap<ClientId, Account>,
    deposits: BTreeMap<TxId, DepositInfo>,
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
            } => self.handle_dispute(client_id, referred_tx_id),
            Event::Resolve {
                client_id,
                referred_tx_id,
            } => self.handle_resolve(client_id, referred_tx_id),
            Event::Chargeback {
                client_id,
                referred_tx_id,
            } => todo!(),
        }
    }

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
            disputed_count: 0,
            amount,
        };
        self.deposits.insert(tx_id, info);

        Ok(())
    }

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

    fn handle_dispute(&mut self, client_id: ClientId, referred_tx_id: TxId) -> anyhow::Result<()> {
        let account = self.accounts.entry(client_id).or_default();

        match self.deposits.get_mut(&referred_tx_id) {
            Some(deposit_info) => {
                if deposit_info.client_id != client_id {
                    // Very weird, huh!
                    anyhow::bail!("disputed transaction with wrong client id!");
                }

                // TODO: reason through what happens with multiple disputes.
                //       might not make sense because associated funds would be
                //       held multiple times...
                // Because multiple disputes are allowed, we increment a counter
                // instead of simply setting a boolean flag.
                deposit_info.disputed_count += 1;

                // TODO: double-check whether to allow negative available funds,
                //       in the case where a disputed deposit has already been
                //       withdrawn.
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

    fn handle_resolve(&mut self, client_id: ClientId, referred_tx_id: TxId) -> anyhow::Result<()> {
        let account = self.accounts.entry(client_id).or_default();

        match self.deposits.get_mut(&referred_tx_id) {
            Some(deposit_info) => {
                if deposit_info.client_id != client_id {
                    // Very weird, huh!
                    anyhow::bail!("resolve transaction with wrong client id!");
                }

                if deposit_info.disputed_count == 0 {
                    anyhow::bail!("resolve non-disputed deposit");
                }

                // TODO: yeah, probably can't have multiple disputes. will need to change this.
                deposit_info.disputed_count -= 1;

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
}

impl PaymentProcessor {
    pub(crate) fn account(&self, client_id: u16) -> Option<&Account> {
        self.accounts.get(&client_id)
    }
}

#[cfg(test)]
mod property_tests;
