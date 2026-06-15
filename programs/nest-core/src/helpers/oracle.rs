use pyth_lazer_solana_contract::protocol::{
    payload::{PayloadData, PayloadPropertyValue},
    ChannelId, PriceFeedId,
};

const LAZER_FEED_ID_BYTES: usize = 4;

fn valid_price_feed_ids(xstock: &[u8; 32], underlying: &[u8; 32], redemption: &[u8; 32]) -> bool {
    let zero = [0; 32];
    if *xstock == zero {
        return false;
    }
    if *underlying != zero && underlying == xstock {
        return false;
    }
    if *redemption != zero && redemption == xstock {
        return false;
    }
    if *underlying != zero && *redemption != zero && underlying == redemption {
        return false;
    }
    true
}

fn validate_collateral_feed_config(
    xstock: &[u8; 32],
    underlying: &[u8; 32],
    redemption: &[u8; 32],
) -> Result<()> {
    require!(
        valid_price_feed_ids(xstock, underlying, redemption),
        CoreError::InvalidParameter
    );
    validate_lazer_feed_config(xstock)?;
    Ok(())
}

pub(crate) fn raw_token_safe_price_from_pyth(
    config: &CollateralConfig,
    xstock_price_update: &Account<PriceUpdateV2>,
) -> Result<u128> {
    require_pyth_core_feed_config(&config.xstock_usd_feed_id)?;
    let clock = Clock::get()?;
    let xstock_usd = read_pyth_price(
        xstock_price_update,
        config.xstock_usd_feed_id,
        pyth_max_age(config)?,
        &clock,
    )?;
    domain::safe_raw_token_price_e8(domain::PricingInputs {
        xstock_usd: to_domain_price(xstock_usd),
        underlying_usd: to_domain_price(inactive_oracle_price(config.underlying_usd_feed_id)),
        redemption_rate: to_domain_price(inactive_oracle_price(config.redemption_rate_feed_id)),
        now: clock.unix_timestamp,
        market_state: domain::MarketState::Regular,
        xstock_policy: policy(config.xstock_usd_feed_id, config)?,
        underlying_policy: policy(config.underlying_usd_feed_id, config)?,
        redemption_policy: policy(config.redemption_rate_feed_id, config)?,
        closed_market_haircut_bps: config.closed_market_haircut_bps,
    })
    .map_err(map_core_error)
}

pub(crate) fn raw_token_safe_price_from_snapshot(
    config: &CollateralConfig,
    oracle: &OracleSnapshot,
) -> Result<u128> {
    let clock = Clock::get()?;
    require!(
        oracle.xstock_usd.feed_id == config.xstock_usd_feed_id,
        CoreError::OracleError
    );
    domain::safe_raw_token_price_e8(domain::PricingInputs {
        xstock_usd: to_domain_price(oracle.xstock_usd),
        underlying_usd: to_domain_price(oracle.underlying_usd),
        redemption_rate: to_domain_price(oracle.redemption_rate),
        now: clock.unix_timestamp,
        market_state: to_domain_market_state(oracle.market_state),
        xstock_policy: policy(config.xstock_usd_feed_id, config)?,
        underlying_policy: policy(config.underlying_usd_feed_id, config)?,
        redemption_policy: policy(config.redemption_rate_feed_id, config)?,
        closed_market_haircut_bps: config.closed_market_haircut_bps,
    })
    .map_err(map_core_error)
}

pub(crate) fn collateral_value_for_raw_from_pyth(
    config: &CollateralConfig,
    xstock_price_update: &Account<PriceUpdateV2>,
    raw_amount: u128,
) -> Result<u128> {
    let price = raw_token_safe_price_from_pyth(config, xstock_price_update)?;
    domain::collateral_value_usd(raw_amount, config.collateral_decimals, price)
        .map_err(map_core_error)
}

pub(crate) fn collateral_value_for_raw_from_snapshot(
    config: &CollateralConfig,
    oracle: &OracleSnapshot,
    raw_amount: u128,
) -> Result<u128> {
    let price = raw_token_safe_price_from_snapshot(config, oracle)?;
    domain::collateral_value_usd(raw_amount, config.collateral_decimals, price)
        .map_err(map_core_error)
}

fn lazer_feed_id_from_config(config: &CollateralConfig) -> Result<PriceFeedId> {
    validate_lazer_feed_config(&config.xstock_usd_feed_id)
}

