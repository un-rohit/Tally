use super::gstin::Gstin;
use crate::error::{AccountingError, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartyRegistrationType {
    Regular,
    Composition,
    Unregistered,
    Consumer,
    Overseas,
    SEZ,
}

impl PartyRegistrationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Regular => "Regular",
            Self::Composition => "Composition",
            Self::Unregistered => "Unregistered",
            Self::Consumer => "Consumer",
            Self::Overseas => "Overseas",
            Self::SEZ => "SEZ",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "Regular" => Ok(Self::Regular),
            "Composition" => Ok(Self::Composition),
            "Unregistered" => Ok(Self::Unregistered),
            "Consumer" => Ok(Self::Consumer),
            "Overseas" => Ok(Self::Overseas),
            "SEZ" => Ok(Self::SEZ),
            _ => Err(AccountingError::Validation(format!(
                "Invalid party registration type '{}'",
                s
            ))),
        }
    }

    pub fn is_registered(&self) -> bool {
        matches!(self, Self::Regular | Self::Composition | Self::SEZ)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyGstRegistration {
    pub reg_id: String,
    pub company_id: String,
    pub gstin: Gstin,
    pub legal_name: String,
    pub trade_name: Option<String>,
    pub state_code: String,
    pub registration_type: String, // "Regular", "Composition"
    pub is_primary: bool,
}

impl CompanyGstRegistration {
    /// Registers a new GSTIN for a company.
    pub fn register(
        conn: &Connection,
        company_id: &str,
        raw_gstin: &str,
        legal_name: &str,
        trade_name: Option<&str>,
        registration_type: &str,
        is_primary: bool,
    ) -> Result<Self> {
        let gstin = Gstin::new(raw_gstin)?;
        let state_code = gstin.state_code().to_string();

        if registration_type != "Regular" && registration_type != "Composition" {
            return Err(AccountingError::Validation(
                "Company registration type must be 'Regular' or 'Composition'".to_string(),
            ));
        }

        let reg_id = Uuid::new_v4().to_string();

        // If primary, demote any other existing primary registration for this company
        if is_primary {
            conn.execute(
                "UPDATE gst_registrations SET is_primary = 0 WHERE company_id = ?1",
                params![company_id],
            )?;
        }

        conn.execute(
            "INSERT INTO gst_registrations (reg_id, company_id, gstin, legal_name, trade_name, state_code, registration_type, is_primary) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                reg_id,
                company_id,
                gstin.as_str(),
                legal_name,
                trade_name,
                state_code,
                registration_type,
                if is_primary { 1 } else { 0 }
            ],
        )?;

        Ok(Self {
            reg_id,
            company_id: company_id.to_string(),
            gstin,
            legal_name: legal_name.to_string(),
            trade_name: trade_name.map(|s| s.to_string()),
            state_code,
            registration_type: registration_type.to_string(),
            is_primary,
        })
    }

    /// Fetches the primary GST registration for a company.
    pub fn get_primary(conn: &Connection, company_id: &str) -> Result<Option<Self>> {
        let mut stmt = conn.prepare(
            "SELECT reg_id, company_id, gstin, legal_name, trade_name, state_code, registration_type, is_primary 
             FROM gst_registrations 
             WHERE company_id = ?1 AND is_primary = 1 LIMIT 1",
        )?;

        let mut rows = stmt.query(params![company_id])?;
        if let Some(row) = rows.next()? {
            let gstin_str: String = row.get(2)?;
            let gstin = Gstin::new_unchecked(&gstin_str)?;
            let is_prim: i64 = row.get(7)?;

            Ok(Some(Self {
                reg_id: row.get(0)?,
                company_id: row.get(1)?,
                gstin,
                legal_name: row.get(3)?,
                trade_name: row.get(4)?,
                state_code: row.get(5)?,
                registration_type: row.get(6)?,
                is_primary: is_prim == 1,
            }))
        } else {
            Ok(None)
        }
    }
}
