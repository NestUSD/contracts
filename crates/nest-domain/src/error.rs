#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestError {
    MathOverflow,
    DivisionByZero,
    InvalidParameter,
    PriceNotPositive,
    ConfidenceTooWide,
    OracleStale,
    WrongFeed,
    CollateralPaused,
    BorrowPaused,
    WithdrawPaused,
    DepositCapExceeded,
    DebtCapExceeded,
    VaultDebtCapExceeded,
    InsufficientCollateral,
    VaultHealthy,
    CooldownActive,
    ClaimWindowActive,
    ClaimWindowExpired,
    LegacyWithdrawalNotMigrated,
    InsufficientAssets,
    Insolvent,
    PrincipalNotCovered,
}

pub type Result<T> = core::result::Result<T, NestError>;
