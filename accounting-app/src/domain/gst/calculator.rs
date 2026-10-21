use super::rates::GstRate;
use crate::domain::money::Paise;
use crate::error::{AccountingError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxLineInput {
    pub line_number: u32,
    pub hsn_code: Option<String>,
    pub description: String,
    pub amount_paise: Paise,
    pub is_tax_inclusive: bool,
    pub rate: GstRate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatedTaxLine {
    pub line_number: u32,
    pub hsn_code: Option<String>,
    pub description: String,
    pub is_tax_inclusive: bool,
    pub gross_amount: Paise,
    pub taxable_amount: Paise,
    pub gst_rate: GstRate,
    pub igst_rate_bp: u32,
    pub cgst_rate_bp: u32,
    pub sgst_rate_bp: u32,
    pub igst_amount: Paise,
    pub cgst_amount: Paise,
    pub sgst_amount: Paise,
    pub total_tax: Paise,
    pub line_total: Paise,
    pub is_reverse_charge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceTaxSummary {
    pub lines: Vec<CalculatedTaxLine>,
    pub total_taxable: Paise,
    pub total_cgst: Paise,
    pub total_sgst: Paise,
    pub total_igst: Paise,
    pub total_tax: Paise,
    pub total_before_rounding: Paise,
    pub round_off_paise: i64, // can be negative (e.g. -40 paise) or positive (e.g. +30 paise)
    pub final_invoice_total: Paise,
    pub is_interstate: bool,
    pub is_reverse_charge: bool,
}

pub struct TaxCalculator;

impl TaxCalculator {
    /// Determines whether supply is inter-state based on supplier state and place of supply.
    pub fn is_interstate(supplier_state: &str, place_of_supply: &str, is_sez: bool) -> bool {
        if is_sez {
            return true; // Supplies to/from SEZ are deemed inter-state under Section 7(5)(b) of IGST Act
        }
        supplier_state.trim() != place_of_supply.trim()
    }

    /// Calculates a single tax line given pricing mode and inter-state status.
    pub fn calculate_line(
        input: &TaxLineInput,
        is_interstate: bool,
        is_reverse_charge: bool,
    ) -> Result<CalculatedTaxLine> {
        let (taxable_paise, total_tax_paise, gross_paise) = if input.is_tax_inclusive {
            // Inclusive pricing:
            // input.amount_paise is gross inclusive amount G
            let g = input.amount_paise.0;
            let r = input.rate.basis_points() as i64;
            if r == 0 {
                (input.amount_paise, Paise(0), input.amount_paise)
            } else {
                // Taxable = round(G * 10000 / (10000 + R))
                let denom = 10000 + r;
                let numer = g
                    .checked_mul(10000)
                    .ok_or_else(|| AccountingError::MathOverflow("Multiplication overflow in tax calculation".to_string()))?;
                let taxable = (numer + denom / 2) / denom;
                let tax = g - taxable;
                (Paise(taxable), Paise(tax), input.amount_paise)
            }
        } else {
            // Exclusive pricing:
            // input.amount_paise is base taxable amount B
            let b = input.amount_paise.0;
            let r = input.rate.basis_points() as i64;
            if r == 0 {
                (input.amount_paise, Paise(0), input.amount_paise)
            } else {
                // Tax = round(B * R / 10000)
                let prod = b
                    .checked_mul(r)
                    .ok_or_else(|| AccountingError::MathOverflow("Multiplication overflow in tax calculation".to_string()))?;
                let tax = (prod + 5000) / 10000;
                let gross = b
                    .checked_add(tax)
                    .ok_or_else(|| AccountingError::MathOverflow("Addition overflow in tax calculation".to_string()))?;
                (input.amount_paise, Paise(tax), Paise(gross))
            }
        };

        let (igst_rate_bp, cgst_rate_bp, sgst_rate_bp, igst_amount, cgst_amount, sgst_amount) =
            if is_interstate {
                (
                    input.rate.basis_points(),
                    0,
                    0,
                    total_tax_paise,
                    Paise(0),
                    Paise(0),
                )
            } else {
                let (cgst_bp, sgst_bp) = input.rate.split_intra_rates();
                let half_tax = total_tax_paise.0 / 2;
                let rem_tax = total_tax_paise.0 - half_tax; // guarantees half_tax + rem_tax == total_tax
                (0, cgst_bp, sgst_bp, Paise(0), Paise(half_tax), Paise(rem_tax))
            };

        Ok(CalculatedTaxLine {
            line_number: input.line_number,
            hsn_code: input.hsn_code.clone(),
            description: input.description.clone(),
            is_tax_inclusive: input.is_tax_inclusive,
            gross_amount: gross_paise,
            taxable_amount: taxable_paise,
            gst_rate: input.rate,
            igst_rate_bp,
            cgst_rate_bp,
            sgst_rate_bp,
            igst_amount,
            cgst_amount,
            sgst_amount,
            total_tax: total_tax_paise,
            line_total: gross_paise,
            is_reverse_charge,
        })
    }

    /// Computes full invoice tax summary across multiple line items and performs invoice-level rounding.
    pub fn calculate_invoice(
        inputs: &[TaxLineInput],
        supplier_state: &str,
        place_of_supply: &str,
        is_sez: bool,
        is_reverse_charge: bool,
        enable_rupee_rounding: bool,
    ) -> Result<InvoiceTaxSummary> {
        if inputs.is_empty() {
            return Err(AccountingError::Validation(
                "Invoice must contain at least one line item".to_string(),
            ));
        }

        let is_interstate = Self::is_interstate(supplier_state, place_of_supply, is_sez);
        let mut calculated_lines = Vec::with_capacity(inputs.len());

        let mut sum_taxable = 0i64;
        let mut sum_cgst = 0i64;
        let mut sum_sgst = 0i64;
        let mut sum_igst = 0i64;
        let mut sum_tax = 0i64;

        for line_in in inputs {
            let line = Self::calculate_line(line_in, is_interstate, is_reverse_charge)?;
            sum_taxable = sum_taxable.checked_add(line.taxable_amount.0).ok_or_else(|| {
                AccountingError::MathOverflow("Taxable sum overflow".to_string())
            })?;
            sum_cgst = sum_cgst.checked_add(line.cgst_amount.0).ok_or_else(|| {
                AccountingError::MathOverflow("CGST sum overflow".to_string())
            })?;
            sum_sgst = sum_sgst.checked_add(line.sgst_amount.0).ok_or_else(|| {
                AccountingError::MathOverflow("SGST sum overflow".to_string())
            })?;
            sum_igst = sum_igst.checked_add(line.igst_amount.0).ok_or_else(|| {
                AccountingError::MathOverflow("IGST sum overflow".to_string())
            })?;
            sum_tax = sum_tax.checked_add(line.total_tax.0).ok_or_else(|| {
                AccountingError::MathOverflow("Tax sum overflow".to_string())
            })?;
            calculated_lines.push(line);
        }

        let before_round = if is_reverse_charge {
            // Under RCM, tax is not added to invoice total payable by customer
            sum_taxable
        } else {
            sum_taxable.checked_add(sum_tax).ok_or_else(|| {
                AccountingError::MathOverflow("Total before rounding overflow".to_string())
            })?
        };

        let (round_off_paise, final_total) = if enable_rupee_rounding {
            // Round to nearest Rupee (100 paise)
            let rem = before_round % 100;
            if rem == 0 {
                (0i64, before_round)
            } else if rem >= 50 {
                let diff = 100 - rem;
                (diff, before_round + diff)
            } else {
                let diff = -rem;
                (diff, before_round + diff)
            }
        } else {
            (0i64, before_round)
        };

        Ok(InvoiceTaxSummary {
            lines: calculated_lines,
            total_taxable: Paise(sum_taxable),
            total_cgst: Paise(sum_cgst),
            total_sgst: Paise(sum_sgst),
            total_igst: Paise(sum_igst),
            total_tax: Paise(sum_tax),
            total_before_rounding: Paise(before_round),
            round_off_paise,
            final_invoice_total: Paise(final_total),
            is_interstate,
            is_reverse_charge,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclusive_intra_state_calculation() {
        // ₹10,000 @ 18% intra-state
        let input = TaxLineInput {
            line_number: 1,
            hsn_code: Some("8471".to_string()),
            description: "Laptop".to_string(),
            amount_paise: Paise::from_rupees(10000).unwrap(),
            is_tax_inclusive: false,
            rate: GstRate::RATE_18,
        };

        let summary = TaxCalculator::calculate_invoice(&[input], "27", "27", false, false, true).unwrap();
        assert_eq!(summary.is_interstate, false);
        assert_eq!(summary.total_taxable, Paise::from_rupees(10000).unwrap());
        assert_eq!(summary.total_cgst, Paise::from_rupees(900).unwrap());
        assert_eq!(summary.total_sgst, Paise::from_rupees(900).unwrap());
        assert_eq!(summary.total_igst, Paise(0));
        assert_eq!(summary.final_invoice_total, Paise::from_rupees(11800).unwrap());
    }

    #[test]
    fn test_inclusive_inter_state_calculation() {
        // ₹1,180 gross @ 18% inclusive inter-state
        let input = TaxLineInput {
            line_number: 1,
            hsn_code: Some("998313".to_string()),
            description: "IT Support".to_string(),
            amount_paise: Paise::from_rupees(1180).unwrap(),
            is_tax_inclusive: true,
            rate: GstRate::RATE_18,
        };

        let summary = TaxCalculator::calculate_invoice(&[input], "27", "29", false, false, true).unwrap();
        assert_eq!(summary.is_interstate, true);
        assert_eq!(summary.total_taxable, Paise::from_rupees(1000).unwrap());
        assert_eq!(summary.total_igst, Paise::from_rupees(180).unwrap());
        assert_eq!(summary.final_invoice_total, Paise::from_rupees(1180).unwrap());
    }

    #[test]
    fn test_rounding_edge_case_odd_paise() {
        // ₹10.05 @ 18% exclusive:
        // 1005 paise * 1800 / 10000 = 180.9 -> 181 paise tax
        // CGST = 181 / 2 = 90 paise, SGST = 91 paise -> Total = 181 paise
        let input = TaxLineInput {
            line_number: 1,
            hsn_code: Some("9983".to_string()),
            description: "Odd Item".to_string(),
            amount_paise: Paise(1005),
            is_tax_inclusive: false,
            rate: GstRate::RATE_18,
        };

        let summary = TaxCalculator::calculate_invoice(&[input], "27", "27", false, false, false).unwrap();
        assert_eq!(summary.total_cgst.0 + summary.total_sgst.0, summary.total_tax.0);
        assert_eq!(summary.total_tax.0, 181);
        assert_eq!(summary.total_cgst.0, 90);
        assert_eq!(summary.total_sgst.0, 91);
    }
}
