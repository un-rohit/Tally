use crate::domain::billwise::{BillAllocation, BillAllocationDraft, BillRefType};
use crate::domain::money::Paise;
use crate::domain::voucher::EntryType;
use crate::error::{AccountingError, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutstandingBill {
    pub ledger_id: String,
    pub party_name: String,
    pub bill_ref: String,
    pub bill_date: NaiveDate,
    pub due_date: Option<NaiveDate>,
    pub credit_days: u32,
    pub original_amount_paise: Paise,
    pub settled_amount_paise: Paise,
    pub outstanding_amount_paise: Paise,
    pub overdue_days: i64,
    pub is_overdue: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyAgingRow {
    pub ledger_id: String,
    pub party_name: String,
    pub current_paise: Paise,
    pub days_1_30_paise: Paise,
    pub days_31_60_paise: Paise,
    pub days_61_90_paise: Paise,
    pub over_90_paise: Paise,
    pub total_outstanding_paise: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillAllocationHistoryItem {
    pub voucher_id: String,
    pub voucher_number: String,
    pub voucher_date: String,
    pub voucher_type: String,
    pub ref_type: String,
    pub ref_name: String,
    pub amount_paise: Paise,
    pub entry_type: String,
}

pub struct BillWiseEngine;

impl BillWiseEngine {
    /// Records a bill allocation within an atomic transaction.
    /// Validates Against Reference allocations against original bill balance.
    pub fn record_allocation(
        tx: &Transaction,
        company_id: &str,
        entry_id: &str,
        draft: &BillAllocationDraft,
    ) -> Result<BillAllocation> {
        if draft.amount_paise.0 <= 0 {
            return Err(AccountingError::Validation(
                "Bill allocation amount must be greater than zero".to_string(),
            ));
        }

        let ref_name = draft.ref_name.trim();
        if ref_name.is_empty() {
            return Err(AccountingError::Validation(
                "Bill reference name cannot be empty".to_string(),
            ));
        }

        // 1. If AgstRef, validate against available unallocated amount of NewRef/Advance
        if draft.ref_type == BillRefType::AgstRef {
            let original_amt: i64 = tx
                .query_row(
                    "SELECT COALESCE(SUM(b.amount_paise), 0)
                     FROM bill_allocations b
                     JOIN voucher_entries e ON b.entry_id = e.entry_id
                     JOIN vouchers v ON e.voucher_id = v.voucher_id
                     WHERE b.ledger_id = ?1 AND b.ref_name = ?2 
                       AND b.ref_type IN ('NewRef', 'Advance')
                       AND v.status = 'Posted'",
                    params![draft.ledger_id, ref_name],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            if original_amt == 0 {
                return Err(AccountingError::Validation(format!(
                    "No active bill or advance found with reference '{}' for this party",
                    ref_name
                )));
            }

            let already_allocated: i64 = tx
                .query_row(
                    "SELECT COALESCE(SUM(b.amount_paise), 0)
                     FROM bill_allocations b
                     JOIN voucher_entries e ON b.entry_id = e.entry_id
                     JOIN vouchers v ON e.voucher_id = v.voucher_id
                     WHERE b.ledger_id = ?1 AND b.ref_name = ?2 
                       AND b.ref_type = 'AgstRef'
                       AND v.status = 'Posted'",
                    params![draft.ledger_id, ref_name],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let remaining = original_amt - already_allocated;
            if draft.amount_paise.0 > remaining {
                return Err(AccountingError::Validation(format!(
                    "Allocation of {} exceeds available outstanding amount of {} for bill '{}'",
                    draft.amount_paise.format_inr(),
                    Paise(remaining).format_inr(),
                    ref_name
                )));
            }
        }

        // 2. If NewRef, prevent duplicates in posted vouchers for same party
        if draft.ref_type == BillRefType::NewRef {
            let duplicate_count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM bill_allocations b
                     JOIN voucher_entries e ON b.entry_id = e.entry_id
                     JOIN vouchers v ON e.voucher_id = v.voucher_id
                     WHERE b.ledger_id = ?1 AND b.ref_name = ?2 AND b.ref_type = 'NewRef'
                       AND v.status = 'Posted'",
                    params![draft.ledger_id, ref_name],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            if duplicate_count > 0 {
                return Err(AccountingError::Validation(format!(
                    "Bill reference '{}' already exists for this party",
                    ref_name
                )));
            }
        }

        let allocation_id = Uuid::new_v4().to_string();
        let due_date_str = draft.due_date.map(|d| d.format("%Y-%m-%d").to_string());
        let entry_type_str = match draft.entry_type {
            EntryType::Dr => "Dr",
            EntryType::Cr => "Cr",
        };

        tx.execute(
            "INSERT INTO bill_allocations (
                allocation_id, company_id, entry_id, ledger_id, ref_type, ref_name, 
                credit_days, due_date, amount_paise, entry_type
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                allocation_id,
                company_id,
                entry_id,
                draft.ledger_id,
                draft.ref_type.as_str(),
                ref_name,
                draft.credit_days,
                due_date_str,
                draft.amount_paise.0,
                entry_type_str,
            ],
        )?;

        Ok(BillAllocation {
            allocation_id,
            company_id: Some(company_id.to_string()),
            entry_id: entry_id.to_string(),
            ledger_id: draft.ledger_id.clone(),
            ref_type: draft.ref_type,
            ref_name: ref_name.to_string(),
            credit_days: draft.credit_days,
            due_date: draft.due_date,
            amount_paise: draft.amount_paise,
            entry_type: draft.entry_type,
        })
    }

    /// Fetches all outstanding bills for a customer or supplier as of a specified date.
    pub fn get_outstanding_bills_for_party(
        conn: &Connection,
        company_id: &str,
        ledger_id: &str,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Vec<OutstandingBill>> {
        let party_name: String = conn
            .query_row(
                "SELECT name FROM ledgers WHERE ledger_id = ?1 AND company_id = ?2",
                params![ledger_id, company_id],
                |r| r.get(0),
            )
            .map_err(|_| AccountingError::NotFound(format!("Ledger {} not found", ledger_id)))?;

        let as_of = as_of_date.unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Get all NewRef and Advance bills
        let sql_bills = "SELECT b.ref_name, v.voucher_date, b.due_date, b.credit_days, b.amount_paise
                         FROM bill_allocations b
                         JOIN voucher_entries e ON b.entry_id = e.entry_id
                         JOIN vouchers v ON e.voucher_id = v.voucher_id
                         WHERE b.ledger_id = ?1 AND v.company_id = ?2 
                           AND b.ref_type IN ('NewRef', 'Advance')
                           AND v.status = 'Posted'
                           AND v.voucher_date <= ?3
                         ORDER BY v.voucher_date ASC, b.ref_name ASC";

        let mut stmt = conn.prepare(sql_bills)?;
        let as_of_str = as_of.format("%Y-%m-%d").to_string();

        let rows = stmt.query_map(params![ledger_id, company_id, as_of_str], |r| {
            let due_opt: Option<String> = r.get(2)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                due_opt,
                r.get::<_, i64>(3)? as u32,
                r.get::<_, i64>(4)?,
            ))
        })?;

        let mut outstanding_list = Vec::new();

        for r_res in rows {
            let (ref_name, bill_date_str, due_date_str, credit_days, orig_amt) = r_res?;
            let bill_date = NaiveDate::parse_from_str(&bill_date_str, "%Y-%m-%d")
                .unwrap_or(as_of);
            let due_date = due_date_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

            // Query total settled against this ref_name as of date
            let settled_amt: i64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(b.amount_paise), 0)
                     FROM bill_allocations b
                     JOIN voucher_entries e ON b.entry_id = e.entry_id
                     JOIN vouchers v ON e.voucher_id = v.voucher_id
                     WHERE b.ledger_id = ?1 AND b.ref_name = ?2 
                       AND b.ref_type = 'AgstRef'
                       AND v.status = 'Posted'
                       AND v.voucher_date <= ?3",
                    params![ledger_id, ref_name, as_of_str],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let remaining = orig_amt - settled_amt;
            if remaining > 0 {
                let effective_due = due_date.unwrap_or_else(|| {
                    bill_date + chrono::Duration::days(credit_days as i64)
                });

                let overdue_days = (as_of - effective_due).num_days();
                let is_overdue = overdue_days > 0;

                outstanding_list.push(OutstandingBill {
                    ledger_id: ledger_id.to_string(),
                    party_name: party_name.clone(),
                    bill_ref: ref_name,
                    bill_date,
                    due_date: Some(effective_due),
                    credit_days,
                    original_amount_paise: Paise(orig_amt),
                    settled_amount_paise: Paise(settled_amt),
                    outstanding_amount_paise: Paise(remaining),
                    overdue_days: overdue_days.max(0),
                    is_overdue,
                });
            }
        }

        Ok(outstanding_list)
    }

    /// Generates Receivables or Payables Ageing Analysis.
    pub fn generate_aging_report(
        conn: &Connection,
        company_id: &str,
        is_receivable: bool,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Vec<PartyAgingRow>> {
        let parent_group = if is_receivable {
            "Sundry Debtors"
        } else {
            "Sundry Creditors"
        };

        let as_of = as_of_date.unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Find all ledgers belonging to group
        let sql = "SELECT l.ledger_id, l.name
                   FROM ledgers l
                   JOIN account_groups g ON l.group_id = g.group_id
                   WHERE l.company_id = ?1 AND (g.name = ?2 OR g.parent_group_id IN (
                       SELECT group_id FROM account_groups WHERE name = ?2 AND company_id = ?1
                   ))
                   ORDER BY l.name ASC";

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![company_id, parent_group], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;

        let mut aging_rows = Vec::new();

        for r_res in rows {
            let (ledger_id, name) = r_res?;
            let bills = Self::get_outstanding_bills_for_party(conn, company_id, &ledger_id, Some(as_of))?;

            if bills.is_empty() {
                continue;
            }

            let mut current: i64 = 0;
            let mut days_1_30: i64 = 0;
            let mut days_31_60: i64 = 0;
            let mut days_61_90: i64 = 0;
            let mut over_90: i64 = 0;
            let mut total: i64 = 0;

            for b in bills {
                let amt = b.outstanding_amount_paise.0;
                total += amt;

                if !b.is_overdue || b.overdue_days <= 0 {
                    current += amt;
                } else if b.overdue_days <= 30 {
                    days_1_30 += amt;
                } else if b.overdue_days <= 60 {
                    days_31_60 += amt;
                } else if b.overdue_days <= 90 {
                    days_61_90 += amt;
                } else {
                    over_90 += amt;
                }
            }

            aging_rows.push(PartyAgingRow {
                ledger_id,
                party_name: name,
                current_paise: Paise(current),
                days_1_30_paise: Paise(days_1_30),
                days_31_60_paise: Paise(days_31_60),
                days_61_90_paise: Paise(days_61_90),
                over_90_paise: Paise(over_90),
                total_outstanding_paise: Paise(total),
            });
        }

        Ok(aging_rows)
    }

    /// Fetches full audit history of a bill (originating invoice and all settlement payments/receipts).
    pub fn get_bill_allocation_history(
        conn: &Connection,
        company_id: &str,
        ledger_id: &str,
        ref_name: &str,
    ) -> Result<Vec<BillAllocationHistoryItem>> {
        let sql = "SELECT v.voucher_id, v.voucher_number, v.voucher_date, vt.name, 
                          b.ref_type, b.ref_name, b.amount_paise, b.entry_type
                   FROM bill_allocations b
                   JOIN voucher_entries e ON b.entry_id = e.entry_id
                   JOIN vouchers v ON e.voucher_id = v.voucher_id
                   JOIN voucher_types vt ON v.voucher_type_id = vt.voucher_type_id
                   WHERE b.company_id = ?1 AND b.ledger_id = ?2 AND b.ref_name = ?3
                     AND v.status = 'Posted'
                   ORDER BY v.voucher_date ASC, b.rowid ASC";

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![company_id, ledger_id, ref_name], |r| {
            Ok(BillAllocationHistoryItem {
                voucher_id: r.get(0)?,
                voucher_number: r.get(1)?,
                voucher_date: r.get(2)?,
                voucher_type: r.get(3)?,
                ref_type: r.get(4)?,
                ref_name: r.get(5)?,
                amount_paise: Paise(r.get(6)?),
                entry_type: r.get(7)?,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }
}
