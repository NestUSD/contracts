use crate::*;

pub fn start_liquidation_with_oracle(ctx: Context<StartLiquidationWithOracle>) -> Result<()> {
    require_keys_eq!(
        ctx.accounts.protocol.liquidation_authority,
        ctx.accounts.liquidator.key(),
        CoreError::Unauthorized
    );
    let now = Clock::get()?.unix_timestamp;
    accrue_vault_fee(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.protocol,
        &mut ctx.accounts.collateral_config,
        now,
    )?;
    let debt_before = ctx.accounts.vault.total_debt()?;
    require!(
        debt_before > 0 && ctx.accounts.vault.collateral_raw > 0,
        CoreError::InvalidParameter
    );
    let raw_price =
        raw_token_safe_price_from_snapshot(&ctx.accounts.collateral_config, &ctx.accounts.oracle)?;
    let collateral_value = domain::collateral_value_usd(
        ctx.accounts.vault.collateral_raw,
        ctx.accounts.collateral_config.collateral_decimals,
        raw_price,
    )
    .map_err(map_core_error)?;
    require_recorded_amount_covered(
        ctx.accounts.collateral_vault.amount,
        ctx.accounts.collateral_config.total_deposits_raw,
    )?;
    let (_, full_liquidation) = domain::max_liquidation_repay(
        domain_vault(&ctx.accounts.vault),
        collateral_value,
        domain_params(&ctx.accounts.collateral_config, &ctx.accounts.protocol),
    )
    .map_err(map_core_error)?;
    require!(full_liquidation, CoreError::InvalidParameter);

    let principal_debt = ctx.accounts.vault.principal_debt;
    let accrued_fee = ctx.accounts.vault.accrued_fee;
    let vault_collateral_raw = ctx.accounts.vault.collateral_raw;
    let (min_settlement_usdc, target_settlement_usdc) = full_liquidation_settlement_terms(
        vault_collateral_raw,
        principal_debt,
        accrued_fee,
        ctx.accounts.collateral_config.collateral_decimals,
        raw_price,
        ctx.accounts.collateral_config.liquidation_penalty_bps,
    )?;
    ctx.accounts.liquidation_receipt.protocol = ctx.accounts.protocol.key();
    ctx.accounts.liquidation_receipt.collateral_config = ctx.accounts.collateral_config.key();
    ctx.accounts.liquidation_receipt.vault = ctx.accounts.vault.key();
    ctx.accounts.liquidation_receipt.liquidator = ctx.accounts.liquidator.key();
    ctx.accounts.liquidation_receipt.collateral_mint = ctx.accounts.collateral_mint.key();
    ctx.accounts.liquidation_receipt.collateral_raw = vault_collateral_raw;
    ctx.accounts.liquidation_receipt.min_settlement_usdc = min_settlement_usdc;
    ctx.accounts.liquidation_receipt.target_settlement_usdc = target_settlement_usdc;
    ctx.accounts.liquidation_receipt.principal_debt = principal_debt;
    ctx.accounts.liquidation_receipt.accrued_fee = accrued_fee;
    ctx.accounts.liquidation_receipt.started_at_ts = now;
    ctx.accounts.liquidation_receipt.bump = ctx.bumps.liquidation_receipt;

    ctx.accounts.protocol.total_debt = ctx
        .accounts
        .protocol
        .total_debt
        .checked_sub(debt_before)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.protocol.total_uncollected_fees = ctx
        .accounts
        .protocol
        .total_uncollected_fees
        .checked_sub(accrued_fee)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.protocol.pending_liquidation_principal = ctx
        .accounts
        .protocol
        .pending_liquidation_principal
        .checked_add(principal_debt)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.protocol.pending_liquidation_fees = ctx
        .accounts
        .protocol
        .pending_liquidation_fees
        .checked_add(accrued_fee)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.collateral_config.total_debt = ctx
        .accounts
        .collateral_config
        .total_debt
        .checked_sub(debt_before)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.collateral_config.total_deposits_raw = ctx
        .accounts
        .collateral_config
        .total_deposits_raw
        .checked_sub(vault_collateral_raw)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.vault.collateral_raw = 0;
    ctx.accounts.vault.principal_debt = 0;
    ctx.accounts.vault.accrued_fee = 0;
    ctx.accounts.vault.last_accrual_ts = now;

    let collateral_seized = u128_to_u64(vault_collateral_raw)?;
    let collateral_vault_before = ctx.accounts.collateral_vault.amount;
    let liquidator_collateral_before = ctx.accounts.liquidator_collateral_account.amount;
    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    token_transfer_checked(
        ctx.accounts.collateral_token_program.to_account_info(),
        ctx.accounts.collateral_vault.to_account_info(),
        ctx.accounts.collateral_mint.to_account_info(),
        ctx.accounts.liquidator_collateral_account.to_account_info(),
        ctx.accounts.protocol.to_account_info(),
        collateral_seized,
        ctx.accounts.collateral_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.collateral_vault.reload()?;
    ctx.accounts.liquidator_collateral_account.reload()?;
    require_token_account_decrease(
        collateral_vault_before,
        ctx.accounts.collateral_vault.amount,
        collateral_seized,
    )?;
    require_token_account_increase(
        liquidator_collateral_before,
        ctx.accounts.liquidator_collateral_account.amount,
        collateral_seized,
    )?;
    Ok(())
}

pub(crate) fn full_liquidation_settlement_terms(
    vault_collateral_raw: u128,
    principal_debt: u128,
    accrued_fee: u128,
    collateral_decimals: u8,
    raw_token_safe_price_e8: u128,
    liquidation_penalty_bps: u16,
) -> Result<(u128, u128)> {
    require!(vault_collateral_raw > 0, CoreError::InvalidParameter);
    let debt = principal_debt
        .checked_add(accrued_fee)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(debt > 0, CoreError::InvalidParameter);
    let penalty = domain::mul_div_up(
        debt,
        liquidation_penalty_bps as u128,
        domain::BPS_DENOMINATOR,
    )
    .map_err(map_core_error)?;
    let target_settlement = debt
        .checked_add(penalty)
        .ok_or(error!(CoreError::MathOverflow))?;
    let target_raw = domain::collateral_raw_for_usd_value(
        target_settlement,
        collateral_decimals,
        raw_token_safe_price_e8,
    )
    .map_err(map_core_error)?;
    require!(target_raw > 0, CoreError::InvalidParameter);

    if target_raw > vault_collateral_raw {
        let collateral_value = domain::collateral_value_usd(
            vault_collateral_raw,
            collateral_decimals,
            raw_token_safe_price_e8,
        )
        .map_err(map_core_error)?;
        return Ok((
            core::cmp::min(collateral_value, target_settlement),
            target_settlement,
        ));
    }

    Ok((target_settlement, target_settlement))
}

pub fn settle_liquidation_proceeds(
    ctx: Context<SettleLiquidationProceeds>,
    usdc_amount: u64,
) -> Result<()> {
    require!(usdc_amount > 0, CoreError::InvalidParameter);
    require_keys_eq!(
        ctx.accounts.protocol.liquidation_authority,
        ctx.accounts.liquidator.key(),
        CoreError::Unauthorized
    );
    let principal_debt = ctx.accounts.liquidation_receipt.principal_debt;
    let accrued_fee = ctx.accounts.liquidation_receipt.accrued_fee;
    let min_settlement_usdc = ctx.accounts.liquidation_receipt.min_settlement_usdc;
    let target_settlement_usdc = ctx.accounts.liquidation_receipt.target_settlement_usdc;
    let settlement_floor = core::cmp::max(principal_debt, min_settlement_usdc);
    require!(
        target_settlement_usdc >= min_settlement_usdc && target_settlement_usdc >= principal_debt,
        CoreError::InvalidParameter
    );
    // Settlement is where seized collateral becomes real backing: the
    // liquidator must return at least the conservative oracle value recorded at
    // seizure time, with principal as the floor for underwater cases.
    require!(
        usdc_amount as u128 >= settlement_floor,
        CoreError::InsufficientLiquidationProceeds
    );

    let liquidator_usdc_before = ctx.accounts.liquidator_usdc_account.amount;
    let psm_vault_before = ctx.accounts.psm_usdc_vault.amount;
    token_transfer_checked(
        ctx.accounts.usdc_token_program.to_account_info(),
        ctx.accounts.liquidator_usdc_account.to_account_info(),
        ctx.accounts.usdc_mint.to_account_info(),
        ctx.accounts.psm_usdc_vault.to_account_info(),
        ctx.accounts.liquidator.to_account_info(),
        usdc_amount,
        ctx.accounts.usdc_mint.decimals,
        &[],
    )?;
    ctx.accounts.liquidator_usdc_account.reload()?;
    ctx.accounts.psm_usdc_vault.reload()?;
    require_token_account_decrease(
        liquidator_usdc_before,
        ctx.accounts.liquidator_usdc_account.amount,
        usdc_amount,
    )?;
    require_token_account_increase(
        psm_vault_before,
        ctx.accounts.psm_usdc_vault.amount,
        usdc_amount,
    )?;

    record_liquidation_debt_settlement(&mut ctx.accounts.protocol, principal_debt, accrued_fee)?;

    let protocol_settlement = core::cmp::min(usdc_amount as u128, target_settlement_usdc);
    let mut revenue_amount = protocol_settlement
        .checked_sub(principal_debt)
        .ok_or(error!(CoreError::MathOverflow))?;
    if revenue_amount > 0 && ctx.accounts.protocol.bad_debt_nusd > 0 {
        let bad_debt_recovery = core::cmp::min(revenue_amount, ctx.accounts.protocol.bad_debt_nusd);
        record_liquidation_bad_debt_recovery(
            &mut ctx.accounts.protocol,
            ctx.accounts.psm_usdc_vault.amount,
            bad_debt_recovery,
        )?;
        revenue_amount = revenue_amount
            .checked_sub(bad_debt_recovery)
            .ok_or(error!(CoreError::MathOverflow))?;
    }
    if revenue_amount > 0 {
        let staking_assets = staking_vault_nusd_from_account(
            &ctx.accounts.staking_state.to_account_info(),
            ctx.accounts.protocol.nusd_mint,
        )?;
        let (insurance_delta_u64, staker_delta_u64, protocol_delta_u64, minted_nusd_u64) =
            apply_realized_psm_yield_accounting(
                &mut ctx.accounts.protocol,
                ctx.accounts.psm_usdc_vault.amount,
                u128_to_u64(revenue_amount)?,
                staking_assets,
                Clock::get()?.unix_timestamp,
                false,
                false,
            )?;
        require!(
            minted_nusd_u64 == u128_to_u64(revenue_amount)?,
            CoreError::MathOverflow
        );
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
        require_recorded_staker_revenue_covered(
            ctx.accounts.staker_revenue_nusd_vault.amount,
            staking_assets,
            ctx.accounts.protocol.realized_revenue_for_stakers,
        )?;
        require_recorded_amount_covered(
            ctx.accounts.protocol_revenue_nusd_vault.amount,
            ctx.accounts.protocol.realized_revenue_for_protocol,
        )?;
    }
    let borrower_surplus = (usdc_amount as u128)
        .checked_sub(protocol_settlement)
        .ok_or(error!(CoreError::MathOverflow))?;
    if borrower_surplus > 0 {
        record_liquidation_borrower_surplus(
            &mut ctx.accounts.protocol,
            ctx.accounts.psm_usdc_vault.amount,
            borrower_surplus,
        )?;
        let nusd_supply_before = ctx.accounts.nusd_mint.supply;
        let borrower_nusd_before = ctx.accounts.borrower_nusd_account.amount;
        let bump = [ctx.accounts.protocol.bump];
        let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
        token_mint_to_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.borrower_nusd_account.to_account_info(),
            ctx.accounts.protocol.to_account_info(),
            u128_to_u64(borrower_surplus)?,
            ctx.accounts.nusd_mint.decimals,
            signer_seeds,
        )?;
        ctx.accounts.borrower_nusd_account.reload()?;
        ctx.accounts.nusd_mint.reload()?;
        require_token_account_increase(
            borrower_nusd_before,
            ctx.accounts.borrower_nusd_account.amount,
            u128_to_u64(borrower_surplus)?,
        )?;
        require_mint_supply_increase(
            nusd_supply_before,
            ctx.accounts.nusd_mint.supply,
            u128_to_u64(borrower_surplus)?,
        )?;
    }
    require_psm_accounting_invariants(&ctx.accounts.protocol, ctx.accounts.psm_usdc_vault.amount)?;
    Ok(())
}

