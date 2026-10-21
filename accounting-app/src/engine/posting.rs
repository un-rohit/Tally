use crate::audit::AuditLogger;
use crate::domain::money::Paise;
use crate::domain::voucher::{EntryType, Voucher, VoucherDraft, VoucherStatus};
use crate::error::{AccountingError, Result};
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde_json::json;
use uuid::Uuid;

pub struct PostingEngine;

impl PostingEngine {
    /// Posts a voucher using an atomic database transaction.
    /// Strictly validates that total debits equal total credits.
    /// Ensures that either the entire voucher and all its lines commit, or everything rolls back.
    pub fn post_voucher(
        conn: &mut Connection,
        draft: &VoucherDraft,
        user_id: Option<&str>,
    ) -> Result<Voucher> {
        // 1. Double-Entry Invariance Validation
        let (total_dr, total_cr, is_balanced) = draft.validate_balance();
        if !is_balanced {
            return Err(AccountingError::UnbalancedVoucher {
                debits: total_dr.0,
                credits: total_cr.0,
            });
        }

        // 2. Begin Atomic SQLite Transaction
        let tx = conn.transaction()?;

        // 3. Verify Financial Year Date Boundaries
        let (books_beg_str, fy_end_str): (String, String) = tx
            .query_row(
                "SELECT books_beginning_date, fy_end_date FROM financial_years 
                 WHERE fy_id = ?1 AND company_id = ?2",
                params![draft.fy_id, draft.company_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| {
                AccountingError::NotFound(format!("Financial Year ID {} not found", draft.fy_id))
            })?;

        let books_beg = NaiveDate::parse_from_str(&books_beg_str, "%Y-%m-%d")
            .map_err(|e| AccountingError::Validation(e.to_string()))?;
        let fy_end = NaiveDate::parse_from_str(&fy_end_str, "%Y-%m-%d")
            .map_err(|e| AccountingError::Validation(e.to_string()))?;

        if draft.voucher_date < books_beg {
            return Err(AccountingError::InvalidVoucherDate(format!(
                "Voucher date {} cannot precede books beginning date {}",
                draft.voucher_date, books_beg
            )));
        }
        if draft.voucher_date > fy_end {
            return Err(AccountingError::InvalidVoucherDate(format!(
                "Voucher date {} exceeds financial year end date {}",
                draft.voucher_date, fy_end
            )));
        }

        // 4. Duplicate Voucher Number Check
        let duplicate_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM vouchers 
             WHERE company_id = ?1 AND fy_id = ?2 AND voucher_type_id = ?3 AND voucher_number = ?4",
            params![
                draft.company_id,
                draft.fy_id,
                draft.voucher_type_id,
                draft.voucher_number
            ],
            |r| r.get(0),
        )?;

        if duplicate_count > 0 {
            return Err(AccountingError::Validation(format!(
                "Duplicate voucher number '{}' for this voucher type and financial year",
                draft.voucher_number
            )));
        }

        // 5. Insert Voucher as 'Draft'
        let voucher_id = Uuid::new_v4().to_string();
        let total_amount = total_dr; // total_dr == total_cr
        let now_str = Utc::now().to_rfc3339();

        tx.execute(
            "INSERT INTO vouchers (voucher_id, company_id, fy_id, voucher_type_id, voucher_number, 
                                  voucher_date, reference_no, reference_date, narration, 
                                  total_amount_paise, status) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'Draft')",
            params![
                voucher_id,
                draft.company_id,
                draft.fy_id,
                draft.voucher_type_id,
                draft.voucher_number,
                draft.voucher_date.format("%Y-%m-%d").to_string(),
                draft.reference_no,
                draft.reference_date.map(|d| d.format("%Y-%m-%d").to_string()),
                draft.narration,
                total_amount.0
            ],
        )?;

        // 6. Insert All Voucher Line Items
        for (idx, line) in draft.entries.iter().enumerate() {
            let entry_id = Uuid::new_v4().to_string();

            // Verify line ledger belongs to same company
            let ledger_company: String = tx
                .query_row(
                    "SELECT company_id FROM ledgers WHERE ledger_id = ?1",
                    params![line.ledger_id],
                    |r| r.get(0),
                )
                .map_err(|_| {
                    AccountingError::NotFound(format!("Ledger ID {} not found", line.ledger_id))
                })?;

            if ledger_company != draft.company_id {
                return Err(AccountingError::CompanyBoundaryViolation(format!(
                    "Ledger {} belongs to different company",
                    line.ledger_id
                )));
            }

            tx.execute(
                "INSERT INTO voucher_entries (entry_id, voucher_id, line_number, ledger_id, entry_type, amount_paise, line_narration) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    entry_id,
                    voucher_id,
                    (idx + 1) as i32,
                    line.ledger_id,
                    line.entry_type.as_str(),
                    line.amount.0,
                    line.narration
                ],
            )?;
        }

        // 7. Transition Voucher Status to 'Posted'
        // This will trigger SQLite trigger `trg_verify_voucher_balance_on_post`
        tx.execute(
            "UPDATE vouchers SET status = 'Posted', posted_at = ?1, posted_by_user_id = ?2 
             WHERE voucher_id = ?3",
            params![now_str, user_id, voucher_id],
        )?;

        // 8. Log to Immutable Audit Trail
        let audit_payload = json!({
            "voucher_id": voucher_id,
            "voucher_number": draft.voucher_number,
            "total_amount_paise": total_amount.0,
            "date": draft.voucher_date.to_string(),
            "lines_count": draft.entries.len(),
        });

        AuditLogger::log(
            &tx,
            &draft.company_id,
            "vouchers",
            &voucher_id,
            "POST",
            user_id,
            None,
            Some(&audit_payload),
        )?;

        // 9. Atomic Commit
        tx.commit()?;

        Ok(Voucher {
            voucher_id,
            company_id: draft.company_id.clone(),
            fy_id: draft.fy_id.clone(),
            voucher_type_id: draft.voucher_type_id.clone(),
            voucher_number: draft.voucher_number.clone(),
            voucher_date: draft.voucher_date,
            reference_no: draft.reference_no.clone(),
            reference_date: draft.reference_date,
            narration: draft.narration.clone(),
            total_amount,
            status: VoucherStatus::Posted,
            posted_at: Some(now_str),
            posted_by_user_id: user_id.map(String::from),
        })
    }

    /// Cancels a posted voucher and creates an immutable audit record.
    pub fn cancel_voucher(
        conn: &mut Connection,
        voucher_id: &str,
        user_id: Option<&str>,
        reason: &str,
    ) -> Result<()> {
        let tx = conn.transaction()?;

        let (status_str, company_id, v_num): (String, String, String) = tx.query_row(
            "SELECT status, company_id, voucher_number FROM vouchers WHERE voucher_id = ?1",
            params![voucher_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        if status_str == "Cancelled" {
            return Err(AccountingError::Validation(
                "Voucher is already cancelled".into(),
            ));
        }

        // Transition to Cancelled
        tx.execute(
            "UPDATE vouchers SET status = 'Cancelled', updated_at = (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) 
             WHERE voucher_id = ?1",
            params![voucher_id],
        )?;

        let audit_data = json!({
            "voucher_id": voucher_id,
            "voucher_number": v_num,
            "reason": reason,
        });

        AuditLogger::log(
            &tx,
            &company_id,
            "vouchers",
            voucher_id,
            "CANCEL",
            user_id,
            Some(&json!({"status": status_str})),
            Some(&audit_data),
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Reverses an existing voucher by creating and posting a compensating reversal voucher.
    pub fn reverse_voucher(
        conn: &mut Connection,
        original_voucher_id: &str,
        reversal_voucher_number: &str,
        reversal_date: NaiveDate,
        user_id: Option<&str>,
        reason: &str,
    ) -> Result<Voucher> {
        // Fetch original voucher details
        let (company_id, fy_id, vt_id, v_num, v_date_str, ref_no): (
            String,
            String,
            String,
            String,
            String,
            Option<String>,
        ) = conn.query_row(
            "SELECT company_id, fy_id, voucher_type_id, voucher_number, voucher_date, reference_no 
             FROM vouchers WHERE voucher_id = ?1",
            params![original_voucher_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?),
        ))?;

        let orig_date = NaiveDate::parse_from_str(&v_date_str, "%Y-%m-%d")
            .map_err(|e| AccountingError::Validation(e.to_string()))?;

        // Fetch original lines
        let mut stmt = conn.prepare(
            "SELECT ledger_id, entry_type, amount_paise, line_narration 
             FROM voucher_entries 
             WHERE voucher_id = ?1 
             ORDER BY line_number ASC",
        )?;

        let rows = stmt.query_map(params![original_voucher_id], |r| {
            let et_str: String = r.get(1)?;
            Ok((
                r.get::<_, String>(0)?,
                EntryType::from_str(&et_str).unwrap(),
                Paise(r.get::<_, i64>(2)?),
                r.get::<_, Option<String>>(3)?,
            ))
        })?;

        let mut reversal_entries = Vec::new();
        for row in rows {
            let (lid, etype, amount, _) = row?;
            reversal_entries.push(crate::domain::voucher::VoucherEntryDraft {
                ledger_id: lid,
                entry_type: etype.opposite(), // Invert Debit and Credit!
                amount,
                narration: Some(format!("Reversal of {}: {}", v_num, reason)),
            });
        }

        let draft = VoucherDraft {
            company_id,
            fy_id,
            voucher_type_id: vt_id,
            voucher_number: reversal_voucher_number.to_string(),
            voucher_date: reversal_date,
            reference_no: Some(ref_no.unwrap_or_else(|| format!("REV-{}", v_num))),
            reference_date: Some(orig_date),
            narration: Some(format!("Reversal of voucher {}. Reason: {}", v_num, reason)),
            entries: reversal_entries,
        };

        // Post the reversal voucher atomically
        Self::post_voucher(conn, &draft, user_id)
    }
}
