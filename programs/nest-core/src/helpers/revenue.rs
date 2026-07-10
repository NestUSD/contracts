fn apply_realized_psm_yield_accounting(
    protocol: &mut Protocol,
    actual_psm_usdc: u64,
    amount: u64,
    staking_assets: u128,
    now: i64,
    enforce_psm_cap: bool,
    enforce_protocol_open: bool,
) -> Result<(u64, u64, u64, u64)> {
    // A yield route is allowed only when the canonical PSM vault already holds
    // more USDC than recorded idle liquidity. The newly minted nUSD is backed
    // by that real surplus and split through the same insurance/staker policy as fees.
    require!(amount > 0, CoreError::InvalidParameter);
    if enforce_protocol_open {
        require!(!protocol.paused, CoreError::Paused);
    }
    require!(protocol.bad_debt_nusd == 0, CoreError::BadDebtOutstanding);

    let amount_u128 = amount as u128;
    let recorded_idle = protocol.psm_idle_usdc;
    let actual_idle = actual_psm_usdc as u128;
    require!(
        actual_idle
            >= recorded_idle
                .checked_add(amount_u128)
                .ok_or(error!(CoreError::MathOverflow))?,
        CoreError::PsmNoSurplus
    );
    let new_liabilities = protocol
        .psm_usdc_liabilities
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    if enforce_psm_cap {
        require!(
            new_liabilities <= protocol.psm_cap,
            CoreError::PsmCapExceeded
        );
    }

    let insurance_before = protocol.insurance_fund_nusd;
    let staker_before = protocol.realized_revenue_for_stakers;
    let mut accounting = domain_protocol(protocol);
    accounting
        .route_realized_fee(amount_u128)
        .map_err(map_core_error)?;
    let insurance_delta = accounting
        .insurance_fund_nusd
        .checked_sub(insurance_before)
        .ok_or(error!(CoreError::MathOverflow))?;
    let raw_staker_delta = accounting
        .realized_revenue_for_stakers
        .checked_sub(staker_before)
        .ok_or(error!(CoreError::MathOverflow))?;
    accounting.realized_revenue_for_stakers = staker_before;
    let minted_nusd = insurance_delta
        .checked_add(raw_staker_delta)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(minted_nusd == amount_u128, CoreError::MathOverflow);

    protocol.insurance_fund_nusd = accounting.insurance_fund_nusd;
    protocol.realized_revenue_for_stakers = staker_before;
    protocol.psm_usdc_liabilities = new_liabilities;
    protocol.psm_nusd_supply = protocol
        .psm_nusd_supply
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_idle_usdc = recorded_idle
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    let (staker_delta, protocol_delta) =
        route_targeted_staker_revenue(protocol, staking_assets, now, raw_staker_delta)?;
    require_psm_accounting_invariants(protocol, actual_psm_usdc)?;

    Ok((
        u128_to_u64(insurance_delta)?,
        u128_to_u64(staker_delta)?,
        u128_to_u64(protocol_delta)?,
        u128_to_u64(minted_nusd)?,
    ))
}
fn route_targeted_staker_revenue(
    protocol: &mut Protocol,
    staking_assets: u128,
    now: i64,
    raw_staker_revenue: u128,
) -> Result<(u128, u128)> {
    // Stakers receive up to the configured target APR. Any realized revenue above
    // the accrued target becomes protocol revenue instead of raising the offer.
    if protocol.staker_target_apr_bps == 0 {
        protocol.realized_revenue_for_stakers = protocol
            .realized_revenue_for_stakers
            .checked_add(raw_staker_revenue)
            .ok_or(error!(CoreError::MathOverflow))?;
        return Ok((raw_staker_revenue, 0));
    }

    sync_staker_target_revenue(protocol, staking_assets, now)?;
    let staker_delta = core::cmp::min(raw_staker_revenue, protocol.staker_target_revenue_due);
    protocol.staker_target_revenue_due = protocol
        .staker_target_revenue_due
        .checked_sub(staker_delta)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.realized_revenue_for_stakers = protocol
        .realized_revenue_for_stakers
        .checked_add(staker_delta)
        .ok_or(error!(CoreError::MathOverflow))?;
    let protocol_delta = raw_staker_revenue
        .checked_sub(staker_delta)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.realized_revenue_for_protocol = protocol
        .realized_revenue_for_protocol
        .checked_add(protocol_delta)
        .ok_or(error!(CoreError::MathOverflow))?;
    Ok((staker_delta, protocol_delta))
}

pub(crate) fn sync_staker_target_revenue(
    protocol: &mut Protocol,
    staking_assets: u128,
    now: i64,
) -> Result<()> {
    require!(now >= 0, CoreError::InvalidParameter);
    if protocol.staker_target_last_accrual_ts == 0 {
        protocol.staker_target_last_accrual_ts = now;
        return Ok(());
    }
    require!(
        protocol.staker_target_last_accrual_ts >= 0,
        CoreError::InvalidParameter
    );
    if now <= protocol.staker_target_last_accrual_ts {
        return Ok(());
    }
    let elapsed = (now - protocol.staker_target_last_accrual_ts) as u128;
    protocol.staker_target_last_accrual_ts = now;
    if staking_assets == 0 || protocol.staker_target_apr_bps == 0 {
        return Ok(());
    }
    let accrued = staking_assets
        .checked_mul(protocol.staker_target_apr_bps as u128)
        .and_then(|value| value.checked_mul(elapsed))
        .ok_or(error!(CoreError::MathOverflow))?
        .checked_div(domain::BPS_DENOMINATOR)
        .ok_or(error!(CoreError::MathOverflow))?
        .checked_div(domain::SECONDS_PER_YEAR)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.staker_target_revenue_due = protocol
        .staker_target_revenue_due
        .checked_add(accrued)
        .ok_or(error!(CoreError::MathOverflow))?;
    Ok(())
}
