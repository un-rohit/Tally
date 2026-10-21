use crate::domain::money::Paise;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoucherStatus {
    Draft,
    Posted,
    Cancelled,
}

impl VoucherStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            VoucherStatus::Draft => "Draft",
            VoucherStatus::Posted => "Posted",
            VoucherStatus::Cancelled => "Cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Draft" => Some(VoucherStatus::Draft),
            "Posted" => Some(VoucherStatus::Posted),
            "Cancelled" => Some(VoucherStatus::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryType {
    Dr,
    Cr,
}

impl EntryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryType::Dr => "Dr",
            EntryType::Cr => "Cr",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Dr" => Some(EntryType::Dr),
            "Cr" => Some(EntryType::Cr),
            _ => None,
        }
    }

    pub fn opposite(&self) -> Self {
        match self {
            EntryType::Dr => EntryType::Cr,
            EntryType::Cr => EntryType::Dr,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherDraft {
    pub company_id: String,
    pub fy_id: String,
    pub voucher_type_id: String,
    pub voucher_number: String,
    pub voucher_date: NaiveDate,
    pub reference_no: Option<String>,
    pub reference_date: Option<NaiveDate>,
    pub narration: Option<String>,
    pub entries: Vec<VoucherEntryDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherEntryDraft {
    pub ledger_id: String,
    pub entry_type: EntryType,
    pub amount: Paise,
    pub narration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voucher {
    pub voucher_id: String,
    pub company_id: String,
    pub fy_id: String,
    pub voucher_type_id: String,
    pub voucher_number: String,
    pub voucher_date: NaiveDate,
    pub reference_no: Option<String>,
    pub reference_date: Option<NaiveDate>,
    pub narration: Option<String>,
    pub total_amount: Paise,
    pub status: VoucherStatus,
    pub posted_at: Option<String>,
    pub posted_by_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherEntry {
    pub entry_id: String,
    pub voucher_id: String,
    pub line_number: i32,
    pub ledger_id: String,
    pub entry_type: EntryType,
    pub amount: Paise,
    pub line_narration: Option<String>,
}

impl VoucherDraft {
    /// Validates double-entry equality: sum of debits must equal sum of credits.
    pub fn validate_balance(&self) -> (Paise, Paise, bool) {
        let mut total_dr = 0i64;
        let mut total_cr = 0i64;

        for entry in &self.entries {
            match entry.entry_type {
                EntryType::Dr => total_dr = total_dr.saturating_add(entry.amount.0),
                EntryType::Cr => total_cr = total_cr.saturating_add(entry.amount.0),
            }
        }

        let is_balanced = total_dr > 0 && total_dr == total_cr;
        (Paise(total_dr), Paise(total_cr), is_balanced)
    }

    /// Creates a compensating reversal voucher draft (swaps Debits and Credits).
    pub fn create_reversal(
        &self,
        new_voucher_number: &str,
        reversal_date: NaiveDate,
        reason: &str,
    ) -> VoucherDraft {
        let reversed_entries = self
            .entries
            .iter()
            .map(|e| VoucherEntryDraft {
                ledger_id: e.ledger_id.clone(),
                entry_type: e.entry_type.opposite(),
                amount: e.amount,
                narration: Some(format!("Reversal of {}: {}", self.voucher_number, reason)),
            })
            .collect();

        VoucherDraft {
            company_id: self.company_id.clone(),
            fy_id: self.fy_id.clone(),
            voucher_type_id: self.voucher_type_id.clone(),
            voucher_number: new_voucher_number.to_string(),
            voucher_date: reversal_date,
            reference_no: Some(format!("REV-{}", self.voucher_number)),
            reference_date: Some(self.voucher_date),
            narration: Some(format!("Reversal entry for voucher {}. Reason: {}", self.voucher_number, reason)),
            entries: reversed_entries,
        }
    }
}
