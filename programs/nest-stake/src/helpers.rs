fn assert_authority(state: &StakingState, signer: &Signer) -> Result<()> {
    require_keys_eq!(state.authority, signer.key(), StakeError::Unauthorized);
    Ok(())
}

fn domain_pool(state: &StakingState) -> domain::StakingPool {
    domain::StakingPool {
        total_shares: state.total_shares,
        staking_vault_nusd: state.staking_vault_nusd,
        unvested_revenue: state.unvested_revenue,
        reserved_pending_claims: state.reserved_pending_claims,
        vesting_start_ts: state.vesting_start_ts,
        vesting_end_ts: state.vesting_end_ts,
        last_vesting_sync_ts: state.last_vesting_sync_ts,
        cooldown_seconds: state.cooldown_seconds,
        revenue_vesting_seconds: state.revenue_vesting_seconds,
    }
}

fn apply_pool(state: &mut StakingState, pool: domain::StakingPool) {
    state.total_shares = pool.total_shares;
    state.staking_vault_nusd = pool.staking_vault_nusd;
    state.unvested_revenue = pool.unvested_revenue;
    state.reserved_pending_claims = pool.reserved_pending_claims;
    state.vesting_start_ts = pool.vesting_start_ts;
    state.vesting_end_ts = pool.vesting_end_ts;
    state.last_vesting_sync_ts = pool.last_vesting_sync_ts;
}

fn checkpoint_staker_target_revenue<'info>(
    protocol: AccountInfo<'info>,
    staking_state: AccountInfo<'info>,
    nest_core_program: AccountInfo<'info>,
    staking_bump: u8,
) -> Result<()> {
    let bump = [staking_bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"staking", &bump]];
    nest_core::cpi::checkpoint_staker_target_revenue(CpiContext::new_with_signer(
        nest_core_program,
        nest_core::cpi::accounts::CheckpointStakerTargetRevenue {
            protocol,
            staking_state,
        },
        signer_seeds,
    ))
}

fn consume_staker_revenue<'info>(
    protocol: AccountInfo<'info>,
    staking_state: AccountInfo<'info>,
    nest_core_program: AccountInfo<'info>,
    staking_bump: u8,
    amount: u64,
) -> Result<()> {
    let bump = [staking_bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"staking", &bump]];
    nest_core::cpi::consume_staker_revenue(
        CpiContext::new_with_signer(
            nest_core_program,
            nest_core::cpi::accounts::ConsumeStakerRevenue {
                protocol,
                staking_state,
            },
            signer_seeds,
        ),
        amount,
    )
}

fn absorb_staking_loss<'info>(
    protocol: AccountInfo<'info>,
    staking_state: AccountInfo<'info>,
    nest_core_program: AccountInfo<'info>,
    staking_bump: u8,
    amount: u64,
) -> Result<()> {
    let bump = [staking_bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"staking", &bump]];
    nest_core::cpi::absorb_staking_loss(
        CpiContext::new_with_signer(
            nest_core_program,
            nest_core::cpi::accounts::AbsorbStakingLoss {
                protocol,
                staking_state,
            },
            signer_seeds,
        ),
        amount,
    )
}

