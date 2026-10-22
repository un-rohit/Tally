import React, { useState } from 'react';
import { Building2, Plus, Calendar, Check, AlertCircle } from 'lucide-react';
import { api } from '../services/api';
import type { Company, FinancialYear } from '../types';

interface CompanyViewProps {
  companies: Company[];
  activeCompany: Company | null;
  onSelectCompany: (company: Company) => void;
  financialYears: FinancialYear[];
  activeFy: FinancialYear | null;
  onSelectFy: (fy: FinancialYear) => void;
  onRefresh: () => void;
}

export const CompanyView: React.FC<CompanyViewProps> = ({
  companies,
  activeCompany,
  onSelectCompany,
  financialYears,
  activeFy,
  onSelectFy,
  onRefresh,
}) => {
  const [showCreateCompany, setShowCreateCompany] = useState(false);
  const [showCreateFy, setShowCreateFy] = useState(false);
  const [compName, setCompName] = useState('');
  const [compState, setCompState] = useState('27'); // Maharashtra default
  const [fyName, setFyName] = useState('2026-2027');
  const [fyStart, setFyStart] = useState('2026-04-01');
  const [fyEnd, setFyEnd] = useState('2027-03-31');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const indianStates = [
    { code: '27', name: 'Maharashtra (27)' },
    { code: '29', name: 'Karnataka (29)' },
    { code: '07', name: 'Delhi (07)' },
    { code: '24', name: 'Gujarat (24)' },
    { code: '33', name: 'Tamil Nadu (33)' },
    { code: '09', name: 'Uttar Pradesh (09)' },
    { code: '19', name: 'West Bengal (19)' },
    { code: '36', name: 'Telangana (36)' },
    { code: '08', name: 'Rajasthan (08)' },
    { code: '06', name: 'Haryana (06)' },
  ];

  const handleCreateCompany = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!compName.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      const created = await api.createCompany(compName.trim(), compState);
      setCompName('');
      setShowCreateCompany(false);
      onRefresh();
      onSelectCompany(created);
    } catch (err: any) {
      setError(err?.toString() || 'Failed to create company');
    } finally {
      setSubmitting(false);
    }
  };

  const handleCreateFy = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    setSubmitting(true);
    setError(null);
    try {
      const created = await api.createFinancialYear(activeCompany.company_id, fyName.trim(), fyStart, fyEnd);
      setShowCreateFy(false);
      onRefresh();
      onSelectFy(created);
    } catch (err: any) {
      setError(err?.toString() || 'Failed to create financial year');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <div>
          <h1 style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
            Company & Period Management
          </h1>
          <p style={{ fontSize: '12px', color: '#64748b' }}>
            Configure enterprise entities, financial years, and regional compliance boundaries
          </p>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button className="btn btn-primary" onClick={() => setShowCreateCompany(true)}>
            <Plus size={14} /> New Company
          </button>
          {activeCompany && (
            <button className="btn btn-secondary" onClick={() => setShowCreateFy(true)}>
              <Calendar size={14} /> Add Financial Year
            </button>
          )}
        </div>
      </div>

      {error && (
        <div style={{ padding: '10px 14px', background: '#fef2f2', border: '1px solid #fecaca', borderRadius: '6px', color: '#b91c1c', marginBottom: '16px', display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px' }}>
          <AlertCircle size={14} />
          <span>{error}</span>
        </div>
      )}

      {/* Companies Grid */}
      <h2 style={{ fontSize: '13px', fontWeight: 600, color: '#334155', marginBottom: '8px' }}>
        Configured Companies ({companies.length})
      </h2>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: '14px', marginBottom: '24px' }}>
        {companies.map((c) => {
          const isSelected = activeCompany?.company_id === c.company_id;
          return (
            <div
              key={c.company_id}
              className="card"
              style={{
                cursor: 'pointer',
                borderColor: isSelected ? 'var(--primary)' : 'var(--border-color)',
                boxShadow: isSelected ? '0 0 0 1px var(--primary)' : 'none',
              }}
              onClick={() => onSelectCompany(c)}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '8px' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <Building2 size={18} color={isSelected ? '#2563eb' : '#64748b'} />
                  <div>
                    <h3 style={{ fontSize: '14px', fontWeight: 600, color: '#0f172a' }}>{c.name}</h3>
                    <span style={{ fontSize: '11px', color: '#64748b' }}>Code: {c.company_number}</span>
                  </div>
                </div>
                {isSelected && (
                  <span style={{ display: 'flex', alignItems: 'center', gap: '4px', background: '#eff6ff', color: '#2563eb', padding: '2px 6px', borderRadius: '4px', fontSize: '10px', fontWeight: 600 }}>
                    <Check size={12} /> Active
                  </span>
                )}
              </div>
              <div style={{ fontSize: '11px', color: '#64748b', display: 'flex', justifyContent: 'space-between' }}>
                <span>State Code: {c.state_code}</span>
                <span>Active Since {c.created_at.slice(0, 10)}</span>
              </div>
            </div>
          );
        })}
      </div>

      {/* Financial Years for Active Company */}
      {activeCompany && (
        <div>
          <h2 style={{ fontSize: '13px', fontWeight: 600, color: '#334155', marginBottom: '8px' }}>
            Financial Years for {activeCompany.name}
          </h2>
          <div className="table-container" style={{ margin: 0 }}>
            <table className="acc-table">
              <thead>
                <tr>
                  <th>FY Name</th>
                  <th>Start Date</th>
                  <th>End Date</th>
                  <th>Books Beginning</th>
                  <th>Status</th>
                  <th>Action</th>
                </tr>
              </thead>
              <tbody>
                {financialYears.length === 0 ? (
                  <tr>
                    <td colSpan={6} style={{ textAlign: 'center', color: '#94a3b8', padding: '16px' }}>
                      No financial years configured yet. Click "Add Financial Year" above.
                    </td>
                  </tr>
                ) : (
                  financialYears.map((fy) => {
                    const isFySelected = activeFy?.fy_id === fy.fy_id;
                    return (
                      <tr key={fy.fy_id} style={{ background: isFySelected ? '#f8fafc' : undefined }}>
                        <td style={{ fontWeight: 600 }}>{fy.fy_name}</td>
                        <td>{fy.start_date}</td>
                        <td>{fy.end_date}</td>
                        <td>{fy.books_beginning_date}</td>
                        <td>
                          {isFySelected ? (
                            <span style={{ color: '#16a34a', fontWeight: 600, fontSize: '11px' }}>● Selected</span>
                          ) : (
                            <span style={{ color: '#64748b', fontSize: '11px' }}>Inactive</span>
                          )}
                        </td>
                        <td>
                          {!isFySelected && (
                            <button
                              className="btn btn-secondary"
                              style={{ padding: '2px 8px', fontSize: '11px' }}
                              onClick={() => onSelectFy(fy)}
                            >
                              Select Period
                            </button>
                          )}
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Create Company Modal */}
      {showCreateCompany && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h3 style={{ fontSize: '14px', fontWeight: 600 }}>Create New Enterprise Company</h3>
              <button
                style={{ background: 'transparent', border: 'none', cursor: 'pointer', fontSize: '16px', color: '#64748b' }}
                onClick={() => setShowCreateCompany(false)}
              >
                ✕
              </button>
            </div>
            <form onSubmit={handleCreateCompany}>
              <div className="modal-body" style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Company Trade Name *
                  </label>
                  <input
                    type="text"
                    required
                    placeholder="e.g. Bharat Logistics Pvt Ltd"
                    className="input"
                    style={{ width: '100%' }}
                    value={compName}
                    onChange={(e) => setCompName(e.target.value)}
                  />
                </div>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    State of Registration (for GST Place of Supply)
                  </label>
                  <select
                    className="select"
                    style={{ width: '100%' }}
                    value={compState}
                    onChange={(e) => setCompState(e.target.value)}
                  >
                    {indianStates.map((s) => (
                      <option key={s.code} value={s.code}>
                        {s.name}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
              <div className="modal-footer">
                <button type="button" className="btn btn-secondary" onClick={() => setShowCreateCompany(false)}>
                  Cancel
                </button>
                <button type="submit" className="btn btn-primary" disabled={submitting}>
                  {submitting ? 'Creating...' : 'Initialize Company & 28 Groups'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Create FY Modal */}
      {showCreateFy && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h3 style={{ fontSize: '14px', fontWeight: 600 }}>Add Financial Accounting Year</h3>
              <button
                style={{ background: 'transparent', border: 'none', cursor: 'pointer', fontSize: '16px', color: '#64748b' }}
                onClick={() => setShowCreateFy(false)}
              >
                ✕
              </button>
            </div>
            <form onSubmit={handleCreateFy}>
              <div className="modal-body" style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Period Label *
                  </label>
                  <input
                    type="text"
                    required
                    placeholder="e.g. 2026-2027"
                    className="input"
                    style={{ width: '100%' }}
                    value={fyName}
                    onChange={(e) => setFyName(e.target.value)}
                  />
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      Start Date *
                    </label>
                    <input
                      type="date"
                      required
                      className="input"
                      style={{ width: '100%' }}
                      value={fyStart}
                      onChange={(e) => setFyStart(e.target.value)}
                    />
                  </div>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      End Date *
                    </label>
                    <input
                      type="date"
                      required
                      className="input"
                      style={{ width: '100%' }}
                      value={fyEnd}
                      onChange={(e) => setFyEnd(e.target.value)}
                    />
                  </div>
                </div>
              </div>
              <div className="modal-footer">
                <button type="button" className="btn btn-secondary" onClick={() => setShowCreateFy(false)}>
                  Cancel
                </button>
                <button type="submit" className="btn btn-primary" disabled={submitting}>
                  {submitting ? 'Creating...' : 'Create Financial Year'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
