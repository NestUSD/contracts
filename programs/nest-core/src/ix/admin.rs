use crate::*;

pub fn set_paused(ctx: Context<MutateProtocol>, paused: bool) -> Result<()> {
    ctx.accounts.protocol.paused = paused;
    Ok(())
}

pub fn set_psm_outflow_circuit_breaker(
    ctx: Context<MutateProtocol>,
    enabled: bool,
    limit_usdc: u128,
    limit_bps: u16,
    window_seconds: i64,
) -> Result<()> {
    if enabled {
        require!(
            (limit_usdc > 0 || limit_bps > 0)
                && limit_bps <= domain::BPS_DENOMINATOR as u16
                && window_seconds > 0
                && window_seconds <= MAX_PSM_OUTFLOW_WINDOW_SECONDS,
            CoreError::InvalidPsmOutflowCircuitBreaker
        );
    } else {
        require!(
            limit_bps <= domain::BPS_DENOMINATOR as u16
                && (window_seconds == 0
                    || (window_seconds > 0 && window_seconds <= MAX_PSM_OUTFLOW_WINDOW_SECONDS)),
            CoreError::InvalidPsmOutflowCircuitBreaker
        );
    }
    ctx.accounts.protocol.psm_outflow_circuit_breaker_enabled = enabled;
    ctx.accounts.protocol.psm_outflow_limit_usdc = limit_usdc;
    ctx.accounts.protocol.psm_outflow_limit_bps = limit_bps;
    ctx.accounts.protocol.psm_outflow_window_seconds = if window_seconds == 0 {
        SECONDS_PER_DAY
    } else {
        window_seconds
    };
    ctx.accounts.protocol.psm_outflow_window_start_ts = 0;
    ctx.accounts.protocol.psm_outflow_window_usdc = 0;
    ctx.accounts.protocol.psm_outflow_window_basis_usdc = 0;
    Ok(())
}

pub fn set_liquidation_authority(
    ctx: Context<MutateProtocol>,
    liquidation_authority: Pubkey,
) -> Result<()> {
    require_keys_neq!(
        liquidation_authority,
        Pubkey::default(),
        CoreError::InvalidParameter
    );
    ctx.accounts.protocol.liquidation_authority = liquidation_authority;
    Ok(())
}

pub fn set_protocol_authority(ctx: Context<MutateProtocol>, authority: Pubkey) -> Result<()> {
    require_keys_neq!(authority, Pubkey::default(), CoreError::InvalidParameter);
    ctx.accounts.protocol.authority = authority;
    Ok(())
}

pub fn set_nest_price_signer(ctx: Context<SetNestPriceSigner>, signer: Pubkey) -> Result<()> {
    require_keys_neq!(signer, Pubkey::default(), CoreError::InvalidParameter);
    let config = &mut ctx.accounts.nest_price_signer;
    config.protocol = ctx.accounts.protocol.key();
    config.authority = ctx.accounts.authority.key();
    config.signer = signer;
    config.bump = ctx.bumps.nest_price_signer;
    Ok(())
}

pub fn set_staker_yield_params(
    ctx: Context<SetStakerYieldParams>,
    staker_target_apr_bps: u64,
    staker_capacity_kamino_apr_bps: u64,
) -> Result<()> {
    require!(
        staker_target_apr_bps <= MAX_STAKER_TARGET_APR_BPS
            && staker_capacity_kamino_apr_bps <= MAX_STAKER_CAPACITY_KAMINO_APR_BPS,
        CoreError::InvalidParameter
    );
    let staking_assets = staking_vault_nusd_from_account(
        &ctx.accounts.staking_state.to_account_info(),
        ctx.accounts.protocol.nusd_mint,
    )?;
    sync_staker_target_revenue(
        &mut ctx.accounts.protocol,
        staking_assets,
        Clock::get()?.unix_timestamp,
    )?;
    ctx.accounts.protocol.staker_target_apr_bps = staker_target_apr_bps;
    ctx.accounts.protocol.staker_capacity_kamino_apr_bps = staker_capacity_kamino_apr_bps;
    Ok(())
}

