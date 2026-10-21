use crate::error::{AccountingError, Result};
use serde::{Deserialize, Serialize};

pub const GSTIN_CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Gstin(String);

impl Gstin {
    /// Validates and constructs a verified 15-character GSTIN.
    pub fn new(raw: &str) -> Result<Self> {
        let cleaned = raw.trim().to_uppercase();
        Self::validate_format(&cleaned)?;
        Self::validate_checksum(&cleaned)?;
        Ok(Self(cleaned))
    }

    /// Constructs without checksum validation for testing or non-standard registrations.
    pub fn new_unchecked(raw: &str) -> Result<Self> {
        let cleaned = raw.trim().to_uppercase();
        Self::validate_format(&cleaned)?;
        Ok(Self(cleaned))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn state_code(&self) -> &str {
        &self.0[0..2]
    }

    pub fn pan(&self) -> &str {
        &self.0[2..12]
    }

    pub fn entity_code(&self) -> char {
        self.0.chars().nth(12).unwrap()
    }

    pub fn check_digit(&self) -> char {
        self.0.chars().nth(14).unwrap()
    }

    /// Validates the 15-character syntactic format of the GSTIN.
    pub fn validate_format(gstin: &str) -> Result<()> {
        if gstin.len() != 15 {
            return Err(AccountingError::Validation(format!(
                "Invalid GSTIN length {}: expected exactly 15 characters",
                gstin.len()
            )));
        }

        let bytes = gstin.as_bytes();

        // 1. State code (chars 0..1): 2 numeric digits
        if !bytes[0].is_ascii_digit() || !bytes[1].is_ascii_digit() {
            return Err(AccountingError::Validation(format!(
                "Invalid GSTIN state code '{}': first 2 characters must be numeric digits",
                &gstin[0..2]
            )));
        }

        let state_num: u32 = gstin[0..2]
            .parse()
            .map_err(|_| AccountingError::Validation("Invalid state code integer".to_string()))?;
        if !(1..=38).contains(&state_num) && state_num != 97 && state_num != 99 {
            return Err(AccountingError::Validation(format!(
                "Invalid GSTIN state code '{}': out of valid range (01-38, 97, 99)",
                &gstin[0..2]
            )));
        }

        // 2. PAN (chars 2..11): 5 uppercase letters, 4 digits, 1 uppercase letter
        for (i, &b) in bytes.iter().enumerate().take(7).skip(2) {
            if !b.is_ascii_uppercase() {
                return Err(AccountingError::Validation(format!(
                    "Invalid GSTIN PAN format at position {}: expected uppercase letter",
                    i + 1
                )));
            }
        }
        for (i, &b) in bytes.iter().enumerate().take(11).skip(7) {
            if !b.is_ascii_digit() {
                return Err(AccountingError::Validation(format!(
                    "Invalid GSTIN PAN format at position {}: expected numeric digit",
                    i + 1
                )));
            }
        }
        if !bytes[11].is_ascii_uppercase() {
            return Err(AccountingError::Validation(
                "Invalid GSTIN PAN format at position 12: expected uppercase letter".to_string(),
            ));
        }

        // 3. Entity code (char 12): 1-9 or A-Z
        if !bytes[12].is_ascii_alphanumeric() {
            return Err(AccountingError::Validation(
                "Invalid GSTIN entity code at position 13: expected alphanumeric character".to_string(),
            ));
        }

        // 4. Default character 'Z' (char 13)
        if bytes[13] != b'Z' {
            return Err(AccountingError::Validation(format!(
                "Invalid GSTIN at position 14: expected 'Z', found '{}'",
                bytes[13] as char
            )));
        }

        // 5. Checksum character (char 14): alphanumeric
        if !bytes[14].is_ascii_alphanumeric() {
            return Err(AccountingError::Validation(
                "Invalid GSTIN check digit at position 15: expected alphanumeric character".to_string(),
            ));
        }

        Ok(())
    }

    /// Computes the 15th check digit for a 14-character GSTIN prefix using official GSTN Mod-36.
    pub fn calculate_checksum(base14: &str) -> Result<char> {
        if base14.len() != 14 {
            return Err(AccountingError::Validation(format!(
                "Input to calculate_checksum must be 14 characters, got {}",
                base14.len()
            )));
        }

        let mut factor = 1;
        let mut total: usize = 0;
        let mod_len = GSTIN_CHARS.len(); // 36

        for ch in base14.chars() {
            let digit = GSTIN_CHARS
                .find(ch)
                .ok_or_else(|| AccountingError::Validation(format!("Invalid character '{}' in GSTIN", ch)))?;

            let product = digit * factor;
            let quotient = product / mod_len;
            let remainder = product % mod_len;
            total += quotient + remainder;

            factor = if factor == 1 { 2 } else { 1 };
        }

        let remainder = total % mod_len;
        let check_index = (mod_len - remainder) % mod_len;
        Ok(GSTIN_CHARS.chars().nth(check_index).unwrap())
    }

    /// Validates the 15th character against the official calculated check digit.
    pub fn validate_checksum(gstin: &str) -> Result<()> {
        let expected = Self::calculate_checksum(&gstin[0..14])?;
        let actual = gstin.chars().nth(14).unwrap();
        if expected != actual {
            return Err(AccountingError::Validation(format!(
                "GSTIN '{}' failed Mod-36 checksum verification: expected '{}', found '{}'",
                gstin, expected, actual
            )));
        }
        Ok(())
    }
}

impl std::fmt::Display for Gstin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_gstin() {
        // Valid test GSTINs verified with official Mod-36 formula
        let gstin1 = Gstin::new("27AAPFU0939F1ZV").unwrap();
        assert_eq!(gstin1.state_code(), "27");
        assert_eq!(gstin1.pan(), "AAPFU0939F");

        let gstin2 = Gstin::new("27AAAAA0000A1Z2").unwrap();
        assert_eq!(gstin2.state_code(), "27");

        let gstin3 = Gstin::new("29ABCDE1234F1ZW").unwrap();
        assert_eq!(gstin3.state_code(), "29");
    }

    #[test]
    fn test_invalid_gstin_checksum() {
        let err = Gstin::new("27AAPFU0939F1ZX");
        assert!(err.is_err(), "Wrong check digit must fail");
    }

    #[test]
    fn test_invalid_gstin_format() {
        assert!(Gstin::new("27AAAAA0000A1Z").is_err(), "Too short");
        assert!(Gstin::new("XXAAAAA0000A1Z2").is_err(), "Invalid state digits");
        assert!(Gstin::new("99AAAAA0000A1Z2").is_ok(), "State 99 center valid");
    }
}
