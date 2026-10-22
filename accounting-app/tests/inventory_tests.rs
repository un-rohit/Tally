use accounting_app::*;
use chrono::NaiveDate;
use rusqlite::Connection;

fn setup_test_company() -> (Connection, Company, FinancialYear) {
    let mut conn = DbMigrator::open_in_memory().expect("Failed to initialize test DB");
    let (comp, fy) = Company::create(
        &mut conn,
        "30001",
        "Test Inventory Corp",
        "Test Inventory Corp",
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
fn test_uom_creation_and_compound_conversion() {
    let (conn, comp, _) = setup_test_company();

    // 1. Create simple UOM: Pcs
    let pcs = Uom::create_simple(&conn, &comp.company_id, "Pcs", Some("Pieces"), 0).unwrap();
    assert_eq!(pcs.symbol, "Pcs");
    assert!(!pcs.is_compound);

    // 2. Create compound UOM: Box of 10 Pcs
    let box_uom =
        Uom::create_compound(&conn, &comp.company_id, "Box", Some("Box of 10"), &pcs.uom_id, 10).unwrap();
    assert!(box_uom.is_compound);
    assert_eq!(box_uom.conversion_factor, 10);

    // 3. Test conversions
    // 5 Boxes (5000 milli) to Pcs = 50,000 milli (50 Pcs)
    let converted_pcs =
        InventoryEngine::convert_quantity(&conn, &comp.company_id, &box_uom.uom_id, &pcs.uom_id, 5000)
            .unwrap();
    assert_eq!(converted_pcs, 50_000);

    // 50 Pcs (50,000 milli) to Boxes = 5,000 milli (5 Boxes)
    let converted_box =
        InventoryEngine::convert_quantity(&conn, &comp.company_id, &pcs.uom_id, &box_uom.uom_id, 50_000)
            .unwrap();
    assert_eq!(converted_box, 5000);
}

#[test]
fn test_stock_receipt_issue_and_negative_stock_prevention() {
    let (mut conn, comp, fy) = setup_test_company();
    let pcs = Uom::create_simple(&conn, &comp.company_id, "Pcs", Some("Pieces"), 0).unwrap();
    let wh = Warehouse::create(&conn, &comp.company_id, "Central Godown", Some("Bhiwandi")).unwrap();

    let grp_cash = AccountGroup::get_by_name(&conn, &comp.company_id, "Cash-in-hand").unwrap().unwrap();
    let grp_pur = AccountGroup::get_by_name(&conn, &comp.company_id, "Purchase Accounts").unwrap().unwrap();
    let grp_sal = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts").unwrap().unwrap();

    let cash = Ledger::create(&conn, &comp.company_id, &grp_cash.group_id, "Cash", Paise::ZERO, None, true).unwrap();
    let purchase = Ledger::create(&conn, &comp.company_id, &grp_pur.group_id, "Purchase A/c", Paise::ZERO, None, false).unwrap();
    let sales = Ledger::create(&conn, &comp.company_id, &grp_sal.group_id, "Sales A/c", Paise::ZERO, None, false).unwrap();

    let vt_sales: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let vt_purchase: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Purchase'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let item = StockItem::create(
        &conn,
        &CreateStockItemRequest {
            company_id: comp.company_id.clone(),
            stock_group_id: None,
            category_id: None,
            uom_id: pcs.uom_id.clone(),
            name: "Steel Widget A".to_string(),
            sku: Some("WGT-A".into()),
            description: None,
            hsn_sac_code: Some("7326".into()),
            gst_rate_basis_points: 1800,
            valuation_method: "FIFO".to_string(),
            opening_qty_milli: 0,
            opening_rate_paise: Paise::ZERO,
            reorder_level_milli: 10_000,
        },
    )
    .unwrap();

    // 1. Attempt issue when available stock is 0 (should fail due to negative stock prevention)
    let issue_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_sales.clone(),
        voucher_number: "INV-001".to_string(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 10).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: Some("Sales attempt without stock".into()),
        entries: vec![
            VoucherEntryDraft {
                ledger_id: cash.ledger_id.clone(),
                entry_type: EntryType::Dr,
                amount: Paise(50000),
                narration: None,
            },
            VoucherEntryDraft {
                ledger_id: sales.ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: Paise(50000),
                narration: None,
            },
        ],
    };

    let fail_res = PostingEngine::post_voucher_with_pre_post_hook(
        &mut conn,
        &issue_draft,
        None,
        |tx, v_id| {
            let req = StockMovementRequest {
                item_id: item.item_id.clone(),
                warehouse_id: wh.warehouse_id.clone(),
                movement_type: MovementType::Out,
                quantity_milli: 5_000,
                rate_paise: Paise(10000),
                batch_number: None,
                expiry_date: None,
            };
            InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-10", &req)?;
            Ok(())
        },
    );

    assert!(fail_res.is_err());
    let err_msg = fail_res.err().unwrap().to_string();
    assert!(err_msg.contains("Insufficient stock") || err_msg.contains("Negative stock is disabled"));

    // 2. Inward Purchase: Receive 20 Pcs @ Rs. 80
    let receipt_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_purchase.clone(),
        voucher_number: "PUR-001".to_string(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: Some("Widget purchase".into()),
        entries: vec![
            VoucherEntryDraft {
                ledger_id: purchase.ledger_id.clone(),
                entry_type: EntryType::Dr,
                amount: Paise(160000),
                narration: None,
            },
            VoucherEntryDraft {
                ledger_id: cash.ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: Paise(160000),
                narration: None,
            },
        ],
    };

    let post_res = PostingEngine::post_voucher_with_pre_post_hook(
        &mut conn,
        &receipt_draft,
        None,
        |tx, v_id| {
            let req = StockMovementRequest {
                item_id: item.item_id.clone(),
                warehouse_id: wh.warehouse_id.clone(),
                movement_type: MovementType::In,
                quantity_milli: 20_000,
                rate_paise: Paise(8000),
                batch_number: Some("BATCH-01".into()),
                expiry_date: None,
            };
            InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-01", &req)?;
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(post_res.status, VoucherStatus::Posted);

    // Stock should now be 20 Pcs (20,000 milli)
    let avail = InventoryEngine::get_available_quantity(&conn, &comp.company_id, &item.item_id, &wh.warehouse_id).unwrap();
    assert_eq!(avail, 20_000);

    // 3. Now Sales Issue of 5 Pcs succeeds
    let post_sale_res = PostingEngine::post_voucher_with_pre_post_hook(
        &mut conn,
        &issue_draft,
        None,
        |tx, v_id| {
            let req = StockMovementRequest {
                item_id: item.item_id.clone(),
                warehouse_id: wh.warehouse_id.clone(),
                movement_type: MovementType::Out,
                quantity_milli: 5_000,
                rate_paise: Paise(8000),
                batch_number: None,
                expiry_date: None,
            };
            InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-10", &req)?;
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(post_sale_res.status, VoucherStatus::Posted);

    // Remaining stock: 15 Pcs (15,000 milli)
    let remaining = InventoryEngine::get_available_quantity(&conn, &comp.company_id, &item.item_id, &wh.warehouse_id).unwrap();
    assert_eq!(remaining, 15_000);
}

#[test]
fn test_fifo_layer_consumption() {
    let (mut conn, comp, fy) = setup_test_company();
    let pcs = Uom::create_simple(&conn, &comp.company_id, "Pcs", Some("Pieces"), 0).unwrap();
    let wh = Warehouse::create(&conn, &comp.company_id, "Main Godown", None).unwrap();
    let grp_cash = AccountGroup::get_by_name(&conn, &comp.company_id, "Cash-in-hand").unwrap().unwrap();
    let grp_pur = AccountGroup::get_by_name(&conn, &comp.company_id, "Purchase Accounts").unwrap().unwrap();
    let grp_sal = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts").unwrap().unwrap();

    let cash = Ledger::create(&conn, &comp.company_id, &grp_cash.group_id, "Cash", Paise::ZERO, None, true).unwrap();
    let purchase = Ledger::create(&conn, &comp.company_id, &grp_pur.group_id, "Purchase A/c", Paise::ZERO, None, false).unwrap();
    let sales = Ledger::create(&conn, &comp.company_id, &grp_sal.group_id, "Sales A/c", Paise::ZERO, None, false).unwrap();

    let vt_sales: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let vt_purchase: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Purchase'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let item = StockItem::create(
        &conn,
        &CreateStockItemRequest {
            company_id: comp.company_id.clone(),
            stock_group_id: None,
            category_id: None,
            uom_id: pcs.uom_id.clone(),
            name: "Component X".to_string(),
            sku: Some("CMP-X".into()),
            description: None,
            hsn_sac_code: None,
            gst_rate_basis_points: 0,
            valuation_method: "FIFO".to_string(),
            opening_qty_milli: 0,
            opening_rate_paise: Paise::ZERO,
            reorder_level_milli: 0,
        },
    )
    .unwrap();

    // Lot 1: 10 Pcs @ Rs. 100 = Rs. 1,000 (Date: 2026-05-01)
    let p1 = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_purchase.clone(),
        voucher_number: "PUR-101".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: purchase.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(100000), narration: None },
            VoucherEntryDraft { ledger_id: cash.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(100000), narration: None },
        ],
    };
    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &p1, None, |tx, v_id| {
        let req = StockMovementRequest {
            item_id: item.item_id.clone(),
            warehouse_id: wh.warehouse_id.clone(),
            movement_type: MovementType::In,
            quantity_milli: 10_000,
            rate_paise: Paise(10000),
            batch_number: None,
            expiry_date: None,
        };
        InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-01", &req)?;
        Ok(())
    }).unwrap();

    // Lot 2: 10 Pcs @ Rs. 120 = Rs. 1,200 (Date: 2026-05-05)
    let p2 = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_purchase.clone(),
        voucher_number: "PUR-102".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 5).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: purchase.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(120000), narration: None },
            VoucherEntryDraft { ledger_id: cash.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(120000), narration: None },
        ],
    };
    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &p2, None, |tx, v_id| {
        let req = StockMovementRequest {
            item_id: item.item_id.clone(),
            warehouse_id: wh.warehouse_id.clone(),
            movement_type: MovementType::In,
            quantity_milli: 10_000,
            rate_paise: Paise(12000),
            batch_number: None,
            expiry_date: None,
        };
        InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-05", &req)?;
        Ok(())
    }).unwrap();

    // Issue: 15 Pcs (Date: 2026-05-10)
    // Under FIFO: consumes 10 Pcs @ Rs 100 + 5 Pcs @ Rs 120
    // Remaining should be: 5 Pcs @ Rs 120 = Rs. 600
    let s1 = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_sales.clone(),
        voucher_number: "SAL-101".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 10).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: cash.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(225000), narration: None },
            VoucherEntryDraft { ledger_id: sales.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(225000), narration: None },
        ],
    };
    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &s1, None, |tx, v_id| {
        let req = StockMovementRequest {
            item_id: item.item_id.clone(),
            warehouse_id: wh.warehouse_id.clone(),
            movement_type: MovementType::Out,
            quantity_milli: 15_000,
            rate_paise: Paise(15000),
            batch_number: None,
            expiry_date: None,
        };
        InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-10", &req)?;
        Ok(())
    }).unwrap();

    // Check FIFO Valuation
    let val = InventoryValuationEngine::calculate_valuation(&conn, &item.item_id, None).unwrap();
    assert_eq!(val.total_closing_qty_milli, 5_000);
    assert_eq!(val.total_closing_value_paise, Paise(60000));
    assert_eq!(val.effective_unit_rate_paise, Paise(12000));
}

