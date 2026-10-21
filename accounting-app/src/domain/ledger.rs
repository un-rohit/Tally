use crate::domain::money::Paise;
use crate::error::{AccountingError, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountNature {
    Asset,
    Liability,
    Income,
    Expense,
}

impl AccountNature {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountNature::Asset => "Asset",
            AccountNature::Liability => "Liability",
            AccountNature::Income => "Income",
            AccountNature::Expense => "Expense",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "Asset" => Ok(AccountNature::Asset),
            "Liability" => Ok(AccountNature::Liability),
            "Income" => Ok(AccountNature::Income),
            "Expense" => Ok(AccountNature::Expense),
            other => Err(AccountingError::Validation(format!(
                "Invalid account nature: {}",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountGroup {
    pub group_id: String,
    pub company_id: String,
    pub name: String,
    pub parent_group_id: Option<String>,
    pub nature: AccountNature,
    pub affects_gross_profit: bool,
    pub is_predefined: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    pub ledger_id: String,
    pub company_id: String,
    pub group_id: String,
    pub name: String,
    pub code: Option<String>,
    pub opening_balance: Paise,
    pub opening_balance_type: Option<String>, // 'Dr' or 'Cr'
    pub is_bill_wise: bool,
    pub is_active: bool,
}

impl AccountGroup {
    /// Seeds the standard 28 accounting groups for a company
    pub fn seed_standard_groups(conn: &mut Connection, company_id: &str) -> Result<()> {
        let tx = conn.transaction()?;

        // Primary Groups (parent_id = NULL)
        let primary_groups = [
            ("Capital Account", AccountNature::Liability, false),
            ("Loans (Liability)", AccountNature::Liability, false),
            ("Current Liabilities", AccountNature::Liability, false),
            ("Fixed Assets", AccountNature::Asset, false),
            ("Investments", AccountNature::Asset, false),
            ("Current Assets", AccountNature::Asset, false),
            ("Branch / Divisions", AccountNature::Liability, false),
            ("Misc. Expenses (ASSET)", AccountNature::Asset, false),
            ("Sales Accounts", AccountNature::Income, true),
            ("Purchase Accounts", AccountNature::Expense, true),
            ("Direct Incomes", AccountNature::Income, true),
            ("Indirect Incomes", AccountNature::Income, false),
            ("Direct Expenses", AccountNature::Expense, true),
            ("Indirect Expenses", AccountNature::Expense, false),
        ];

        let mut group_ids = std::collections::HashMap::new();

        for (name, nature, affects_gp) in primary_groups {
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO account_groups (group_id, company_id, name, parent_group_id, nature, affects_gross_profit, is_predefined) 
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5, 1)",
                params![id, company_id, name, nature.as_str(), affects_gp as i32],
            )?;
            group_ids.insert(name.to_string(), id);
        }

        // Secondary / Sub-groups
        let secondary_groups = [
            ("Reserves & Surplus", "Capital Account", AccountNature::Liability, false),
            ("Bank OD A/c", "Loans (Liability)", AccountNature::Liability, false),
            ("Secured Loans", "Loans (Liability)", AccountNature::Liability, false),
            ("Unsecured Loans", "Loans (Liability)", AccountNature::Liability, false),
            ("Duties & Taxes", "Current Liabilities", AccountNature::Liability, false),
            ("Provisions", "Current Liabilities", AccountNature::Liability, false),
            ("Sundry Creditors", "Current Liabilities", AccountNature::Liability, false),
            ("Bank Accounts", "Current Assets", AccountNature::Asset, false),
            ("Cash-in-hand", "Current Assets", AccountNature::Asset, false),
            ("Deposits (Asset)", "Current Assets", AccountNature::Asset, false),
            ("Loans & Advances (Asset)", "Current Assets", AccountNature::Asset, false),
            ("Stock-in-hand", "Current Assets", AccountNature::Asset, false),
            ("Sundry Debtors", "Current Assets", AccountNature::Asset, false),
        ];

        for (name, parent_name, nature, affects_gp) in secondary_groups {
            let parent_id = group_ids.get(parent_name).ok_or_else(|| {
                AccountingError::NotFound(format!("Parent group '{}' not found", parent_name))
            })?;
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO account_groups (group_id, company_id, name, parent_group_id, nature, affects_gross_profit, is_predefined) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
                params![id, company_id, name, parent_id, nature.as_str(), affects_gp as i32],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_by_name(conn: &Connection, company_id: &str, name: &str) -> Result<Option<AccountGroup>> {
        let mut stmt = conn.prepare(
            "SELECT group_id, company_id, name, parent_group_id, nature, affects_gross_profit, is_predefined 
             FROM account_groups WHERE company_id = ?1 AND name = ?2",
        )?;

        let mut rows = stmt.query(params![company_id, name])?;
        if let Some(row) = rows.next()? {
            let nat_str: String = row.get(4)?;
            Ok(Some(AccountGroup {
                group_id: row.get(0)?,
                company_id: row.get(1)?,
                name: row.get(2)?,
                parent_group_id: row.get(3)?,
                nature: AccountNature::from_str(&nat_str)?,
                affects_gross_profit: row.get::<_, i32>(5)? == 1,
                is_predefined: row.get::<_, i32>(6)? == 1,
            }))
        } else {
            Ok(None)
        }
    }
}

impl Ledger {
    pub fn create(
        conn: &Connection,
        company_id: &str,
        group_id: &str,
        name: &str,
        opening_balance: Paise,
        opening_type: Option<&str>,
        is_bill_wise: bool,
    ) -> Result<Ledger> {
        // Validate group exists
        let group_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM account_groups WHERE group_id = ?1 AND company_id = ?2",
            params![group_id, company_id],
            |r| r.get(0),
        )?;

        if group_exists == 0 {
            return Err(AccountingError::NotFound(format!(
                "Account group ID {} not found in company",
                group_id
            )));
        }

        let ledger_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO ledgers (ledger_id, company_id, group_id, name, opening_balance_paise, opening_balance_type, is_bill_wise) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                ledger_id,
                company_id,
                group_id,
                name,
                opening_balance.0,
                opening_type,
                is_bill_wise as i32
            ],
        )?;

        Ok(Ledger {
            ledger_id,
            company_id: company_id.to_string(),
            group_id: group_id.to_string(),
            name: name.to_string(),
            code: None,
            opening_balance,
            opening_balance_type: opening_type.map(String::from),
            is_bill_wise,
            is_active: true,
        })
    }

    pub fn get_by_name(conn: &Connection, company_id: &str, name: &str) -> Result<Option<Ledger>> {
        let mut stmt = conn.prepare(
            "SELECT ledger_id, company_id, group_id, name, code, opening_balance_paise, 
                    opening_balance_type, is_bill_wise, is_active 
             FROM ledgers WHERE company_id = ?1 AND name = ?2",
        )?;

        let mut rows = stmt.query(params![company_id, name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Ledger {
                ledger_id: row.get(0)?,
                company_id: row.get(1)?,
                group_id: row.get(2)?,
                name: row.get(3)?,
                code: row.get(4)?,
                opening_balance: Paise(row.get(5)?),
                opening_balance_type: row.get(6)?,
                is_bill_wise: row.get::<_, i32>(7)? == 1,
                is_active: row.get::<_, i32>(8)? == 1,
            }))
        } else {
            Ok(None)
        }
    }

    /// Computes the net closing balance of a ledger considering opening balance and posted voucher entries.
    pub fn get_closing_balance(conn: &Connection, ledger_id: &str) -> Result<Paise> {
        let (opening, op_type, nature): (i64, Option<String>, String) = conn.query_row(
            "SELECT l.opening_balance_paise, l.opening_balance_type, g.nature 
             FROM ledgers l JOIN account_groups g ON l.group_id = g.group_id 
             WHERE l.ledger_id = ?1",
            params![ledger_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        // Signed opening: Dr is positive, Cr is negative (for Asset/Expense)
        // or Dr is negative, Cr is positive (for Liability/Income)
        let mut balance = match op_type.as_deref() {
            Some("Dr") => opening,
            Some("Cr") => -opening,
            _ => 0,
        };

        // Query only POSTED vouchers
        let mut stmt = conn.prepare(
            "SELECT ve.entry_type, SUM(ve.amount_paise) 
             FROM voucher_entries ve 
             JOIN vouchers v ON ve.voucher_id = v.voucher_id 
             WHERE ve.ledger_id = ?1 AND v.status = 'Posted' 
             GROUP BY ve.entry_type",
        )?;

        let rows = stmt.query_map(params![ledger_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;

        for row in rows {
            let (entry_type, total) = row?;
            if entry_type == "Dr" {
                balance = balance.checked_add(total).ok_or_else(|| {
                    AccountingError::MathOverflow("Debit addition overflow".into())
                })?;
            } else {
                balance = balance.checked_sub(total).ok_or_else(|| {
                    AccountingError::MathOverflow("Credit subtraction overflow".into())
                })?;
            }
        }

        // If Liability or Income, a credit balance is naturally positive
        if nature == "Liability" || nature == "Income" {
            balance = -balance;
        }

        Ok(Paise(balance))
    }
}
