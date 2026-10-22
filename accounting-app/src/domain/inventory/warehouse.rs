use crate::error::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warehouse {
    pub warehouse_id: String,
    pub company_id: String,
    pub name: String,
    pub address: Option<String>,
    pub is_active: bool,
}

impl Warehouse {
    pub fn create(
        conn: &Connection,
        company_id: &str,
        name: &str,
        address: Option<&str>,
    ) -> Result<Self> {
        let warehouse_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO warehouses (warehouse_id, company_id, name, address, is_active) 
             VALUES (?1, ?2, ?3, ?4, 1)",
            params![warehouse_id, company_id, name, address],
        )?;

        Ok(Self {
            warehouse_id,
            company_id: company_id.to_string(),
            name: name.to_string(),
            address: address.map(|s| s.to_string()),
            is_active: true,
        })
    }

    pub fn list_by_company(conn: &Connection, company_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT warehouse_id, company_id, name, address, is_active 
             FROM warehouses WHERE company_id = ?1 ORDER BY name",
        )?;

        let rows = stmt.query_map(params![company_id], |r| {
            let active: i64 = r.get(4)?;
            Ok(Self {
                warehouse_id: r.get(0)?,
                company_id: r.get(1)?,
                name: r.get(2)?,
                address: r.get(3)?,
                is_active: active == 1,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    /// Seeds default warehouse "Main Location" if no warehouses exist.
    pub fn seed_default_warehouse(conn: &Connection, company_id: &str) -> Result<Self> {
        let existing = Self::list_by_company(conn, company_id)?;
        if let Some(w) = existing.into_iter().next() {
            Ok(w)
        } else {
            Self::create(conn, company_id, "Main Location", Some("Primary Godown"))
        }
    }
}
