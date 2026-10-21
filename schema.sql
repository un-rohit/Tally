-- ============================================================================
-- Clean-Room Accounting Database Schema (SQLite 3)
-- Standard: Double-Entry Invariance, Decimal-Safe Arithmetic, Audit Immutability
-- ============================================================================

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

-- ============================================================================
-- 1. COMPANIES & FINANCIAL YEARS
-- ============================================================================

CREATE TABLE IF NOT EXISTS companies (
    company_id TEXT PRIMARY KEY, -- UUID v4
    company_number TEXT UNIQUE NOT NULL, -- e.g. '10001'
    name TEXT NOT NULL,
    mailing_name TEXT NOT NULL,
    address TEXT,
    state_code TEXT NOT NULL, -- e.g. '27' for Maharashtra
    country TEXT DEFAULT 'India',
    pincode TEXT,
    telephone TEXT,
    email TEXT,
    pan TEXT,
    cin TEXT,
    currency_symbol TEXT DEFAULT '₹',
    currency_formal_name TEXT DEFAULT 'INR',
    decimal_places INTEGER DEFAULT 2,
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS financial_years (
    fy_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    fy_start_date TEXT NOT NULL, -- YYYY-MM-DD
    fy_end_date TEXT NOT NULL,   -- YYYY-MM-DD
    books_beginning_date TEXT NOT NULL, -- YYYY-MM-DD
    is_closed INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (books_beginning_date >= fy_start_date),
    CHECK (fy_end_date > fy_start_date),
    UNIQUE (company_id, fy_start_date)
);

-- ============================================================================
-- 2. CHART OF ACCOUNTS (GROUPS & LEDGERS)
-- ============================================================================

CREATE TABLE IF NOT EXISTS account_groups (
    group_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    parent_group_id TEXT REFERENCES account_groups(group_id),
    nature TEXT NOT NULL CHECK (nature IN ('Asset', 'Liability', 'Income', 'Expense')),
    affects_gross_profit INTEGER DEFAULT 0, -- 1 for Trading A/c (Direct), 0 for P&L (Indirect)
    is_predefined INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, name)
);

CREATE TABLE IF NOT EXISTS ledgers (
    ledger_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES account_groups(group_id),
    name TEXT NOT NULL,
    code TEXT,
    opening_balance_paise INTEGER DEFAULT 0, -- Stored as integer paise (₹1.00 = 100 paise)
    opening_balance_type TEXT CHECK (opening_balance_type IN ('Dr', 'Cr', NULL)),
    is_bill_wise INTEGER DEFAULT 0, -- 1 = Maintain balances bill-by-bill
    credit_period_days INTEGER DEFAULT 0,
    pan TEXT,
    gstin TEXT,
    mailing_name TEXT,
    address TEXT,
    state_code TEXT,
    pincode TEXT,
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, name)
);

-- ============================================================================
-- 3. VOUCHER TYPES, VOUCHERS & ENTRIES
-- ============================================================================

CREATE TABLE IF NOT EXISTS voucher_types (
    voucher_type_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    base_type TEXT NOT NULL CHECK (base_type IN (
        'Contra', 'Payment', 'Receipt', 'Journal', 
        'Sales', 'Purchase', 'CreditNote', 'DebitNote', 'StockJournal'
    )),
    numbering_method TEXT DEFAULT 'Automatic' CHECK (numbering_method IN ('Automatic', 'Manual')),
    prefix TEXT DEFAULT '',
    suffix TEXT DEFAULT '',
    starting_number INTEGER DEFAULT 1,
    is_active INTEGER DEFAULT 1,
    UNIQUE (company_id, name)
);

