-- ============================================================================
-- Migration: V003__inventory_and_billwise.sql
-- Phase 3 Inventory & Phase 4 Bill-Wise Accounting
-- ============================================================================

-- 1. UNITS OF MEASURE (Simple & Compound)
CREATE TABLE IF NOT EXISTS units_of_measure (
    uom_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    formal_name TEXT,
    decimal_places INTEGER DEFAULT 0 CHECK (decimal_places BETWEEN 0 AND 4),
    is_compound INTEGER DEFAULT 0 CHECK (is_compound IN (0, 1)),
    base_uom_id TEXT REFERENCES units_of_measure(uom_id),
    conversion_factor INTEGER DEFAULT 1 CHECK (conversion_factor >= 1),
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, symbol)
);

-- 2. STOCK GROUPS & CATEGORIES
CREATE TABLE IF NOT EXISTS stock_groups (
    stock_group_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    parent_group_id TEXT REFERENCES stock_groups(stock_group_id),
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, name)
);

CREATE TABLE IF NOT EXISTS stock_categories (
    category_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, name)
);

-- 3. GODOWNS / WAREHOUSES
CREATE TABLE IF NOT EXISTS warehouses (
    warehouse_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    address TEXT,
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, name)
);

-- 4. STOCK ITEMS
CREATE TABLE IF NOT EXISTS stock_items (
    item_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    stock_group_id TEXT REFERENCES stock_groups(stock_group_id),
    category_id TEXT REFERENCES stock_categories(category_id),
    uom_id TEXT NOT NULL REFERENCES units_of_measure(uom_id),
    name TEXT NOT NULL,
    sku TEXT,
    description TEXT,
    hsn_sac_code TEXT,
    gst_rate_basis_points INTEGER DEFAULT 1800 CHECK (gst_rate_basis_points >= 0),
    valuation_method TEXT DEFAULT 'FIFO' CHECK (valuation_method IN ('FIFO', 'WeightedAverage')),
    opening_qty_milli INTEGER DEFAULT 0, -- Stored as milli-units (e.g. 1.000 kg = 1000)
    opening_rate_paise INTEGER DEFAULT 0,
    opening_value_paise INTEGER DEFAULT 0,
    reorder_level_milli INTEGER DEFAULT 0,
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, name)
);

-- 5. INVENTORY TRANSACTIONS (Stock Movements)
CREATE TABLE IF NOT EXISTS inventory_transactions (
    inv_tx_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    voucher_id TEXT NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    voucher_entry_id TEXT REFERENCES voucher_entries(entry_id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES stock_items(item_id),
    warehouse_id TEXT NOT NULL REFERENCES warehouses(warehouse_id),
    movement_type TEXT NOT NULL CHECK (movement_type IN ('IN', 'OUT', 'TRANSFER_IN', 'TRANSFER_OUT', 'ADJUSTMENT_IN', 'ADJUSTMENT_OUT')),
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli > 0),
    rate_paise INTEGER NOT NULL CHECK (rate_paise >= 0),
    amount_paise INTEGER NOT NULL CHECK (amount_paise >= 0),
    transaction_date TEXT NOT NULL,
    batch_number TEXT,
    expiry_date TEXT,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 6. COMPANY SETTINGS TABLE (Configurable negative stock, rounding, etc.)
CREATE TABLE IF NOT EXISTS company_settings (
    setting_id TEXT PRIMARY KEY,
    company_id TEXT UNIQUE NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    allow_negative_stock INTEGER DEFAULT 0 CHECK (allow_negative_stock IN (0, 1)),
    default_valuation_method TEXT DEFAULT 'FIFO' CHECK (default_valuation_method IN ('FIFO', 'WeightedAverage')),
    default_credit_period_days INTEGER DEFAULT 30,
    updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 7. BILL ALLOCATIONS ENHANCEMENTS
-- Add company_id and indexes to bill_allocations for fast receivables/payables tracking
ALTER TABLE bill_allocations ADD COLUMN company_id TEXT REFERENCES companies(company_id) ON DELETE CASCADE;

-- INDEXES FOR INVENTORY & BILL-WISE LOOKUPS
CREATE INDEX IF NOT EXISTS idx_inv_tx_lookup ON inventory_transactions(company_id, item_id, warehouse_id, transaction_date);
CREATE INDEX IF NOT EXISTS idx_inv_tx_voucher ON inventory_transactions(voucher_id);
CREATE INDEX IF NOT EXISTS idx_stock_items_company ON stock_items(company_id, is_active);
CREATE INDEX IF NOT EXISTS idx_bill_alloc_party_ref ON bill_allocations(company_id, ledger_id, ref_name);
CREATE INDEX IF NOT EXISTS idx_bill_alloc_due_date ON bill_allocations(due_date);

-- IMMUTABILITY TRIGGERS FOR INVENTORY
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_inv_tx_insert
BEFORE INSERT ON inventory_transactions
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = NEW.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot add inventory movements to an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_inv_tx_update
BEFORE UPDATE ON inventory_transactions
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = OLD.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot modify inventory movements of an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_inv_tx_delete
BEFORE DELETE ON inventory_transactions
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = OLD.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot delete inventory movements of an already posted voucher.');
END;