#[test]
fn test_weighted_average_valuation() {
    let (mut conn, comp, fy) = setup_test_company();
    let pcs = Uom::create_simple(&conn, &comp.company_id, "Pcs", Some("Pieces"), 0).unwrap();
    let wh = Warehouse::create(&conn, &comp.company_id, "Godown", None).unwrap();
    let grp_cash = AccountGroup::get_by_name(&conn, &comp.company_id, "Cash-in-hand").unwrap().unwrap();
    let grp_pur = AccountGroup::get_by_name(&conn, &comp.company_id, "Purchase Accounts").unwrap().unwrap();
    let grp_sal = AccountGroup::get_by_name(&conn, &comp.company_id, "Sales Accounts").unwrap().unwrap();

    let cash = Ledger::create(&conn, &comp.company_id, &grp_cash.group_id, "Cash", Paise::ZERO, None, true).unwrap();
    let purchase = Ledger::create(&conn, &comp.company_id, &grp_pur.group_id, "Purchase A/c", Paise::ZERO, None, false).unwrap();
    let sales = Ledger::create(&conn, &comp.company_id, &grp_sal.group_id, "Sales A/c", Paise::ZERO, None, false).unwrap();

    let vt_sales: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let vt_purchase: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Purchase'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let item = StockItem::create(
        &conn,
        &CreateStockItemRequest {
            company_id: comp.company_id.clone(),
            stock_group_id: None,
            category_id: None,
            uom_id: pcs.uom_id.clone(),
            name: "Weighted Commodity Y".to_string(),
            sku: Some("CMD-Y".into()),
            description: None,
            hsn_sac_code: None,
            gst_rate_basis_points: 0,
            valuation_method: "WeightedAverage".to_string(),
            opening_qty_milli: 0,
            opening_rate_paise: Paise::ZERO,
            reorder_level_milli: 0,
        },
    )
    .unwrap();

    // Purchase 1: 10 Pcs @ Rs 100 = Rs 1,000
    let p1 = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_purchase.clone(),
        voucher_number: "P-WA-1".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: purchase.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(100000), narration: None },
            VoucherEntryDraft { ledger_id: cash.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(100000), narration: None },
        ],
    };
    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &p1, None, |tx, v_id| {
        let req = StockMovementRequest {
            item_id: item.item_id.clone(),
            warehouse_id: wh.warehouse_id.clone(),
            movement_type: MovementType::In,
            quantity_milli: 10_000,
            rate_paise: Paise(10000),
            batch_number: None,
            expiry_date: None,
        };
        InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-01", &req)?;
        Ok(())
    }).unwrap();

    // Purchase 2: 10 Pcs @ Rs 200 = Rs 2,000
    // Total: 20 Pcs, Total Value: Rs 3,000 => Avg rate: Rs 150/pc
    let p2 = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_purchase.clone(),
        voucher_number: "P-WA-2".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: purchase.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(200000), narration: None },
            VoucherEntryDraft { ledger_id: cash.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(200000), narration: None },
        ],
    };
    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &p2, None, |tx, v_id| {
        let req = StockMovementRequest {
            item_id: item.item_id.clone(),
            warehouse_id: wh.warehouse_id.clone(),
            movement_type: MovementType::In,
            quantity_milli: 10_000,
            rate_paise: Paise(20000),
            batch_number: None,
            expiry_date: None,
        };
        InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-02", &req)?;
        Ok(())
    }).unwrap();

    // Sale: 5 Pcs (consumes at avg rate of Rs 150 = Rs 750)
    // Remaining: 15 Pcs @ Rs 150 = Rs 2,250
    let s1 = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_sales.clone(),
        voucher_number: "S-WA-1".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 3).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: cash.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(100000), narration: None },
            VoucherEntryDraft { ledger_id: sales.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(100000), narration: None },
        ],
    };
    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &s1, None, |tx, v_id| {
        let req = StockMovementRequest {
            item_id: item.item_id.clone(),
            warehouse_id: wh.warehouse_id.clone(),
            movement_type: MovementType::Out,
            quantity_milli: 5_000,
            rate_paise: Paise(15000),
            batch_number: None,
            expiry_date: None,
        };
        InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-03", &req)?;
        Ok(())
    }).unwrap();

    let val = InventoryValuationEngine::calculate_valuation(&conn, &item.item_id, None).unwrap();
    assert_eq!(val.total_closing_qty_milli, 15_000);
    assert_eq!(val.total_closing_value_paise, Paise(225000));
    assert_eq!(val.effective_unit_rate_paise, Paise(15000));
}

