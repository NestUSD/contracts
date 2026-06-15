fn valid_kamino_program_id(program_id: Pubkey) -> bool {
    program_id == KLEND_PROGRAM_ID || program_id == KLEND_STAGING_PROGRAM_ID
}

struct PsmKaminoCpiAccounts<'a, 'info> {
    protocol_state: &'a Protocol,
    protocol: AccountInfo<'info>,
    usdc_mint: AccountInfo<'info>,
    psm_usdc_vault: AccountInfo<'info>,
    reserve_collateral_mint: AccountInfo<'info>,
    protocol_kamino_collateral_vault: AccountInfo<'info>,
    reserve: AccountInfo<'info>,
    lending_market: AccountInfo<'info>,
    lending_market_authority: AccountInfo<'info>,
    reserve_liquidity_supply: AccountInfo<'info>,
    pyth_oracle: AccountInfo<'info>,
    switchboard_price_oracle: AccountInfo<'info>,
    switchboard_twap_oracle: AccountInfo<'info>,
    scope_prices: AccountInfo<'info>,
    kamino_program: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    instructions_sysvar: AccountInfo<'info>,
}

fn psm_kamino_cpi_accounts<'a, 'info>(
    accounts: &'a PsmKamino<'info>,
) -> PsmKaminoCpiAccounts<'a, 'info> {
    PsmKaminoCpiAccounts {
        protocol_state: &accounts.protocol,
        protocol: accounts.protocol.to_account_info(),
        usdc_mint: accounts.usdc_mint.to_account_info(),
        psm_usdc_vault: accounts.psm_usdc_vault.to_account_info(),
        reserve_collateral_mint: accounts.reserve_collateral_mint.to_account_info(),
        protocol_kamino_collateral_vault: accounts
            .protocol_kamino_collateral_vault
            .to_account_info(),
        reserve: accounts.reserve.to_account_info(),
        lending_market: accounts.lending_market.to_account_info(),
        lending_market_authority: accounts.lending_market_authority.to_account_info(),
        reserve_liquidity_supply: accounts.reserve_liquidity_supply.to_account_info(),
        pyth_oracle: accounts.pyth_oracle.to_account_info(),
        switchboard_price_oracle: accounts.switchboard_price_oracle.to_account_info(),
        switchboard_twap_oracle: accounts.switchboard_twap_oracle.to_account_info(),
        scope_prices: accounts.scope_prices.to_account_info(),
        kamino_program: accounts.kamino_program.to_account_info(),
        token_program: accounts.token_program.to_account_info(),
        instructions_sysvar: accounts.instructions_sysvar.to_account_info(),
    }
}

fn psm_kamino_yield_cpi_accounts<'a, 'info>(
    accounts: &'a RealizePsmKaminoYield<'info>,
) -> PsmKaminoCpiAccounts<'a, 'info> {
    PsmKaminoCpiAccounts {
        protocol_state: &accounts.protocol,
        protocol: accounts.protocol.to_account_info(),
        usdc_mint: accounts.usdc_mint.to_account_info(),
        psm_usdc_vault: accounts.psm_usdc_vault.to_account_info(),
        reserve_collateral_mint: accounts.reserve_collateral_mint.to_account_info(),
        protocol_kamino_collateral_vault: accounts
            .protocol_kamino_collateral_vault
            .to_account_info(),
        reserve: accounts.reserve.to_account_info(),
        lending_market: accounts.lending_market.to_account_info(),
        lending_market_authority: accounts.lending_market_authority.to_account_info(),
        reserve_liquidity_supply: accounts.reserve_liquidity_supply.to_account_info(),
        pyth_oracle: accounts.pyth_oracle.to_account_info(),
        switchboard_price_oracle: accounts.switchboard_price_oracle.to_account_info(),
        switchboard_twap_oracle: accounts.switchboard_twap_oracle.to_account_info(),
        scope_prices: accounts.scope_prices.to_account_info(),
        kamino_program: accounts.kamino_program.to_account_info(),
        token_program: accounts.usdc_token_program.to_account_info(),
        instructions_sysvar: accounts.instructions_sysvar.to_account_info(),
    }
}

fn assert_kamino_psm_accounts(accounts: &PsmKamino<'_>) -> Result<()> {
    assert_kamino_psm_cpi_accounts(&psm_kamino_cpi_accounts(accounts))
}

fn assert_kamino_psm_yield_accounts(accounts: &RealizePsmKaminoYield<'_>) -> Result<()> {
    assert_kamino_psm_cpi_accounts(&psm_kamino_yield_cpi_accounts(accounts))
}

