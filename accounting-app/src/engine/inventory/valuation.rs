use crate::domain::money::Paise;
use crate::error::{AccountingError, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockValuationResult {
    pub item_id: String,
    pub valuation_method: String,
    pub total_closing_qty_milli: i64,
    pub total_closing_value_paise: Paise,
    pub effective_unit_rate_paise: Paise,
}

#[derive(Debug, Clone)]
struct FifoLot {
    qty_remaining_milli: i64,
    rate_paise: Paise,
}

pub struct InventoryValuationEngine;

impl InventoryValuationEngine {
    /// Computes closing stock quantity, valuation, and effective rate as of a specified date.
    pub fn calculate_valuation(
        conn: &Connection,
        item_id: &str,
        as_of_date: Option<NaiveDate>,
    ) -> Result<StockValuationResult> {
        let (company_id, valuation_method, op_qty, op_rate): (String, String, i64, i64) = conn
            .query_row(
                "SELECT company_id, valuation_method, opening_qty_milli, opening_rate_paise 
                 FROM stock_items WHERE item_id = ?1",
                params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|_| AccountingError::NotFound(format!("Stock item {} not found", item_id)))?;

        let date_clause = if let Some(d) = as_of_date {
            format!("AND t.transaction_date <= '{}'", d.format("%Y-%m-%d"))
        } else {
            "".to_string()
        };

        if valuation_method == "WeightedAverage" {
            Self::calculate_weighted_average(conn, &company_id, item_id, op_qty, Paise(op_rate), &date_clause)
        } else {
            // Default to FIFO
            Self::calculate_fifo(conn, &company_id, item_id, op_qty, Paise(op_rate), &date_clause)
        }
    }

    /// Calculates valuation using First-In-First-Out (FIFO) cost layers.
    fn calculate_fifo(
        conn: &Connection,
        _company_id: &str,
        item_id: &str,
        op_qty: i64,
        op_rate: Paise,
        date_clause: &str,
    ) -> Result<StockValuationResult> {
        // Collect inward lots
        let mut inward_lots = Vec::new();

        // 1. Opening stock layer
        if op_qty > 0 {
            inward_lots.push(FifoLot {
                qty_remaining_milli: op_qty,
                rate_paise: op_rate,
            });
        }

        // 2. Chronological inward transactions
        let sql_inward = format!(
            "SELECT t.quantity_milli, t.rate_paise 
             FROM inventory_transactions t
             JOIN vouchers v ON t.voucher_id = v.voucher_id
             WHERE t.item_id = ?1 AND v.status = 'Posted'
               AND t.movement_type IN ('IN', 'TRANSFER_IN', 'ADJUSTMENT_IN') {date_clause}
             ORDER BY t.transaction_date ASC, t.rowid ASC"
        );

        let mut stmt_in = conn.prepare(&sql_inward)?;
        let in_rows = stmt_in.query_map(params![item_id], |r| {
            Ok(FifoLot {
                qty_remaining_milli: r.get(0)?,
                rate_paise: Paise(r.get(1)?),
            })
        })?;

        for lot in in_rows {
            inward_lots.push(lot?);
        }

        // 3. Chronological outward transactions
        let sql_outward = format!(
            "SELECT t.quantity_milli 
             FROM inventory_transactions t
             JOIN vouchers v ON t.voucher_id = v.voucher_id
             WHERE t.item_id = ?1 AND v.status = 'Posted'
               AND t.movement_type IN ('OUT', 'TRANSFER_OUT', 'ADJUSTMENT_OUT') {date_clause}
             ORDER BY t.transaction_date ASC, t.rowid ASC"
        );

        let mut stmt_out = conn.prepare(&sql_outward)?;
        let out_rows = stmt_out.query_map(params![item_id], |r| r.get::<_, i64>(0))?;

        // 4. Consume FIFO layers with outward issues
        for out_qty_res in out_rows {
            let mut qty_to_consume = out_qty_res?;

            for lot in inward_lots.iter_mut() {
                if lot.qty_remaining_milli == 0 {
                    continue;
                }

                if lot.qty_remaining_milli >= qty_to_consume {
                    lot.qty_remaining_milli -= qty_to_consume;
                    qty_to_consume = 0;
                    break;
                } else {
                    qty_to_consume -= lot.qty_remaining_milli;
                    lot.qty_remaining_milli = 0;
                }
            }
        }

        // 5. Aggregate remaining FIFO layers into closing valuation
        let mut total_closing_qty: i64 = 0;
        let mut total_closing_value_paise: i128 = 0;

        for lot in inward_lots {
            if lot.qty_remaining_milli > 0 {
                total_closing_qty += lot.qty_remaining_milli;
                let lot_val = (lot.qty_remaining_milli as i128) * (lot.rate_paise.0 as i128) / 1000;
                total_closing_value_paise += lot_val;
            }
        }

        let effective_rate = if total_closing_qty > 0 {
            Paise(((total_closing_value_paise * 1000) / total_closing_qty as i128) as i64)
        } else {
            Paise::ZERO
        };

        Ok(StockValuationResult {
            item_id: item_id.to_string(),
            valuation_method: "FIFO".to_string(),
            total_closing_qty_milli: total_closing_qty,
            total_closing_value_paise: Paise(total_closing_value_paise as i64),
            effective_unit_rate_paise: effective_rate,
        })
    }

    /// Calculates valuation using Weighted-Average cost methodology.
    fn calculate_weighted_average(
        conn: &Connection,
        _company_id: &str,
        item_id: &str,
        op_qty: i64,
        op_rate: Paise,
        date_clause: &str,
    ) -> Result<StockValuationResult> {
        let mut current_qty: i64 = op_qty;
        let mut current_value: i128 = if op_qty > 0 {
            (op_qty as i128 * op_rate.0 as i128) / 1000
        } else {
            0
        };

        let sql = format!(
            "SELECT t.movement_type, t.quantity_milli, t.rate_paise 
             FROM inventory_transactions t
             JOIN vouchers v ON t.voucher_id = v.voucher_id
             WHERE t.item_id = ?1 AND v.status = 'Posted' {date_clause}
             ORDER BY t.transaction_date ASC, t.rowid ASC"
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![item_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;

        for r_res in rows {
            let (m_type, qty, rate) = r_res?;
            let is_inward = matches!(m_type.as_str(), "IN" | "TRANSFER_IN" | "ADJUSTMENT_IN");

            if is_inward {
                let in_val = (qty as i128 * rate as i128) / 1000;
                current_qty += qty;
                current_value += in_val;
            } else {
                // Outward issue: consume at running weighted-average rate
                if current_qty > 0 {
                    let avg_rate = (current_value * 1000) / current_qty as i128;
                    let out_val = (qty as i128 * avg_rate) / 1000;
                    current_qty -= qty;
                    current_value = (current_value - out_val).max(0);
                } else {
                    current_qty -= qty;
                }
            }
        }

        let effective_rate = if current_qty > 0 && current_value > 0 {
            Paise(((current_value * 1000) / current_qty as i128) as i64)
        } else {
            Paise::ZERO
        };

        Ok(StockValuationResult {
            item_id: item_id.to_string(),
            valuation_method: "WeightedAverage".to_string(),
            total_closing_qty_milli: current_qty,
            total_closing_value_paise: Paise(current_value as i64),
            effective_unit_rate_paise: effective_rate,
        })
    }
}
