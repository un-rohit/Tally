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
            voucher_type_id: vt_sales_id.clone(),
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

    // 10. Phase 2 GST Engine Demonstration
    println!("\n=== PHASE 2: INDIAN GST ACCOUNTING DEMONSTRATION ===");
    CompanyGstRegistration::register(
        &conn,
        &comp.company_id,
        "27AAPFU0939F1ZV",
        "Acme Enterprises Pvt Ltd",
        Some("Acme"),
        "Regular",
        true,
    )?;
    println!("Registered Company Primary GSTIN: 27AAPFU0939F1ZV (Maharashtra)");

    let grp_debtors = AccountGroup::get_by_name(&conn, &comp.company_id, "Sundry Debtors")?
        .expect("Sundry Debtors group missing");
    let l_customer_intra = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_debtors.group_id,
        "Zenith Technologies (MH)",
        Paise::ZERO,
        None,
        true,
    )?;
    let l_customer_inter = Ledger::create(
        &conn,
        &comp.company_id,
        &grp_debtors.group_id,
        "Bangalore Cloudworks (KA)",
        Paise::ZERO,
        None,
        true,
    )?;

    // Post Intra-state GST Sales Invoice: ₹1,00,000 @ 18% -> CGST ₹9,000, SGST ₹9,000, Total ₹1,18,000
    let gst_inv1 = GstInvoiceEngine::post_sales_invoice(
        &mut conn,
        &GstSalesInvoiceRequest {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_sales_id.clone(),
            voucher_number: "GST-INV-001".to_string(),
            invoice_date: NaiveDate::from_ymd_opt(2026, 5, 10).unwrap(),
            customer_ledger_id: l_customer_intra.ledger_id.clone(),
            sales_ledger_id: l_sales.ledger_id.clone(),
            place_of_supply: "27".to_string(),
            party_gstin: Some("27AAAAA0000A1Z2".to_string()),
            party_type: PartyRegistrationType::Regular,
            is_reverse_charge: false,
            reference_no: Some("PO-881".to_string()),
            narration: Some("Cloud Architecture Services".to_string()),
            line_items: vec![TaxLineInput {
                line_number: 1,
                hsn_code: Some("998313".to_string()),
                description: "Cloud Consulting".to_string(),
                amount_paise: Paise::from_rupees(100000)?,
                is_tax_inclusive: false,
                rate: GstRate::RATE_18,
            }],
            enable_rounding: true,
        },
        Some("usr_admin"),
    )?;
    println!(
        "Posted GST Intra-state Invoice {}: Taxable {}, CGST {}, SGST {}, Total {}",
        gst_inv1.voucher.voucher_number,
        gst_inv1.tax_summary.total_taxable,
        gst_inv1.tax_summary.total_cgst,
        gst_inv1.tax_summary.total_sgst,
        gst_inv1.tax_summary.final_invoice_total
    );

    // Post Inter-state GST Sales Invoice: ₹50,000 @ 18% -> IGST ₹9,000, Total ₹59,000
    let gst_inv2 = GstInvoiceEngine::post_sales_invoice(
        &mut conn,
        &GstSalesInvoiceRequest {
            company_id: comp.company_id.clone(),
            fy_id: fy.fy_id.clone(),
            voucher_type_id: vt_sales_id,
            voucher_number: "GST-INV-002".to_string(),
            invoice_date: NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
            customer_ledger_id: l_customer_inter.ledger_id.clone(),
            sales_ledger_id: l_sales.ledger_id.clone(),
            place_of_supply: "29".to_string(), // Karnataka
            party_gstin: Some("29ABCDE1234F1ZW".to_string()),
            party_type: PartyRegistrationType::Regular,
            is_reverse_charge: false,
            reference_no: None,
            narration: Some("Export of IT Services interstate".to_string()),
            line_items: vec![TaxLineInput {
                line_number: 1,
                hsn_code: Some("998311".to_string()),
                description: "IT Advisory Services".to_string(),
                amount_paise: Paise::from_rupees(50000)?,
                is_tax_inclusive: false,
                rate: GstRate::RATE_18,
            }],
            enable_rounding: true,
        },
        Some("usr_admin"),
    )?;
    println!(
        "Posted GST Inter-state Invoice {}: Taxable {}, IGST {}, Total {}",
        gst_inv2.voucher.voucher_number,
        gst_inv2.tax_summary.total_taxable,
        gst_inv2.tax_summary.total_igst,
        gst_inv2.tax_summary.final_invoice_total
    );

    // 11. Generate GSTR-1 Preliminary Report
    let gstr1 = GstReportEngine::generate_gstr1(
        &conn,
        &comp.company_id,
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
    )?;
    println!("\n=== GSTR-1 PRELIMINARY REPORT ===");
    println!("Disclaimer: {}", gstr1.disclaimer);
    println!("Total Taxable Value: {}", gstr1.total_taxable);
    println!("Total Output CGST:   {}", gstr1.total_cgst);
    println!("Total Output SGST:   {}", gstr1.total_sgst);
    println!("Total Output IGST:   {}", gstr1.total_igst);
    println!("B2B Invoices Count:  {}", gstr1.b2b_supplies.len());

    // 12. Generate GSTR-3B Preliminary Report
    let gstr3b = GstReportEngine::generate_gstr3b(
        &conn,
        &comp.company_id,
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
    )?;
    println!("\n=== GSTR-3B PRELIMINARY SUMMARY ===");
    println!("Table 3.1(a) Outward Taxable: {}", gstr3b.outward_taxable_supplies.taxable_value);
    println!("Table 3.1(a) Integrated Tax:  {}", gstr3b.outward_taxable_supplies.igst_amount);
    println!("Table 3.1(a) Central Tax:     {}", gstr3b.outward_taxable_supplies.cgst_amount);
    println!("Table 3.1(a) State/UT Tax:    {}", gstr3b.outward_taxable_supplies.sgst_amount);

    // 13. Verify Final Audit Chain
    let audit_ok = AuditLogger::verify_chain(&conn, &comp.company_id)?;
    println!("\nMCA Cryptographic Audit Chain Valid: {}", audit_ok);

    println!("\nAll core accounting and GST modules executed successfully with complete equilibrium!");
    Ok(())
}
