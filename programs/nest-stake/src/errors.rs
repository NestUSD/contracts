#[error_code]
pub enum StakeError {
    #[msg("staking is paused")]
    Paused,
    #[msg("unauthorized")]
    Unauthorized,
    #[msg("math overflow")]
    MathOverflow,
    #[msg("invalid parameter")]
    InvalidParameter,
    #[msg("cooldown is still active")]
    CooldownActive,
    #[msg("unstake claim window is still active")]
    ClaimWindowActive,
    #[msg("unstake claim window has expired")]
    ClaimWindowExpired,
    #[msg("protocol bad debt must be resolved before this operation")]
    BadDebtOutstanding,
    #[msg("insufficient assets")]
    InsufficientAssets,
    #[msg("insolvent")]
    Insolvent,
    #[msg("pending withdrawal already completed")]
    AlreadyCompleted,
    #[msg("amount does not fit in SPL token u64 amount")]
    AmountOverflow,
    #[msg("token transfer fee or hook reduced received amount")]
    TransferFeeNotSupported,
    #[msg("staking capacity for target APR exceeded")]
    StakeCapacityExceeded,
}