fn staking_authority_and_capacity_from_protocol(
    protocol: &AccountInfo,
    expected_nusd_mint: Pubkey,
) -> Result<(Pubkey, Option<u128>)> {
    require_core_protocol_account(
        protocol,
        expected_nusd_mint,
        CORE_PROTOCOL_STAKER_CAPACITY_KAMINO_APR_BPS_OFFSET + 8,
    )?;
    let data = protocol.try_borrow_data()?;
    let authority = read_pubkey_at(&data, CORE_PROTOCOL_AUTHORITY_OFFSET)?;

    let target_apr_bps = read_u64_at(&data, CORE_PROTOCOL_STAKER_TARGET_APR_BPS_OFFSET)?;
    if target_apr_bps == 0 {
        return Ok((authority, None));
    }

    let total_debt = read_u128_at(&data, CORE_PROTOCOL_TOTAL_DEBT_OFFSET)?;
    let insurance_fund_nusd = read_u128_at(&data, CORE_PROTOCOL_INSURANCE_FUND_NUSD_OFFSET)?;
    let psm_idle_usdc = read_u128_at(&data, CORE_PROTOCOL_PSM_IDLE_USDC_OFFSET)?;
    let psm_kamino_deployed_usdc =
        read_u128_at(&data, CORE_PROTOCOL_PSM_KAMINO_DEPLOYED_USDC_OFFSET)?;
    let stability_fee_apr_bps = read_u64_at(&data, CORE_PROTOCOL_STABILITY_FEE_APR_BPS_OFFSET)?;
    let insurance_target_bps = read_u16_at(&data, CORE_PROTOCOL_INSURANCE_TARGET_BPS_OFFSET)?;
    let insurance_fee_share_bps = read_u16_at(&data, CORE_PROTOCOL_INSURANCE_FEE_SHARE_BPS_OFFSET)?;
    let kamino_apr_bps = read_u64_at(&data, CORE_PROTOCOL_STAKER_CAPACITY_KAMINO_APR_BPS_OFFSET)?;

    let borrow_revenue = total_debt
        .checked_mul(stability_fee_apr_bps as u128)
        .ok_or(error!(StakeError::MathOverflow))?
        .checked_div(domain::BPS_DENOMINATOR)
        .ok_or(error!(StakeError::MathOverflow))?;
    let deployable_psm_usdc = psm_idle_usdc
        .checked_add(psm_kamino_deployed_usdc)
        .ok_or(error!(StakeError::MathOverflow))?;
    let kamino_revenue = deployable_psm_usdc
        .checked_mul(kamino_apr_bps as u128)
        .ok_or(error!(StakeError::MathOverflow))?
        .checked_div(domain::BPS_DENOMINATOR)
        .ok_or(error!(StakeError::MathOverflow))?;
    let gross_revenue = borrow_revenue
        .checked_add(kamino_revenue)
        .ok_or(error!(StakeError::MathOverflow))?;
    let insurance_target = total_debt
        .checked_mul(insurance_target_bps as u128)
        .ok_or(error!(StakeError::MathOverflow))?
        .checked_div(domain::BPS_DENOMINATOR)
        .ok_or(error!(StakeError::MathOverflow))?;
    let insurance_shortfall = if insurance_target > insurance_fund_nusd {
        insurance_target
            .checked_sub(insurance_fund_nusd)
            .ok_or(error!(StakeError::MathOverflow))?
    } else {
        0
    };
    let insurance_share = gross_revenue
        .checked_mul(insurance_fee_share_bps as u128)
        .ok_or(error!(StakeError::MathOverflow))?
        .checked_div(domain::BPS_DENOMINATOR)
        .ok_or(error!(StakeError::MathOverflow))?;
    let net_revenue = gross_revenue
        .checked_sub(core::cmp::min(insurance_share, insurance_shortfall))
        .ok_or(error!(StakeError::MathOverflow))?;
    let capacity = net_revenue
        .checked_mul(domain::BPS_DENOMINATOR)
        .ok_or(error!(StakeError::MathOverflow))?
        .checked_div(target_apr_bps as u128)
        .ok_or(error!(StakeError::MathOverflow))?;
    Ok((authority, Some(capacity)))
}

fn require_core_protocol_account(
    protocol: &AccountInfo,
    expected_nusd_mint: Pubkey,
    minimum_data_len: usize,
) -> Result<()> {
    require_keys_eq!(
        *protocol.owner,
        NEST_CORE_PROGRAM_ID,
        StakeError::InvalidParameter
    );
    let (expected_protocol, _) = Pubkey::find_program_address(&[b"protocol"], &NEST_CORE_PROGRAM_ID);
    require_keys_eq!(
        protocol.key(),
        expected_protocol,
        StakeError::InvalidParameter
    );
    let data = protocol.try_borrow_data()?;
    require!(
        data.len() >= minimum_data_len
            && data.get(0..8) == Some(&CORE_PROTOCOL_DISCRIMINATOR),
        StakeError::InvalidParameter
    );
    require_keys_eq!(
        read_pubkey_at(&data, CORE_PROTOCOL_NUSD_MINT_OFFSET)?,
        expected_nusd_mint,
        StakeError::InvalidParameter
    );
    Ok(())
}

fn require_no_bad_debt(protocol: &AccountInfo, expected_nusd_mint: Pubkey) -> Result<()> {
    require_core_protocol_account(
        protocol,
        expected_nusd_mint,
        CORE_PROTOCOL_BAD_DEBT_NUSD_OFFSET + 16,
    )?;
    let data = protocol.try_borrow_data()?;
    require!(
        read_u128_at(&data, CORE_PROTOCOL_BAD_DEBT_NUSD_OFFSET)? == 0,
        StakeError::BadDebtOutstanding
    );
    Ok(())
}

