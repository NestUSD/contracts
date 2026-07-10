use crate::*;

pub(crate) fn require_psm_redemption_solvent(protocol: &Protocol) -> Result<()> {
    require!(protocol.bad_debt_nusd == 0, CoreError::BadDebtOutstanding);
    Ok(())
}

pub fn psm_swap_in(ctx: Context<PsmSwapIn>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    require!(!ctx.accounts.protocol.paused, CoreError::Paused);
    let amount_u128 = amount as u128;
    let user_nusd_before = ctx.accounts.user_nusd_account.amount;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;
    let protocol = &mut ctx.accounts.protocol;
    let new_liability = protocol
        .psm_usdc_liabilities
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(new_liability <= protocol.psm_cap, CoreError::PsmCapExceeded);
    protocol.psm_usdc_liabilities = new_liability;
    protocol.psm_nusd_supply = protocol
        .psm_nusd_supply
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_idle_usdc = protocol
        .psm_idle_usdc
        .checked_add(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    let user_usdc_before = ctx.accounts.user_usdc_account.amount;
    let psm_vault_before = ctx.accounts.psm_usdc_vault.amount;
    token_transfer_checked(
        ctx.accounts.usdc_token_program.to_account_info(),
        ctx.accounts.user_usdc_account.to_account_info(),
        ctx.accounts.usdc_mint.to_account_info(),
        ctx.accounts.psm_usdc_vault.to_account_info(),
        ctx.accounts.user.to_account_info(),
        amount,
        ctx.accounts.usdc_mint.decimals,
        &[],
    )?;
    ctx.accounts.user_usdc_account.reload()?;
    ctx.accounts.psm_usdc_vault.reload()?;
    require_token_account_decrease(
        user_usdc_before,
        ctx.accounts.user_usdc_account.amount,
        amount,
    )?;
    require_token_account_increase(psm_vault_before, ctx.accounts.psm_usdc_vault.amount, amount)?;
    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    token_mint_to_checked(
        ctx.accounts.nusd_token_program.to_account_info(),
        ctx.accounts.nusd_mint.to_account_info(),
        ctx.accounts.user_nusd_account.to_account_info(),
        ctx.accounts.protocol.to_account_info(),
        amount,
        ctx.accounts.nusd_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.user_nusd_account.reload()?;
    ctx.accounts.nusd_mint.reload()?;
    require_token_account_increase(
        user_nusd_before,
        ctx.accounts.user_nusd_account.amount,
        amount,
    )?;
    require_mint_supply_increase(nusd_supply_before, ctx.accounts.nusd_mint.supply, amount)?;
    require_psm_accounting_invariants(&ctx.accounts.protocol, ctx.accounts.psm_usdc_vault.amount)?;
    Ok(())
}

pub fn psm_swap_out(ctx: Context<PsmSwapOut>, amount: u64) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    require!(!ctx.accounts.protocol.paused, CoreError::Paused);
    require_psm_redemption_solvent(&ctx.accounts.protocol)?;
    let amount_u128 = amount as u128;
    let now = Clock::get()?.unix_timestamp;
    let user_nusd_before = ctx.accounts.user_nusd_account.amount;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;
    let protocol = &mut ctx.accounts.protocol;
    require!(
        protocol.psm_idle_usdc >= amount_u128,
        CoreError::PsmInsufficientLiquidity
    );
    require!(
        ctx.accounts.user_nusd_account.amount >= amount,
        CoreError::PsmInsufficientLiquidity
    );
    let psm_idle_before = protocol.psm_idle_usdc;
    let outflow_allowed = record_psm_outflow_or_pause(protocol, amount_u128, psm_idle_before, now)?;
    if !outflow_allowed {
        return Ok(());
    }
    protocol.psm_usdc_liabilities = protocol
        .psm_usdc_liabilities
        .checked_sub(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_nusd_supply = protocol
        .psm_nusd_supply
        .checked_sub(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    protocol.psm_idle_usdc = protocol
        .psm_idle_usdc
        .checked_sub(amount_u128)
        .ok_or(error!(CoreError::MathOverflow))?;
    token_burn_checked(
        ctx.accounts.nusd_token_program.to_account_info(),
        ctx.accounts.nusd_mint.to_account_info(),
        ctx.accounts.user_nusd_account.to_account_info(),
        ctx.accounts.user.to_account_info(),
        amount,
        ctx.accounts.nusd_mint.decimals,
        &[],
    )?;
    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    let psm_vault_before = ctx.accounts.psm_usdc_vault.amount;
    let user_usdc_before = ctx.accounts.user_usdc_account.amount;
    token_transfer_checked(
        ctx.accounts.usdc_token_program.to_account_info(),
        ctx.accounts.psm_usdc_vault.to_account_info(),
        ctx.accounts.usdc_mint.to_account_info(),
        ctx.accounts.user_usdc_account.to_account_info(),
        ctx.accounts.protocol.to_account_info(),
        amount,
        ctx.accounts.usdc_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.user_usdc_account.reload()?;
    ctx.accounts.psm_usdc_vault.reload()?;
    require_token_account_decrease(psm_vault_before, ctx.accounts.psm_usdc_vault.amount, amount)?;
    require_token_account_increase(
        user_usdc_before,
        ctx.accounts.user_usdc_account.amount,
        amount,
    )?;
    ctx.accounts.user_nusd_account.reload()?;
    ctx.accounts.nusd_mint.reload()?;
    require_token_account_decrease(
        user_nusd_before,
        ctx.accounts.user_nusd_account.amount,
        amount,
    )?;
    require_mint_supply_decrease(nusd_supply_before, ctx.accounts.nusd_mint.supply, amount)?;
    require_psm_accounting_invariants(&ctx.accounts.protocol, ctx.accounts.psm_usdc_vault.amount)?;
    Ok(())
}