fn assert_kamino_psm_cpi_accounts(accounts: &PsmKaminoCpiAccounts<'_, '_>) -> Result<()> {
    let kamino_program_id = accounts.protocol_state.kamino_program_id;
    require!(
        accounts.protocol_state.psm_kamino_collateral_vault != Pubkey::default(),
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        accounts.protocol_kamino_collateral_vault.key(),
        accounts.protocol_state.psm_kamino_collateral_vault,
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        *accounts.reserve.owner,
        kamino_program_id,
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        *accounts.lending_market.owner,
        kamino_program_id,
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        *accounts.reserve_collateral_mint.owner,
        accounts.token_program.key(),
        CoreError::InvalidParameter
    );

    let reserve_data = accounts.reserve.try_borrow_data()?;
    let reserve_lending_market = read_klend_reserve_pubkey(
        &reserve_data,
        KLEND_RESERVE_LENDING_MARKET_OFFSET,
    )?;
    let reserve_liquidity_mint = read_klend_reserve_pubkey(
        &reserve_data,
        KLEND_RESERVE_LIQUIDITY_MINT_OFFSET,
    )?;
    let reserve_liquidity_supply = read_klend_reserve_pubkey(
        &reserve_data,
        KLEND_RESERVE_LIQUIDITY_SUPPLY_OFFSET,
    )?;
    let reserve_token_program = read_klend_reserve_pubkey(
        &reserve_data,
        KLEND_RESERVE_LIQUIDITY_TOKEN_PROGRAM_OFFSET,
    )?;
    let reserve_collateral_mint = read_klend_reserve_pubkey(
        &reserve_data,
        KLEND_RESERVE_COLLATERAL_MINT_OFFSET,
    )?;
    require_keys_eq!(
        reserve_lending_market,
        accounts.lending_market.key(),
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        reserve_liquidity_mint,
        accounts.usdc_mint.key(),
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        reserve_token_program,
        accounts.token_program.key(),
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        accounts.reserve_liquidity_supply.key(),
        reserve_liquidity_supply,
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        accounts.reserve_collateral_mint.key(),
        reserve_collateral_mint,
        CoreError::InvalidParameter
    );

    let (expected_lending_market_authority, _) = Pubkey::find_program_address(
        &[b"lma", accounts.lending_market.key().as_ref()],
        &kamino_program_id,
    );
    require_keys_eq!(
        accounts.lending_market_authority.key(),
        expected_lending_market_authority,
        CoreError::InvalidParameter
    );
    Ok(())
}

fn read_klend_reserve_pubkey(data: &[u8], offset: usize) -> Result<Pubkey> {
    let end = offset
        .checked_add(KLEND_RESERVE_PUBKEY_FIELD_SIZE)
        .ok_or(error!(CoreError::MathOverflow))?;
    require!(data.len() >= end, CoreError::InvalidParameter);
    let mut value = [0_u8; KLEND_RESERVE_PUBKEY_FIELD_SIZE];
    value.copy_from_slice(&data[offset..end]);
    Ok(Pubkey::new_from_array(value))
}

fn invoke_klend_refresh_reserve(accounts: &PsmKaminoCpiAccounts<'_, '_>) -> Result<()> {
    let mut account_metas = vec![
        AccountMeta::new(accounts.reserve.key(), false),
        AccountMeta::new_readonly(accounts.lending_market.key(), false),
    ];
    let mut account_infos = vec![accounts.reserve.clone(), accounts.lending_market.clone()];
    push_klend_optional_refresh_account(
        &mut account_metas,
        &mut account_infos,
        accounts.pyth_oracle.clone(),
        accounts.kamino_program.clone(),
        accounts.protocol_state.kamino_program_id,
    );
    push_klend_optional_refresh_account(
        &mut account_metas,
        &mut account_infos,
        accounts.switchboard_price_oracle.clone(),
        accounts.kamino_program.clone(),
        accounts.protocol_state.kamino_program_id,
    );
    push_klend_optional_refresh_account(
        &mut account_metas,
        &mut account_infos,
        accounts.switchboard_twap_oracle.clone(),
        accounts.kamino_program.clone(),
        accounts.protocol_state.kamino_program_id,
    );
    push_klend_optional_refresh_account(
        &mut account_metas,
        &mut account_infos,
        accounts.scope_prices.clone(),
        accounts.kamino_program.clone(),
        accounts.protocol_state.kamino_program_id,
    );
    account_infos.push(accounts.kamino_program.clone());
    let ix = Instruction {
        program_id: accounts.protocol_state.kamino_program_id,
        accounts: account_metas,
        data: KLEND_REFRESH_RESERVE_DISCRIMINATOR.to_vec(),
    };
    invoke_signed(&ix, &account_infos, &[])?;
    Ok(())
}