fn staking_capacity_from_protocol(
    protocol: &AccountInfo,
    expected_nusd_mint: Pubkey,
) -> Result<Option<u128>> {
    Ok(staking_authority_and_capacity_from_protocol(protocol, expected_nusd_mint)?.1)
}

fn read_u128_at(data: &[u8], offset: usize) -> Result<u128> {
    let slice = data
        .get(offset..offset + 16)
        .ok_or(error!(StakeError::InvalidParameter))?;
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(slice);
    Ok(u128::from_le_bytes(bytes))
}

fn read_u64_at(data: &[u8], offset: usize) -> Result<u64> {
    let slice = data
        .get(offset..offset + 8)
        .ok_or(error!(StakeError::InvalidParameter))?;
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(slice);
    Ok(u64::from_le_bytes(bytes))
}

fn read_u16_at(data: &[u8], offset: usize) -> Result<u16> {
    let slice = data
        .get(offset..offset + 2)
        .ok_or(error!(StakeError::InvalidParameter))?;
    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(slice);
    Ok(u16::from_le_bytes(bytes))
}

fn read_pubkey_at(data: &[u8], offset: usize) -> Result<Pubkey> {
    let slice = data
        .get(offset..offset + 32)
        .ok_or(error!(StakeError::InvalidParameter))?;
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(slice);
    Ok(Pubkey::new_from_array(bytes))
}

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
    u64::try_from(amount).map_err(|_| error!(StakeError::AmountOverflow))
}

fn require_token_account_unencumbered(account: &InterfaceAccount<TokenAccount>) -> Result<()> {
    require!(
        account.delegate == COption::None
            && account.delegated_amount == 0
            && account.close_authority == COption::None
            && account.is_native == COption::None,
        StakeError::InvalidParameter
    );
    Ok(())
}

fn require_token_account_increase(before: u64, after: u64, expected_delta: u64) -> Result<()> {
    let expected_after = before
        .checked_add(expected_delta)
        .ok_or(error!(StakeError::MathOverflow))?;
    require!(after == expected_after, StakeError::TransferFeeNotSupported);
    Ok(())
}

fn require_token_account_decrease(before: u64, after: u64, expected_delta: u64) -> Result<()> {
    let expected_after = before
        .checked_sub(expected_delta)
        .ok_or(error!(StakeError::TransferFeeNotSupported))?;
    require!(after == expected_after, StakeError::TransferFeeNotSupported);
    Ok(())
}

fn require_mint_supply_increase(before: u64, after: u64, expected_delta: u64) -> Result<()> {
    let expected_after = before
        .checked_add(expected_delta)
        .ok_or(error!(StakeError::MathOverflow))?;
    require!(after == expected_after, StakeError::TransferFeeNotSupported);
    Ok(())
}

fn require_mint_supply_decrease(before: u64, after: u64, expected_delta: u64) -> Result<()> {
    let expected_after = before
        .checked_sub(expected_delta)
        .ok_or(error!(StakeError::TransferFeeNotSupported))?;
    require!(after == expected_after, StakeError::TransferFeeNotSupported);
    Ok(())
}

fn require_recorded_staking_vault_covered(actual_amount: u64, recorded_amount: u128) -> Result<()> {
    require!(
        actual_amount as u128 >= recorded_amount,
        StakeError::InsufficientAssets
    );
    Ok(())
}

fn map_stake_error(error: domain::NestError) -> Error {
    match error {
        domain::NestError::MathOverflow | domain::NestError::DivisionByZero => {
            error!(StakeError::MathOverflow)
        }
        domain::NestError::InvalidParameter => error!(StakeError::InvalidParameter),
        domain::NestError::CooldownActive => error!(StakeError::CooldownActive),
        domain::NestError::ClaimWindowActive => error!(StakeError::ClaimWindowActive),
        domain::NestError::ClaimWindowExpired => error!(StakeError::ClaimWindowExpired),
        domain::NestError::InsufficientAssets => error!(StakeError::InsufficientAssets),
        domain::NestError::Insolvent => error!(StakeError::Insolvent),
        _ => error!(StakeError::InvalidParameter),
    }
}
