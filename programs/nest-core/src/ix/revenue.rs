use crate::*;

pub fn deposit_protocol_revenue(ctx: Context<DepositProtocolRevenue>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    require!(
        ctx.accounts.source_nusd_account.amount >= amount,
        CoreError::InvalidParameter
    );
    require_keys_neq!(
        ctx.accounts.source_nusd_account.key(),
        ctx.accounts.insurance_nusd_vault.key(),
        CoreError::InvalidParameter
    );
    require_keys_neq!(
        ctx.accounts.source_nusd_account.key(),
        ctx.accounts.staker_revenue_nusd_vault.key(),
        CoreError::InvalidParameter
    );
    require_keys_neq!(
        ctx.accounts.source_nusd_account.key(),
        ctx.accounts.protocol_revenue_nusd_vault.key(),
        CoreError::InvalidParameter
    );

    let staking_assets = staking_vault_nusd_from_account(
        &ctx.accounts.staking_state.to_account_info(),
        ctx.accounts.protocol.nusd_mint,
    )?;
    let allocation = apply_protocol_revenue_accounting(
        &mut ctx.accounts.protocol,
        staking_assets,
        Clock::get()?.unix_timestamp,
        amount as u128,
    )?;
    let bad_debt_repaid = u128_to_u64(allocation.bad_debt_repaid)?;
    let insurance = u128_to_u64(allocation.insurance)?;
    let stakers = u128_to_u64(allocation.stakers)?;
    let protocol_revenue = u128_to_u64(allocation.protocol)?;

    let source_before = ctx.accounts.source_nusd_account.amount;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;
    if bad_debt_repaid > 0 {
        token_burn_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.source_nusd_account.to_account_info(),
            ctx.accounts.source.to_account_info(),
            bad_debt_repaid,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
    }
    if insurance > 0 {
        let insurance_before = ctx.accounts.insurance_nusd_vault.amount;
        token_transfer_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.source_nusd_account.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.insurance_nusd_vault.to_account_info(),
            ctx.accounts.source.to_account_info(),
            insurance,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
        ctx.accounts.insurance_nusd_vault.reload()?;
        require_token_account_increase(
            insurance_before,
            ctx.accounts.insurance_nusd_vault.amount,
            insurance,
        )?;
    }
    if stakers > 0 {
        let staker_revenue_before = ctx.accounts.staker_revenue_nusd_vault.amount;
        token_transfer_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.source_nusd_account.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.staker_revenue_nusd_vault.to_account_info(),
            ctx.accounts.source.to_account_info(),
            stakers,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
        ctx.accounts.staker_revenue_nusd_vault.reload()?;
        require_token_account_increase(
            staker_revenue_before,
            ctx.accounts.staker_revenue_nusd_vault.amount,
            stakers,
        )?;
    }
    if protocol_revenue > 0 {
        let protocol_revenue_before = ctx.accounts.protocol_revenue_nusd_vault.amount;
        token_transfer_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.source_nusd_account.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.protocol_revenue_nusd_vault.to_account_info(),
            ctx.accounts.source.to_account_info(),
            protocol_revenue,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
        ctx.accounts.protocol_revenue_nusd_vault.reload()?;
        require_token_account_increase(
            protocol_revenue_before,
            ctx.accounts.protocol_revenue_nusd_vault.amount,
            protocol_revenue,
        )?;
    }

    ctx.accounts.source_nusd_account.reload()?;
    ctx.accounts.nusd_mint.reload()?;
    require_token_account_decrease(
        source_before,
        ctx.accounts.source_nusd_account.amount,
        amount,
    )?;
    require_mint_supply_decrease(
        nusd_supply_before,
        ctx.accounts.nusd_mint.supply,
        bad_debt_repaid,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.insurance_nusd_vault.amount,
        ctx.accounts.protocol.insurance_fund_nusd,
    )?;
    require_staker_revenue_covered(
        &ctx.accounts.protocol,
        ctx.accounts.staker_revenue_nusd_vault.amount,
        stakers as u128,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.protocol_revenue_nusd_vault.amount,
        ctx.accounts.protocol.realized_revenue_for_protocol,
    )?;

    emit!(ProtocolRevenueDeposited {
        protocol: ctx.accounts.protocol.key(),
        source: ctx.accounts.source.key(),
        source_nusd_account: ctx.accounts.source_nusd_account.key(),
        amount_nusd: amount,
        bad_debt_repaid_nusd: bad_debt_repaid,
        insurance_nusd: insurance,
        staker_nusd: stakers,
        protocol_nusd: protocol_revenue,
    });
    Ok(())
}

