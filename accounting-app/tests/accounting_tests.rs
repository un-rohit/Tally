use accounting_app::*;
use chrono::NaiveDate;
use rusqlite::Connection;

fn setup_test_company() -> (Connection, Company, FinancialYear) {
    let mut conn = DbMigrator::open_in_memory().expect("Failed to initialize test DB");
    let (comp, fy) = Company::create(
        &mut conn,
        "10001",
        "Test Accounting Corp",
        "Test Accounting Corp",
        "27",
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2027, 3, 31).unwrap(),
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
    )
    .expect("Failed to create company");

    AccountGroup::seed_standard_groups(&mut conn, &comp.company_id)
        .expect("Failed to seed standard groups");

    (conn, comp, fy)
}

#[test]
fn test_company_and_financial_year_creation() {
    let mut conn = DbMigrator::open_in_memory().unwrap();

    // Valid creation
    let (comp, fy) = Company::create(
        &mut conn,
        "20001",
        "Global Logistics Ltd",
        "Global Logistics",
        "29", // Karnataka
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2027, 3, 31).unwrap(),
        NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
    )
    .unwrap();

    assert_eq!(comp.company_number, "20001");
    assert_eq!(fy.books_beginning_date, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());

    // Invalid: books_beginning prior to fy_start must fail
    let err = Company::create(
        &mut conn,
        "20002",
        "Invalid Dates Ltd",
        "Invalid Dates",
        "29",
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2027, 3, 31).unwrap(),
        NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(), // Invalid
    );
    assert!(err.is_err(), "Books beginning prior to FY start must be rejected");
}

#[test]
fn test_seed_standard_28_groups() {
    let (conn, comp, _) = setup_test_company();

    // Count groups
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM account_groups WHERE company_id = ?1",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // 14 Primary + 13 Secondary = 27 (or 28 with Misc)
    assert!(count >= 27, "Expected at least 27 standard groups, found {}", count);

    // Verify parent relationships
    let bank_grp = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts")
        .unwrap()
        .expect("Bank Accounts group missing");
    assert_eq!(bank_grp.nature, AccountNature::Asset);
    assert!(bank_grp.parent_group_id.is_some());

    let curr_assets = AccountGroup::get_by_name(&conn, &comp.company_id, "Current Assets")
        .unwrap()
        .expect("Current Assets group missing");
    assert_eq!(bank_grp.parent_group_id.as_deref(), Some(curr_assets.group_id.as_str()));
}

#[test]
fn test_voucher_posting_balanced() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts")
        .unwrap()
        .unwrap();
    let grp_capital = AccountGroup::get_by_name(&conn, &comp.company_id, "Capital Account")
        .unwrap()
        .unwrap();

    let l_bank = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_bank.group_id,
        "ICICI Current A/c",
        Paise::ZERO,
        None,
        false,
    )
    .unwrap();

    let l_cap = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_capital.group_id,
        "Founder Equity",
        Paise::ZERO,
        None,
        false,
    )
    .unwrap();

    let vt_receipt: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Receipt'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // Balanced voucher: Dr Bank ₹10,00,000, Cr Capital ₹10,00,000
    let v_posted = PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_receipt,
            voucher_number: "RCP-101".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 5).unwrap(),
            reference_no: None,
            reference_date: None,
            narration: Some("Initial seed capital".into()),
            entries: vec![
                VoucherEntryDraft {
                    ledger_id: l_bank.ledger_id.clone(),
                    entry_type: EntryType::Dr,
                    amount: Paise::from_rupees(1000000).unwrap(),
                    narration: None,
                },
                VoucherEntryDraft {
                    ledger_id: l_cap.ledger_id.clone(),
                    entry_type: EntryType::Cr,
                    amount: Paise::from_rupees(1000000).unwrap(),
                    narration: None,
                },
            ],
        },
        Some("admin"),
    )
    .expect("Balanced voucher should post successfully");

    assert_eq!(v_posted.status, VoucherStatus::Posted);

    // Verify closing balances
    let bal_bank = Ledger::get_closing_balance(&conn, &l_bank.ledger_id).unwrap();
    let bal_cap = Ledger::get_closing_balance(&conn, &l_cap.ledger_id).unwrap();

    assert_eq!(bal_bank, Paise::from_rupees(1000000).unwrap());
    assert_eq!(bal_cap, Paise::from_rupees(1000000).unwrap());
}

