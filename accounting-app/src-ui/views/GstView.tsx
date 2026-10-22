import React, { useState } from 'react';
import { FileSpreadsheet, Download, AlertTriangle, Calculator } from 'lucide-react';
import { formatINR } from '../services/api';
import type { Company, FinancialYear } from '../types';

interface GstViewProps {
  company: Company | null;
  fy: FinancialYear | null;
}

export const GstView: React.FC<GstViewProps> = ({ company, fy }) => {
  const [activeTab, setActiveTab] = useState<'calculator' | 'gstr1' | 'gstr3b'>('calculator');

  // Interactive GST Calculator / Invoice simulator state
  const [partyState, setPartyState] = useState('27'); // Maharashtra default
  const [taxRateBps, setTaxRateBps] = useState(1800); // 18%
  const [isInclusive, setIsInclusive] = useState(false);
  const [priceRupees, setPriceRupees] = useState('10000');
  const [isReverseCharge, setIsReverseCharge] = useState(false);

  const compState = company?.state_code || '27';
  const isIntraState = partyState === compState;

  // Calculation
  const amountPaise = Math.round((parseFloat(priceRupees) || 0) * 100);
  let taxablePaise = 0;
  let cgstPaise = 0;
  let sgstPaise = 0;
  let igstPaise = 0;
  let totalInvoicePaise = 0;

  if (isInclusive) {
    taxablePaise = Math.round((amountPaise * 10000) / (10000 + taxRateBps));
    const totalTaxPaise = amountPaise - taxablePaise;
    if (isIntraState) {
      cgstPaise = Math.floor(totalTaxPaise / 2);
      sgstPaise = totalTaxPaise - cgstPaise;
    } else {
      igstPaise = totalTaxPaise;
    }
    totalInvoicePaise = amountPaise;
  } else {
    taxablePaise = amountPaise;
    if (isIntraState) {
      const halfRate = Math.floor(taxRateBps / 2);
      cgstPaise = Math.round((taxablePaise * halfRate) / 10000);
      sgstPaise = Math.round((taxablePaise * halfRate) / 10000);
      totalInvoicePaise = taxablePaise + cgstPaise + sgstPaise;
    } else {
      igstPaise = Math.round((taxablePaise * taxRateBps) / 10000);
      totalInvoicePaise = taxablePaise + igstPaise;
    }
  }

  const exportGstr1Csv = () => {
    const csvContent = "data:text/csv;charset=utf-8,"
      + "GSTIN/UIN,Receiver Name,Invoice Number,Invoice Date,Invoice Value,Place of Supply,Reverse Charge,Invoice Type,Rate,Taxable Value,Cess Amount\n"
      + `27AAPCA1234F1Z5,Demo Client Ltd,INV-101,2026-04-10,${(totalInvoicePaise / 100).toFixed(2)},${partyState}-State,N,Regular,${(taxRateBps / 100).toFixed(2)},${(taxablePaise / 100).toFixed(2)},0.00\n`;
    const encodedUri = encodeURI(csvContent);
    const link = document.createElement("a");
    link.setAttribute("href", encodedUri);
    link.setAttribute("download", `GSTR1_Preliminary_${company?.company_number || 'export'}.csv`);
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  if (!company) {
    return <div className="card">Please select an active company first.</div>;
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <div>
          <h1 style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
            GST Statutory Portal & Invoicing Engine
          </h1>
          <p style={{ fontSize: '12px', color: '#64748b' }}>
            Company Origin State: {company.state_code} | Automatic CGST/SGST vs IGST bifurcations
          </p>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button className="btn btn-secondary" onClick={exportGstr1Csv}>
            <Download size={14} /> Export GSTR-1 CSV
          </button>
        </div>
      </div>

      {/* Statutory Disclaimer Alert */}
      <div style={{
        padding: '10px 14px',
        background: '#fffbeb',
        border: '1px solid #fef08a',
        borderRadius: '6px',
        color: '#854d0e',
        marginBottom: '16px',
        display: 'flex',
        alignItems: 'center',
        gap: '8px',
        fontSize: '11px',
      }}>
        <AlertTriangle size={16} color="#d97706" style={{ flexShrink: 0 }} />
        <span>
          <strong>PRELIMINARY STATUTORY NOTICE:</strong> All GSTR-1 and GSTR-3B summaries and tax classification tables in this release are preliminary tools designed for internal accounting audit. Reconcile with the official GSTN common portal prior to statutory return filings.
        </span>
      </div>

      {/* Tabs */}
      <div style={{ display: 'flex', gap: '8px', borderBottom: '1px solid #e2e8f0', marginBottom: '16px' }}>
        <button
          className={`btn ${activeTab === 'calculator' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('calculator')}
        >
          <Calculator size={14} /> GST Invoice Rate Calculator
        </button>
        <button
          className={`btn ${activeTab === 'gstr1' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('gstr1')}
        >
          <FileSpreadsheet size={14} /> Preliminary GSTR-1
        </button>
        <button
          className={`btn ${activeTab === 'gstr3b' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('gstr3b')}
        >
          <FileSpreadsheet size={14} /> Preliminary GSTR-3B
        </button>
      </div>

      {/* Tab 1: Calculator */}
      {activeTab === 'calculator' && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          <div className="card">
            <h3 style={{ fontSize: '13px', fontWeight: 600, marginBottom: '12px' }}>Invoice Parameters</h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div>
                <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                  Place of Supply (Customer State)
                </label>
                <select
                  className="select"
                  style={{ width: '100%' }}
                  value={partyState}
                  onChange={(e) => setPartyState(e.target.value)}
                >
                  <option value={compState}>Intra-State (Same State: {compState} - CGST + SGST)</option>
                  <option value="29">Inter-State: Karnataka (29 - IGST)</option>
                  <option value="07">Inter-State: Delhi (07 - IGST)</option>
                  <option value="24">Inter-State: Gujarat (24 - IGST)</option>
                  <option value="33">Inter-State: Tamil Nadu (33 - IGST)</option>
                </select>
              </div>

              <div>
                <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                  GST Slab Rate
                </label>
                <select
                  className="select"
                  style={{ width: '100%' }}
                  value={taxRateBps}
                  onChange={(e) => setTaxRateBps(parseInt(e.target.value))}
                >
                  <option value={0}>0% (Exempt)</option>
                  <option value={500}>5% (2.5% CGST + 2.5% SGST)</option>
                  <option value={1200}>12% (6% CGST + 6% SGST)</option>
                  <option value={1800}>18% (9% CGST + 9% SGST)</option>
                  <option value={2800}>28% (14% CGST + 14% SGST)</option>
                </select>
              </div>

              <div>
                <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                  Invoice Item Amount (₹)
                </label>
                <input
                  type="number"
                  step="0.01"
                  className="input"
                  style={{ width: '100%', fontFamily: 'var(--font-mono)' }}
                  value={priceRupees}
                  onChange={(e) => setPriceRupees(e.target.value)}
                />
              </div>

              <div style={{ display: 'flex', gap: '16px' }}>
                <label style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '12px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={isInclusive}
                    onChange={(e) => setIsInclusive(e.target.checked)}
                  />
                  <span>Tax Inclusive Price</span>
                </label>
                <label style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '12px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={isReverseCharge}
                    onChange={(e) => setIsReverseCharge(e.target.checked)}
                  />
                  <span>Reverse Charge (RCM)</span>
                </label>
              </div>
            </div>
          </div>

          {/* Tax Breakdown Result Card */}
          <div className="card" style={{ background: '#f8fafc' }}>
            <h3 style={{ fontSize: '13px', fontWeight: 600, marginBottom: '12px' }}>
              Computed Statutory Tax Breakdown
            </h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid #e2e8f0' }}>
                <span style={{ color: '#64748b' }}>Classification:</span>
                <span style={{ fontWeight: 600, color: isIntraState ? '#2563eb' : '#7c3aed' }}>
                  {isIntraState ? 'Intra-State (CGST + SGST)' : 'Inter-State (IGST)'}
                </span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid #e2e8f0' }}>
                <span style={{ color: '#64748b' }}>Taxable Value:</span>
                <span style={{ fontFamily: 'var(--font-mono)', fontWeight: 600 }}>{formatINR(taxablePaise)}</span>
              </div>
              {isIntraState ? (
                <>
                  <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid #e2e8f0' }}>
                    <span style={{ color: '#64748b' }}>CGST ({(taxRateBps / 200).toFixed(1)}%):</span>
                    <span style={{ fontFamily: 'var(--font-mono)' }}>{formatINR(cgstPaise)}</span>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid #e2e8f0' }}>
                    <span style={{ color: '#64748b' }}>SGST ({(taxRateBps / 200).toFixed(1)}%):</span>
                    <span style={{ fontFamily: 'var(--font-mono)' }}>{formatINR(sgstPaise)}</span>
                  </div>
                </>
              ) : (
                <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid #e2e8f0' }}>
                  <span style={{ color: '#64748b' }}>IGST ({(taxRateBps / 100).toFixed(1)}%):</span>
                  <span style={{ fontFamily: 'var(--font-mono)' }}>{formatINR(igstPaise)}</span>
                </div>
              )}
              <div style={{ display: 'flex', justifyContent: 'space-between', padding: '10px 0', borderTop: '2px solid #cbd5e1', marginTop: '6px' }}>
                <span style={{ fontWeight: 700, fontSize: '14px' }}>Invoice Total:</span>
                <span style={{ fontFamily: 'var(--font-mono)', fontWeight: 700, fontSize: '15px', color: '#16a34a' }}>
                  {formatINR(totalInvoicePaise)}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tab 2: Preliminary GSTR-1 */}
      {activeTab === 'gstr1' && (
        <div className="card">
          <h3 style={{ fontSize: '13px', fontWeight: 600, marginBottom: '8px' }}>
            Preliminary GSTR-1: Outward Supplies Summary
          </h3>
          <div className="table-container" style={{ margin: 0 }}>
            <table className="acc-table">
              <thead>
                <tr>
                  <th>Section</th>
                  <th>Description</th>
                  <th className="text-right">Taxable Value</th>
                  <th className="text-right">CGST</th>
                  <th className="text-right">SGST</th>
                  <th className="text-right">IGST</th>
                  <th className="text-right">Total Invoice Value</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td style={{ fontWeight: 600 }}>4A, 4B (B2B)</td>
                  <td>Registered B2B Invoices</td>
                  <td className="text-right">{formatINR(taxablePaise)}</td>
                  <td className="text-right">{formatINR(cgstPaise)}</td>
                  <td className="text-right">{formatINR(sgstPaise)}</td>
                  <td className="text-right">{formatINR(igstPaise)}</td>
                  <td className="text-right" style={{ fontWeight: 600 }}>{formatINR(totalInvoicePaise)}</td>
                </tr>
                <tr>
                  <td style={{ fontWeight: 600 }}>5A (B2CL)</td>
                  <td>B2C Large Inter-State (&gt; ₹2.5L)</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right" style={{ fontWeight: 600 }}>{formatINR(0)}</td>
                </tr>
                <tr>
                  <td style={{ fontWeight: 600 }}>7 (B2CS)</td>
                  <td>B2C Small Supplies</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right" style={{ fontWeight: 600 }}>{formatINR(0)}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Tab 3: Preliminary GSTR-3B */}
      {activeTab === 'gstr3b' && (
        <div className="card">
          <h3 style={{ fontSize: '13px', fontWeight: 600, marginBottom: '8px' }}>
            Preliminary GSTR-3B: Monthly Summary Return
          </h3>
          <div className="table-container" style={{ margin: 0 }}>
            <table className="acc-table">
              <thead>
                <tr>
                  <th>Table</th>
                  <th>Nature of Supply</th>
                  <th className="text-right">Total Taxable Value</th>
                  <th className="text-right">Integrated Tax</th>
                  <th className="text-right">Central Tax</th>
                  <th className="text-right">State/UT Tax</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td style={{ fontWeight: 600 }}>3.1(a)</td>
                  <td>Outward taxable supplies (other than zero rated, nil rated and exempted)</td>
                  <td className="text-right">{formatINR(taxablePaise)}</td>
                  <td className="text-right">{formatINR(igstPaise)}</td>
                  <td className="text-right">{formatINR(cgstPaise)}</td>
                  <td className="text-right">{formatINR(sgstPaise)}</td>
                </tr>
                <tr>
                  <td style={{ fontWeight: 600 }}>4(A)(5)</td>
                  <td>All other Eligible Input Tax Credit (ITC)</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right">{formatINR(0)}</td>
                  <td className="text-right">{formatINR(0)}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
};
