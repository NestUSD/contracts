#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct InitializeStakingParams {
    // Retained only to preserve the deployed initialize instruction ABI.
    pub cooldown_seconds: i64,
    pub revenue_vesting_seconds: i64,
}
