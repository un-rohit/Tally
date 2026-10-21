use crate::error::{AccountingError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};

/// Exact integer monetary amount in minor units (Paise for INR, Cents for USD).
/// ₹1.00 = 100 paise.
/// Prevents all IEEE-754 floating point precision issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Paise(pub i64);

impl Paise {
    pub const ZERO: Paise = Paise(0);

    pub fn from_rupees(rupees: i64) -> Result<Self> {
        rupees
            .checked_mul(100)
            .map(Paise)
            .ok_or_else(|| AccountingError::MathOverflow("Rupee to paise overflow".into()))
    }

    pub fn from_decimal_rupees(dec: Decimal) -> Result<Self> {
        let paise_dec = (dec * Decimal::from(100)).round();
        let val: i64 = paise_dec
            .try_into()
            .map_err(|_| AccountingError::MathOverflow("Decimal to paise conversion overflow".into()))?;
        Ok(Paise(val))
    }

    pub fn to_decimal_rupees(&self) -> Decimal {
        Decimal::from(self.0) / Decimal::from(100)
    }

    pub fn checked_add(self, other: Paise) -> Result<Paise> {
        self.0
            .checked_add(other.0)
            .map(Paise)
            .ok_or_else(|| AccountingError::MathOverflow("Addition overflow".into()))
    }

    pub fn checked_sub(self, other: Paise) -> Result<Paise> {
        self.0
            .checked_sub(other.0)
            .map(Paise)
            .ok_or_else(|| AccountingError::MathOverflow("Subtraction overflow".into()))
    }

    pub fn checked_mul_qty(self, qty_milli: i64) -> Result<Paise> {
        // qty_milli is quantity in thousandths (1.000 = 1000)
        let total = (self.0 as i128)
            .checked_mul(qty_milli as i128)
            .ok_or_else(|| AccountingError::MathOverflow("Multiplication overflow".into()))?;
        let res = (total / 1000) as i64;
        Ok(Paise(res))
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn is_positive(&self) -> bool {
        self.0 > 0
    }

    pub fn is_negative(&self) -> bool {
        self.0 < 0
    }

    pub fn abs(&self) -> Paise {
        Paise(self.0.abs())
    }

    pub fn format_inr(&self) -> String {
        let is_neg = self.0 < 0;
        let abs_val = self.0.abs();
        let rupees = abs_val / 100;
        let paise = abs_val % 100;

        // Format Indian Numbering System (Lakhs, Crores)
        let r_str = rupees.to_string();
        let formatted_r = if r_str.len() <= 3 {
            r_str
        } else {
            let last_three = &r_str[r_str.len() - 3..];
            let remaining = &r_str[..r_str.len() - 3];
            let mut groups = Vec::new();
            let mut rem = remaining;
            while rem.len() > 2 {
                groups.push(&rem[rem.len() - 2..]);
                rem = &rem[..rem.len() - 2];
            }
            if !rem.is_empty() {
                groups.push(rem);
            }
            groups.reverse();
            format!("{},{}", groups.join(","), last_three)
        };

        let sign = if is_neg { "-" } else { "" };
        format!("{}₹{}.{:02}", sign, formatted_r, paise)
    }
}

impl Add for Paise {
    type Output = Result<Paise>;
    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs)
    }
}

impl Sub for Paise {
    type Output = Result<Paise>;
    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs)
    }
}

impl fmt::Display for Paise {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_inr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_decimal_safety() {
        // 0.10 + 0.20 in float is 0.30000000000000004
        // In Paise it must be exact
        let p1 = Paise::from_decimal_rupees(dec!(0.10)).unwrap();
        let p2 = Paise::from_decimal_rupees(dec!(0.20)).unwrap();
        let sum = (p1 + p2).unwrap();
        assert_eq!(sum, Paise(30));
        assert_eq!(sum.to_decimal_rupees(), dec!(0.30));
    }

    #[test]
    fn test_inr_formatting() {
        let p = Paise(123456789); // ₹12,34,567.89
        assert_eq!(p.format_inr(), "₹12,34,567.89");

        let neg = Paise(-50050); // -₹500.50
        assert_eq!(neg.format_inr(), "-₹500.50");
    }
}