#[test]
fn test_unbalanced_voucher_rolls_back_completely() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts")
        .unwrap()
        .unwrap();
    let l_bank = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_bank.group_id,
        "Bank A/c",
        Paise::ZERO,
        None,
        false,
    )
    .unwrap();

    let vt_receipt: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Receipt'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // Unbalanced: Dr ₹5,000 vs Cr ₹4,000
    let res = PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_receipt,
            voucher_number: "BAD-001".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            reference_no: None,
            reference_date: None,
            narration: None,
            entries: vec![
                VoucherEntryDraft {
                    ledger_id: l_bank.ledger_id.clone(),
                    entry_type: EntryType::Dr,
                    amount: Paise::from_rupees(5000).unwrap(),
                    narration: None,
                },
                VoucherEntryDraft {
                    ledger_id: l_bank.ledger_id.clone(),
                    entry_type: EntryType::Cr,
                    amount: Paise::from_rupees(4000).unwrap(),
                    narration: None,
                },
            ],
        },
        Some("admin"),
    );

    assert!(res.is_err(), "Unbalanced voucher must be rejected");

    // Verify atomic rollback: no voucher or entries inserted
    let voucher_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vouchers WHERE voucher_number = 'BAD-001'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(voucher_count, 0, "No voucher record should exist after rollback");

    let entry_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM voucher_entries", [], |r| r.get(0))
        .unwrap();
    assert_eq!(entry_count, 0, "No voucher entries should exist after rollback");
}

#[test]
fn test_duplicate_voucher_number_rejected() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts")
        .unwrap()
        .unwrap();
    let l_bank = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_bank.group_id,
        "Bank A/c",
        Paise::ZERO,
        None,
        false,
    )
    .unwrap();

    let vt_receipt: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Receipt'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_receipt.clone(),
        voucher_number: "VCH-001".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft {
                ledger_id: l_bank.ledger_id.clone(),
                entry_type: EntryType::Dr,
                amount: Paise::from_rupees(1000).unwrap(),
                narration: None,
            },
            VoucherEntryDraft {
                ledger_id: l_bank.ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: Paise::from_rupees(1000).unwrap(),
                narration: None,
            },
        ],
    };

    // First post succeeds
    assert!(PostingEngine::post_voucher(&mut conn, &draft, None).is_ok());

    // Duplicate post with exact same voucher number in same FY must fail
    let duplicate_res = PostingEngine::post_voucher(&mut conn, &draft, None);
    assert!(duplicate_res.is_err(), "Duplicate voucher number must be rejected");
}

#[test]
fn test_voucher_date_boundaries() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts")
        .unwrap()
        .unwrap();
    let l_bank = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_bank.group_id,
        "Bank A/c",
        Paise::ZERO,
        None,
        false,
    )
    .unwrap();

    let vt_receipt: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Receipt'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // Date before FY: 2026-03-31 (FY starts 2026-04-01)
    let early_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_receipt.clone(),
        voucher_number: "EARLY-1".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft {
                ledger_id: l_bank.ledger_id.clone(),
                entry_type: EntryType::Dr,
                amount: Paise::from_rupees(100).unwrap(),
                narration: None,
            },
            VoucherEntryDraft {
                ledger_id: l_bank.ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: Paise::from_rupees(100).unwrap(),
                narration: None,
            },
        ],
    };
    assert!(
        PostingEngine::post_voucher(&mut conn, &early_draft, None).is_err(),
        "Date before books beginning must fail"
    );

    // Date after FY: 2027-04-01 (FY ends 2027-03-31)
    let late_draft = VoucherDraft {
        voucher_number: "LATE-1".into(),
        voucher_date: NaiveDate::from_ymd_opt(2027, 4, 1).unwrap(),
        ..early_draft
    };
    assert!(
        PostingEngine::post_voucher(&mut conn, &late_draft, None).is_err(),
        "Date after FY end must fail"
    );
}