CREATE TABLE IF NOT EXISTS vouchers (
    voucher_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    fy_id TEXT NOT NULL REFERENCES financial_years(fy_id),
    voucher_type_id TEXT NOT NULL REFERENCES voucher_types(voucher_type_id),
    voucher_number TEXT NOT NULL,
    voucher_date TEXT NOT NULL, -- YYYY-MM-DD
    reference_no TEXT,
    reference_date TEXT, -- YYYY-MM-DD
    narration TEXT,
    total_amount_paise INTEGER NOT NULL CHECK (total_amount_paise >= 0),
    status TEXT DEFAULT 'Draft' CHECK (status IN ('Draft', 'Posted', 'Cancelled')),
    posted_at TEXT,
    posted_by_user_id TEXT,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, fy_id, voucher_type_id, voucher_number)
);

CREATE TABLE IF NOT EXISTS voucher_entries (
    entry_id TEXT PRIMARY KEY, -- UUID v4
    voucher_id TEXT NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    line_number INTEGER NOT NULL,
    ledger_id TEXT NOT NULL REFERENCES ledgers(ledger_id),
    entry_type TEXT NOT NULL CHECK (entry_type IN ('Dr', 'Cr')),
    amount_paise INTEGER NOT NULL CHECK (amount_paise > 0),
    line_narration TEXT,
    UNIQUE (voucher_id, line_number)
);

CREATE TABLE IF NOT EXISTS bill_allocations (
    allocation_id TEXT PRIMARY KEY, -- UUID v4
    entry_id TEXT NOT NULL REFERENCES voucher_entries(entry_id) ON DELETE CASCADE,
    ledger_id TEXT NOT NULL REFERENCES ledgers(ledger_id),
    ref_type TEXT NOT NULL CHECK (ref_type IN ('NewRef', 'AgstRef', 'Advance', 'OnAccount')),
    ref_name TEXT NOT NULL,
    credit_days INTEGER DEFAULT 0,
    due_date TEXT,
    amount_paise INTEGER NOT NULL CHECK (amount_paise > 0),
    entry_type TEXT NOT NULL CHECK (entry_type IN ('Dr', 'Cr'))
);

CREATE TABLE IF NOT EXISTS sales_purchase_details (
    detail_id TEXT PRIMARY KEY, -- UUID v4
    voucher_id TEXT UNIQUE NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    party_ledger_id TEXT NOT NULL REFERENCES ledgers(ledger_id),
    place_of_supply TEXT NOT NULL, -- 2-digit State Code
    buyer_name TEXT,
    buyer_address TEXT,
    buyer_gstin TEXT,
    is_interstate INTEGER NOT NULL,
    is_reverse_charge INTEGER DEFAULT 0,
    dispatch_doc_no TEXT,
    dispatched_through TEXT,
    destination TEXT,
    carrier_vehicle_no TEXT
);

-- ============================================================================
-- 4. INVENTORY (UNITS, GROUPS, WAREHOUSES, ITEMS & MOVEMENTS)
-- ============================================================================

CREATE TABLE IF NOT EXISTS units_of_measure (
    uom_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    formal_name TEXT,
    decimal_places INTEGER DEFAULT 0 CHECK (decimal_places BETWEEN 0 AND 4),
    is_compound INTEGER DEFAULT 0,
    base_uom_id TEXT REFERENCES units_of_measure(uom_id),
    conversion_factor INTEGER DEFAULT 1,
    UNIQUE (company_id, symbol)
);

CREATE TABLE IF NOT EXISTS stock_groups (
    stock_group_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    parent_group_id TEXT REFERENCES stock_groups(stock_group_id),
    UNIQUE (company_id, name)
);

CREATE TABLE IF NOT EXISTS stock_categories (
    category_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    UNIQUE (company_id, name)
);

CREATE TABLE IF NOT EXISTS warehouses (
    warehouse_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    address TEXT,
    is_active INTEGER DEFAULT 1,
    UNIQUE (company_id, name)
);

