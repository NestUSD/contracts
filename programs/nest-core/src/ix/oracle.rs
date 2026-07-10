use crate::*;

pub fn refresh_lazer_oracle(
    ctx: Context<RefreshLazerOracle>,
    message_data: Vec<u8>,
    ed25519_instruction_index: u16,
    signature_index: u8,
) -> Result<()> {
    let cpi_accounts = pyth_lazer_solana_contract::cpi::accounts::VerifyMessage {
        payer: ctx.accounts.payer.to_account_info(),
        storage: ctx.accounts.pyth_lazer_storage.to_account_info(),
        treasury: ctx.accounts.pyth_lazer_treasury.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
        instructions_sysvar: ctx.accounts.instructions_sysvar.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(
        ctx.accounts.pyth_lazer_program.to_account_info(),
        cpi_accounts,
    );
    let verified = pyth_lazer_solana_contract::cpi::verify_message(
        cpi_ctx,
        message_data,
        ed25519_instruction_index,
        signature_index,
    )?;
    let verified = verified.get();
    let (xstock_usd, source_publish_time_us) =
        read_lazer_price(&verified.payload, &ctx.accounts.collateral_config)?;
    let clock = Clock::get()?;
    validate_xstock_oracle_price(
        &ctx.accounts.collateral_config,
        xstock_usd,
        clock.unix_timestamp,
    )?;
    require_monotonic_oracle_update(&ctx.accounts.oracle, xstock_usd, source_publish_time_us)?;

    let oracle = &mut ctx.accounts.oracle;
    oracle.protocol = ctx.accounts.protocol.key();
    oracle.collateral_config = ctx.accounts.collateral_config.key();
    oracle.xstock_usd = xstock_usd;
    oracle.source_publish_time_us = source_publish_time_us;
    clear_reserved_oracle_fields(oracle);
    oracle.bump = ctx.bumps.oracle;
    Ok(())
}

pub fn refresh_signed_oracle(
    ctx: Context<RefreshSignedOracle>,
    payload: SignedPricePayload,
    ed25519_instruction_index: u16,
) -> Result<()> {
    let clock = Clock::get()?;
    let signed_message = signed_price_message(&payload)?;
    verify_ed25519_instruction(
        &ctx.accounts.instructions_sysvar,
        ed25519_instruction_index,
        &ctx.accounts.nest_price_signer.signer,
        &signed_message,
    )?;

    require!(
        payload.magic == SIGNED_PRICE_MAGIC && payload.version == SIGNED_PRICE_VERSION,
        CoreError::InvalidParameter
    );
    validate_signed_feed_config(&ctx.accounts.collateral_config.xstock_usd_feed_id)?;
    require!(
        payload.feed_id == ctx.accounts.collateral_config.xstock_usd_feed_id,
        CoreError::OracleError
    );
    require!(
        payload.price_e8 > 0
            && payload.expires_at >= payload.publish_time
            && payload.expires_at >= clock.unix_timestamp,
        CoreError::OracleError
    );

    let xstock_usd = OraclePriceAccount {
        feed_id: payload.feed_id,
        price_e8: payload.price_e8,
        confidence_e8: payload.confidence_e8,
        publish_time: payload.publish_time,
    };

    validate_xstock_oracle_price(
        &ctx.accounts.collateral_config,
        xstock_usd,
        clock.unix_timestamp,
    )?;
    let source_publish_time_us = payload
        .publish_time
        .checked_mul(1_000_000)
        .ok_or(error!(CoreError::MathOverflow))?;
    require_monotonic_oracle_update(&ctx.accounts.oracle, xstock_usd, source_publish_time_us)?;

    let oracle = &mut ctx.accounts.oracle;
    oracle.protocol = ctx.accounts.protocol.key();
    oracle.collateral_config = ctx.accounts.collateral_config.key();
    oracle.xstock_usd = xstock_usd;
    oracle.source_publish_time_us = source_publish_time_us;
    clear_reserved_oracle_fields(oracle);
    oracle.bump = ctx.bumps.oracle;
    Ok(())
}

fn clear_reserved_oracle_fields(oracle: &mut OracleSnapshot) {
    oracle.reserved_underlying_usd = inactive_oracle_price([0; 32]);
    oracle.reserved_redemption_rate = inactive_oracle_price([0; 32]);
    oracle.reserved_market_state = MarketStateAccount::Regular;
}