pub(crate) fn record_liquidation_borrower_surplus(
    protocol: &mut Protocol,
    actual_psm_usdc: u64,
    amount: u128,
) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    let actual_idle = actual_psm_usdc as u128;
    require!(
        actual_idle
            >= protocol
                .psm_idle_usdc
                .checked_add(amount)
                .ok_or(error!(CoreError::MathOverflow))?,
        CoreError::PsmNoSurplus
    );
    protocol.psm_usdc_liabilities = protocol
        .psm_usdc_liabilities
        .checked_add(amount)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_nusd_supply = protocol
        .psm_nusd_supply
        .checked_add(amount)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_idle_usdc = protocol
        .psm_idle_usdc
        .checked_add(amount)
        .ok_or(error!(CoreError::MathOverflow))?;
    require_psm_accounting_invariants(protocol, actual_psm_usdc)?;
    Ok(())
}

pub(crate) fn record_liquidation_bad_debt_recovery(
    protocol: &mut Protocol,
    actual_psm_usdc: u64,
    amount: u128,
) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    require!(
        amount <= protocol.bad_debt_nusd,
        CoreError::InvalidParameter
    );
    let actual_idle = actual_psm_usdc as u128;
    require!(
        actual_idle
            >= protocol
                .psm_idle_usdc
                .checked_add(amount)
                .ok_or(error!(CoreError::MathOverflow))?,
        CoreError::PsmNoSurplus
    );

    let mut accounting = domain_protocol(protocol);
    accounting.cover_bad_debt(amount).map_err(map_core_error)?;
    protocol.bad_debt_nusd = accounting.bad_debt_nusd;
    protocol.psm_usdc_liabilities = protocol
        .psm_usdc_liabilities
        .checked_add(amount)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_nusd_supply = protocol
        .psm_nusd_supply
        .checked_add(amount)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_idle_usdc = protocol
        .psm_idle_usdc
        .checked_add(amount)
        .ok_or(error!(CoreError::MathOverflow))?;
    require_psm_accounting_invariants(protocol, actual_psm_usdc)?;
    Ok(())
}

pub(crate) fn record_liquidation_debt_settlement(
    protocol: &mut Protocol,
    principal_debt: u128,
    accrued_fee: u128,
) -> Result<()> {
    protocol.pending_liquidation_principal = protocol
        .pending_liquidation_principal
        .checked_sub(principal_debt)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.pending_liquidation_fees = protocol
        .pending_liquidation_fees
        .checked_sub(accrued_fee)
        .ok_or(error!(CoreError::MathOverflow))?;

    // This converts already-issued borrower debt into USDC-backed PSM
    // liabilities after collateral has been seized, so the voluntary PSM cap
    // must not block settlement.
    protocol.psm_usdc_liabilities = protocol
        .psm_usdc_liabilities
        .checked_add(principal_debt)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_nusd_supply = protocol
        .psm_nusd_supply
        .checked_add(principal_debt)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_idle_usdc = protocol
        .psm_idle_usdc
        .checked_add(principal_debt)
        .ok_or(error!(CoreError::MathOverflow))?;
    Ok(())
}
