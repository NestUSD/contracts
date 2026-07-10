#[account]
#[derive(InitSpace)]
pub struct Protocol {
    pub authority: Pubkey,
    pub liquidation_authority: Pubkey,
    pub nusd_mint: Pubkey,
    pub usdc_mint: Pubkey,
    pub psm_usdc_vault: Pubkey,
    pub insurance_nusd_vault: Pubkey,
    pub staker_revenue_nusd_vault: Pubkey,
    pub staker_revenue_authority: Pubkey,
    pub total_debt: u128,
    pub total_uncollected_fees: u128,
    /// Lifetime revenue routed to stakers. This is not an outstanding liability.
    pub realized_revenue_for_stakers: u128,
    pub insurance_fund_nusd: u128,
    pub bad_debt_nusd: u128,
    pub psm_usdc_liabilities: u128,
    pub psm_nusd_supply: u128,
    pub psm_idle_usdc: u128,
    pub psm_kamino_deployed_usdc: u128,
    pub psm_kamino_collateral_vault: Pubkey,
    pub psm_cap: u128,
    pub protocol_debt_cap: u128,
    pub stability_fee_apr_bps: u64,
    pub psm_swap_in_fee_bps: u16,
    pub psm_swap_out_fee_bps: u16,
    pub kamino_program_id: Pubkey,
    pub insurance_target_bps: u16,
    pub insurance_fee_share_bps: u16,
    pub reserved: bool,
    pub paused: bool,
    pub bump: u8,
    pub protocol_revenue_nusd_vault: Pubkey,
    pub realized_revenue_for_protocol: u128,
    pub staker_target_revenue_due: u128,
    pub staker_target_last_accrual_ts: i64,
    pub staker_target_apr_bps: u64,
    pub staker_capacity_kamino_apr_bps: u64,
    pub pending_liquidation_principal: u128,
    pub pending_liquidation_fees: u128,
    pub psm_outflow_window_start_ts: i64,
    pub psm_outflow_window_usdc: u128,
    pub psm_outflow_limit_usdc: u128,
    pub psm_outflow_limit_bps: u16,
    pub psm_outflow_window_seconds: i64,
    pub psm_outflow_circuit_breaker_enabled: bool,
    pub psm_outflow_window_basis_usdc: u128,
    pub buyback_authority: Pubkey,
    pub buyback_usdc_account: Pubkey,
}

#[account]
#[derive(InitSpace)]
pub struct CollateralConfig {
    pub protocol: Pubkey,
    pub collateral_mint: Pubkey,
    pub collateral_vault: Pubkey,
    pub insurance_collateral_vault: Pubkey,
    pub token_program: Pubkey,
    pub symbol: [u8; 16],
    pub collateral_decimals: u8,
    pub xstock_usd_feed_id: [u8; 32],
    // These legacy slots preserve the deployed account layout.
    pub reserved_underlying_usd_feed_id: [u8; 32],
    pub reserved_redemption_rate_feed_id: [u8; 32],
    pub borrow_ltv_bps: u16,
    pub liquidation_threshold_bps: u16,
    pub liquidation_penalty_bps: u16,
    pub close_factor_bps: u16,
    pub max_confidence_bps: u16,
    pub max_staleness_seconds: i64,
    pub reserved_closed_market_max_staleness_seconds: i64,
    pub reserved_underlying_closed_market_max_staleness_seconds: i64,
    pub reserved_closed_market_haircut_bps: u16,
    pub per_vault_debt_cap: u128,
    pub protocol_debt_cap: u128,
    pub deposit_cap_raw: u128,
    pub total_debt: u128,
    pub total_deposits_raw: u128,
    pub deposits_paused: bool,
    pub borrows_paused: bool,
    pub withdraws_paused: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub owner: Pubkey,
    pub collateral_config: Pubkey,
    pub collateral_raw: u128,
    pub principal_debt: u128,
    pub accrued_fee: u128,
    pub last_accrual_ts: i64,
    pub bump: u8,
}

impl Vault {
    pub fn total_debt(&self) -> Result<u128> {
        self.principal_debt
            .checked_add(self.accrued_fee)
            .ok_or(error!(CoreError::MathOverflow))
    }
}

#[account]
#[derive(InitSpace)]
pub struct LiquidationReceipt {
    pub protocol: Pubkey,
    pub collateral_config: Pubkey,
    pub vault: Pubkey,
    pub liquidator: Pubkey,
    pub collateral_mint: Pubkey,
    pub collateral_raw: u128,
    pub min_settlement_usdc: u128,
    pub target_settlement_usdc: u128,
    pub principal_debt: u128,
    pub accrued_fee: u128,
    pub started_at_ts: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct OracleSnapshot {
    pub protocol: Pubkey,
    pub collateral_config: Pubkey,
    pub xstock_usd: OraclePriceAccount,
    // These legacy slots preserve the deployed account layout.
    pub reserved_underlying_usd: OraclePriceAccount,
    pub reserved_redemption_rate: OraclePriceAccount,
    pub reserved_market_state: MarketStateAccount,
    pub reserved_calendar_valid_until_ts: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct NestPriceSignerConfig {
    pub protocol: Pubkey,
    pub authority: Pubkey,
    pub signer: Pubkey,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct OraclePriceAccount {
    pub feed_id: [u8; 32],
    pub price_e8: u64,
    pub confidence_e8: u64,
    pub publish_time: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum MarketStateAccount {
    Regular,
    Extended,
    Closed,
}
