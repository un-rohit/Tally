import React, { useEffect, useState } from 'react';
import { Layout } from './components/Layout';
import { DashboardView } from './views/DashboardView';
import { CompanyView } from './views/CompanyView';
import { LedgersView } from './views/LedgersView';
import { VouchersView } from './views/VouchersView';
import { GstView } from './views/GstView';
import { InventoryView } from './views/InventoryView';
import { BillWiseView } from './views/BillWiseView';
import { ReportsView } from './views/ReportsView';
import { SettingsView } from './views/SettingsView';
import { api } from './services/api';
import type { Company, FinancialYear } from './types';

export const App: React.FC = () => {
  const [currentTab, setCurrentTab] = useState('dashboard');
  const [companies, setCompanies] = useState<Company[]>([]);
  const [activeCompany, setActiveCompany] = useState<Company | null>(null);
  const [financialYears, setFinancialYears] = useState<FinancialYear[]>([]);
  const [activeFy, setActiveFy] = useState<FinancialYear | null>(null);
  const [loading, setLoading] = useState(true);

  const loadCompaniesAndFys = async () => {
    try {
      const compList = await api.getCompanies();
      setCompanies(compList);

      if (compList.length > 0) {
        const selected = activeCompany || compList[0];
        setActiveCompany(selected);

        const fyList = await api.getFinancialYears(selected.company_id);
        setFinancialYears(fyList);
        if (fyList.length > 0 && !activeFy) {
          setActiveFy(fyList[0]);
        }
      } else {
        // If no company exists on first run, direct user to companies setup
        setCurrentTab('companies');
      }
    } catch (err) {
      console.error('Initialization error:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadCompaniesAndFys();
  }, []);

  const handleSelectCompany = async (comp: Company) => {
    setActiveCompany(comp);
    try {
      const fyList = await api.getFinancialYears(comp.company_id);
      setFinancialYears(fyList);
      if (fyList.length > 0) {
        setActiveFy(fyList[0]);
      } else {
        setActiveFy(null);
      }
    } catch (err) {
      console.error('Failed to load FYs for company:', err);
    }
  };

  if (loading) {
    return (
      <div style={{ height: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#f8fafc', color: '#64748b', fontSize: '14px', fontFamily: 'var(--font-sans)' }}>
        Loading Accounting Engine...
      </div>
    );
  }

  return (
    <Layout
      currentTab={currentTab}
      onTabChange={setCurrentTab}
      companies={companies}
      activeCompany={activeCompany}
      onSelectCompany={handleSelectCompany}
      financialYears={financialYears}
      activeFy={activeFy}
      onSelectFy={setActiveFy}
    >
      {currentTab === 'dashboard' && (
        <DashboardView
          company={activeCompany}
          fy={activeFy}
          onNavigate={setCurrentTab}
        />
      )}
      {currentTab === 'companies' && (
        <CompanyView
          companies={companies}
          activeCompany={activeCompany}
          onSelectCompany={handleSelectCompany}
          financialYears={financialYears}
          activeFy={activeFy}
          onSelectFy={setActiveFy}
          onRefresh={loadCompaniesAndFys}
        />
      )}
      {currentTab === 'ledgers' && (
        <LedgersView company={activeCompany} />
      )}
      {currentTab === 'vouchers' && (
        <VouchersView company={activeCompany} fy={activeFy} />
      )}
      {currentTab === 'gst' && (
        <GstView company={activeCompany} fy={activeFy} />
      )}
      {currentTab === 'inventory' && (
        <InventoryView company={activeCompany} />
      )}
      {currentTab === 'billwise' && (
        <BillWiseView company={activeCompany} />
      )}
      {currentTab === 'reports' && (
        <ReportsView company={activeCompany} />
      )}
      {currentTab === 'settings' && (
        <SettingsView company={activeCompany} />
      )}
    </Layout>
  );
};
