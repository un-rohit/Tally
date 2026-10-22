use accounting_app::*;
use chrono::NaiveDate;
use rusqlite::Connection;

fn setup_test_company() -> (Connection, Company, FinancialYear) {
    let mut conn = DbMigrator::open_in_memory().expect("Failed to initialize test DB");
    let (comp, fy) = Company::create(
        &mut conn,
        "40001",
        "Test BillWise Corp",
        "Test BillWise Corp",
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
fn test_billwise_full_and_partial_payment() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_debtors = AccountGroup::get_by_name(&conn, &comp.company_id, "Sundry Debtors").unwrap().unwrap();
    let grp_sales = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts").unwrap().unwrap();
    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts").unwrap().unwrap();

    let customer = Ledger::create(&conn, &comp.company_id, &grp_debtors.group_id, "Apex Traders", Paise::ZERO, None, false).unwrap();
    let sales = Ledger::create(&conn, &comp.company_id, &grp_sales.group_id, "Sales A/c", Paise::ZERO, None, false).unwrap();
    let bank = Ledger::create(&conn, &comp.company_id, &grp_bank.group_id, "HDFC Bank", Paise::ZERO, None, true).unwrap();

    let vt_sales: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let vt_receipt: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Receipt'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // 1. Post Sales Invoice #INV-1001 for ₹10,000 (10,00,000 paise) with NewRef
    let inv_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_sales.clone(),
        voucher_number: "INV-1001".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: Some("Sales to Apex Traders".into()),
        entries: vec![
            VoucherEntryDraft {
                ledger_id: customer.ledger_id.clone(),
                entry_type: EntryType::Dr,
                amount: Paise(1000000), // ₹10,000
                narration: None,
            },
            VoucherEntryDraft {
                ledger_id: sales.ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: Paise(1000000),
                narration: None,
            },
        ],
    };

    let post_inv = PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &inv_draft, None, |tx, v_id| {
        // Query entry_id for customer line
        let entry_id: String = tx.query_row(
            "SELECT entry_id FROM voucher_entries WHERE voucher_id = ?1 AND ledger_id = ?2",
            rusqlite::params![v_id, customer.ledger_id],
            |r| r.get(0),
        )?;

        let alloc = BillAllocationDraft {
            ledger_id: customer.ledger_id.clone(),
            ref_type: BillRefType::NewRef,
            ref_name: "INV-1001".to_string(),
            credit_days: 30,
            due_date: Some(NaiveDate::from_ymd_opt(2026, 5, 10).unwrap()),
            amount_paise: Paise(1000000),
            entry_type: EntryType::Dr,
        };
        BillWiseEngine::record_allocation(tx, &comp.company_id, &entry_id, &alloc)?;
        Ok(())
    }).unwrap();

    assert_eq!(post_inv.status, VoucherStatus::Posted);

    // Verify initial outstanding: ₹10,000
    let bills = BillWiseEngine::get_outstanding_bills_for_party(&conn, &comp.company_id, &customer.ledger_id, None).unwrap();
    assert_eq!(bills.len(), 1);
    assert_eq!(bills[0].bill_ref, "INV-1001");
    assert_eq!(bills[0].original_amount_paise, Paise(1000000));
    assert_eq!(bills[0].settled_amount_paise, Paise::ZERO);
    assert_eq!(bills[0].outstanding_amount_paise, Paise(1000000));

    // 2. Partial Payment: Receipt of ₹4,000 (4,00,000 paise) AgstRef
    let rcp1_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_receipt.clone(),
        voucher_number: "RCP-2001".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: Some("Partial payment 1".into()),
        entries: vec![
            VoucherEntryDraft {
                ledger_id: bank.ledger_id.clone(),
                entry_type: EntryType::Dr,
                amount: Paise(400000), // ₹4,000
                narration: None,
            },
            VoucherEntryDraft {
                ledger_id: customer.ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: Paise(400000),
                narration: None,
            },
        ],
    };

    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &rcp1_draft, None, |tx, v_id| {
        let entry_id: String = tx.query_row(
            "SELECT entry_id FROM voucher_entries WHERE voucher_id = ?1 AND ledger_id = ?2",
            rusqlite::params![v_id, customer.ledger_id],
            |r| r.get(0),
        )?;

        let alloc = BillAllocationDraft {
            ledger_id: customer.ledger_id.clone(),
            ref_type: BillRefType::AgstRef,
            ref_name: "INV-1001".to_string(),
            credit_days: 0,
            due_date: None,
            amount_paise: Paise(400000),
            entry_type: EntryType::Cr,
        };
        BillWiseEngine::record_allocation(tx, &comp.company_id, &entry_id, &alloc)?;
        Ok(())
    }).unwrap();

    // Check outstanding after partial payment: remaining ₹6,000
    let bills2 = BillWiseEngine::get_outstanding_bills_for_party(&conn, &comp.company_id, &customer.ledger_id, None).unwrap();
    assert_eq!(bills2.len(), 1);
    assert_eq!(bills2[0].settled_amount_paise, Paise(400000));
    assert_eq!(bills2[0].outstanding_amount_paise, Paise(600000));

    // 3. Final Payment: Receipt of remaining ₹6,000
    let rcp2_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_receipt.clone(),
        voucher_number: "RCP-2002".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 25).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: Some("Final settlement".into()),
        entries: vec![
            VoucherEntryDraft {
                ledger_id: bank.ledger_id.clone(),
                entry_type: EntryType::Dr,
                amount: Paise(600000),
                narration: None,
            },
            VoucherEntryDraft {
                ledger_id: customer.ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: Paise(600000),
                narration: None,
            },
        ],
    };

    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &rcp2_draft, None, |tx, v_id| {
        let entry_id: String = tx.query_row(
            "SELECT entry_id FROM voucher_entries WHERE voucher_id = ?1 AND ledger_id = ?2",
            rusqlite::params![v_id, customer.ledger_id],
            |r| r.get(0),
        )?;

        let alloc = BillAllocationDraft {
            ledger_id: customer.ledger_id.clone(),
            ref_type: BillRefType::AgstRef,
            ref_name: "INV-1001".to_string(),
            credit_days: 0,
            due_date: None,
            amount_paise: Paise(600000),
            entry_type: EntryType::Cr,
        };
        BillWiseEngine::record_allocation(tx, &comp.company_id, &entry_id, &alloc)?;
        Ok(())
    }).unwrap();

    // Outstanding should now be empty (fully cleared)
    let bills3 = BillWiseEngine::get_outstanding_bills_for_party(&conn, &comp.company_id, &customer.ledger_id, None).unwrap();
    assert_eq!(bills3.len(), 0);

    // 4. Verify history of the bill
    let history = BillWiseEngine::get_bill_allocation_history(&conn, &comp.company_id, &customer.ledger_id, "INV-1001").unwrap();
    assert_eq!(history.len(), 3); // 1 NewRef + 2 AgstRef
    assert_eq!(history[0].ref_type, "NewRef");
    assert_eq!(history[1].ref_type, "AgstRef");
    assert_eq!(history[2].ref_type, "AgstRef");
}

