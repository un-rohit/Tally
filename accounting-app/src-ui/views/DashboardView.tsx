import React, { useEffect, useState } from 'react';
import {
  TrendingUp,
  TrendingDown,
  Wallet,
  Users,
  Building2,
  PackageCheck,
  ReceiptText,
  AlertCircle,
  PlusCircle,
} from 'lucide-react';
import { api, formatINR } from '../services/api';
import type { Company, DashboardSummary, FinancialYear } from '../types';

interface DashboardViewProps {
  company: Company | null;
  fy: FinancialYear | null;
  onNavigate: (tab: string) => void;
}

export const DashboardView: React.FC<DashboardViewProps> = ({
  company,
  fy,
  onNavigate,
}) => {
  const [data, setData] = useState<DashboardSummary | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!company || !fy) return;

    setLoading(true);
    setError(null);
    api
      .getDashboardSummary(company.company_id, fy.fy_id)
      .then((res) => {
        setData(res);
        setLoading(false);
      })
      .catch((err) => {
        setError(err?.toString() || 'Failed to load dashboard summary');
        setLoading(false);
      });
  }, [company, fy]);

  if (!company) {
    return (
      <div className="card" style={{ textAlign: 'center', padding: '40px' }}>
        <Building2 size={48} color="#94a3b8" style={{ margin: '0 auto 16px' }} />
        <h2 style={{ fontSize: '18px', fontWeight: 600, marginBottom: '8px' }}>
          Welcome to the Independent Accounting System
        </h2>
        <p style={{ color: '#64748b', marginBottom: '20px', maxWidth: '480px', margin: '0 auto 20px' }}>
          No company is currently selected. Create a company to begin recording vouchers, tracking stock, and preparing financial statements.
        </p>
        <button className="btn btn-primary" onClick={() => onNavigate('companies')}>
          <PlusCircle size={14} /> Create or Select Company
        </button>
      </div>
    );
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <div>
          <h1 style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
            Executive Accounting Dashboard
          </h1>
          <p style={{ fontSize: '12px', color: '#64748b' }}>
            {company.name} | Active Period: {fy ? fy.fy_name : 'No FY selected'}
          </p>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button className="btn btn-primary" onClick={() => onNavigate('vouchers')}>
            <PlusCircle size={13} /> New Voucher
          </button>
          <button className="btn btn-secondary" onClick={() => onNavigate('reports')}>
            View Balance Sheet
          </button>
        </div>
      </div>

      {error && (
        <div style={{ padding: '10px 14px', background: '#fef2f2', border: '1px solid #fecaca', borderRadius: '6px', color: '#b91c1c', marginBottom: '16px', display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px' }}>
          <AlertCircle size={14} />
          <span>{error}</span>
        </div>
      )}

      {/* KPI Cards */}
      <div className="kpi-grid">
        <div className="kpi-card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span className="kpi-label">Total Sales (Turnover)</span>
            <TrendingUp size={16} color="#16a34a" />
          </div>
          <div className="kpi-value">
            {formatINR(data?.total_sales_paise || 0)}
          </div>
          <span style={{ fontSize: '11px', color: '#64748b' }}>For {fy?.fy_name}</span>
        </div>

        <div className="kpi-card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span className="kpi-label">Total Purchases</span>
            <TrendingDown size={16} color="#2563eb" />
          </div>
          <div className="kpi-value">
            {formatINR(data?.total_purchases_paise || 0)}
          </div>
          <span style={{ fontSize: '11px', color: '#64748b' }}>Direct & Trading purchases</span>
        </div>

        <div className="kpi-card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span className="kpi-label">Cash & Bank Balances</span>
            <Wallet size={16} color="#0891b2" />
          </div>
          <div className="kpi-value">
            {formatINR(data?.cash_bank_balance_paise || 0)}
          </div>
          <span style={{ fontSize: '11px', color: '#64748b' }}>Liquid working capital</span>
        </div>

        <div className="kpi-card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span className="kpi-label">Receivables (Debtors)</span>
            <Users size={16} color="#7c3aed" />
          </div>
          <div className="kpi-value">
            {formatINR(data?.total_receivables_paise || 0)}
          </div>
          <span style={{ fontSize: '11px', color: '#64748b' }}>Customer outstanding</span>
        </div>

        <div className="kpi-card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span className="kpi-label">Payables (Creditors)</span>
            <Building2 size={16} color="#ea580c" />
          </div>
          <div className="kpi-value">
            {formatINR(data?.total_payables_paise || 0)}
          </div>
          <span style={{ fontSize: '11px', color: '#64748b' }}>Supplier outstanding</span>
        </div>

        <div className="kpi-card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span className="kpi-label">Closing Inventory</span>
            <PackageCheck size={16} color="#059669" />
          </div>
          <div className="kpi-value">
            {formatINR(data?.inventory_valuation_paise || 0)}
          </div>
          <span style={{ fontSize: '11px', color: '#64748b' }}>Valued via FIFO / Avg</span>
        </div>
      </div>

      {/* Recent Vouchers Section */}
      <div className="card">
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <ReceiptText size={16} color="#475569" />
            <span style={{ fontWeight: 600, fontSize: '13px' }}>Recent Posted Transactions</span>
          </div>
          <button className="btn btn-secondary" style={{ padding: '3px 8px', fontSize: '11px' }} onClick={() => onNavigate('vouchers')}>
            View Full Day Book
          </button>
        </div>

        <div className="table-container" style={{ margin: 0 }}>
          <table className="acc-table">
            <thead>
              <tr>
                <th>Date</th>
                <th>Voucher No</th>
                <th>Type</th>
                <th>Narration</th>
                <th className="text-right">Amount</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {!data?.recent_vouchers || data.recent_vouchers.length === 0 ? (
                <tr>
                  <td colSpan={6} style={{ textAlign: 'center', color: '#94a3b8', padding: '24px' }}>
                    No transactions recorded yet in this financial year.
                  </td>
                </tr>
              ) : (
                data.recent_vouchers.map((v) => (
                  <tr key={v.voucher_id}>
                    <td>{v.voucher_date}</td>
                    <td style={{ fontWeight: 600 }}>{v.voucher_number}</td>
                    <td>
                      <span style={{ background: '#f1f5f9', padding: '2px 6px', borderRadius: '4px', fontSize: '11px', fontWeight: 500 }}>
                        {v.voucher_type}
                      </span>
                    </td>
                    <td style={{ color: '#64748b', maxWidth: '300px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {v.narration || '—'}
                    </td>
                    <td className="text-right" style={{ fontWeight: 600 }}>
                      {formatINR(v.total_amount_paise)}
                    </td>
                    <td>
                      <span style={{ color: '#16a34a', fontWeight: 600, fontSize: '11px' }}>
                        {v.status}
                      </span>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
