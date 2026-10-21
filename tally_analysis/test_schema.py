import sqlite3

con = sqlite3.connect(':memory:')
with open('d:/Tally/schema.sql', 'r', encoding='utf-8') as f:
    sql = f.read()

con.executescript(sql)
cur = con.cursor()
cur.execute("SELECT count(1) FROM sqlite_master WHERE type='table'")
print("Tables count:", cur.fetchone()[0])
cur.execute("SELECT count(1) FROM sqlite_master WHERE type='trigger'")
print("Triggers count:", cur.fetchone()[0])
cur.execute("SELECT count(1) FROM sqlite_master WHERE type='index'")
print("Indexes count:", cur.fetchone()[0])

# Test 1: Verify Triggers on Vouchers
# Create company & FY
cur.execute("INSERT INTO companies (company_id, company_number, name, mailing_name, state_code) VALUES ('comp-1', '10001', 'Test Corp', 'Test Corp', '27')")
cur.execute("INSERT INTO financial_years (fy_id, company_id, fy_start_date, fy_end_date, books_beginning_date) VALUES ('fy-1', 'comp-1', '2026-04-01', '2027-03-31', '2026-04-01')")
cur.execute("INSERT INTO account_groups (group_id, company_id, name, nature) VALUES ('grp-1', 'comp-1', 'Bank Accounts', 'Asset')")
cur.execute("INSERT INTO ledgers (ledger_id, company_id, group_id, name) VALUES ('led-1', 'comp-1', 'grp-1', 'HDFC Bank')")
cur.execute("INSERT INTO ledgers (ledger_id, company_id, group_id, name) VALUES ('led-2', 'comp-1', 'grp-1', 'ICICI Bank')")
cur.execute("INSERT INTO voucher_types (voucher_type_id, company_id, name, base_type) VALUES ('vt-1', 'comp-1', 'Contra', 'Contra')")
cur.execute("INSERT INTO vouchers (voucher_id, company_id, fy_id, voucher_type_id, voucher_number, voucher_date, total_amount_paise, status) VALUES ('vch-1', 'comp-1', 'fy-1', 'vt-1', '1', '2026-05-01', 500000, 'Draft')")

# Add balanced entries (5000.00 Dr, 5000.00 Cr)
cur.execute("INSERT INTO voucher_entries (entry_id, voucher_id, line_number, ledger_id, entry_type, amount_paise) VALUES ('e-1', 'vch-1', 1, 'led-1', 'Dr', 500000)")
cur.execute("INSERT INTO voucher_entries (entry_id, voucher_id, line_number, ledger_id, entry_type, amount_paise) VALUES ('e-2', 'vch-1', 2, 'led-2', 'Cr', 500000)")

# Post the voucher (should succeed)
cur.execute("UPDATE vouchers SET status = 'Posted' WHERE voucher_id = 'vch-1'")
con.commit()
print("Balanced voucher posted successfully!")

# Test 2: Try to mutate posted voucher (should fail)
try:
    cur.execute("UPDATE vouchers SET total_amount_paise = 600000 WHERE voucher_id = 'vch-1'")
    print("FAILED: mutation allowed on posted voucher!")
except sqlite3.DatabaseError as e:
    print("SUCCESS: Mutation blocked by trigger:", e)

# Test 3: Unbalanced voucher post
cur.execute("INSERT INTO vouchers (voucher_id, company_id, fy_id, voucher_type_id, voucher_number, voucher_date, total_amount_paise, status) VALUES ('vch-2', 'comp-1', 'fy-1', 'vt-1', '2', '2026-05-02', 100000, 'Draft')")
cur.execute("INSERT INTO voucher_entries (entry_id, voucher_id, line_number, ledger_id, entry_type, amount_paise) VALUES ('e-3', 'vch-2', 1, 'led-1', 'Dr', 100000)")
try:
    cur.execute("UPDATE vouchers SET status = 'Posted' WHERE voucher_id = 'vch-2'")
    print("FAILED: Unbalanced voucher posted!")
except sqlite3.DatabaseError as e:
    print("SUCCESS: Unbalanced voucher post blocked by trigger:", e)

# Test 4: Audit trail tampering
cur.execute("INSERT INTO audit_trail (company_id, entity_type, entity_id, action, record_hash) VALUES ('comp-1', 'vouchers', 'vch-1', 'POST', 'hash-123')")
con.commit()
try:
    cur.execute("DELETE FROM audit_trail WHERE log_id = 1")
    print("FAILED: Audit trail deletion allowed!")
except sqlite3.DatabaseError as e:
    print("SUCCESS: Audit trail deletion blocked by trigger:", e)