pub fn initialize_staker_revenue_accounting(
    ctx: Context<InitializeStakerRevenueAccounting>,
    outstanding_revenue: u64,
) -> Result<()> {
    initialize_staker_revenue_accounting_state(
        &mut ctx.accounts.protocol,
        outstanding_revenue as u128,
        ctx.accounts.staker_revenue_nusd_vault.amount as u128,
    )
}

pub(crate) fn initialize_staker_revenue_accounting_state(
    protocol: &mut Protocol,
    outstanding_revenue: u128,
    staker_revenue_vault_balance: u128,
) -> Result<()> {
    require!(protocol.paused, CoreError::ProtocolMustBePaused);
    require!(
        !protocol.staker_revenue_accounting_initialized,
        CoreError::StakerRevenueAccountingAlreadyInitialized
    );
    require!(
        outstanding_revenue <= staker_revenue_vault_balance,
        CoreError::InvalidParameter
    );
    protocol.realized_revenue_for_stakers = outstanding_revenue;
    protocol.staker_revenue_accounting_initialized = true;
    Ok(())
}

pub fn set_psm_kamino_collateral_vault(ctx: Context<SetPsmKaminoCollateralVault>) -> Result<()> {
    require!(
        ctx.accounts.protocol.psm_kamino_collateral_vault == Pubkey::default()
            && ctx.accounts.protocol.psm_kamino_deployed_usdc == 0
            && ctx.accounts.protocol_kamino_collateral_vault.amount == 0
            && ctx.accounts.protocol_kamino_collateral_vault.key()
                != ctx.accounts.protocol.psm_usdc_vault
            && ctx.accounts.protocol_kamino_collateral_vault.key()
                != ctx.accounts.protocol.insurance_nusd_vault
            && ctx.accounts.protocol_kamino_collateral_vault.key()
                != ctx.accounts.protocol.staker_revenue_nusd_vault
            && ctx.accounts.protocol_kamino_collateral_vault.key()
                != ctx.accounts.protocol.protocol_revenue_nusd_vault
            && ctx.accounts.protocol_kamino_collateral_vault.mint
                != ctx.accounts.protocol.usdc_mint
            && ctx.accounts.protocol_kamino_collateral_vault.mint
                != ctx.accounts.protocol.nusd_mint,
        CoreError::InvalidParameter
    );
    require_token_account_unencumbered(&ctx.accounts.protocol_kamino_collateral_vault)?;
    ctx.accounts.protocol.psm_kamino_collateral_vault =
        ctx.accounts.protocol_kamino_collateral_vault.key();
    Ok(())
}

pub fn set_buyback_config(ctx: Context<SetBuybackConfig>, buyback_authority: Pubkey) -> Result<()> {
    require_keys_neq!(
        buyback_authority,
        Pubkey::default(),
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        ctx.accounts.buyback_usdc_account.owner,
        buyback_authority,
        CoreError::InvalidParameter
    );
    require!(
        ctx.accounts.buyback_usdc_account.key() != ctx.accounts.protocol.psm_usdc_vault
            && ctx.accounts.buyback_usdc_account.key()
                != ctx.accounts.protocol.insurance_nusd_vault
            && ctx.accounts.buyback_usdc_account.key()
                != ctx.accounts.protocol.staker_revenue_nusd_vault
            && ctx.accounts.buyback_usdc_account.key()
                != ctx.accounts.protocol.protocol_revenue_nusd_vault,
        CoreError::InvalidParameter
    );
    ctx.accounts.protocol.buyback_authority = buyback_authority;
    ctx.accounts.protocol.buyback_usdc_account = ctx.accounts.buyback_usdc_account.key();
    Ok(())
}

pub fn set_collateral_paused(
    ctx: Context<MutateCollateral>,
    deposits_paused: bool,
    borrows_paused: bool,
    withdraws_paused: bool,
) -> Result<()> {
    ctx.accounts.collateral_config.deposits_paused = deposits_paused;
    ctx.accounts.collateral_config.borrows_paused = borrows_paused;
    ctx.accounts.collateral_config.withdraws_paused = withdraws_paused;
    Ok(())
}

