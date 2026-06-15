fn psm_kamino_target_deployed(idle_usdc: u128, deployed_usdc: u128) -> Result<u128> {
    idle_usdc
        .checked_add(deployed_usdc)
        .ok_or(error!(CoreError::MathOverflow))?
        .checked_mul(PSM_KAMINO_TARGET_BPS)
        .ok_or(error!(CoreError::MathOverflow))?
        .checked_div(PSM_KAMINO_BPS_DENOMINATOR)
        .ok_or(error!(CoreError::MathOverflow))
}
fn require_psm_accounting_invariants(protocol: &Protocol, actual_psm_usdc: u64) -> Result<()> {
    require!(
        protocol.psm_usdc_liabilities == protocol.psm_nusd_supply,
        CoreError::InvalidParameter
    );
    let tracked_psm_assets = protocol
        .psm_idle_usdc
        .checked_add(protocol.psm_kamino_deployed_usdc)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(
        tracked_psm_assets <= protocol.psm_usdc_liabilities,
        CoreError::InvalidParameter
    );
    require_recorded_amount_covered(actual_psm_usdc, protocol.psm_idle_usdc)
}

fn record_psm_outflow_or_pause(
    protocol: &mut Protocol,
    amount: u128,
    liquid_basis: u128,
    now: i64,
) -> Result<bool> {
    if !protocol.psm_outflow_circuit_breaker_enabled {
        return Ok(true);
    }
    require!(
        (protocol.psm_outflow_limit_usdc > 0 || protocol.psm_outflow_limit_bps > 0)
            && protocol.psm_outflow_limit_bps <= domain::BPS_DENOMINATOR as u16
            && protocol.psm_outflow_window_seconds > 0
            && protocol.psm_outflow_window_seconds <= MAX_PSM_OUTFLOW_WINDOW_SECONDS,
        CoreError::InvalidPsmOutflowCircuitBreaker
    );
    let window_expired = protocol.psm_outflow_window_start_ts <= 0
        || now < protocol.psm_outflow_window_start_ts
        || now
            >= protocol
                .psm_outflow_window_start_ts
                .checked_add(protocol.psm_outflow_window_seconds)
                .ok_or(error!(CoreError::MathOverflow))?;
    if window_expired {
        protocol.psm_outflow_window_start_ts = now;
        protocol.psm_outflow_window_usdc = 0;
        protocol.psm_outflow_window_basis_usdc = liquid_basis;
    } else if protocol.psm_outflow_window_basis_usdc == 0 {
        protocol.psm_outflow_window_basis_usdc = liquid_basis;
    }
    let bps_limit = if protocol.psm_outflow_limit_bps > 0 {
        Some(
            protocol
                .psm_outflow_window_basis_usdc
                .checked_mul(protocol.psm_outflow_limit_bps as u128)
                .ok_or(error!(CoreError::MathOverflow))?
                .checked_div(domain::BPS_DENOMINATOR)
                .ok_or(error!(CoreError::MathOverflow))?,
        )
    } else {
        None
    };
    let active_limit = match (protocol.psm_outflow_limit_usdc, bps_limit) {
        (0, Some(limit)) => limit,
        (limit, None) => limit,
        (limit, Some(bps_limit)) => core::cmp::min(limit, bps_limit),
    };
    let next_window_usdc = protocol
        .psm_outflow_window_usdc
        .checked_add(amount)
        .ok_or(error!(CoreError::MathOverflow))?;
    if next_window_usdc >= active_limit {
        protocol.paused = true;
        return Ok(false);
    }
    protocol.psm_outflow_window_usdc = next_window_usdc;
    Ok(true)
}
