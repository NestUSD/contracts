use crate::{
    checked_mul, mul_div_down, mul_div_up, pow10, NestError, Result, BPS_DENOMINATOR, PRICE_SCALE,
    USD_SCALE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketState {
    Regular,
    Extended,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OraclePrice {
    pub feed_id: [u8; 32],
    pub price_e8: u64,
    pub confidence_e8: u64,
    pub publish_time: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricePolicy {
    pub expected_feed_id: [u8; 32],
    pub max_staleness_seconds: i64,
    pub max_confidence_bps: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricingInputs {
    pub xstock_usd: OraclePrice,
    pub underlying_usd: OraclePrice,
    pub redemption_rate: OraclePrice,
    pub now: i64,
    pub market_state: MarketState,
    pub xstock_policy: PricePolicy,
    pub underlying_policy: PricePolicy,
    pub redemption_policy: PricePolicy,
    pub closed_market_haircut_bps: u16,
}

pub fn lower_confidence_bound(price: OraclePrice, policy: PricePolicy, now: i64) -> Result<u128> {
    if policy.max_staleness_seconds <= 0
        || policy.max_confidence_bps == 0
        || policy.max_confidence_bps > BPS_DENOMINATOR as u16
    {
        return Err(NestError::InvalidParameter);
    }
    if price.feed_id != policy.expected_feed_id {
        return Err(NestError::WrongFeed);
    }
    if price.price_e8 == 0 {
        return Err(NestError::PriceNotPositive);
    }
    let age = now
        .checked_sub(price.publish_time)
        .ok_or(NestError::OracleStale)?;
    if age < 0 || age > policy.max_staleness_seconds {
        return Err(NestError::OracleStale);
    }
    let price_u128 = price.price_e8 as u128;
    let conf_u128 = price.confidence_e8 as u128;
    if checked_mul(conf_u128, BPS_DENOMINATOR)?
        > checked_mul(price_u128, policy.max_confidence_bps as u128)?
    {
        return Err(NestError::ConfidenceTooWide);
    }
    price_u128
        .checked_sub(conf_u128)
        .filter(|value| *value > 0)
        .ok_or(NestError::PriceNotPositive)
}

pub fn safe_raw_token_price_e8(inputs: PricingInputs) -> Result<u128> {
    // Collateral is priced from the xStock/USD feed. The remaining feed fields
    // are inert under this policy but still validated by the shared input type.
    lower_confidence_bound(inputs.xstock_usd, inputs.xstock_policy, inputs.now)
}

pub fn collateral_value_usd(
    raw_token_amount: u128,
    token_decimals: u8,
    raw_token_price_e8: u128,
) -> Result<u128> {
    let token_scale = pow10(token_decimals)?;
    let denominator = checked_mul(token_scale, PRICE_SCALE)?;
    let numerator_scale = checked_mul(raw_token_price_e8, USD_SCALE)?;
    mul_div_down(raw_token_amount, numerator_scale, denominator)
}

pub fn collateral_raw_for_usd_value(
    usd_value: u128,
    token_decimals: u8,
    raw_token_price_e8: u128,
) -> Result<u128> {
    if raw_token_price_e8 == 0 {
        return Err(NestError::PriceNotPositive);
    }
    let token_scale = pow10(token_decimals)?;
    let numerator = checked_mul(checked_mul(usd_value, token_scale)?, PRICE_SCALE)?;
    let denominator = checked_mul(raw_token_price_e8, USD_SCALE)?;
    mul_div_up(numerator, 1, denominator)
}
