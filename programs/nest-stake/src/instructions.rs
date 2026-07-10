use crate::*;

pub fn initialize_staking(
    ctx: Context<InitializeStaking>,
    params: InitializeStakingParams,
) -> Result<()> {
    let (expected_nusd_mint_authority, _) =
        Pubkey::find_program_address(&[b"protocol"], &NEST_CORE_PROGRAM_ID);
    require!(
        params.cooldown_seconds == domain::DEFAULT_COOLDOWN_SECONDS
            && params.revenue_vesting_seconds == domain::DEFAULT_REVENUE_VESTING_SECONDS
            && ctx.accounts.nusd_mint.decimals == STABLECOIN_DECIMALS
            && ctx.accounts.snusd_mint.decimals == STABLECOIN_DECIMALS
            && ctx.accounts.snusd_mint.supply == 0
            && ctx.accounts.nusd_token_program.key() == SPL_TOKEN_PROGRAM_ID
            && ctx.accounts.snusd_token_program.key() == SPL_TOKEN_PROGRAM_ID
            && ctx.accounts.nusd_mint.mint_authority == COption::Some(expected_nusd_mint_authority)
            && ctx.accounts.snusd_mint.mint_authority
                == COption::Some(ctx.accounts.staking_state.key())
            && ctx.accounts.nusd_mint.freeze_authority == COption::None
            && ctx.accounts.snusd_mint.freeze_authority == COption::None
            && ctx.accounts.staking_nusd_vault.key() != ctx.accounts.revenue_nusd_vault.key()
            && ctx.accounts.staking_nusd_vault.amount == 0
            && ctx.accounts.revenue_nusd_vault.amount == 0,
        StakeError::InvalidParameter
    );
    require_token_account_unencumbered(&ctx.accounts.staking_nusd_vault)?;
    require_token_account_unencumbered(&ctx.accounts.revenue_nusd_vault)?;
    require_keys_eq!(
        *ctx.accounts.nusd_mint.to_account_info().owner,
        ctx.accounts.nusd_token_program.key(),
        StakeError::InvalidParameter
    );
    require_keys_eq!(
        *ctx.accounts.snusd_mint.to_account_info().owner,
        ctx.accounts.snusd_token_program.key(),
        StakeError::InvalidParameter
    );
    let state = &mut ctx.accounts.staking_state;
    state.authority = ctx.accounts.authority.key();
    state.nusd_mint = ctx.accounts.nusd_mint.key();
    state.snusd_mint = ctx.accounts.snusd_mint.key();
    state.staking_nusd_vault = ctx.accounts.staking_nusd_vault.key();
    state.revenue_nusd_vault = ctx.accounts.revenue_nusd_vault.key();
    state.revenue_baseline_nusd = 0;
    state.total_shares = 0;
    state.total_user_shares = 0;
    state.staking_vault_nusd = 0;
    state.realized_loss_nusd = 0;
    state.unvested_revenue = 0;
    state.reserved_pending_claims = 0;
    state.vesting_start_ts = 0;
    state.vesting_end_ts = 0;
    state.last_vesting_sync_ts = 0;
    state.cooldown_seconds = params.cooldown_seconds;
    state.revenue_vesting_seconds = params.revenue_vesting_seconds;
    state.paused = false;
    state.bump = ctx.bumps.staking_state;
    Ok(())
}

pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
    require!(amount > 0, StakeError::InvalidParameter);
    require!(!ctx.accounts.staking_state.paused, StakeError::Paused);
    let was_empty = ctx.accounts.staking_state.total_shares == 0;
    let pending_revenue = if was_empty {
        0
    } else {
        (ctx.accounts.revenue_nusd_vault.amount as u128)
            .checked_sub(ctx.accounts.staking_state.revenue_baseline_nusd)
            .ok_or(error!(StakeError::InsufficientAssets))?
    };
    let mut pool = domain_pool(&ctx.accounts.staking_state);
    let now = Clock::get()?.unix_timestamp;
    let minted_shares = pool
        .stake(amount as u128, pending_revenue, now)
        .map_err(map_stake_error)?;
    if let Some(capacity) = staking_capacity_from_protocol(
        &ctx.accounts.protocol.to_account_info(),
        ctx.accounts.staking_state.nusd_mint,
    )? {
        require!(
            pool.staking_vault_nusd <= capacity,
            StakeError::StakeCapacityExceeded
        );
    }
    apply_pool(&mut ctx.accounts.staking_state, pool);
    if was_empty {
        ctx.accounts.staking_state.revenue_baseline_nusd =
            ctx.accounts.revenue_nusd_vault.amount as u128;
    }
    ctx.accounts.staking_state.total_user_shares = ctx
        .accounts
        .staking_state
        .total_user_shares
        .checked_add(minted_shares)
        .ok_or(error!(StakeError::MathOverflow))?;
    let owner_nusd_before = ctx.accounts.owner_nusd_account.amount;
    let staking_vault_before = ctx.accounts.staking_nusd_vault.amount;
    token_transfer_checked(
        ctx.accounts.nusd_token_program.to_account_info(),
        ctx.accounts.owner_nusd_account.to_account_info(),
        ctx.accounts.nusd_mint.to_account_info(),
        ctx.accounts.staking_nusd_vault.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        ctx.accounts.nusd_mint.decimals,
        &[],
    )?;
    ctx.accounts.owner_nusd_account.reload()?;
    ctx.accounts.staking_nusd_vault.reload()?;
    require_token_account_decrease(
        owner_nusd_before,
        ctx.accounts.owner_nusd_account.amount,
        amount,
    )?;
    require_token_account_increase(
        staking_vault_before,
        ctx.accounts.staking_nusd_vault.amount,
        amount,
    )?;
    require_recorded_staking_vault_covered(
        ctx.accounts.staking_nusd_vault.amount,
        ctx.accounts.staking_state.staking_vault_nusd,
    )?;
    let minted_shares_u64 = u128_to_u64(minted_shares)?;
    let owner_snusd_before = ctx.accounts.owner_snusd_account.amount;
    let snusd_supply_before = ctx.accounts.snusd_mint.supply;
    let bump = [ctx.accounts.staking_state.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"staking", &bump]];
    token_mint_to_checked(
        ctx.accounts.snusd_token_program.to_account_info(),
        ctx.accounts.snusd_mint.to_account_info(),
        ctx.accounts.owner_snusd_account.to_account_info(),
        ctx.accounts.staking_state.to_account_info(),
        minted_shares_u64,
        ctx.accounts.snusd_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.owner_snusd_account.reload()?;
    ctx.accounts.snusd_mint.reload()?;
    require_token_account_increase(
        owner_snusd_before,
        ctx.accounts.owner_snusd_account.amount,
        minted_shares_u64,
    )?;
    require_mint_supply_increase(
        snusd_supply_before,
        ctx.accounts.snusd_mint.supply,
        minted_shares_u64,
    )?;
    Ok(())
}

