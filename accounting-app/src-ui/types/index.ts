export interface Company {
  company_id: string;
  company_number: string;
  name: string;
  formal_name: string;
  state_code: string;
  is_active: boolean;
  created_at: string;
}

export interface FinancialYear {
  fy_id: string;
  company_id: string;
  fy_name: string;
  start_date: string;
  end_date: string;
  books_beginning_date: string;
  is_locked: boolean;
}

export interface AccountGroup {
  group_id: string;
  company_id: string;
  name: string;
  parent_group_id: string | null;
  nature: 'Asset' | 'Liability' | 'Income' | 'Expense';
  affects_gross_profit: boolean;
}

export interface Ledger {
  ledger_id: string;
  company_id: string;
  group_id: string;
  name: string;
  opening_balance_paise: number;
  current_balance_paise: number;
  is_bank_or_cash: boolean;
  is_active: boolean;
}

export interface VoucherEntryItem {
  ledger_id: string;
  ledger_name?: string;
  entry_type: 'Dr' | 'Cr';
  amount_paise: number;
  narration?: string;
}

export interface VoucherSummary {
  voucher_id: string;
  voucher_number: string;
  voucher_type: string;
  voucher_date: string;
  narration: string | null;
  status: 'Draft' | 'Posted' | 'Cancelled';
  total_amount_paise: number;
}

export interface StockItem {
  item_id: string;
  company_id: string;
  stock_group_id: string | null;
  category_id: string | null;
  uom_id: string;
  uom_symbol?: string;
  name: string;
  sku: string | null;
  description: string | null;
  hsn_sac_code: string | null;
  gst_rate_basis_points: number;
  valuation_method: 'FIFO' | 'WeightedAverage';
  opening_qty_milli: number;
  opening_rate_paise: number;
  opening_value_paise: number;
  reorder_level_milli: number;
  is_active: boolean;
}

export interface Uom {
  uom_id: string;
  company_id: string;
  symbol: string;
  formal_name: string | null;
  decimal_places: number;
  is_compound: boolean;
  base_uom_id: string | null;
  conversion_factor: number;
}

export interface Warehouse {
  warehouse_id: string;
  company_id: string;
  name: string;
  address: string | null;
  is_active: boolean;
}

export interface StockSummaryItem {
  item_id: string;
  item_name: string;
  sku: string | null;
  uom_symbol: string;
  valuation_method: string;
  opening_qty_milli: number;
  opening_value_paise: number;
  inward_qty_milli: number;
  inward_value_paise: number;
  outward_qty_milli: number;
  outward_value_paise: number;
  closing_qty_milli: number;
  closing_rate_paise: number;
  closing_value_paise: number;
}

export interface OutstandingBill {
  ledger_id: string;
  party_name: string;
  bill_ref: string;
  bill_date: string;
  due_date: string | null;
  credit_days: number;
  original_amount_paise: number;
  settled_amount_paise: number;
  outstanding_amount_paise: number;
  overdue_days: number;
  is_overdue: boolean;
}

export interface PartyAgingRow {
  ledger_id: string;
  party_name: string;
  current_paise: number;
  days_1_30_paise: number;
  days_31_60_paise: number;
  days_61_90_paise: number;
  over_90_paise: number;
  total_outstanding_paise: number;
}

export interface TrialBalanceItem {
  ledger_id: string;
  ledger_name: string;
  group_name: string;
  debit_paise: number;
  credit_paise: number;
}

export interface FinancialReportRow {
  group_name: string;
  nature: string;
  amount_paise: number;
}

export interface DashboardSummary {
  company_name: string;
  fy_name: string;
  total_sales_paise: number;
  total_purchases_paise: number;
  cash_bank_balance_paise: number;
  total_receivables_paise: number;
  total_payables_paise: number;
  inventory_valuation_paise: number;
  recent_vouchers: VoucherSummary[];
}