pub(crate) fn validate_lazer_feed_config(xstock_usd_feed_id: &[u8; 32]) -> Result<PriceFeedId> {
    require!(
        xstock_usd_feed_id[LAZER_FEED_ID_BYTES..]
            .iter()
            .all(|byte| *byte == 0),
        CoreError::InvalidParameter
    );
    let mut feed_id = [0_u8; LAZER_FEED_ID_BYTES];
    feed_id.copy_from_slice(&xstock_usd_feed_id[..LAZER_FEED_ID_BYTES]);
    let feed_id = u32::from_le_bytes(feed_id);
    require!(feed_id > 0, CoreError::InvalidParameter);
    Ok(PriceFeedId(feed_id))
}

pub(crate) fn require_pyth_core_feed_config(xstock_usd_feed_id: &[u8; 32]) -> Result<()> {
    // Lazer feed keys and Pyth receiver feed ids are intentionally disjoint, so
    // each collateral can use exactly one refresh path.
    require!(
        validate_lazer_feed_config(xstock_usd_feed_id).is_err(),
        CoreError::InvalidParameter
    );
    Ok(())
}

fn lazer_feed_key(feed_id: PriceFeedId) -> [u8; 32] {
    let mut key = [0_u8; 32];
    key[..LAZER_FEED_ID_BYTES].copy_from_slice(&feed_id.0.to_le_bytes());
    key
}

fn read_lazer_price(payload: &[u8], config: &CollateralConfig) -> Result<OraclePriceAccount> {
    let expected_feed_id = lazer_feed_id_from_config(config)?;
    let data = PayloadData::deserialize_slice_le(payload).map_err(|_| error!(CoreError::OracleError))?;
    require!(
        data.channel_id == ChannelId::REAL_TIME
            || data.channel_id == ChannelId::FIXED_RATE_50
            || data.channel_id == ChannelId::FIXED_RATE_200
            || data.channel_id == ChannelId::FIXED_RATE_1000,
        CoreError::OracleError
    );
    let feed = data
        .feeds
        .iter()
        .find(|feed| feed.feed_id == expected_feed_id)
        .ok_or(error!(CoreError::OracleError))?;
    let mut price = None;
    let mut exponent = None;
    let mut confidence = None;
    let mut feed_update_timestamp = None;
    let mut publisher_count = None;
    for property in &feed.properties {
        match property {
            PayloadPropertyValue::Price(Some(value)) => price = Some(*value),
            PayloadPropertyValue::Exponent(value) => exponent = Some(*value),
            PayloadPropertyValue::Confidence(Some(value)) => confidence = Some(*value),
            PayloadPropertyValue::FeedUpdateTimestamp(Some(value)) => {
                feed_update_timestamp = Some(*value)
            }
            PayloadPropertyValue::PublisherCount(value) => publisher_count = Some(*value),
            _ => {}
        }
    }
    require!(publisher_count.unwrap_or(0) > 0, CoreError::OracleError);
    let publish_time_us = feed_update_timestamp.ok_or(error!(CoreError::OracleError))?;
    require!(
        publish_time_us.as_micros() <= data.timestamp_us.as_micros(),
        CoreError::OracleError
    );
    let publish_time = i64::try_from(publish_time_us.as_micros() / 1_000_000)
        .map_err(|_| error!(CoreError::MathOverflow))?;
    lazer_price_to_oracle_price(
        expected_feed_id,
        price.ok_or(error!(CoreError::OracleError))?,
        confidence.ok_or(error!(CoreError::OracleError))?,
        exponent.ok_or(error!(CoreError::OracleError))?,
        publish_time,
    )
}

fn pyth_max_age(config: &CollateralConfig) -> Result<u64> {
    let max_age = xstock_max_staleness_seconds(config)?;
    u64::try_from(max_age).map_err(|_| error!(CoreError::InvalidParameter))
}

fn read_pyth_price(
    price_update: &Account<PriceUpdateV2>,
    feed_id: [u8; 32],
    max_age: u64,
    clock: &Clock,
) -> Result<OraclePriceAccount> {
    let price = price_update
        .get_price_no_older_than_with_custom_verification_level(
            clock,
            max_age,
            &feed_id,
            VerificationLevel::Partial {
                num_signatures: PYTH_MIN_GUARDIAN_SIGNATURES,
            },
        )
        .map_err(|_| error!(CoreError::OracleError))?;
    pyth_price_to_oracle_price(feed_id, price)
}

fn pyth_fixed_price_feed_account(feed_id: [u8; 32]) -> Pubkey {
    let shard = PYTH_PRICE_FEED_SHARD_ID.to_le_bytes();
    Pubkey::find_program_address(&[&shard, &feed_id], &PYTH_PUSH_ORACLE_PROGRAM_ID).0
}

