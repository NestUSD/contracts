use crate::*;

pub fn initialize_protocol(
    ctx: Context<InitializeProtocol>,
    params: InitializeProtocolParams,
) -> Result<()> {
    require!(
        params.psm_cap > 0
            && params.protocol_debt_cap > 0
            && params.stability_fee_apr_bps <= MAX_STABILITY_FEE_APR_BPS
            && params.staker_target_apr_bps <= MAX_STAKER_TARGET_APR_BPS
            && params.staker_capacity_kamino_apr_bps <= MAX_STAKER_CAPACITY_KAMINO_APR_BPS
            && params.psm_swap_in_fee_bps == 0
            && params.psm_swap_out_fee_bps == 0
            && valid_kamino_program_id(params.kamino_program_id),
        CoreError::InvalidParameter
    );
    require!(
        ctx.accounts.psm_usdc_vault.amount == 0
            && ctx.accounts.insurance_nusd_vault.amount == 0
            && ctx.accounts.staker_revenue_nusd_vault.amount == 0
            && ctx.accounts.protocol_revenue_nusd_vault.amount == 0,
        CoreError::InvalidParameter
    );
    require_token_account_unencumbered(&ctx.accounts.psm_usdc_vault)?;
    require_token_account_unencumbered(&ctx.accounts.insurance_nusd_vault)?;
    require_token_account_unencumbered(&ctx.accounts.staker_revenue_nusd_vault)?;
    require_token_account_unencumbered(&ctx.accounts.protocol_revenue_nusd_vault)?;
    require_distinct_protocol_init_vaults(
        ctx.accounts.psm_usdc_vault.key(),
        ctx.accounts.insurance_nusd_vault.key(),
        ctx.accounts.staker_revenue_nusd_vault.key(),
        ctx.accounts.protocol_revenue_nusd_vault.key(),
    )?;
    require!(
        ctx.accounts.nusd_mint.decimals == STABLECOIN_DECIMALS
            && ctx.accounts.usdc_mint.decimals == STABLECOIN_DECIMALS,
        CoreError::InvalidParameter
    );
    require!(
        ctx.accounts.nusd_mint.supply == 0,
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        ctx.accounts.nusd_token_program.key(),
        SPL_TOKEN_PROGRAM_ID,
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        ctx.accounts.usdc_token_program.key(),
        SPL_TOKEN_PROGRAM_ID,
        CoreError::InvalidParameter
    );
    require!(
        ctx.accounts.nusd_mint.mint_authority == COption::Some(ctx.accounts.protocol.key()),
        CoreError::InvalidParameter
    );
    require!(
        ctx.accounts.nusd_mint.freeze_authority == COption::None,
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        *ctx.accounts.nusd_mint.to_account_info().owner,
        ctx.accounts.nusd_token_program.key(),
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        *ctx.accounts.usdc_mint.to_account_info().owner,
        ctx.accounts.usdc_token_program.key(),
        CoreError::InvalidParameter
    );
    let (expected_staker_revenue_authority, _) =
        Pubkey::find_program_address(&[b"staking"], &NEST_STAKE_PROGRAM_ID);
    require_keys_eq!(
        ctx.accounts.staker_revenue_authority.key(),
        expected_staker_revenue_authority,
        CoreError::InvalidParameter
    );
    let protocol = &mut ctx.accounts.protocol;
    protocol.authority = ctx.accounts.authority.key();
    protocol.liquidation_authority = ctx.accounts.authority.key();
    protocol.nusd_mint = ctx.accounts.nusd_mint.key();
    protocol.usdc_mint = ctx.accounts.usdc_mint.key();
    protocol.psm_usdc_vault = ctx.accounts.psm_usdc_vault.key();
    protocol.insurance_nusd_vault = ctx.accounts.insurance_nusd_vault.key();
    protocol.staker_revenue_nusd_vault = ctx.accounts.staker_revenue_nusd_vault.key();
    protocol.staker_revenue_authority = ctx.accounts.staker_revenue_authority.key();
    protocol.protocol_revenue_nusd_vault = ctx.accounts.protocol_revenue_nusd_vault.key();
    protocol.total_debt = 0;
    protocol.total_uncollected_fees = 0;
    protocol.realized_revenue_for_stakers = 0;
    protocol.realized_revenue_for_protocol = 0;
    protocol.insurance_fund_nusd = 0;
    protocol.staker_target_revenue_due = 0;
    protocol.bad_debt_nusd = 0;
    protocol.psm_usdc_liabilities = 0;
    protocol.psm_nusd_supply = 0;
    protocol.psm_idle_usdc = 0;
    protocol.psm_kamino_deployed_usdc = 0;
    protocol.psm_kamino_collateral_vault = Pubkey::default();
    protocol.psm_cap = params.psm_cap;
    protocol.protocol_debt_cap = params.protocol_debt_cap;
    protocol.stability_fee_apr_bps = params.stability_fee_apr_bps;
    protocol.psm_swap_in_fee_bps = params.psm_swap_in_fee_bps;
    protocol.psm_swap_out_fee_bps = params.psm_swap_out_fee_bps;
    protocol.kamino_program_id = params.kamino_program_id;
    protocol.staker_target_last_accrual_ts = Clock::get()?.unix_timestamp;
    protocol.staker_target_apr_bps = params.staker_target_apr_bps;
    protocol.staker_capacity_kamino_apr_bps = params.staker_capacity_kamino_apr_bps;
    protocol.pending_liquidation_principal = 0;
    protocol.pending_liquidation_fees = 0;
    protocol.psm_outflow_window_start_ts = 0;
    protocol.psm_outflow_window_usdc = 0;
    protocol.psm_outflow_limit_usdc = 0;
    protocol.psm_outflow_limit_bps = 0;
    protocol.psm_outflow_window_seconds = SECONDS_PER_DAY;
    protocol.psm_outflow_circuit_breaker_enabled = false;
    protocol.psm_outflow_window_basis_usdc = 0;
    protocol.buyback_authority = Pubkey::default();
    protocol.buyback_usdc_account = Pubkey::default();
    protocol.insurance_target_bps = domain::INSURANCE_TARGET_BPS;
    protocol.insurance_fee_share_bps = domain::INSURANCE_FEE_SHARE_BPS;
    protocol.reserved = false;
    protocol.paused = false;
    protocol.bump = ctx.bumps.protocol;
    Ok(())
}

