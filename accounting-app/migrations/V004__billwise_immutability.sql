-- ============================================================================
-- Migration: V004__billwise_immutability.sql
-- Enforce posted voucher immutability on bill allocations
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_bill_alloc_insert
BEFORE INSERT ON bill_allocations
FOR EACH ROW
WHEN (
    SELECT v.status FROM voucher_entries e 
    JOIN vouchers v ON e.voucher_id = v.voucher_id 
    WHERE e.entry_id = NEW.entry_id
) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot add bill allocations to an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_bill_alloc_update
BEFORE UPDATE ON bill_allocations
FOR EACH ROW
WHEN (
    SELECT v.status FROM voucher_entries e 
    JOIN vouchers v ON e.voucher_id = v.voucher_id 
    WHERE e.entry_id = OLD.entry_id
) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot modify bill allocations of an already posted voucher.');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_bill_alloc_delete
BEFORE DELETE ON bill_allocations
FOR EACH ROW
WHEN (
    SELECT v.status FROM voucher_entries e 
    JOIN vouchers v ON e.voucher_id = v.voucher_id 
    WHERE e.entry_id = OLD.entry_id
) = 'Posted'
BEGIN
    SELECT RAISE(ABORT, 'ERR_POSTED_VOUCHER_IMMUTABLE: Cannot delete bill allocations of an already posted voucher.');
END;
