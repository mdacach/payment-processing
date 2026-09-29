use crate::types::Money;

// TODO: consider a state machine design here, where a locked account is a final state.

/// A client's available, held, and total funds, and whether the account is locked.
#[derive(Debug, Clone, Default)]
pub struct Account {
    total: Money,
    available: Money,
    held: Money,
    is_locked: bool,
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

    /// Credits a deposit to available and total funds.
    ///
    /// Returns an error if any of the balances would overflow.
    pub(super) fn deposit(&mut self, amount: Money) -> anyhow::Result<()> {
        let total = self
            .total
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("total balance overflow"))?;
        let available = self
            .available
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("available balance overflow"))?;
        self.total = total;
        self.available = available;
        Ok(())
    }

    /// Debits a withdrawal from available and total funds.
    ///
    /// Returns an error if available funds are insufficient.
    pub(super) fn withdrawal(&mut self, amount: Money) -> anyhow::Result<()> {
        if self.available < amount {
            anyhow::bail!("insufficient funds");
        }

        let total = self
            .total
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("total balance overflow"))?;
        let available = self
            .available
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("available balance overflow"))?;
        self.total = total;
        self.available = available;
        Ok(())
    }

    /// Holds disputed deposit funds, moving them from available to held.
    ///
    /// Returns an error if the held balance would overflow.
    pub(super) fn dispute(&mut self, amount: Money) -> anyhow::Result<()> {
        let available = self
            .available
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("available balance overflow"))?;
        let held = self
            .held
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("held balance overflow"))?;
        self.available = available;
        self.held = held;
        Ok(())
    }

    /// Releases disputed deposit funds from held back to available.
    ///
    /// Returns an error if the available balance would overflow.
    pub(super) fn resolve(&mut self, amount: Money) -> anyhow::Result<()> {
        let available = self
            .available
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("available balance overflow"))?;
        let held = self
            .held
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("held balance overflow"))?;
        self.available = available;
        self.held = held;
        Ok(())
    }

    /// Withdraws held funds for a chargeback and locks the account.
    pub(super) fn chargeback(&mut self, amount: Money) -> anyhow::Result<()> {
        let held = self
            .held
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("held balance overflow"))?;
        let total = self
            .total
            .checked_sub(amount)
            .ok_or_else(|| anyhow::anyhow!("total balance overflow"))?;
        self.held = held;
        self.total = total;
        self.is_locked = true;
        Ok(())
    }
}
