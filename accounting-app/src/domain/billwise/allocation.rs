use crate::domain::money::Paise;
use crate::domain::voucher::EntryType;
use crate::error::{AccountingError, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillRefType {
    NewRef,
    AgstRef,
    Advance,
    OnAccount,
}

impl BillRefType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NewRef => "NewRef",
            Self::AgstRef => "AgstRef",
            Self::Advance => "Advance",
            Self::OnAccount => "OnAccount",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "NewRef" => Ok(Self::NewRef),
            "AgstRef" => Ok(Self::AgstRef),
            "Advance" => Ok(Self::Advance),
            "OnAccount" => Ok(Self::OnAccount),
            _ => Err(AccountingError::Validation(format!(
                "Invalid bill reference type: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillAllocation {
    pub allocation_id: String,
    pub company_id: Option<String>,
    pub entry_id: String,
    pub ledger_id: String,
    pub ref_type: BillRefType,
    pub ref_name: String,
    pub credit_days: u32,
    pub due_date: Option<NaiveDate>,
    pub amount_paise: Paise,
    pub entry_type: EntryType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillAllocationDraft {
    pub ledger_id: String,
    pub ref_type: BillRefType,
    pub ref_name: String,
    pub credit_days: u32,
    pub due_date: Option<NaiveDate>,
    pub amount_paise: Paise,
    pub entry_type: EntryType,
}
