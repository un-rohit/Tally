use crate::domain::ledger::AccountNature;
use crate::domain::money::Paise;
use crate::error::{AccountingError, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialBalanceItem {
    pub ledger_id: String,
    pub ledger_name: String,
    pub group_name: String,
    pub nature: AccountNature,
    pub debit_balance: Paise,
    pub credit_balance: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialBalanceReport {
    pub items: Vec<TrialBalanceItem>,
    pub total_debit: Paise,
    pub total_credit: Paise,
    pub is_balanced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeExpenseItem {
    pub ledger_id: String,
    pub ledger_name: String,
    pub group_name: String,
    pub amount: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitAndLossReport {
    pub direct_incomes: Vec<IncomeExpenseItem>,
    pub direct_expenses: Vec<IncomeExpenseItem>,
    pub total_direct_income: Paise,
    pub total_direct_expense: Paise,
    pub gross_profit: Paise, // positive is profit, negative is loss

    pub indirect_incomes: Vec<IncomeExpenseItem>,
    pub indirect_expenses: Vec<IncomeExpenseItem>,
    pub total_indirect_income: Paise,
    pub total_indirect_expense: Paise,
    pub net_profit: Paise, // positive is profit, negative is loss
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetItem {
    pub ledger_id: String,
    pub ledger_name: String,
    pub group_name: String,
    pub amount: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetReport {
    pub liabilities: Vec<BalanceSheetItem>,
    pub assets: Vec<BalanceSheetItem>,
    pub total_liabilities: Paise,
    pub total_assets: Paise,
    pub net_profit_transferred: Paise,
    pub total_with_profit: Paise,
    pub is_balanced: bool,
}

pub struct ReportEngine;

impl ReportEngine {
    /// Generates the Trial Balance for a company across all posted vouchers up to a specified date.
    pub fn generate_trial_balance(
        conn: &Connection,
        company_id: &str,
        as_of_date: Option<NaiveDate>,
    ) -> Result<TrialBalanceReport> {
        let date_clause = if let Some(d) = as_of_date {
            format!("AND v.voucher_date <= '{}'", d.format("%Y-%m-%d"))
        } else {
            "".to_string()
        };

        let sql = format!(
            "SELECT 
                l.ledger_id, 
                l.name, 
                g.name, 
                g.nature,
                l.opening_balance_paise,
                l.opening_balance_type,
                COALESCE(SUM(CASE WHEN ve.entry_type = 'Dr' THEN ve.amount_paise ELSE 0 END), 0) AS total_dr,
                COALESCE(SUM(CASE WHEN ve.entry_type = 'Cr' THEN ve.amount_paise ELSE 0 END), 0) AS total_cr
             FROM ledgers l
             JOIN account_groups g ON l.group_id = g.group_id
             LEFT JOIN voucher_entries ve ON l.ledger_id = ve.ledger_id
             LEFT JOIN vouchers v ON ve.voucher_id = v.voucher_id AND v.status = 'Posted' {date_clause}
             WHERE l.company_id = ?1
             GROUP BY l.ledger_id, l.name, g.name, g.nature, l.opening_balance_paise, l.opening_balance_type
             ORDER BY g.nature, g.name, l.name"
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![company_id], |r| {
            let nat_str: String = r.get(3)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                AccountNature::from_str(&nat_str).unwrap(),
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })?;

        let mut items = Vec::new();
        let mut sum_dr = 0i64;
        let mut sum_cr = 0i64;

        for row in rows {
            let (lid, lname, gname, nature, op_paise, op_type, tx_dr, tx_cr) = row?;

            let mut net_signed: i64 = match op_type.as_deref() {
                Some("Dr") => op_paise,
                Some("Cr") => -op_paise,
                _ => 0,
            };

            net_signed = net_signed
                .checked_add(tx_dr)
                .ok_or_else(|| AccountingError::MathOverflow("Debit addition overflow".into()))?
                .checked_sub(tx_cr)
                .ok_or_else(|| AccountingError::MathOverflow("Credit subtraction overflow".into()))?;

            if net_signed == 0 && tx_dr == 0 && tx_cr == 0 && op_paise == 0 {
                continue; // Skip zero-balance untouched ledgers
            }

            let (dr_bal, cr_bal) = if net_signed > 0 {
                sum_dr = sum_dr.checked_add(net_signed).ok_or_else(|| {
                    AccountingError::MathOverflow("Trial balance debit sum overflow".into())
                })?;
                (Paise(net_signed), Paise::ZERO)
            } else if net_signed < 0 {
                let abs_val = net_signed.abs();
                sum_cr = sum_cr.checked_add(abs_val).ok_or_else(|| {
                    AccountingError::MathOverflow("Trial balance credit sum overflow".into())
                })?;
                (Paise::ZERO, Paise(abs_val))
            } else {
                (Paise::ZERO, Paise::ZERO)
            };

            items.push(TrialBalanceItem {
                ledger_id: lid,
                ledger_name: lname,
                group_name: gname,
                nature,
                debit_balance: dr_bal,
                credit_balance: cr_bal,
            });
        }

        let is_balanced = sum_dr == sum_cr;

        Ok(TrialBalanceReport {
            items,
            total_debit: Paise(sum_dr),
            total_credit: Paise(sum_cr),
            is_balanced,
        })
    }

    /// Generates Profit & Loss statement (Trading account & Net profit).
    pub fn generate_profit_and_loss(
        conn: &Connection,
        company_id: &str,
        as_of_date: Option<NaiveDate>,
    ) -> Result<ProfitAndLossReport> {
        let tb = Self::generate_trial_balance(conn, company_id, as_of_date)?;

        let mut direct_incomes = Vec::new();
        let mut direct_expenses = Vec::new();
        let mut indirect_incomes = Vec::new();
        let mut indirect_expenses = Vec::new();

        let mut sum_dir_inc = 0i64;
        let mut sum_dir_exp = 0i64;
        let mut sum_ind_inc = 0i64;
        let mut sum_ind_exp = 0i64;

        for item in tb.items {
            match item.nature {
                AccountNature::Income => {
                    // Check if group affects gross profit
                    let affects_gp: bool = conn.query_row(
                        "SELECT affects_gross_profit FROM account_groups WHERE company_id = ?1 AND name = ?2",
                        params![company_id, item.group_name],
                        |r| Ok(r.get::<_, i32>(0)? == 1),
                    ).unwrap_or(false);

                    let amt = item.credit_balance.0 - item.debit_balance.0;
                    if affects_gp {
                        sum_dir_inc = sum_dir_inc.saturating_add(amt);
                        direct_incomes.push(IncomeExpenseItem {
                            ledger_id: item.ledger_id,
                            ledger_name: item.ledger_name,
                            group_name: item.group_name,
                            amount: Paise(amt),
                        });
                    } else {
                        sum_ind_inc = sum_ind_inc.saturating_add(amt);
                        indirect_incomes.push(IncomeExpenseItem {
                            ledger_id: item.ledger_id,
                            ledger_name: item.ledger_name,
                            group_name: item.group_name,
                            amount: Paise(amt),
                        });
                    }
                }
                AccountNature::Expense => {
                    let affects_gp: bool = conn.query_row(
                        "SELECT affects_gross_profit FROM account_groups WHERE company_id = ?1 AND name = ?2",
                        params![company_id, item.group_name],
                        |r| Ok(r.get::<_, i32>(0)? == 1),
                    ).unwrap_or(false);

                    let amt = item.debit_balance.0 - item.credit_balance.0;
                    if affects_gp {
                        sum_dir_exp = sum_dir_exp.saturating_add(amt);
                        direct_expenses.push(IncomeExpenseItem {
                            ledger_id: item.ledger_id,
                            ledger_name: item.ledger_name,
                            group_name: item.group_name,
                            amount: Paise(amt),
                        });
                    } else {
                        sum_ind_exp = sum_ind_exp.saturating_add(amt);
                        indirect_expenses.push(IncomeExpenseItem {
                            ledger_id: item.ledger_id,
                            ledger_name: item.ledger_name,
                            group_name: item.group_name,
                            amount: Paise(amt),
                        });
                    }
                }
                _ => {} // Ignore Asset & Liability in P&L
            }
        }

        let gross_profit = sum_dir_inc - sum_dir_exp;
        let net_profit = gross_profit + sum_ind_inc - sum_ind_exp;

        Ok(ProfitAndLossReport {
            direct_incomes,
            direct_expenses,
            total_direct_income: Paise(sum_dir_inc),
            total_direct_expense: Paise(sum_dir_exp),
            gross_profit: Paise(gross_profit),
            indirect_incomes,
            indirect_expenses,
            total_indirect_income: Paise(sum_ind_inc),
            total_indirect_expense: Paise(sum_ind_exp),
            net_profit: Paise(net_profit),
        })
    }

    /// Generates Balance Sheet and asserts Liabilities + Net Profit = Assets.
    pub fn generate_balance_sheet(
        conn: &Connection,
        company_id: &str,
        as_of_date: Option<NaiveDate>,
    ) -> Result<BalanceSheetReport> {
        let tb = Self::generate_trial_balance(conn, company_id, as_of_date)?;
        let pnl = Self::generate_profit_and_loss(conn, company_id, as_of_date)?;

        let mut liabilities = Vec::new();
        let mut assets = Vec::new();
        let mut sum_liab = 0i64;
        let mut sum_assets = 0i64;

        for item in tb.items {
            match item.nature {
                AccountNature::Liability => {
                    let amt = item.credit_balance.0 - item.debit_balance.0;
                    sum_liab = sum_liab.saturating_add(amt);
                    liabilities.push(BalanceSheetItem {
                        ledger_id: item.ledger_id,
                        ledger_name: item.ledger_name,
                        group_name: item.group_name,
                        amount: Paise(amt),
                    });
                }
                AccountNature::Asset => {
                    let amt = item.debit_balance.0 - item.credit_balance.0;
                    sum_assets = sum_assets.saturating_add(amt);
                    assets.push(BalanceSheetItem {
                        ledger_id: item.ledger_id,
                        ledger_name: item.ledger_name,
                        group_name: item.group_name,
                        amount: Paise(amt),
                    });
                }
                _ => {} // Income and Expense handled via Net Profit
            }
        }

        let net_profit_val = pnl.net_profit.0;
        let total_liab_with_profit = sum_liab + net_profit_val;
        let is_balanced = total_liab_with_profit == sum_assets;

        Ok(BalanceSheetReport {
            liabilities,
            assets,
            total_liabilities: Paise(sum_liab),
            total_assets: Paise(sum_assets),
            net_profit_transferred: Paise(net_profit_val),
            total_with_profit: Paise(total_liab_with_profit),
            is_balanced,
        })
    }
}
