import { invoke } from '@tauri-apps/api/core';
import type {
  AccountGroup,
  Company,
  DashboardSummary,
  FinancialReportRow,
  FinancialYear,
  Ledger,
  OutstandingBill,
  PartyAgingRow,
  StockItem,
  StockSummaryItem,
  TrialBalanceItem,
  Uom,
  VoucherSummary,
  Warehouse,
} from '../types';

export const api = {
  // Companies
  getCompanies: () => invoke<Company[]>('get_companies'),
  createCompany: (name: string, state_code: string) =>
    invoke<Company>('create_company', { name, stateCode: state_code }),
  getFinancialYears: (companyId: string) =>
    invoke<FinancialYear[]>('get_financial_years', { companyId }),
  createFinancialYear: (companyId: string, fyName: string, startDate: string, endDate: string) =>
    invoke<FinancialYear>('create_financial_year', { companyId, fyName, startDate, endDate }),

  // Chart of accounts
  getAccountGroups: (companyId: string) =>
    invoke<AccountGroup[]>('get_account_groups', { companyId }),
  getLedgers: (companyId: string) =>
    invoke<Ledger[]>('get_ledgers', { companyId }),
  createLedger: (companyId: string, groupId: string, name: string, openingBalancePaise: number, isBankOrCash: boolean) =>
    invoke<Ledger>('create_ledger', {
      companyId,
      groupId,
      name,
      openingBalancePaise,
      isBankOrCash,
    }),

  // Vouchers
  getVouchers: (companyId: string, fyId?: string) =>
    invoke<VoucherSummary[]>('get_vouchers', { companyId, fyId }),
  postVoucher: (draft: any) =>
    invoke<VoucherSummary>('post_voucher', { draft }),
  cancelVoucher: (voucherId: string, reason: string) =>
    invoke<void>('cancel_voucher', { voucherId, reason }),

  // Reports
  getTrialBalance: (companyId: string, asOfDate?: string) =>
    invoke<{ items: TrialBalanceItem[]; total_debit_paise: number; total_credit_paise: number; is_balanced: boolean }>(
      'get_trial_balance',
      { companyId, asOfDate }
    ),
  getProfitAndLoss: (companyId: string, fromDate?: string, toDate?: string) =>
    invoke<{
      income_rows: FinancialReportRow[];
      expense_rows: FinancialReportRow[];
      gross_profit_paise: number;
      net_profit_paise: number;
    }>('get_profit_and_loss', { companyId, fromDate, toDate }),
  getBalanceSheet: (companyId: string, asOfDate?: string) =>
    invoke<{
      asset_rows: FinancialReportRow[];
      liability_rows: FinancialReportRow[];
      total_assets_paise: number;
      total_liabilities_paise: number;
      difference_paise: number;
    }>('get_balance_sheet', { companyId, asOfDate }),

  // Inventory
  getStockItems: (companyId: string) =>
    invoke<StockItem[]>('get_stock_items', { companyId }),
  createStockItem: (req: any) =>
    invoke<StockItem>('create_stock_item', { req }),
  getUoms: (companyId: string) =>
    invoke<Uom[]>('get_uoms', { companyId }),
  createUom: (companyId: string, symbol: string, formalName?: string, decimalPlaces?: number) =>
    invoke<Uom>('create_uom', { companyId, symbol, formalName, decimalPlaces }),
  getWarehouses: (companyId: string) =>
    invoke<Warehouse[]>('get_warehouses', { companyId }),
  createWarehouse: (companyId: string, name: string, address?: string) =>
    invoke<Warehouse>('create_warehouse', { companyId, name, address }),
  getStockSummary: (companyId: string, asOfDate?: string) =>
    invoke<StockSummaryItem[]>('get_stock_summary', { companyId, asOfDate }),

  // Bill-Wise
  getOutstandingBills: (companyId: string, ledgerId: string, asOfDate?: string) =>
    invoke<OutstandingBill[]>('get_outstanding_bills', { companyId, ledgerId, asOfDate }),
  getAgingReport: (companyId: string, isReceivable: boolean, asOfDate?: string) =>
    invoke<PartyAgingRow[]>('get_aging_report', { companyId, isReceivable, asOfDate }),

  // Dashboard
  getDashboardSummary: (companyId: string, fyId: string) =>
    invoke<DashboardSummary>('get_dashboard_summary', { companyId, fyId }),
};

export function formatINR(paise: number): string {
  const isNeg = paise < 0;
  const abs = Math.abs(paise);
  const rupees = Math.floor(abs / 100);
  const remainder = abs % 100;
  const paiseStr = remainder.toString().padStart(2, '0');

  const rStr = rupees.toString();
  let formattedRupees = '';
  if (rStr.length <= 3) {
    formattedRupees = rStr;
  } else {
    const lastThree = rStr.slice(-3);
    const remaining = rStr.slice(0, -3);
    const groups: string[] = [];
    let rem = remaining;
    while (rem.length > 2) {
      groups.push(rem.slice(-2));
      rem = rem.slice(0, -2);
    }
    if (rem.length > 0) groups.push(rem);
    groups.reverse();
    formattedRupees = groups.join(',') + ',' + lastThree;
  }

  return `${isNeg ? '-' : ''}₹${formattedRupees}.${paiseStr}`;
}
