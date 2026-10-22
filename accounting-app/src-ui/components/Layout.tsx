import React from 'react';
import {
  LayoutDashboard,
  Building2,
  BookOpen,
  Receipt,
  FileSpreadsheet,
  Package,
  CalendarClock,
  PieChart,
  Settings,
  Search,
  Database,
  CheckCircle2,
} from 'lucide-react';
import type { Company, FinancialYear } from '../types';

interface LayoutProps {
  currentTab: string;
  onTabChange: (tab: string) => void;
  companies: Company[];
  activeCompany: Company | null;
  onSelectCompany: (company: Company) => void;
  financialYears: FinancialYear[];
  activeFy: FinancialYear | null;
  onSelectFy: (fy: FinancialYear) => void;
  children: React.ReactNode;
}

export const Layout: React.FC<LayoutProps> = ({
  currentTab,
  onTabChange,
  companies,
  activeCompany,
  onSelectCompany,
  financialYears,
  activeFy,
  onSelectFy,
  children,
}) => {
  const navItems = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { id: 'companies', label: 'Companies & FY', icon: Building2 },
    { id: 'ledgers', label: 'Chart of Accounts', icon: BookOpen },
    { id: 'vouchers', label: 'Vouchers & Day Book', icon: Receipt },
    { id: 'gst', label: 'GST Statutory Portal', icon: FileSpreadsheet },
    { id: 'inventory', label: 'Inventory & Stock', icon: Package },
    { id: 'billwise', label: 'Outstanding & Ageing', icon: CalendarClock },
    { id: 'reports', label: 'Financial Statements', icon: PieChart },
    { id: 'settings', label: 'Settings & Backup', icon: Settings },
  ];

  return (
    <div className="app-container">
      {/* Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-header">
          <BookOpen className="text-blue-500" size={20} color="#3b82f6" />
          <div>
            <div className="brand-title">ACCOUNTING ENGINE</div>
            <div className="brand-badge">INDEPENDENT ERP</div>
          </div>
        </div>

        <nav className="sidebar-nav">
          <div className="nav-section-title">Operations</div>
          {navItems.slice(0, 4).map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                onClick={() => onTabChange(item.id)}
                className={`nav-item ${currentTab === item.id ? 'active' : ''}`}
              >
                <Icon size={16} />
                <span>{item.label}</span>
              </button>
            );
          })}

          <div className="nav-section-title">Compliance & Supply</div>
          {navItems.slice(4, 7).map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                onClick={() => onTabChange(item.id)}
                className={`nav-item ${currentTab === item.id ? 'active' : ''}`}
              >
                <Icon size={16} />
                <span>{item.label}</span>
              </button>
            );
          })}

          <div className="nav-section-title">Analysis & System</div>
          {navItems.slice(7).map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                onClick={() => onTabChange(item.id)}
                className={`nav-item ${currentTab === item.id ? 'active' : ''}`}
              >
                <Icon size={16} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
      </aside>

      {/* Main Content Area */}
      <div className="main-wrapper">
        {/* Topbar */}
        <header className="topbar">
          <div className="topbar-left">
            <div className="company-selector-badge">
              <Building2 size={14} className="text-slate-500" />
              <select
                className="select"
                style={{ border: 'none', background: 'transparent', fontWeight: 600, padding: 0 }}
                value={activeCompany?.company_id || ''}
                onChange={(e) => {
                  const comp = companies.find((c) => c.company_id === e.target.value);
                  if (comp) onSelectCompany(comp);
                }}
              >
                {companies.length === 0 ? (
                  <option value="">No Company Configured</option>
                ) : (
                  companies.map((c) => (
                    <option key={c.company_id} value={c.company_id}>
                      {c.name} ({c.company_number})
                    </option>
                  ))
                )}
              </select>
            </div>

            {activeFy && (
              <div className="fy-badge">
                FY: {activeFy.fy_name}
              </div>
            )}
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
            <div style={{ position: 'relative' }}>
              <Search size={14} style={{ position: 'absolute', left: 8, top: 7, color: '#94a3b8' }} />
              <input
                type="text"
                placeholder="Search ledger, voucher, item..."
                className="input"
                style={{ paddingLeft: '28px', width: '220px' }}
              />
            </div>
            <span style={{ fontSize: '11px', color: '#64748b' }}>
              {new Date().toLocaleDateString('en-IN', { weekday: 'short', day: '2-digit', month: 'short', year: 'numeric' })}
            </span>
          </div>
        </header>

        {/* Viewport Content */}
        <main className="content-viewport">
          {children}
        </main>

        {/* Status Bar */}
        <footer className="statusbar">
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <Database size={12} color="#16a34a" />
            <span>SQLite Engine: WAL Mode & Foreign Keys Active</span>
            <span style={{ color: '#cbd5e1' }}>|</span>
            <CheckCircle2 size={12} color="#2563eb" />
            <span>State: {activeCompany ? `Connected (${activeCompany.name})` : 'Awaiting Company Setup'}</span>
          </div>
          <div>
            <span>Checked Arithmetic (Paise) | Double-Entry Equilibrium Enforced</span>
          </div>
        </footer>
      </div>
    </div>
  );
};
