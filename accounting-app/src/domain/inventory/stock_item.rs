use crate::domain::money::Paise;
use crate::error::{AccountingError, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockGroup {
    pub stock_group_id: String,
    pub company_id: String,
    pub name: String,
    pub parent_group_id: Option<String>,
}

impl StockGroup {
    pub fn create(
        conn: &Connection,
        company_id: &str,
        name: &str,
        parent_group_id: Option<&str>,
    ) -> Result<Self> {
        let stock_group_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO stock_groups (stock_group_id, company_id, name, parent_group_id) 
             VALUES (?1, ?2, ?3, ?4)",
            params![stock_group_id, company_id, name, parent_group_id],
        )?;

        Ok(Self {
            stock_group_id,
            company_id: company_id.to_string(),
            name: name.to_string(),
            parent_group_id: parent_group_id.map(|s| s.to_string()),
        })
    }

    pub fn list_by_company(conn: &Connection, company_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT stock_group_id, company_id, name, parent_group_id 
             FROM stock_groups WHERE company_id = ?1 ORDER BY name",
        )?;

        let rows = stmt.query_map(params![company_id], |r| {
            Ok(Self {
                stock_group_id: r.get(0)?,
                company_id: r.get(1)?,
                name: r.get(2)?,
                parent_group_id: r.get(3)?,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockCategory {
    pub category_id: String,
    pub company_id: String,
    pub name: String,
}

impl StockCategory {
    pub fn create(conn: &Connection, company_id: &str, name: &str) -> Result<Self> {
        let category_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO stock_categories (category_id, company_id, name) VALUES (?1, ?2, ?3)",
            params![category_id, company_id, name],
        )?;

        Ok(Self {
            category_id,
            company_id: company_id.to_string(),
            name: name.to_string(),
        })
    }

    pub fn list_by_company(conn: &Connection, company_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT category_id, company_id, name FROM stock_categories WHERE company_id = ?1 ORDER BY name",
        )?;

        let rows = stmt.query_map(params![company_id], |r| {
            Ok(Self {
                category_id: r.get(0)?,
                company_id: r.get(1)?,
                name: r.get(2)?,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockItem {
    pub item_id: String,
    pub company_id: String,
    pub stock_group_id: Option<String>,
    pub category_id: Option<String>,
    pub uom_id: String,
    pub name: String,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub hsn_sac_code: Option<String>,
    pub gst_rate_basis_points: u32,
    pub valuation_method: String, // "FIFO", "WeightedAverage"
    pub opening_qty_milli: i64,
    pub opening_rate_paise: Paise,
    pub opening_value_paise: Paise,
    pub reorder_level_milli: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStockItemRequest {
    pub company_id: String,
    pub stock_group_id: Option<String>,
    pub category_id: Option<String>,
    pub uom_id: String,
    pub name: String,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub hsn_sac_code: Option<String>,
    pub gst_rate_basis_points: u32,
    pub valuation_method: String,
    pub opening_qty_milli: i64,
    pub opening_rate_paise: Paise,
    pub reorder_level_milli: i64,
}

impl StockItem {
    pub fn create(conn: &Connection, req: &CreateStockItemRequest) -> Result<Self> {
        if req.valuation_method != "FIFO" && req.valuation_method != "WeightedAverage" {
            return Err(AccountingError::Validation(
                "Valuation method must be 'FIFO' or 'WeightedAverage'".to_string(),
            ));
        }

        // Calculate opening value = (qty_milli * rate_paise) / 1000
        let opening_value = if req.opening_qty_milli > 0 && req.opening_rate_paise.0 > 0 {
            let total = (req.opening_qty_milli as i128)
                .checked_mul(req.opening_rate_paise.0 as i128)
                .ok_or_else(|| AccountingError::MathOverflow("Opening stock value overflow".into()))?;
            Paise((total / 1000) as i64)
        } else {
            Paise::ZERO
        };

        let item_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO stock_items (
                item_id, company_id, stock_group_id, category_id, uom_id, name, sku, description, 
                hsn_sac_code, gst_rate_basis_points, valuation_method, opening_qty_milli, opening_rate_paise, 
                opening_value_paise, reorder_level_milli, is_active
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 1)",
            params![
                item_id,
                req.company_id,
                req.stock_group_id,
                req.category_id,
                req.uom_id,
                req.name,
                req.sku,
                req.description,
                req.hsn_sac_code,
                req.gst_rate_basis_points as i64,
                req.valuation_method,
                req.opening_qty_milli,
                req.opening_rate_paise.0,
                opening_value.0,
                req.reorder_level_milli,
            ],
        )?;

        Ok(Self {
            item_id,
            company_id: req.company_id.clone(),
            stock_group_id: req.stock_group_id.clone(),
            category_id: req.category_id.clone(),
            uom_id: req.uom_id.clone(),
            name: req.name.clone(),
            sku: req.sku.clone(),
            description: req.description.clone(),
            hsn_sac_code: req.hsn_sac_code.clone(),
            gst_rate_basis_points: req.gst_rate_basis_points,
            valuation_method: req.valuation_method.clone(),
            opening_qty_milli: req.opening_qty_milli,
            opening_rate_paise: req.opening_rate_paise,
            opening_value_paise: opening_value,
            reorder_level_milli: req.reorder_level_milli,
            is_active: true,
        })
    }

    pub fn list_by_company(conn: &Connection, company_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT 
                item_id, company_id, stock_group_id, category_id, uom_id, name, sku, description, 
                hsn_sac_code, gst_rate_basis_points, valuation_method, opening_qty_milli, opening_rate_paise, 
                opening_value_paise, reorder_level_milli, is_active 
             FROM stock_items WHERE company_id = ?1 ORDER BY name",
        )?;

        let rows = stmt.query_map(params![company_id], |r| {
            let active: i64 = r.get(15)?;
            Ok(Self {
                item_id: r.get(0)?,
                company_id: r.get(1)?,
                stock_group_id: r.get(2)?,
                category_id: r.get(3)?,
                uom_id: r.get(4)?,
                name: r.get(5)?,
                sku: r.get(6)?,
                description: r.get(7)?,
                hsn_sac_code: r.get(8)?,
                gst_rate_basis_points: r.get::<_, i64>(9)? as u32,
                valuation_method: r.get(10)?,
                opening_qty_milli: r.get(11)?,
                opening_rate_paise: Paise(r.get(12)?),
                opening_value_paise: Paise(r.get(13)?),
                reorder_level_milli: r.get(14)?,
                is_active: active == 1,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    pub fn find_by_id(conn: &Connection, item_id: &str) -> Result<Option<Self>> {
        let mut stmt = conn.prepare(
            "SELECT 
                item_id, company_id, stock_group_id, category_id, uom_id, name, sku, description, 
                hsn_sac_code, gst_rate_basis_points, valuation_method, opening_qty_milli, opening_rate_paise, 
                opening_value_paise, reorder_level_milli, is_active 
             FROM stock_items WHERE item_id = ?1",
        )?;

        let mut rows = stmt.query(params![item_id])?;
        if let Some(r) = rows.next()? {
            let active: i64 = r.get(15)?;
            Ok(Some(Self {
                item_id: r.get(0)?,
                company_id: r.get(1)?,
                stock_group_id: r.get(2)?,
                category_id: r.get(3)?,
                uom_id: r.get(4)?,
                name: r.get(5)?,
                sku: r.get(6)?,
                description: r.get(7)?,
                hsn_sac_code: r.get(8)?,
                gst_rate_basis_points: r.get::<_, i64>(9)? as u32,
                valuation_method: r.get(10)?,
                opening_qty_milli: r.get(11)?,
                opening_rate_paise: Paise(r.get(12)?),
                opening_value_paise: Paise(r.get(13)?),
                reorder_level_milli: r.get(14)?,
                is_active: active == 1,
            }))
        } else {
            Ok(None)
        }
    }
}