pub fn trip_collateral_vault_coverage_breaker(
    ctx: Context<TripCollateralVaultCoverageBreaker>,
) -> Result<()> {
    require!(
        collateral_vault_coverage_broken(
            ctx.accounts.collateral_vault.amount,
            ctx.accounts.collateral_config.total_deposits_raw,
            ctx.accounts.collateral_vault.is_frozen(),
        ),
        CoreError::InvalidParameter
    );
    ctx.accounts.collateral_config.deposits_paused = true;
    ctx.accounts.collateral_config.borrows_paused = true;
    ctx.accounts.collateral_config.withdraws_paused = true;
    Ok(())
}

pub fn trip_collateral_emergency_breaker(
    ctx: Context<TripCollateralEmergencyBreaker>,
) -> Result<()> {
    ctx.accounts.collateral_config.deposits_paused = true;
    ctx.accounts.collateral_config.borrows_paused = true;
    ctx.accounts.collateral_config.withdraws_paused = true;
    Ok(())
}

pub fn set_collateral_risk_params(
    ctx: Context<MutateCollateral>,
    borrow_ltv_bps: u16,
    liquidation_threshold_bps: u16,
    liquidation_penalty_bps: u16,
    close_factor_bps: u16,
) -> Result<()> {
    require!(
        borrow_ltv_bps > 0
            && borrow_ltv_bps < liquidation_threshold_bps
            && liquidation_threshold_bps < domain::BPS_DENOMINATOR as u16
            && liquidation_penalty_bps > 0
            && liquidation_penalty_bps <= domain::BPS_DENOMINATOR as u16
            && close_factor_bps > 0
            && close_factor_bps <= domain::BPS_DENOMINATOR as u16,
        CoreError::InvalidParameter
    );
    ctx.accounts.collateral_config.borrow_ltv_bps = borrow_ltv_bps;
    ctx.accounts.collateral_config.liquidation_threshold_bps = liquidation_threshold_bps;
    ctx.accounts.collateral_config.liquidation_penalty_bps = liquidation_penalty_bps;
    ctx.accounts.collateral_config.close_factor_bps = close_factor_bps;
    Ok(())
}

pub fn set_collateral_caps(
    ctx: Context<MutateCollateral>,
    per_vault_debt_cap: u128,
    protocol_debt_cap: u128,
    deposit_cap_raw: u128,
) -> Result<()> {
    require!(
        per_vault_debt_cap > 0 && protocol_debt_cap > 0 && deposit_cap_raw > 0,
        CoreError::InvalidParameter
    );
    ctx.accounts.collateral_config.per_vault_debt_cap = per_vault_debt_cap;
    ctx.accounts.collateral_config.protocol_debt_cap = protocol_debt_cap;
    ctx.accounts.collateral_config.deposit_cap_raw = deposit_cap_raw;
    Ok(())
}

pub fn set_collateral_oracle_params(
    ctx: Context<MutateCollateral>,
    xstock_usd_feed_id: [u8; 32],
    max_confidence_bps: u16,
    max_staleness_seconds: i64,
) -> Result<()> {
    require!(
        max_confidence_bps > 0
            && max_confidence_bps <= domain::BPS_DENOMINATOR as u16
            && max_staleness_seconds > 0
            && max_staleness_seconds <= MAX_XSTOCK_PRICE_STALENESS_SECONDS,
        CoreError::InvalidParameter
    );
    validate_collateral_feed_config(&xstock_usd_feed_id)?;
    ctx.accounts.collateral_config.xstock_usd_feed_id = xstock_usd_feed_id;
    ctx.accounts
        .collateral_config
        .reserved_underlying_usd_feed_id = [0; 32];
    ctx.accounts
        .collateral_config
        .reserved_redemption_rate_feed_id = [0; 32];
    ctx.accounts.collateral_config.max_confidence_bps = max_confidence_bps;
    ctx.accounts.collateral_config.max_staleness_seconds = max_staleness_seconds;
    ctx.accounts
        .collateral_config
        .reserved_closed_market_max_staleness_seconds = 0;
    ctx.accounts
        .collateral_config
        .reserved_underlying_closed_market_max_staleness_seconds = 0;
    ctx.accounts
        .collateral_config
        .reserved_closed_market_haircut_bps = 0;
    Ok(())
}
