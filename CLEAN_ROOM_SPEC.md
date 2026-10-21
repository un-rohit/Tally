# Clean-Room Engineering Specification: Independent Accounting System

**Document Version**: 1.0.0  
**Target Goal**: Architecture and functional specification for an independent, clean-room double-entry accounting software compatible with standard business workflows and data interchange formats modeled after TallyPrime.  
**Audit Reference**: Based on verified functional boundaries in [`D:\Tally\TALLY_FEATURE_MAP.md`](file:///D:/Tally/TALLY_FEATURE_MAP.md) and [`D:\Tally\TALLY_VERIFICATION.md`](file:///D:/Tally/TALLY_VERIFICATION.md).  
**Engineering Methodology**: Clean-room implementation. No proprietary code, assets, decompiled routines, or private binary storage algorithms are utilized. All storage, processing logic, and interface models are designed independently using standard relational paradigms (PostgreSQL/SQLite) and published accounting/tax standards.

---

## Clean-Room Compliance & Architectural Principles

1. **Storage Independence**: The application will **not** attempt to duplicate or reverse-engineer Tally's proprietary `.1800` binary chunk files (`Manager.1800`, `TranMgr.1800`, `TSDBK1800`). Instead, it uses a standard, ACID-compliant relational database schema (ANSI SQL DDL) with transparent foreign key constraints, indexes, and transactions.
2. **Standard Double-Entry Invariance**: Every transaction enforces the accounting equation:
   $$\sum \text{Debits} = \sum \text{Credits}$$
   No unbalanced transactions can be committed under any circumstance.
3. **Statutory Integrity**: Indian Goods and Services Tax (GST) calculation and MCA (Ministry of Corporate Affairs) audit trail rules are implemented according to published statutory notifications (CBIC, Companies Act 2013 Rule 3(1)).
4. **Separation of Verified Requirements vs. Assumptions**:
   * **[VERIFIED]**: Behaviors, field validations, and protocol schemas directly proven through unencrypted `.rdata` strings, export tables, and companion DLLs.
   * **[ASSUMPTION]**: Design choices or standard accounting conventions adopted where proprietary internals were shielded by virtualization.

---

## Module 1: Company Management and Financial Years

### 1.1 Functional Requirements
* Multi-company support: The system must allow creating, altering, selecting, and closing independent company entities.
* Financial Year Definition: Every company has a 12-month statutory Financial Year (FY) starting on April 1st (default in India) and ending on March 31st of the following calendar year.
* "Books Beginning From": Allows books of accounts to start on or after the FY Start Date (e.g., a business founded on October 15, 2026 has FY starting April 1, 2026, but books starting October 15, 2026).
* Historical Multi-Year Continuity: Supports spanning across consecutive financial years without requiring a disruptive "Year-End Closing" freeze; balances roll forward automatically.
* Base Currency Configuration: Currency symbol (e.g., ₹), formal name (INR), decimal places (default: 2), and number formatting (Lakhs/Crores vs Millions/Billions).

### 1.2 Inputs and Outputs
* **Inputs**:
  * Company Name, Mailing Name, Address, State, Pincode, Country.
  * Financial Year From Date (`YYYY-MM-DD`).
  * Books Beginning From Date (`YYYY-MM-DD`).
  * Base Currency Symbol, Decimal Places, Decimal Symbol, Suffix/Prefix symbol to amount.
  * Corporate Identity Number (CIN), PAN, Primary Contact Details.
* **Outputs**:
  * Company Profile Object / State.
  * Unique Company Identifier (`company_id` / GUID).
  * System Company Number (5-digit numeric sequence: e.g., `10001`).

### 1.3 Business Rules and Validation
* **Date Bounds**: `books_beginning_from >= financial_year_from`.
* **Period Length**: Default financial year duration is exactly 365 days (366 in leap years).
* **Unique Company Identification**: System assigns a collision-free 5-digit number or UUID.
* **Voucher Date Enforcement**: No voucher can have a transaction date strictly prior to `books_beginning_from`.

### 1.4 Database Entities and Relationships (SQL DDL)

```sql
CREATE TABLE companies (
    company_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_number VARCHAR(10) UNIQUE NOT NULL, -- e.g., '10001'
    name VARCHAR(255) NOT NULL,
    mailing_name VARCHAR(255) NOT NULL,
    address TEXT,
    state_code VARCHAR(2) NOT NULL, -- e.g., '27' for Maharashtra
    country VARCHAR(50) DEFAULT 'India',
    pincode VARCHAR(10),
    telephone VARCHAR(50),
    email VARCHAR(100),
    pan VARCHAR(10),
    cin VARCHAR(21),
    currency_symbol VARCHAR(10) DEFAULT '₹',
    currency_formal_name VARCHAR(20) DEFAULT 'INR',
    decimal_places SMALLINT DEFAULT 2,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    is_active BOOLEAN DEFAULT TRUE
);

CREATE TABLE financial_periods (
    period_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    fy_start_date DATE NOT NULL,
    fy_end_date DATE NOT NULL,
    books_beginning_date DATE NOT NULL,
    is_closed BOOLEAN DEFAULT FALSE,
    CONSTRAINT check_books_date CHECK (books_beginning_date >= fy_start_date),
    CONSTRAINT check_fy_range CHECK (fy_end_date > fy_start_date),
    CONSTRAINT unique_company_fy UNIQUE (company_id, fy_start_date)
);
```

### 1.5 Error Handling and Edge Cases
* **Invalid Date Sequence**: Attempting to set `books_beginning_from < fy_start_date` triggers `ERR_INVALID_BOOKS_DATE`.
* **Leap Year Crossing**: System correctly validates February 29th for leap years (e.g. 2028).
* **Multi-Instance Collision**: File locking or relational row-level locking (`SELECT ... FOR UPDATE`) prevents concurrent alterations of company settings.

### 1.6 Acceptance Tests
* **TC-COMP-01**: Create company with `fy_start = '2026-04-01'` and `books_beginning = '2026-06-01'`. Validate company record creation with generated 5-digit number.
* **TC-COMP-02**: Attempt to create company with `books_beginning = '2026-03-31'`. Must reject with validation error.
* **TC-COMP-03**: Create voucher dated `2026-05-15`. Must fail with error: "Date cannot be before books beginning date".

---

## Module 2: Chart of Accounts, Groups, and Ledgers

### 2.1 Functional Requirements
* Hierarchical Chart of Accounts: Tree structure consisting of **Groups** (categories) and **Ledgers** (posting accounts).
* 28 Predefined Primary and Secondary Groups [VERIFIED: exact group ontology from accounting standards]:
  * **Capital Account** (Reserves & Surplus)
  * **Loans (Liability)** (Bank OD A/c, Secured Loans, Unsecured Loans)
  * **Current Liabilities** (Duties & Taxes, Provisions, Sundry Creditors)
  * **Fixed Assets**
  * **Investments**
  * **Current Assets** (Bank Accounts, Cash-in-hand, Deposits, Loans & Advances, Stock-in-hand, Sundry Debtors)
  * **Branch / Divisions**, **Misc. Expenses**
  * **Sales Accounts**, **Purchase Accounts**
  * **Direct Incomes**, **Indirect Incomes**
  * **Direct Expenses**, **Indirect Expenses**
* Ledger Master: Posting accounts created under any group (e.g., "State Bank of India" under "Bank Accounts").
* Bill-by-Bill Tracking: Ledgers under Sundry Debtors/Creditors can enable "Maintain balances bill-by-bill" to track individual invoice dues, credit periods, and aging.

### 2.2 Inputs and Outputs
* **Inputs**:
  * Group: Name, Parent Group ID, Nature of Group (Asset, Liability, Income, Expense), Sub-ledger behavior.
  * Ledger: Name, Group ID, Opening Balance, Opening Balance Type (Debit/Credit), Bill-by-bill flag, Mailing Address, PAN, GSTIN.
* **Outputs**:
  * Hierarchical tree view (Chart of Accounts).
  * Ledger Current Balance (computed on the fly from opening balance + journal debits - journal credits).

### 2.3 Business Rules and Validation
* **Predefined Group Invariance**: System root groups cannot be renamed or deleted.
* **Parent-Child Loop Prevention**: A group cannot be placed under one of its own descendants.
* **Posting Isolation**: Vouchers can only post to **Ledgers**, never directly to **Groups**.
* **Accounting Nature Inheritance**: A child group or ledger strictly inherits the `Nature` (Asset, Liability, Income, Expense) of its root ancestor.

### 2.4 Database Entities and Relationships (SQL DDL)

```sql
CREATE TYPE account_nature AS ENUM ('Asset', 'Liability', 'Income', 'Expense');

CREATE TABLE account_groups (
    group_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name VARCHAR(150) NOT NULL,
    parent_group_id UUID REFERENCES account_groups(group_id),
    nature account_nature NOT NULL,
    is_predefined BOOLEAN DEFAULT FALSE,
    affects_gross_profit BOOLEAN DEFAULT FALSE, -- True for Direct Income/Expenses
    sort_order INT DEFAULT 0,
    CONSTRAINT unique_group_per_company UNIQUE (company_id, name)
);

CREATE TABLE ledgers (
    ledger_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES account_groups(group_id),
    name VARCHAR(200) NOT NULL,
    opening_balance NUMERIC(15, 2) DEFAULT 0.00,
    opening_balance_type VARCHAR(2) CHECK (opening_balance_type IN ('Dr', 'Cr')),
    is_bill_wise BOOLEAN DEFAULT FALSE,
    credit_period_days INT DEFAULT 0,
    pan VARCHAR(10),
    gstin VARCHAR(15),
    mailing_name VARCHAR(255),
    address TEXT,
    state_code VARCHAR(2),
    pincode VARCHAR(10),
    is_active BOOLEAN DEFAULT TRUE,
    CONSTRAINT unique_ledger_per_company UNIQUE (company_id, name)
);
```

### 2.5 Error Handling and Edge Cases
* **Circular Group Dependency**: Validated via recursive CTE prior to insert/update. Triggers `ERR_CIRCULAR_GROUP_REFERENCE`.
* **Deletion of Active Ledger**: Attempting to delete a ledger referenced in any voucher triggers `ERR_LEDGER_HAS_TRANSACTIONS`.
* **Zero Opening Balance**: Allowed with NULL balance type.

### 2.6 Acceptance Tests
* **TC-COA-01**: Seed the 28 predefined standard groups upon company initialization.
* **TC-COA-02**: Create a ledger "Acme Corp" under "Sundry Debtors". Verify that its inherited nature is `Asset`.
* **TC-COA-03**: Attempt to create duplicate ledger "Acme Corp" in same company. Expect uniqueness violation.

---

## Module 3: Double-Entry Bookkeeping and Voucher Processing

### 3.1 Functional Requirements
* Standard Voucher Types [VERIFIED: unencrypted TDL strings and shortcuts]:
  * **Contra (`F4`)**: Cash and Bank transfers only.
  * **Payment (`F5`)**: Outflow of funds from Cash/Bank.
  * **Receipt (`F6`)**: Inflow of funds to Cash/Bank.
  * **Journal (`F7`)**: Non-cash adjustments, provisions, and depreciation.
  * **Sales (`F8`)**: Invoicing goods or services to customers.
  * **Purchase (`F9`)**: Inward supply of goods or services from vendors.
  * **Credit Note (`Alt+F6`)**: Sales returns or price reductions.
  * **Debit Note (`Alt+F5`)**: Purchase returns or vendor adjustments.
* Multi-line Journal Entry: A voucher consists of $N \ge 2$ entries (each specifying Ledger, Debit/Credit flag, and Amount).
* Bill Allocation Details: For bill-wise ledgers, each entry line allows tagging:
  * `New Ref`: Creates a new outstanding bill document with due date.
  * `Agst Ref`: Settles an existing unpaid bill reference.
  * `Advance`: Tags payment received before invoice generation.
  * `On Account`: Unallocated lump-sum payment.

### 3.2 Inputs and Outputs
* **Inputs**:
  * Voucher Type, Voucher Number, Date, Reference Number.
  * Narration (header and line-level).
  * Line Items: `ledger_id`, `entry_type` (`Dr` / `Cr`), `amount`, `bill_allocations` list.
* **Outputs**:
  * Committed Voucher with immutable `voucher_id`.
  * Updated Ledger Balances.
  * Updated Bill Outstandings ledger.

### 3.3 Business Rules and Validation
* **Double-Entry Balance Constraint**:
  $$\sum \text{Amount}_{\text{Dr}} - \sum \text{Amount}_{\text{Cr}} = 0$$
  If difference $\ne 0$, transaction cannot commit.
* **Contra Account Restriction**: Line items in a Contra voucher can only be ledgers classified under `Bank Accounts`, `Cash-in-hand`, or `Bank OD`.
* **Minimum Line Rule**: Every voucher must have at least one Debit and at least one Credit.

### 3.4 Database Entities and Relationships (SQL DDL)

```sql
CREATE TYPE voucher_class AS ENUM (
    'Contra', 'Payment', 'Receipt', 'Journal', 
    'Sales', 'Purchase', 'CreditNote', 'DebitNote', 'StockJournal'
);

CREATE TABLE vouchers (
    voucher_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    voucher_type voucher_class NOT NULL,
    voucher_number VARCHAR(50) NOT NULL,
    voucher_date DATE NOT NULL,
    reference_no VARCHAR(100),
    reference_date DATE,
    narration TEXT,
    total_amount NUMERIC(15, 2) NOT NULL,
    is_cancelled BOOLEAN DEFAULT FALSE,
    is_optional BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT unique_voucher_num_per_company UNIQUE (company_id, voucher_type, voucher_number)
);

CREATE TABLE voucher_lines (
    line_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    voucher_id UUID NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    ledger_id UUID NOT NULL REFERENCES ledgers(ledger_id),
    line_number INT NOT NULL,
    entry_type VARCHAR(2) NOT NULL CHECK (entry_type IN ('Dr', 'Cr')),
    amount NUMERIC(15, 2) NOT NULL CHECK (amount > 0),
    line_narration TEXT
);

CREATE TYPE bill_ref_type AS ENUM ('New Ref', 'Agst Ref', 'Advance', 'On Account');

CREATE TABLE bill_allocations (
    allocation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    line_id UUID NOT NULL REFERENCES voucher_lines(line_id) ON DELETE CASCADE,
    ledger_id UUID NOT NULL REFERENCES ledgers(ledger_id),
    ref_type bill_ref_type NOT NULL,
    ref_name VARCHAR(100) NOT NULL,
    credit_days INT DEFAULT 0,
    due_date DATE,
    amount NUMERIC(15, 2) NOT NULL,
    entry_type VARCHAR(2) NOT NULL CHECK (entry_type IN ('Dr', 'Cr'))
);
```

### 3.5 Error Handling and Edge Cases
* **Unbalanced Voucher**: Triggers `ERR_VOUCHER_UNBALANCED` with calculated difference.
* **Negative Line Amount**: Negative values rejected; credits and debits must be positive absolute amounts differentiated by `entry_type`.
* **Over-settlement**: Attempting an `Agst Ref` allocation for more than the outstanding bill balance warns the user or creates an excess credit.

### 3.6 Acceptance Tests
* **TC-VCH-01**: Post a Journal voucher: Dr Depreciation ₹5,000, Cr Machinery ₹5,000. Commit successfully.
* **TC-VCH-02**: Attempt to post voucher: Dr Rent ₹10,000, Cr Bank ₹9,000. Expect rejection with error `Difference: ₹1,000`.
* **TC-VCH-03**: Post a Contra voucher: Dr Petty Cash ₹2,000, Cr State Bank of India ₹2,000. Commit successfully.

---

## Module 4: Sales, Purchases, Payments, and Receipts

### 4.1 Functional Requirements
* **Item Invoice vs. Accounting Invoice**:
  * Item Invoice mode allows adding inventory items, quantity, rate, unit, and item-level taxes.
  * Accounting Invoice mode allows selecting service ledgers directly with lumpsum amounts.
* **Customer / Supplier Party Ledger Selection**:
  * Must automatically retrieve party's GSTIN, billing state, and address.
* **Automatic Tax Computation**:
  * If Party State = Company State $\to$ Intra-state $\to$ CGST + SGST ledgers.
  * If Party State $\ne$ Company State $\to$ Inter-state $\to$ IGST ledger.
* **Sundry Charges / Additional Expenses**:
  * Add freight, packaging charges, or trade discounts before or after tax calculation.

### 4.2 Inputs and Outputs
* **Inputs**:
  * Party Ledger ID, Place of Supply (State), Dispatch Details, Buyer Address.
  * Line items: Stock Item / Service Ledger, Quantity, Rate, Discount %, Tax Rate.
  * Payment terms, Due Date.
* **Outputs**:
  * Formatted Tax Invoice (PDF / Print layout).
  * Automatic Journal Postings:
    * Sales: Dr Party A/c, Cr Sales A/c, Cr Output CGST, Cr Output SGST.
    * Purchase: Dr Purchase A/c, Dr Input CGST, Dr Input SGST, Cr Vendor A/c.

### 4.3 Business Rules and Validation
* **Tax Invariance**: Tax amounts must equal $\text{Taxable Value} \times \text{Rate} / 100$ rounded to 2 decimal places.
* **Invoice Round-Off**: Optional automatic ledger posting to "Round Off" A/c for decimal rounding to nearest whole rupee.
* **Stock Deductions**: In Item Invoice mode, committing a Sales voucher triggers immediate reduction of inventory in the selected Godown (Warehouse).

### 4.4 Database Entities and Relationships (SQL DDL)

```sql
CREATE TABLE sales_purchase_details (
    invoice_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    voucher_id UUID NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    party_ledger_id UUID NOT NULL REFERENCES ledgers(ledger_id),
    place_of_supply VARCHAR(2) NOT NULL, -- State Code
    buyer_name VARCHAR(255),
    buyer_address TEXT,
    buyer_gstin VARCHAR(15),
    is_interstate BOOLEAN NOT NULL,
    is_reverse_charge BOOLEAN DEFAULT FALSE,
    dispatch_doc_no VARCHAR(100),
    dispatched_through VARCHAR(100),
    destination VARCHAR(100),
    carrier_vehicle_no VARCHAR(50)
);
```

### 4.5 Error Handling and Edge Cases
* **Invalid GSTIN on B2B Invoice**: If transaction tagged as B2B but GSTIN format fails checksum, flag as warning or exception in GST summary.
* **Zero Rate Items**: Supported for nil-rated or exempt goods.
* **Negative Stock Warning**: If quantity exceeds available stock, display alert based on company settings (Allow Negative Stock: Yes/No).

### 4.6 Acceptance Tests
* **TC-INV-01**: Create intra-state sales invoice: 10 units @ ₹1,000 = ₹10,000. Apply 18% GST (9% CGST = ₹900, 9% SGST = ₹900). Total: ₹11,800. Verify generated ledger entries.
* **TC-INV-02**: Create inter-state sales invoice to different state. Verify application of 18% IGST = ₹1,800.

---

## Module 5: Inventory and Stock Valuation

### 5.1 Functional Requirements
* Inventory Master Hierarchy:
  * **Units of Measure (UOM)**: Simple (e.g., `Nos`, `Kgs`, `Boxes`) or Compound (e.g., `1 Box = 10 Nos`).
  * **Stock Groups**: Hierarchical categorisation of goods (e.g., Raw Materials, Finished Goods).
  * **Stock Categories**: Cross-group attributes (e.g., Brand, Size).
  * **Godowns / Locations**: Physical storage warehouses.
  * **Stock Items**: Individual SKU with standard cost, reorder levels, UOM, and HSN code.
* Stock Valuation Methods [VERIFIED: standard accounting methods in Tally literature]:
  * **FIFO (First In First Out)**
  * **Weighted Average Cost**
  * **Last Purchase Cost**
  * **Standard Cost**
  * **Zero Cost**

### 5.2 Inputs and Outputs
* **Inputs**:
  * Stock Item Master: Name, Group, UOM, Opening Qty, Opening Rate, Valuation Method.
  * Transaction Line: Item ID, Godown ID, Inward/Outward Qty, Rate, Discount.
* **Outputs**:
  * Stock Summary Report (Opening, Inwards, Outwards, Closing Balance Qty & Value).
  * Godown-wise breakdown.
  * Closing Stock value posted to Balance Sheet.

### 5.3 Business Rules and Valuation Algorithms
* **FIFO Cost Queue**:
  * Each inward receipt creates a cost lot $(Q_i, R_i)$.
  * When outward issue occurs for quantity $Q_{\text{out}}$, consume lots sequentially from oldest available lot $i=1 \dots k$.
* **Weighted Average Method**:
  $$\text{New Average Rate} = \frac{(\text{Current Qty} \times \text{Current Rate}) + (\text{Inward Qty} \times \text{Inward Rate})}{\text{Current Qty} + \text{Inward Qty}}$$

### 5.4 Database Entities and Relationships (SQL DDL)

```sql
CREATE TABLE units_of_measure (
    uom_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    symbol VARCHAR(20) NOT NULL,
    formal_name VARCHAR(100),
    decimal_places SMALLINT DEFAULT 0,
    CONSTRAINT unique_uom_per_company UNIQUE (company_id, symbol)
);

CREATE TABLE stock_items (
    item_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name VARCHAR(200) NOT NULL,
    uom_id UUID NOT NULL REFERENCES units_of_measure(uom_id),
    valuation_method VARCHAR(30) DEFAULT 'FIFO',
    hsn_sac_code VARCHAR(10),
    gst_rate NUMERIC(5, 2) DEFAULT 0.00,
    opening_qty NUMERIC(12, 3) DEFAULT 0.000,
    opening_rate NUMERIC(15, 2) DEFAULT 0.00,
    opening_value NUMERIC(15, 2) DEFAULT 0.00,
    CONSTRAINT unique_item_per_company UNIQUE (company_id, name)
);

CREATE TABLE godowns (
    godown_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    name VARCHAR(150) NOT NULL,
    address TEXT,
    CONSTRAINT unique_godown_per_company UNIQUE (company_id, name)
);

CREATE TABLE inventory_entries (
    entry_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    voucher_id UUID NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    item_id UUID NOT NULL REFERENCES stock_items(item_id),
    godown_id UUID NOT NULL REFERENCES godowns(godown_id),
    movement_type VARCHAR(3) CHECK (movement_type IN ('IN', 'OUT')),
    quantity NUMERIC(12, 3) NOT NULL,
    rate NUMERIC(15, 2) NOT NULL,
    amount NUMERIC(15, 2) NOT NULL
);
```

### 5.5 Error Handling and Edge Cases
* **Negative Inventory**: When outward movement leaves closing stock $< 0$, report valuation must fallback to Last Known Purchase Cost or Zero Cost.
* **Compound UOM Conversion**: Validated strictly against declared conversion factor (e.g. $1 \text{ Box} = 12 \text{ Nos}$, cannot receive $0.5 \text{ Box}$ unless fractional units allowed).

### 5.6 Acceptance Tests
* **TC-STK-01**: Inward 100 units @ ₹50 (Lot 1). Inward 50 units @ ₹60 (Lot 2). Outward 120 units using FIFO. Lot 1 consumed (100 @ ₹50 = ₹5,000) and Lot 2 consumed (20 @ ₹60 = ₹1,200). Total COGS = ₹6,200. Remaining: 30 units @ ₹60 = ₹1,800.
* **TC-STK-02**: Calculate same outward using Weighted Average. Average rate = $(5000+3000)/150 = ₹53.33$. Total COGS = ₹6,400.

---

## Module 6: Trial Balance, Profit and Loss, and Balance Sheet

### 6.1 Functional Requirements
* **Trial Balance**: Aggregated report of all ledgers with Debit and Credit closing balances for a given reporting period.
  * Invariant: Total Debits must equal Total Credits.
* **Profit and Loss Account (Income Statement)**:
  * **Trading Account**: Sales & Direct Incomes vs. Purchases, Direct Expenses, and Opening/Closing Stock. Derives **Gross Profit / Loss**.
  * **Operating & Net Statement**: Gross Profit + Indirect Incomes - Indirect Expenses. Derives **Net Profit / Loss**.
* **Balance Sheet**:
  * **Liabilities**: Capital Account, Loans, Current Liabilities, Net Profit transferred from P&L.
  * **Assets**: Fixed Assets, Investments, Current Assets, Closing Stock.
  * Invariant: $\text{Total Liabilities} = \text{Total Assets}$.

### 6.2 Inputs and Outputs
* **Inputs**:
  * Date range (`from_date` to `to_date`).
  * Display format: Horizontal (T-format) vs Vertical (Schedule III format).
  * Drill-down request (Group $\to$ Subgroup $\to$ Ledger $\to$ Monthly Summary $\to$ Voucher Register).
* **Outputs**:
  * Formatted Financial Statements.
  * Trial Balance Table.

### 6.3 Mathematical & Accounting Formulation

1. **Ledger Net Balance**:
   $$\text{Balance} = \text{Opening} + \sum \text{Dr} - \sum \text{Cr}$$
2. **Gross Profit Calculation**:
   $$\text{Gross Profit} = (\text{Sales} + \text{Direct Incomes} + \text{Closing Stock}) - (\text{Opening Stock} + \text{Purchases} + \text{Direct Expenses})$$
3. **Net Profit Calculation**:
   $$\text{Net Profit} = \text{Gross Profit} + \text{Indirect Incomes} - \text{Indirect Expenses}$$
4. **Balance Sheet Equilibrium**:
   $$\text{Capital} + \text{Net Profit} + \text{Liabilities} = \text{Fixed Assets} + \text{Current Assets} (\text{incl. Closing Stock}) + \text{Investments}$$

### 6.4 Reporting Database Views (SQL)

```sql
CREATE OR REPLACE VIEW view_trial_balance AS
SELECT 
    l.company_id,
    l.ledger_id,
    l.name AS ledger_name,
    g.name AS group_name,
    g.nature,
    SUM(CASE WHEN vl.entry_type = 'Dr' THEN vl.amount ELSE 0 END) AS total_debit,
    SUM(CASE WHEN vl.entry_type = 'Cr' THEN vl.amount ELSE 0 END) AS total_credit,
    (l.opening_balance + 
     SUM(CASE WHEN vl.entry_type = 'Dr' THEN vl.amount ELSE 0 END) - 
     SUM(CASE WHEN vl.entry_type = 'Cr' THEN vl.amount ELSE 0 END)) AS net_closing_balance
FROM ledgers l
JOIN account_groups g ON l.group_id = g.group_id
LEFT JOIN voucher_lines vl ON l.ledger_id = vl.ledger_id
LEFT JOIN vouchers v ON vl.voucher_id = v.voucher_id AND v.is_cancelled = FALSE
GROUP BY l.company_id, l.ledger_id, l.name, g.name, g.nature, l.opening_balance;
```

### 6.5 Error Handling and Edge Cases
* **Difference in Opening Balances**: If the sum of all ledger opening balances does not balance ($\sum \text{Dr} \ne \sum \text{Cr}$), the difference is automatically displayed on the Balance Sheet as "Difference in Opening Balances" under Assets or Liabilities until reconciled.
* **Negative Ledger Balance**: Displays on proper side with negative sign or moves to contra-group with alert.

### 6.6 Acceptance Tests
* **TC-REP-01**: Execute comprehensive journal entries for sales, purchases, rent, and capital. Generate Trial Balance. Verify `Total Debit == Total Credit`.
* **TC-REP-02**: Validate that Net Profit from P&L dynamically updates the Capital Account line in Balance Sheet.

---

## Module 7: GST Accounting Workflows for India

### 7.1 Functional Requirements
* **GSTIN Validation**: Enforce 15-character alphanumeric format:
  * Regex: `^[0-9]{2}[A-Z]{5}[0-9]{4}[A-Z]{1}[1-9A-Z]{1}Z[0-9A-Z]{1}$`
  * First 2 digits: State code (`01` to `38`).
  * Digits 3–12: PAN of the entity.
* **Tax Rate Slabs**: Support 0%, 0.25%, 3%, 5%, 12%, 18%, 28% and Cess.
* **Reverse Charge Mechanism (RCM)**: When buying from unregistered dealers or notified services, liability shifts to recipient. System generates self-invoicing and dual tax entries (Input Tax Credit + Tax Liability).
* **Statutory Reports**:
  * **GSTR-1**: Outward supplies broken down into B2B Invoices (4A, 4B), B2C Large (5A), B2C Small (7), Credit/Debit Notes (9B), Nil Rated/Exempt (8), and HSN Summary (12).
  * **GSTR-3B**: Summary table of Outward Supplies (3.1), Eligible ITC (4), and Exempt Inward Supplies (5).

### 7.2 Inputs and Outputs
* **Inputs**:
  * HSN/SAC Code per Stock Item / Service.
  * Place of Supply (POS state code).
  * Party GST Registration Type: Regular, Composition, Unregistered, Consumer, Overseas/SEZ.
* **Outputs**:
  * GSTR-1 JSON payload compliant with GST Portal schema v1.1.
  * GSTR-3B summary numbers.

### 7.3 Tax Partitioning Engine Logic

```
IF (buyer_registration == 'SEZ' OR place_of_supply != company_state_code):
    tax_type = 'IGST'
    apply_rate(tax_type, item_gst_rate)
ELSE:
    tax_type = 'INTRA'
    apply_rate('CGST', item_gst_rate / 2.0)
    apply_rate('SGST', item_gst_rate / 2.0)
```

### 7.4 Database Entities and Relationships (SQL DDL)

```sql
CREATE TABLE gst_registrations (
    reg_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(company_id) ON DELETE CASCADE,
    gstin VARCHAR(15) NOT NULL,
    legal_name VARCHAR(255) NOT NULL,
    trade_name VARCHAR(255),
    state_code VARCHAR(2) NOT NULL,
    registration_type VARCHAR(20) DEFAULT 'Regular', -- Regular, Composition
    is_primary BOOLEAN DEFAULT TRUE,
    CONSTRAINT check_gstin_format CHECK (gstin ~ '^[0-9]{2}[A-Z]{5}[0-9]{4}[A-Z]{1}[1-9A-Z]{1}Z[0-9A-Z]{1}$')
);

CREATE TABLE hsn_sac_directory (
    code VARCHAR(10) PRIMARY KEY,
    description TEXT NOT NULL,
    type VARCHAR(5) CHECK (type IN ('HSN', 'SAC')),
    default_rate NUMERIC(5, 2) NOT NULL
);
```

### 7.5 Error Handling and Edge Cases
* **Mismatched Tax Ledgers**: Attempting to select an IGST ledger on an intra-state voucher warns user or blocks voucher acceptance.
* **E-Way Bill Threshold**: Invoices with taxable value exceeding ₹50,000 trigger mandatory capture of transporter ID, distance, and vehicle number.

### 7.6 Acceptance Tests
* **TC-GST-01**: Validate GSTIN `27AAAAA0000A1Z5` (valid). Validate `27AAAAA0000A1Z` (invalid length, rejected).
* **TC-GST-02**: Generate GSTR-1 JSON for a company with 10 B2B sales and 5 B2C sales. Validate JSON against the public GSTN schema.

---

## Module 8: User Permissions and Audit Logs (Edit Log Mode)

### 8.1 Functional Requirements
* Role-Based Access Control (RBAC): Admin, Data Entry, Auditor, Read-Only roles.
* Granular Permissions: Create, Alter, Delete, Print, Export permissions per module.
* **MCA Statutory Audit Trail Compliance (Edit Log)**:
  * Implements requirements equivalent to TallyPrime Edit Log / Companies Act Rule 3(1).
  * **Permanent Audit Trail**: Every insert, update, soft-delete, or cancellation of a master or voucher must record an unalterable audit log entry.
  * **No Log Modification or Deletion**: Audit logs cannot be disabled, edited, or purged by any user, including administrators.

### 8.2 Inputs and Outputs
* **Inputs**:
  * User credentials, session tokens, action triggers.
* **Outputs**:
  * Immutable Audit Log entries containing: Timestamp (UTC), User ID, Action (`CREATE`, `UPDATE`, `CANCEL`, `DELETE`), Entity Type, Entity ID, Snapshot of record **Before**, Snapshot of record **After**.

### 8.3 Business Rules and Immutability Safeguards
* **Cryptographic Chaining**: Each audit log record contains a SHA-256 hash calculated from:
  $$\text{Hash}_n = \text{SHA256}(\text{LogID} \,\|\, \text{Timestamp} \,\|\, \text{DataBefore} \,\|\, \text{DataAfter} \,\|\, \text{Hash}_{n-1})$$
  This forms a tamper-evident blockchain ledger within the database.
* **Database Triggers**: Implemented via database-level `BEFORE UPDATE OR DELETE` triggers that throw an uncatchable exception if any process tries to mutate the `audit_trail` table.

### 8.4 Database Entities and Relationships (SQL DDL)

```sql
CREATE TABLE users (
    user_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    full_name VARCHAR(150),
    role VARCHAR(50) DEFAULT 'DataEntry',
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE audit_trail (
    log_id BIGSERIAL PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(company_id),
    table_name VARCHAR(100) NOT NULL,
    record_id UUID NOT NULL,
    action VARCHAR(10) NOT NULL CHECK (action IN ('INSERT', 'UPDATE', 'CANCEL', 'DELETE')),
    user_id UUID REFERENCES users(user_id),
    timestamp TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    data_before JSONB,
    data_after JSONB,
    prev_hash VARCHAR(64),
    record_hash VARCHAR(64) NOT NULL
);

-- Immutable Trigger on audit_trail to prevent updates or deletes
CREATE OR REPLACE FUNCTION prevent_audit_tampering()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Audit trail records are strictly immutable and cannot be updated or deleted.';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_immutable_audit
BEFORE UPDATE OR DELETE ON audit_trail
FOR EACH ROW EXECUTE FUNCTION prevent_audit_tampering();
```

### 8.5 Error Handling and Edge Cases
* **Audit Tampering Attempt**: Direct SQL query `DELETE FROM audit_trail` fails immediately with fatal trigger exception.
* **Orphan Audit Entries**: Cascade deletions of parent entities do not cascade to the audit log; the log retains the historical JSONB payload.

### 8.6 Acceptance Tests
* **TC-AUD-01**: Alter amount of Voucher V-101 from ₹5,000 to ₹6,500. Verify entry in `audit_trail` showing `data_before.total_amount = 5000` and `data_after.total_amount = 6500`.
* **TC-AUD-02**: Attempt to execute `UPDATE audit_trail SET action = 'ALTER'`. Must fail with trigger error.

---

## Module 9: Data Backup, Restoration, and Reporting

### 9.1 Functional Requirements
* **Lossless Single-File Archive**: Export all company tables, masters, vouchers, and audit logs into a compressed, tamper-verified backup bundle (`.tar.gz` / `.zip`).
* **Checksum Verification**: Every backup package contains a manifest with SHA-256 digests for all internal data files.
* **Point-in-Time Restore**: Ability to restore a backup to a new company ID without overwriting or conflicting with active companies.
* **Multi-Format Export Engine**: Export any report or document to:
  * PDF (vector formatted invoice/report)
  * Excel (`.xlsx`)
  * CSV (standard comma-separated values)
  * XML / JSON

### 9.2 Inputs and Outputs
* **Inputs**:
  * Source `company_id`, target destination path, optional password encryption.
* **Outputs**:
  * Standalone backup archive: `Backup_[CompanyNumber]_[YYYYMMDD_HHMMSS].tlypkg`.
  * Restoration Report: Total vouchers, ledgers, and items imported.

### 9.3 Database Backup Schema & Export Manifest

```json
{
  "manifest_version": "1.0",
  "software": "Independent Clean-Room Accounting System",
  "company_number": "10001",
  "company_name": "Acme Technologies Pvt Ltd",
  "backup_timestamp": "2026-10-22T04:30:00Z",
  "schema_version": "1.0.0",
  "file_checksums": {
    "company.json": "a3b8...",
    "chart_of_accounts.json": "c4d1...",
    "vouchers.json": "9f2e...",
    "inventory.json": "5b7a...",
    "audit_trail.json": "1e8c..."
  },
  "package_sha256": "856e..."
}
```

### 9.4 Error Handling and Edge Cases
* **Corrupted Backup Archive**: If any internal file hash fails to match the manifest, restoration aborts immediately before any database write occurs.
* **Version Mismatch**: Restoring a backup created in a newer schema version halts with `ERR_BACKUP_SCHEMA_INCOMPATIBLE`.

### 9.5 Acceptance Tests
* **TC-BAK-01**: Execute full backup of Company 10001. Delete company. Restore backup into Company 10002. Verify exact ledger and balance sheet equality.

---

## Module 10: Documented XML and ODBC Interoperability Requirements

### 10.1 Functional Requirements
* **HTTP XML Integration Server**:
  * Listen on HTTP loopback port 9000 (configurable).
  * Handle HTTP POST requests carrying XML payloads.
  * Support `<ENVELOPE>` schema [VERIFIED: extracted directly from `.rdata`]:
    * Export Requests: `<ENVELOPE><HEADER><TALLYREQUEST>Export Data</TALLYREQUEST></HEADER><BODY><EXPORTDATA>...</EXPORTDATA></BODY></ENVELOPE>`.
    * Import Transactions: `<ENVELOPE><HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER><BODY><IMPORTDATA><REQUESTDESC>...</REQUESTDESC><TALLYMESSAGE>...</TALLYMESSAGE></IMPORTDATA></BODY></ENVELOPE>`.
* **ODBC Integration Bridge**:
  * Provide an ANSI SQL-92 query endpoint that translates relational SQL queries into accounting dataset results.
  * Support queries such as:
    `SELECT $Name, $Parent, $ClosingBalance FROM Ledger`
    `SELECT $Date, $VoucherNumber, $PartyLedgerName, $Amount FROM Voucher`

### 10.2 XML Envelope Schema Specification

```xml
<!-- Example Clean-Room Compatible XML Export Response -->
<ENVELOPE>
  <HEADER>
    <VERSION>1.0</VERSION>
    <STATUS>SUCCESS</STATUS>
  </HEADER>
  <BODY>
    <DATA>
      <COLLECTION>
        <LEDGER NAME="HDFC Bank" ACTION="Create">
          <NAME>HDFC Bank</NAME>
          <PARENT>Bank Accounts</PARENT>
          <OPENINGBALANCE>-50000.00</OPENINGBALANCE>
          <CLOSINGBALANCE>-85400.00</CLOSINGBALANCE>
          <ISBILLWISE>No</ISBILLWISE>
        </LEDGER>
      </COLLECTION>
    </DATA>
  </BODY>
</ENVELOPE>
```

### 10.3 REST & SOAP Endpoint Specifications

| Protocol / Transport | Port | Method | URI | Payload / Format |
| :--- | :---: | :---: | :--- | :--- |
| **HTTP Loopback** | `9000` | `POST` | `/` | XML `<ENVELOPE>` containing `<EXPORTDATA>` or `<IMPORTDATA>` |
| **REST API** | `9001` | `GET` | `/api/v1/companies` | JSON: List of active companies |
| **REST API** | `9001` | `POST` | `/api/v1/vouchers` | JSON: Create new multi-line voucher |
| **ODBC Wire Server** | `9002` | TCP | Native | PostgreSQL wire protocol allowing direct BI/ODBC connection |

### 10.4 Error Handling and Edge Cases
* **Malformed XML**: If incoming XML body has syntax errors or mismatched tags, return XML response:
  ```xml
  <ENVELOPE>
    <HEADER><STATUS>FAILURE</STATUS></HEADER>
    <BODY><ERROR>Start and end tags are not same</ERROR></BODY>
  </ENVELOPE>
  ```
* **Unknown Object in Collection**: When an unknown field is requested, ignore gracefully rather than terminating the session.

### 10.5 Acceptance Tests
* **TC-INT-01**: Transmit XML POST request to `http://localhost:9000/` requesting `Ledger` list. Verify valid XML `<ENVELOPE>` returned within 200ms.
* **TC-INT-02**: Connect external Python ODBC / Excel client to port 9002. Execute `SELECT * FROM ledgers`. Verify rowset output.

---

## 11. Unresolved Requirements & Research Agenda

Before writing production application code, the following architectural decisions must be aligned:

1. **User Interface Framework**:
   * *Option A*: Traditional terminal-style TUI (using Go `bubbletea` or Rust `ratatui`) providing exact keyboard-first hotkey parity.
   * *Option B*: Modern Desktop GUI (Electron, Tauri, or Flutter) with rich styling and web capabilities.
2. **Database Engine Choice for Local Installation**:
   * *Option A*: Embedded SQLite (zero-configuration, single-file per company, fast startup).
   * *Option B*: Embedded or client-server PostgreSQL (powerful concurrency, native JSONB, robust trigger-level audit trails).
3. **E-Way Bill & E-Invoice Direct API Integration**:
   * Direct GSP (GST Suvidha Provider) integration requirements for real-time IRN (Invoice Reference Number) generation from within sales invoices.
4. **TDL Compatibility Layer Scope**:
   * Decide whether to build a full TDL language parser/compiler to run legacy third-party `.tcp` scripts, or expose a modern TypeScript / Python plugin SDK.
