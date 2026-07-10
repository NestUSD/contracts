use crate::*;

pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    require!(!ctx.accounts.protocol.paused, CoreError::Paused);
    let config = &mut ctx.accounts.collateral_config;
    require!(!config.deposits_paused, CoreError::CollateralPaused);
    require_keys_eq!(
        config.token_program,
        ctx.accounts.collateral_token_program.key(),
        CoreError::InvalidParameter
    );
    require_recorded_amount_covered(
        ctx.accounts.collateral_vault.amount,
        config.total_deposits_raw,
    )?;
    let amount_u128 = amount as u128;
    let total_deposits = config
        .total_deposits_raw
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(
        total_deposits <= config.deposit_cap_raw,
        CoreError::DepositCapExceeded
    );
    let owner_collateral_before = ctx.accounts.owner_collateral_account.amount;
    let collateral_vault_before = ctx.accounts.collateral_vault.amount;
    token_transfer_checked(
        ctx.accounts.collateral_token_program.to_account_info(),
        ctx.accounts.owner_collateral_account.to_account_info(),
        ctx.accounts.collateral_mint.to_account_info(),
        ctx.accounts.collateral_vault.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        ctx.accounts.collateral_mint.decimals,
        &[],
    )?;
    ctx.accounts.owner_collateral_account.reload()?;
    ctx.accounts.collateral_vault.reload()?;
    require_token_account_decrease(
        owner_collateral_before,
        ctx.accounts.owner_collateral_account.amount,
        amount,
    )?;
    require_token_account_increase(
        collateral_vault_before,
        ctx.accounts.collateral_vault.amount,
        amount,
    )?;
    ctx.accounts.vault.collateral_raw = ctx
        .accounts
        .vault
        .collateral_raw
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    config.total_deposits_raw = total_deposits;
    Ok(())
}

