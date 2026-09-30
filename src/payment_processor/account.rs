use crate::types::Money;

/// A client's available, held, and total funds, and whether the account is locked.
#[derive(Debug, Clone, Default)]
pub struct Account {
    total: Money,
    available: Money,
    held: Money,
    is_locked: bool,
}

/// Access guard to an account that is guaranteed to be active.
///
/// Note that the exclusive borrow makes sure that no one else accesses this
/// account, and thus nothing can lock it until we're done with it.
pub(super) struct ActiveAccountGuard<'a> {
    account: &'a mut Account,
}

impl Account {
    /// Returns the total funds for the account.
    ///
    /// Total funds include funds that are available or held.
    pub fn total(&self) -> Money {
        self.total
    }

    /// Returns the funds available for withdrawal.
    pub fn available(&self) -> Money {
        self.available
    }

    /// Returns the funds currently held for disputes.
    pub fn held(&self) -> Money {
        self.held
    }

    /// Returns whether this account is locked.
    pub fn is_locked(&self) -> bool {
        self.is_locked
    }

    pub(super) fn try_active(&mut self) -> anyhow::Result<ActiveAccountGuard<'_>> {
        if self.is_locked {
            anyhow::bail!("account is locked");
        }
        Ok(ActiveAccountGuard { account: self })
    }
}

impl ActiveAccountGuard<'_> {
    /// Credits a deposit to available and total funds.
    ///
    /// Returns an error if any of the balances would overflow.
    pub(super) fn deposit(&mut self, amount: Money) -> anyhow::Result<()> {
        let total = self
            .account
            .total
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("total balance overflow"))?;
        let available = self
            .account
            .available
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("available balance overflow"))?;
        self.account.total = total;
        self.account.available = available;
        Ok(())
    }

    /// Debits a withdrawal from available and total funds.
    ///
    /// Returns an error if available funds are insufficient.
    pub(super) fn withdrawal(&mut self, amount: Money) -> anyhow::Result<()> {
        if self.account.available < amount {
            anyhow::bail!("insufficient funds");
        }

        let total = self
            .account
            .total
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("total balance overflow"))?;
        let available = self
            .account
            .available
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("available balance overflow"))?;
        self.account.total = total;
        self.account.available = available;
        Ok(())
    }

    /// Holds disputed deposit funds, moving them from available to held.
    ///
    /// Returns an error if the held balance would overflow.
    pub(super) fn dispute(&mut self, amount: Money) -> anyhow::Result<()> {
        let available = self
            .account
            .available
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("available balance overflow"))?;
        let held = self
            .account
            .held
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("held balance overflow"))?;
        self.account.available = available;
        self.account.held = held;
        Ok(())
    }

    /// Releases disputed deposit funds from held back to available.
    ///
    /// Returns an error if the available balance would overflow.
    pub(super) fn resolve(&mut self, amount: Money) -> anyhow::Result<()> {
        let available = self
            .account
            .available
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("available balance overflow"))?;
        let held = self
            .account
            .held
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("held balance overflow"))?;
        self.account.available = available;
        self.account.held = held;
        Ok(())
    }

    /// Withdraws held funds for a chargeback and locks the account.
    ///
    /// Note that a chargeback consumes the active account guard, and thus
    /// no other operation can be performed in sequence.
    pub(super) fn chargeback(self, amount: Money) -> anyhow::Result<()> {
        let held = self
            .account
            .held
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("held balance overflow"))?;
        let total = self
            .account
            .total
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("total balance overflow"))?;
        self.account.held = held;
        self.account.total = total;
        self.account.is_locked = true;
        Ok(())
    }
}
