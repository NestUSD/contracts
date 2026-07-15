use crate::{
    checked_add, checked_sub, collateral_raw_for_usd_value,
    collateral_value_usd as raw_collateral_value_usd, mul_div_down, mul_div_up, NestError, Result,
    BPS_DENOMINATOR, DEFAULT_CLOSE_FACTOR_BPS, FULL_LIQUIDATION_HEALTH_FACTOR_BPS,
    INSURANCE_FEE_SHARE_BPS, INSURANCE_TARGET_BPS, SECONDS_PER_YEAR,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollateralParams {
    pub borrow_ltv_bps: u16,
    pub liquidation_threshold_bps: u16,
    pub liquidation_penalty_bps: u16,
    pub close_factor_bps: u16,
    pub full_liquidation_threshold_bps: u16,
    pub per_vault_debt_cap: u128,
    pub protocol_debt_cap: u128,
    pub collateral_debt_cap: u128,
    pub collateral_debt_outstanding: u128,
    pub deposit_cap_raw: u128,
    pub deposits_paused: bool,
    pub borrows_paused: bool,
    pub withdraws_paused: bool,
}

impl CollateralParams {
    pub fn validate(&self) -> Result<()> {
        if self.borrow_ltv_bps == 0
            || self.borrow_ltv_bps >= self.liquidation_threshold_bps
            || self.liquidation_threshold_bps >= BPS_DENOMINATOR as u16
            || self.close_factor_bps == 0
            || self.close_factor_bps > BPS_DENOMINATOR as u16
            || self.liquidation_penalty_bps == 0
            || self.liquidation_penalty_bps > BPS_DENOMINATOR as u16
            || self.full_liquidation_threshold_bps > BPS_DENOMINATOR as u16
        {
            return Err(NestError::InvalidParameter);
        }
        Ok(())
    }
}

impl Default for CollateralParams {
    fn default() -> Self {
        Self {
            borrow_ltv_bps: 4_500,
            liquidation_threshold_bps: 5_500,
            liquidation_penalty_bps: 800,
            close_factor_bps: DEFAULT_CLOSE_FACTOR_BPS,
            full_liquidation_threshold_bps: FULL_LIQUIDATION_HEALTH_FACTOR_BPS,
            per_vault_debt_cap: 50_000_000_000,
            protocol_debt_cap: 500_000_000_000,
            collateral_debt_cap: 100_000_000_000,
            collateral_debt_outstanding: 0,
            deposit_cap_raw: u128::MAX,
            deposits_paused: false,
            borrows_paused: false,
            withdraws_paused: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProtocolAccounting {
    pub total_debt: u128,
    pub pending_liquidation_principal: u128,
    pub pending_liquidation_fees: u128,
    pub total_uncollected_fees: u128,
    pub realized_revenue_for_stakers: u128,
    pub insurance_fund_nusd: u128,
    pub bad_debt_nusd: u128,
    pub insurance_target_bps: u16,
    pub insurance_fee_share_bps: u16,
}

impl ProtocolAccounting {
    pub fn new() -> Self {
        Self {
            insurance_target_bps: INSURANCE_TARGET_BPS,
            insurance_fee_share_bps: INSURANCE_FEE_SHARE_BPS,
            ..Self::default()
        }
    }

    pub fn cdp_insurance_target(&self) -> Result<u128> {
        mul_div_down(
            self.total_debt,
            self.insurance_target_bps as u128,
            BPS_DENOMINATOR,
        )
    }

    pub fn route_realized_fee(&mut self, amount: u128) -> Result<()> {
        let target = self.cdp_insurance_target()?;
        let insurance_share = if self.insurance_fund_nusd >= target {
            0
        } else {
            let desired = mul_div_down(
                amount,
                self.insurance_fee_share_bps as u128,
                BPS_DENOMINATOR,
            )?;
            core::cmp::min(desired, target - self.insurance_fund_nusd)
        };
        let staker_share = checked_sub(amount, insurance_share)?;
        self.insurance_fund_nusd = checked_add(self.insurance_fund_nusd, insurance_share)?;
        self.realized_revenue_for_stakers =
            checked_add(self.realized_revenue_for_stakers, staker_share)?;
        Ok(())
    }

    pub fn cover_bad_debt(&mut self, amount: u128) -> Result<()> {
        if amount == 0 || amount > self.bad_debt_nusd {
            return Err(NestError::InvalidParameter);
        }
        self.bad_debt_nusd = checked_sub(self.bad_debt_nusd, amount)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vault {
    pub collateral_raw: u128,
    pub principal_debt: u128,
    pub accrued_fee: u128,
    pub last_accrual_ts: i64,
}

impl Vault {
    pub fn total_debt(&self) -> Result<u128> {
        checked_add(self.principal_debt, self.accrued_fee)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepayOutcome {
    pub fee_paid: u128,
    pub principal_paid: u128,
    pub bad_debt_repaid: u128,
    pub overpayment: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiquidationOutcome {
    pub repaid_debt: u128,
    pub fee_paid: u128,
    pub principal_paid: u128,
    pub liquidation_penalty_nusd: u128,
    pub keeper_collateral_raw: u128,
    pub insurance_collateral_raw: u128,
    pub insurance_nusd_burned: u128,
    pub fee_cancelled: u128,
    pub bad_debt_repaid: u128,
    pub bad_debt: u128,
    pub full_liquidation: bool,
}

pub fn accrue_stability_fee(
    vault: &mut Vault,
    protocol: &mut ProtocolAccounting,
    stability_fee_apr_bps: u64,
    now: i64,
) -> Result<u128> {
    if now <= vault.last_accrual_ts {
        return Ok(0);
    }
    let elapsed = now
        .checked_sub(vault.last_accrual_ts)
        .ok_or(NestError::MathOverflow)? as u128;
    let debt = vault.total_debt()?;
    if debt == 0 {
        vault.last_accrual_ts = now;
        return Ok(0);
    }
    if stability_fee_apr_bps == 0 {
        vault.last_accrual_ts = now;
        return Ok(0);
    }
    let numerator = debt
        .checked_mul(stability_fee_apr_bps as u128)
        .and_then(|v| v.checked_mul(elapsed))
        .ok_or(NestError::MathOverflow)?;
    let denominator = BPS_DENOMINATOR
        .checked_mul(SECONDS_PER_YEAR)
        .ok_or(NestError::MathOverflow)?;
    let mut fee = numerator
        .checked_div(denominator)
        .ok_or(NestError::DivisionByZero)?;
    if fee == 0 && numerator > 0 {
        fee = 1;
    }
    vault.last_accrual_ts = now;
    if fee == 0 {
        return Ok(0);
    }
    vault.accrued_fee = checked_add(vault.accrued_fee, fee)?;
    protocol.total_debt = checked_add(protocol.total_debt, fee)?;
    protocol.total_uncollected_fees = checked_add(protocol.total_uncollected_fees, fee)?;
    Ok(fee)
}

pub fn health_factor_bps(
    collateral_value_usd: u128,
    debt: u128,
    liquidation_threshold_bps: u16,
) -> Result<u128> {
    if debt == 0 {
        return Ok(u128::MAX);
    }
    mul_div_down(
        collateral_value_usd,
        liquidation_threshold_bps as u128,
        debt,
    )
}

fn health_factor_at_least_bps(
    collateral_value_usd: u128,
    debt: u128,
    liquidation_threshold_bps: u16,
    required_health_factor_bps: u16,
) -> Result<bool> {
    if debt == 0 {
        return Ok(true);
    }
    let collateral_side = collateral_value_usd
        .checked_mul(liquidation_threshold_bps as u128)
        .ok_or(NestError::MathOverflow)?;
    let debt_side = debt
        .checked_mul(required_health_factor_bps as u128)
        .ok_or(NestError::MathOverflow)?;
    Ok(collateral_side >= debt_side)
}

pub fn can_borrow(
    collateral_value_usd: u128,
    vault: Vault,
    params: CollateralParams,
    protocol: ProtocolAccounting,
    borrow_amount: u128,
) -> Result<()> {
    params.validate()?;
    if borrow_amount == 0 {
        return Err(NestError::InvalidParameter);
    }
    if params.borrows_paused {
        return Err(NestError::BorrowPaused);
    }
    let debt_after = checked_add(vault.total_debt()?, borrow_amount)?;
    let borrow_limit = mul_div_down(
        collateral_value_usd,
        params.borrow_ltv_bps as u128,
        BPS_DENOMINATOR,
    )?;
    if debt_after > borrow_limit {
        return Err(NestError::InsufficientCollateral);
    }
    if debt_after > params.per_vault_debt_cap {
        return Err(NestError::VaultDebtCapExceeded);
    }
    let pending_liquidation_debt = checked_add(
        protocol.pending_liquidation_principal,
        protocol.pending_liquidation_fees,
    )?;
    let reserved_debt = checked_add(protocol.total_debt, pending_liquidation_debt)?;
    if checked_add(reserved_debt, borrow_amount)? > params.protocol_debt_cap {
        return Err(NestError::DebtCapExceeded);
    }
    // Pending receipts are tracked globally. Reserving all of them against each
    // market is conservative, but prevents a market cap from reopening while
    // seized collateral is still awaiting settlement.
    let reserved_collateral_debt =
        checked_add(params.collateral_debt_outstanding, pending_liquidation_debt)?;
    if checked_add(reserved_collateral_debt, borrow_amount)? > params.collateral_debt_cap {
        return Err(NestError::DebtCapExceeded);
    }
    Ok(())
}

pub fn borrow(
    vault: &mut Vault,
    protocol: &mut ProtocolAccounting,
    collateral_value_usd: u128,
    params: CollateralParams,
    borrow_amount: u128,
) -> Result<()> {
    can_borrow(
        collateral_value_usd,
        *vault,
        params,
        *protocol,
        borrow_amount,
    )?;
    vault.principal_debt = checked_add(vault.principal_debt, borrow_amount)?;
    protocol.total_debt = checked_add(protocol.total_debt, borrow_amount)?;
    Ok(())
}

pub fn can_withdraw(
    collateral_value_usd: u128,
    vault: Vault,
    params: CollateralParams,
) -> Result<()> {
    params.validate()?;
    if params.withdraws_paused {
        return Err(NestError::WithdrawPaused);
    }
    let debt = vault.total_debt()?;
    if debt == 0 {
        return Ok(());
    }
    let borrow_limit = mul_div_down(
        collateral_value_usd,
        params.borrow_ltv_bps as u128,
        BPS_DENOMINATOR,
    )?;
    if debt > borrow_limit {
        return Err(NestError::InsufficientCollateral);
    }
    Ok(())
}

pub fn repay(
    vault: &mut Vault,
    protocol: &mut ProtocolAccounting,
    amount: u128,
) -> Result<RepayOutcome> {
    let mut outcome = collect_repayment(vault, protocol, amount)?;
    outcome.bad_debt_repaid = retire_bad_debt(protocol, outcome.fee_paid)?;
    protocol.route_realized_fee(checked_sub(outcome.fee_paid, outcome.bad_debt_repaid)?)?;
    Ok(outcome)
}

fn collect_repayment(
    vault: &mut Vault,
    protocol: &mut ProtocolAccounting,
    amount: u128,
) -> Result<RepayOutcome> {
    if amount == 0 {
        return Err(NestError::InvalidParameter);
    }
    let fee_paid = core::cmp::min(amount, vault.accrued_fee);
    vault.accrued_fee = checked_sub(vault.accrued_fee, fee_paid)?;
    protocol.total_uncollected_fees = checked_sub(protocol.total_uncollected_fees, fee_paid)?;
    protocol.total_debt = checked_sub(protocol.total_debt, fee_paid)?;

    let remaining = checked_sub(amount, fee_paid)?;
    let principal_paid = core::cmp::min(remaining, vault.principal_debt);
    vault.principal_debt = checked_sub(vault.principal_debt, principal_paid)?;
    protocol.total_debt = checked_sub(protocol.total_debt, principal_paid)?;
    let overpayment = checked_sub(remaining, principal_paid)?;

    Ok(RepayOutcome {
        fee_paid,
        principal_paid,
        bad_debt_repaid: 0,
        overpayment,
    })
}

fn retire_bad_debt(protocol: &mut ProtocolAccounting, amount: u128) -> Result<u128> {
    let repaid = core::cmp::min(amount, protocol.bad_debt_nusd);
    protocol.bad_debt_nusd = checked_sub(protocol.bad_debt_nusd, repaid)?;
    Ok(repaid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct DebtWriteOff {
    fee_cancelled: u128,
    principal_cancelled: u128,
}

fn write_off_debt(
    vault: &mut Vault,
    protocol: &mut ProtocolAccounting,
    amount: u128,
) -> Result<DebtWriteOff> {
    if amount == 0 {
        return Ok(DebtWriteOff::default());
    }
    let debt = vault.total_debt()?;
    let writeoff = core::cmp::min(amount, debt);
    let fee_cancelled = core::cmp::min(writeoff, vault.accrued_fee);
    vault.accrued_fee = checked_sub(vault.accrued_fee, fee_cancelled)?;
    protocol.total_uncollected_fees = checked_sub(protocol.total_uncollected_fees, fee_cancelled)?;
    protocol.total_debt = checked_sub(protocol.total_debt, fee_cancelled)?;

    let remaining = checked_sub(writeoff, fee_cancelled)?;
    let principal_cancelled = core::cmp::min(remaining, vault.principal_debt);
    vault.principal_debt = checked_sub(vault.principal_debt, principal_cancelled)?;
    protocol.total_debt = checked_sub(protocol.total_debt, principal_cancelled)?;
    Ok(DebtWriteOff {
        fee_cancelled,
        principal_cancelled,
    })
}

pub fn max_liquidation_repay(
    vault: Vault,
    collateral_value_usd: u128,
    params: CollateralParams,
) -> Result<(u128, bool)> {
    let debt = vault.total_debt()?;
    if health_factor_at_least_bps(
        collateral_value_usd,
        debt,
        params.liquidation_threshold_bps,
        BPS_DENOMINATOR as u16,
    )? {
        return Err(NestError::VaultHealthy);
    }
    let close_factor_repay = mul_div_up(debt, params.close_factor_bps as u128, BPS_DENOMINATOR)?;
    let hard_full = !health_factor_at_least_bps(
        collateral_value_usd,
        debt,
        params.liquidation_threshold_bps,
        params.full_liquidation_threshold_bps,
    )?;
    let full = if hard_full || close_factor_repay >= debt {
        true
    } else {
        partial_liquidation_would_remain_unhealthy(
            collateral_value_usd,
            debt,
            close_factor_repay,
            params,
        )?
    };
    let max_repay = if full { debt } else { close_factor_repay };
    Ok((max_repay, full))
}

fn partial_liquidation_would_remain_unhealthy(
    collateral_value_usd: u128,
    debt: u128,
    partial_repay: u128,
    params: CollateralParams,
) -> Result<bool> {
    let remaining_debt = checked_sub(debt, partial_repay)?;
    if remaining_debt == 0 {
        return Ok(false);
    }
    let penalty_value = mul_div_up(
        partial_repay,
        params.liquidation_penalty_bps as u128,
        BPS_DENOMINATOR,
    )?;
    let seized_value = checked_add(partial_repay, penalty_value)?;
    if seized_value >= collateral_value_usd {
        return Ok(true);
    }
    let remaining_collateral_value = checked_sub(collateral_value_usd, seized_value)?;
    Ok(!health_factor_at_least_bps(
        remaining_collateral_value,
        remaining_debt,
        params.liquidation_threshold_bps,
        BPS_DENOMINATOR as u16,
    )?)
}

pub fn liquidate(
    vault: &mut Vault,
    protocol: &mut ProtocolAccounting,
    collateral_value_usd: u128,
    raw_token_safe_price_e8: u128,
    token_decimals: u8,
    params: CollateralParams,
    requested_repay: u128,
) -> Result<LiquidationOutcome> {
    if requested_repay == 0 {
        return Err(NestError::InvalidParameter);
    }
    if raw_token_safe_price_e8 == 0 {
        return Err(NestError::PriceNotPositive);
    }
    let (max_repay, full) = max_liquidation_repay(*vault, collateral_value_usd, params)?;
    let repay_amount = core::cmp::min(requested_repay, max_repay);
    let debt_before = vault.total_debt()?;
    let repay_amount = core::cmp::min(repay_amount, debt_before);

    let penalty_value = mul_div_up(
        repay_amount,
        params.liquidation_penalty_bps as u128,
        BPS_DENOMINATOR,
    )?;
    let keeper_value = checked_add(repay_amount, penalty_value)?;

    let keeper_raw =
        collateral_raw_for_usd_value(keeper_value, token_decimals, raw_token_safe_price_e8)?;
    let (keeper_raw, exhausts_collateral) = if vault.collateral_raw >= keeper_raw {
        let exhausts_collateral = full && vault.collateral_raw == keeper_raw;
        (keeper_raw, exhausts_collateral)
    } else {
        if !full {
            return Err(NestError::InsufficientCollateral);
        }
        (vault.collateral_raw, true)
    };
    if full && !exhausts_collateral && repay_amount < debt_before {
        return Err(NestError::InvalidParameter);
    }
    let delivered_value =
        raw_collateral_value_usd(keeper_raw, token_decimals, raw_token_safe_price_e8)?;
    let delivered_penalty_value = if delivered_value > repay_amount {
        checked_sub(delivered_value, repay_amount)?
    } else {
        0
    };
    let liquidation_penalty_nusd = core::cmp::min(penalty_value, delivered_penalty_value);

    let repay_outcome = collect_repayment(vault, protocol, repay_amount)?;
    vault.collateral_raw = checked_sub(vault.collateral_raw, keeper_raw)?;

    let mut insurance_nusd_burned = 0;
    let mut fee_cancelled = 0;
    let mut bad_debt = 0;
    if full && exhausts_collateral && vault.collateral_raw == 0 {
        let fee_writeoff = write_off_debt(vault, protocol, vault.accrued_fee)?;
        fee_cancelled = fee_writeoff.fee_cancelled;

        insurance_nusd_burned = core::cmp::min(protocol.insurance_fund_nusd, vault.principal_debt);
        protocol.insurance_fund_nusd =
            checked_sub(protocol.insurance_fund_nusd, insurance_nusd_burned)?;
        let insured_writeoff = write_off_debt(vault, protocol, insurance_nusd_burned)?;
        if insured_writeoff.fee_cancelled != 0
            || insured_writeoff.principal_cancelled != insurance_nusd_burned
        {
            return Err(NestError::MathOverflow);
        }

        let uncovered = vault.principal_debt;
        if uncovered > 0 {
            let uncovered_writeoff = write_off_debt(vault, protocol, uncovered)?;
            if uncovered_writeoff.fee_cancelled != 0 {
                return Err(NestError::MathOverflow);
            }
            bad_debt = uncovered_writeoff.principal_cancelled;
            protocol.bad_debt_nusd = checked_add(protocol.bad_debt_nusd, bad_debt)?;
        }
    }

    let fee_bad_debt_repaid = retire_bad_debt(protocol, repay_outcome.fee_paid)?;
    protocol.route_realized_fee(checked_sub(repay_outcome.fee_paid, fee_bad_debt_repaid)?)?;
    let penalty_bad_debt_repaid = retire_bad_debt(protocol, liquidation_penalty_nusd)?;
    // A realized liquidation penalty follows the same insurance and staker
    // routing as other protocol revenue after outstanding bad debt is retired.
    protocol.route_realized_fee(checked_sub(
        liquidation_penalty_nusd,
        penalty_bad_debt_repaid,
    )?)?;
    let bad_debt_repaid = checked_add(fee_bad_debt_repaid, penalty_bad_debt_repaid)?;

    Ok(LiquidationOutcome {
        repaid_debt: repay_amount,
        fee_paid: repay_outcome.fee_paid,
        principal_paid: repay_outcome.principal_paid,
        liquidation_penalty_nusd,
        keeper_collateral_raw: keeper_raw,
        insurance_collateral_raw: 0,
        insurance_nusd_burned,
        fee_cancelled,
        bad_debt_repaid,
        bad_debt,
        full_liquidation: full,
    })
}
