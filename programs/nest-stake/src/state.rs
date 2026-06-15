#[account]
#[derive(InitSpace)]
pub struct StakingState {
    pub authority: Pubkey,
    pub nusd_mint: Pubkey,
    pub snusd_mint: Pubkey,
    pub staking_nusd_vault: Pubkey,
    pub revenue_nusd_vault: Pubkey,
    pub revenue_baseline_nusd: u128,
    pub total_shares: u128,
    pub total_user_shares: u128,
    pub staking_vault_nusd: u128,
    pub realized_loss_nusd: u128,
    pub unvested_revenue: u128,
    pub reserved_pending_claims: u128,
    pub vesting_start_ts: i64,
    pub vesting_end_ts: i64,
    pub last_vesting_sync_ts: i64,
    pub cooldown_seconds: i64,
    pub revenue_vesting_seconds: i64,
    pub paused: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct PendingWithdrawalAccount {
    pub owner: Pubkey,
    pub staking_state: Pubkey,
    pub shares: u128,
    pub request_ts: i64,
    pub assets_redeemed: u128,
    pub completed: bool,
    pub bump: u8,
}
