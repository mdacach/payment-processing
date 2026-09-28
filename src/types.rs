use std::{error::Error, fmt, str::FromStr};

pub(crate) type TxId = u32;
pub(crate) type ClientId = u16;

/// An amount stored in ten-thousandths of a currency unit.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct Money(i64);

impl Default for Money {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum MoneyError {
    InvalidDecimal,
    TooManyFractionalDigits,
    OutOfRange,
    NonFinite,
}

impl fmt::Display for MoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDecimal => write!(f, "invalid decimal amount"),
            Self::TooManyFractionalDigits => {
                write!(f, "amount has more than four fractional digits")
            }
            Self::OutOfRange => write!(f, "amount is out of range"),
            Self::NonFinite => write!(f, "amount must be finite"),
        }
    }
}

impl Error for MoneyError {}

impl Money {
    pub(crate) const ZERO: Self = Self(0);

    pub(crate) const fn from_minor_units(units: i64) -> Self {
        Self(units)
    }

    pub(crate) const fn minor_units(self) -> i64 {
        self.0
    }

    pub(crate) fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self::from_minor_units)
    }

    pub(crate) fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self::from_minor_units)
    }
}

impl FromStr for Money {
    type Err = MoneyError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (negative, unsigned) = match input.as_bytes().first() {
            Some(b'-') => (true, &input[1..]),
            Some(b'+') => (false, &input[1..]),
            _ => (false, input),
        };
        let (whole, fraction) = match unsigned.split_once('.') {
            Some((whole, fraction)) => (whole, Some(fraction)),
            None => (unsigned, None),
        };
        if whole.is_empty()
            || !whole.bytes().all(|digit| digit.is_ascii_digit())
            || fraction.is_some_and(|digits| {
                digits.is_empty() || !digits.bytes().all(|digit| digit.is_ascii_digit())
            })
        {
            return Err(MoneyError::InvalidDecimal);
        }
        let fraction = fraction.unwrap_or("");
        if fraction.len() > 4 {
            return Err(MoneyError::TooManyFractionalDigits);
        }
        let whole = parse_digits(whole)?;
        let fraction_value = parse_digits(fraction)?;
        let padding = 10_i128.pow((4 - fraction.len()) as u32);
        let magnitude = whole
            .checked_mul(10_000)
            .and_then(|value| value.checked_add(fraction_value * padding))
            .ok_or(MoneyError::OutOfRange)?;
        let signed = if negative { -magnitude } else { magnitude };
        i64::try_from(signed)
            .map(Self::from_minor_units)
            .map_err(|_| MoneyError::OutOfRange)
    }
}

fn parse_digits(digits: &str) -> Result<i128, MoneyError> {
    digits.bytes().try_fold(0_i128, |value, digit| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add((digit - b'0') as i128))
            .ok_or(MoneyError::OutOfRange)
    })
}

impl TryFrom<f64> for Money {
    type Error = MoneyError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() {
            return Err(MoneyError::NonFinite);
        }
        // This recovers the decimal carried by the float, not necessarily
        // the caller's original decimal text.
        let decimal = value.to_string();
        expand_scientific(&decimal)?.parse()
    }
}

fn expand_scientific(decimal: &str) -> Result<String, MoneyError> {
    let Some((mantissa, exponent)) = decimal.split_once(['e', 'E']) else {
        return Ok(decimal.to_owned());
    };
    let exponent: i32 = exponent.parse().map_err(|_| MoneyError::InvalidDecimal)?;
    let (sign, mantissa) = match mantissa.as_bytes().first() {
        Some(b'-') => ("-", &mantissa[1..]),
        Some(b'+') => ("+", &mantissa[1..]),
        _ => ("", mantissa),
    };
    let decimal_point = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits: String = mantissa.chars().filter(|&c| c != '.').collect();
    let new_point = decimal_point
        .checked_add(exponent)
        .ok_or(MoneyError::OutOfRange)?;
    if new_point <= 0 {
        Ok(format!(
            "{sign}0.{}{digits}",
            "0".repeat((-new_point) as usize)
        ))
    } else if new_point as usize >= digits.len() {
        Ok(format!(
            "{sign}{digits}{}",
            "0".repeat(new_point as usize - digits.len())
        ))
    } else {
        let (whole, fraction) = digits.split_at(new_point as usize);
        Ok(format!("{sign}{whole}.{fraction}"))
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magnitude = self.minor_units().unsigned_abs();
        let sign = if self.minor_units() < 0 { "-" } else { "" };
        write!(f, "{sign}{}.{:04}", magnitude / 10_000, magnitude % 10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_boundaries() {
        for (input, units, output) in [
            ("0", 0, "0.0000"),
            ("+1.5", 15_000, "1.5000"),
            ("1.5050", 15_050, "1.5050"),
            ("-0.0001", -1, "-0.0001"),
            ("922337203685477.5807", i64::MAX, "922337203685477.5807"),
            ("-922337203685477.5808", i64::MIN, "-922337203685477.5808"),
        ] {
            let money: Money = input.parse().unwrap();
            assert_eq!(money.minor_units(), units);
            assert_eq!(money.to_string(), output);
        }
        for input in ["", ".1", "1.", "1.2.3", " 1", "1e2", "1.23456"] {
            assert!(input.parse::<Money>().is_err(), "accepted {input:?}");
        }
        assert_eq!(
            "922337203685477.5808".parse::<Money>(),
            Err(MoneyError::OutOfRange)
        );
        assert_eq!(
            "-922337203685477.5809".parse::<Money>(),
            Err(MoneyError::OutOfRange)
        );
    }

    #[test]
    fn float_conversion() {
        assert_eq!(
            Money::try_from(1.5050_f64),
            Ok(Money::from_minor_units(15_050))
        );
        assert_eq!(Money::try_from(0.0001_f64), Ok(Money::from_minor_units(1)));
        assert!(Money::try_from(0.1_f64 + 0.2_f64).is_err());
        assert!(Money::try_from(1.23456_f64).is_err());
        assert_eq!(Money::try_from(f64::NAN), Err(MoneyError::NonFinite));
        assert_eq!(Money::try_from(f64::INFINITY), Err(MoneyError::NonFinite));
        assert!(Money::try_from(1e100).is_err());
        assert_eq!(expand_scientific("1.5e-4").unwrap(), "0.00015");
    }

    #[test]
    fn checked_arithmetic() {
        assert_eq!(
            Money::ZERO.checked_add(Money::from_minor_units(1)),
            Some(Money::from_minor_units(1))
        );
        assert_eq!(
            Money::ZERO.checked_sub(Money::from_minor_units(1)),
            Some(Money::from_minor_units(-1))
        );
        assert_eq!(
            Money::from_minor_units(i64::MAX).checked_add(Money::from_minor_units(1)),
            None
        );
        assert_eq!(
            Money::from_minor_units(i64::MIN).checked_sub(Money::from_minor_units(1)),
            None
        );
    }
}