fn push_klend_optional_refresh_account<'info>(
    account_metas: &mut Vec<AccountMeta>,
    account_infos: &mut Vec<AccountInfo<'info>>,
    oracle_account: AccountInfo<'info>,
    kamino_program: AccountInfo<'info>,
    kamino_program_id: Pubkey,
) {
    // KLend optional oracle accounts are disabled by passing the program account
    // at the corresponding slot. Keep metas and infos aligned for CPI.
    if is_disabled_klend_optional_account(*oracle_account.key, kamino_program_id) {
        account_metas.push(AccountMeta::new_readonly(kamino_program_id, false));
        account_infos.push(kamino_program);
    } else {
        account_metas.push(AccountMeta::new_readonly(*oracle_account.key, false));
        account_infos.push(oracle_account);
    }
}

fn is_disabled_klend_optional_account(account: Pubkey, kamino_program_id: Pubkey) -> bool {
    account == Pubkey::default() || account == KLEND_NULL_PUBKEY || account == kamino_program_id
}

fn invoke_klend_deposit_reserve_liquidity(
    accounts: &PsmKaminoCpiAccounts<'_, '_>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = KLEND_DEPOSIT_RESERVE_LIQUIDITY_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    let ix = Instruction {
        program_id: accounts.protocol_state.kamino_program_id,
        accounts: vec![
            AccountMeta::new_readonly(accounts.protocol.key(), true),
            AccountMeta::new(accounts.reserve.key(), false),
            AccountMeta::new_readonly(accounts.lending_market.key(), false),
            AccountMeta::new_readonly(accounts.lending_market_authority.key(), false),
            AccountMeta::new_readonly(accounts.usdc_mint.key(), false),
            AccountMeta::new(accounts.reserve_liquidity_supply.key(), false),
            AccountMeta::new(accounts.reserve_collateral_mint.key(), false),
            AccountMeta::new(accounts.psm_usdc_vault.key(), false),
            AccountMeta::new(accounts.protocol_kamino_collateral_vault.key(), false),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(accounts.token_program.key(), false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
        ],
        data,
    };
    invoke_signed(
        &ix,
        &[
            accounts.protocol.clone(),
            accounts.reserve.clone(),
            accounts.lending_market.clone(),
            accounts.lending_market_authority.clone(),
            accounts.usdc_mint.clone(),
            accounts.reserve_liquidity_supply.clone(),
            accounts.reserve_collateral_mint.clone(),
            accounts.psm_usdc_vault.clone(),
            accounts.protocol_kamino_collateral_vault.clone(),
            accounts.token_program.clone(),
            accounts.token_program.clone(),
            accounts.instructions_sysvar.clone(),
            accounts.kamino_program.clone(),
        ],
        signer_seeds,
    )?;
    Ok(())
}

fn invoke_klend_redeem_reserve_collateral(
    accounts: &PsmKaminoCpiAccounts<'_, '_>,
    collateral_amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = KLEND_REDEEM_RESERVE_COLLATERAL_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&collateral_amount.to_le_bytes());
    let ix = Instruction {
        program_id: accounts.protocol_state.kamino_program_id,
        accounts: vec![
            AccountMeta::new_readonly(accounts.protocol.key(), true),
            AccountMeta::new_readonly(accounts.lending_market.key(), false),
            AccountMeta::new(accounts.reserve.key(), false),
            AccountMeta::new_readonly(accounts.lending_market_authority.key(), false),
            AccountMeta::new_readonly(accounts.usdc_mint.key(), false),
            AccountMeta::new(accounts.reserve_collateral_mint.key(), false),
            AccountMeta::new(accounts.reserve_liquidity_supply.key(), false),
            AccountMeta::new(accounts.protocol_kamino_collateral_vault.key(), false),
            AccountMeta::new(accounts.psm_usdc_vault.key(), false),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(accounts.token_program.key(), false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
        ],
        data,
    };
    invoke_signed(
        &ix,
        &[
            accounts.protocol.clone(),
            accounts.lending_market.clone(),
            accounts.reserve.clone(),
            accounts.lending_market_authority.clone(),
            accounts.usdc_mint.clone(),
            accounts.reserve_collateral_mint.clone(),
            accounts.reserve_liquidity_supply.clone(),
            accounts.protocol_kamino_collateral_vault.clone(),
            accounts.psm_usdc_vault.clone(),
            accounts.token_program.clone(),
            accounts.token_program.clone(),
            accounts.instructions_sysvar.clone(),
            accounts.kamino_program.clone(),
        ],
        signer_seeds,
    )?;
    Ok(())
}

#[cfg(test)]
mod kamino_helper_tests {
    use super::*;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn write_key(data: &mut [u8], offset: usize, value: Pubkey) {
        let end = offset + KLEND_RESERVE_PUBKEY_FIELD_SIZE;
        data[offset..end].copy_from_slice(value.as_ref());
    }

    #[test]
    fn reads_klend_reserve_pubkeys_at_expected_offsets() {
        let lending_market = key(1);
        let liquidity_mint = key(2);
        let liquidity_supply = key(3);
        let token_program = key(4);
        let collateral_mint = key(5);
        let mut reserve_data =
            vec![0_u8; KLEND_RESERVE_COLLATERAL_MINT_OFFSET + KLEND_RESERVE_PUBKEY_FIELD_SIZE];
        write_key(
            &mut reserve_data,
            KLEND_RESERVE_LENDING_MARKET_OFFSET,
            lending_market,
        );
        write_key(
            &mut reserve_data,
            KLEND_RESERVE_LIQUIDITY_MINT_OFFSET,
            liquidity_mint,
        );
        write_key(
            &mut reserve_data,
            KLEND_RESERVE_LIQUIDITY_SUPPLY_OFFSET,
            liquidity_supply,
        );
        write_key(
            &mut reserve_data,
            KLEND_RESERVE_LIQUIDITY_TOKEN_PROGRAM_OFFSET,
            token_program,
        );
        write_key(
            &mut reserve_data,
            KLEND_RESERVE_COLLATERAL_MINT_OFFSET,
            collateral_mint,
        );

        assert_eq!(
            read_klend_reserve_pubkey(&reserve_data, KLEND_RESERVE_LENDING_MARKET_OFFSET)
                .unwrap(),
            lending_market
        );
        assert_eq!(
            read_klend_reserve_pubkey(&reserve_data, KLEND_RESERVE_LIQUIDITY_MINT_OFFSET)
                .unwrap(),
            liquidity_mint
        );
        assert_eq!(
            read_klend_reserve_pubkey(&reserve_data, KLEND_RESERVE_LIQUIDITY_SUPPLY_OFFSET)
                .unwrap(),
            liquidity_supply
        );
        assert_eq!(
            read_klend_reserve_pubkey(
                &reserve_data,
                KLEND_RESERVE_LIQUIDITY_TOKEN_PROGRAM_OFFSET
            )
            .unwrap(),
            token_program
        );
        assert_eq!(
            read_klend_reserve_pubkey(&reserve_data, KLEND_RESERVE_COLLATERAL_MINT_OFFSET)
                .unwrap(),
            collateral_mint
        );
    }

    #[test]
    fn rejects_short_klend_reserve_data_for_required_pubkeys() {
        for offset in [
            KLEND_RESERVE_LENDING_MARKET_OFFSET,
            KLEND_RESERVE_LIQUIDITY_MINT_OFFSET,
            KLEND_RESERVE_LIQUIDITY_SUPPLY_OFFSET,
            KLEND_RESERVE_LIQUIDITY_TOKEN_PROGRAM_OFFSET,
            KLEND_RESERVE_COLLATERAL_MINT_OFFSET,
        ] {
            let short_data = vec![0_u8; offset + KLEND_RESERVE_PUBKEY_FIELD_SIZE - 1];
            assert!(read_klend_reserve_pubkey(&short_data, offset).is_err());
        }
    }

    #[test]
    fn recognizes_disabled_klend_optional_oracle_accounts() {
        assert!(is_disabled_klend_optional_account(
            Pubkey::default(),
            KLEND_PROGRAM_ID
        ));
        assert!(is_disabled_klend_optional_account(
            KLEND_NULL_PUBKEY,
            KLEND_PROGRAM_ID
        ));
        assert!(is_disabled_klend_optional_account(
            KLEND_STAGING_PROGRAM_ID,
            KLEND_STAGING_PROGRAM_ID
        ));
        assert!(!is_disabled_klend_optional_account(
            key(9),
            KLEND_PROGRAM_ID
        ));
    }
}
