fn staking_vault_nusd_from_account(
    staking_state: &AccountInfo,
    expected_nusd_mint: Pubkey,
) -> Result<u128> {
    // Core cannot deserialize nest-stake accounts directly, so it reads the one
    // field it needs by offset after checking PDA, owner, discriminator, and nUSD mint.
    let (expected_staking_state, _) =
        Pubkey::find_program_address(&[b"staking"], &NEST_STAKE_PROGRAM_ID);
    require_keys_eq!(
        staking_state.key(),
        expected_staking_state,
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        *staking_state.owner,
        NEST_STAKE_PROGRAM_ID,
        CoreError::InvalidParameter
    );
    let data = staking_state.try_borrow_data()?;
    require!(
        data.len() >= STAKING_STATE_STAKING_VAULT_NUSD_OFFSET + 16
            && data.get(0..8) == Some(&STAKING_STATE_DISCRIMINATOR),
        CoreError::InvalidParameter
    );
    require_keys_eq!(
        read_pubkey_at(&data, STAKING_STATE_NUSD_MINT_OFFSET)?,
        expected_nusd_mint,
        CoreError::InvalidParameter
    );
    read_u128_at(&data, STAKING_STATE_STAKING_VAULT_NUSD_OFFSET)
}

fn read_u128_at(data: &[u8], offset: usize) -> Result<u128> {
    let slice = data
        .get(offset..offset + 16)
        .ok_or(error!(CoreError::InvalidParameter))?;
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(slice);
    Ok(u128::from_le_bytes(bytes))
}

fn read_pubkey_at(data: &[u8], offset: usize) -> Result<Pubkey> {
    let slice = data
        .get(offset..offset + 32)
        .ok_or(error!(CoreError::InvalidParameter))?;
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(slice);
    Ok(Pubkey::new_from_array(bytes))
}