#[test]
fn test_stock_transfer_between_warehouses() {
    let (mut conn, comp, fy) = setup_test_company();
    let pcs = Uom::create_simple(&conn, &comp.company_id, "Pcs", Some("Pieces"), 0).unwrap();
    let wh_a = Warehouse::create(&conn, &comp.company_id, "Warehouse A (Source)", None).unwrap();
    let wh_b = Warehouse::create(&conn, &comp.company_id, "Warehouse B (Dest)", None).unwrap();
    let grp_cash = AccountGroup::get_by_name(&conn, &comp.company_id, "Cash-in-hand").unwrap().unwrap();
    let grp_pur = AccountGroup::get_by_name(&conn, &comp.company_id, "Purchase Accounts").unwrap().unwrap();
    let grp_ca = AccountGroup::get_by_name(&conn, &comp.company_id, "Current Assets").unwrap().unwrap();

    let cash = Ledger::create(&conn, &comp.company_id, &grp_cash.group_id, "Cash", Paise::ZERO, None, true).unwrap();
    let purchase = Ledger::create(&conn, &comp.company_id, &grp_pur.group_id, "Purchase A/c", Paise::ZERO, None, false).unwrap();
    let journal_transit = Ledger::create(&conn, &comp.company_id, &grp_ca.group_id, "Stock Transit", Paise::ZERO, None, false).unwrap();

    let vt_purchase: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Purchase'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let vt_journal: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Journal'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let item = StockItem::create(
        &conn,
        &CreateStockItemRequest {
            company_id: comp.company_id.clone(),
            stock_group_id: None,
            category_id: None,
            uom_id: pcs.uom_id.clone(),
            name: "Transferable Item Z".to_string(),
            sku: None,
            description: None,
            hsn_sac_code: None,
            gst_rate_basis_points: 0,
            valuation_method: "FIFO".to_string(),
            opening_qty_milli: 0,
            opening_rate_paise: Paise::ZERO,
            reorder_level_milli: 0,
        },
    )
    .unwrap();

    // 1. Initial purchase into Warehouse A: 50 Pcs
    let p_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_purchase.clone(),
        voucher_number: "PUR-TRF-01".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: purchase.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(500000), narration: None },
            VoucherEntryDraft { ledger_id: cash.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(500000), narration: None },
        ],
    };
    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &p_draft, None, |tx, v_id| {
        let req = StockMovementRequest {
            item_id: item.item_id.clone(),
            warehouse_id: wh_a.warehouse_id.clone(),
            movement_type: MovementType::In,
            quantity_milli: 50_000,
            rate_paise: Paise(10000),
            batch_number: None,
            expiry_date: None,
        };
        InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-01", &req)?;
        Ok(())
    }).unwrap();

    // 2. Transfer 20 Pcs from Warehouse A to Warehouse B
    let trf_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_journal.clone(),
        voucher_number: "TRF-001".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: Some("Transfer A to B".into()),
        entries: vec![
            VoucherEntryDraft { ledger_id: journal_transit.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(200000), narration: None },
            VoucherEntryDraft { ledger_id: journal_transit.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(200000), narration: None },
        ],
    };

    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &trf_draft, None, |tx, v_id| {
        let trf_req = StockTransferRequest {
            item_id: item.item_id.clone(),
            from_warehouse_id: wh_a.warehouse_id.clone(),
            to_warehouse_id: wh_b.warehouse_id.clone(),
            quantity_milli: 20_000,
            rate_paise: Paise(10000),
        };
        InventoryEngine::record_transfer(tx, &comp.company_id, v_id, "2026-05-02", &trf_req)?;
        Ok(())
    }).unwrap();

    // Verify balances in both warehouses
    let bal_a = InventoryEngine::get_available_quantity(&conn, &comp.company_id, &item.item_id, &wh_a.warehouse_id).unwrap();
    let bal_b = InventoryEngine::get_available_quantity(&conn, &comp.company_id, &item.item_id, &wh_b.warehouse_id).unwrap();

    assert_eq!(bal_a, 30_000);
    assert_eq!(bal_b, 20_000);
}