CREATE TABLE IF NOT EXISTS stock_items (
    item_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    stock_group_id TEXT REFERENCES stock_groups(stock_group_id),
    category_id TEXT REFERENCES stock_categories(category_id),
    uom_id TEXT NOT NULL REFERENCES units_of_measure(uom_id),
    name TEXT NOT NULL,
    sku TEXT,
    hsn_sac_code TEXT,
    gst_rate_basis_points INTEGER DEFAULT 1800, -- e.g. 18.00% = 1800 bp
    valuation_method TEXT DEFAULT 'FIFO' CHECK (valuation_method IN ('FIFO', 'WeightedAverage', 'LastPurchaseCost', 'StandardCost', 'ZeroCost')),
    opening_qty_milli INTEGER DEFAULT 0, -- Stored as milli-units (1.000 kg = 1000)
    opening_rate_paise INTEGER DEFAULT 0,
    opening_value_paise INTEGER DEFAULT 0,
    reorder_level_milli INTEGER DEFAULT 0,
    is_active INTEGER DEFAULT 1,
    UNIQUE (company_id, name)
);

CREATE TABLE IF NOT EXISTS inventory_transactions (
    inv_tx_id TEXT PRIMARY KEY, -- UUID v4
    voucher_id TEXT NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    voucher_entry_id TEXT REFERENCES voucher_entries(entry_id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES stock_items(item_id),
    warehouse_id TEXT NOT NULL REFERENCES warehouses(warehouse_id),
    movement_type TEXT NOT NULL CHECK (movement_type IN ('IN', 'OUT')),
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli > 0),
    rate_paise INTEGER NOT NULL CHECK (rate_paise >= 0),
    amount_paise INTEGER NOT NULL CHECK (amount_paise >= 0),
    batch_number TEXT,
    expiry_date TEXT -- YYYY-MM-DD
);

-- ============================================================================
-- 5. GST REGISTRATIONS & STATUTORY DETAILS
-- ============================================================================

CREATE TABLE IF NOT EXISTS gst_registrations (
    reg_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    gstin TEXT NOT NULL,
    legal_name TEXT NOT NULL,
    trade_name TEXT,
    state_code TEXT NOT NULL,
    registration_type TEXT DEFAULT 'Regular' CHECK (registration_type IN ('Regular', 'Composition')),
    is_primary INTEGER DEFAULT 1,
    CHECK (length(gstin) = 15)
);

