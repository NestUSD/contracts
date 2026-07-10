fn domain_vault(vault: &Vault) -> domain::Vault {
    domain::Vault {
        collateral_raw: vault.collateral_raw,
        principal_debt: vault.principal_debt,
        accrued_fee: vault.accrued_fee,
        last_accrual_ts: vault.last_accrual_ts,
    }
}

fn domain_protocol(protocol: &Protocol) -> domain::ProtocolAccounting {
    domain::ProtocolAccounting {
        total_debt: protocol.total_debt,
        pending_liquidation_principal: protocol.pending_liquidation_principal,
        pending_liquidation_fees: protocol.pending_liquidation_fees,
        total_uncollected_fees: protocol.total_uncollected_fees,
        realized_revenue_for_stakers: protocol.realized_revenue_for_stakers,
        insurance_fund_nusd: protocol.insurance_fund_nusd,
        bad_debt_nusd: protocol.bad_debt_nusd,
        insurance_target_bps: protocol.insurance_target_bps,
        insurance_fee_share_bps: protocol.insurance_fee_share_bps,
    }
}

fn apply_protocol(protocol: &mut Protocol, accounting: domain::ProtocolAccounting) {
    protocol.total_debt = accounting.total_debt;
    protocol.total_uncollected_fees = accounting.total_uncollected_fees;
    protocol.realized_revenue_for_stakers = accounting.realized_revenue_for_stakers;
    protocol.insurance_fund_nusd = accounting.insurance_fund_nusd;
    protocol.bad_debt_nusd = accounting.bad_debt_nusd;
}

fn domain_params(config: &CollateralConfig, protocol: &Protocol) -> domain::CollateralParams {
    domain::CollateralParams {
        borrow_ltv_bps: config.borrow_ltv_bps,
        liquidation_threshold_bps: config.liquidation_threshold_bps,
        liquidation_penalty_bps: config.liquidation_penalty_bps,
        close_factor_bps: config.close_factor_bps,
        full_liquidation_threshold_bps: domain::FULL_LIQUIDATION_HEALTH_FACTOR_BPS,
        per_vault_debt_cap: config.per_vault_debt_cap,
        protocol_debt_cap: protocol.protocol_debt_cap,
        collateral_debt_cap: config.protocol_debt_cap,
        collateral_debt_outstanding: config.total_debt,
        deposit_cap_raw: config.deposit_cap_raw,
        deposits_paused: config.deposits_paused,
        borrows_paused: config.borrows_paused,
        withdraws_paused: config.withdraws_paused,
    }
}

fn accrue_vault_fee(
    vault_account: &mut Vault,
    protocol_account: &mut Protocol,
    config_account: &mut CollateralConfig,
    now: i64,
) -> Result<()> {
    let mut vault = domain_vault(vault_account);
    let mut protocol = domain_protocol(protocol_account);
    let fee = domain::accrue_stability_fee(
        &mut vault,
        &mut protocol,
        protocol_account.stability_fee_apr_bps,
        now,
    )
    .map_err(map_core_error)?;
    vault_account.accrued_fee = vault.accrued_fee;
    vault_account.last_accrual_ts = vault.last_accrual_ts;
    apply_protocol(protocol_account, protocol);
    config_account.total_debt = config_account
        .total_debt
        .checked_add(fee)
        .ok_or(error!(CoreError::MathOverflow))?;
    Ok(())
}
