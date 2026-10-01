use std::{fmt, str::FromStr};

use primitive_fixed_point_decimal::ConstScaleFpdec;

// TODO: consider using newtypes instead of type aliases. a newtype is harder to get wrong.
pub type TxId = u32;
pub type ClientId = u16;

/// A validated amount from a transaction.
///
/// Valid amounts are equal or greater than 0.0001, the smallest decimal
/// precision supported by the system. Valid amounts have no upper limit
/// (apart from their internal representation's maximum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionAmount(ConstScaleFpdec<i64, 4>);

/// An account balance.
///
/// Note that an account's balance can become negative after disputes or chargebacks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Balance(ConstScaleFpdec<i64, 4>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TransactionAmountError {
    #[error("invalid amount")]
    InvalidDecimal,
    #[error("amount must be at least 0.0001")]
    BelowMinimum,
}

impl TransactionAmount {
    fn new(value: ConstScaleFpdec<i64, 4>) -> Result<Self, TransactionAmountError> {
        if value <= ConstScaleFpdec::default() {
            return Err(TransactionAmountError::BelowMinimum);
        }
        Ok(Self(value))
    }

    /// Constructs an amount from ten-thousandths of a currency unit.
    pub fn try_from_mantissa(mantissa: i64) -> Result<Self, TransactionAmountError> {
        Self::new(ConstScaleFpdec::from_mantissa(mantissa))
    }
}

impl FromStr for TransactionAmount {
    type Err = TransactionAmountError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let value = raw
            .parse()
            .map_err(|_| TransactionAmountError::InvalidDecimal)?;
        Self::new(value)
    }
}

impl fmt::Display for TransactionAmount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl Balance {
    /// Constructs a balance from ten-thousandths of a currency unit.
    pub fn from_mantissa(mantissa: i64) -> Self {
        Self(ConstScaleFpdec::from_mantissa(mantissa))
    }

    pub(crate) fn covers(self, amount: TransactionAmount) -> bool {
        self.0 >= amount.0
    }

    pub(crate) fn checked_add_amount(self, amount: TransactionAmount) -> Option<Self> {
        self.0.checked_add(amount.0).map(Self)
    }

    pub(crate) fn checked_sub_amount(self, amount: TransactionAmount) -> Option<Self> {
        self.0.checked_sub(amount.0).map(Self)
    }
}

// Adding and subtracting balances are used for testing, while production code
// should always add or subtract transaction amounts instead.
#[cfg(test)]
impl Balance {
    pub(crate) fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub(crate) fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}

impl fmt::Display for Balance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_balance_arithmetic_keeps_four_decimal_places() {
        let balance = Balance::default()
            .checked_add_amount("1.0001".parse().unwrap())
            .unwrap()
            .checked_sub_amount("0.0001".parse().unwrap())
            .unwrap();

        assert_eq!(balance, Balance::from_mantissa(10_000));
    }

    #[test]
    fn minimum_and_large_transaction_amounts_are_accepted() {
        for mantissa in [1, 100_000_000_001] {
            let amount = TransactionAmount::try_from_mantissa(mantissa).unwrap();
            let balance = Balance::default().checked_add_amount(amount).unwrap();
            assert_eq!(balance.checked_sub_amount(amount), Some(Balance::default()));
        }
        assert_eq!(
            TransactionAmount::try_from_mantissa(0),
            Err(TransactionAmountError::BelowMinimum)
        );
    }
}