#[test]
fn test_posted_voucher_immutability_and_triggers() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts")
        .unwrap()
        .unwrap();
    let l_bank = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_bank.group_id,
        "Bank A/c",
        Paise::ZERO,
        None,
        false,
    )
    .unwrap();

    let vt_receipt: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Receipt'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let v = PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_receipt,
            voucher_number: "IMMUT-001".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            reference_no: None,
            reference_date: None,
            narration: None,
            entries: vec![
                VoucherEntryDraft {
                    ledger_id: l_bank.ledger_id.clone(),
                    entry_type: EntryType::Dr,
                    amount: Paise::from_rupees(2000).unwrap(),
                    narration: None,
                },
                VoucherEntryDraft {
                    ledger_id: l_bank.ledger_id.clone(),
                    entry_type: EntryType::Cr,
                    amount: Paise::from_rupees(2000).unwrap(),
                    narration: None,
                },
            ],
        },
        None,
    )
    .unwrap();

    // 1. Attempt to alter amount on posted voucher -> Trigger must block
    let update_amt = conn.execute(
        "UPDATE vouchers SET total_amount_paise = 999999 WHERE voucher_id = ?1",
        [&v.voucher_id],
    );
    assert!(update_amt.is_err(), "Trigger must block altering total amount of posted voucher");

    // 2. Attempt to alter date on posted voucher -> Trigger must block
    let update_date = conn.execute(
        "UPDATE vouchers SET voucher_date = '2026-05-01' WHERE voucher_id = ?1",
        [&v.voucher_id],
    );
    assert!(update_date.is_err(), "Trigger must block altering date of posted voucher");

    // 3. Attempt to insert line into posted voucher -> Trigger must block
    let insert_line = conn.execute(
        "INSERT INTO voucher_entries VALUES ('fake-id', ?1, 3, ?2, 'Dr', 1000, NULL)",
        [&v.voucher_id, &l_bank.ledger_id],
    );
    assert!(insert_line.is_err(), "Trigger must block adding lines to posted voucher");

    // 4. Attempt to update existing line of posted voucher -> Trigger must block
    let update_line = conn.execute(
        "UPDATE voucher_entries SET amount_paise = 88888 WHERE voucher_id = ?1",
        [&v.voucher_id],
    );
    assert!(update_line.is_err(), "Trigger must block modifying line of posted voucher");

    // 5. Attempt to delete line of posted voucher -> Trigger must block
    let delete_line = conn.execute(
        "DELETE FROM voucher_entries WHERE voucher_id = ?1",
        [&v.voucher_id],
    );
    assert!(delete_line.is_err(), "Trigger must block deleting line of posted voucher");
}

