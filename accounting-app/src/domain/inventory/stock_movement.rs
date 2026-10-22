use crate::domain::money::Paise;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MovementType {
    In,
    Out,
    TransferIn,
    TransferOut,
    AdjustmentIn,
    AdjustmentOut,
}

impl MovementType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::In => "IN",
            Self::Out => "OUT",
            Self::TransferIn => "TRANSFER_IN",
            Self::TransferOut => "TRANSFER_OUT",
            Self::AdjustmentIn => "ADJUSTMENT_IN",
            Self::AdjustmentOut => "ADJUSTMENT_OUT",
        }
    }

    pub fn is_inward(&self) -> bool {
        matches!(self, Self::In | Self::TransferIn | Self::AdjustmentIn)
    }

    pub fn is_outward(&self) -> bool {
        matches!(self, Self::Out | Self::TransferOut | Self::AdjustmentOut)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryTransaction {
    pub inv_tx_id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub voucher_entry_id: Option<String>,
    pub item_id: String,
    pub warehouse_id: String,
    pub movement_type: String,
    pub quantity_milli: i64,
    pub rate_paise: Paise,
    pub amount_paise: Paise,
    pub transaction_date: String,
    pub batch_number: Option<String>,
    pub expiry_date: Option<String>,
}
