fn token_transfer_checked<'info>(
    token_program: AccountInfo<'info>,
    from: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    to: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let accounts = TransferChecked {
        from,
        mint,
        to,
        authority,
    };
    if signer_seeds.is_empty() {
        token_interface::transfer_checked(
            CpiContext::new(token_program, accounts),
            amount,
            decimals,
        )
    } else {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(token_program, accounts, signer_seeds),
            amount,
            decimals,
        )
    }
}

fn token_mint_to_checked<'info>(
    token_program: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    to: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let accounts = MintToChecked {
        mint,
        to,
        authority,
    };
    token_interface::mint_to_checked(
        CpiContext::new_with_signer(token_program, accounts, signer_seeds),
        amount,
        decimals,
    )
}

fn token_burn_checked<'info>(
    token_program: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    from: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let accounts = BurnChecked {
        mint,
        from,
        authority,
    };
    if signer_seeds.is_empty() {
        token_interface::burn_checked(CpiContext::new(token_program, accounts), amount, decimals)
    } else {
        token_interface::burn_checked(
            CpiContext::new_with_signer(token_program, accounts, signer_seeds),
            amount,
            decimals,
        )
    }
}

fn u128_to_u64(amount: u128) -> Result<u64> {
    u64::try_from(amount).map_err(|_| error!(CoreError::AmountOverflow))
}

fn require_token_account_unencumbered(account: &InterfaceAccount<TokenAccount>) -> Result<()> {
    require!(
        account.delegate == COption::None
            && account.delegated_amount == 0
            && account.close_authority == COption::None
            && account.is_native == COption::None,
        CoreError::InvalidParameter
    );
    Ok(())
}

fn require_token_account_increase(before: u64, after: u64, expected_delta: u64) -> Result<()> {
    let expected_after = before
        .checked_add(expected_delta)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(after == expected_after, CoreError::TransferFeeNotSupported);
    Ok(())
}

fn require_token_account_decrease(before: u64, after: u64, expected_delta: u64) -> Result<()> {
    let expected_after = before
        .checked_sub(expected_delta)
        .ok_or(error!(CoreError::TransferFeeNotSupported))?;
    require!(after == expected_after, CoreError::TransferFeeNotSupported);
    Ok(())
}

fn require_mint_supply_increase(before: u64, after: u64, expected_delta: u64) -> Result<()> {
    let expected_after = before
        .checked_add(expected_delta)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(after == expected_after, CoreError::TransferFeeNotSupported);
    Ok(())
}

fn require_mint_supply_decrease(before: u64, after: u64, expected_delta: u64) -> Result<()> {
    let expected_after = before
        .checked_sub(expected_delta)
        .ok_or(error!(CoreError::TransferFeeNotSupported))?;
    require!(after == expected_after, CoreError::TransferFeeNotSupported);
    Ok(())
}

fn require_recorded_amount_covered(actual_amount: u64, recorded_amount: u128) -> Result<()> {
    require!(
        (actual_amount as u128) >= recorded_amount,
        CoreError::InvalidParameter
    );
    Ok(())
}

fn require_new_staker_revenue_covered(
    staker_revenue_vault_balance: u64,
    newly_routed_revenue: u128,
) -> Result<()> {
    require_recorded_amount_covered(staker_revenue_vault_balance, newly_routed_revenue)
}
