use crate::domain::gst::{
    CompanyGstRegistration, Gstin, InvoiceTaxSummary, PartyRegistrationType,
    TaxCalculator, TaxLineInput,
};
use crate::domain::ledger::Ledger;
use crate::domain::money::Paise;
use crate::domain::voucher::{EntryType, Voucher, VoucherDraft, VoucherEntryDraft};
use crate::engine::posting::PostingEngine;
use crate::error::{AccountingError, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardGstLedgers {
    pub output_cgst_id: String,
    pub output_sgst_id: String,
    pub output_igst_id: String,
    pub input_cgst_id: String,
    pub input_sgst_id: String,
    pub input_igst_id: String,
    pub rcm_liability_cgst_id: String,
    pub rcm_liability_sgst_id: String,
    pub rcm_liability_igst_id: String,
    pub round_off_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GstSalesInvoiceRequest {
    pub company_id: String,
    pub fy_id: String,
    pub voucher_type_id: String,
    pub voucher_number: String,
    pub invoice_date: NaiveDate,
    pub customer_ledger_id: String,
    pub sales_ledger_id: String,
    pub place_of_supply: String, // 2-digit state code
    pub party_gstin: Option<String>,
    pub party_type: PartyRegistrationType,
    pub is_reverse_charge: bool,
    pub reference_no: Option<String>,
    pub narration: Option<String>,
    pub line_items: Vec<TaxLineInput>,
    pub enable_rounding: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GstPurchaseInvoiceRequest {
    pub company_id: String,
    pub fy_id: String,
    pub voucher_type_id: String,
    pub voucher_number: String,
    pub invoice_date: NaiveDate,
    pub supplier_ledger_id: String,
    pub expense_or_purchase_ledger_id: String,
    pub place_of_supply: String,
    pub supplier_gstin: Option<String>,
    pub party_type: PartyRegistrationType,
    pub is_reverse_charge: bool,
    pub reference_no: Option<String>,
    pub narration: Option<String>,
    pub line_items: Vec<TaxLineInput>,
    pub enable_rounding: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GstCreditNoteRequest {
    pub company_id: String,
    pub fy_id: String,
    pub voucher_type_id: String,
    pub voucher_number: String,
    pub note_date: NaiveDate,
    pub original_voucher_id: Option<String>,
    pub original_invoice_no: String,
    pub original_invoice_date: NaiveDate,
    pub customer_ledger_id: String,
    pub sales_ledger_id: String,
    pub place_of_supply: String,
    pub party_gstin: Option<String>,
    pub party_type: PartyRegistrationType,
    pub narration: Option<String>,
    pub line_items: Vec<TaxLineInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GstInvoiceResult {
    pub voucher: Voucher,
    pub tax_summary: InvoiceTaxSummary,
    pub invoice_type: String, // "B2B", "B2CL", "B2CS", "Export", "NilRated", "Exempt"
}

pub struct GstInvoiceEngine;

impl GstInvoiceEngine {
    /// Ensures that the standard Indian GST tax ledgers exist for a company under 'Duties & Taxes'.
    pub fn provision_standard_gst_ledgers(
        conn: &mut Connection,
        company_id: &str,
    ) -> Result<StandardGstLedgers> {
        let duties_group_id: String = conn
            .query_row(
                "SELECT group_id FROM account_groups WHERE company_id = ?1 AND name = 'Duties & Taxes'",
                params![company_id],
                |r| r.get(0),
            )
            .map_err(|_| AccountingError::NotFound("Predefined group 'Duties & Taxes' not found".to_string()))?;

        let indirect_exp_group_id: String = conn
            .query_row(
                "SELECT group_id FROM account_groups WHERE company_id = ?1 AND name = 'Indirect Expenses'",
                params![company_id],
                |r| r.get(0),
            )
            .map_err(|_| AccountingError::NotFound("Predefined group 'Indirect Expenses' not found".to_string()))?;

        let get_or_create = |c: &mut Connection, name: &str, group_id: &str| -> Result<String> {
            let existing: Option<String> = c
                .query_row(
                    "SELECT ledger_id FROM ledgers WHERE company_id = ?1 AND name = ?2",
                    params![company_id, name],
                    |r| r.get(0),
                )
                .ok();

            if let Some(id) = existing {
                Ok(id)
            } else {
                let l = Ledger::create(c, company_id, group_id, name, Paise(0), None, false)?;
                Ok(l.ledger_id)
            }
        };

        Ok(StandardGstLedgers {
            output_cgst_id: get_or_create(conn, "Output CGST", &duties_group_id)?,
            output_sgst_id: get_or_create(conn, "Output SGST", &duties_group_id)?,
            output_igst_id: get_or_create(conn, "Output IGST", &duties_group_id)?,
            input_cgst_id: get_or_create(conn, "Input CGST", &duties_group_id)?,
            input_sgst_id: get_or_create(conn, "Input SGST", &duties_group_id)?,
            input_igst_id: get_or_create(conn, "Input IGST", &duties_group_id)?,
            rcm_liability_cgst_id: get_or_create(conn, "RCM Liability CGST", &duties_group_id)?,
            rcm_liability_sgst_id: get_or_create(conn, "RCM Liability SGST", &duties_group_id)?,
            rcm_liability_igst_id: get_or_create(conn, "RCM Liability IGST", &duties_group_id)?,
            round_off_id: get_or_create(conn, "Round Off", &indirect_exp_group_id)?,
        })
    }

    /// Determines the GST invoice classification (B2B, B2CL, B2CS, NilRated).
    pub fn classify_sales_invoice(
        party_type: PartyRegistrationType,
        is_interstate: bool,
        invoice_total: Paise,
        is_nil_rated: bool,
    ) -> &'static str {
        if is_nil_rated {
            return "NilRated";
        }
        if party_type.is_registered() {
            "B2B"
        } else if is_interstate && invoice_total.0 > 25000000 {
            // Unregistered inter-state supply exceeding ₹2,50,000 threshold (in paise)
            "B2CL"
        } else {
            "B2CS"
        }
    }

    /// Posts a GST Sales Invoice with atomic transaction, ledger generation, and GST metadata tracking.
    pub fn post_sales_invoice(
        conn: &mut Connection,
        req: &GstSalesInvoiceRequest,
        user_id: Option<&str>,
    ) -> Result<GstInvoiceResult> {
        // 1. Fetch Company primary GST registration
        let comp_gst = CompanyGstRegistration::get_primary(conn, &req.company_id)?
            .ok_or_else(|| AccountingError::Validation("Company has no primary GST registration".to_string()))?;

        // 2. Validate Customer GSTIN if registered
        if req.party_type.is_registered() {
            let g = req.party_gstin.as_deref().ok_or_else(|| {
                AccountingError::Validation("GSTIN is required for registered party".to_string())
            })?;
            Gstin::new(g)?;
        }

        let is_sez = req.party_type == PartyRegistrationType::SEZ;

        // 3. Calculate tax summary across lines
        let tax_summary = TaxCalculator::calculate_invoice(
            &req.line_items,
            &comp_gst.state_code,
            &req.place_of_supply,
            is_sez,
            req.is_reverse_charge,
            req.enable_rounding,
        )?;

        // 4. Provision standard GST ledgers
        let gst_ledgers = Self::provision_standard_gst_ledgers(conn, &req.company_id)?;

        let is_nil_or_exempt = tax_summary.total_tax.0 == 0;
        let invoice_type = Self::classify_sales_invoice(
            req.party_type,
            tax_summary.is_interstate,
            tax_summary.final_invoice_total,
            is_nil_or_exempt,
        );

        // 5. Build Accounting Entries
        let mut entries = Vec::new();

        // Customer Dr (total receivable)
        entries.push(VoucherEntryDraft {
            ledger_id: req.customer_ledger_id.clone(),
            entry_type: EntryType::Dr,
            amount: tax_summary.final_invoice_total,
            narration: Some(format!("Bill to Customer {}", req.voucher_number)),
        });

        // Sales Cr (taxable base amount)
        entries.push(VoucherEntryDraft {
            ledger_id: req.sales_ledger_id.clone(),
            entry_type: EntryType::Cr,
            amount: tax_summary.total_taxable,
            narration: Some("Taxable Sales".to_string()),
        });

        // Taxes Cr (if not reverse charge)
        if !req.is_reverse_charge {
            if tax_summary.is_interstate {
                if tax_summary.total_igst.0 > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.output_igst_id,
                        entry_type: EntryType::Cr,
                        amount: tax_summary.total_igst,
                        narration: Some("Output IGST".to_string()),
                    });
                }
            } else {
                if tax_summary.total_cgst.0 > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.output_cgst_id,
                        entry_type: EntryType::Cr,
                        amount: tax_summary.total_cgst,
                        narration: Some("Output CGST".to_string()),
                    });
                }
                if tax_summary.total_sgst.0 > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.output_sgst_id,
                        entry_type: EntryType::Cr,
                        amount: tax_summary.total_sgst,
                        narration: Some("Output SGST".to_string()),
                    });
                }
            }
        }

        // Round Off entry
        if tax_summary.round_off_paise != 0 {
            if tax_summary.round_off_paise > 0 {
                // Total was rounded up -> Customer Dr increased -> Cr Round Off
                entries.push(VoucherEntryDraft {
                    ledger_id: gst_ledgers.round_off_id,
                    entry_type: EntryType::Cr,
                    amount: Paise(tax_summary.round_off_paise),
                    narration: Some("Round off adjustment".to_string()),
                });
            } else {
                // Total was rounded down -> Customer Dr decreased -> Dr Round Off
                entries.push(VoucherEntryDraft {
                    ledger_id: gst_ledgers.round_off_id,
                    entry_type: EntryType::Dr,
                    amount: Paise(-tax_summary.round_off_paise),
                    narration: Some("Round off adjustment".to_string()),
                });
            }
        }

        let draft = VoucherDraft {
            company_id: req.company_id.clone(),
            fy_id: req.fy_id.clone(),
            voucher_type_id: req.voucher_type_id.clone(),
            voucher_number: req.voucher_number.clone(),
            voucher_date: req.invoice_date,
            reference_no: req.reference_no.clone(),
            reference_date: Some(req.invoice_date),
            narration: req.narration.clone(),
            entries,
        };

        // 6. Post Voucher atomically
        let voucher = PostingEngine::post_voucher(conn, &draft, user_id)?;

        // 7. Insert GST metadata tables within the same database
        let gst_detail_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO voucher_gst_details (gst_detail_id, voucher_id, party_ledger_id, party_gstin, 
                                              place_of_supply, is_interstate, is_reverse_charge, invoice_type) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                gst_detail_id,
                voucher.voucher_id,
                req.customer_ledger_id,
                req.party_gstin,
                req.place_of_supply,
                if tax_summary.is_interstate { 1 } else { 0 },
                if req.is_reverse_charge { 1 } else { 0 },
                invoice_type,
            ],
        )?;

        for line in &tax_summary.lines {
            let tax_item_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO voucher_gst_tax_items (
                    tax_item_id, voucher_id, line_number, hsn_code, description, 
                    is_tax_inclusive, gross_amount_paise, taxable_amount_paise, 
                    gst_rate_basis_points, igst_rate_basis_points, cgst_rate_basis_points, sgst_rate_basis_points, 
                    igst_amount_paise, cgst_amount_paise, sgst_amount_paise, total_tax_paise, total_line_amount_paise, is_reverse_charge
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    tax_item_id,
                    voucher.voucher_id,
                    line.line_number as i32,
                    line.hsn_code,
                    line.description,
                    if line.is_tax_inclusive { 1 } else { 0 },
                    line.gross_amount.0,
                    line.taxable_amount.0,
                    line.gst_rate.basis_points() as i32,
                    line.igst_rate_bp as i32,
                    line.cgst_rate_bp as i32,
                    line.sgst_rate_bp as i32,
                    line.igst_amount.0,
                    line.cgst_amount.0,
                    line.sgst_amount.0,
                    line.total_tax.0,
                    line.line_total.0,
                    if line.is_reverse_charge { 1 } else { 0 },
                ],
            )?;
        }

        Ok(GstInvoiceResult {
            voucher,
            tax_summary,
            invoice_type: invoice_type.to_string(),
        })
    }

    /// Posts a GST Purchase Invoice with atomic transaction, input tax credit entries, and RCM support.
    pub fn post_purchase_invoice(
        conn: &mut Connection,
        req: &GstPurchaseInvoiceRequest,
        user_id: Option<&str>,
    ) -> Result<GstInvoiceResult> {
        let comp_gst = CompanyGstRegistration::get_primary(conn, &req.company_id)?
            .ok_or_else(|| AccountingError::Validation("Company has no primary GST registration".to_string()))?;

        if req.party_type.is_registered() {
            let g = req.supplier_gstin.as_deref().ok_or_else(|| {
                AccountingError::Validation("GSTIN is required for registered supplier".to_string())
            })?;
            Gstin::new(g)?;
        }

        let is_sez = req.party_type == PartyRegistrationType::SEZ;

        let tax_summary = TaxCalculator::calculate_invoice(
            &req.line_items,
            &comp_gst.state_code,
            &req.place_of_supply,
            is_sez,
            req.is_reverse_charge,
            req.enable_rounding,
        )?;

        let gst_ledgers = Self::provision_standard_gst_ledgers(conn, &req.company_id)?;

        let mut entries = Vec::new();

        // Expense / Purchase Dr
        entries.push(VoucherEntryDraft {
            ledger_id: req.expense_or_purchase_ledger_id.clone(),
            entry_type: EntryType::Dr,
            amount: tax_summary.total_taxable,
            narration: Some("Taxable Inward Supply".to_string()),
        });

        if req.is_reverse_charge {
            // Reverse Charge: Supplier is payable only for taxable amount
            entries.push(VoucherEntryDraft {
                ledger_id: req.supplier_ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: tax_summary.total_taxable,
                narration: Some("Payable to Supplier (Excl. RCM Tax)".to_string()),
            });

            // Tax entries: Eligible Input Tax Credit Dr and RCM Liability Cr
            if tax_summary.is_interstate {
                if tax_summary.total_igst.0 > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.input_igst_id,
                        entry_type: EntryType::Dr,
                        amount: tax_summary.total_igst,
                        narration: Some("Input IGST (RCM Credit)".to_string()),
                    });
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.rcm_liability_igst_id,
                        entry_type: EntryType::Cr,
                        amount: tax_summary.total_igst,
                        narration: Some("RCM Liability IGST".to_string()),
                    });
                }
            } else {
                if tax_summary.total_cgst.0 > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.input_cgst_id,
                        entry_type: EntryType::Dr,
                        amount: tax_summary.total_cgst,
                        narration: Some("Input CGST (RCM Credit)".to_string()),
                    });
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.rcm_liability_cgst_id,
                        entry_type: EntryType::Cr,
                        amount: tax_summary.total_cgst,
                        narration: Some("RCM Liability CGST".to_string()),
                    });
                }
                if tax_summary.total_sgst.0 > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.input_sgst_id,
                        entry_type: EntryType::Dr,
                        amount: tax_summary.total_sgst,
                        narration: Some("Input SGST (RCM Credit)".to_string()),
                    });
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.rcm_liability_sgst_id,
                        entry_type: EntryType::Cr,
                        amount: tax_summary.total_sgst,
                        narration: Some("RCM Liability SGST".to_string()),
                    });
                }
            }
        } else {
            // Forward Charge: Input tax is paid to supplier
            if tax_summary.is_interstate {
                if tax_summary.total_igst.0 > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.input_igst_id,
                        entry_type: EntryType::Dr,
                        amount: tax_summary.total_igst,
                        narration: Some("Input IGST".to_string()),
                    });
                }
            } else {
                if tax_summary.total_cgst.0 > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.input_cgst_id,
                        entry_type: EntryType::Dr,
                        amount: tax_summary.total_cgst,
                        narration: Some("Input CGST".to_string()),
                    });
                }
                if tax_summary.total_sgst.0 > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.input_sgst_id,
                        entry_type: EntryType::Dr,
                        amount: tax_summary.total_sgst,
                        narration: Some("Input SGST".to_string()),
                    });
                }
            }

            // Round Off entry
            if tax_summary.round_off_paise != 0 {
                if tax_summary.round_off_paise > 0 {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.round_off_id,
                        entry_type: EntryType::Dr,
                        amount: Paise(tax_summary.round_off_paise),
                        narration: Some("Round off adjustment".to_string()),
                    });
                } else {
                    entries.push(VoucherEntryDraft {
                        ledger_id: gst_ledgers.round_off_id,
                        entry_type: EntryType::Cr,
                        amount: Paise(-tax_summary.round_off_paise),
                        narration: Some("Round off adjustment".to_string()),
                    });
                }
            }

            // Supplier Cr (Total invoice amount)
            entries.push(VoucherEntryDraft {
                ledger_id: req.supplier_ledger_id.clone(),
                entry_type: EntryType::Cr,
                amount: tax_summary.final_invoice_total,
                narration: Some(format!("Bill from Supplier {}", req.voucher_number)),
            });
        }

        let draft = VoucherDraft {
            company_id: req.company_id.clone(),
            fy_id: req.fy_id.clone(),
            voucher_type_id: req.voucher_type_id.clone(),
            voucher_number: req.voucher_number.clone(),
            voucher_date: req.invoice_date,
            reference_no: req.reference_no.clone(),
            reference_date: Some(req.invoice_date),
            narration: req.narration.clone(),
            entries,
        };

        let voucher = PostingEngine::post_voucher(conn, &draft, user_id)?;

        let gst_detail_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO voucher_gst_details (gst_detail_id, voucher_id, party_ledger_id, party_gstin, 
                                              place_of_supply, is_interstate, is_reverse_charge, invoice_type) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'B2B')",
            params![
                gst_detail_id,
                voucher.voucher_id,
                req.supplier_ledger_id,
                req.supplier_gstin,
                req.place_of_supply,
                if tax_summary.is_interstate { 1 } else { 0 },
                if req.is_reverse_charge { 1 } else { 0 },
            ],
        )?;

        for line in &tax_summary.lines {
            let tax_item_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO voucher_gst_tax_items (
                    tax_item_id, voucher_id, line_number, hsn_code, description, 
                    is_tax_inclusive, gross_amount_paise, taxable_amount_paise, 
                    gst_rate_basis_points, igst_rate_basis_points, cgst_rate_basis_points, sgst_rate_basis_points, 
                    igst_amount_paise, cgst_amount_paise, sgst_amount_paise, total_tax_paise, total_line_amount_paise, is_reverse_charge
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    tax_item_id,
                    voucher.voucher_id,
                    line.line_number as i32,
                    line.hsn_code,
                    line.description,
                    if line.is_tax_inclusive { 1 } else { 0 },
                    line.gross_amount.0,
                    line.taxable_amount.0,
                    line.gst_rate.basis_points() as i32,
                    line.igst_rate_bp as i32,
                    line.cgst_rate_bp as i32,
                    line.sgst_rate_bp as i32,
                    line.igst_amount.0,
                    line.cgst_amount.0,
                    line.sgst_amount.0,
                    line.total_tax.0,
                    line.line_total.0,
                    if line.is_reverse_charge { 1 } else { 0 },
                ],
            )?;
        }

        Ok(GstInvoiceResult {
            voucher,
            tax_summary,
            invoice_type: "Purchase".to_string(),
        })
    }

    /// Posts a Credit Note (Sales Return) reversing sales and output taxes.
    pub fn post_credit_note(
        conn: &mut Connection,
        req: &GstCreditNoteRequest,
        user_id: Option<&str>,
    ) -> Result<GstInvoiceResult> {
        let comp_gst = CompanyGstRegistration::get_primary(conn, &req.company_id)?
            .ok_or_else(|| AccountingError::Validation("Company has no primary GST registration".to_string()))?;

        let is_sez = req.party_type == PartyRegistrationType::SEZ;

        let tax_summary = TaxCalculator::calculate_invoice(
            &req.line_items,
            &comp_gst.state_code,
            &req.place_of_supply,
            is_sez,
            false,
            false,
        )?;

        let gst_ledgers = Self::provision_standard_gst_ledgers(conn, &req.company_id)?;

        let mut entries = Vec::new();

        // Sales Dr (Reversal)
        entries.push(VoucherEntryDraft {
            ledger_id: req.sales_ledger_id.clone(),
            entry_type: EntryType::Dr,
            amount: tax_summary.total_taxable,
            narration: Some(format!("Sales Return agst Inv {}", req.original_invoice_no)),
        });

        // Output Tax Dr (Reversal)
        if tax_summary.is_interstate {
            if tax_summary.total_igst.0 > 0 {
                entries.push(VoucherEntryDraft {
                    ledger_id: gst_ledgers.output_igst_id,
                    entry_type: EntryType::Dr,
                    amount: tax_summary.total_igst,
                    narration: Some("Output IGST Reversal".to_string()),
                });
            }
        } else {
            if tax_summary.total_cgst.0 > 0 {
                entries.push(VoucherEntryDraft {
                    ledger_id: gst_ledgers.output_cgst_id,
                    entry_type: EntryType::Dr,
                    amount: tax_summary.total_cgst,
                    narration: Some("Output CGST Reversal".to_string()),
                });
            }
            if tax_summary.total_sgst.0 > 0 {
                entries.push(VoucherEntryDraft {
                    ledger_id: gst_ledgers.output_sgst_id,
                    entry_type: EntryType::Dr,
                    amount: tax_summary.total_sgst,
                    narration: Some("Output SGST Reversal".to_string()),
                });
            }
        }

        // Customer Cr (Credit to customer account)
        entries.push(VoucherEntryDraft {
            ledger_id: req.customer_ledger_id.clone(),
            entry_type: EntryType::Cr,
            amount: tax_summary.final_invoice_total,
            narration: Some(format!("Credit Note {}", req.voucher_number)),
        });

        let draft = VoucherDraft {
            company_id: req.company_id.clone(),
            fy_id: req.fy_id.clone(),
            voucher_type_id: req.voucher_type_id.clone(),
            voucher_number: req.voucher_number.clone(),
            voucher_date: req.note_date,
            reference_no: Some(req.original_invoice_no.clone()),
            reference_date: Some(req.original_invoice_date),
            narration: req.narration.clone(),
            entries,
        };

        let voucher = PostingEngine::post_voucher(conn, &draft, user_id)?;

        let gst_detail_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO voucher_gst_details (gst_detail_id, voucher_id, party_ledger_id, party_gstin, 
                                              place_of_supply, is_interstate, is_reverse_charge, invoice_type,
                                              original_voucher_id, original_invoice_no, original_invoice_date) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'CDNR', ?8, ?9, ?10)",
            params![
                gst_detail_id,
                voucher.voucher_id,
                req.customer_ledger_id,
                req.party_gstin,
                req.place_of_supply,
                if tax_summary.is_interstate { 1 } else { 0 },
                0,
                req.original_voucher_id,
                req.original_invoice_no,
                req.original_invoice_date.format("%Y-%m-%d").to_string(),
            ],
        )?;

        for line in &tax_summary.lines {
            let tax_item_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO voucher_gst_tax_items (
                    tax_item_id, voucher_id, line_number, hsn_code, description, 
                    is_tax_inclusive, gross_amount_paise, taxable_amount_paise, 
                    gst_rate_basis_points, igst_rate_basis_points, cgst_rate_basis_points, sgst_rate_basis_points, 
                    igst_amount_paise, cgst_amount_paise, sgst_amount_paise, total_tax_paise, total_line_amount_paise, is_reverse_charge
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, 0)",
                params![
                    tax_item_id,
                    voucher.voucher_id,
                    line.line_number as i32,
                    line.hsn_code,
                    line.description,
                    0,
                    line.gross_amount.0,
                    line.taxable_amount.0,
                    line.gst_rate.basis_points() as i32,
                    line.igst_rate_bp as i32,
                    line.cgst_rate_bp as i32,
                    line.sgst_rate_bp as i32,
                    line.igst_amount.0,
                    line.cgst_amount.0,
                    line.sgst_amount.0,
                    line.total_tax.0,
                    line.line_total.0,
                ],
            )?;
        }

        Ok(GstInvoiceResult {
            voucher,
            tax_summary,
            invoice_type: "CDNR".to_string(),
        })
    }
}
