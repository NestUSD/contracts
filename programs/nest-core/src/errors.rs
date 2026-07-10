#[error_code]
pub enum CoreError {
    #[msg("invalid parameter")]
    InvalidParameter,
    #[msg("math overflow")]
    MathOverflow,
    #[msg("protocol is paused")]
    Paused,
    #[msg("unauthorized")]
    Unauthorized,
    #[msg("collateral is paused")]
    CollateralPaused,
    #[msg("borrow is paused")]
    BorrowPaused,
    #[msg("withdraw is paused")]
    WithdrawPaused,
    #[msg("deposit cap exceeded")]
    DepositCapExceeded,
    #[msg("debt cap exceeded")]
    DebtCapExceeded,
    #[msg("vault debt cap exceeded")]
    VaultDebtCapExceeded,
    #[msg("insufficient collateral")]
    InsufficientCollateral,
    #[msg("vault is healthy")]
    VaultHealthy,
    #[msg("oracle error")]
    OracleError,
    #[msg("psm cap exceeded")]
    PsmCapExceeded,
    #[msg("psm has insufficient idle liquidity")]
    PsmInsufficientLiquidity,
    #[msg("psm has no realized surplus usdc to route")]
    PsmNoSurplus,
    #[msg("psm kamino deployment target is already reached")]
    PsmKaminoTargetReached,
    #[msg("remaining Kamino cTokens do not cover tracked PSM principal")]
    PsmKaminoPrincipalNotCovered,
    #[msg("amount does not fit in SPL token u64 amount")]
    AmountOverflow,
    #[msg("token transfer fee or hook reduced received amount")]
    TransferFeeNotSupported,
    #[msg("bad debt must be covered before routing psm surplus as yield")]
    BadDebtOutstanding,
    #[msg("insufficient protocol revenue")]
    InsufficientProtocolRevenue,
    #[msg("liquidation sale proceeds do not cover seized vault principal")]
    InsufficientLiquidationProceeds,
    #[msg("psm outflow circuit breaker configuration is invalid")]
    InvalidPsmOutflowCircuitBreaker,
    #[msg("psm outflow circuit breaker limit exceeded")]
    PsmOutflowLimitExceeded,
}