pub fn add_collateral(ctx: Context<AddCollateral>, params: AddCollateralParams) -> Result<()> {
    assert_authority(&ctx.accounts.protocol, &ctx.accounts.authority)?;
    validate_collateral_feed_config(&params.xstock_usd_feed_id)?;
    require!(
        params.collateral_decimals > 0
            && params.collateral_decimals <= 18
            && params.borrow_ltv_bps > 0
            && params.borrow_ltv_bps < params.liquidation_threshold_bps
            && params.liquidation_threshold_bps < domain::BPS_DENOMINATOR as u16
            && params.liquidation_penalty_bps > 0
            && params.liquidation_penalty_bps <= domain::BPS_DENOMINATOR as u16
            && params.close_factor_bps > 0
            && params.close_factor_bps <= domain::BPS_DENOMINATOR as u16
            && params.max_confidence_bps > 0
            && params.max_confidence_bps <= domain::BPS_DENOMINATOR as u16
            && params.max_staleness_seconds > 0
            && params.max_staleness_seconds <= MAX_XSTOCK_PRICE_STALENESS_SECONDS
            && valid_collateral_token_program(params.token_program)
            && params.per_vault_debt_cap > 0
            && params.protocol_debt_cap > 0
            && params.deposit_cap_raw > 0,
        CoreError::InvalidParameter
    );
    require!(
        ctx.accounts.collateral_mint.decimals == params.collateral_decimals
            && ctx.accounts.collateral_vault.key() != ctx.accounts.insurance_collateral_vault.key(),
        CoreError::InvalidParameter
    );
    require_no_protocol_value_vault_aliases(
        &ctx.accounts.protocol,
        ctx.accounts.collateral_vault.key(),
        ctx.accounts.insurance_collateral_vault.key(),
    )?;
    require!(
        ctx.accounts.collateral_vault.amount == 0
            && ctx.accounts.insurance_collateral_vault.amount == 0,
        CoreError::InvalidParameter
    );
    require_token_account_unencumbered(&ctx.accounts.collateral_vault)?;
    require_token_account_unencumbered(&ctx.accounts.insurance_collateral_vault)?;
    require_keys_eq!(
        *ctx.accounts.collateral_mint.to_account_info().owner,
        ctx.accounts.collateral_token_program.key(),
        CoreError::InvalidParameter
    );
    let config = &mut ctx.accounts.collateral_config;
    config.protocol = ctx.accounts.protocol.key();
    config.collateral_mint = ctx.accounts.collateral_mint.key();
    config.collateral_vault = ctx.accounts.collateral_vault.key();
    config.insurance_collateral_vault = ctx.accounts.insurance_collateral_vault.key();
    config.token_program = ctx.accounts.collateral_token_program.key();
    config.symbol = params.symbol;
    config.collateral_decimals = params.collateral_decimals;
    config.xstock_usd_feed_id = params.xstock_usd_feed_id;
    config.reserved_underlying_usd_feed_id = [0; 32];
    config.reserved_redemption_rate_feed_id = [0; 32];
    config.borrow_ltv_bps = params.borrow_ltv_bps;
    config.liquidation_threshold_bps = params.liquidation_threshold_bps;
    config.liquidation_penalty_bps = params.liquidation_penalty_bps;
    config.close_factor_bps = params.close_factor_bps;
    config.max_confidence_bps = params.max_confidence_bps;
    config.max_staleness_seconds = params.max_staleness_seconds;
    config.reserved_closed_market_max_staleness_seconds = 0;
    config.reserved_underlying_closed_market_max_staleness_seconds = 0;
    config.reserved_closed_market_haircut_bps = 0;
    config.per_vault_debt_cap = params.per_vault_debt_cap;
    config.protocol_debt_cap = params.protocol_debt_cap;
    config.deposit_cap_raw = params.deposit_cap_raw;
    config.total_debt = 0;
    config.total_deposits_raw = 0;
    config.deposits_paused = false;
    config.borrows_paused = false;
    config.withdraws_paused = false;
    config.bump = ctx.bumps.collateral_config;
    Ok(())
}

pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.owner = ctx.accounts.owner.key();
    vault.collateral_config = ctx.accounts.collateral_config.key();
    vault.collateral_raw = 0;
    vault.principal_debt = 0;
    vault.accrued_fee = 0;
    vault.last_accrual_ts = Clock::get()?.unix_timestamp;
    vault.bump = ctx.bumps.vault;
    Ok(())
}

pub(crate) fn require_no_protocol_value_vault_aliases(
    protocol: &Protocol,
    collateral_vault: Pubkey,
    insurance_collateral_vault: Pubkey,
) -> Result<()> {
    let market_vaults = [collateral_vault, insurance_collateral_vault];
    let protocol_value_vaults = [
        protocol.psm_usdc_vault,
        protocol.insurance_nusd_vault,
        protocol.staker_revenue_nusd_vault,
        protocol.protocol_revenue_nusd_vault,
    ];
    for market_vault in market_vaults {
        for protocol_vault in protocol_value_vaults {
            require_keys_neq!(market_vault, protocol_vault, CoreError::InvalidParameter);
        }
    }
    Ok(())
}

pub(crate) fn require_distinct_protocol_init_vaults(
    psm_usdc_vault: Pubkey,
    insurance_nusd_vault: Pubkey,
    staker_revenue_nusd_vault: Pubkey,
    protocol_revenue_nusd_vault: Pubkey,
) -> Result<()> {
    let vaults = [
        psm_usdc_vault,
        insurance_nusd_vault,
        staker_revenue_nusd_vault,
        protocol_revenue_nusd_vault,
    ];
    for i in 0..vaults.len() {
        for j in (i + 1)..vaults.len() {
            require_keys_neq!(vaults[i], vaults[j], CoreError::InvalidParameter);
        }
    }
    Ok(())
}

pub(crate) fn valid_collateral_token_program(token_program: Pubkey) -> bool {
    token_program == SPL_TOKEN_PROGRAM_ID || token_program == TOKEN_2022_PROGRAM_ID
}