pub fn withdraw_with_oracle(ctx: Context<WithdrawWithOracle>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    require!(!ctx.accounts.protocol.paused, CoreError::Paused);
    accrue_vault_fee(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.protocol,
        &mut ctx.accounts.collateral_config,
        Clock::get()?.unix_timestamp,
    )?;
    let amount_u128 = amount as u128;
    let config = &ctx.accounts.collateral_config;
    require!(!config.withdraws_paused, CoreError::WithdrawPaused);
    require_keys_eq!(
        config.token_program,
        ctx.accounts.collateral_token_program.key(),
        CoreError::InvalidParameter
    );
    require_recorded_amount_covered(
        ctx.accounts.collateral_vault.amount,
        config.total_deposits_raw,
    )?;
    let current = ctx.accounts.vault.collateral_raw;
    require!(amount_u128 <= current, CoreError::InsufficientCollateral);

    let mut vault = domain_vault(&ctx.accounts.vault);
    vault.collateral_raw = current
        .checked_sub(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    if vault.total_debt().map_err(map_core_error)? > 0 {
        let value = collateral_value_for_raw_from_snapshot(
            config,
            &ctx.accounts.oracle,
            vault.collateral_raw,
        )?;
        domain::can_withdraw(value, vault, domain_params(config, &ctx.accounts.protocol))
            .map_err(map_core_error)?;
    }

    ctx.accounts.vault.collateral_raw = vault.collateral_raw;
    ctx.accounts.collateral_config.total_deposits_raw = ctx
        .accounts
        .collateral_config
        .total_deposits_raw
        .checked_sub(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    let collateral_vault_before = ctx.accounts.collateral_vault.amount;
    let owner_collateral_before = ctx.accounts.owner_collateral_account.amount;
    token_transfer_checked(
        ctx.accounts.collateral_token_program.to_account_info(),
        ctx.accounts.collateral_vault.to_account_info(),
        ctx.accounts.collateral_mint.to_account_info(),
        ctx.accounts.owner_collateral_account.to_account_info(),
        ctx.accounts.protocol.to_account_info(),
        amount,
        ctx.accounts.collateral_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.collateral_vault.reload()?;
    ctx.accounts.owner_collateral_account.reload()?;
    require_token_account_decrease(
        collateral_vault_before,
        ctx.accounts.collateral_vault.amount,
        amount,
    )?;
    require_token_account_increase(
        owner_collateral_before,
        ctx.accounts.owner_collateral_account.amount,
        amount,
    )?;
    Ok(())
}

pub fn accrue_fee(ctx: Context<MutateVault>) -> Result<()> {
    accrue_vault_fee(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.protocol,
        &mut ctx.accounts.collateral_config,
        Clock::get()?.unix_timestamp,
    )
}

pub fn mint_nusd_with_oracle(ctx: Context<MintNusdWithOracle>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    require!(!ctx.accounts.protocol.paused, CoreError::Paused);
    accrue_vault_fee(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.protocol,
        &mut ctx.accounts.collateral_config,
        Clock::get()?.unix_timestamp,
    )?;
    require_keys_eq!(
        ctx.accounts.collateral_config.token_program,
        ctx.accounts.collateral_token_program.key(),
        CoreError::InvalidParameter
    );
    require_recorded_amount_covered(
        ctx.accounts.collateral_vault.amount,
        ctx.accounts.collateral_config.total_deposits_raw,
    )?;
    let collateral_value = collateral_value_for_raw_from_snapshot(
        &ctx.accounts.collateral_config,
        &ctx.accounts.oracle,
        ctx.accounts.vault.collateral_raw,
    )?;
    let mut vault = domain_vault(&ctx.accounts.vault);
    let mut protocol = domain_protocol(&ctx.accounts.protocol);
    let params = domain_params(&ctx.accounts.collateral_config, &ctx.accounts.protocol);
    domain::borrow(
        &mut vault,
        &mut protocol,
        collateral_value,
        params,
        amount as u128,
    )
    .map_err(map_core_error)?;

    ctx.accounts.vault.principal_debt = vault.principal_debt;
    ctx.accounts.protocol.total_debt = protocol.total_debt;
    ctx.accounts.collateral_config.total_debt = ctx
        .accounts
        .collateral_config
        .total_debt
        .checked_add(amount as u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    let owner_nusd_before = ctx.accounts.owner_nusd_account.amount;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;
    token_mint_to_checked(
        ctx.accounts.nusd_token_program.to_account_info(),
        ctx.accounts.nusd_mint.to_account_info(),
        ctx.accounts.owner_nusd_account.to_account_info(),
        ctx.accounts.protocol.to_account_info(),
        amount,
        ctx.accounts.nusd_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.owner_nusd_account.reload()?;
    ctx.accounts.nusd_mint.reload()?;
    require_token_account_increase(
        owner_nusd_before,
        ctx.accounts.owner_nusd_account.amount,
        amount,
    )?;
    require_mint_supply_increase(nusd_supply_before, ctx.accounts.nusd_mint.supply, amount)?;
    Ok(())
}

pub fn repay_nusd(ctx: Context<RepayNusd>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    let now = Clock::get()?.unix_timestamp;
    accrue_vault_fee(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.protocol,
        &mut ctx.accounts.collateral_config,
        now,
    )?;
    let staking_assets = staking_vault_nusd_from_account(
        &ctx.accounts.staking_state.to_account_info(),
        ctx.accounts.protocol.nusd_mint,
    )?;
    let debt_before = ctx.accounts.vault.total_debt()?;
    let insurance_before = ctx.accounts.protocol.insurance_fund_nusd;
    let staker_before = ctx.accounts.protocol.realized_revenue_for_stakers;
    let mut vault = domain_vault(&ctx.accounts.vault);
    let mut protocol = domain_protocol(&ctx.accounts.protocol);
    let out = domain::repay(&mut vault, &mut protocol, amount as u128).map_err(map_core_error)?;
    let paid = out
        .fee_paid
        .checked_add(out.principal_paid)
        .ok_or(error!(CoreError::MathOverflow))?;
    let insurance_delta = protocol
        .insurance_fund_nusd
        .checked_sub(insurance_before)
        .ok_or(error!(CoreError::MathOverflow))?;
    let staker_delta = protocol
        .realized_revenue_for_stakers
        .checked_sub(staker_before)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.realized_revenue_for_stakers = staker_before;
    let paid_u64 = u128_to_u64(paid)?;
    let owner_nusd_before = ctx.accounts.owner_nusd_account.amount;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;

    ctx.accounts.vault.principal_debt = vault.principal_debt;
    ctx.accounts.vault.accrued_fee = vault.accrued_fee;
    apply_protocol(&mut ctx.accounts.protocol, protocol);
    ctx.accounts.collateral_config.total_debt = ctx
        .accounts
        .collateral_config
        .total_debt
        .checked_sub(core::cmp::min(debt_before, paid))
        .ok_or(error!(CoreError::MathOverflow))?;
    let (staker_delta, protocol_delta) = route_targeted_staker_revenue(
        &mut ctx.accounts.protocol,
        staking_assets,
        now,
        staker_delta,
    )?;
    let routed_fee = insurance_delta
        .checked_add(staker_delta)
        .and_then(|v| v.checked_add(protocol_delta))
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(
        routed_fee
            == out
                .fee_paid
                .checked_sub(out.bad_debt_repaid)
                .ok_or(error!(CoreError::MathOverflow))?,
        CoreError::MathOverflow
    );
    let insurance_delta_u64 = u128_to_u64(insurance_delta)?;
    if insurance_delta_u64 > 0 {
        let insurance_vault_before = ctx.accounts.insurance_nusd_vault.amount;
        token_transfer_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.owner_nusd_account.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.insurance_nusd_vault.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            insurance_delta_u64,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
        ctx.accounts.insurance_nusd_vault.reload()?;
        require_token_account_increase(
            insurance_vault_before,
            ctx.accounts.insurance_nusd_vault.amount,
            insurance_delta_u64,
        )?;
    }
    let staker_delta_u64 = u128_to_u64(staker_delta)?;
    if staker_delta_u64 > 0 {
        let staker_revenue_before = ctx.accounts.staker_revenue_nusd_vault.amount;
        token_transfer_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.owner_nusd_account.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.staker_revenue_nusd_vault.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            staker_delta_u64,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
        ctx.accounts.staker_revenue_nusd_vault.reload()?;
        require_token_account_increase(
            staker_revenue_before,
            ctx.accounts.staker_revenue_nusd_vault.amount,
            staker_delta_u64,
        )?;
    }
    let protocol_delta_u64 = u128_to_u64(protocol_delta)?;
    if protocol_delta_u64 > 0 {
        let protocol_revenue_before = ctx.accounts.protocol_revenue_nusd_vault.amount;
        token_transfer_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.owner_nusd_account.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.protocol_revenue_nusd_vault.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            protocol_delta_u64,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
        ctx.accounts.protocol_revenue_nusd_vault.reload()?;
        require_token_account_increase(
            protocol_revenue_before,
            ctx.accounts.protocol_revenue_nusd_vault.amount,
            protocol_delta_u64,
        )?;
    }
    let burned_nusd_u64 = u128_to_u64(
        out.principal_paid
            .checked_add(out.bad_debt_repaid)
            .ok_or(error!(CoreError::MathOverflow))?,
    )?;
    if burned_nusd_u64 > 0 {
        token_burn_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.owner_nusd_account.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            burned_nusd_u64,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
    }
    ctx.accounts.owner_nusd_account.reload()?;
    ctx.accounts.nusd_mint.reload()?;
    require_token_account_decrease(
        owner_nusd_before,
        ctx.accounts.owner_nusd_account.amount,
        paid_u64,
    )?;
    require_mint_supply_decrease(
        nusd_supply_before,
        ctx.accounts.nusd_mint.supply,
        burned_nusd_u64,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.insurance_nusd_vault.amount,
        ctx.accounts.protocol.insurance_fund_nusd,
    )?;
    require_staker_revenue_covered(
        &ctx.accounts.protocol,
        ctx.accounts.staker_revenue_nusd_vault.amount,
        staker_delta,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.protocol_revenue_nusd_vault.amount,
        ctx.accounts.protocol.realized_revenue_for_protocol,
    )?;
    Ok(())
}