#[test]
fn test_inventory_reports_generation() {
    let (mut conn, comp, fy) = setup_test_company();
    let pcs = Uom::create_simple(&conn, &comp.company_id, "Pcs", Some("Pieces"), 0).unwrap();
    let wh = Warehouse::create(&conn, &comp.company_id, "Central WH", None).unwrap();
    let grp_cash = AccountGroup::get_by_name(&conn, &comp.company_id, "Cash-in-hand").unwrap().unwrap();
    let grp_pur = AccountGroup::get_by_name(&conn, &comp.company_id, "Purchase Accounts").unwrap().unwrap();

    let cash = Ledger::create(&conn, &comp.company_id, &grp_cash.group_id, "Cash", Paise::ZERO, None, true).unwrap();
    let purchase = Ledger::create(&conn, &comp.company_id, &grp_pur.group_id, "Purchase A/c", Paise::ZERO, None, false).unwrap();
    let grp = StockGroup::create(&conn, &comp.company_id, "Electronics", None).unwrap();

    let vt_purchase: String = conn
        .query_row(
            "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Purchase'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let item = StockItem::create(
        &conn,
        &CreateStockItemRequest {
            company_id: comp.company_id.clone(),
            stock_group_id: Some(grp.stock_group_id.clone()),
            category_id: None,
            uom_id: pcs.uom_id.clone(),
            name: "LED Monitor 24in".to_string(),
            sku: Some("MON-24".into()),
            description: None,
            hsn_sac_code: Some("8528".into()),
            gst_rate_basis_points: 1800,
            valuation_method: "FIFO".to_string(),
            opening_qty_milli: 5_000,
            opening_rate_paise: Paise(1000000),
            reorder_level_milli: 10_000,
        },
    )
    .unwrap();

    // Inward movement of 10 Pcs
    let p_draft = VoucherDraft {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: vt_purchase.clone(),
        voucher_number: "PUR-RPT-1".into(),
        voucher_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        reference_no: None,
        reference_date: None,
        narration: None,
        entries: vec![
            VoucherEntryDraft { ledger_id: purchase.ledger_id.clone(), entry_type: EntryType::Dr, amount: Paise(10000000), narration: None },
            VoucherEntryDraft { ledger_id: cash.ledger_id.clone(), entry_type: EntryType::Cr, amount: Paise(10000000), narration: None },
        ],
    };
    PostingEngine::post_voucher_with_pre_post_hook(&mut conn, &p_draft, None, |tx, v_id| {
        let req = StockMovementRequest {
            item_id: item.item_id.clone(),
            warehouse_id: wh.warehouse_id.clone(),
            movement_type: MovementType::In,
            quantity_milli: 10_000,
            rate_paise: Paise(1000000),
            batch_number: None,
            expiry_date: None,
        };
        InventoryEngine::record_movement(tx, &comp.company_id, v_id, None, "2026-05-01", &req)?;
        Ok(())
    }).unwrap();

    // 1. Stock Summary
    let summary = InventoryReportEngine::generate_stock_summary(&conn, &comp.company_id, None, None).unwrap();
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].closing_qty_milli, 15_000);
    assert_eq!(summary[0].closing_value_paise, Paise(15000000));

    // 2. Stock Item Ledger
    let ledger_entries = InventoryReportEngine::generate_stock_item_ledger(&conn, &comp.company_id, &item.item_id, None, None).unwrap();
    assert_eq!(ledger_entries.len(), 1);
    assert_eq!(ledger_entries[0].inward_qty_milli, 10_000);
    assert_eq!(ledger_entries[0].balance_qty_milli, 15_000);

    // 3. Valuation Reconciliation
    let recon = InventoryReportEngine::generate_valuation_reconciliation(&conn, &comp.company_id, None).unwrap();
    assert_eq!(recon.total_items_count, 1);
    assert_eq!(recon.total_inventory_valuation_paise, Paise(15000000));
}