pub fn checkpoint_staker_target_revenue(ctx: Context<CheckpointStakerTargetRevenue>) -> Result<()> {
    let staking_assets = staking_vault_nusd_from_account(
        &ctx.accounts.staking_state.to_account_info(),
        ctx.accounts.protocol.nusd_mint,
    )?;
    sync_staker_target_revenue(
        &mut ctx.accounts.protocol,
        staking_assets,
        Clock::get()?.unix_timestamp,
    )
}

pub fn consume_staker_revenue(ctx: Context<ConsumeStakerRevenue>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    let staking_assets = staking_vault_nusd_from_account(
        &ctx.accounts.staking_state.to_account_info(),
        ctx.accounts.protocol.nusd_mint,
    )?;
    sync_staker_target_revenue(
        &mut ctx.accounts.protocol,
        staking_assets,
        Clock::get()?.unix_timestamp,
    )?;
    record_staker_revenue_consumption(&mut ctx.accounts.protocol, amount as u128)
}

pub(crate) fn record_staker_revenue_consumption(
    protocol: &mut Protocol,
    amount: u128,
) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    require!(
        protocol.staker_revenue_accounting_initialized,
        CoreError::StakerRevenueAccountingNotInitialized
    );
    protocol.realized_revenue_for_stakers = protocol
        .realized_revenue_for_stakers
        .checked_sub(amount)
        .ok_or(error!(CoreError::InvalidParameter))?;
    Ok(())
}

pub fn absorb_staking_loss(ctx: Context<AbsorbStakingLoss>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    record_staking_loss_absorption(&mut ctx.accounts.protocol, amount as u128)
}

pub(crate) fn record_staking_loss_absorption(protocol: &mut Protocol, amount: u128) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    let mut accounting = domain_protocol(protocol);
    accounting.cover_bad_debt(amount).map_err(map_core_error)?;
    protocol.bad_debt_nusd = accounting.bad_debt_nusd;
    Ok(())
}

pub fn realize_psm_yield(ctx: Context<RealizePsmYield>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    // This path routes only surplus USDC already sitting in the canonical
    // PSM vault above recorded idle liabilities.
    let staking_assets = staking_vault_nusd_from_account(
        &ctx.accounts.staking_state.to_account_info(),
        ctx.accounts.protocol.nusd_mint,
    )?;
    let (insurance_delta_u64, staker_delta_u64, protocol_delta_u64, minted_nusd_u64) =
        apply_realized_psm_yield_accounting(
            &mut ctx.accounts.protocol,
            ctx.accounts.psm_usdc_vault.amount,
            amount,
            staking_assets,
            Clock::get()?.unix_timestamp,
            true,
            true,
        )?;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;

    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
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
    require_staker_revenue_covered(
        &ctx.accounts.protocol,
        ctx.accounts.staker_revenue_nusd_vault.amount,
        staker_delta_u64 as u128,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.protocol_revenue_nusd_vault.amount,
        ctx.accounts.protocol.realized_revenue_for_protocol,
    )?;
    Ok(())
}

pub fn cover_bad_debt(ctx: Context<CoverBadDebt>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    let amount_u128 = amount as u128;
    let recorded_idle = ctx.accounts.protocol.psm_idle_usdc;
    let actual_idle = ctx.accounts.psm_usdc_vault.amount as u128;
    require!(
        actual_idle
            >= recorded_idle
                .checked_add(amount_u128)
                .ok_or(error!(CoreError::MathOverflow))?,
        CoreError::PsmNoSurplus
    );
    let new_liabilities = ctx
        .accounts
        .protocol
        .psm_usdc_liabilities
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(
        new_liabilities <= ctx.accounts.protocol.psm_cap,
        CoreError::PsmCapExceeded
    );

    let mut accounting = domain_protocol(&ctx.accounts.protocol);
    accounting
        .cover_bad_debt(amount_u128)
        .map_err(map_core_error)?;
    ctx.accounts.protocol.bad_debt_nusd = accounting.bad_debt_nusd;
    ctx.accounts.protocol.psm_usdc_liabilities = new_liabilities;
    ctx.accounts.protocol.psm_nusd_supply = ctx
        .accounts
        .protocol
        .psm_nusd_supply
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.protocol.psm_idle_usdc = recorded_idle
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    require_psm_accounting_invariants(&ctx.accounts.protocol, ctx.accounts.psm_usdc_vault.amount)?;
    Ok(())
}