fn pyth_price_to_oracle_price(feed_id: [u8; 32], price: Price) -> Result<OraclePriceAccount> {
    require!(price.price > 0, CoreError::OracleError);
    let price_e8 = scale_pyth_price_to_e8(price.price as u128, price.exponent, false)?;
    let confidence_e8 = scale_pyth_price_to_e8(price.conf as u128, price.exponent, true)?;
    Ok(OraclePriceAccount {
        feed_id,
        price_e8: u128_to_u64(price_e8)?,
        confidence_e8: u128_to_u64(confidence_e8)?,
        publish_time: price.publish_time,
    })
}

fn lazer_price_to_oracle_price(
    feed_id: PriceFeedId,
    price: pyth_lazer_solana_contract::protocol::Price,
    confidence: pyth_lazer_solana_contract::protocol::Price,
    exponent: i16,
    publish_time: i64,
) -> Result<OraclePriceAccount> {
    let price_mantissa = price.mantissa_i64();
    let confidence_mantissa = confidence.mantissa_i64();
    require!(
        price_mantissa > 0 && confidence_mantissa >= 0,
        CoreError::OracleError
    );
    let price_e8 = scale_pyth_price_to_e8(
        u128::try_from(price_mantissa).map_err(|_| error!(CoreError::OracleError))?,
        i32::from(exponent),
        false,
    )?;
    let confidence_e8 = scale_pyth_price_to_e8(
        u128::try_from(confidence_mantissa).map_err(|_| error!(CoreError::OracleError))?,
        i32::from(exponent),
        true,
    )?;
    Ok(OraclePriceAccount {
        feed_id: lazer_feed_key(feed_id),
        price_e8: u128_to_u64(price_e8)?,
        confidence_e8: u128_to_u64(confidence_e8)?,
        publish_time,
    })
}

fn inactive_oracle_price(feed_id: [u8; 32]) -> OraclePriceAccount {
    OraclePriceAccount {
        feed_id,
        price_e8: 0,
        confidence_e8: 0,
        publish_time: 0,
    }
}

fn scale_pyth_price_to_e8(value: u128, exponent: i32, round_up: bool) -> Result<u128> {
    let adjustment = exponent
        .checked_add(8)
        .ok_or(error!(CoreError::MathOverflow))?;
    if adjustment >= 0 {
        let factor = pow10_i32(adjustment)?;
        value
            .checked_mul(factor)
            .ok_or(error!(CoreError::MathOverflow))
    } else {
        let divisor = pow10_i32(-adjustment)?;
        if round_up && value > 0 {
            value
                .checked_add(
                    divisor
                        .checked_sub(1)
                        .ok_or(error!(CoreError::MathOverflow))?,
                )
                .ok_or(error!(CoreError::MathOverflow))?
                .checked_div(divisor)
                .ok_or(error!(CoreError::MathOverflow))
        } else {
            value
                .checked_div(divisor)
                .ok_or(error!(CoreError::MathOverflow))
        }
    }
}

fn pow10_i32(exponent: i32) -> Result<u128> {
    require!((0..=38).contains(&exponent), CoreError::InvalidParameter);
    let mut value = 1_u128;
    for _ in 0..exponent {
        value = value
            .checked_mul(10)
            .ok_or(error!(CoreError::MathOverflow))?;
    }
    Ok(value)
}
fn to_domain_price(price: OraclePriceAccount) -> domain::OraclePrice {
    domain::OraclePrice {
        feed_id: price.feed_id,
        price_e8: price.price_e8,
        confidence_e8: price.confidence_e8,
        publish_time: price.publish_time,
    }
}

fn to_domain_market_state(state: MarketStateAccount) -> domain::MarketState {
    match state {
        MarketStateAccount::Regular => domain::MarketState::Regular,
        MarketStateAccount::Extended => domain::MarketState::Extended,
        MarketStateAccount::Closed => domain::MarketState::Closed,
    }
}

fn policy(feed_id: [u8; 32], config: &CollateralConfig) -> Result<domain::PricePolicy> {
    Ok(domain::PricePolicy {
        expected_feed_id: feed_id,
        max_staleness_seconds: xstock_max_staleness_seconds(config)?,
        max_confidence_bps: config.max_confidence_bps,
    })
}

fn xstock_max_staleness_seconds(config: &CollateralConfig) -> Result<i64> {
    require!(
        config.max_staleness_seconds > 0
            && config.max_staleness_seconds <= MAX_XSTOCK_PRICE_STALENESS_SECONDS,
        CoreError::InvalidParameter
    );
    Ok(config.max_staleness_seconds)
}
