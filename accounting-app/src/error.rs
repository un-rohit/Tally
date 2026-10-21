use thiserror::Error;

#[derive(Error, Debug)]
pub enum AccountingError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Mathematical overflow / checked arithmetic error: {0}")]
    MathOverflow(String),

    #[error("Unbalanced voucher: Debits ({debits}) do not equal Credits ({credits})")]
    UnbalancedVoucher { debits: i64, credits: i64 },

    #[error("Voucher date error: {0}")]
    InvalidVoucherDate(String),

    #[error("Company boundary violation: {0}")]
    CompanyBoundaryViolation(String),

    #[error("Voucher immutability error: {0}")]
    VoucherImmutable(String),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, AccountingError>;
