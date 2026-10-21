-- ============================================================================
-- Migration: V002__gst_engine.sql
-- Indian GST Accounting Engine Schema
-- ============================================================================

-- 1. GST Registrations (Company Level)
CREATE TABLE IF NOT EXISTS gst_registrations (
    reg_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    gstin TEXT NOT NULL,
    legal_name TEXT NOT NULL,
    trade_name TEXT,
    state_code TEXT NOT NULL,
    registration_type TEXT DEFAULT 'Regular' CHECK (registration_type IN ('Regular', 'Composition')),
    is_primary INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(gstin) = 15),
    UNIQUE (company_id, gstin)
);

-- 2. HSN / SAC Directory with Standard Rates
CREATE TABLE IF NOT EXISTS hsn_sac_directory (
    code TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    item_type TEXT NOT NULL CHECK (item_type IN ('Goods', 'Services')),
    default_gst_basis_points INTEGER NOT NULL CHECK (default_gst_basis_points >= 0),
    effective_from TEXT DEFAULT '2017-07-01'
);

-- 3. Party GST attributes on Ledgers
ALTER TABLE ledgers ADD COLUMN party_type TEXT DEFAULT 'Regular' CHECK (party_type IN ('Regular', 'Composition', 'Unregistered', 'Consumer', 'Overseas', 'SEZ'));
ALTER TABLE ledgers ADD COLUMN is_reverse_charge_applicable INTEGER DEFAULT 0;

-- 4. Voucher GST Metadata
CREATE TABLE IF NOT EXISTS voucher_gst_details (
    gst_detail_id TEXT PRIMARY KEY,
    voucher_id TEXT UNIQUE NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    party_ledger_id TEXT REFERENCES ledgers(ledger_id),
    party_gstin TEXT,
    place_of_supply TEXT NOT NULL,
    is_interstate INTEGER NOT NULL CHECK (is_interstate IN (0, 1)),
    is_reverse_charge INTEGER DEFAULT 0 CHECK (is_reverse_charge IN (0, 1)),
    invoice_type TEXT DEFAULT 'B2B' CHECK (invoice_type IN ('B2B', 'B2CL', 'B2CS', 'Export', 'NilRated', 'Exempt')),
    supply_type TEXT DEFAULT 'Taxable' CHECK (supply_type IN ('Taxable', 'ZeroRated', 'NilRated', 'Exempt', 'NonGST')),
    eway_bill_no TEXT,
    eway_bill_date TEXT,
    original_voucher_id TEXT REFERENCES vouchers(voucher_id),
    original_invoice_no TEXT,
    original_invoice_date TEXT
);

-- 5. Voucher Line-Level Tax Items (supports multi-rate invoices, inclusive/exclusive pricing)
CREATE TABLE IF NOT EXISTS voucher_gst_tax_items (
    tax_item_id TEXT PRIMARY KEY,
    voucher_id TEXT NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    line_number INTEGER NOT NULL,
    hsn_code TEXT,
    description TEXT NOT NULL,
    is_tax_inclusive INTEGER DEFAULT 0 CHECK (is_tax_inclusive IN (0, 1)),
    gross_amount_paise INTEGER NOT NULL CHECK (gross_amount_paise >= 0),
    taxable_amount_paise INTEGER NOT NULL CHECK (taxable_amount_paise >= 0),
    gst_rate_basis_points INTEGER NOT NULL CHECK (gst_rate_basis_points >= 0),
    igst_rate_basis_points INTEGER NOT NULL DEFAULT 0,
    cgst_rate_basis_points INTEGER NOT NULL DEFAULT 0,
    sgst_rate_basis_points INTEGER NOT NULL DEFAULT 0,
    igst_amount_paise INTEGER NOT NULL DEFAULT 0 CHECK (igst_amount_paise >= 0),
    cgst_amount_paise INTEGER NOT NULL DEFAULT 0 CHECK (cgst_amount_paise >= 0),
    sgst_amount_paise INTEGER NOT NULL DEFAULT 0 CHECK (sgst_amount_paise >= 0),
    total_tax_paise INTEGER NOT NULL DEFAULT 0 CHECK (total_tax_paise >= 0),
    total_line_amount_paise INTEGER NOT NULL CHECK (total_line_amount_paise >= 0),
    is_reverse_charge INTEGER DEFAULT 0 CHECK (is_reverse_charge IN (0, 1)),
    UNIQUE (voucher_id, line_number)
);

-- Indexes for GST reporting
CREATE INDEX IF NOT EXISTS idx_gst_details_pos ON voucher_gst_details(place_of_supply, invoice_type);
CREATE INDEX IF NOT EXISTS idx_gst_tax_items_voucher ON voucher_gst_tax_items(voucher_id);
CREATE INDEX IF NOT EXISTS idx_gst_tax_items_hsn ON voucher_gst_tax_items(hsn_code);

-- Immutability triggers for GST tables
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_voucher_gst_update
BEFORE UPDATE ON voucher_gst_details
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = OLD.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot update GST details of an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_voucher_gst_delete
BEFORE DELETE ON voucher_gst_details
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = OLD.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot delete GST details of an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_gst_item_insert
BEFORE INSERT ON voucher_gst_tax_items
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = NEW.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot insert tax items into an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_gst_item_update
BEFORE UPDATE ON voucher_gst_tax_items
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = OLD.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot modify tax items of an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_gst_item_delete
BEFORE DELETE ON voucher_gst_tax_items
FOR EACH ROW
WHEN (SELECT status FROM vouchers WHERE voucher_id = OLD.voucher_id) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot delete tax items of an already posted voucher.');
END;

-- Predefined HSN / SAC Seed Data
INSERT OR IGNORE INTO hsn_sac_directory (code, description, item_type, default_gst_basis_points) VALUES
('998311', 'Management consulting and management services', 'Services', 1800),
('998313', 'Information technology consulting and support services', 'Services', 1800),
('847130', 'Portable automatic data processing machines (Laptops)', 'Goods', 1800),
('851713', 'Smartphones and cellular devices', 'Goods', 1800),
('0401', 'Milk and cream, not concentrated nor containing added sugar', 'Goods', 0),
('1006', 'Rice in the husk (Paddy or rough)', 'Goods', 0),
('996511', 'Road transport services of goods by GTA', 'Services', 500),
('998211', 'Legal advisory and representation services', 'Services', 1800);