pub fn harvest(ctx: Context<Harvest>, amount: u64) -> Result<()> {
    assert_authority(&ctx.accounts.staking_state, &ctx.accounts.authority)?;
    require!(amount > 0, StakeError::InvalidParameter);
    require!(
        ctx.accounts.staking_state.total_shares > 0,
        StakeError::InvalidParameter
    );
    let amount_u128 = amount as u128;
    let revenue_balance = ctx.accounts.revenue_nusd_account.amount as u128;
    let harvestable = revenue_balance
        .checked_sub(ctx.accounts.staking_state.revenue_baseline_nusd)
        .ok_or(error!(StakeError::InsufficientAssets))?;
    require!(amount_u128 <= harvestable, StakeError::InsufficientAssets);
    let mut pool = domain_pool(&ctx.accounts.staking_state);
    let now = Clock::get()?.unix_timestamp;
    pool.harvest(amount_u128, now).map_err(map_stake_error)?;
    apply_pool(&mut ctx.accounts.staking_state, pool);
    let bump = [ctx.accounts.staking_state.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"staking", &bump]];
    let revenue_vault_before = ctx.accounts.revenue_nusd_account.amount;
    let staking_vault_before = ctx.accounts.staking_nusd_vault.amount;
    token_transfer_checked(
        ctx.accounts.nusd_token_program.to_account_info(),
        ctx.accounts.revenue_nusd_account.to_account_info(),
        ctx.accounts.nusd_mint.to_account_info(),
        ctx.accounts.staking_nusd_vault.to_account_info(),
        ctx.accounts.staking_state.to_account_info(),
        amount,
        ctx.accounts.nusd_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.revenue_nusd_account.reload()?;
    ctx.accounts.staking_nusd_vault.reload()?;
    require_token_account_decrease(
        revenue_vault_before,
        ctx.accounts.revenue_nusd_account.amount,
        amount,
    )?;
    require_token_account_increase(
        staking_vault_before,
        ctx.accounts.staking_nusd_vault.amount,
        amount,
    )?;
    require_recorded_staking_vault_covered(
        ctx.accounts.staking_nusd_vault.amount,
        ctx.accounts.staking_state.staking_vault_nusd,
    )?;
    Ok(())
}

pub fn request_unstake(ctx: Context<RequestUnstake>, shares: u64) -> Result<()> {
    require!(shares > 0, StakeError::InvalidParameter);
    require!(!ctx.accounts.staking_state.paused, StakeError::Paused);
    let shares_u128 = shares as u128;
    require!(
        shares_u128 <= ctx.accounts.staking_state.total_user_shares,
        StakeError::InvalidParameter
    );
    let mut pool = domain_pool(&ctx.accounts.staking_state);
    let now = Clock::get()?.unix_timestamp;
    let pending = pool
        .request_unstake(shares_u128, now)
        .map_err(map_stake_error)?;
    apply_pool(&mut ctx.accounts.staking_state, pool);
    ctx.accounts.staking_state.total_user_shares = ctx
        .accounts
        .staking_state
        .total_user_shares
        .checked_sub(pending.shares)
        .ok_or(error!(StakeError::MathOverflow))?;

    let account = &mut ctx.accounts.pending_withdrawal;
    account.owner = ctx.accounts.owner.key();
    account.staking_state = ctx.accounts.staking_state.key();
    account.shares = pending.shares;
    account.request_ts = pending.request_ts;
    account.completed = false;
    account.bump = ctx.bumps.pending_withdrawal;
    let owner_snusd_before = ctx.accounts.owner_snusd_account.amount;
    let snusd_supply_before = ctx.accounts.snusd_mint.supply;
    token_burn_checked(
        ctx.accounts.snusd_token_program.to_account_info(),
        ctx.accounts.snusd_mint.to_account_info(),
        ctx.accounts.owner_snusd_account.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        shares,
        ctx.accounts.snusd_mint.decimals,
        &[],
    )?;
    ctx.accounts.owner_snusd_account.reload()?;
    ctx.accounts.snusd_mint.reload()?;
    require_token_account_decrease(
        owner_snusd_before,
        ctx.accounts.owner_snusd_account.amount,
        shares,
    )?;
    require_mint_supply_decrease(snusd_supply_before, ctx.accounts.snusd_mint.supply, shares)?;
    Ok(())
}

