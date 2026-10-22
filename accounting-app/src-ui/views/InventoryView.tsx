import React, { useEffect, useState } from 'react';
import { Package, Plus, MapPin, Scale, AlertCircle } from 'lucide-react';
import { api, formatINR } from '../services/api';
import type { Company, StockItem, StockSummaryItem, Uom, Warehouse } from '../types';

interface InventoryViewProps {
  company: Company | null;
}

export const InventoryView: React.FC<InventoryViewProps> = ({ company }) => {
  const [activeTab, setActiveTab] = useState<'summary' | 'items' | 'uoms' | 'godowns'>('summary');
  const [items, setItems] = useState<StockItem[]>([]);
  const [uoms, setUoms] = useState<Uom[]>([]);
  const [warehouses, setWarehouses] = useState<Warehouse[]>([]);
  const [summary, setSummary] = useState<StockSummaryItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // New Item modal
  const [showItemModal, setShowItemModal] = useState(false);
  const [showUomModal, setShowUomModal] = useState(false);
  const [showWhModal, setShowWhModal] = useState(false);

  // Form states - item
  const [itemName, setItemName] = useState('');
  const [itemSku, setItemSku] = useState('');
  const [itemUomId, setItemUomId] = useState('');
  const [itemHsn, setItemHsn] = useState('');
  const [itemGstBps, setItemGstBps] = useState(1800);
  const [itemValMethod, setItemValMethod] = useState<'FIFO' | 'WeightedAverage'>('FIFO');
  const [itemOpQty, setItemOpQty] = useState('0');
  const [itemOpRate, setItemOpRate] = useState('0');

  // Form states - uom
  const [uomSymbol, setUomSymbol] = useState('');
  const [uomFormal, setUomFormal] = useState('');
  const [uomDecimals, setUomDecimals] = useState(0);

  // Form states - warehouse
  const [whName, setWhName] = useState('');
  const [whAddress, setWhAddress] = useState('');

  const [submitting, setSubmitting] = useState(false);

  const loadData = () => {
    if (!company) return;
    setLoading(true);
    setError(null);
    Promise.all([
      api.getStockItems(company.company_id),
      api.getUoms(company.company_id),
      api.getWarehouses(company.company_id),
      api.getStockSummary(company.company_id),
    ])
      .then(([i, u, w, s]) => {
        setItems(i);
        setUoms(u);
        setWarehouses(w);
        setSummary(s);
        if (u.length > 0 && !itemUomId) {
          setItemUomId(u[0].uom_id);
        }
        setLoading(false);
      })
      .catch((err) => {
        setError(err?.toString() || 'Failed to load inventory data');
        setLoading(false);
      });
  };

  useEffect(() => {
    loadData();
  }, [company]);

  const handleCreateItem = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!company || !itemName.trim() || !itemUomId) return;
    setSubmitting(true);
    setError(null);
    try {
      const opQtyMilli = Math.round(parseFloat(itemOpQty || '0') * 1000);
      const opRatePaise = Math.round(parseFloat(itemOpRate || '0') * 100);

      await api.createStockItem({
        company_id: company.company_id,
        stock_group_id: null,
        category_id: null,
        uom_id: itemUomId,
        name: itemName.trim(),
        sku: itemSku.trim() || null,
        description: null,
        hsn_sac_code: itemHsn.trim() || null,
        gst_rate_basis_points: itemGstBps,
        valuation_method: itemValMethod,
        opening_qty_milli: opQtyMilli,
        opening_rate_paise: opRatePaise,
        reorder_level_milli: 0,
      });

      setItemName('');
      setItemSku('');
      setItemOpQty('0');
      setItemOpRate('0');
      setShowItemModal(false);
      loadData();
    } catch (err: any) {
      setError(err?.toString() || 'Failed to create stock item');
    } finally {
      setSubmitting(false);
    }
  };

  const handleCreateUom = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!company || !uomSymbol.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      await api.createUom(company.company_id, uomSymbol.trim(), uomFormal.trim() || undefined, uomDecimals);
      setUomSymbol('');
      setUomFormal('');
      setShowUomModal(false);
      loadData();
    } catch (err: any) {
      setError(err?.toString() || 'Failed to create UOM');
    } finally {
      setSubmitting(false);
    }
  };

  const handleCreateWarehouse = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!company || !whName.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      await api.createWarehouse(company.company_id, whName.trim(), whAddress.trim() || undefined);
      setWhName('');
      setWhAddress('');
      setShowWhModal(false);
      loadData();
    } catch (err: any) {
      setError(err?.toString() || 'Failed to create godown');
    } finally {
      setSubmitting(false);
    }
  };

  if (!company) {
    return <div className="card">Please select an active company first.</div>;
  }

  const totalValuationPaise = summary.reduce((sum, item) => sum + item.closing_value_paise, 0);

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <div>
          <h1 style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
            Inventory Management & Stock Valuation
          </h1>
          <p style={{ fontSize: '12px', color: '#64748b' }}>
            Real-time FIFO & Weighted Average valuation, multi-godown locations, and units of measure
          </p>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button className="btn btn-secondary" onClick={() => setShowUomModal(true)}>
            <Scale size={14} /> Add UOM
          </button>
          <button className="btn btn-secondary" onClick={() => setShowWhModal(true)}>
            <MapPin size={14} /> Add Godown
          </button>
          <button className="btn btn-primary" onClick={() => setShowItemModal(true)}>
            <Plus size={14} /> New Stock Item
          </button>
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
          className={`btn ${activeTab === 'summary' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('summary')}
        >
          <Package size={14} /> Stock Summary & Valuation ({formatINR(totalValuationPaise)})
        </button>
        <button
          className={`btn ${activeTab === 'items' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('items')}
        >
          Master Items Catalog ({items.length})
        </button>
        <button
          className={`btn ${activeTab === 'uoms' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('uoms')}
        >
          Units of Measure ({uoms.length})
        </button>
        <button
          className={`btn ${activeTab === 'godowns' ? 'btn-primary' : 'btn-secondary'}`}
          style={{ borderRadius: '4px 4px 0 0', borderBottom: 'none' }}
          onClick={() => setActiveTab('godowns')}
        >
          Godowns / Locations ({warehouses.length})
        </button>
      </div>

      {/* Stock Summary Tab */}
      {activeTab === 'summary' && (
        <div className="card">
          <div className="table-container" style={{ margin: 0 }}>
            <table className="acc-table">
              <thead>
                <tr>
                  <th>Item Name</th>
                  <th>UOM</th>
                  <th>Method</th>
                  <th className="text-right">Opening Qty</th>
                  <th className="text-right">Inward</th>
                  <th className="text-right">Outward</th>
                  <th className="text-right">Closing Balance Qty</th>
                  <th className="text-right">Effective Rate</th>
                  <th className="text-right">Closing Valuation</th>
                </tr>
              </thead>
              <tbody>
                {summary.length === 0 ? (
                  <tr>
                    <td colSpan={9} style={{ textAlign: 'center', color: '#94a3b8', padding: '24px' }}>
                      No inventory items found. Click "New Stock Item" above to add items.
                    </td>
                  </tr>
                ) : (
                  summary.map((s) => (
                    <tr key={s.item_id}>
                      <td style={{ fontWeight: 600 }}>{s.item_name}</td>
                      <td>{s.uom_symbol}</td>
                      <td>
                        <span style={{ fontSize: '10px', background: '#f1f5f9', padding: '2px 5px', borderRadius: '4px', fontWeight: 600 }}>
                          {s.valuation_method}
                        </span>
                      </td>
                      <td className="text-right">{(s.opening_qty_milli / 1000).toFixed(2)}</td>
                      <td className="text-right" style={{ color: '#16a34a' }}>+{(s.inward_qty_milli / 1000).toFixed(2)}</td>
                      <td className="text-right" style={{ color: '#dc2626' }}>-{(s.outward_qty_milli / 1000).toFixed(2)}</td>
                      <td className="text-right" style={{ fontWeight: 700 }}>{(s.closing_qty_milli / 1000).toFixed(2)}</td>
                      <td className="text-right">{formatINR(s.closing_rate_paise)}</td>
                      <td className="text-right" style={{ fontWeight: 700, color: '#2563eb' }}>
                        {formatINR(s.closing_value_paise)}
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
              {summary.length > 0 && (
                <tfoot>
                  <tr style={{ background: '#f8fafc', fontWeight: 700 }}>
                    <td colSpan={8} style={{ textAlign: 'right', padding: '10px 12px' }}>
                      Total Inventory Valuation:
                    </td>
                    <td className="text-right" style={{ color: '#16a34a', fontSize: '13px', padding: '10px 12px' }}>
                      {formatINR(totalValuationPaise)}
                    </td>
                  </tr>
                </tfoot>
              )}
            </table>
          </div>
        </div>
      )}

      {/* Master Items Catalog */}
      {activeTab === 'items' && (
        <div className="card">
          <div className="table-container" style={{ margin: 0 }}>
            <table className="acc-table">
              <thead>
                <tr>
                  <th>Item Name</th>
                  <th>SKU</th>
                  <th>HSN / SAC</th>
                  <th>GST Rate</th>
                  <th>Valuation Costing</th>
                  <th>Base UOM</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {items.length === 0 ? (
                  <tr>
                    <td colSpan={7} style={{ textAlign: 'center', color: '#94a3b8', padding: '20px' }}>
                      No items registered yet.
                    </td>
                  </tr>
                ) : (
                  items.map((i) => (
                    <tr key={i.item_id}>
                      <td style={{ fontWeight: 600 }}>{i.name}</td>
                      <td style={{ fontFamily: 'var(--font-mono)' }}>{i.sku || '—'}</td>
                      <td>{i.hsn_sac_code || '—'}</td>
                      <td>{(i.gst_rate_basis_points / 100).toFixed(1)}%</td>
                      <td>{i.valuation_method}</td>
                      <td>{uoms.find((u) => u.uom_id === i.uom_id)?.symbol || i.uom_id}</td>
                      <td>
                        <span style={{ color: i.is_active ? '#16a34a' : '#64748b', fontSize: '11px', fontWeight: 600 }}>
                          {i.is_active ? 'Active' : 'Deactivated'}
                        </span>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* UOMs Tab */}
      {activeTab === 'uoms' && (
        <div className="card">
          <div className="table-container" style={{ margin: 0 }}>
            <table className="acc-table">
              <thead>
                <tr>
                  <th>Symbol</th>
                  <th>Formal Name</th>
                  <th>Decimal Places</th>
                  <th>Compound Unit?</th>
                  <th>Conversion Factor</th>
                </tr>
              </thead>
              <tbody>
                {uoms.map((u) => (
                  <tr key={u.uom_id}>
                    <td style={{ fontWeight: 600 }}>{u.symbol}</td>
                    <td>{u.formal_name || '—'}</td>
                    <td>{u.decimal_places}</td>
                    <td>{u.is_compound ? 'Yes (Compound)' : 'No (Simple)'}</td>
                    <td>{u.is_compound ? `1 ${u.symbol} = ${u.conversion_factor} ${uoms.find(b => b.uom_id === u.base_uom_id)?.symbol || 'Base'}` : '1'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Godowns Tab */}
      {activeTab === 'godowns' && (
        <div className="card">
          <div className="table-container" style={{ margin: 0 }}>
            <table className="acc-table">
              <thead>
                <tr>
                  <th>Godown / Warehouse Name</th>
                  <th>Location Address</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {warehouses.map((w) => (
                  <tr key={w.warehouse_id}>
                    <td style={{ fontWeight: 600 }}>{w.name}</td>
                    <td>{w.address || '—'}</td>
                    <td>
                      <span style={{ color: w.is_active ? '#16a34a' : '#64748b', fontSize: '11px', fontWeight: 600 }}>
                        {w.is_active ? 'Active' : 'Inactive'}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Create Item Modal */}
      {showItemModal && (
        <div className="modal-overlay">
          <div className="modal-content" style={{ maxWidth: '640px' }}>
            <div className="modal-header">
              <h3 style={{ fontSize: '14px', fontWeight: 600 }}>Create New Stock Item</h3>
              <button
                style={{ background: 'transparent', border: 'none', cursor: 'pointer', fontSize: '16px', color: '#64748b' }}
                onClick={() => setShowItemModal(false)}
              >
                ✕
              </button>
            </div>
            <form onSubmit={handleCreateItem}>
              <div className="modal-body" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: '12px' }}>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      Stock Item Name *
                    </label>
                    <input
                      type="text"
                      required
                      placeholder="e.g. Copper Wire 2.5mm"
                      className="input"
                      style={{ width: '100%' }}
                      value={itemName}
                      onChange={(e) => setItemName(e.target.value)}
                    />
                  </div>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      SKU / Item Code
                    </label>
                    <input
                      type="text"
                      placeholder="CW-25"
                      className="input"
                      style={{ width: '100%' }}
                      value={itemSku}
                      onChange={(e) => setItemSku(e.target.value)}
                    />
                  </div>
                </div>

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '12px' }}>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      Base Unit (UOM) *
                    </label>
                    <select
                      className="select"
                      style={{ width: '100%' }}
                      value={itemUomId}
                      onChange={(e) => setItemUomId(e.target.value)}
                    >
                      {uoms.map((u) => (
                        <option key={u.uom_id} value={u.uom_id}>
                          {u.symbol} ({u.formal_name || u.symbol})
                        </option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      HSN / SAC Code
                    </label>
                    <input
                      type="text"
                      placeholder="e.g. 7408"
                      className="input"
                      style={{ width: '100%' }}
                      value={itemHsn}
                      onChange={(e) => setItemHsn(e.target.value)}
                    />
                  </div>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      Valuation Costing
                    </label>
                    <select
                      className="select"
                      style={{ width: '100%' }}
                      value={itemValMethod}
                      onChange={(e) => setItemValMethod(e.target.value as any)}
                    >
                      <option value="FIFO">FIFO (First In First Out)</option>
                      <option value="WeightedAverage">Weighted Average</option>
                    </select>
                  </div>
                </div>

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      Opening Stock Quantity
                    </label>
                    <input
                      type="number"
                      step="0.001"
                      className="input"
                      style={{ width: '100%' }}
                      value={itemOpQty}
                      onChange={(e) => setItemOpQty(e.target.value)}
                    />
                  </div>
                  <div>
                    <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                      Opening Rate per Unit (₹)
                    </label>
                    <input
                      type="number"
                      step="0.01"
                      className="input"
                      style={{ width: '100%' }}
                      value={itemOpRate}
                      onChange={(e) => setItemOpRate(e.target.value)}
                    />
                  </div>
                </div>
              </div>
              <div className="modal-footer">
                <button type="button" className="btn btn-secondary" onClick={() => setShowItemModal(false)}>
                  Cancel
                </button>
                <button type="submit" className="btn btn-primary" disabled={submitting}>
                  {submitting ? 'Creating...' : 'Register Stock Item'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Create UOM Modal */}
      {showUomModal && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h3 style={{ fontSize: '14px', fontWeight: 600 }}>Create Unit of Measure</h3>
              <button
                style={{ background: 'transparent', border: 'none', cursor: 'pointer', fontSize: '16px', color: '#64748b' }}
                onClick={() => setShowUomModal(false)}
              >
                ✕
              </button>
            </div>
            <form onSubmit={handleCreateUom}>
              <div className="modal-body" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Symbol *
                  </label>
                  <input
                    type="text"
                    required
                    placeholder="e.g. Kg, Pcs, Ltr, Mtr"
                    className="input"
                    style={{ width: '100%' }}
                    value={uomSymbol}
                    onChange={(e) => setUomSymbol(e.target.value)}
                  />
                </div>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Formal Name
                  </label>
                  <input
                    type="text"
                    placeholder="e.g. Kilograms"
                    className="input"
                    style={{ width: '100%' }}
                    value={uomFormal}
                    onChange={(e) => setUomFormal(e.target.value)}
                  />
                </div>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Decimal Places (0 to 4)
                  </label>
                  <input
                    type="number"
                    min="0"
                    max="4"
                    className="input"
                    style={{ width: '100%' }}
                    value={uomDecimals}
                    onChange={(e) => setUomDecimals(parseInt(e.target.value) || 0)}
                  />
                </div>
              </div>
              <div className="modal-footer">
                <button type="button" className="btn btn-secondary" onClick={() => setShowUomModal(false)}>
                  Cancel
                </button>
                <button type="submit" className="btn btn-primary" disabled={submitting}>
                  Save UOM
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Create Godown Modal */}
      {showWhModal && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h3 style={{ fontSize: '14px', fontWeight: 600 }}>Create Godown / Warehouse</h3>
              <button
                style={{ background: 'transparent', border: 'none', cursor: 'pointer', fontSize: '16px', color: '#64748b' }}
                onClick={() => setShowWhModal(false)}
              >
                ✕
              </button>
            </div>
            <form onSubmit={handleCreateWarehouse}>
              <div className="modal-body" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Godown Name *
                  </label>
                  <input
                    type="text"
                    required
                    placeholder="e.g. Central Warehouse Bhiwandi"
                    className="input"
                    style={{ width: '100%' }}
                    value={whName}
                    onChange={(e) => setWhName(e.target.value)}
                  />
                </div>
                <div>
                  <label style={{ display: 'block', fontSize: '11px', fontWeight: 600, marginBottom: '4px' }}>
                    Location / Address
                  </label>
                  <input
                    type="text"
                    placeholder="City, Industrial Area"
                    className="input"
                    style={{ width: '100%' }}
                    value={whAddress}
                    onChange={(e) => setWhAddress(e.target.value)}
                  />
                </div>
              </div>
              <div className="modal-footer">
                <button type="button" className="btn btn-secondary" onClick={() => setShowWhModal(false)}>
                  Cancel
                </button>
                <button type="submit" className="btn btn-primary" disabled={submitting}>
                  Save Godown
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
