use crate::*;

pub fn liquidate_with_oracle(
    ctx: Context<LiquidateWithOracle>,
    requested_repay: u64,
) -> Result<()> {
    require!(requested_repay > 0, CoreError::InvalidParameter);
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
    let staking_assets = staking_vault_nusd_from_account(
        &ctx.accounts.staking_state.to_account_info(),
        ctx.accounts.protocol.nusd_mint,
    )?;
    let collateral_value = collateral_value_for_raw_from_snapshot(
        &ctx.accounts.collateral_config,
        &ctx.accounts.oracle,
        ctx.accounts.vault.collateral_raw,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.collateral_vault.amount,
        ctx.accounts.collateral_config.total_deposits_raw,
    )?;
    let raw_price =
        raw_token_safe_price_from_snapshot(&ctx.accounts.collateral_config, &ctx.accounts.oracle)?;
    let debt_before = ctx.accounts.vault.total_debt()?;
    let insurance_before = ctx.accounts.protocol.insurance_fund_nusd;
    let staker_before = ctx.accounts.protocol.realized_revenue_for_stakers;
    let mut vault = domain_vault(&ctx.accounts.vault);
    let mut protocol = domain_protocol(&ctx.accounts.protocol);
    let params = domain_params(&ctx.accounts.collateral_config, &ctx.accounts.protocol);
    let out = domain::liquidate(
        &mut vault,
        &mut protocol,
        collateral_value,
        raw_price,
        ctx.accounts.collateral_config.collateral_decimals,
        params,
        requested_repay as u128,
    )
    .map_err(map_core_error)?;
    let insurance_after_before_burn = protocol
        .insurance_fund_nusd
        .checked_add(out.insurance_nusd_burned)
        .ok_or(error!(CoreError::MathOverflow))?;
    let insurance_delta = insurance_after_before_burn
        .checked_sub(insurance_before)
        .ok_or(error!(CoreError::MathOverflow))?;
    let staker_delta = protocol
        .realized_revenue_for_stakers
        .checked_sub(staker_before)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.realized_revenue_for_stakers = staker_before;
    let liquidator_paid_u64 = u128_to_u64(
        out.fee_paid
            .checked_add(out.principal_paid)
            .and_then(|amount| amount.checked_add(out.staker_penalty_nusd))
            .ok_or(error!(CoreError::MathOverflow))?,
    )?;
    let nusd_burned_u64 = u128_to_u64(
        out.principal_paid
            .checked_add(out.insurance_nusd_burned)
            .ok_or(error!(CoreError::MathOverflow))?,
    )?;
    let liquidator_nusd_before = ctx.accounts.liquidator_nusd_account.amount;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;

    ctx.accounts.vault.collateral_raw = vault.collateral_raw;
    ctx.accounts.vault.principal_debt = vault.principal_debt;
    ctx.accounts.vault.accrued_fee = vault.accrued_fee;
    apply_protocol(&mut ctx.accounts.protocol, protocol);
    let repaid_debt_reduction = out
        .fee_paid
        .checked_add(out.principal_paid)
        .ok_or(error!(CoreError::MathOverflow))?;
    let debt_reduction = out
        .insurance_nusd_burned
        .checked_add(out.bad_debt)
        .and_then(|amount| amount.checked_add(repaid_debt_reduction))
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.collateral_config.total_debt = ctx
        .accounts
        .collateral_config
        .total_debt
        .checked_sub(core::cmp::min(debt_before, debt_reduction))
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.collateral_config.total_deposits_raw = ctx
        .accounts
        .collateral_config
        .total_deposits_raw
        .checked_sub(
            out.keeper_collateral_raw
                .checked_add(out.insurance_collateral_raw)
                .ok_or(error!(CoreError::MathOverflow))?,
        )
        .ok_or(error!(CoreError::MathOverflow))?;
    let (staker_delta, protocol_delta) = route_targeted_staker_revenue(
        &mut ctx.accounts.protocol,
        staking_assets,
        now,
        staker_delta,
    )?;
    let routed_charge = insurance_delta
        .checked_add(staker_delta)
        .and_then(|amount| amount.checked_add(protocol_delta))
        .ok_or(error!(CoreError::MathOverflow))?;
    let expected_routed_charge = out
        .fee_paid
        .checked_add(out.staker_penalty_nusd)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(
        routed_charge == expected_routed_charge,
        CoreError::MathOverflow
    );
    let insurance_nusd_before = ctx.accounts.insurance_nusd_vault.amount;
    let insurance_delta_u64 = u128_to_u64(insurance_delta)?;
    if insurance_delta_u64 > 0 {
        let insurance_fee_vault_before = ctx.accounts.insurance_nusd_vault.amount;
        token_transfer_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.liquidator_nusd_account.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.insurance_nusd_vault.to_account_info(),
            ctx.accounts.liquidator.to_account_info(),
            insurance_delta_u64,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
        ctx.accounts.insurance_nusd_vault.reload()?;
        require_token_account_increase(
            insurance_fee_vault_before,
            ctx.accounts.insurance_nusd_vault.amount,
            insurance_delta_u64,
        )?;
    }
    let staker_delta_u64 = u128_to_u64(staker_delta)?;
    if staker_delta_u64 > 0 {
        let staker_revenue_before = ctx.accounts.staker_revenue_nusd_vault.amount;
        token_transfer_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.liquidator_nusd_account.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.staker_revenue_nusd_vault.to_account_info(),
            ctx.accounts.liquidator.to_account_info(),
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
            ctx.accounts.liquidator_nusd_account.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.protocol_revenue_nusd_vault.to_account_info(),
            ctx.accounts.liquidator.to_account_info(),
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
    let principal_paid_u64 = u128_to_u64(out.principal_paid)?;
    if principal_paid_u64 > 0 {
        token_burn_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.liquidator_nusd_account.to_account_info(),
            ctx.accounts.liquidator.to_account_info(),
            principal_paid_u64,
            ctx.accounts.nusd_mint.decimals,
            &[],
        )?;
    }
    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    let insurance_cover_u64 = u128_to_u64(out.insurance_nusd_burned)?;
    if insurance_cover_u64 > 0 {
        token_burn_checked(
            ctx.accounts.nusd_token_program.to_account_info(),
            ctx.accounts.nusd_mint.to_account_info(),
            ctx.accounts.insurance_nusd_vault.to_account_info(),
            ctx.accounts.protocol.to_account_info(),
            insurance_cover_u64,
            ctx.accounts.nusd_mint.decimals,
            signer_seeds,
        )?;
    }
    let keeper_collateral = u128_to_u64(out.keeper_collateral_raw)?;
    let collateral_vault_before = ctx.accounts.collateral_vault.amount;
    if keeper_collateral > 0 {
        let keeper_collateral_before = ctx.accounts.liquidator_collateral_account.amount;
        token_transfer_checked(
            ctx.accounts.collateral_token_program.to_account_info(),
            ctx.accounts.collateral_vault.to_account_info(),
            ctx.accounts.collateral_mint.to_account_info(),
            ctx.accounts.liquidator_collateral_account.to_account_info(),
            ctx.accounts.protocol.to_account_info(),
            keeper_collateral,
            ctx.accounts.collateral_mint.decimals,
            signer_seeds,
        )?;
        ctx.accounts.liquidator_collateral_account.reload()?;
        require_token_account_increase(
            keeper_collateral_before,
            ctx.accounts.liquidator_collateral_account.amount,
            keeper_collateral,
        )?;
    }
    let insurance_collateral = u128_to_u64(out.insurance_collateral_raw)?;
    if insurance_collateral > 0 {
        let insurance_collateral_before = ctx.accounts.insurance_collateral_account.amount;
        token_transfer_checked(
            ctx.accounts.collateral_token_program.to_account_info(),
            ctx.accounts.collateral_vault.to_account_info(),
            ctx.accounts.collateral_mint.to_account_info(),
            ctx.accounts.insurance_collateral_account.to_account_info(),
            ctx.accounts.protocol.to_account_info(),
            insurance_collateral,
            ctx.accounts.collateral_mint.decimals,
            signer_seeds,
        )?;
        ctx.accounts.insurance_collateral_account.reload()?;
        require_token_account_increase(
            insurance_collateral_before,
            ctx.accounts.insurance_collateral_account.amount,
            insurance_collateral,
        )?;
    }
    let total_collateral_seized = keeper_collateral
        .checked_add(insurance_collateral)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.collateral_vault.reload()?;
    ctx.accounts.liquidator_nusd_account.reload()?;
    ctx.accounts.nusd_mint.reload()?;
    ctx.accounts.insurance_nusd_vault.reload()?;
    require_token_account_decrease(
        collateral_vault_before,
        ctx.accounts.collateral_vault.amount,
        total_collateral_seized,
    )?;
    let expected_insurance_nusd_after = (insurance_nusd_before as u128)
        .checked_add(insurance_delta)
        .ok_or(error!(CoreError::MathOverflow))?
        .checked_sub(out.insurance_nusd_burned)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(
        ctx.accounts.insurance_nusd_vault.amount as u128 == expected_insurance_nusd_after,
        CoreError::TransferFeeNotSupported
    );
    require_token_account_decrease(
        liquidator_nusd_before,
        ctx.accounts.liquidator_nusd_account.amount,
        liquidator_paid_u64,
    )?;
    require_mint_supply_decrease(
        nusd_supply_before,
        ctx.accounts.nusd_mint.supply,
        nusd_burned_u64,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.insurance_nusd_vault.amount,
        ctx.accounts.protocol.insurance_fund_nusd,
    )?;
    require_new_staker_revenue_covered(
        ctx.accounts.staker_revenue_nusd_vault.amount,
        staker_delta,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.protocol_revenue_nusd_vault.amount,
        ctx.accounts.protocol.realized_revenue_for_protocol,
    )?;
    Ok(())
}