#[test]
fn test_voucher_cancellation_and_reversal() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts")
        .unwrap()
        .unwrap();
    let grp_sales = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts")
        .unwrap()
        .unwrap();

    let l_bank = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_bank.group_id,
        "Bank A/c",
        Paise::ZERO,
        None,
        false,
    )
    .unwrap();

    let l_sales = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_sales.group_id,
        "Sales A/c",
        Paise::ZERO,
        None,
        false,
    )
    .unwrap();

    let vt_sales: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // Post Sales voucher of ₹50,000
    let v_sales = PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_sales,
            voucher_number: "SALE-501".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
            reference_no: None,
            reference_date: None,
            narration: Some("Software Sale".into()),
            entries: vec![
                VoucherEntryDraft {
                    ledger_id: l_bank.ledger_id.clone(),
                    entry_type: EntryType::Dr,
                    amount: Paise::from_rupees(50000).unwrap(),
                    narration: None,
                },
                VoucherEntryDraft {
                    ledger_id: l_sales.ledger_id.clone(),
                    entry_type: EntryType::Cr,
                    amount: Paise::from_rupees(50000).unwrap(),
                    narration: None,
                },
            ],
        },
        Some("accountant"),
    )
    .unwrap();

    // Verify balance before reversal: Bank has +50,000, Sales has +50,000
    assert_eq!(
        Ledger::get_closing_balance(&conn, &l_bank.ledger_id).unwrap(),
        Paise::from_rupees(50000).unwrap()
    );

    // Reversal: Reverse with a compensating voucher
    let v_rev = PostingEngine::reverse_voucher(
        &mut conn,
        &v_sales.voucher_id,
        "REV-SALE-501",
        NaiveDate::from_ymd_opt(2026, 4, 16).unwrap(),
        Some("accountant"),
        "Client cancelled contract",
    )
    .unwrap();

    assert_eq!(v_rev.voucher_number, "REV-SALE-501");

    // Both vouchers exist in audit trail and database, but net ledger balance is now ₹0!
    assert_eq!(
        Ledger::get_closing_balance(&conn, &l_bank.ledger_id).unwrap(),
        Paise::ZERO
    );
    assert_eq!(
        Ledger::get_closing_balance(&conn, &l_sales.ledger_id).unwrap(),
        Paise::ZERO
    );

    // Also test Cancellation on a voucher
    PostingEngine::cancel_voucher(&mut conn, &v_sales.voucher_id, Some("admin"), "Audit Void").unwrap();
    let status_str: String = conn
        .query_row(
            "SELECT status FROM vouchers WHERE voucher_id = ?1",
            [&v_sales.voucher_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status_str, "Cancelled");
}

