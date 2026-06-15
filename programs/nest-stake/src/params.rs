#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct InitializeStakingParams {
    pub cooldown_seconds: i64,
    pub revenue_vesting_seconds: i64,
}
