use accounting_app::*;
use chrono::NaiveDate;
use rusqlite::Connection;

fn setup_gst_company() -> (Connection, Company, FinancialYear, String, String, String) {
    let mut conn = DbMigrator::open_in_memory().expect("Failed to initialize test DB");

    // Company in Maharashtra (State Code 27)
    let (comp, fy) = Company::create(
        &mut conn,
        "30001",
        "Apex Digital India Pvt Ltd",
        "Apex Digital India",
        "27",
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2027, 3, 31).unwrap(),
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
    )
    .expect("Failed to create company");

    AccountGroup::seed_standard_groups(&mut conn, &comp.company_id)
        .expect("Failed to seed standard groups");

    // Register Company GSTIN: 27AAPFU0939F1ZV (Maharashtra)
    CompanyGstRegistration::register(
        &conn,
        &comp.company_id,
        "27AAPFU0939F1ZV",
        "Apex Digital India Pvt Ltd",
        Some("Apex Digital"),
        "Regular",
        true,
    )
    .expect("Failed to register company GSTIN");

    // Find Sales Accounts group and Sundry Debtors group
    let sales_group_id: String = conn
        .query_row(
            "SELECT group_id FROM account_groups WHERE company_id = ?1 AND name = 'Sales Accounts'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let debtors_group_id: String = conn
        .query_row(
            "SELECT group_id FROM account_groups WHERE company_id = ?1 AND name = 'Sundry Debtors'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let creditors_group_id: String = conn
        .query_row(
            "SELECT group_id FROM account_groups WHERE company_id = ?1 AND name = 'Sundry Creditors'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let sales_ledger = Ledger::create(
        &mut conn,
        &comp.company_id,
        &sales_group_id,
        "Software Sales",
        Paise(0),
        None,
        false,
    )
    .unwrap();

    let debtor_ledger = Ledger::create(
        &mut conn,
        &comp.company_id,
        &debtors_group_id,
        "TechNova Solutions",
        Paise(0),
        None,
        true,
    )
    .unwrap();

    let creditor_ledger = Ledger::create(
        &mut conn,
        &comp.company_id,
        &creditors_group_id,
        "CloudCore Infrastructure",
        Paise(0),
        None,
        true,
    )
    .unwrap();

    (
        conn,
        comp,
        fy,
        sales_ledger.ledger_id,
        debtor_ledger.ledger_id,
        creditor_ledger.ledger_id,
    )
}

fn get_sales_voucher_type_id(conn: &Connection, company_id: &str) -> String {
    conn.query_row(
        "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Sales'",
        [company_id],
        |r| r.get(0),
    )
    .unwrap()
}

fn get_purchase_voucher_type_id(conn: &Connection, company_id: &str) -> String {
    conn.query_row(
        "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'Purchase'",
        [company_id],
        |r| r.get(0),
    )
    .unwrap()
}

fn get_credit_note_voucher_type_id(conn: &Connection, company_id: &str) -> String {
    conn.query_row(
        "SELECT voucher_type_id FROM voucher_types WHERE company_id = ?1 AND base_type = 'CreditNote'",
        [company_id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn test_gstin_validation_and_registration() {
    let mut conn = DbMigrator::open_in_memory().unwrap();
    let (comp, _) = Company::create(
        &mut conn,
        "30002",
        "Validation Corp",
        "Validation Corp",
        "29",
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2027, 3, 31).unwrap(),
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
    )
    .unwrap();

    // 1. Valid GSTIN
    let reg = CompanyGstRegistration::register(
        &conn,
        &comp.company_id,
        "29ABCDE1234F1ZW",
        "Validation Corp",
        None,
        "Regular",
        true,
    )
    .unwrap();

    assert_eq!(reg.state_code, "29");
    assert_eq!(reg.gstin.as_str(), "29ABCDE1234F1ZW");
    assert_eq!(reg.gstin.pan(), "ABCDE1234F");

    // 2. Invalid Checksum
    let err_chk = CompanyGstRegistration::register(
        &conn,
        &comp.company_id,
        "29ABCDE1234F1ZX", // Wrong check digit
        "Validation Corp",
        None,
        "Regular",
        false,
    );
    assert!(err_chk.is_err(), "Invalid Mod-36 check digit must be rejected");

    // 3. Invalid Format
    let err_fmt = CompanyGstRegistration::register(
        &conn,
        &comp.company_id,
        "29ABCDE1234F1Z", // Too short
        "Validation Corp",
        None,
        "Regular",
        false,
    );
    assert!(err_fmt.is_err(), "Invalid length must be rejected");
}

#[test]
fn test_intra_state_b2b_sales_invoice() {
    let (mut conn, comp, fy, sales_id, debtor_id, _) = setup_gst_company();
    let v_type_id = get_sales_voucher_type_id(&conn, &comp.company_id);

    // Intra-state B2B invoice: Seller 27 (MH), Buyer 27 (MH)
    // ₹1,00,000 @ 18% -> CGST ₹9,000, SGST ₹9,000, Total ₹1,18,000
    let req = GstSalesInvoiceRequest {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: v_type_id,
        voucher_number: "INV-2026-001".to_string(),
        invoice_date: NaiveDate::from_ymd_opt(2026, 5, 10).unwrap(),
        customer_ledger_id: debtor_id.clone(),
        sales_ledger_id: sales_id.clone(),
        place_of_supply: "27".to_string(),
        party_gstin: Some("27AAAAA0000A1Z2".to_string()),
        party_type: PartyRegistrationType::Regular,
        is_reverse_charge: false,
        reference_no: Some("PO-991".to_string()),
        narration: Some("Software Consulting Services".to_string()),
        line_items: vec![TaxLineInput {
            line_number: 1,
            hsn_code: Some("998313".to_string()),
            description: "Consulting Support".to_string(),
            amount_paise: Paise::from_rupees(100000).unwrap(),
            is_tax_inclusive: false,
            rate: GstRate::RATE_18,
        }],
        enable_rounding: true,
    };

    let result = GstInvoiceEngine::post_sales_invoice(&mut conn, &req, Some("admin")).unwrap();

    assert_eq!(result.invoice_type, "B2B");
    assert_eq!(result.tax_summary.is_interstate, false);
    assert_eq!(result.tax_summary.total_taxable, Paise::from_rupees(100000).unwrap());
    assert_eq!(result.tax_summary.total_cgst, Paise::from_rupees(9000).unwrap());
    assert_eq!(result.tax_summary.total_sgst, Paise::from_rupees(9000).unwrap());
    assert_eq!(result.tax_summary.total_igst, Paise(0));
    assert_eq!(result.tax_summary.final_invoice_total, Paise::from_rupees(118000).unwrap());

    // Check ledger balance: Debtor owes ₹1,18,000
    let debtor_bal = Ledger::get_closing_balance(&conn, &debtor_id).unwrap();
    assert_eq!(debtor_bal, Paise::from_rupees(118000).unwrap());
}

#[test]
fn test_inter_state_b2b_sales_invoice() {
    let (mut conn, comp, fy, sales_id, debtor_id, _) = setup_gst_company();
    let v_type_id = get_sales_voucher_type_id(&conn, &comp.company_id);

    // Inter-state B2B invoice: Seller 27 (MH), Buyer 29 (KA)
    // ₹2,00,000 @ 18% -> IGST ₹36,000, Total ₹2,36,000
    let req = GstSalesInvoiceRequest {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: v_type_id,
        voucher_number: "INV-2026-002".to_string(),
        invoice_date: NaiveDate::from_ymd_opt(2026, 5, 12).unwrap(),
        customer_ledger_id: debtor_id.clone(),
        sales_ledger_id: sales_id.clone(),
        place_of_supply: "29".to_string(), // Karnataka
        party_gstin: Some("29ABCDE1234F1ZW".to_string()),
        party_type: PartyRegistrationType::Regular,
        is_reverse_charge: false,
        reference_no: None,
        narration: Some("Interstate Cloud Migration".to_string()),
        line_items: vec![TaxLineInput {
            line_number: 1,
            hsn_code: Some("998311".to_string()),
            description: "Cloud Architecture".to_string(),
            amount_paise: Paise::from_rupees(200000).unwrap(),
            is_tax_inclusive: false,
            rate: GstRate::RATE_18,
        }],
        enable_rounding: true,
    };

    let result = GstInvoiceEngine::post_sales_invoice(&mut conn, &req, Some("admin")).unwrap();

    assert_eq!(result.invoice_type, "B2B");
    assert_eq!(result.tax_summary.is_interstate, true);
    assert_eq!(result.tax_summary.total_taxable, Paise::from_rupees(200000).unwrap());
    assert_eq!(result.tax_summary.total_igst, Paise::from_rupees(36000).unwrap());
    assert_eq!(result.tax_summary.total_cgst, Paise(0));
    assert_eq!(result.tax_summary.total_sgst, Paise(0));
    assert_eq!(result.tax_summary.final_invoice_total, Paise::from_rupees(236000).unwrap());
}

#[test]
fn test_multi_rate_invoice() {
    let (mut conn, comp, fy, sales_id, debtor_id, _) = setup_gst_company();
    let v_type_id = get_sales_voucher_type_id(&conn, &comp.company_id);

    // Multi-rate invoice:
    // Line 1: ₹50,000 @ 18% -> CGST 4,500, SGST 4,500
    // Line 2: ₹20,000 @ 5%  -> CGST 500, SGST 500
    // Line 3: ₹10,000 @ 0%  -> Tax 0
    // Total Taxable: ₹80,000. Total CGST: ₹5,000. Total SGST: ₹5,000. Total: ₹90,000.
    let req = GstSalesInvoiceRequest {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: v_type_id,
        voucher_number: "INV-2026-MULTI".to_string(),
        invoice_date: NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        customer_ledger_id: debtor_id.clone(),
        sales_ledger_id: sales_id.clone(),
        place_of_supply: "27".to_string(),
        party_gstin: Some("27AAAAA0000A1Z2".to_string()),
        party_type: PartyRegistrationType::Regular,
        is_reverse_charge: false,
        reference_no: None,
        narration: Some("Multi-rate mixed invoice".to_string()),
        line_items: vec![
            TaxLineInput {
                line_number: 1,
                hsn_code: Some("847130".to_string()),
                description: "Hardware Laptops".to_string(),
                amount_paise: Paise::from_rupees(50000).unwrap(),
                is_tax_inclusive: false,
                rate: GstRate::RATE_18,
            },
            TaxLineInput {
                line_number: 2,
                hsn_code: Some("996511".to_string()),
                description: "Freight Delivery".to_string(),
                amount_paise: Paise::from_rupees(20000).unwrap(),
                is_tax_inclusive: false,
                rate: GstRate::RATE_5,
            },
            TaxLineInput {
                line_number: 3,
                hsn_code: Some("1006".to_string()),
                description: "Grain sample exempt".to_string(),
                amount_paise: Paise::from_rupees(10000).unwrap(),
                is_tax_inclusive: false,
                rate: GstRate::NIL,
            },
        ],
        enable_rounding: true,
    };

    let result = GstInvoiceEngine::post_sales_invoice(&mut conn, &req, Some("admin")).unwrap();

    assert_eq!(result.tax_summary.total_taxable, Paise::from_rupees(80000).unwrap());
    assert_eq!(result.tax_summary.total_cgst, Paise::from_rupees(5000).unwrap());
    assert_eq!(result.tax_summary.total_sgst, Paise::from_rupees(5000).unwrap());
    assert_eq!(result.tax_summary.final_invoice_total, Paise::from_rupees(90000).unwrap());

    // Verify 3 tax items recorded in database
    let items_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM voucher_gst_tax_items WHERE voucher_id = ?1",
            [&result.voucher.voucher_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(items_count, 3);
}

#[test]
fn test_tax_inclusive_pricing_and_rounding() {
    let (mut conn, comp, fy, sales_id, debtor_id, _) = setup_gst_company();
    let v_type_id = get_sales_voucher_type_id(&conn, &comp.company_id);

    // Retail consumer invoice: ₹5,900 inclusive of 18% tax
    // Taxable = 5000.00, CGST = 450.00, SGST = 450.00, Total = 5900.00
    let req = GstSalesInvoiceRequest {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: v_type_id,
        voucher_number: "RET-001".to_string(),
        invoice_date: NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
        customer_ledger_id: debtor_id.clone(),
        sales_ledger_id: sales_id.clone(),
        place_of_supply: "27".to_string(),
        party_gstin: None,
        party_type: PartyRegistrationType::Consumer,
        is_reverse_charge: false,
        reference_no: None,
        narration: Some("Retail Sale".to_string()),
        line_items: vec![TaxLineInput {
            line_number: 1,
            hsn_code: Some("851713".to_string()),
            description: "Smartphone".to_string(),
            amount_paise: Paise::from_rupees(5900).unwrap(),
            is_tax_inclusive: true,
            rate: GstRate::RATE_18,
        }],
        enable_rounding: true,
    };

    let result = GstInvoiceEngine::post_sales_invoice(&mut conn, &req, Some("admin")).unwrap();

    assert_eq!(result.invoice_type, "B2CS");
    assert_eq!(result.tax_summary.total_taxable, Paise::from_rupees(5000).unwrap());
    assert_eq!(result.tax_summary.total_cgst, Paise::from_rupees(450).unwrap());
    assert_eq!(result.tax_summary.total_sgst, Paise::from_rupees(450).unwrap());
    assert_eq!(result.tax_summary.final_invoice_total, Paise::from_rupees(5900).unwrap());
}

#[test]
fn test_reverse_charge_purchase_scenario() {
    let (mut conn, comp, fy, _, _, creditor_id) = setup_gst_company();
    let v_type_id = get_purchase_voucher_type_id(&conn, &comp.company_id);

    // Expense ledger for legal fees
    let indirect_exp_group_id: String = conn
        .query_row(
            "SELECT group_id FROM account_groups WHERE company_id = ?1 AND name = 'Indirect Expenses'",
            [&comp.company_id],
            |r| r.get(0),
        )
        .unwrap();

    let legal_exp_ledger = Ledger::create(
        &mut conn,
        &comp.company_id,
        &indirect_exp_group_id,
        "Legal Advisory Fees",
        Paise(0),
        None,
        false,
    )
    .unwrap();

    // RCM Purchase: Advocate Legal Services ₹40,000 @ 18% RCM (intra-state 27 -> 27)
    // Supplier payable: ₹40,000
    // Tax: CGST ₹3,600, SGST ₹3,600
    // RCM Liability Cr ₹3,600 CGST + ₹3,600 SGST
    // Input Tax Credit Dr ₹3,600 CGST + ₹3,600 SGST
    let req = GstPurchaseInvoiceRequest {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: v_type_id,
        voucher_number: "PUR-RCM-001".to_string(),
        invoice_date: NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
        supplier_ledger_id: creditor_id.clone(),
        expense_or_purchase_ledger_id: legal_exp_ledger.ledger_id.clone(),
        place_of_supply: "27".to_string(),
        supplier_gstin: None,
        party_type: PartyRegistrationType::Unregistered,
        is_reverse_charge: true,
        reference_no: Some("LAW-BILL-10".to_string()),
        narration: Some("Legal services under reverse charge".to_string()),
        line_items: vec![TaxLineInput {
            line_number: 1,
            hsn_code: Some("998211".to_string()),
            description: "High Court Representation".to_string(),
            amount_paise: Paise::from_rupees(40000).unwrap(),
            is_tax_inclusive: false,
            rate: GstRate::RATE_18,
        }],
        enable_rounding: true,
    };

    let result = GstInvoiceEngine::post_purchase_invoice(&mut conn, &req, Some("admin")).unwrap();

    assert_eq!(result.tax_summary.is_reverse_charge, true);
    assert_eq!(result.tax_summary.total_taxable, Paise::from_rupees(40000).unwrap());
    assert_eq!(result.tax_summary.total_cgst, Paise::from_rupees(3600).unwrap());
    assert_eq!(result.tax_summary.total_sgst, Paise::from_rupees(3600).unwrap());

    // Check creditor balance: Only base amount ₹40,000 owed to advocate
    let advocate_bal = Ledger::get_closing_balance(&conn, &creditor_id).unwrap();
    assert_eq!(advocate_bal, Paise::from_rupees(40000).unwrap());

    // Check GSTR-3B Table 3.1(d) reflects RCM liability
    let gstr3b = GstReportEngine::generate_gstr3b(
        &conn,
        &comp.company_id,
        NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
    )
    .unwrap();

    assert_eq!(gstr3b.inward_supplies_reverse_charge.taxable_value, Paise::from_rupees(40000).unwrap());
    assert_eq!(gstr3b.inward_supplies_reverse_charge.cgst_amount, Paise::from_rupees(3600).unwrap());
    assert_eq!(gstr3b.inward_supplies_reverse_charge.sgst_amount, Paise::from_rupees(3600).unwrap());
    assert_eq!(gstr3b.itc_rcm_inward.cgst_amount, Paise::from_rupees(3600).unwrap());
}

#[test]
fn test_credit_note_reversal_workflow() {
    let (mut conn, comp, fy, sales_id, debtor_id, _) = setup_gst_company();
    let sales_type_id = get_sales_voucher_type_id(&conn, &comp.company_id);
    let cn_type_id = get_credit_note_voucher_type_id(&conn, &comp.company_id);

    // 1. Post original invoice: ₹1,00,000 + 18% GST (₹1,18,000)
    let inv_req = GstSalesInvoiceRequest {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: sales_type_id,
        voucher_number: "INV-ORIG-100".to_string(),
        invoice_date: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        customer_ledger_id: debtor_id.clone(),
        sales_ledger_id: sales_id.clone(),
        place_of_supply: "27".to_string(),
        party_gstin: Some("27AAAAA0000A1Z2".to_string()),
        party_type: PartyRegistrationType::Regular,
        is_reverse_charge: false,
        reference_no: None,
        narration: None,
        line_items: vec![TaxLineInput {
            line_number: 1,
            hsn_code: Some("998313".to_string()),
            description: "Consulting".to_string(),
            amount_paise: Paise::from_rupees(100000).unwrap(),
            is_tax_inclusive: false,
            rate: GstRate::RATE_18,
        }],
        enable_rounding: true,
    };
    let inv_res = GstInvoiceEngine::post_sales_invoice(&mut conn, &inv_req, Some("admin")).unwrap();

    // 2. Post credit note (50% return: ₹50,000 + ₹9,000 GST = ₹59,000)
    let cn_req = GstCreditNoteRequest {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: cn_type_id,
        voucher_number: "CN-001".to_string(),
        note_date: NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
        original_voucher_id: Some(inv_res.voucher.voucher_id),
        original_invoice_no: "INV-ORIG-100".to_string(),
        original_invoice_date: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        customer_ledger_id: debtor_id.clone(),
        sales_ledger_id: sales_id.clone(),
        place_of_supply: "27".to_string(),
        party_gstin: Some("27AAAAA0000A1Z2".to_string()),
        party_type: PartyRegistrationType::Regular,
        narration: Some("50% discount credit note".to_string()),
        line_items: vec![TaxLineInput {
            line_number: 1,
            hsn_code: Some("998313".to_string()),
            description: "Consulting Discount".to_string(),
            amount_paise: Paise::from_rupees(50000).unwrap(),
            is_tax_inclusive: false,
            rate: GstRate::RATE_18,
        }],
    };

    let cn_res = GstInvoiceEngine::post_credit_note(&mut conn, &cn_req, Some("admin")).unwrap();
    assert_eq!(cn_res.invoice_type, "CDNR");

    // Net customer balance should now be ₹1,18,000 - ₹59,000 = ₹59,000
    let debtor_bal = Ledger::get_closing_balance(&conn, &debtor_id).unwrap();
    assert_eq!(debtor_bal, Paise::from_rupees(59000).unwrap());

    // Verify GSTR-1 contains the credit note in Table 9B
    let gstr1 = GstReportEngine::generate_gstr1(
        &conn,
        &comp.company_id,
        NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 7, 31).unwrap(),
    )
    .unwrap();

    assert_eq!(gstr1.credit_notes.len(), 1);
    assert_eq!(gstr1.credit_notes[0].note_number, "CN-001");
    assert_eq!(gstr1.credit_notes[0].original_invoice_no, "INV-ORIG-100");
    assert_eq!(gstr1.credit_notes[0].taxable_value, Paise::from_rupees(50000).unwrap());
}

#[test]
fn test_gst_posted_voucher_immutability_triggers() {
    let (mut conn, comp, fy, sales_id, debtor_id, _) = setup_gst_company();
    let v_type_id = get_sales_voucher_type_id(&conn, &comp.company_id);

    let req = GstSalesInvoiceRequest {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: v_type_id,
        voucher_number: "INV-IMMUTABLE-01".to_string(),
        invoice_date: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        customer_ledger_id: debtor_id,
        sales_ledger_id: sales_id,
        place_of_supply: "27".to_string(),
        party_gstin: Some("27AAAAA0000A1Z2".to_string()),
        party_type: PartyRegistrationType::Regular,
        is_reverse_charge: false,
        reference_no: None,
        narration: None,
        line_items: vec![TaxLineInput {
            line_number: 1,
            hsn_code: Some("847130".to_string()),
            description: "Server hardware".to_string(),
            amount_paise: Paise::from_rupees(50000).unwrap(),
            is_tax_inclusive: false,
            rate: GstRate::RATE_18,
        }],
        enable_rounding: true,
    };

    let res = GstInvoiceEngine::post_sales_invoice(&mut conn, &req, Some("admin")).unwrap();
    let vid = res.voucher.voucher_id;

    // 1. Attempt UPDATE on voucher_gst_details -> Must fail via trigger
    let upd_res = conn.execute(
        "UPDATE voucher_gst_details SET place_of_supply = '29' WHERE voucher_id = ?1",
        [&vid],
    );
    assert!(upd_res.is_err(), "Trigger must prevent updating GST details on posted voucher");

    // 2. Attempt DELETE on voucher_gst_details -> Must fail via trigger
    let del_res = conn.execute(
        "DELETE FROM voucher_gst_details WHERE voucher_id = ?1",
        [&vid],
    );
    assert!(del_res.is_err(), "Trigger must prevent deleting GST details on posted voucher");

    // 3. Attempt UPDATE on voucher_gst_tax_items -> Must fail via trigger
    let item_upd = conn.execute(
        "UPDATE voucher_gst_tax_items SET taxable_amount_paise = 1 WHERE voucher_id = ?1",
        [&vid],
    );
    assert!(item_upd.is_err(), "Trigger must prevent updating GST tax items on posted voucher");

    // 4. Attempt DELETE on voucher_gst_tax_items -> Must fail via trigger
    let item_del = conn.execute(
        "DELETE FROM voucher_gst_tax_items WHERE voucher_id = ?1",
        [&vid],
    );
    assert!(item_del.is_err(), "Trigger must prevent deleting GST tax items on posted voucher");
}

#[test]
fn test_failed_posting_rollback_integrity() {
    let (mut conn, comp, fy, sales_id, debtor_id, _) = setup_gst_company();
    let v_type_id = get_sales_voucher_type_id(&conn, &comp.company_id);

    // Provide invalid date (prior to books beginning date)
    let req = GstSalesInvoiceRequest {
        company_id: comp.company_id.clone(),
        fy_id: fy.fy_id.clone(),
        voucher_type_id: v_type_id,
        voucher_number: "INV-FAIL-01".to_string(),
        invoice_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(), // Invalid year
        customer_ledger_id: debtor_id,
        sales_ledger_id: sales_id,
        place_of_supply: "27".to_string(),
        party_gstin: Some("27AAAAA0000A1Z2".to_string()),
        party_type: PartyRegistrationType::Regular,
        is_reverse_charge: false,
        reference_no: None,
        narration: None,
        line_items: vec![TaxLineInput {
            line_number: 1,
            hsn_code: Some("847130".to_string()),
            description: "Hardware".to_string(),
            amount_paise: Paise::from_rupees(50000).unwrap(),
            is_tax_inclusive: false,
            rate: GstRate::RATE_18,
        }],
        enable_rounding: true,
    };

    let post_res = GstInvoiceEngine::post_sales_invoice(&mut conn, &req, Some("admin"));
    assert!(post_res.is_err(), "Invalid voucher date must fail");

    // Verify complete transaction rollback: 0 vouchers, 0 lines, 0 gst records
    let v_count: i64 = conn.query_row("SELECT COUNT(*) FROM vouchers", [], |r| r.get(0)).unwrap();
    let e_count: i64 = conn.query_row("SELECT COUNT(*) FROM voucher_entries", [], |r| r.get(0)).unwrap();
    let g_count: i64 = conn.query_row("SELECT COUNT(*) FROM voucher_gst_details", [], |r| r.get(0)).unwrap();
    let t_count: i64 = conn.query_row("SELECT COUNT(*) FROM voucher_gst_tax_items", [], |r| r.get(0)).unwrap();

    assert_eq!(v_count, 0);
    assert_eq!(e_count, 0);
    assert_eq!(g_count, 0);
    assert_eq!(t_count, 0);
}

#[test]
fn test_end_to_end_reconciliation_and_exports() {
    let (mut conn, comp, fy, sales_id, debtor_id, creditor_id) = setup_gst_company();
    let sales_type_id = get_sales_voucher_type_id(&conn, &comp.company_id);
    let pur_type_id = get_purchase_voucher_type_id(&conn, &comp.company_id);

    // 1. Post B2B Intra-state Sales Invoice: ₹1,00,000 @ 18% -> CGST ₹9,000, SGST ₹9,000
    GstInvoiceEngine::post_sales_invoice(
        &mut conn,
        &GstSalesInvoiceRequest {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: sales_type_id.clone(),
            voucher_number: "RECON-INV-1".to_string(),
            invoice_date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            customer_ledger_id: debtor_id.clone(),
            sales_ledger_id: sales_id.clone(),
            place_of_supply: "27".to_string(),
            party_gstin: Some("27AAAAA0000A1Z2".to_string()),
            party_type: PartyRegistrationType::Regular,
            is_reverse_charge: false,
            reference_no: None,
            narration: None,
            line_items: vec![TaxLineInput {
                line_number: 1,
                hsn_code: Some("998313".to_string()),
                description: "IT Service".to_string(),
                amount_paise: Paise::from_rupees(100000).unwrap(),
                is_tax_inclusive: false,
                rate: GstRate::RATE_18,
            }],
            enable_rounding: true,
        },
        Some("admin"),
    )
    .unwrap();

    // 2. Post B2B Inter-state Sales Invoice: ₹50,000 @ 18% -> IGST ₹9,000
    GstInvoiceEngine::post_sales_invoice(
        &mut conn,
        &GstSalesInvoiceRequest {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: sales_type_id,
            voucher_number: "RECON-INV-2".to_string(),
            invoice_date: NaiveDate::from_ymd_opt(2026, 9, 5).unwrap(),
            customer_ledger_id: debtor_id.clone(),
            sales_ledger_id: sales_id.clone(),
            place_of_supply: "29".to_string(),
            party_gstin: Some("29ABCDE1234F1ZW".to_string()),
            party_type: PartyRegistrationType::Regular,
            is_reverse_charge: false,
            reference_no: None,
            narration: None,
            line_items: vec![TaxLineInput {
                line_number: 1,
                hsn_code: Some("998311".to_string()),
                description: "Interstate Consulting".to_string(),
                amount_paise: Paise::from_rupees(50000).unwrap(),
                is_tax_inclusive: false,
                rate: GstRate::RATE_18,
            }],
            enable_rounding: true,
        },
        Some("admin"),
    )
    .unwrap();

    // 3. Post B2B Purchase Invoice: ₹30,000 @ 18% (intra-state) -> Input CGST ₹2,700, Input SGST ₹2,700
    GstInvoiceEngine::post_purchase_invoice(
        &mut conn,
        &GstPurchaseInvoiceRequest {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: pur_type_id,
            voucher_number: "RECON-PUR-1".to_string(),
            invoice_date: NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            supplier_ledger_id: creditor_id,
            expense_or_purchase_ledger_id: sales_id,
            place_of_supply: "27".to_string(),
            supplier_gstin: Some("27AAAAA0000A1Z2".to_string()),
            party_type: PartyRegistrationType::Regular,
            is_reverse_charge: false,
            reference_no: None,
            narration: None,
            line_items: vec![TaxLineInput {
                line_number: 1,
                hsn_code: Some("847130".to_string()),
                description: "Equipment Purchase".to_string(),
                amount_paise: Paise::from_rupees(30000).unwrap(),
                is_tax_inclusive: false,
                rate: GstRate::RATE_18,
            }],
            enable_rounding: true,
        },
        Some("admin"),
    )
    .unwrap();

    // 4. Generate Reports for September 2026
    let from_date = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let to_date = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();

    let gstr1 = GstReportEngine::generate_gstr1(&conn, &comp.company_id, from_date, to_date).unwrap();
    let gstr3b = GstReportEngine::generate_gstr3b(&conn, &comp.company_id, from_date, to_date).unwrap();
    let tb = ReportEngine::generate_trial_balance(&conn, &comp.company_id, Some(to_date)).unwrap();

    assert!(tb.is_balanced, "Trial Balance must be in exact equilibrium");

    // Reconciliation Assertions:
    // GSTR-1 total taxable outward = ₹1,00,000 + ₹50,000 = ₹1,50,000
    assert_eq!(gstr1.total_taxable, Paise::from_rupees(150000).unwrap());
    assert_eq!(gstr1.total_cgst, Paise::from_rupees(9000).unwrap());
    assert_eq!(gstr1.total_sgst, Paise::from_rupees(9000).unwrap());
    assert_eq!(gstr1.total_igst, Paise::from_rupees(9000).unwrap());

    // GSTR-3B Outward Taxable supplies must match GSTR-1
    assert_eq!(gstr3b.outward_taxable_supplies.taxable_value, gstr1.total_taxable);
    assert_eq!(gstr3b.outward_taxable_supplies.cgst_amount, gstr1.total_cgst);
    assert_eq!(gstr3b.outward_taxable_supplies.sgst_amount, gstr1.total_sgst);
    assert_eq!(gstr3b.outward_taxable_supplies.igst_amount, gstr1.total_igst);

    // GSTR-3B Input Tax Credit: ₹2,700 CGST, ₹2,700 SGST
    assert_eq!(gstr3b.itc_all_other.cgst_amount, Paise::from_rupees(2700).unwrap());
    assert_eq!(gstr3b.itc_all_other.sgst_amount, Paise::from_rupees(2700).unwrap());

    // Check JSON and CSV exports
    let gstr1_json = gstr1.to_json().unwrap();
    assert!(gstr1_json.contains(GST_PRELIMINARY_DISCLAIMER));
    assert!(gstr1_json.contains("RECON-INV-1"));

    let gstr3b_csv = gstr3b.to_summary_csv();
    assert!(gstr3b_csv.contains(GST_PRELIMINARY_DISCLAIMER));
    assert!(gstr3b_csv.contains("150000.00"));
}