#[test]
fn test_over_allocation_rejection() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_creditors = AccountGroup::get_by_name(&conn, &comp.company_id, "Sundry Creditors").unwrap().unwrap();
    let grp_purchase = AccountGroup::get_by_name(&conn, &comp.company_id, "Purchase Accounts").unwrap().unwrap();
    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts").unwrap().unwrap();

    let supplier = Ledger::create(&conn, &comp.company_id, &grp_creditors.group_id, "Zenith Suppliers", Paise::ZERO, None, false).unwrap();
    let purchase = Ledger::create(&conn, &comp.company_id, &grp_purchase.group_id, "Purchase A/c", Paise::ZERO, None, false).unwrap();
    let bank = Ledger::create(&conn, &comp.company_id, &grp_bank.group_id, "SBI Current", Paise::ZERO, None, true).unwrap();

    let vt_purchase: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Purchase'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let vt_payment: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Payment'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // Purchase bill #PB-500 of ₹5,000
    let pur_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_purchase.clone(),
        voucher_number: "PB-500".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft {
                ledger_id: purchase.ledger_id.clone(),
                entry_type: EntryType::Dr,
                amount: Paise(500000), // ₹5,000
                narration: None,
            },
            VoucherEntryDraft {
                ledger_id: supplier.ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: Paise(500000),
                narration: None,
            },
        ],
    };

    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &pur_draft, None, |tx, v_id| {
        let entry_id: String = tx.query_row(
            "SELECT entry_id FROM voucher_entries WHERE voucher_id = ?1 AND ledger_id = ?2",
            rusqlite::params![v_id, supplier.ledger_id],
            |r| r.get(0),
        )?;

        let alloc = BillAllocationDraft {
            ledger_id: supplier.ledger_id.clone(),
            ref_type: BillRefType::NewRef,
            ref_name: "PB-500".to_string(),
            credit_days: 15,
            due_date: Some(NaiveDate::from_ymd_opt(2026, 4, 30).unwrap()),
            amount_paise: Paise(500000),
            entry_type: EntryType::Cr,
        };
        BillWiseEngine::record_allocation(tx, &comp.company_id, &entry_id, &alloc)?;
        Ok(())
    }).unwrap();

    // Attempt payment of ₹6,000 against ₹5,000 bill (must fail)
    let pmt_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_payment.clone(),
        voucher_number: "PMT-801".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 25).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft {
                ledger_id: supplier.ledger_id.clone(),
                entry_type: EntryType::Dr,
                amount: Paise(600000), // ₹6,000
                narration: None,
            },
            VoucherEntryDraft {
                ledger_id: bank.ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: Paise(600000),
                narration: None,
            },
        ],
    };

    let fail_res = PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &pmt_draft, None, |tx, v_id| {
        let entry_id: String = tx.query_row(
            "SELECT entry_id FROM voucher_entries WHERE voucher_id = ?1 AND ledger_id = ?2",
            rusqlite::params![v_id, supplier.ledger_id],
            |r| r.get(0),
        )?;

        let alloc = BillAllocationDraft {
            ledger_id: supplier.ledger_id.clone(),
            ref_type: BillRefType::AgstRef,
            ref_name: "PB-500".to_string(),
            credit_days: 0,
            due_date: None,
            amount_paise: Paise(600000),
            entry_type: EntryType::Dr,
        };
        BillWiseEngine::record_allocation(tx, &comp.company_id, &entry_id, &alloc)?;
        Ok(())
    });

    assert!(fail_res.is_err());
    let err_str = fail_res.err().unwrap().to_string();
    assert!(err_str.contains("exceeds available outstanding amount"));
}

