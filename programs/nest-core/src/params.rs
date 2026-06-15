#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct InitializeProtocolParams {
    pub psm_cap: u128,
    pub protocol_debt_cap: u128,
    pub stability_fee_apr_bps: u64,
    pub psm_swap_in_fee_bps: u16,
    pub psm_swap_out_fee_bps: u16,
    pub kamino_program_id: Pubkey,
    pub staker_target_apr_bps: u64,
    pub staker_capacity_kamino_apr_bps: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct AddCollateralParams {
    pub collateral_mint: Pubkey,
    pub token_program: Pubkey,
    pub symbol: [u8; 16],
    pub collateral_decimals: u8,
    pub xstock_usd_feed_id: [u8; 32],
    pub underlying_usd_feed_id: [u8; 32],
    pub redemption_rate_feed_id: [u8; 32],
    pub borrow_ltv_bps: u16,
    pub liquidation_threshold_bps: u16,
    pub liquidation_penalty_bps: u16,
    pub close_factor_bps: u16,
    pub max_confidence_bps: u16,
    pub max_staleness_seconds: i64,
    pub closed_market_max_staleness_seconds: i64,
    pub underlying_closed_market_max_staleness_seconds: i64,
    pub closed_market_haircut_bps: u16,
    pub per_vault_debt_cap: u128,
    pub protocol_debt_cap: u128,
    pub deposit_cap_raw: u128,
}
