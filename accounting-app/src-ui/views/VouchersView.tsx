import React, { useEffect, useState } from 'react';
import { Plus, Receipt, Trash2, AlertCircle, CheckCircle2, RotateCcw } from 'lucide-react';
import { api, formatINR } from '../services/api';
import type { Company, FinancialYear, Ledger, VoucherSummary } from '../types';

interface VouchersViewProps {
  company: Company | null;
  fy: FinancialYear | null;
}

interface EntryLine {
  id: string;
  ledger_id: string;
  entry_type: 'Dr' | 'Cr';
  amountRupees: string;
  narration: string;
}

export const VouchersView: React.FC<VouchersViewProps> = ({ company, fy }) => {
  const [vouchers, setVouchers] = useState<VoucherSummary[]>([]);
  const [ledgers, setLedgers] = useState<Ledger[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showEntryModal, setShowEntryModal] = useState(false);

  // Filter
  const [filterType, setFilterType] = useState('All');

  // Form states
  const [voucherType, setVoucherType] = useState('Sales');
  const [voucherNumber, setVoucherNumber] = useState('');
  const [voucherDate, setVoucherDate] = useState(new Date().toISOString().slice(0, 10));
  const [voucherNarration, setVoucherNarration] = useState('');
  const [lines, setLines] = useState<EntryLine[]>([
    { id: '1', ledger_id: '', entry_type: 'Dr', amountRupees: '0', narration: '' },
    { id: '2', ledger_id: '', entry_type: 'Cr', amountRupees: '0', narration: '' },
  ]);
  const [submitting, setSubmitting] = useState(false);

  const loadData = () => {
    if (!company) return;
    setLoading(true);
    setError(null);
    Promise.all([
      api.getVouchers(company.company_id, fy?.fy_id),
      api.getLedgers(company.company_id),
    ])
      .then(([vList, lList]) => {
        setVouchers(vList);
        setLedgers(lList);
        if (lList.length >= 2 && lines[0].ledger_id === '') {
          setLines([
            { id: '1', ledger_id: lList[0].ledger_id, entry_type: 'Dr', amountRupees: '1000', narration: '' },
            { id: '2', ledger_id: lList[1].ledger_id, entry_type: 'Cr', amountRupees: '1000', narration: '' },
          ]);
        }
        setLoading(false);
      })
      .catch((err) => {
        setError(err?.toString() || 'Failed to load vouchers');
        setLoading(false);
      });
  };

  useEffect(() => {
    loadData();
    // Auto generate voucher number
    if (company) {
      const prefix = voucherType.slice(0, 3).toUpperCase();
      const rand = Math.floor(100 + Math.random() * 900);
      setVoucherNumber(`${prefix}-${rand}`);
    }
  }, [company, fy, voucherType]);

  const addLine = (type: 'Dr' | 'Cr') => {
    setLines([
      ...lines,
      {
        id: Math.random().toString(),
        ledger_id: ledgers[0]?.ledger_id || '',
        entry_type: type,
        amountRupees: '0',
        narration: '',
      },
    ]);
  };

  const removeLine = (id: string) => {
    if (lines.length <= 2) return;
    setLines(lines.filter((l) => l.id !== id));
  };

  const updateLine = (id: string, field: keyof EntryLine, val: any) => {
    setLines(
      lines.map((l) => (l.id === id ? { ...l, [field]: val } : l))
    );
  };

  // Calculations
  const totalDebitPaise = lines
    .filter((l) => l.entry_type === 'Dr')
    .reduce((sum, l) => sum + Math.round((parseFloat(l.amountRupees) || 0) * 100), 0);

  const totalCreditPaise = lines
    .filter((l) => l.entry_type === 'Cr')
    .reduce((sum, l) => sum + Math.round((parseFloat(l.amountRupees) || 0) * 100), 0);

  const diffPaise = Math.abs(totalDebitPaise - totalCreditPaise);
  const isBalanced = totalDebitPaise > 0 && totalDebitPaise === totalCreditPaise;

  const handlePostVoucher = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!company || !fy || !isBalanced) return;

    setSubmitting(true);
    setError(null);
    try {
      const draft = {
        company_id: company.company_id,
        financial_year_id: fy.fy_id,
        voucher_type: voucherType,
        voucher_number: voucherNumber.trim(),
        date: voucherDate,
        reference_number: null,
        narration: voucherNarration.trim() || null,
        entries: lines.map((l) => ({
          ledger_id: l.ledger_id,
          entry_type: l.entry_type,
          amount_paise: Math.round((parseFloat(l.amountRupees) || 0) * 100),
          narration: l.narration.trim() || null,
        })),
      };

      await api.postVoucher(draft);
      setShowEntryModal(false);
      setVoucherNarration('');
      loadData();
    } catch (err: any) {
      setError(err?.toString() || 'Failed to post voucher');
    } finally {
      setSubmitting(false);
    }
  };

  const handleCancelVoucher = async (voucherId: string) => {
    const reason = window.prompt('Enter reason for voucher cancellation (Required for audit trail):');
    if (!reason || !reason.trim()) return;

    try {
      await api.cancelVoucher(voucherId, reason.trim());
      loadData();
    } catch (err: any) {
      alert(err?.toString() || 'Failed to cancel voucher');
    }
  };

  const filteredVouchers = filterType === 'All'
    ? vouchers
    : vouchers.filter((v) => v.voucher_type === filterType);

  if (!company) {
    return <div className="card">Please select an active company first.</div>;
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <div>
          <h1 style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
            Day Book & Voucher Processing
          </h1>
          <p style={{ fontSize: '12px', color: '#64748b' }}>
            Multi-entry balanced accounting ledger transactions with immutable audit trails
          </p>
        </div>
        <button
          className="btn btn-primary"
          onClick={() => {
            if (!fy) {
              alert('Please select or create an active Financial Year first in Company screen');
              return;
            }
            setShowEntryModal(true);
          }}
        >
          <Plus size={14} /> New Voucher Entry
        </button>
      </div>

      {error && (
        <div style={{ padding: '10px 14px', background: '#fef2f2', border: '1px solid #fecaca', borderRadius: '6px', color: '#b91c1c', marginBottom: '16px', display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px' }}>
          <AlertCircle size={14} />
          <span>{error}</span>
        </div>
      )}

      {/* Filter Toolbar */}
      <div style={{ display: 'flex', gap: '12px', alignItems: 'center', marginBottom: '12px' }}>
        <span style={{ fontSize: '12px', fontWeight: 600, color: '#475569' }}>Filter Type:</span>
        <select
          className="select"
          style={{ minWidth: '160px' }}
          value={filterType}
          onChange={(e) => setFilterType(e.target.value)}
        >
          <option value="All">All Vouchers ({vouchers.length})</option>
          <option value="Sales">Sales</option>
          <option value="Purchase">Purchase</option>
          <option value="Receipt">Receipt</option>
          <option value="Payment">Payment</option>
          <option value="Contra">Contra</option>
          <option value="Journal">Journal</option>
        </select>
      </div>

      {/* Day Book Table */}
      <div className="table-container" style={{ margin: 0 }}>
        <table className="acc-table">
          <thead>
            <tr>
              <th>Date</th>
              <th>Voucher No</th>
              <th>Type</th>
              <th>Narration</th>
              <th className="text-right">Total Debit/Credit</th>
              <th>Status</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            {filteredVouchers.length === 0 ? (
              <tr>
                <td colSpan={7} style={{ textAlign: 'center', color: '#94a3b8', padding: '28px' }}>
                  No vouchers found. Click "New Voucher Entry" to post a transaction.
                </td>
              </tr>
            ) : (
              filteredVouchers.map((v) => (
                <tr key={v.voucher_id}>
                  <td>{v.voucher_date}</td>
                  <td style={{ fontWeight: 600 }}>{v.voucher_number}</td>
                  <td>
                    <span style={{ background: '#f1f5f9', padding: '2px 6px', borderRadius: '4px', fontSize: '11px', fontWeight: 500 }}>
                      {v.voucher_type}
                    </span>
                  </td>
                  <td style={{ color: '#64748b', maxWidth: '280px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {v.narration || '—'}
                  </td>
                  <td className="text-right" style={{ fontWeight: 600 }}>
                    {formatINR(v.total_amount_paise)}
                  </td>
                  <td>
                    <span style={{
                      fontWeight: 600,
                      fontSize: '11px',
                      color: v.status === 'Posted' ? '#16a34a' : v.status === 'Cancelled' ? '#dc2626' : '#d97706',
                    }}>
                      {v.status === 'Posted' ? '● Posted' : v.status === 'Cancelled' ? '✕ Cancelled' : 'Draft'}
                    </span>
                  </td>
                  <td>
                    {v.status === 'Posted' && (
                      <button
                        className="btn btn-secondary"
                        style={{ padding: '2px 6px', fontSize: '10px' }}
                        title="Cancel/Reverse Voucher"
                        onClick={() => handleCancelVoucher(v.voucher_id)}
                      >
                        <RotateCcw size={11} /> Cancel
                      </button>
                    )}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* New Voucher Entry Modal */}
      {showEntryModal && (
        <div className="modal-overlay">
          <div className="modal-content" style={{ maxWidth: '850px' }}>
            <div className="modal-header">
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <Receipt size={18} color="#2563eb" />
                <h3 style={{ fontSize: '14px', fontWeight: 600 }}>Post Accounting Voucher Entry</h3>
              </div>
              <button
                style={{ background: 'transparent', border: 'none', cursor: 'pointer', fontSize: '16px', color: '#64748b' }}
                onClick={() => setShowEntryModal(false)}
              >
                ✕
              </button>
            </div>
            <form onSubmit={handlePostVoucher}>
              <div className="modal-body" style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
                {/* Header Inputs */}
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '12px' }}>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      Voucher Type *
                    </label>
                    <select
                      className="select"
                      style={{ width: '100%' }}
                      value={voucherType}
                      onChange={(e) => setVoucherType(e.target.value)}
                    >
                      <option value="Sales">Sales</option>
                      <option value="Purchase">Purchase</option>
                      <option value="Receipt">Receipt</option>
                      <option value="Payment">Payment</option>
                      <option value="Contra">Contra</option>
                      <option value="Journal">Journal</option>
                    </select>
                  </div>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      Voucher Number *
                    </label>
                    <input
                      type="text"
                      required
                      className="input"
                      style={{ width: '100%' }}
                      value={voucherNumber}
                      onChange={(e) => setVoucherNumber(e.target.value)}
                    />
                  </div>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      Transaction Date *
                    </label>
                    <input
                      type="date"
                      required
                      className="input"
                      style={{ width: '100%' }}
                      value={voucherDate}
                      onChange={(e) => setVoucherDate(e.target.value)}
                    />
                  </div>
                </div>

                {/* Multi-line Entries */}
                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '6px' }}>
                    <span style={{ fontSize: '12px', fontWeight: 600, color: '#334155' }}>Debit & Credit Entries</span>
                    <div style={{ display: 'flex', gap: '6px' }}>
                      <button type="button" className="btn btn-secondary" style={{ padding: '2px 8px', fontSize: '11px' }} onClick={() => addLine('Dr')}>
                        + Add Dr Line
                      </button>
                      <button type="button" className="btn btn-secondary" style={{ padding: '2px 8px', fontSize: '11px' }} onClick={() => addLine('Cr')}>
                        + Add Cr Line
                      </button>
                    </div>
                  </div>

                  <table className="acc-table" style={{ border: '1px solid #e2e8f0', borderRadius: '4px' }}>
                    <thead>
                      <tr>
                        <th style={{ width: '60px' }}>Dr / Cr</th>
                        <th>Ledger Account</th>
                        <th style={{ width: '150px' }} className="text-right">Amount (₹)</th>
                        <th>Line Narration</th>
                        <th style={{ width: '40px' }}></th>
                      </tr>
                    </thead>
                    <tbody>
                      {lines.map((l) => (
                        <tr key={l.id}>
                          <td>
                            <select
                              className="select"
                              style={{ width: '100%', fontWeight: 700 }}
                              value={l.entry_type}
                              onChange={(e) => updateLine(l.id, 'entry_type', e.target.value as 'Dr' | 'Cr')}
                            >
                              <option value="Dr">Dr</option>
                              <option value="Cr">Cr</option>
                            </select>
                          </td>
                          <td>
                            <select
                              className="select"
                              style={{ width: '100%' }}
                              value={l.ledger_id}
                              onChange={(e) => updateLine(l.id, 'ledger_id', e.target.value)}
                            >
                              {ledgers.map((item) => (
                                <option key={item.ledger_id} value={item.ledger_id}>
                                  {item.name}
                                </option>
                              ))}
                            </select>
                          </td>
                          <td>
                            <input
                              type="number"
                              step="0.01"
                              className="input"
                              style={{ width: '100%', textAlign: 'right', fontFamily: 'var(--font-mono)' }}
                              value={l.amountRupees}
                              onChange={(e) => updateLine(l.id, 'amountRupees', e.target.value)}
                            />
                          </td>
                          <td>
                            <input
                              type="text"
                              placeholder="Optional line note"
                              className="input"
                              style={{ width: '100%' }}
                              value={l.narration}
                              onChange={(e) => updateLine(l.id, 'narration', e.target.value)}
                            />
                          </td>
                          <td>
                            <button
                              type="button"
                              style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: '#dc2626' }}
                              onClick={() => removeLine(l.id)}
                              disabled={lines.length <= 2}
                            >
                              <Trash2 size={14} />
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>

                {/* Balance Equilibrium Summary */}
                <div style={{
                  padding: '12px 16px',
                  background: isBalanced ? '#f0fdf4' : '#fffbeb',
                  border: `1px solid ${isBalanced ? '#bbf7d0' : '#fef08a'}`,
                  borderRadius: '6px',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    {isBalanced ? (
                      <CheckCircle2 size={18} color="#16a34a" />
                    ) : (
                      <AlertCircle size={18} color="#d97706" />
                    )}
                    <div>
                      <div style={{ fontWeight: 600, fontSize: '12px', color: isBalanced ? '#166534' : '#854d0e' }}>
                        {isBalanced ? 'Voucher is perfectly balanced' : `Voucher is unbalanced! Difference: ${formatINR(diffPaise)}`}
                      </div>
                      <div style={{ fontSize: '11px', color: '#64748b' }}>
                        Total Debits: {formatINR(totalDebitPaise)} | Total Credits: {formatINR(totalCreditPaise)}
                      </div>
                    </div>
                  </div>
                  <div style={{ fontFamily: 'var(--font-mono)', fontSize: '14px', fontWeight: 700, color: '#0f172a' }}>
                    {formatINR(totalDebitPaise)}
                  </div>
                </div>

                {/* Overall Narration */}
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Overall Voucher Narration
                  </label>
                  <textarea
                    rows={2}
                    className="input"
                    style={{ width: '100%', resize: 'none' }}
                    placeholder="Enter narration for audit record..."
                    value={voucherNarration}
                    onChange={(e) => setVoucherNarration(e.target.value)}
                  />
                </div>
              </div>
              <div className="modal-footer">
                <button type="button" className="btn btn-secondary" onClick={() => setShowEntryModal(false)}>
                  Cancel
                </button>
                <button
                  type="submit"
                  className="btn btn-primary"
                  disabled={!isBalanced || submitting}
                  style={{ opacity: isBalanced ? 1 : 0.5, cursor: isBalanced ? 'pointer' : 'not-allowed' }}
                >
                  {submitting ? 'Posting...' : 'Post Immutable Voucher'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