#[test]
fn test_ageing_report_buckets() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_debtors = AccountGroup::get_by_name(&conn, &comp.company_id, "Sundry Debtors").unwrap().unwrap();
    let grp_sales = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts").unwrap().unwrap();

    let customer = Ledger::create(&conn, &comp.company_id, &grp_debtors.group_id, "Beta Corp", Paise::ZERO, None, false).unwrap();
    let sales = Ledger::create(&conn, &comp.company_id, &grp_sales.group_id, "Sales A/c", Paise::ZERO, None, false).unwrap();

    let vt_sales: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // Helper to post an invoice with specific due date
    let mut post_test_invoice = |v_num: &str, v_date: NaiveDate, due: NaiveDate, amt_paise: i64| {
        let draft = VoucherDraft {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_sales.clone(),
            voucher_number: v_num.to_string(),
            voucher_date: v_date,
            reference_no: None,
            reference_date: None,
            narration: None,
            entries: vec![
                VoucherEntryDraft { ledger_id: customer.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(amt_paise), narration: None },
                VoucherEntryDraft { ledger_id: sales.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(amt_paise), narration: None },
            ],
        };

        PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &draft, None, |tx, v_id| {
            let entry_id: String = tx.query_row(
                "SELECT entry_id FROM voucher_entries WHERE voucher_id = ?1 AND ledger_id = ?2",
                rusqlite::params![v_id, customer.ledger_id],
                |r| r.get(0),
            )?;

            let alloc = BillAllocationDraft {
                ledger_id: customer.ledger_id.clone(),
                ref_type: BillRefType::NewRef,
                ref_name: v_num.to_string(),
                credit_days: 0,
                due_date: Some(due),
                amount_paise: Paise(amt_paise),
                entry_type: EntryType::Dr,
            };
            BillWiseEngine::record_allocation(tx, &comp.company_id, &entry_id, &alloc)?;
            Ok(())
        }).unwrap();
    };

    // As of evaluation date: 2026-08-01
    // 1. Current (not overdue): due 2026-08-15 (₹1,000)
    post_test_invoice("INV-CURR", NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(), NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(), 100000);

    // 2. 1-30 days overdue: due 2026-07-15 (17 days overdue) (₹2,000)
    post_test_invoice("INV-30", NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(), NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(), 200000);

    // 3. 31-60 days overdue: due 2026-06-15 (47 days overdue) (₹3,000)
    post_test_invoice("INV-60", NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(), NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(), 300000);

    // 4. Over 90 days overdue: due 2026-04-15 (108 days overdue) (₹5,000)
    post_test_invoice("INV-90+", NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(), NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(), 500000);

    // Run Ageing Report as of 2026-08-01
    let as_of = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
    let aging = BillWiseEngine::generate_aging_report(&conn, &comp.company_id, true, Some(as_of)).unwrap();

    assert_eq!(aging.len(), 1);
    let row = &aging[0];
    assert_eq!(row.party_name, "Beta Corp");
    assert_eq!(row.current_paise, Paise(100000));     // ₹1,000
    assert_eq!(row.days_1_30_paise, Paise(200000));   // ₹2,000
    assert_eq!(row.days_31_60_paise, Paise(300000));  // ₹3,000
    assert_eq!(row.days_61_90_paise, Paise::ZERO);
    assert_eq!(row.over_90_paise, Paise(500000));     // ₹5,000
    assert_eq!(row.total_outstanding_paise, Paise(1100000)); // Total ₹11,000
}