CREATE TABLE IF NOT EXISTS hsn_sac_directory (
    code TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    type TEXT CHECK (type IN ('HSN', 'SAC')),
    default_gst_basis_points INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS voucher_gst_details (
    gst_detail_id TEXT PRIMARY KEY, -- UUID v4
    voucher_id TEXT UNIQUE NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    party_gstin TEXT,
    place_of_supply TEXT NOT NULL,
    is_interstate INTEGER NOT NULL,
    is_reverse_charge INTEGER DEFAULT 0,
    invoice_type TEXT DEFAULT 'B2B' CHECK (invoice_type IN ('B2B', 'B2CL', 'B2CS', 'Export', 'NilRated')),
    eway_bill_no TEXT,
    eway_bill_date TEXT,
    irn_hash TEXT
);

-- ============================================================================
-- 6. USERS, PERMISSIONS & MCA AUDIT TRAIL
-- ============================================================================

CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY, -- UUID v4
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL, -- Argon2id hash
    full_name TEXT,
    role TEXT DEFAULT 'DataEntry' CHECK (role IN ('Admin', 'Accountant', 'DataEntry', 'Auditor')),
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS audit_trail (
    log_id INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id TEXT NOT NULL REFERENCES companies(company_id),
    entity_type TEXT NOT NULL, -- 'vouchers', 'ledgers', 'stock_items'
    entity_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('INSERT', 'UPDATE', 'POST', 'CANCEL', 'DELETE')),
    user_id TEXT REFERENCES users(user_id),
    timestamp TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    data_before JSON,
    data_after JSON,
    prev_hash TEXT,
    record_hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS backup_metadata (
    backup_id TEXT PRIMARY KEY, -- UUID v4
    company_id TEXT NOT NULL REFERENCES companies(company_id),
    backup_file_name TEXT NOT NULL,
    file_size_bytes INTEGER NOT NULL,
    sha256_hash TEXT NOT NULL,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT REFERENCES users(user_id),
    schema_version TEXT NOT NULL
);

-- ============================================================================
-- 7. PERFORMANCE INDEXES
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_vouchers_lookup 
ON vouchers(company_id, fy_id, voucher_type_id, voucher_date);

CREATE INDEX IF NOT EXISTS idx_voucher_entries_ledger 
ON voucher_entries(ledger_id, entry_type, amount_paise);

CREATE INDEX IF NOT EXISTS idx_bill_alloc_ledger_ref 
ON bill_allocations(ledger_id, ref_name, ref_type);

CREATE INDEX IF NOT EXISTS idx_inventory_item_movement 
ON inventory_transactions(item_id, warehouse_id, movement_type);

CREATE INDEX IF NOT EXISTS idx_audit_entity_lookup 
ON audit_trail(company_id, entity_type, entity_id);

-- ============================================================================
-- 8. DATABASE TRIGGERS (INTEGRITY & IMMUTABILITY)
-- ============================================================================

-- Trigger 1: Enforce double-entry balance before posting voucher
CREATE TRIGGER IF NOT EXISTS trg_verify_voucher_balance_on_post
BEFORE UPDATE OF status ON vouchers
FOR EACH ROW
WHEN NEW.status = 'Posted'
BEGIN
    SELECT CASE
        WHEN (
            SELECT COALESCE(SUM(CASE WHEN entry_type = 'Dr' THEN amount_paise ELSE -amount_paise END), -1)
            FROM voucher_entries
            WHERE voucher_id = NEW.voucher_id
        ) != 0
        THEN RAISE(ABORT, 'ERR_VOUCHER_UNBALANCED: Total Debits must exactly equal Total Credits before posting.')
    END;
END;

-- Trigger 2: Prevent modifications to already posted vouchers
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_voucher_update
BEFORE UPDATE ON vouchers
FOR EACH ROW
WHEN OLD.status = 'Posted' AND NEW.status = 'Posted'
BEGIN
    SELECT CASE
        WHEN OLD.total_amount_paise != NEW.total_amount_paise 
          OR OLD.voucher_date != NEW.voucher_date
          OR OLD.voucher_number != NEW.voucher_number
          OR OLD.voucher_type_id != NEW.voucher_type_id
        THEN RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Posted vouchers cannot be modified. Cancel or reverse with a credit/debit note.')
    END;
END;

-- Trigger 3: Prevent adding entries to already posted vouchers
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_line_insert
BEFORE INSERT ON voucher_entries
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = NEW.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot add lines to an already posted voucher.');
END;

-- Trigger 4: Audit Trail Immutability (MCA Rule 3(1) Compliance)
CREATE TRIGGER IF NOT EXISTS trg_audit_trail_immutable_update
BEFORE UPDATE ON audit_trail
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'ERR_AUDIT_TAMPERING: Audit trail entries cannot be updated under MCA compliance.');
END;

CREATE TRIGGER IF NOT EXISTS trg_audit_trail_immutable_delete
BEFORE DELETE ON audit_trail
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'ERR_AUDIT_TAMPERING: Audit trail entries cannot be deleted under MCA compliance.');
END;

-- ============================================================================
-- 9. PREDEFINED SEED DATA (28 STANDARD GROUPS & DEFAULT VOUCHER TYPES)
-- ============================================================================

-- Note: In a production run, company_id is bound during initialization.
-- Seed helper template inserts standard primary and secondary groups.
INSERT OR IGNORE INTO hsn_sac_directory (code, description, type, default_gst_basis_points) VALUES
('998311', 'Management consulting and management services', 'SAC', 1800),
('998313', 'Information technology consulting and support services', 'SAC', 1800),
('847130', 'Portable automatic data processing machines (Laptops)', 'HSN', 1800),
('851713', 'Smartphones and cellular devices', 'HSN', 1800),
('0401', 'Milk and cream, not concentrated nor containing added sugar', 'HSN', 0),
('1006', 'Rice in the husk (Paddy or rough)', 'HSN', 0);
