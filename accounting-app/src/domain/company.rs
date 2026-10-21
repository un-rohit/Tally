use crate::error::{AccountingError, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub company_id: String,
    pub company_number: String,
    pub name: String,
    pub mailing_name: String,
    pub address: Option<String>,
    pub state_code: String,
    pub country: String,
    pub pincode: Option<String>,
    pub pan: Option<String>,
    pub currency_symbol: String,
    pub currency_formal_name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialYear {
    pub fy_id: String,
    pub company_id: String,
    pub fy_start_date: NaiveDate,
    pub fy_end_date: NaiveDate,
    pub books_beginning_date: NaiveDate,
    pub is_closed: bool,
}

impl Company {
    pub fn create(
        conn: &mut Connection,
        company_number: &str,
        name: &str,
        mailing_name: &str,
        state_code: &str,
        fy_start: NaiveDate,
        fy_end: NaiveDate,
        books_beginning: NaiveDate,
    ) -> Result<(Company, FinancialYear)> {
        if books_beginning < fy_start {
            return Err(AccountingError::Validation(
                "Books beginning date cannot precede financial year start date".into(),
            ));
        }
        if fy_end <= fy_start {
            return Err(AccountingError::Validation(
                "Financial year end date must be after start date".into(),
            ));
        }

        let tx = conn.transaction()?;
        let company_id = Uuid::new_v4().to_string();
        let fy_id = Uuid::new_v4().to_string();

        tx.execute(
            "INSERT INTO companies (company_id, company_number, name, mailing_name, state_code) 
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![company_id, company_number, name, mailing_name, state_code],
        )?;

        tx.execute(
            "INSERT INTO financial_years (fy_id, company_id, fy_start_date, fy_end_date, books_beginning_date) 
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                fy_id,
                company_id,
                fy_start.format("%Y-%m-%d").to_string(),
                fy_end.format("%Y-%m-%d").to_string(),
                books_beginning.format("%Y-%m-%d").to_string(),
            ],
        )?;

        // Seed default voucher types for the new company
        let default_types = [
            ("Contra", "Contra"),
            ("Payment", "Payment"),
            ("Receipt", "Receipt"),
            ("Journal", "Journal"),
            ("Sales", "Sales"),
            ("Purchase", "Purchase"),
            ("Credit Note", "CreditNote"),
            ("Debit Note", "DebitNote"),
            ("Stock Journal", "StockJournal"),
        ];

        for (v_name, base_type) in default_types {
            let vt_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO voucher_types (voucher_type_id, company_id, name, base_type) 
                 VALUES (?1, ?2, ?3, ?4)",
                params![vt_id, company_id, v_name, base_type],
            )?;
        }

        tx.commit()?;

        let comp = Company {
            company_id: company_id.clone(),
            company_number: company_number.to_string(),
            name: name.to_string(),
            mailing_name: mailing_name.to_string(),
            address: None,
            state_code: state_code.to_string(),
            country: "India".to_string(),
            pincode: None,
            pan: None,
            currency_symbol: "₹".to_string(),
            currency_formal_name: "INR".to_string(),
            is_active: true,
        };

        let fy = FinancialYear {
            fy_id,
            company_id,
            fy_start_date: fy_start,
            fy_end_date: fy_end,
            books_beginning_date: books_beginning,
            is_closed: false,
        };

        Ok((comp, fy))
    }

    pub fn get_by_number(conn: &Connection, company_number: &str) -> Result<Option<Company>> {
        let mut stmt = conn.prepare(
            "SELECT company_id, company_number, name, mailing_name, address, state_code, 
                    country, pincode, pan, currency_symbol, currency_formal_name, is_active 
             FROM companies WHERE company_number = ?1",
        )?;

        let mut rows = stmt.query(params![company_number])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Company {
                company_id: row.get(0)?,
                company_number: row.get(1)?,
                name: row.get(2)?,
                mailing_name: row.get(3)?,
                address: row.get(4)?,
                state_code: row.get(5)?,
                country: row.get(6)?,
                pincode: row.get(7)?,
                pan: row.get(8)?,
                currency_symbol: row.get(9)?,
                currency_formal_name: row.get(10)?,
                is_active: row.get::<_, i64>(11)? == 1,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_active_fy(conn: &Connection, company_id: &str) -> Result<Option<FinancialYear>> {
        let mut stmt = conn.prepare(
            "SELECT fy_id, company_id, fy_start_date, fy_end_date, books_beginning_date, is_closed 
             FROM financial_years 
             WHERE company_id = ?1 AND is_closed = 0 
             ORDER BY fy_start_date DESC LIMIT 1",
        )?;

        let mut rows = stmt.query(params![company_id])?;
        if let Some(row) = rows.next()? {
            let start_str: String = row.get(2)?;
            let end_str: String = row.get(3)?;
            let books_str: String = row.get(4)?;

            Ok(Some(FinancialYear {
                fy_id: row.get(0)?,
                company_id: row.get(1)?,
                fy_start_date: NaiveDate::parse_from_str(&start_str, "%Y-%m-%d")
                    .map_err(|e| AccountingError::Validation(e.to_string()))?,
                fy_end_date: NaiveDate::parse_from_str(&end_str, "%Y-%m-%d")
                    .map_err(|e| AccountingError::Validation(e.to_string()))?,
                books_beginning_date: NaiveDate::parse_from_str(&books_str, "%Y-%m-%d")
                    .map_err(|e| AccountingError::Validation(e.to_string()))?,
                is_closed: row.get::<_, i64>(5)? == 1,
            }))
        } else {
            Ok(None)
        }
    }
}