#[test]
fn test_billwise_immutability_trigger() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_debtors = AccountGroup::get_by_name(&conn, &comp.company_id, "Sundry Debtors").unwrap().unwrap();
    let grp_sales = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts").unwrap().unwrap();

    let customer = Ledger::create(&conn, &comp.company_id, &grp_debtors.group_id, "Gamma Ltd", Paise::ZERO, None, false).unwrap();
    let sales = Ledger::create(&conn, &comp.company_id, &grp_sales.group_id, "Sales A/c", Paise::ZERO, None, false).unwrap();

    let vt_sales: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // Post standard sales voucher without allocation
    let draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_sales.clone(),
        voucher_number: "INV-IMMUTABLE".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: customer.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(50000), narration: None },
            VoucherEntryDraft { ledger_id: sales.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(50000), narration: None },
        ],
    };

    let posted_v = PostingEngine::post_voucher(&mut conn, &draft, None).unwrap();
    assert_eq!(posted_v.status, VoucherStatus::Posted);

    // Retrieve entry_id of posted voucher
    let entry_id: String = conn.query_row(
        "SELECT entry_id FROM voucher_entries WHERE voucher_id = ?1 AND ledger_id = ?2",
        rusqlite::params![posted_v.voucher_id, customer.ledger_id],
        |r| r.get(0),
    ).unwrap();

    // Direct attempt to INSERT into bill_allocations on a posted voucher MUST fail via SQLite trigger
    let trigger_res = conn.execute(
        "INSERT INTO bill_allocations (
            allocation_id, company_id, entry_id, ledger_id, ref_type, ref_name, 
            credit_days, due_date, amount_paise, entry_type
        ) VALUES ('alloc-hack', ?1, ?2, ?3, 'NewRef', 'HACK-01', 0, NULL, 50000, 'Dr')",
        rusqlite::params![comp.company_id, entry_id, customer.ledger_id],
    );

    assert!(trigger_res.is_err(), "Trigger must abort inserting allocation on posted voucher");
    let err_msg = trigger_res.err().unwrap().to_string();
    assert!(err_msg.contains("ERR_POSTED_VOUCHER_IMMUTABLE"));
}

