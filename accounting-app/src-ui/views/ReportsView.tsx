import React, { useEffect, useState } from 'react';
import { PieChart, CheckCircle2, AlertCircle, TrendingUp, TrendingDown, Scale } from 'lucide-react';
import { api, formatINR } from '../services/api';
import type { Company, FinancialReportRow, TrialBalanceItem } from '../types';

interface ReportsViewProps {
  company: Company | null;
}

export const ReportsView: React.FC<ReportsViewProps> = ({ company }) => {
  const [reportType, setReportType] = useState<'tb' | 'pl' | 'bs'>('tb');
  const [tbData, setTbData] = useState<{
    items: TrialBalanceItem[];
    total_debit_paise: number;
    total_credit_paise: number;
    is_balanced: boolean;
  } | null>(null);

  const [plData, setPlData] = useState<{
    income_rows: FinancialReportRow[];
    expense_rows: FinancialReportRow[];
    gross_profit_paise: number;
    net_profit_paise: number;
  } | null>(null);

  const [bsData, setBsData] = useState<{
    asset_rows: FinancialReportRow[];
    liability_rows: FinancialReportRow[];
    total_assets_paise: number;
    total_liabilities_paise: number;
    difference_paise: number;
  } | null>(null);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadReport = () => {
    if (!company) return;
    setLoading(true);
    setError(null);

    if (reportType === 'tb') {
      api.getTrialBalance(company.company_id)
        .then((res) => {
          setTbData(res);
          setLoading(false);
        })
        .catch((err) => {
          setError(err?.toString() || 'Failed to load Trial Balance');
          setLoading(false);
        });
    } else if (reportType === 'pl') {
      api.getProfitAndLoss(company.company_id)
        .then((res) => {
          setPlData(res);
          setLoading(false);
        })
        .catch((err) => {
          setError(err?.toString() || 'Failed to load Profit & Loss');
          setLoading(false);
        });
    } else if (reportType === 'bs') {
      api.getBalanceSheet(company.company_id)
        .then((res) => {
          setBsData(res);
          setLoading(false);
        })
        .catch((err) => {
          setError(err?.toString() || 'Failed to load Balance Sheet');
          setLoading(false);
        });
    }
  };

  useEffect(() => {
    loadReport();
  }, [company, reportType]);

  if (!company) {
    return <div className="card">Please select an active company first.</div>;
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <div>
          <h1 style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
            Financial Statements & Statutory Reports
          </h1>
          <p style={{ fontSize: '12px', color: '#64748b' }}>
            Double-entry ledger balances, trading profit & loss, and balance sheet equilibrium
          </p>
        </div>
      </div>

      {error && (
        <div style={{ padding: '10px 14px', background: '#fef2f2', border: '1px solid #fecaca', borderRadius: '6px', color: '#b91c1c', marginBottom: '16px', display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px' }}>
          <AlertCircle size={14} />
          <span>{error}</span>
        </div>
      )}

      {/* Selector Tabs */}
      <div style={{ display: 'flex', gap: '8px', borderBottom: '1px solid #e2e8f0', marginBottom: '16px' }}>
        <button
          className={`btn ${reportType === 'tb' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setReportType('tb')}
        >
          <Scale size={14} /> Trial Balance
        </button>
        <button
          className={`btn ${reportType === 'pl' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setReportType('pl')}
        >
          <TrendingUp size={14} /> Profit & Loss Statement
        </button>
        <button
          className={`btn ${reportType === 'bs' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setReportType('bs')}
        >
          <PieChart size={14} /> Balance Sheet
        </button>
      </div>

      {/* 1. Trial Balance */}
      {reportType === 'tb' && tbData && (
        <div className="card">
          <div style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            padding: '10px 14px',
            background: tbData.is_balanced ? '#f0fdf4' : '#fef2f2',
            border: `1px solid ${tbData.is_balanced ? '#bbf7d0' : '#fecaca'}`,
            borderRadius: '6px',
            marginBottom: '12px',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              {tbData.is_balanced ? (
                <CheckCircle2 size={18} color="#16a34a" />
              ) : (
                <AlertCircle size={18} color="#dc2626" />
              )}
              <span style={{ fontWeight: 600, color: tbData.is_balanced ? '#166534' : '#991b1b' }}>
                {tbData.is_balanced
                  ? 'Trial Balance in Perfect Equilibrium (Total Debit = Total Credit)'
                  : 'Trial Balance is Unbalanced! Reconcile ledger differences.'}
              </span>
            </div>
            <div style={{ fontFamily: 'var(--font-mono)', fontWeight: 700 }}>
              {formatINR(tbData.total_debit_paise)}
            </div>
          </div>

          <div className="table-container" style={{ margin: 0 }}>
            <table className="acc-table">
              <thead>
                <tr>
                  <th>Particulars (Ledger Account)</th>
                  <th>Group</th>
                  <th className="text-right">Debit Balance (₹)</th>
                  <th className="text-right">Credit Balance (₹)</th>
                </tr>
              </thead>
              <tbody>
                {tbData.items.length === 0 ? (
                  <tr>
                    <td colSpan={4} style={{ textAlign: 'center', color: '#94a3b8', padding: '24px' }}>
                      No transactions found.
                    </td>
                  </tr>
                ) : (
                  tbData.items.map((i) => (
                    <tr key={i.ledger_id}>
                      <td style={{ fontWeight: 600 }}>{i.ledger_name}</td>
                      <td style={{ color: '#64748b' }}>{i.group_name}</td>
                      <td className="text-right">{i.debit_paise > 0 ? formatINR(i.debit_paise) : '—'}</td>
                      <td className="text-right">{i.credit_paise > 0 ? formatINR(i.credit_paise) : '—'}</td>
                    </tr>
                  ))
                )}
              </tbody>
              <tfoot>
                <tr style={{ background: '#f8fafc', fontWeight: 700 }}>
                  <td colSpan={2} style={{ textAlign: 'right', padding: '10px 12px' }}>
                    Total:
                  </td>
                  <td className="text-right" style={{ padding: '10px 12px' }}>{formatINR(tbData.total_debit_paise)}</td>
                  <td className="text-right" style={{ padding: '10px 12px' }}>{formatINR(tbData.total_credit_paise)}</td>
                </tr>
              </tfoot>
            </table>
          </div>
        </div>
      )}

      {/* 2. Profit & Loss */}
      {reportType === 'pl' && plData && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          {/* Expenses */}
          <div className="card">
            <h3 style={{ fontSize: '13px', fontWeight: 600, color: '#dc2626', marginBottom: '8px' }}>
              Expenses
            </h3>
            <div className="table-container" style={{ margin: 0 }}>
              <table className="acc-table">
                <thead>
                  <tr>
                    <th>Particulars</th>
                    <th className="text-right">Amount (₹)</th>
                  </tr>
                </thead>
                <tbody>
                  {plData.expense_rows.length === 0 ? (
                    <tr>
                      <td colSpan={2} style={{ textAlign: 'center', color: '#94a3b8', padding: '16px' }}>
                        No expense entries
                      </td>
                    </tr>
                  ) : (
                    plData.expense_rows.map((r, idx) => (
                      <tr key={idx}>
                        <td>{r.group_name}</td>
                        <td className="text-right">{formatINR(r.amount_paise)}</td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>

          {/* Income & Net Profit */}
          <div className="card">
            <h3 style={{ fontSize: '13px', fontWeight: 600, color: '#16a34a', marginBottom: '8px' }}>
              Income & Profit
            </h3>
            <div className="table-container" style={{ margin: 0 }}>
              <table className="acc-table">
                <thead>
                  <tr>
                    <th>Particulars</th>
                    <th className="text-right">Amount (₹)</th>
                  </tr>
                </thead>
                <tbody>
                  {plData.income_rows.length === 0 ? (
                    <tr>
                      <td colSpan={2} style={{ textAlign: 'center', color: '#94a3b8', padding: '16px' }}>
                        No income entries
                      </td>
                    </tr>
                  ) : (
                    plData.income_rows.map((r, idx) => (
                      <tr key={idx}>
                        <td>{r.group_name}</td>
                        <td className="text-right">{formatINR(r.amount_paise)}</td>
                      </tr>
                    ))
                  )}
                </tbody>
                <tfoot>
                  <tr style={{ background: '#f0fdf4', fontWeight: 700 }}>
                    <td style={{ padding: '10px 12px' }}>Net Profit / (Loss):</td>
                    <td className="text-right" style={{ color: plData.net_profit_paise >= 0 ? '#16a34a' : '#dc2626', padding: '10px 12px' }}>
                      {formatINR(plData.net_profit_paise)}
                    </td>
                  </tr>
                </tfoot>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* 3. Balance Sheet */}
      {reportType === 'bs' && bsData && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          {/* Liabilities */}
          <div className="card">
            <h3 style={{ fontSize: '13px', fontWeight: 600, color: '#475569', marginBottom: '8px' }}>
              Liabilities & Equity
            </h3>
            <div className="table-container" style={{ margin: 0 }}>
              <table className="acc-table">
                <thead>
                  <tr>
                    <th>Liabilities</th>
                    <th className="text-right">Amount (₹)</th>
                  </tr>
                </thead>
                <tbody>
                  {bsData.liability_rows.length === 0 ? (
                    <tr>
                      <td colSpan={2} style={{ textAlign: 'center', color: '#94a3b8', padding: '16px' }}>
                        No liability entries
                      </td>
                    </tr>
                  ) : (
                    bsData.liability_rows.map((r, idx) => (
                      <tr key={idx}>
                        <td>{r.group_name}</td>
                        <td className="text-right">{formatINR(r.amount_paise)}</td>
                      </tr>
                    ))
                  )}
                </tbody>
                <tfoot>
                  <tr style={{ background: '#f8fafc', fontWeight: 700 }}>
                    <td style={{ padding: '10px 12px' }}>Total Liabilities:</td>
                    <td className="text-right" style={{ padding: '10px 12px' }}>{formatINR(bsData.total_liabilities_paise)}</td>
                  </tr>
                </tfoot>
              </table>
            </div>
          </div>

          {/* Assets */}
          <div className="card">
            <h3 style={{ fontSize: '13px', fontWeight: 600, color: '#475569', marginBottom: '8px' }}>
              Assets
            </h3>
            <div className="table-container" style={{ margin: 0 }}>
              <table className="acc-table">
                <thead>
                  <tr>
                    <th>Assets</th>
                    <th className="text-right">Amount (₹)</th>
                  </tr>
                </thead>
                <tbody>
                  {bsData.asset_rows.length === 0 ? (
                    <tr>
                      <td colSpan={2} style={{ textAlign: 'center', color: '#94a3b8', padding: '16px' }}>
                        No asset entries
                      </td>
                    </tr>
                  ) : (
                    bsData.asset_rows.map((r, idx) => (
                      <tr key={idx}>
                        <td>{r.group_name}</td>
                        <td className="text-right">{formatINR(r.amount_paise)}</td>
                      </tr>
                    ))
                  )}
                </tbody>
                <tfoot>
                  <tr style={{ background: '#f8fafc', fontWeight: 700 }}>
                    <td style={{ padding: '10px 12px' }}>Total Assets:</td>
                    <td className="text-right" style={{ padding: '10px 12px' }}>{formatINR(bsData.total_assets_paise)}</td>
                  </tr>
                </tfoot>
              </table>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
