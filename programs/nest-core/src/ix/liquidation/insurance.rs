use crate::*;

pub fn withdraw_insurance_collateral(
    ctx: Context<WithdrawInsuranceCollateral>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, CoreError::InvalidParameter);
    let bump = [ctx.accounts.protocol.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"protocol", &bump]];
    let insurance_collateral_before = ctx.accounts.insurance_collateral_vault.amount;
    let authority_collateral_before = ctx.accounts.authority_collateral_account.amount;
    token_transfer_checked(
        ctx.accounts.collateral_token_program.to_account_info(),
        ctx.accounts.insurance_collateral_vault.to_account_info(),
        ctx.accounts.collateral_mint.to_account_info(),
        ctx.accounts.authority_collateral_account.to_account_info(),
        ctx.accounts.protocol.to_account_info(),
        amount,
        ctx.accounts.collateral_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.insurance_collateral_vault.reload()?;
    ctx.accounts.authority_collateral_account.reload()?;
    require_token_account_decrease(
        insurance_collateral_before,
        ctx.accounts.insurance_collateral_vault.amount,
        amount,
    )?;
    require_token_account_increase(
        authority_collateral_before,
        ctx.accounts.authority_collateral_account.amount,
        amount,
    )?;
    Ok(())
}
