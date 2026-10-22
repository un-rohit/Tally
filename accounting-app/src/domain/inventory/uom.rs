use crate::error::{AccountingError, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Uom {
    pub uom_id: String,
    pub company_id: String,
    pub symbol: String,
    pub formal_name: Option<String>,
    pub decimal_places: u32,
    pub is_compound: bool,
    pub base_uom_id: Option<String>,
    pub conversion_factor: u32,
}

impl Uom {
    pub fn create_simple(
        conn: &Connection,
        company_id: &str,
        symbol: &str,
        formal_name: Option<&str>,
        decimal_places: u32,
    ) -> Result<Self> {
        let uom_id = Uuid::new_v4().to_string();
        if decimal_places > 4 {
            return Err(AccountingError::Validation(
                "Decimal places for UOM cannot exceed 4".to_string(),
            ));
        }

        conn.execute(
            "INSERT INTO units_of_measure (uom_id, company_id, symbol, formal_name, decimal_places, is_compound, conversion_factor) 
             VALUES (?1, ?2, ?3, ?4, ?5, 0, 1)",
            params![uom_id, company_id, symbol, formal_name, decimal_places],
        )?;

        Ok(Self {
            uom_id,
            company_id: company_id.to_string(),
            symbol: symbol.to_string(),
            formal_name: formal_name.map(|s| s.to_string()),
            decimal_places,
            is_compound: false,
            base_uom_id: None,
            conversion_factor: 1,
        })
    }

    pub fn create_compound(
        conn: &Connection,
        company_id: &str,
        symbol: &str,
        formal_name: Option<&str>,
        base_uom_id: &str,
        conversion_factor: u32,
    ) -> Result<Self> {
        if conversion_factor == 0 {
            return Err(AccountingError::Validation(
                "Conversion factor for compound UOM must be greater than zero".to_string(),
            ));
        }

        let uom_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO units_of_measure (uom_id, company_id, symbol, formal_name, decimal_places, is_compound, base_uom_id, conversion_factor) 
             VALUES (?1, ?2, ?3, ?4, 0, 1, ?5, ?6)",
            params![uom_id, company_id, symbol, formal_name, base_uom_id, conversion_factor],
        )?;

        Ok(Self {
            uom_id,
            company_id: company_id.to_string(),
            symbol: symbol.to_string(),
            formal_name: formal_name.map(|s| s.to_string()),
            decimal_places: 0,
            is_compound: true,
            base_uom_id: Some(base_uom_id.to_string()),
            conversion_factor,
        })
    }

    /// Seeds standard Indian accounting UOMs: Pcs, Kgs, Box (10 Pcs), Nos, Ltr, Mtr.
    pub fn seed_standard_uoms(conn: &Connection, company_id: &str) -> Result<()> {
        let pcs = Self::create_simple(conn, company_id, "Pcs", Some("Pieces"), 0)?;
        Self::create_simple(conn, company_id, "Nos", Some("Numbers"), 0)?;
        Self::create_simple(conn, company_id, "Kgs", Some("Kilograms"), 3)?;
        Self::create_simple(conn, company_id, "Ltr", Some("Litres"), 2)?;
        Self::create_simple(conn, company_id, "Mtr", Some("Metres"), 2)?;
        Self::create_compound(conn, company_id, "Box", Some("Box of 10 Pcs"), &pcs.uom_id, 10)?;
        Ok(())
    }

    pub fn list_by_company(conn: &Connection, company_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT uom_id, company_id, symbol, formal_name, decimal_places, is_compound, base_uom_id, conversion_factor 
             FROM units_of_measure WHERE company_id = ?1 ORDER BY symbol",
        )?;

        let rows = stmt.query_map(params![company_id], |r| {
            let is_comp: i64 = r.get(5)?;
            Ok(Self {
                uom_id: r.get(0)?,
                company_id: r.get(1)?,
                symbol: r.get(2)?,
                formal_name: r.get(3)?,
                decimal_places: r.get(4)?,
                is_compound: is_comp == 1,
                base_uom_id: r.get(6)?,
                conversion_factor: r.get(7)?,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }
}
