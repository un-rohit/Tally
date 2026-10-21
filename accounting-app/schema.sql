-- ============================================================================
-- Clean-Room Accounting Database Schema (SQLite 3)
-- Standard: Double-Entry Invariance, Decimal-Safe Arithmetic, Audit Immutability
-- ============================================================================

PRAGMA foreign_keys = ON;

-- 1. COMPANIES & FINANCIAL YEARS
CREATE TABLE IF NOT EXISTS companies (
    company_id TEXT PRIMARY KEY,
    company_number TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    mailing_name TEXT NOT NULL,
    address TEXT,
    state_code TEXT NOT NULL,
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
    fy_id TEXT PRIMARY KEY,
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

-- 2. CHART OF ACCOUNTS (GROUPS & LEDGERS)
CREATE TABLE IF NOT EXISTS account_groups (
    group_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    parent_group_id TEXT REFERENCES account_groups(group_id),
    nature TEXT NOT NULL CHECK (nature IN ('Asset', 'Liability', 'Income', 'Expense')),
    affects_gross_profit INTEGER DEFAULT 0,
    is_predefined INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, name)
);

CREATE TABLE IF NOT EXISTS ledgers (
    ledger_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES account_groups(group_id),
    name TEXT NOT NULL,
    code TEXT,
    opening_balance_paise INTEGER DEFAULT 0,
    opening_balance_type TEXT CHECK (opening_balance_type IN ('Dr', 'Cr', NULL)),
    is_bill_wise INTEGER DEFAULT 0,
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

-- 3. VOUCHER TYPES, VOUCHERS & ENTRIES
CREATE TABLE IF NOT EXISTS voucher_types (
    voucher_type_id TEXT PRIMARY KEY,
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
    voucher_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    fy_id TEXT NOT NULL REFERENCES financial_years(fy_id),
    voucher_type_id TEXT NOT NULL REFERENCES voucher_types(voucher_type_id),
    voucher_number TEXT NOT NULL,
    voucher_date TEXT NOT NULL, -- YYYY-MM-DD
    reference_no TEXT,
    reference_date TEXT,
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
    entry_id TEXT PRIMARY KEY,
    voucher_id TEXT NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    line_number INTEGER NOT NULL,
    ledger_id TEXT NOT NULL REFERENCES ledgers(ledger_id),
    entry_type TEXT NOT NULL CHECK (entry_type IN ('Dr', 'Cr')),
    amount_paise INTEGER NOT NULL CHECK (amount_paise > 0),
    line_narration TEXT,
    UNIQUE (voucher_id, line_number)
);

CREATE TABLE IF NOT EXISTS bill_allocations (
    allocation_id TEXT PRIMARY KEY,
    entry_id TEXT NOT NULL REFERENCES voucher_entries(entry_id) ON DELETE CASCADE,
    ledger_id TEXT NOT NULL REFERENCES ledgers(ledger_id),
    ref_type TEXT NOT NULL CHECK (ref_type IN ('NewRef', 'AgstRef', 'Advance', 'OnAccount')),
    ref_name TEXT NOT NULL,
    credit_days INTEGER DEFAULT 0,
    due_date TEXT,
    amount_paise INTEGER NOT NULL CHECK (amount_paise > 0),
    entry_type TEXT NOT NULL CHECK (entry_type IN ('Dr', 'Cr'))
);

CREATE TABLE IF NOT EXISTS audit_trail (
    log_id INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id TEXT NOT NULL REFERENCES companies(company_id),
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('INSERT', 'UPDATE', 'POST', 'CANCEL', 'DELETE')),
    user_id TEXT,
    timestamp TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    data_before JSON,
    data_after JSON,
    prev_hash TEXT,
    record_hash TEXT NOT NULL
);

-- INDEXES
CREATE INDEX IF NOT EXISTS idx_vouchers_lookup 
ON vouchers(company_id, fy_id, voucher_type_id, voucher_date);

CREATE INDEX IF NOT EXISTS idx_voucher_entries_ledger 
ON voucher_entries(ledger_id, entry_type, amount_paise);

CREATE INDEX IF NOT EXISTS idx_bill_alloc_ledger_ref 
ON bill_allocations(ledger_id, ref_name, ref_type);

CREATE INDEX IF NOT EXISTS idx_audit_entity_lookup 
ON audit_trail(company_id, entity_type, entity_id);

-- TRIGGERS

-- 1. Date Boundary Trigger: Voucher date must be within FY and >= books_beginning_date
CREATE TRIGGER IF NOT EXISTS trg_verify_voucher_date_within_fy
BEFORE INSERT ON vouchers
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN NEW.voucher_date < (SELECT books_beginning_date FROM financial_years WHERE fy_id = NEW.fy_id)
        THEN RAISE(ABORT, 'ERR_VOUCHER_DATE_BEFORE_BOOKS: Voucher date cannot precede books beginning date.')
        WHEN NEW.voucher_date > (SELECT fy_end_date FROM financial_years WHERE fy_id = NEW.fy_id)
        THEN RAISE(ABORT, 'ERR_VOUCHER_DATE_OUTSIDE_FY: Voucher date cannot exceed financial year end date.')
    END;
END;

-- 2. Double-Entry Balance Verification on Posting
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
        WHEN (
            SELECT COUNT(*) FROM voucher_entries 
            WHERE voucher_id = NEW.voucher_id AND entry_type = 'Dr'
        ) = 0
        THEN RAISE(ABORT, 'ERR_VOUCHER_NO_DEBIT: Voucher must contain at least one Debit line.')
        WHEN (
            SELECT COUNT(*) FROM voucher_entries 
            WHERE voucher_id = NEW.voucher_id AND entry_type = 'Cr'
        ) = 0
        THEN RAISE(ABORT, 'ERR_VOUCHER_NO_CREDIT: Voucher must contain at least one Credit line.')
    END;
END;

-- 3. Prevent inserting a voucher directly as 'Posted' without line validation
CREATE TRIGGER IF NOT EXISTS trg_prevent_direct_posted_insert
BEFORE INSERT ON vouchers
FOR EACH ROW
WHEN NEW.status = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_INVALID_VOUCHER_LIFECYCLE: Vouchers must be created as Draft before posting.');
END;

-- 4. Prevent modifying header of posted vouchers
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
          OR OLD.company_id != NEW.company_id
          OR OLD.fy_id != NEW.fy_id
        THEN RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Posted vouchers cannot be modified. Cancel or reverse with a credit/debit note.')
    END;
END;

-- 5. Prevent inserting, updating, or deleting lines of posted vouchers
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_line_insert
BEFORE INSERT ON voucher_entries
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = NEW.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot add lines to an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_line_update
BEFORE UPDATE ON voucher_entries
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = OLD.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot modify lines of an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_line_delete
BEFORE DELETE ON voucher_entries
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = OLD.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot delete lines of an already posted voucher.');
END;

-- 6. MCA Audit Trail Immutability Triggers
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
