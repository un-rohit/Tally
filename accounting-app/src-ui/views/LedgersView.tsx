import React, { useEffect, useState } from 'react';
import { BookOpen, Plus, Folder, AlertCircle } from 'lucide-react';
import { api, formatINR } from '../services/api';
import type { AccountGroup, Company, Ledger } from '../types';

interface LedgersViewProps {
  company: Company | null;
}

export const LedgersView: React.FC<LedgersViewProps> = ({ company }) => {
  const [groups, setGroups] = useState<AccountGroup[]>([]);
  const [ledgers, setLedgers] = useState<Ledger[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedGroup, setSelectedGroup] = useState<string>('all');
  const [showCreateModal, setShowCreateModal] = useState(false);

  // Form states
  const [ledgerName, setLedgerName] = useState('');
  const [groupId, setGroupId] = useState('');
  const [openingBalRupees, setOpeningBalRupees] = useState('0');
  const [isBankOrCash, setIsBankOrCash] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const loadData = () => {
    if (!company) return;
    setLoading(true);
    setError(null);
    Promise.all([api.getAccountGroups(company.company_id), api.getLedgers(company.company_id)])
      .then(([g, l]) => {
        setGroups(g);
        setLedgers(l);
        if (g.length > 0 && !groupId) {
          setGroupId(g[0].group_id);
        }
        setLoading(false);
      })
      .catch((err) => {
        setError(err?.toString() || 'Failed to load chart of accounts');
        setLoading(false);
      });
  };

  useEffect(() => {
    loadData();
  }, [company]);

  const handleCreateLedger = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!company || !ledgerName.trim() || !groupId) return;
    setSubmitting(true);
    setError(null);
    try {
      const opPaise = Math.round(parseFloat(openingBalRupees || '0') * 100);
      await api.createLedger(company.company_id, groupId, ledgerName.trim(), opPaise, isBankOrCash);
      setLedgerName('');
      setOpeningBalRupees('0');
      setIsBankOrCash(false);
      setShowCreateModal(false);
      loadData();
    } catch (err: any) {
      setError(err?.toString() || 'Failed to create ledger');
    } finally {
      setSubmitting(false);
    }
  };

  const filteredLedgers = selectedGroup === 'all'
    ? ledgers
    : ledgers.filter((l) => l.group_id === selectedGroup);

  if (!company) {
    return <div className="card">Please select an active company first.</div>;
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <div>
          <h1 style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
            Chart of Accounts & Ledgers
          </h1>
          <p style={{ fontSize: '12px', color: '#64748b' }}>
            Standard 28-Group hierarchy compliant with Indian Accounting Standards and double-entry rules
          </p>
        </div>
        <button className="btn btn-primary" onClick={() => setShowCreateModal(true)}>
          <Plus size={14} /> New Ledger
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
        <span style={{ fontSize: '12px', fontWeight: 600, color: '#475569' }}>Filter by Group:</span>
        <select
          className="select"
          style={{ minWidth: '220px' }}
          value={selectedGroup}
          onChange={(e) => setSelectedGroup(e.target.value)}
        >
          <option value="all">All Groups ({groups.length} groups, {ledgers.length} ledgers)</option>
          {groups.map((g) => (
            <option key={g.group_id} value={g.group_id}>
              {g.name} ({g.nature})
            </option>
          ))}
        </select>
      </div>

      {/* Ledgers Table */}
      <div className="table-container" style={{ margin: 0 }}>
        <table className="acc-table">
          <thead>
            <tr>
              <th>Ledger Name</th>
              <th>Parent Account Group</th>
              <th>Nature</th>
              <th>Bank/Cash Flag</th>
              <th className="text-right">Opening Balance</th>
              <th className="text-right">Current Balance</th>
            </tr>
          </thead>
          <tbody>
            {filteredLedgers.length === 0 ? (
              <tr>
                <td colSpan={6} style={{ textAlign: 'center', color: '#94a3b8', padding: '24px' }}>
                  No ledgers found for this filter.
                </td>
              </tr>
            ) : (
              filteredLedgers.map((l) => {
                const grp = groups.find((g) => g.group_id === l.group_id);
                return (
                  <tr key={l.ledger_id}>
                    <td style={{ fontWeight: 600 }}>{l.name}</td>
                    <td>
                      <span style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Folder size={12} color="#64748b" />
                        {grp?.name || '—'}
                      </span>
                    </td>
                    <td>
                      <span style={{
                        fontSize: '10px',
                        fontWeight: 600,
                        padding: '2px 6px',
                        borderRadius: '4px',
                        background: grp?.nature === 'Asset' ? '#eff6ff' : grp?.nature === 'Liability' ? '#fff7ed' : grp?.nature === 'Income' ? '#f0fdf4' : '#fef2f2',
                        color: grp?.nature === 'Asset' ? '#2563eb' : grp?.nature === 'Liability' ? '#c2410c' : grp?.nature === 'Income' ? '#16a34a' : '#dc2626',
                      }}>
                        {grp?.nature}
                      </span>
                    </td>
                    <td>{l.is_bank_or_cash ? '✓ Liquid' : 'No'}</td>
                    <td className="text-right">{formatINR(l.opening_balance_paise)}</td>
                    <td className="text-right" style={{ fontWeight: 600 }}>
                      {formatINR(l.current_balance_paise)}
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

      {/* Create Ledger Modal */}
      {showCreateModal && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h3 style={{ fontSize: '14px', fontWeight: 600 }}>Create New Ledger Account</h3>
              <button
                style={{ background: 'transparent', border: 'none', cursor: 'pointer', fontSize: '16px', color: '#64748b' }}
                onClick={() => setShowCreateModal(false)}
              >
                ✕
              </button>
            </div>
            <form onSubmit={handleCreateLedger}>
              <div className="modal-body" style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Ledger Account Name *
                  </label>
                  <input
                    type="text"
                    required
                    placeholder="e.g. Reliance Industries Ltd, Office Rent, HDFC Salary A/c"
                    className="input"
                    style={{ width: '100%' }}
                    value={ledgerName}
                    onChange={(e) => setLedgerName(e.target.value)}
                  />
                </div>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Under Group *
                  </label>
                  <select
                    className="select"
                    style={{ width: '100%' }}
                    value={groupId}
                    onChange={(e) => setGroupId(e.target.value)}
                  >
                    {groups.map((g) => (
                      <option key={g.group_id} value={g.group_id}>
                        {g.name} ({g.nature})
                      </option>
                    ))}
                  </select>
                </div>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Opening Balance (₹)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    className="input"
                    style={{ width: '100%' }}
                    value={openingBalRupees}
                    onChange={(e) => setOpeningBalRupees(e.target.value)}
                  />
                </div>
                <div>
                  <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px', cursor: 'pointer' }}>
                    <input
                      type="checkbox"
                      checked={isBankOrCash}
                      onChange={(e) => setIsBankOrCash(e.target.checked)}
                    />
                    <span>Is Bank or Cash Account (Liquid Asset for contra and payments)</span>
                  </label>
                </div>
              </div>
              <div className="modal-footer">
                <button type="button" className="btn btn-secondary" onClick={() => setShowCreateModal(false)}>
                  Cancel
                </button>
                <button type="submit" className="btn btn-primary" disabled={submitting}>
                  {submitting ? 'Creating...' : 'Create Ledger'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
