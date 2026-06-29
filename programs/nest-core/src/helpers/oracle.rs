use pyth_lazer_solana_contract::protocol::{
    payload::{PayloadData, PayloadPropertyValue},
    ChannelId, PriceFeedId,
};

const LAZER_FEED_ID_BYTES: usize = 4;
const SIGNED_PRICE_MAGIC: [u8; 8] = *b"NESTPRC1";
const SIGNED_PRICE_VERSION: u8 = 1;
const ED25519_SIGNATURE_OFFSETS_START: usize = 2;
const ED25519_SIGNATURE_OFFSETS_SIZE: usize = 14;
const ED25519_CURRENT_INSTRUCTION: usize = u16::MAX as usize;

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
    require!(
        is_lazer_feed_key(xstock) || is_signed_feed_key(xstock),
        CoreError::InvalidParameter
    );
    Ok(())
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
        xstock_policy: policy(config.xstock_usd_feed_id, config)?,
        underlying_policy: policy(config.underlying_usd_feed_id, config)?,
        redemption_policy: policy(config.redemption_rate_feed_id, config)?,
    })
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

fn validate_xstock_oracle_price(
    config: &CollateralConfig,
    xstock_usd: OraclePriceAccount,
    now: i64,
) -> Result<()> {
    domain::safe_raw_token_price_e8(domain::PricingInputs {
        xstock_usd: to_domain_price(xstock_usd),
        underlying_usd: to_domain_price(inactive_oracle_price(config.underlying_usd_feed_id)),
        redemption_rate: to_domain_price(inactive_oracle_price(config.redemption_rate_feed_id)),
        now,
        xstock_policy: policy(config.xstock_usd_feed_id, config)?,
        underlying_policy: policy(config.underlying_usd_feed_id, config)?,
        redemption_policy: policy(config.redemption_rate_feed_id, config)?,
    })
    .map(|_| ())
    .map_err(map_core_error)
}

fn lazer_feed_id_from_config(config: &CollateralConfig) -> Result<PriceFeedId> {
    validate_lazer_feed_config(&config.xstock_usd_feed_id)
}

fn is_lazer_feed_key(feed_id: &[u8; 32]) -> bool {
    feed_id[LAZER_FEED_ID_BYTES..]
        .iter()
        .all(|byte| *byte == 0)
}

pub(crate) fn validate_lazer_feed_config(xstock_usd_feed_id: &[u8; 32]) -> Result<PriceFeedId> {
    require!(is_lazer_feed_key(xstock_usd_feed_id), CoreError::InvalidParameter);
    let mut feed_id = [0_u8; LAZER_FEED_ID_BYTES];
    feed_id.copy_from_slice(&xstock_usd_feed_id[..LAZER_FEED_ID_BYTES]);
    let feed_id = u32::from_le_bytes(feed_id);
    require!(feed_id > 0, CoreError::InvalidParameter);
    Ok(PriceFeedId(feed_id))
}

fn is_signed_feed_key(feed_id: &[u8; 32]) -> bool {
    let zero = [0_u8; 32];
    *feed_id != zero && !is_lazer_feed_key(feed_id)
}

fn validate_signed_feed_config(feed_id: &[u8; 32]) -> Result<()> {
    require!(is_signed_feed_key(feed_id), CoreError::InvalidParameter);
    Ok(())
}

fn lazer_feed_key(feed_id: PriceFeedId) -> [u8; 32] {
    let mut key = [0_u8; 32];
    key[..LAZER_FEED_ID_BYTES].copy_from_slice(&feed_id.0.to_le_bytes());
    key
}

fn read_lazer_price(payload: &[u8], config: &CollateralConfig) -> Result<OraclePriceAccount> {
    let expected_feed_id = lazer_feed_id_from_config(config)?;
    let data =
        PayloadData::deserialize_slice_le(payload).map_err(|_| error!(CoreError::OracleError))?;
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

fn signed_price_message(payload: &SignedPricePayload) -> Result<Vec<u8>> {
    payload
        .try_to_vec()
        .map_err(|_| error!(CoreError::OracleError))
}

fn verify_ed25519_instruction(
    instructions_sysvar: &AccountInfo,
    instruction_index: u16,
    signer: &Pubkey,
    message: &[u8],
) -> Result<()> {
    let ix = sysvar::instructions::load_instruction_at_checked(
        instruction_index as usize,
        instructions_sysvar,
    )
    .map_err(|_| error!(CoreError::OracleError))?;
    require_keys_eq!(ix.program_id, ed25519_program::ID, CoreError::OracleError);
    require!(
        ix.accounts.is_empty()
            && ix.data.len() >= ED25519_SIGNATURE_OFFSETS_START + ED25519_SIGNATURE_OFFSETS_SIZE
            && ix.data[0] == 1
            && ix.data[1] == 0,
        CoreError::OracleError
    );

    let offset = ED25519_SIGNATURE_OFFSETS_START;
    let signature_offset = read_u16_le(&ix.data, offset)?;
    let signature_instruction_index = read_u16_le(&ix.data, offset + 2)?;
    let public_key_offset = read_u16_le(&ix.data, offset + 4)?;
    let public_key_instruction_index = read_u16_le(&ix.data, offset + 6)?;
    let message_data_offset = read_u16_le(&ix.data, offset + 8)?;
    let message_data_size = read_u16_le(&ix.data, offset + 10)?;
    let message_instruction_index = read_u16_le(&ix.data, offset + 12)?;

    require!(
        signature_instruction_index == ED25519_CURRENT_INSTRUCTION
            && public_key_instruction_index == ED25519_CURRENT_INSTRUCTION
            && message_instruction_index == ED25519_CURRENT_INSTRUCTION,
        CoreError::OracleError
    );
    let signature_end = signature_offset
        .checked_add(64)
        .ok_or(error!(CoreError::MathOverflow))?;
    let public_key_end = public_key_offset
        .checked_add(32)
        .ok_or(error!(CoreError::MathOverflow))?;
    let message_end = message_data_offset
        .checked_add(message_data_size)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(
        signature_end <= ix.data.len()
            && public_key_end <= ix.data.len()
            && message_end <= ix.data.len(),
        CoreError::OracleError
    );

    let signed_public_key = &ix.data[public_key_offset..public_key_end];
    require!(
        signed_public_key == signer.as_ref(),
        CoreError::Unauthorized
    );
    let signed_message = &ix.data[message_data_offset..message_end];
    require!(signed_message == message, CoreError::OracleError);
    Ok(())
}

fn read_u16_le(data: &[u8], offset: usize) -> Result<usize> {
    let end = offset
        .checked_add(2)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(end <= data.len(), CoreError::OracleError);
    Ok(u16::from_le_bytes([data[offset], data[offset + 1]]) as usize)
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
    let price_e8 = scale_oracle_price_to_e8(
        u128::try_from(price_mantissa).map_err(|_| error!(CoreError::OracleError))?,
        i32::from(exponent),
        false,
    )?;
    let confidence_e8 = scale_oracle_price_to_e8(
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

fn scale_oracle_price_to_e8(value: u128, exponent: i32, round_up: bool) -> Result<u128> {
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
