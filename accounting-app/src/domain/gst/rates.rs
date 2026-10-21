use crate::error::{AccountingError, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GstRate(pub u32); // Stored in basis points (e.g., 1800 = 18.00%, 500 = 5.00%)

impl GstRate {
    pub const NIL: GstRate = GstRate(0);
    pub const RATE_0_25: GstRate = GstRate(25);
    pub const RATE_3: GstRate = GstRate(300);
    pub const RATE_5: GstRate = GstRate(500);
    pub const RATE_12: GstRate = GstRate(1200);
    pub const RATE_18: GstRate = GstRate(1800);
    pub const RATE_28: GstRate = GstRate(2800);

    pub fn from_basis_points(bp: u32) -> Self {
        Self(bp)
    }

    pub fn from_percent(percent: u32) -> Self {
        Self(percent * 100)
    }

    pub fn basis_points(&self) -> u32 {
        self.0
    }

    /// Splits rate into (CGST rate bp, SGST rate bp) for intra-state supply.
    pub fn split_intra_rates(&self) -> (u32, u32) {
        let half = self.0 / 2;
        (half, self.0 - half)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn to_display_string(&self) -> String {
        let pct = self.0 / 100;
        let rem = self.0 % 100;
        if rem == 0 {
            format!("{}%", pct)
        } else {
            format!("{}.{:02}%", pct, rem)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsnSacItem {
    pub code: String,
    pub description: String,
    pub item_type: String, // "Goods" or "Services"
    pub default_rate: GstRate,
}

impl HsnSacItem {
    /// Look up an HSN/SAC code from the directory.
    pub fn find_by_code(conn: &Connection, code: &str) -> Result<Option<Self>> {
        let mut stmt = conn.prepare(
            "SELECT code, description, item_type, default_gst_basis_points 
             FROM hsn_sac_directory WHERE code = ?1",
        )?;

        let mut rows = stmt.query(params![code])?;
        if let Some(row) = rows.next()? {
            let bp: i64 = row.get(3)?;
            Ok(Some(Self {
                code: row.get(0)?,
                description: row.get(1)?,
                item_type: row.get(2)?,
                default_rate: GstRate::from_basis_points(bp as u32),
            }))
        } else {
            Ok(None)
        }
    }

    /// Insert or update an HSN/SAC code in the directory.
    pub fn upsert(
        conn: &Connection,
        code: &str,
        description: &str,
        item_type: &str,
        rate: GstRate,
    ) -> Result<()> {
        if item_type != "Goods" && item_type != "Services" {
            return Err(AccountingError::Validation(
                "item_type must be either 'Goods' or 'Services'".to_string(),
            ));
        }

        conn.execute(
            "INSERT INTO hsn_sac_directory (code, description, item_type, default_gst_basis_points) 
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(code) DO UPDATE SET 
                description = excluded.description,
                item_type = excluded.item_type,
                default_gst_basis_points = excluded.default_gst_basis_points",
            params![code, description, item_type, rate.basis_points() as i64],
        )?;

        Ok(())
    }
}
