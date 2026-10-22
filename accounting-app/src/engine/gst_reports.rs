use crate::domain::gst::CompanyGstRegistration;
use crate::domain::money::Paise;
use crate::error::{AccountingError, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

pub const GST_PRELIMINARY_DISCLAIMER: &str =
    "PRELIMINARY REPORT - PENDING OFFICIAL GSTN VALIDATION";

// ============================================================================
// GSTR-1 REPORT MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1B2bItem {
    pub customer_gstin: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub invoice_value: Paise,
    pub place_of_supply: String,
    pub is_reverse_charge: bool,
    pub rate_basis_points: u32,
    pub taxable_value: Paise,
    pub igst_amount: Paise,
    pub cgst_amount: Paise,
    pub sgst_amount: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1B2clItem {
    pub invoice_number: String,
    pub invoice_date: String,
    pub invoice_value: Paise,
    pub place_of_supply: String,
    pub rate_basis_points: u32,
    pub taxable_value: Paise,
    pub igst_amount: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1B2csItem {
    pub place_of_supply: String,
    pub rate_basis_points: u32,
    pub taxable_value: Paise,
    pub igst_amount: Paise,
    pub cgst_amount: Paise,
    pub sgst_amount: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1CdnrItem {
    pub customer_gstin: String,
    pub note_number: String,
    pub note_date: String,
    pub original_invoice_no: String,
    pub original_invoice_date: String,
    pub place_of_supply: String,
    pub rate_basis_points: u32,
    pub taxable_value: Paise,
    pub igst_amount: Paise,
    pub cgst_amount: Paise,
    pub sgst_amount: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1HsnItem {
    pub hsn_code: String,
    pub description: String,
    pub taxable_value: Paise,
    pub igst_amount: Paise,
    pub cgst_amount: Paise,
    pub sgst_amount: Paise,
    pub total_tax_amount: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1Report {
    pub disclaimer: String,
    pub company_id: String,
    pub company_gstin: String,
    pub from_date: String,
    pub to_date: String,
    pub b2b_supplies: Vec<Gstr1B2bItem>,
    pub b2cl_supplies: Vec<Gstr1B2clItem>,
    pub b2cs_supplies: Vec<Gstr1B2csItem>,
    pub credit_notes: Vec<Gstr1CdnrItem>,
    pub nil_exempt_supplies: Vec<Gstr1B2csItem>,
    pub hsn_summary: Vec<Gstr1HsnItem>,
    pub total_taxable: Paise,
    pub total_igst: Paise,
    pub total_cgst: Paise,
    pub total_sgst: Paise,
}

// ============================================================================
// GSTR-3B REPORT MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr3bSupplySection {
    pub taxable_value: Paise,
    pub igst_amount: Paise,
    pub cgst_amount: Paise,
    pub sgst_amount: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr3bItcSection {
    pub igst_amount: Paise,
    pub cgst_amount: Paise,
    pub sgst_amount: Paise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr3bReport {
    pub disclaimer: String,
    pub company_id: String,
    pub company_gstin: String,
    pub from_date: String,
    pub to_date: String,

    // Table 3.1 Outward supplies and inward supplies liable to reverse charge
    pub outward_taxable_supplies: Gstr3bSupplySection,    // 3.1(a)
    pub outward_zero_rated_supplies: Gstr3bSupplySection, // 3.1(b)
    pub other_outward_supplies_nil_exempt: Paise,         // 3.1(c)
    pub inward_supplies_reverse_charge: Gstr3bSupplySection, // 3.1(d)

    // Table 4 Eligible Input Tax Credit (ITC)
    pub itc_rcm_inward: Gstr3bItcSection, // 4(A)(3)
    pub itc_all_other: Gstr3bItcSection,  // 4(A)(5)
    pub net_itc_available: Gstr3bItcSection, // 4(C)
}

pub struct GstReportEngine;

impl GstReportEngine {
    /// Generates the GSTR-1 preliminary report for a given date range.
    pub fn generate_gstr1(
        conn: &Connection,
        company_id: &str,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<Gstr1Report> {
        let comp_gst = CompanyGstRegistration::get_primary(conn, company_id)?
            .ok_or_else(|| AccountingError::Validation("Company has no primary GST registration".to_string()))?;

        let from_str = from_date.format("%Y-%m-%d").to_string();
        let to_str = to_date.format("%Y-%m-%d").to_string();

        let mut b2b_supplies = Vec::new();
        let mut b2cl_supplies = Vec::new();
        let mut b2cs_supplies = Vec::new();
        let mut credit_notes = Vec::new();
        let mut nil_exempt_supplies = Vec::new();

        let mut total_taxable_paise = 0i64;
        let mut total_igst_paise = 0i64;
        let mut total_cgst_paise = 0i64;
        let mut total_sgst_paise = 0i64;

        // Query all posted vouchers with GST details in date range
        let mut stmt = conn.prepare(
            "SELECT 
                v.voucher_id, v.voucher_number, v.voucher_date, v.total_amount_paise,
                gd.party_gstin, gd.place_of_supply, gd.is_interstate, gd.is_reverse_charge, gd.invoice_type,
                gd.original_invoice_no, gd.original_invoice_date,
                ti.line_number, ti.hsn_code, ti.description, ti.taxable_amount_paise, ti.gst_rate_basis_points,
                ti.igst_amount_paise, ti.cgst_amount_paise, ti.sgst_amount_paise, ti.total_tax_paise
             FROM vouchers v
             JOIN voucher_gst_details gd ON v.voucher_id = gd.voucher_id
             JOIN voucher_gst_tax_items ti ON v.voucher_id = ti.voucher_id
             WHERE v.company_id = ?1 AND v.status = 'Posted'
               AND v.voucher_date BETWEEN ?2 AND ?3
             ORDER BY v.voucher_date, v.voucher_number, ti.line_number",
        )?;

        let rows = stmt.query_map(params![company_id, from_str, to_str], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, i64>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, String>(13)?,
                r.get::<_, i64>(14)?,
                r.get::<_, i64>(15)?,
                r.get::<_, i64>(16)?,
                r.get::<_, i64>(17)?,
                r.get::<_, i64>(18)?,
                r.get::<_, i64>(19)?,
            ))
        })?;

        for r_res in rows {
            let (
                _vid,
                v_num,
                v_date,
                v_total,
                p_gstin,
                pos,
                _is_inter,
                is_rcm,
                inv_type,
                orig_inv_no,
                orig_inv_dt,
                _line_num,
                _hsn,
                _desc,
                taxable,
                rate_bp,
                igst,
                cgst,
                sgst,
                _tot_tax,
            ) = r_res?;

            // Skip purchase records in GSTR-1
            if inv_type == "Purchase" {
                continue;
            }

            if inv_type == "CDNR" {
                credit_notes.push(Gstr1CdnrItem {
                    customer_gstin: p_gstin.unwrap_or_default(),
                    note_number: v_num,
                    note_date: v_date,
                    original_invoice_no: orig_inv_no.unwrap_or_default(),
                    original_invoice_date: orig_inv_dt.unwrap_or_default(),
                    place_of_supply: pos,
                    rate_basis_points: rate_bp as u32,
                    taxable_value: Paise(taxable),
                    igst_amount: Paise(igst),
                    cgst_amount: Paise(cgst),
                    sgst_amount: Paise(sgst),
                });
            } else if inv_type == "NilRated" || rate_bp == 0 {
                nil_exempt_supplies.push(Gstr1B2csItem {
                    place_of_supply: pos,
                    rate_basis_points: 0,
                    taxable_value: Paise(taxable),
                    igst_amount: Paise(0),
                    cgst_amount: Paise(0),
                    sgst_amount: Paise(0),
                });
                total_taxable_paise += taxable;
            } else if inv_type == "B2B" {
                b2b_supplies.push(Gstr1B2bItem {
                    customer_gstin: p_gstin.unwrap_or_default(),
                    invoice_number: v_num,
                    invoice_date: v_date,
                    invoice_value: Paise(v_total),
                    place_of_supply: pos,
                    is_reverse_charge: is_rcm == 1,
                    rate_basis_points: rate_bp as u32,
                    taxable_value: Paise(taxable),
                    igst_amount: Paise(igst),
                    cgst_amount: Paise(cgst),
                    sgst_amount: Paise(sgst),
                });
                total_taxable_paise += taxable;
                total_igst_paise += igst;
                total_cgst_paise += cgst;
                total_sgst_paise += sgst;
            } else if inv_type == "B2CL" {
                b2cl_supplies.push(Gstr1B2clItem {
                    invoice_number: v_num,
                    invoice_date: v_date,
                    invoice_value: Paise(v_total),
                    place_of_supply: pos,
                    rate_basis_points: rate_bp as u32,
                    taxable_value: Paise(taxable),
                    igst_amount: Paise(igst),
                });
                total_taxable_paise += taxable;
                total_igst_paise += igst;
            } else {
                // B2CS
                b2cs_supplies.push(Gstr1B2csItem {
                    place_of_supply: pos,
                    rate_basis_points: rate_bp as u32,
                    taxable_value: Paise(taxable),
                    igst_amount: Paise(igst),
                    cgst_amount: Paise(cgst),
                    sgst_amount: Paise(sgst),
                });
                total_taxable_paise += taxable;
                total_igst_paise += igst;
                total_cgst_paise += cgst;
                total_sgst_paise += sgst;
            }
        }

        // Table 12: HSN Summary aggregation
        let mut hsn_stmt = conn.prepare(
            "SELECT 
                ti.hsn_code,
                ti.description,
                SUM(ti.taxable_amount_paise) AS tot_taxable,
                SUM(ti.igst_amount_paise) AS tot_igst,
                SUM(ti.cgst_amount_paise) AS tot_cgst,
                SUM(ti.sgst_amount_paise) AS tot_sgst,
                SUM(ti.total_tax_paise) AS tot_tax
             FROM voucher_gst_tax_items ti
             JOIN vouchers v ON ti.voucher_id = v.voucher_id
             JOIN voucher_gst_details gd ON v.voucher_id = gd.voucher_id
             WHERE v.company_id = ?1 AND v.status = 'Posted'
               AND v.voucher_date BETWEEN ?2 AND ?3
               AND gd.invoice_type != 'Purchase'
             GROUP BY ti.hsn_code, ti.description
             ORDER BY ti.hsn_code",
        )?;

        let hsn_rows = hsn_stmt.query_map(params![company_id, from_str, to_str], |r| {
            let hsn: Option<String> = r.get(0)?;
            Ok(Gstr1HsnItem {
                hsn_code: hsn.unwrap_or_else(|| "N/A".to_string()),
                description: r.get(1)?,
                taxable_value: Paise(r.get(2)?),
                igst_amount: Paise(r.get(3)?),
                cgst_amount: Paise(r.get(4)?),
                sgst_amount: Paise(r.get(5)?),
                total_tax_amount: Paise(r.get(6)?),
            })
        })?;

        let mut hsn_summary = Vec::new();
        for item in hsn_rows {
            hsn_summary.push(item?);
        }

        Ok(Gstr1Report {
            disclaimer: GST_PRELIMINARY_DISCLAIMER.to_string(),
            company_id: company_id.to_string(),
            company_gstin: comp_gst.gstin.to_string(),
            from_date: from_str,
            to_date: to_str,
            b2b_supplies,
            b2cl_supplies,
            b2cs_supplies,
            credit_notes,
            nil_exempt_supplies,
            hsn_summary,
            total_taxable: Paise(total_taxable_paise),
            total_igst: Paise(total_igst_paise),
            total_cgst: Paise(total_cgst_paise),
            total_sgst: Paise(total_sgst_paise),
        })
    }

    /// Generates the GSTR-3B preliminary summary report for a given date range.
    pub fn generate_gstr3b(
        conn: &Connection,
        company_id: &str,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<Gstr3bReport> {
        let comp_gst = CompanyGstRegistration::get_primary(conn, company_id)?
            .ok_or_else(|| AccountingError::Validation("Company has no primary GST registration".to_string()))?;

        let from_str = from_date.format("%Y-%m-%d").to_string();
        let to_str = to_date.format("%Y-%m-%d").to_string();

        // 1. Table 3.1(a): Outward Taxable Supplies (net of credit notes as per CBIC guidelines)
        let (out_taxable, out_igst, out_cgst, out_sgst): (i64, i64, i64, i64) = conn.query_row(
            "SELECT 
                COALESCE(SUM(CASE WHEN vt.base_type = 'CreditNote' THEN -ti.taxable_amount_paise ELSE ti.taxable_amount_paise END), 0),
                COALESCE(SUM(CASE WHEN vt.base_type = 'CreditNote' THEN -ti.igst_amount_paise ELSE ti.igst_amount_paise END), 0),
                COALESCE(SUM(CASE WHEN vt.base_type = 'CreditNote' THEN -ti.cgst_amount_paise ELSE ti.cgst_amount_paise END), 0),
                COALESCE(SUM(CASE WHEN vt.base_type = 'CreditNote' THEN -ti.sgst_amount_paise ELSE ti.sgst_amount_paise END), 0)
             FROM voucher_gst_tax_items ti
             JOIN vouchers v ON ti.voucher_id = v.voucher_id
             JOIN voucher_gst_details gd ON v.voucher_id = gd.voucher_id
             JOIN voucher_types vt ON v.voucher_type_id = vt.voucher_type_id
             WHERE v.company_id = ?1 AND v.status = 'Posted'
               AND v.voucher_date BETWEEN ?2 AND ?3
               AND vt.base_type IN ('Sales', 'CreditNote')
               AND ti.gst_rate_basis_points > 0",
            params![company_id, from_str, to_str],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;

        // 2. Table 3.1(c): Other Outward Supplies (Nil/Exempt)
        let out_exempt: i64 = conn.query_row(
            "SELECT COALESCE(SUM(ti.taxable_amount_paise), 0)
             FROM voucher_gst_tax_items ti
             JOIN vouchers v ON ti.voucher_id = v.voucher_id
             JOIN voucher_gst_details gd ON v.voucher_id = gd.voucher_id
             JOIN voucher_types vt ON v.voucher_type_id = vt.voucher_type_id
             WHERE v.company_id = ?1 AND v.status = 'Posted'
               AND v.voucher_date BETWEEN ?2 AND ?3
               AND vt.base_type = 'Sales'
               AND ti.gst_rate_basis_points = 0",
            params![company_id, from_str, to_str],
            |r| r.get(0),
        )?;

        // 3. Table 3.1(d): Inward supplies liable to reverse charge (Liability)
        let (rcm_taxable, rcm_igst, rcm_cgst, rcm_sgst): (i64, i64, i64, i64) = conn.query_row(
            "SELECT 
                COALESCE(SUM(ti.taxable_amount_paise), 0),
                COALESCE(SUM(ti.igst_amount_paise), 0),
                COALESCE(SUM(ti.cgst_amount_paise), 0),
                COALESCE(SUM(ti.sgst_amount_paise), 0)
             FROM voucher_gst_tax_items ti
             JOIN vouchers v ON ti.voucher_id = v.voucher_id
             JOIN voucher_gst_details gd ON v.voucher_id = gd.voucher_id
             WHERE v.company_id = ?1 AND v.status = 'Posted'
               AND v.voucher_date BETWEEN ?2 AND ?3
               AND gd.is_reverse_charge = 1",
            params![company_id, from_str, to_str],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;

        // 4. Table 4(A)(5): All other ITC (Regular Purchases)
        let (itc_igst, itc_cgst, itc_sgst): (i64, i64, i64) = conn.query_row(
            "SELECT 
                COALESCE(SUM(ti.igst_amount_paise), 0),
                COALESCE(SUM(ti.cgst_amount_paise), 0),
                COALESCE(SUM(ti.sgst_amount_paise), 0)
             FROM voucher_gst_tax_items ti
             JOIN vouchers v ON ti.voucher_id = v.voucher_id
             JOIN voucher_gst_details gd ON v.voucher_id = gd.voucher_id
             WHERE v.company_id = ?1 AND v.status = 'Posted'
               AND v.voucher_date BETWEEN ?2 AND ?3
               AND gd.invoice_type = 'Purchase'
               AND gd.is_reverse_charge = 0",
            params![company_id, from_str, to_str],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        let itc_rcm = Gstr3bItcSection {
            igst_amount: Paise(rcm_igst),
            cgst_amount: Paise(rcm_cgst),
            sgst_amount: Paise(rcm_sgst),
        };

        let itc_other = Gstr3bItcSection {
            igst_amount: Paise(itc_igst),
            cgst_amount: Paise(itc_cgst),
            sgst_amount: Paise(itc_sgst),
        };

        let net_itc = Gstr3bItcSection {
            igst_amount: Paise(rcm_igst + itc_igst),
            cgst_amount: Paise(rcm_cgst + itc_cgst),
            sgst_amount: Paise(rcm_sgst + itc_sgst),
        };

        Ok(Gstr3bReport {
            disclaimer: GST_PRELIMINARY_DISCLAIMER.to_string(),
            company_id: company_id.to_string(),
            company_gstin: comp_gst.gstin.to_string(),
            from_date: from_str,
            to_date: to_str,
            outward_taxable_supplies: Gstr3bSupplySection {
                taxable_value: Paise(out_taxable),
                igst_amount: Paise(out_igst),
                cgst_amount: Paise(out_cgst),
                sgst_amount: Paise(out_sgst),
            },
            outward_zero_rated_supplies: Gstr3bSupplySection {
                taxable_value: Paise(0),
                igst_amount: Paise(0),
                cgst_amount: Paise(0),
                sgst_amount: Paise(0),
            },
            other_outward_supplies_nil_exempt: Paise(out_exempt),
            inward_supplies_reverse_charge: Gstr3bSupplySection {
                taxable_value: Paise(rcm_taxable),
                igst_amount: Paise(rcm_igst),
                cgst_amount: Paise(rcm_cgst),
                sgst_amount: Paise(rcm_sgst),
            },
            itc_rcm_inward: itc_rcm,
            itc_all_other: itc_other,
            net_itc_available: net_itc,
        })
    }
}

// Format and Export Methods
impl Gstr1Report {
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AccountingError::Validation(e.to_string()))
    }

    pub fn to_b2b_csv(&self) -> String {
        let mut csv = String::new();
        csv.push_str(&format!("# {}\n", self.disclaimer));
        csv.push_str("GSTIN/UIN of Recipient,Invoice Number,Invoice date,Invoice Value,Place Of Supply,Reverse Charge,Rate,Taxable Value,Integrated Tax,Central Tax,State/UT Tax\n");
        for b in &self.b2b_supplies {
            csv.push_str(&format!(
                "{},{},{},{:.2},{},{},{:.2},{:.2},{:.2},{:.2},{:.2}\n",
                b.customer_gstin,
                b.invoice_number,
                b.invoice_date,
                b.invoice_value.to_decimal_rupees(),
                b.place_of_supply,
                if b.is_reverse_charge { "Y" } else { "N" },
                b.rate_basis_points as f64 / 100.0,
                b.taxable_value.to_decimal_rupees(),
                b.igst_amount.to_decimal_rupees(),
                b.cgst_amount.to_decimal_rupees(),
                b.sgst_amount.to_decimal_rupees(),
            ));
        }
        csv
    }
}

impl Gstr3bReport {
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AccountingError::Validation(e.to_string()))
    }

    pub fn to_summary_csv(&self) -> String {
        let mut csv = String::new();
        csv.push_str(&format!("# {}\n", self.disclaimer));
        csv.push_str("Nature of Supplies,Total Taxable value,Integrated Tax,Central Tax,State/UT Tax\n");
        csv.push_str(&format!(
            "(a) Outward taxable supplies (other than zero rated, nil rated and exempted),{:.2},{:.2},{:.2},{:.2}\n",
            self.outward_taxable_supplies.taxable_value.to_decimal_rupees(),
            self.outward_taxable_supplies.igst_amount.to_decimal_rupees(),
            self.outward_taxable_supplies.cgst_amount.to_decimal_rupees(),
            self.outward_taxable_supplies.sgst_amount.to_decimal_rupees(),
        ));
        csv.push_str(&format!(
            "(c) Other outward supplies (Nil rated, exempted),{:.2},0.00,0.00,0.00\n",
            self.other_outward_supplies_nil_exempt.to_decimal_rupees(),
        ));
        csv.push_str(&format!(
            "(d) Inward supplies (liable to reverse charge),{:.2},{:.2},{:.2},{:.2}\n",
            self.inward_supplies_reverse_charge.taxable_value.to_decimal_rupees(),
            self.inward_supplies_reverse_charge.igst_amount.to_decimal_rupees(),
            self.inward_supplies_reverse_charge.cgst_amount.to_decimal_rupees(),
            self.inward_supplies_reverse_charge.sgst_amount.to_decimal_rupees(),
        ));
        csv.push_str("\nEligible ITC,Integrated Tax,Central Tax,State/UT Tax\n");
        csv.push_str(&format!(
            "(A)(3) Inward supplies liable to reverse charge,{:.2},{:.2},{:.2}\n",
            self.itc_rcm_inward.igst_amount.to_decimal_rupees(),
            self.itc_rcm_inward.cgst_amount.to_decimal_rupees(),
            self.itc_rcm_inward.sgst_amount.to_decimal_rupees(),
        ));
        csv.push_str(&format!(
            "(A)(5) All other ITC,{:.2},{:.2},{:.2}\n",
            self.itc_all_other.igst_amount.to_decimal_rupees(),
            self.itc_all_other.cgst_amount.to_decimal_rupees(),
            self.itc_all_other.sgst_amount.to_decimal_rupees(),
        ));
        csv.push_str(&format!(
            "(C) Net ITC Available,{:.2},{:.2},{:.2}\n",
            self.net_itc_available.igst_amount.to_decimal_rupees(),
            self.net_itc_available.cgst_amount.to_decimal_rupees(),
            self.net_itc_available.sgst_amount.to_decimal_rupees(),
        ));
        csv
    }
}
