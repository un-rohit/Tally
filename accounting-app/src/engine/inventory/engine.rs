use crate::domain::inventory::{InventoryTransaction, MovementType, StockItem, Uom};
use crate::domain::money::Paise;
use crate::engine::inventory::valuation::InventoryValuationEngine;
use crate::error::{AccountingError, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockMovementRequest {
    pub item_id: String,
    pub warehouse_id: String,
    pub movement_type: MovementType,
    pub quantity_milli: i64,
    pub rate_paise: Paise,
    pub batch_number: Option<String>,
    pub expiry_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockTransferRequest {
    pub item_id: String,
    pub from_warehouse_id: String,
    pub to_warehouse_id: String,
    pub quantity_milli: i64,
    pub rate_paise: Paise,
}

pub struct InventoryEngine;

impl InventoryEngine {
    /// Checks company policy for negative stock. Returns true if negative stock is permitted.
    pub fn is_negative_stock_allowed(conn: &Connection, company_id: &str) -> Result<bool> {
        let allowed: Option<i64> = conn
            .query_row(
                "SELECT allow_negative_stock FROM company_settings WHERE company_id = ?1",
                params![company_id],
                |r| r.get(0),
            )
            .ok();

        Ok(allowed.unwrap_or(0) == 1)
    }

    /// Sets negative stock policy for a company.
    pub fn set_negative_stock_policy(
        conn: &Connection,
        company_id: &str,
        allow_negative: bool,
    ) -> Result<()> {
        let setting_id = Uuid::new_v4().to_string();
        let val = if allow_negative { 1 } else { 0 };

        conn.execute(
            "INSERT INTO company_settings (setting_id, company_id, allow_negative_stock) 
             VALUES (?1, ?2, ?3)
             ON CONFLICT(company_id) DO UPDATE SET allow_negative_stock = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![setting_id, company_id, val],
        )?;

        Ok(())
    }

    /// Computes the current available quantity (in milli-units) for an item in a specific warehouse.
    pub fn get_available_quantity(
        conn: &Connection,
        company_id: &str,
        item_id: &str,
        warehouse_id: &str,
    ) -> Result<i64> {
        // Inward transactions
        let inward: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(t.quantity_milli), 0)
                 FROM inventory_transactions t
                 JOIN vouchers v ON t.voucher_id = v.voucher_id
                 WHERE t.company_id = ?1 AND t.item_id = ?2 AND t.warehouse_id = ?3 
                   AND v.status = 'Posted'
                   AND t.movement_type IN ('IN', 'TRANSFER_IN', 'ADJUSTMENT_IN')",
                params![company_id, item_id, warehouse_id],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Outward transactions
        let outward: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(t.quantity_milli), 0)
                 FROM inventory_transactions t
                 JOIN vouchers v ON t.voucher_id = v.voucher_id
                 WHERE t.company_id = ?1 AND t.item_id = ?2 AND t.warehouse_id = ?3 
                   AND v.status = 'Posted'
                   AND t.movement_type IN ('OUT', 'TRANSFER_OUT', 'ADJUSTMENT_OUT')",
                params![company_id, item_id, warehouse_id],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Check opening stock if this warehouse is the primary warehouse
        let is_primary: bool = conn
            .query_row(
                "SELECT (SELECT warehouse_id FROM warehouses WHERE company_id = ?1 ORDER BY created_at ASC LIMIT 1) = ?2",
                params![company_id, warehouse_id],
                |r| r.get(0),
            )
            .unwrap_or(false);

        let op_qty: i64 = if is_primary {
            conn.query_row(
                "SELECT opening_qty_milli FROM stock_items WHERE item_id = ?1",
                params![item_id],
                |r| r.get(0),
            )
            .unwrap_or(0)
        } else {
            0
        };

        Ok(op_qty + inward - outward)
    }

    /// Records an inventory movement within a transaction (prior to voucher posting).
    pub fn record_movement(
        tx: &Transaction,
        company_id: &str,
        voucher_id: &str,
        voucher_entry_id: Option<&str>,
        transaction_date: &str,
        req: &StockMovementRequest,
    ) -> Result<InventoryTransaction> {
        if req.quantity_milli <= 0 {
            return Err(AccountingError::Validation(
                "Stock movement quantity must be greater than zero".to_string(),
            ));
        }

        // If outward movement, check negative stock rule
        if req.movement_type.is_outward() {
            let allow_negative = Self::is_negative_stock_allowed(tx, company_id)?;
            if !allow_negative {
                let available =
                    Self::get_available_quantity(tx, company_id, &req.item_id, &req.warehouse_id)?;
                if available < req.quantity_milli {
                    let item_name: String = tx
                        .query_row(
                            "SELECT name FROM stock_items WHERE item_id = ?1",
                            params![req.item_id],
                            |r| r.get(0),
                        )
                        .unwrap_or_else(|_| req.item_id.clone());

                    return Err(AccountingError::Validation(format!(
                        "Insufficient stock for '{}'. Available: {:.3}, Requested: {:.3}. Negative stock is disabled.",
                        item_name,
                        available as f64 / 1000.0,
                        req.quantity_milli as f64 / 1000.0
                    )));
                }
            }
        }

        // Calculate amount_paise = (quantity_milli * rate_paise) / 1000
        let amount_paise = Paise(
            ((req.quantity_milli as i128 * req.rate_paise.0 as i128) / 1000) as i64,
        );

        let inv_tx_id = Uuid::new_v4().to_string();

        tx.execute(
            "INSERT INTO inventory_transactions (
                inv_tx_id, company_id, voucher_id, voucher_entry_id, item_id, warehouse_id, 
                movement_type, quantity_milli, rate_paise, amount_paise, transaction_date, 
                batch_number, expiry_date
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                inv_tx_id,
                company_id,
                voucher_id,
                voucher_entry_id,
                req.item_id,
                req.warehouse_id,
                req.movement_type.as_str(),
                req.quantity_milli,
                req.rate_paise.0,
                amount_paise.0,
                transaction_date,
                req.batch_number,
                req.expiry_date,
            ],
        )?;

        Ok(InventoryTransaction {
            inv_tx_id,
            company_id: company_id.to_string(),
            voucher_id: voucher_id.to_string(),
            voucher_entry_id: voucher_entry_id.map(|s| s.to_string()),
            item_id: req.item_id.clone(),
            warehouse_id: req.warehouse_id.clone(),
            movement_type: req.movement_type.as_str().to_string(),
            quantity_milli: req.quantity_milli,
            rate_paise: req.rate_paise,
            amount_paise,
            transaction_date: transaction_date.to_string(),
            batch_number: req.batch_number.clone(),
            expiry_date: req.expiry_date.clone(),
        })
    }

    /// Records an inventory transfer between two warehouses atomically.
    pub fn record_transfer(
        tx: &Transaction,
        company_id: &str,
        voucher_id: &str,
        transaction_date: &str,
        req: &StockTransferRequest,
    ) -> Result<(InventoryTransaction, InventoryTransaction)> {
        if req.from_warehouse_id == req.to_warehouse_id {
            return Err(AccountingError::Validation(
                "Source and destination warehouses cannot be the same for stock transfer"
                    .to_string(),
            ));
        }

        // 1. Outward from source
        let out_req = StockMovementRequest {
            item_id: req.item_id.clone(),
            warehouse_id: req.from_warehouse_id.clone(),
            movement_type: MovementType::TransferOut,
            quantity_milli: req.quantity_milli,
            rate_paise: req.rate_paise,
            batch_number: None,
            expiry_date: None,
        };
        let out_tx = Self::record_movement(tx, company_id, voucher_id, None, transaction_date, &out_req)?;

        // 2. Inward to destination
        let in_req = StockMovementRequest {
            item_id: req.item_id.clone(),
            warehouse_id: req.to_warehouse_id.clone(),
            movement_type: MovementType::TransferIn,
            quantity_milli: req.quantity_milli,
            rate_paise: req.rate_paise,
            batch_number: None,
            expiry_date: None,
        };
        let in_tx = Self::record_movement(tx, company_id, voucher_id, None, transaction_date, &in_req)?;

        Ok((out_tx, in_tx))
    }

    /// Converts quantity from one UOM to another, handling simple and compound units.
    pub fn convert_quantity(
        conn: &Connection,
        company_id: &str,
        from_uom_id: &str,
        to_uom_id: &str,
        qty_milli: i64,
    ) -> Result<i64> {
        if from_uom_id == to_uom_id {
            return Ok(qty_milli);
        }

        let uoms = Uom::list_by_company(conn, company_id)?;
        let from_uom = uoms
            .iter()
            .find(|u| u.uom_id == from_uom_id)
            .ok_or_else(|| AccountingError::NotFound(format!("UOM {} not found", from_uom_id)))?;
        let to_uom = uoms
            .iter()
            .find(|u| u.uom_id == to_uom_id)
            .ok_or_else(|| AccountingError::NotFound(format!("UOM {} not found", to_uom_id)))?;

        // Case 1: from_uom is compound, base is to_uom (e.g. Box of 10 Pcs -> Pcs)
        if from_uom.is_compound && from_uom.base_uom_id.as_deref() == Some(to_uom_id) {
            let res = (qty_milli as i128)
                .checked_mul(from_uom.conversion_factor as i128)
                .ok_or_else(|| AccountingError::MathOverflow("UOM conversion overflow".into()))?;
            return Ok(res as i64);
        }

        // Case 2: to_uom is compound, base is from_uom (e.g. Pcs -> Box of 10 Pcs)
        if to_uom.is_compound && to_uom.base_uom_id.as_deref() == Some(from_uom_id) {
            if to_uom.conversion_factor == 0 {
                return Err(AccountingError::Validation("Zero conversion factor".into()));
            }
            return Ok(qty_milli / (to_uom.conversion_factor as i64));
        }

        Err(AccountingError::Validation(format!(
            "No direct conversion between UOM '{}' and '{}'",
            from_uom.symbol, to_uom.symbol
        )))
    }
}