pub fn complete_unstake(ctx: Context<CompleteUnstake>) -> Result<()> {
    assert_authority(&ctx.accounts.staking_state, &ctx.accounts.authority)?;
    require!(!ctx.accounts.staking_state.paused, StakeError::Paused);
    require!(
        !ctx.accounts.pending_withdrawal.completed,
        StakeError::AlreadyCompleted
    );
    let mut pool = domain_pool(&ctx.accounts.staking_state);
    let now = Clock::get()?.unix_timestamp;
    let assets = pool
        .complete_unstake(
            domain::PendingWithdrawal {
                shares: ctx.accounts.pending_withdrawal.shares,
                request_ts: ctx.accounts.pending_withdrawal.request_ts,
            },
            now,
        )
        .map_err(map_stake_error)?;
    apply_pool(&mut ctx.accounts.staking_state, pool);
    ctx.accounts.pending_withdrawal.assets_redeemed = assets;
    ctx.accounts.pending_withdrawal.completed = true;
    let bump = [ctx.accounts.staking_state.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"staking", &bump]];
    let redeemed_assets = u128_to_u64(assets)?;
    let owner_nusd_before = ctx.accounts.owner_nusd_account.amount;
    let staking_vault_before = ctx.accounts.staking_nusd_vault.amount;
    token_transfer_checked(
        ctx.accounts.nusd_token_program.to_account_info(),
        ctx.accounts.staking_nusd_vault.to_account_info(),
        ctx.accounts.nusd_mint.to_account_info(),
        ctx.accounts.owner_nusd_account.to_account_info(),
        ctx.accounts.staking_state.to_account_info(),
        redeemed_assets,
        ctx.accounts.nusd_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.staking_nusd_vault.reload()?;
    ctx.accounts.owner_nusd_account.reload()?;
    require_token_account_decrease(
        staking_vault_before,
        ctx.accounts.staking_nusd_vault.amount,
        redeemed_assets,
    )?;
    require_token_account_increase(
        owner_nusd_before,
        ctx.accounts.owner_nusd_account.amount,
        redeemed_assets,
    )?;
    require_recorded_staking_vault_covered(
        ctx.accounts.staking_nusd_vault.amount,
        ctx.accounts.staking_state.staking_vault_nusd,
    )?;
    Ok(())
}

pub fn realize_loss(ctx: Context<RealizeLoss>, amount: u64) -> Result<()> {
    require!(amount > 0, StakeError::InvalidParameter);
    assert_authority(&ctx.accounts.staking_state, &ctx.accounts.authority)?;
    let mut pool = domain_pool(&ctx.accounts.staking_state);
    pool.realize_loss(amount as u128).map_err(map_stake_error)?;
    apply_pool(&mut ctx.accounts.staking_state, pool);
    ctx.accounts.staking_state.realized_loss_nusd = ctx
        .accounts
        .staking_state
        .realized_loss_nusd
        .checked_add(amount as u128)
        .ok_or(error!(StakeError::MathOverflow))?;
    let bump = [ctx.accounts.staking_state.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[b"staking", &bump]];
    let staking_vault_before = ctx.accounts.staking_nusd_vault.amount;
    let nusd_supply_before = ctx.accounts.nusd_mint.supply;
    token_burn_checked(
        ctx.accounts.nusd_token_program.to_account_info(),
        ctx.accounts.nusd_mint.to_account_info(),
        ctx.accounts.staking_nusd_vault.to_account_info(),
        ctx.accounts.staking_state.to_account_info(),
        amount,
        ctx.accounts.nusd_mint.decimals,
        signer_seeds,
    )?;
    ctx.accounts.staking_nusd_vault.reload()?;
    ctx.accounts.nusd_mint.reload()?;
    require_token_account_decrease(
        staking_vault_before,
        ctx.accounts.staking_nusd_vault.amount,
        amount,
    )?;
    require_mint_supply_decrease(nusd_supply_before, ctx.accounts.nusd_mint.supply, amount)?;
    require_recorded_staking_vault_covered(
        ctx.accounts.staking_nusd_vault.amount,
        ctx.accounts.staking_state.staking_vault_nusd,
    )?;
    Ok(())
}

pub fn set_paused(ctx: Context<MutateStake>, paused: bool) -> Result<()> {
    assert_authority(&ctx.accounts.staking_state, &ctx.accounts.authority)?;
    ctx.accounts.staking_state.paused = paused;
    Ok(())
}

pub fn set_staking_authority(ctx: Context<MutateStake>, authority: Pubkey) -> Result<()> {
    assert_authority(&ctx.accounts.staking_state, &ctx.accounts.authority)?;
    require_keys_neq!(authority, Pubkey::default(), StakeError::InvalidParameter);
    ctx.accounts.staking_state.authority = authority;
    Ok(())
}
