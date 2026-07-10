#[derive(Accounts)]
pub struct InitializeStaking<'info> {
    #[account(init, payer = authority, space = 8 + StakingState::INIT_SPACE, seeds = [b"staking"], bump)]
    pub staking_state: Box<Account<'info, StakingState>>,
    /// CHECK: The initializer validates the canonical core PDA, owner, discriminator, and nUSD mint.
    #[account(owner = NEST_CORE_PROGRAM_ID)]
    pub protocol: UncheckedAccount<'info>,
    pub nusd_mint: InterfaceAccount<'info, Mint>,
    pub snusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = nusd_mint, token::authority = staking_state, token::token_program = nusd_token_program)]
    pub staking_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = nusd_mint, token::authority = staking_state, token::token_program = nusd_token_program)]
    pub revenue_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        constraint = program.programdata_address()? == Some(program_data.key()) @ StakeError::Unauthorized
    )]
    pub program: Program<'info, crate::program::NestStake>,
    #[account(
        constraint = program_data.upgrade_authority_address == Some(authority.key()) @ StakeError::Unauthorized
    )]
    pub program_data: Account<'info, ProgramData>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub snusd_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MutateStake<'info> {
    #[account(mut, seeds = [b"staking"], bump = staking_state.bump, has_one = authority)]
    pub staking_state: Box<Account<'info, StakingState>>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct RealizeLoss<'info> {
    #[account(mut, seeds = [b"staking"], bump = staking_state.bump, has_one = authority)]
    pub staking_state: Box<Account<'info, StakingState>>,
    #[account(mut, address = staking_state.nusd_mint)]
    pub nusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, address = staking_state.staking_nusd_vault, token::mint = nusd_mint, token::authority = staking_state, token::token_program = nusd_token_program)]
    pub staking_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut, seeds = [b"staking"], bump = staking_state.bump)]
    pub staking_state: Box<Account<'info, StakingState>>,
    /// CHECK: Owned by nest-core; the helper verifies the discriminator and nUSD mint before reading economics.
    #[account(owner = NEST_CORE_PROGRAM_ID)]
    pub protocol: UncheckedAccount<'info>,
    #[account(mut, address = staking_state.nusd_mint)]
    pub nusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, address = staking_state.snusd_mint)]
    pub snusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = nusd_mint, token::authority = owner, token::token_program = nusd_token_program)]
    pub owner_nusd_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, address = staking_state.staking_nusd_vault, token::mint = nusd_mint, token::authority = staking_state, token::token_program = nusd_token_program)]
    pub staking_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = snusd_mint, token::authority = owner, token::token_program = snusd_token_program)]
    pub owner_snusd_account: InterfaceAccount<'info, TokenAccount>,
    #[account(address = staking_state.revenue_nusd_vault, token::mint = nusd_mint, token::authority = staking_state, token::token_program = nusd_token_program)]
    pub revenue_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    pub owner: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub snusd_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Harvest<'info> {
    #[account(mut, seeds = [b"staking"], bump = staking_state.bump, has_one = authority)]
    pub staking_state: Box<Account<'info, StakingState>>,
    #[account(mut, address = staking_state.nusd_mint)]
    pub nusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, address = staking_state.revenue_nusd_vault, token::mint = nusd_mint, token::authority = staking_state, token::token_program = nusd_token_program)]
    pub revenue_nusd_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, address = staking_state.staking_nusd_vault, token::mint = nusd_mint, token::authority = staking_state, token::token_program = nusd_token_program)]
    pub staking_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct RequestUnstake<'info> {
    #[account(mut, seeds = [b"staking"], bump = staking_state.bump)]
    pub staking_state: Box<Account<'info, StakingState>>,
    #[account(mut, address = staking_state.snusd_mint)]
    pub snusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = snusd_mint, token::authority = owner, token::token_program = snusd_token_program)]
    pub owner_snusd_account: InterfaceAccount<'info, TokenAccount>,
    #[account(init, payer = owner, space = 8 + PendingWithdrawalAccount::INIT_SPACE, seeds = [b"pending", owner.key().as_ref(), staking_state.key().as_ref()], bump)]
    pub pending_withdrawal: Box<Account<'info, PendingWithdrawalAccount>>,
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub snusd_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CompleteUnstake<'info> {
    #[account(mut, seeds = [b"staking"], bump = staking_state.bump, has_one = authority)]
    pub staking_state: Box<Account<'info, StakingState>>,
    #[account(mut, has_one = owner, has_one = staking_state, close = owner)]
    pub pending_withdrawal: Box<Account<'info, PendingWithdrawalAccount>>,
    #[account(mut, address = staking_state.nusd_mint)]
    pub nusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, address = staking_state.staking_nusd_vault, token::mint = nusd_mint, token::authority = staking_state, token::token_program = nusd_token_program)]
    pub staking_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = nusd_mint, token::authority = owner, token::token_program = nusd_token_program)]
    pub owner_nusd_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub authority: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}
