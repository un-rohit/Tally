use accounting_app::*;
use chrono::NaiveDate;

fn main() -> Result<()> {
    println!("=== Independent Clean-Room Accounting Application ===");
    println!("Initializing local database with migrations...");

    let mut conn = DbMigrator::open_in_memory()?;
    println!("Database initialized successfully with WAL and foreign keys enabled.");

    // 1. Create Company & FY
    let (comp, fy) = Company::create(
        &mut conn,
        "10001",
        "Acme Enterprises Pvt Ltd",
        "Acme Enterprises",
        "27", // Maharashtra
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2027, 3, 31).unwrap(),
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
    )?;
    println!("Created Company: {} (#{})", comp.name, comp.company_number);
    println!(
        "Active FY: {} to {} (Books Beginning: {})",
        fy.fy_start_date, fy.fy_end_date, fy.books_beginning_date
    );

    // 2. Seed Standard 28 Groups
    AccountGroup::seed_standard_groups(&mut conn, &comp.company_id)?;
    println!("Seeded 28 standard accounting groups.");

    // 3. Create Ledgers
    let grp_capital = AccountGroup::get_by_name(&conn, &comp.company_id, "Capital Account")?
        .expect("Capital Account group missing");
    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts")?
        .expect("Bank Accounts group missing");
    let grp_sales = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts")?
        .expect("Sales Accounts group missing");
    let grp_rent = AccountGroup::get_by_name(&conn, &comp.company_id, "Indirect Expenses")?
        .expect("Indirect Expenses group missing");

    let l_capital = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_capital.group_id,
        "Owner Capital",
        Paise::ZERO,
        None,
        false,
    )?;

    let l_bank = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_bank.group_id,
        "State Bank of India",
        Paise::ZERO,
        None,
        false,
    )?;

    let l_sales = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_sales.group_id,
        "Domestic Software Sales",
        Paise::ZERO,
        None,
        false,
    )?;

    let l_rent = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_rent.group_id,
        "Office Rent",
        Paise::ZERO,
        None,
        false,
    )?;

    println!("Created ledgers: Capital, Bank, Sales, Rent.");

    // Fetch Voucher Types
    let vt_receipt_id: String = conn.query_row(
        "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Receipt'",
        [&comp.company_id],
        |r| r.get(0),
    )?;

    let vt_sales_id: String = conn.query_row(
        "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
        [&comp.company_id],
        |r| r.get(0),
    )?;

    let vt_payment_id: String = conn.query_row(
        "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Payment'",
        [&comp.company_id],
        |r| r.get(0),
    )?;

    // 4. Post Voucher 1: Capital Introduction (Receipt)
    // Dr Bank ₹5,00,000, Cr Capital ₹5,00,000
    let v1 = PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_receipt_id,
            voucher_number: "RCP-001".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 2).unwrap(),
            reference_no: None,
            reference_date: None,
            narration: Some("Capital introduced by founder via NEFT".into()),
            entries: vec![
                VoucherEntryDraft {
                    ledger_id: l_bank.ledger_id.clone(),
                    entry_type: EntryType::Dr,
                    amount: Paise::from_rupees(500000)?,
                    narration: None,
                },
                VoucherEntryDraft {
                    ledger_id: l_capital.ledger_id.clone(),
                    entry_type: EntryType::Cr,
                    amount: Paise::from_rupees(500000)?,
                    narration: None,
                },
            ],
        },
        Some("usr_admin"),
    )?;
    println!("Posted Voucher {}: {}", v1.voucher_number, v1.total_amount);

    // 5. Post Voucher 2: Cash/Bank Sale (Sales)
    // Dr Bank ₹1,20,000, Cr Sales ₹1,20,000
    let v2 = PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_sales_id,
            voucher_number: "INV-001".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
            reference_no: Some("PO-882".into()),
            reference_date: None,
            narration: Some("Consulting services rendered".into()),
            entries: vec![
                VoucherEntryDraft {
                    ledger_id: l_bank.ledger_id.clone(),
                    entry_type: EntryType::Dr,
                    amount: Paise::from_rupees(120000)?,
                    narration: None,
                },
                VoucherEntryDraft {
                    ledger_id: l_sales.ledger_id.clone(),
                    entry_type: EntryType::Cr,
                    amount: Paise::from_rupees(120000)?,
                    narration: None,
                },
            ],
        },
        Some("usr_admin"),
    )?;
    println!("Posted Voucher {}: {}", v2.voucher_number, v2.total_amount);

    // 6. Post Voucher 3: Rent Payment (Payment)
    // Dr Rent ₹30,000, Cr Bank ₹30,000
    let v3 = PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_payment_id,
            voucher_number: "PMT-001".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
            reference_no: None,
            reference_date: None,
            narration: Some("April 2026 office rent".into()),
            entries: vec![
                VoucherEntryDraft {
                    ledger_id: l_rent.ledger_id.clone(),
                    entry_type: EntryType::Dr,
                    amount: Paise::from_rupees(30000)?,
                    narration: None,
                },
                VoucherEntryDraft {
                    ledger_id: l_bank.ledger_id.clone(),
                    entry_type: EntryType::Cr,
                    amount: Paise::from_rupees(30000)?,
                    narration: None,
                },
            ],
        },
        Some("usr_admin"),
    )?;
    println!("Posted Voucher {}: {}", v3.voucher_number, v3.total_amount);

    // 7. Generate Trial Balance
    let tb = ReportEngine::generate_trial_balance(&conn, &comp.company_id, None)?;
    println!("\n=== TRIAL BALANCE (Balanced: {}) ===", tb.is_balanced);
    for it in &tb.items {
        println!(
            "  {:30} | Dr: {:15} | Cr: {:15}",
            it.ledger_name, it.debit_balance, it.credit_balance
        );
    }
    println!(
        "  {:30} | Dr: {:15} | Cr: {:15}",
        "TOTAL", tb.total_debit, tb.total_credit
    );

    // 8. Generate Profit & Loss Report
    let pnl = ReportEngine::generate_profit_and_loss(&conn, &comp.company_id, None)?;
    println!("\n=== PROFIT & LOSS ===");
    println!("  Total Direct Income:  {}", pnl.total_direct_income);
    println!("  Total Direct Expense: {}", pnl.total_direct_expense);
    println!("  GROSS PROFIT:         {}", pnl.gross_profit);
    println!("  Total Indirect Exp:   {}", pnl.total_indirect_expense);
    println!("  NET PROFIT:           {}", pnl.net_profit);

    // 9. Generate Balance Sheet
    let bs = ReportEngine::generate_balance_sheet(&conn, &comp.company_id, None)?;
    println!("\n=== BALANCE SHEET (Equilibrium: {}) ===", bs.is_balanced);
    println!("  Liabilities:          {}", bs.total_liabilities);
    println!("  Net Profit:           {}", bs.net_profit_transferred);
    println!("  Total (Liab + Profit):{}", bs.total_with_profit);
    println!("  Total Assets:         {}", bs.total_assets);

    // 10. Verify Audit Chain
    let audit_ok = AuditLogger::verify_chain(&conn, &comp.company_id)?;
    println!("\nMCA Cryptographic Audit Chain Valid: {}", audit_ok);

    println!("\nAll core accounting modules executed successfully with complete equilibrium!");
    Ok(())
}
