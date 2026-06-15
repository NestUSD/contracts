use crate::*;

pub fn rebalance_psm_usdc_to_kamino(ctx: Context<PsmKamino>, max_amount: u64) -> Result<()> {
    require!(max_amount > 0, CoreError::InvalidParameter);
    require!(!ctx.accounts.protocol.paused, CoreError::Paused);
    assert_kamino_psm_accounts(ctx.accounts)?;

    let protocol = &ctx.accounts.protocol;
    let target_deployed =
        psm_kamino_target_deployed(protocol.psm_idle_usdc, protocol.psm_kamino_deployed_usdc)?;
    require!(
        protocol.psm_kamino_deployed_usdc < target_deployed,
        CoreError::PsmKaminoTargetReached
    );
    let target_delta = target_deployed
        .checked_sub(protocol.psm_kamino_deployed_usdc)
        .ok_or(error!(CoreError::MathOverflow))?;
    let amount_u128 = core::cmp::min(
        core::cmp::min(max_amount as u128, target_delta),
        protocol.psm_idle_usdc,
    );
    require!(amount_u128 > 0, CoreError::PsmInsufficientLiquidity);
    let amount = u128_to_u64(amount_u128)?;

    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    let psm_vault_before = ctx.accounts.psm_usdc_vault.amount;
    let kamino_collateral_before = ctx.accounts.protocol_kamino_collateral_vault.amount;

    invoke_klend_refresh_reserve(&psm_kamino_cpi_accounts(ctx.accounts))?;
    invoke_klend_deposit_reserve_liquidity(
        &psm_kamino_cpi_accounts(ctx.accounts),
        amount,
        signer_seeds,
    )?;

    ctx.accounts.psm_usdc_vault.reload()?;
    ctx.accounts.protocol_kamino_collateral_vault.reload()?;
    let actual_deployed = psm_vault_before
        .checked_sub(ctx.accounts.psm_usdc_vault.amount)
        .ok_or(error!(CoreError::TransferFeeNotSupported))?;
    require!(actual_deployed > 0, CoreError::TransferFeeNotSupported);
    require!(
        actual_deployed <= amount,
        CoreError::TransferFeeNotSupported
    );
    require!(
        ctx.accounts.protocol_kamino_collateral_vault.amount > kamino_collateral_before,
        CoreError::TransferFeeNotSupported
    );
    let protocol = &mut ctx.accounts.protocol;
    protocol.psm_idle_usdc = protocol
        .psm_idle_usdc
        .checked_sub(actual_deployed as u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_kamino_deployed_usdc = protocol
        .psm_kamino_deployed_usdc
        .checked_add(actual_deployed as u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    require_psm_accounting_invariants(&ctx.accounts.protocol, ctx.accounts.psm_usdc_vault.amount)?;
    Ok(())
}

pub fn redeem_psm_usdc_from_kamino(ctx: Context<PsmKamino>, collateral_amount: u64) -> Result<()> {
    require!(collateral_amount > 0, CoreError::InvalidParameter);
    assert_kamino_psm_accounts(ctx.accounts)?;
    require!(
        ctx.accounts.protocol.psm_kamino_deployed_usdc > 0,
        CoreError::PsmInsufficientLiquidity
    );

    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    let psm_vault_before = ctx.accounts.psm_usdc_vault.amount;
    let kamino_collateral_before = ctx.accounts.protocol_kamino_collateral_vault.amount;

    invoke_klend_refresh_reserve(&psm_kamino_cpi_accounts(ctx.accounts))?;
    invoke_klend_redeem_reserve_collateral(
        &psm_kamino_cpi_accounts(ctx.accounts),
        collateral_amount,
        signer_seeds,
    )?;

    ctx.accounts.psm_usdc_vault.reload()?;
    ctx.accounts.protocol_kamino_collateral_vault.reload()?;
    require_token_account_decrease(
        kamino_collateral_before,
        ctx.accounts.protocol_kamino_collateral_vault.amount,
        collateral_amount,
    )?;
    let usdc_received = ctx
        .accounts
        .psm_usdc_vault
        .amount
        .checked_sub(psm_vault_before)
        .ok_or(error!(CoreError::TransferFeeNotSupported))?;
    require!(usdc_received > 0, CoreError::TransferFeeNotSupported);

    let principal_return = core::cmp::min(
        usdc_received as u128,
        ctx.accounts.protocol.psm_kamino_deployed_usdc,
    );
    ctx.accounts.protocol.psm_kamino_deployed_usdc = ctx
        .accounts
        .protocol
        .psm_kamino_deployed_usdc
        .checked_sub(principal_return)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.protocol.psm_idle_usdc = ctx
        .accounts
        .protocol
        .psm_idle_usdc
        .checked_add(principal_return)
        .ok_or(error!(CoreError::MathOverflow))?;
    require_psm_accounting_invariants(&ctx.accounts.protocol, ctx.accounts.psm_usdc_vault.amount)?;
    Ok(())
}

pub fn realize_psm_kamino_yield(
    ctx: Context<RealizePsmKaminoYield>,
    collateral_amount: u64,
    min_yield: u64,
) -> Result<()> {
    require!(
        collateral_amount > 0 && min_yield > 0,
        CoreError::InvalidParameter
    );
    require!(!ctx.accounts.protocol.paused, CoreError::Paused);
    require!(
        ctx.accounts.protocol.bad_debt_nusd == 0,
        CoreError::BadDebtOutstanding
    );
    assert_kamino_psm_yield_accounts(ctx.accounts)?;
    require!(
        ctx.accounts.protocol.psm_kamino_deployed_usdc > 0,
        CoreError::PsmInsufficientLiquidity
    );

    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    let psm_vault_before = ctx.accounts.psm_usdc_vault.amount;
    let kamino_collateral_before = ctx.accounts.protocol_kamino_collateral_vault.amount;
    let deployed_before = ctx.accounts.protocol.psm_kamino_deployed_usdc;
    require!(
        collateral_amount < kamino_collateral_before,
        CoreError::PsmKaminoPrincipalNotCovered
    );

    // Redeem a slice, then prove the remaining cTokens still cover tracked
    // principal before treating the received USDC as yield.
    invoke_klend_refresh_reserve(&psm_kamino_yield_cpi_accounts(ctx.accounts))?;
    invoke_klend_redeem_reserve_collateral(
        &psm_kamino_yield_cpi_accounts(ctx.accounts),
        collateral_amount,
        signer_seeds,
    )?;

    ctx.accounts.psm_usdc_vault.reload()?;
    ctx.accounts.protocol_kamino_collateral_vault.reload()?;
    require_token_account_decrease(
        kamino_collateral_before,
        ctx.accounts.protocol_kamino_collateral_vault.amount,
        collateral_amount,
    )?;
    let usdc_received = ctx
        .accounts
        .psm_usdc_vault
        .amount
        .checked_sub(psm_vault_before)
        .ok_or(error!(CoreError::TransferFeeNotSupported))?;
    require!(usdc_received >= min_yield, CoreError::PsmNoSurplus);

    let remaining_value_lower_bound = domain::verify_kamino_profit_slice(
        collateral_amount as u128,
        ctx.accounts.protocol_kamino_collateral_vault.amount as u128,
        usdc_received as u128,
        deployed_before,
    )
    .map_err(map_core_error)?
    .remaining_value_lower_bound;
    require!(
        remaining_value_lower_bound >= deployed_before,
        CoreError::PsmKaminoPrincipalNotCovered
    );

    let staking_assets = staking_vault_nusd_from_account(
        &ctx.accounts.staking_state.to_account_info(),
        ctx.accounts.protocol.nusd_mint,
    )?;
    let (insurance_delta_u64, staker_delta_u64, protocol_delta_u64, minted_nusd_u64) =
        apply_realized_psm_yield_accounting(
            &mut ctx.accounts.protocol,
            ctx.accounts.psm_usdc_vault.amount,
            usdc_received,
            staking_assets,
            Clock::get()?.unix_timestamp,
        )?;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;

    if insurance_delta_u64 > 0 {
        let insurance_vault_before = ctx.accounts.insurance_nusd_vault.amount;
        token_mint_to_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.insurance_nusd_vault.to_account_info(),
            ctx.accounts.protocol.to_account_info(),
            insurance_delta_u64,
            ctx.accounts.nusd_mint.decimals,
            signer_seeds,
        )?;
        ctx.accounts.insurance_nusd_vault.reload()?;
        require_token_account_increase(
            insurance_vault_before,
            ctx.accounts.insurance_nusd_vault.amount,
            insurance_delta_u64,
        )?;
    }
    if staker_delta_u64 > 0 {
        let staker_revenue_before = ctx.accounts.staker_revenue_nusd_vault.amount;
        token_mint_to_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.staker_revenue_nusd_vault.to_account_info(),
            ctx.accounts.protocol.to_account_info(),
            staker_delta_u64,
            ctx.accounts.nusd_mint.decimals,
            signer_seeds,
        )?;
        ctx.accounts.staker_revenue_nusd_vault.reload()?;
        require_token_account_increase(
            staker_revenue_before,
            ctx.accounts.staker_revenue_nusd_vault.amount,
            staker_delta_u64,
        )?;
    }
    if protocol_delta_u64 > 0 {
        let protocol_revenue_before = ctx.accounts.protocol_revenue_nusd_vault.amount;
        token_mint_to_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.protocol_revenue_nusd_vault.to_account_info(),
            ctx.accounts.protocol.to_account_info(),
            protocol_delta_u64,
            ctx.accounts.nusd_mint.decimals,
            signer_seeds,
        )?;
        ctx.accounts.protocol_revenue_nusd_vault.reload()?;
        require_token_account_increase(
            protocol_revenue_before,
            ctx.accounts.protocol_revenue_nusd_vault.amount,
            protocol_delta_u64,
        )?;
    }
    ctx.accounts.nusd_mint.reload()?;
    require_mint_supply_increase(
        nusd_supply_before,
        ctx.accounts.nusd_mint.supply,
        minted_nusd_u64,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.insurance_nusd_vault.amount,
        ctx.accounts.protocol.insurance_fund_nusd,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.protocol_revenue_nusd_vault.amount,
        ctx.accounts.protocol.realized_revenue_for_protocol,
    )?;
    require_psm_accounting_invariants(&ctx.accounts.protocol, ctx.accounts.psm_usdc_vault.amount)?;
    Ok(())
}