#[test]
fn test_advance_and_on_account_allocation() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_debtors = AccountGroup::get_by_name(&conn, &comp.company_id, "Sundry Debtors").unwrap().unwrap();
    let grp_bank = AccountGroup::get_by_name(&conn, &comp.company_id, "Bank Accounts").unwrap().unwrap();
    let grp_sales = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts").unwrap().unwrap();

    let customer = Ledger::create(&conn, &comp.company_id, &grp_debtors.group_id, "Delta Corp", Paise::ZERO, None, false).unwrap();
    let bank = Ledger::create(&conn, &comp.company_id, &grp_bank.group_id, "Kotak Bank", Paise::ZERO, None, true).unwrap();
    let sales = Ledger::create(&conn, &comp.company_id, &grp_sales.group_id, "Sales A/c", Paise::ZERO, None, false).unwrap();

    let vt_receipt: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Receipt'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let vt_sales: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    // 1. Advance receipt of ₹5,000 before invoice exists
    let adv_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_receipt.clone(),
        voucher_number: "ADV-001".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: Some("Advance received".into()),
        entries: vec![
            VoucherEntryDraft { ledger_id: bank.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(500000), narration: None },
            VoucherEntryDraft { ledger_id: customer.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(500000), narration: None },
        ],
    };

    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &adv_draft, None, |tx, v_id| {
        let entry_id: String = tx.query_row(
            "SELECT entry_id FROM voucher_entries WHERE voucher_id = ?1 AND ledger_id = ?2",
            rusqlite::params![v_id, customer.ledger_id],
            |r| r.get(0),
        )?;

        let alloc = BillAllocationDraft {
            ledger_id: customer.ledger_id.clone(),
            ref_type: BillRefType::Advance,
            ref_name: "ADV-001".to_string(),
            credit_days: 0,
            due_date: None,
            amount_paise: Paise(500000),
            entry_type: EntryType::Cr,
        };
        BillWiseEngine::record_allocation(tx, &comp.company_id, &entry_id, &alloc)?;
        Ok(())
    }).unwrap();

    // Verify Advance shows in outstanding balance
    let adv_bills = BillWiseEngine::get_outstanding_bills_for_party(&conn, &comp.company_id, &customer.ledger_id, None).unwrap();
    assert_eq!(adv_bills.len(), 1);
    assert_eq!(adv_bills[0].bill_ref, "ADV-001");
    assert_eq!(adv_bills[0].outstanding_amount_paise, Paise(500000));

    // 2. Invoice for ₹5,000 adjusted against Advance ADV-001
    let inv_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_sales.clone(),
        voucher_number: "INV-DELTA".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: Some("Invoice settled against advance".into()),
        entries: vec![
            VoucherEntryDraft { ledger_id: customer.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(500000), narration: None },
            VoucherEntryDraft { ledger_id: sales.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(500000), narration: None },
        ],
    };

    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &inv_draft, None, |tx, v_id| {
        let entry_id: String = tx.query_row(
            "SELECT entry_id FROM voucher_entries WHERE voucher_id = ?1 AND ledger_id = ?2",
            rusqlite::params![v_id, customer.ledger_id],
            |r| r.get(0),
        )?;

        let alloc = BillAllocationDraft {
            ledger_id: customer.ledger_id.clone(),
            ref_type: BillRefType::AgstRef,
            ref_name: "ADV-001".to_string(),
            credit_days: 0,
            due_date: None,
            amount_paise: Paise(500000),
            entry_type: EntryType::Dr,
        };
        BillWiseEngine::record_allocation(tx, &comp.company_id, &entry_id, &alloc)?;
        Ok(())
    }).unwrap();

    // Advance should now be completely settled
    let final_bills = BillWiseEngine::get_outstanding_bills_for_party(&conn, &comp.company_id, &customer.ledger_id, None).unwrap();
    assert_eq!(final_bills.len(), 0);
}

#[test]
fn test_rollback_on_failed_allocation() {
    let (mut conn, comp, fy) = setup_test_company();

    let grp_debtors = AccountGroup::get_by_name(&conn, &comp.company_id, "Sundry Debtors").unwrap().unwrap();
    let grp_sales = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts").unwrap().unwrap();

    let customer = Ledger::create(&conn, &comp.company_id, &grp_debtors.group_id, "Echo Traders", Paise::ZERO, None, false).unwrap();
    let sales = Ledger::create(&conn, &comp.company_id, &grp_sales.group_id, "Sales A/c", Paise::ZERO, None, false).unwrap();

    let vt_sales: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_sales.clone(),
        voucher_number: "INV-FAIL".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: customer.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(50000), narration: None },
            VoucherEntryDraft { ledger_id: sales.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(50000), narration: None },
        ],
    };

    // Pre-post hook fails deliberately on allocation
    let fail_res = PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &draft, None, |tx, _v_id| {
        // Deliberate invalid allocation: empty ref_name
        let alloc = BillAllocationDraft {
            ledger_id: customer.ledger_id.clone(),
            ref_type: BillRefType::NewRef,
            ref_name: "".to_string(), // Invalid!
            credit_days: 0,
            due_date: None,
            amount_paise: Paise(50000),
            entry_type: EntryType::Dr,
        };
        BillWiseEngine::record_allocation(tx, &comp.company_id, "dummy-entry", &alloc)?;
        Ok(())
    });

    assert!(fail_res.is_err());

    // Verify complete rollback: no voucher, no entries, no allocations exist
    let v_count: i64 = conn.query_row("SELECT COUNT(*) FROM vouchers WHERE voucher_number = 'INV-FAIL'", [], |r| r.get(0)).unwrap();
    let alloc_count: i64 = conn.query_row("SELECT COUNT(*) FROM bill_allocations WHERE ledger_id = ?1", [&customer.ledger_id], |r| r.get(0)).unwrap();

    assert_eq!(v_count, 0);
    assert_eq!(alloc_count, 0);
}

