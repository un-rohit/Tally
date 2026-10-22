import React, { useEffect, useState } from 'react';
import { CalendarClock, Users, Building2, AlertCircle } from 'lucide-react';
import { api, formatINR } from '../services/api';
import type { Company, Ledger, OutstandingBill, PartyAgingRow } from '../types';

interface BillWiseViewProps {
  company: Company | null;
}

export const BillWiseView: React.FC<BillWiseViewProps> = ({ company }) => {
  const [activeTab, setActiveTab] = useState<'receivables' | 'payables' | 'aging'>('receivables');
  const [parties, setParties] = useState<Ledger[]>([]);
  const [selectedPartyId, setSelectedPartyId] = useState<string>('');
  const [bills, setBills] = useState<OutstandingBill[]>([]);
  const [agingRows, setAgingRows] = useState<PartyAgingRow[]>([]);
  const [isReceivableAging, setIsReceivableAging] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadParties = () => {
    if (!company) return;
    setLoading(true);
    setError(null);
    api.getLedgers(company.company_id)
      .then((lList) => {
        setParties(lList);
        if (lList.length > 0 && !selectedPartyId) {
          setSelectedPartyId(lList[0].ledger_id);
        }
        setLoading(false);
      })
      .catch((err) => {
        setError(err?.toString() || 'Failed to load ledgers');
        setLoading(false);
      });
  };

  useEffect(() => {
    loadParties();
  }, [company]);

  useEffect(() => {
    if (!company) return;
    if (activeTab === 'aging') {
      setLoading(true);
      api.getAgingReport(company.company_id, isReceivableAging)
        .then((rows) => {
          setAgingRows(rows);
          setLoading(false);
        })
        .catch((err) => {
          setError(err?.toString() || 'Failed to load ageing report');
          setLoading(false);
        });
    } else if (selectedPartyId) {
      setLoading(true);
      api.getOutstandingBills(company.company_id, selectedPartyId)
        .then((bList) => {
          setBills(bList);
          setLoading(false);
        })
        .catch((err) => {
          setError(err?.toString() || 'Failed to load outstanding bills');
          setLoading(false);
        });
    }
  }, [company, selectedPartyId, activeTab, isReceivableAging]);

  if (!company) {
    return <div className="card">Please select an active company first.</div>;
  }

  const totalOutstandingPaise = bills.reduce((sum, b) => sum + b.outstanding_amount_paise, 0);

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <div>
          <h1 style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
            Bill-Wise Accounting & Outstanding Tracking
          </h1>
          <p style={{ fontSize: '12px', color: '#64748b' }}>
            Invoice-by-invoice settlement, partial payments, credit periods, and overdue ageing buckets
          </p>
        </div>
      </div>

      {error && (
        <div style={{ padding: '10px 14px', background: '#fef2f2', border: '1px solid #fecaca', borderRadius: '6px', color: '#b91c1c', marginBottom: '16px', display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px' }}>
          <AlertCircle size={14} />
          <span>{error}</span>
        </div>
      )}

      {/* Tabs */}
      <div style={{ display: 'flex', gap: '8px', borderBottom: '1px solid #e2e8f0', marginBottom: '16px' }}>
        <button
          className={`btn ${activeTab === 'receivables' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('receivables')}
        >
          <Users size={14} /> Customer Outstanding
        </button>
        <button
          className={`btn ${activeTab === 'payables' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('payables')}
        >
          <Building2 size={14} /> Supplier Outstanding
        </button>
        <button
          className={`btn ${activeTab === 'aging' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('aging')}
        >
          <CalendarClock size={14} /> Ageing Analysis (Current, 1-30, 31-60, 61-90, &gt;90 Days)
        </button>
      </div>

      {/* Individual Party Outstanding Tab */}
      {(activeTab === 'receivables' || activeTab === 'payables') && (
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '12px' }}>
            <span style={{ fontSize: '12px', fontWeight: 600, color: '#475569' }}>Select Party:</span>
            <select
              className="select"
              style={{ minWidth: '260px' }}
              value={selectedPartyId}
              onChange={(e) => setSelectedPartyId(e.target.value)}
            >
              {parties.map((p) => (
                <option key={p.ledger_id} value={p.ledger_id}>
                  {p.name}
                </option>
              ))}
            </select>
            {bills.length > 0 && (
              <span style={{ fontSize: '12px', color: '#1e293b', fontWeight: 600 }}>
                Total Pending: <span style={{ color: '#dc2626' }}>{formatINR(totalOutstandingPaise)}</span>
              </span>
            )}
          </div>

          <div className="card">
            <div className="table-container" style={{ margin: 0 }}>
              <table className="acc-table">
                <thead>
                  <tr>
                    <th>Bill Reference</th>
                    <th>Invoice Date</th>
                    <th>Due Date</th>
                    <th>Credit Period</th>
                    <th className="text-right">Original Amount</th>
                    <th className="text-right">Settled Amount</th>
                    <th className="text-right">Outstanding Balance</th>
                    <th>Overdue Days</th>
                  </tr>
                </thead>
                <tbody>
                  {bills.length === 0 ? (
                    <tr>
                      <td colSpan={8} style={{ textAlign: 'center', color: '#94a3b8', padding: '24px' }}>
                        No outstanding bills found for this account. All references are fully settled.
                      </td>
                    </tr>
                  ) : (
                    bills.map((b) => (
                      <tr key={b.bill_ref}>
                        <td style={{ fontWeight: 600 }}>{b.bill_ref}</td>
                        <td>{b.bill_date}</td>
                        <td>{b.due_date || 'Immediate'}</td>
                        <td>{b.credit_days} days</td>
                        <td className="text-right">{formatINR(b.original_amount_paise)}</td>
                        <td className="text-right" style={{ color: '#16a34a' }}>{formatINR(b.settled_amount_paise)}</td>
                        <td className="text-right" style={{ fontWeight: 700, color: '#dc2626' }}>
                          {formatINR(b.outstanding_amount_paise)}
                        </td>
                        <td>
                          {b.is_overdue ? (
                            <span style={{ color: '#dc2626', fontWeight: 600, fontSize: '11px' }}>
                              ⚠ {b.overdue_days} days overdue
                            </span>
                          ) : (
                            <span style={{ color: '#16a34a', fontSize: '11px' }}>Not Overdue</span>
                          )}
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* Ageing Analysis Tab */}
      {activeTab === 'aging' && (
        <div className="card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
            <div style={{ display: 'flex', gap: '8px' }}>
              <button
                className={`btn ${isReceivableAging ? 'btn-primary' : 'btn-secondary'}`}
                style={{ fontSize: '11px', padding: '3px 10px' }}
                onClick={() => setIsReceivableAging(true)}
              >
                Debtors (Receivables Ageing)
              </button>
              <button
                className={`btn ${!isReceivableAging ? 'btn-primary' : 'btn-secondary'}`}
                style={{ fontSize: '11px', padding: '3px 10px' }}
                onClick={() => setIsReceivableAging(false)}
              >
                Creditors (Payables Ageing)
              </button>
            </div>
            <span style={{ fontSize: '11px', color: '#64748b' }}>
              Bucketed by invoice due date as of today
            </span>
          </div>

          <div className="table-container" style={{ margin: 0 }}>
            <table className="acc-table">
              <thead>
                <tr>
                  <th>Party Name</th>
                  <th className="text-right">Current (Not Due)</th>
                  <th className="text-right">1 – 30 Days</th>
                  <th className="text-right">31 – 60 Days</th>
                  <th className="text-right">61 – 90 Days</th>
                  <th className="text-right">&gt; 90 Days Overdue</th>
                  <th className="text-right">Total Outstanding</th>
                </tr>
              </thead>
              <tbody>
                {agingRows.length === 0 ? (
                  <tr>
                    <td colSpan={7} style={{ textAlign: 'center', color: '#94a3b8', padding: '24px' }}>
                      No party outstanding balances detected.
                    </td>
                  </tr>
                ) : (
                  agingRows.map((r) => (
                    <tr key={r.ledger_id}>
                      <td style={{ fontWeight: 600 }}>{r.party_name}</td>
                      <td className="text-right">{formatINR(r.current_paise)}</td>
                      <td className="text-right">{formatINR(r.days_1_30_paise)}</td>
                      <td className="text-right">{formatINR(r.days_31_60_paise)}</td>
                      <td className="text-right">{formatINR(r.days_61_90_paise)}</td>
                      <td className="text-right" style={{ color: r.over_90_paise > 0 ? '#dc2626' : undefined, fontWeight: r.over_90_paise > 0 ? 600 : 400 }}>
                        {formatINR(r.over_90_paise)}
                      </td>
                      <td className="text-right" style={{ fontWeight: 700 }}>
                        {formatINR(r.total_outstanding_paise)}
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
};
