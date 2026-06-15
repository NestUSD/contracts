use crate::*;

pub fn release_buyback_surplus(
    ctx: Context<ReleaseBuybackSurplus>,
    amount: u64,
    min_psm_idle_after: u64,
) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    require!(!ctx.accounts.protocol.paused, CoreError::Paused);
    require!(
        ctx.accounts.protocol.bad_debt_nusd == 0,
        CoreError::BadDebtOutstanding
    );
    require_psm_accounting_invariants(&ctx.accounts.protocol, ctx.accounts.psm_usdc_vault.amount)?;

    let amount_u128 = amount as u128;
    require!(
        amount_u128 <= ctx.accounts.protocol.realized_revenue_for_protocol,
        CoreError::InsufficientProtocolRevenue
    );
    require!(
        (ctx.accounts.protocol_revenue_nusd_vault.amount as u128) >= amount_u128,
        CoreError::InsufficientProtocolRevenue
    );
    let min_idle_after = min_psm_idle_after as u128;
    require!(
        ctx.accounts.protocol.psm_idle_usdc
            >= amount_u128
                .checked_add(min_idle_after)
                .ok_or(error!(CoreError::MathOverflow))?,
        CoreError::PsmInsufficientLiquidity
    );
    require!(
        ctx.accounts.protocol.psm_usdc_liabilities >= amount_u128
            && ctx.accounts.protocol.psm_nusd_supply >= amount_u128,
        CoreError::InvalidParameter
    );

    ctx.accounts.protocol.realized_revenue_for_protocol = ctx
        .accounts
        .protocol
        .realized_revenue_for_protocol
        .checked_sub(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.protocol.psm_usdc_liabilities = ctx
        .accounts
        .protocol
        .psm_usdc_liabilities
        .checked_sub(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.protocol.psm_nusd_supply = ctx
        .accounts
        .protocol
        .psm_nusd_supply
        .checked_sub(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    ctx.accounts.protocol.psm_idle_usdc = ctx
        .accounts
        .protocol
        .psm_idle_usdc
        .checked_sub(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;

    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    let protocol_revenue_before = ctx.accounts.protocol_revenue_nusd_vault.amount;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;
    let psm_usdc_before = ctx.accounts.psm_usdc_vault.amount;
    let buyback_usdc_before = ctx.accounts.buyback_usdc_account.amount;
    token_burn_checked(
        ctx.accounts.nusd_token_program.to_account_info(),
        ctx.accounts.nusd_mint.to_account_info(),
        ctx.accounts.protocol_revenue_nusd_vault.to_account_info(),
        ctx.accounts.protocol.to_account_info(),
        amount,
        ctx.accounts.nusd_mint.decimals,
        signer_seeds,
    )?;
    token_transfer_checked(
        ctx.accounts.usdc_token_program.to_account_info(),
        ctx.accounts.psm_usdc_vault.to_account_info(),
        ctx.accounts.usdc_mint.to_account_info(),
        ctx.accounts.buyback_usdc_account.to_account_info(),
        ctx.accounts.protocol.to_account_info(),
        amount,
        ctx.accounts.usdc_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.protocol_revenue_nusd_vault.reload()?;
    ctx.accounts.nusd_mint.reload()?;
    ctx.accounts.psm_usdc_vault.reload()?;
    ctx.accounts.buyback_usdc_account.reload()?;
    require_token_account_decrease(
        protocol_revenue_before,
        ctx.accounts.protocol_revenue_nusd_vault.amount,
        amount,
    )?;
    require_mint_supply_decrease(nusd_supply_before, ctx.accounts.nusd_mint.supply, amount)?;
    require_token_account_decrease(psm_usdc_before, ctx.accounts.psm_usdc_vault.amount, amount)?;
    require_token_account_increase(
        buyback_usdc_before,
        ctx.accounts.buyback_usdc_account.amount,
        amount,
    )?;
    require_recorded_amount_covered(
        ctx.accounts.protocol_revenue_nusd_vault.amount,
        ctx.accounts.protocol.realized_revenue_for_protocol,
    )?;
    require_psm_accounting_invariants(&ctx.accounts.protocol, ctx.accounts.psm_usdc_vault.amount)?;
    Ok(())
}
