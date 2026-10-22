use crate::domain::money::Paise;
use crate::engine::inventory::valuation::InventoryValuationEngine;
use crate::error::{AccountingError, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSummaryItem {
    pub item_id: String,
    pub item_name: String,
    pub sku: Option<String>,
    pub uom_symbol: String,
    pub valuation_method: String,
    pub opening_qty_milli: i64,
    pub opening_value_paise: Paise,
    pub inward_qty_milli: i64,
    pub inward_value_paise: Paise,
    pub outward_qty_milli: i64,
    pub outward_value_paise: Paise,
    pub closing_qty_milli: i64,
    pub closing_rate_paise: Paise,
    pub closing_value_paise: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockItemLedgerEntry {
    pub transaction_date: String,
    pub voucher_id: String,
    pub voucher_number: String,
    pub voucher_type: String,
    pub warehouse_name: String,
    pub movement_type: String,
    pub inward_qty_milli: i64,
    pub inward_rate_paise: Paise,
    pub inward_amount_paise: Paise,
    pub outward_qty_milli: i64,
    pub outward_rate_paise: Paise,
    pub outward_amount_paise: Paise,
    pub balance_qty_milli: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodownStockItem {
    pub warehouse_id: String,
    pub warehouse_name: String,
    pub item_id: String,
    pub item_name: String,
    pub uom_symbol: String,
    pub quantity_milli: i64,
    pub value_paise: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowStockAlert {
    pub item_id: String,
    pub item_name: String,
    pub uom_symbol: String,
    pub current_qty_milli: i64,
    pub reorder_level_milli: i64,
    pub shortfall_milli: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryValuationReconciliation {
    pub total_items_count: usize,
    pub total_inventory_valuation_paise: Paise,
    pub items: Vec<StockSummaryItem>,
}

pub struct InventoryReportEngine;

impl InventoryReportEngine {
    /// Generates the Stock Summary report across all items or filtered by group.
    pub fn generate_stock_summary(
        conn: &Connection,
        company_id: &str,
        as_of_date: Option<NaiveDate>,
        stock_group_id: Option<&str>,
    ) -> Result<Vec<StockSummaryItem>> {
        let mut sql = "SELECT i.item_id, i.name, i.sku, u.symbol, i.valuation_method, 
                              i.opening_qty_milli, i.opening_value_paise
                       FROM stock_items i
                       JOIN units_of_measure u ON i.uom_id = u.uom_id
                       WHERE i.company_id = ?1 AND i.is_active = 1".to_string();

        if stock_group_id.is_some() {
            sql.push_str(" AND i.stock_group_id = ?2");
        }
        sql.push_str(" ORDER BY i.name ASC");

        let mut stmt = conn.prepare(&sql)?;
        let items_data: Vec<(String, String, Option<String>, String, String, i64, i64)> =
            if let Some(grp) = stock_group_id {
                let rows = stmt.query_map(params![company_id, grp], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ))
                })?;
                let mut v = Vec::new();
                for r in rows {
                    v.push(r?);
                }
                v
            } else {
                let rows = stmt.query_map(params![company_id], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ))
                })?;
                let mut v = Vec::new();
                for r in rows {
                    v.push(r?);
                }
                v
            };

        let date_clause = if let Some(d) = as_of_date {
            format!("AND t.transaction_date <= '{}'", d.format("%Y-%m-%d"))
        } else {
            "".to_string()
        };

        let mut summary_items = Vec::new();

        for (item_id, name, sku, symbol, method, op_qty, op_val) in items_data {
            // Total inward movements
            let sql_in = format!(
                "SELECT COALESCE(SUM(t.quantity_milli), 0), COALESCE(SUM(t.amount_paise), 0)
                 FROM inventory_transactions t
                 JOIN vouchers v ON t.voucher_id = v.voucher_id
                 WHERE t.item_id = ?1 AND v.status = 'Posted'
                   AND t.movement_type IN ('IN', 'TRANSFER_IN', 'ADJUSTMENT_IN') {date_clause}"
            );
            let (in_qty, in_val): (i64, i64) = conn
                .query_row(&sql_in, params![item_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap_or((0, 0));

            // Total outward movements
            let sql_out = format!(
                "SELECT COALESCE(SUM(t.quantity_milli), 0), COALESCE(SUM(t.amount_paise), 0)
                 FROM inventory_transactions t
                 JOIN vouchers v ON t.voucher_id = v.voucher_id
                 WHERE t.item_id = ?1 AND v.status = 'Posted'
                   AND t.movement_type IN ('OUT', 'TRANSFER_OUT', 'ADJUSTMENT_OUT') {date_clause}"
            );
            let (out_qty, out_val): (i64, i64) = conn
                .query_row(&sql_out, params![item_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap_or((0, 0));

            // Valuation using FIFO or Weighted Average
            let val_res = InventoryValuationEngine::calculate_valuation(conn, &item_id, as_of_date)?;

            summary_items.push(StockSummaryItem {
                item_id,
                item_name: name,
                sku,
                uom_symbol: symbol,
                valuation_method: method,
                opening_qty_milli: op_qty,
                opening_value_paise: Paise(op_val),
                inward_qty_milli: in_qty,
                inward_value_paise: Paise(in_val),
                outward_qty_milli: out_qty,
                outward_value_paise: Paise(out_val),
                closing_qty_milli: val_res.total_closing_qty_milli,
                closing_rate_paise: val_res.effective_unit_rate_paise,
                closing_value_paise: val_res.total_closing_value_paise,
            });
        }

        Ok(summary_items)
    }

    /// Generates detailed stock item ledger tracking every movement chronologically.
    pub fn generate_stock_item_ledger(
        conn: &Connection,
        company_id: &str,
        item_id: &str,
        from_date: Option<NaiveDate>,
        to_date: Option<NaiveDate>,
    ) -> Result<Vec<StockItemLedgerEntry>> {
        let (op_qty, _op_rate): (i64, i64) = conn
            .query_row(
                "SELECT opening_qty_milli, opening_rate_paise FROM stock_items WHERE item_id = ?1 AND company_id = ?2",
                params![item_id, company_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| AccountingError::NotFound(format!("Stock item {} not found", item_id)))?;

        let mut running_qty = op_qty;

        let sql = "SELECT t.transaction_date, t.voucher_id, v.voucher_number, vt.name, 
                          w.name, t.movement_type, t.quantity_milli, t.rate_paise, t.amount_paise
                   FROM inventory_transactions t
                   JOIN vouchers v ON t.voucher_id = v.voucher_id
                   JOIN voucher_types vt ON v.voucher_type_id = vt.voucher_type_id
                   JOIN warehouses w ON t.warehouse_id = w.warehouse_id
                   WHERE t.item_id = ?1 AND v.status = 'Posted'
                   ORDER BY t.transaction_date ASC, t.rowid ASC";

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![item_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
            ))
        })?;

        let mut entries = Vec::new();

        for r_res in rows {
            let (tx_date, v_id, v_num, v_type, wh_name, m_type, qty, rate, amt) = r_res?;
            let is_inward = matches!(m_type.as_str(), "IN" | "TRANSFER_IN" | "ADJUSTMENT_IN");

            let (in_qty, in_rate, in_amt, out_qty, out_rate, out_amt) = if is_inward {
                running_qty += qty;
                (qty, Paise(rate), Paise(amt), 0, Paise::ZERO, Paise::ZERO)
            } else {
                running_qty -= qty;
                (0, Paise::ZERO, Paise::ZERO, qty, Paise(rate), Paise(amt))
            };

            // Apply date filters if provided
            let parsed_date = NaiveDate::parse_from_str(&tx_date, "%Y-%m-%d").ok();
            let matches_from = from_date.map_or(true, |f| parsed_date.map_or(true, |d| d >= f));
            let matches_to = to_date.map_or(true, |t| parsed_date.map_or(true, |d| d <= t));

            if matches_from && matches_to {
                entries.push(StockItemLedgerEntry {
                    transaction_date: tx_date,
                    voucher_id: v_id,
                    voucher_number: v_num,
                    voucher_type: v_type,
                    warehouse_name: wh_name,
                    movement_type: m_type,
                    inward_qty_milli: in_qty,
                    inward_rate_paise: in_rate,
                    inward_amount_paise: in_amt,
                    outward_qty_milli: out_qty,
                    outward_rate_paise: out_rate,
                    outward_amount_paise: out_amt,
                    balance_qty_milli: running_qty,
                });
            }
        }

        Ok(entries)
    }

    /// Generates Godown-wise (warehouse-wise) stock allocation.
    pub fn generate_godown_stock_report(
        conn: &Connection,
        company_id: &str,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Vec<GodownStockItem>> {
        let date_clause = if let Some(d) = as_of_date {
            format!("AND t.transaction_date <= '{}'", d.format("%Y-%m-%d"))
        } else {
            "".to_string()
        };

        let sql = format!(
            "SELECT w.warehouse_id, w.name, i.item_id, i.name, u.symbol,
                    COALESCE(SUM(CASE WHEN t.movement_type IN ('IN', 'TRANSFER_IN', 'ADJUSTMENT_IN') THEN t.quantity_milli 
                                     WHEN t.movement_type IN ('OUT', 'TRANSFER_OUT', 'ADJUSTMENT_OUT') THEN -t.quantity_milli 
                                     ELSE 0 END), 0) as balance_qty
             FROM warehouses w
             JOIN stock_items i ON w.company_id = i.company_id
             JOIN units_of_measure u ON i.uom_id = u.uom_id
             LEFT JOIN inventory_transactions t ON t.warehouse_id = w.warehouse_id AND t.item_id = i.item_id {date_clause}
             LEFT JOIN vouchers v ON t.voucher_id = v.voucher_id AND v.status = 'Posted'
             WHERE w.company_id = ?1 AND w.is_active = 1 AND i.is_active = 1
             GROUP BY w.warehouse_id, i.item_id
             HAVING balance_qty != 0
             ORDER BY w.name, i.name"
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![company_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?;

        let mut list = Vec::new();
        for item_res in rows {
            let (wh_id, wh_name, itm_id, itm_name, sym, qty) = item_res?;
            let val_res = InventoryValuationEngine::calculate_valuation(conn, &itm_id, as_of_date)?;
            let item_val_paise = if val_res.total_closing_qty_milli > 0 {
                Paise(((qty as i128 * val_res.effective_unit_rate_paise.0 as i128) / 1000) as i64)
            } else {
                Paise::ZERO
            };

            list.push(GodownStockItem {
                warehouse_id: wh_id,
                warehouse_name: wh_name,
                item_id: itm_id,
                item_name: itm_name,
                uom_symbol: sym,
                quantity_milli: qty,
                value_paise: item_val_paise,
            });
        }

        Ok(list)
    }

    /// Generates Low Stock alert report based on reorder levels.
    pub fn generate_low_stock_report(
        conn: &Connection,
        company_id: &str,
    ) -> Result<Vec<LowStockAlert>> {
        let mut alerts = Vec::new();
        let summary = Self::generate_stock_summary(conn, company_id, None, None)?;

        for item in summary {
            let reorder: i64 = conn
                .query_row(
                    "SELECT reorder_level_milli FROM stock_items WHERE item_id = ?1",
                    params![item.item_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            if reorder > 0 && item.closing_qty_milli <= reorder {
                alerts.push(LowStockAlert {
                    item_id: item.item_id,
                    item_name: item.item_name,
                    uom_symbol: item.uom_symbol,
                    current_qty_milli: item.closing_qty_milli,
                    reorder_level_milli: reorder,
                    shortfall_milli: reorder - item.closing_qty_milli,
                });
            }
        }

        Ok(alerts)
    }

    /// Generates complete Inventory Valuation Reconciliation for financial balance sheet audit.
    pub fn generate_valuation_reconciliation(
        conn: &Connection,
        company_id: &str,
        as_of_date: Option<NaiveDate>,
    ) -> Result<InventoryValuationReconciliation> {
        let items = Self::generate_stock_summary(conn, company_id, as_of_date, None)?;
        let total_val: i64 = items.iter().map(|i| i.closing_value_paise.0).sum();

        Ok(InventoryValuationReconciliation {
            total_items_count: items.len(),
            total_inventory_valuation_paise: Paise(total_val),
            items,
        })
    }
}