#[test]
fn test_trial_balance_profit_and_loss_and_balance_sheet_equilibrium() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_cap = AccountGroup::get_by_name(&conn, &comp.company_id, "Capital Account").unwrap().unwrap();
    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts").unwrap().unwrap();
    let grp_sales = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts").unwrap().unwrap();
    let grp_exp = AccountGroup::get_by_name(&conn, &comp.company_id, "Indirect Expenses").unwrap().unwrap();

    let l_cap = Ledger::create(&conn, &comp.company_id, &grp_cap.group_id, "Capital Equity", Paise::ZERO, None, false).unwrap();
    let l_bank = Ledger::create(&conn, &comp.company_id, &grp_bank.group_id, "HDFC Current", Paise::ZERO, None, false).unwrap();
    let l_sales = Ledger::create(&conn, &comp.company_id, &grp_sales.group_id, "Consulting Revenue", Paise::ZERO, None, false).unwrap();
    let l_rent = Ledger::create(&conn, &comp.company_id, &grp_exp.group_id, "Commercial Rent", Paise::ZERO, None, false).unwrap();

    let vt_receipt: String = conn.query_row("SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Receipt'", [&comp.company_id], |r| r.get(0)).unwrap();
    let vt_sales: String = conn.query_row("SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'", [&comp.company_id], |r| r.get(0)).unwrap();
    let vt_payment: String = conn.query_row("SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Payment'", [&comp.company_id], |r| r.get(0)).unwrap();

    // 1. Introduce Capital: ₹10,00,000 (Dr Bank, Cr Capital)
    PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_receipt,
            voucher_number: "RCP-1".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
            reference_no: None,
            reference_date: None,
            narration: None,
            entries: vec![
                VoucherEntryDraft { ledger_id: l_bank.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise::from_rupees(1000000).unwrap(), narration: None },
                VoucherEntryDraft { ledger_id: l_cap.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise::from_rupees(1000000).unwrap(), narration: None },
            ],
        },
        None,
    ).unwrap();

    // 2. Sales: ₹2,50,000 (Dr Bank, Cr Sales)
    PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_sales,
            voucher_number: "SAL-1".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
            reference_no: None,
            reference_date: None,
            narration: None,
            entries: vec![
                VoucherEntryDraft { ledger_id: l_bank.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise::from_rupees(250000).unwrap(), narration: None },
                VoucherEntryDraft { ledger_id: l_sales.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise::from_rupees(250000).unwrap(), narration: None },
            ],
        },
        None,
    ).unwrap();

    // 3. Rent Payment: ₹50,000 (Dr Rent, Cr Bank)
    PostingEngine::post_voucher(
        &mut conn,
        &VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_payment,
            voucher_number: "PMT-1".into(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
            reference_no: None,
            reference_date: None,
            narration: None,
            entries: vec![
                VoucherEntryDraft { ledger_id: l_rent.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise::from_rupees(50000).unwrap(), narration: None },
                VoucherEntryDraft { ledger_id: l_bank.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise::from_rupees(50000).unwrap(), narration: None },
            ],
        },
        None,
    ).unwrap();

    // --- TEST TRIAL BALANCE ---
    let tb = ReportEngine::generate_trial_balance(&conn, &comp.company_id, None).unwrap();
    assert!(tb.is_balanced, "Trial Balance must be in exact mathematical equilibrium");
    // Total Debits: Bank net 12,00,000 + Rent 50,000 = 12,50,000
    // Total Credits: Capital 10,00,000 + Sales 2,50,000 = 12,50,000
    assert_eq!(tb.total_debit, Paise::from_rupees(1250000).unwrap());
    assert_eq!(tb.total_credit, Paise::from_rupees(1250000).unwrap());

    // --- TEST PROFIT & LOSS ---
    let pnl = ReportEngine::generate_profit_and_loss(&conn, &comp.company_id, None).unwrap();
    assert_eq!(pnl.total_direct_income, Paise::from_rupees(250000).unwrap()); // Sales
    assert_eq!(pnl.gross_profit, Paise::from_rupees(250000).unwrap());
    assert_eq!(pnl.total_indirect_expense, Paise::from_rupees(50000).unwrap()); // Rent
    assert_eq!(pnl.net_profit, Paise::from_rupees(200000).unwrap()); // Net Profit = ₹2,00,000

    // --- TEST BALANCE SHEET EQUILIBRIUM ---
    let bs = ReportEngine::generate_balance_sheet(&conn, &comp.company_id, None).unwrap();
    assert!(bs.is_balanced, "Balance Sheet must be in exact equilibrium (Liabilities + Net Profit == Assets)");
    // Liabilities: Capital ₹10,00,000
    assert_eq!(bs.total_liabilities, Paise::from_rupees(1000000).unwrap());
    // Net Profit: ₹2,00,000
    assert_eq!(bs.net_profit_transferred, Paise::from_rupees(200000).unwrap());
    // Total Liabilities with Net Profit: ₹12,00,000
    assert_eq!(bs.total_with_profit, Paise::from_rupees(1200000).unwrap());
    // Total Assets: Bank balance (10,00,000 + 2,50,000 - 50,000 = ₹12,00,000)
    assert_eq!(bs.total_assets, Paise::from_rupees(1200000).unwrap());
}

#[test]
fn test_mca_audit_trail_tamper_resistance() {
    let (conn, comp, _) = setup_test_company();

    // Verify chain is valid initially
    assert!(AuditLogger::verify_chain(&conn, &comp.company_id).unwrap());

    // Attempt direct delete -> Trigger must abort
    let del_res = conn.execute("DELETE FROM audit_trail WHERE company_id = ?1", [&comp.company_id]);
    assert!(del_res.is_err(), "Trigger must prevent audit trail record deletion");

    // Attempt direct update -> Trigger must abort
    let upd_res = conn.execute("UPDATE audit_trail SET action = 'HACKED' WHERE company_id = ?1", [&comp.company_id]);
    assert!(upd_res.is_err(), "Trigger must prevent audit trail record modification");
}
