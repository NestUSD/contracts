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
    let xstock_usd = read_lazer_price(&verified.payload, &ctx.accounts.collateral_config)?;
    let clock = Clock::get()?;
    domain::safe_raw_token_price_e8(domain::PricingInputs {
        xstock_usd: to_domain_price(xstock_usd),
        underlying_usd: to_domain_price(inactive_oracle_price(
            ctx.accounts.collateral_config.underlying_usd_feed_id,
        )),
        redemption_rate: to_domain_price(inactive_oracle_price(
            ctx.accounts.collateral_config.redemption_rate_feed_id,
        )),
        now: clock.unix_timestamp,
        xstock_policy: policy(
            ctx.accounts.collateral_config.xstock_usd_feed_id,
            &ctx.accounts.collateral_config,
        )?,
        underlying_policy: policy(
            ctx.accounts.collateral_config.underlying_usd_feed_id,
            &ctx.accounts.collateral_config,
        )?,
        redemption_policy: policy(
            ctx.accounts.collateral_config.redemption_rate_feed_id,
            &ctx.accounts.collateral_config,
        )?,
    })
    .map_err(map_core_error)?;

    let oracle = &mut ctx.accounts.oracle;
    oracle.protocol = ctx.accounts.protocol.key();
    oracle.collateral_config = ctx.accounts.collateral_config.key();
    oracle.xstock_usd = xstock_usd;
    oracle.underlying_usd =
        inactive_oracle_price(ctx.accounts.collateral_config.underlying_usd_feed_id);
    oracle.redemption_rate =
        inactive_oracle_price(ctx.accounts.collateral_config.redemption_rate_feed_id);
    oracle.market_state = MarketStateAccount::Regular;
    oracle.calendar_valid_until_ts = 0;
    oracle.bump = ctx.bumps.oracle;
    Ok(())
}
