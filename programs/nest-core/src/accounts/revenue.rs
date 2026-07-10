#[derive(Accounts)]
pub struct RealizePsmYield<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    pub authority: Signer<'info>,
    #[account(address = protocol.usdc_mint)]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, address = protocol.nusd_mint)]
    pub nusd_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, address = protocol.psm_usdc_vault, token::mint = usdc_mint, token::authority = protocol, token::token_program = usdc_token_program)]
    pub psm_usdc_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, address = protocol.insurance_nusd_vault, token::mint = nusd_mint, token::authority = protocol, token::token_program = nusd_token_program)]
    pub insurance_nusd_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        address = protocol.staker_revenue_nusd_vault,
        token::mint = nusd_mint,
        token::token_program = nusd_token_program,
        constraint = staker_revenue_nusd_vault.owner == protocol.staker_revenue_authority @ CoreError::InvalidParameter
    )]
    pub staker_revenue_nusd_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, address = protocol.protocol_revenue_nusd_vault, token::mint = nusd_mint, token::authority = protocol, token::token_program = nusd_token_program)]
    pub protocol_revenue_nusd_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    /// CHECK: Owned by nest-stake; the helper verifies discriminator and nUSD mint before reading staking assets.
    #[account(owner = NEST_STAKE_PROGRAM_ID)]
    pub staking_state: UncheckedAccount<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub usdc_token_program: Interface<'info, TokenInterface>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct CoverBadDebt<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(address = protocol.usdc_mint)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, address = protocol.psm_usdc_vault, token::mint = usdc_mint, token::authority = protocol, token::token_program = usdc_token_program)]
    pub psm_usdc_vault: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub usdc_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct CheckpointStakerTargetRevenue<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    /// CHECK: Only nest-stake can sign for its canonical state PDA.
    #[account(
        signer,
        owner = NEST_STAKE_PROGRAM_ID,
        address = Pubkey::find_program_address(&[b"staking"], &NEST_STAKE_PROGRAM_ID).0
    )]
    pub staking_state: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct ConsumeStakerRevenue<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    /// CHECK: Only nest-stake can sign for its canonical state PDA.
    #[account(
        signer,
        owner = NEST_STAKE_PROGRAM_ID,
        address = protocol.staker_revenue_authority
    )]
    pub staking_state: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct AbsorbStakingLoss<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    /// CHECK: Only nest-stake can sign for its canonical state PDA.
    #[account(
        signer,
        owner = NEST_STAKE_PROGRAM_ID,
        address = Pubkey::find_program_address(&[b"staking"], &NEST_STAKE_PROGRAM_ID).0
    )]
    pub staking_state: UncheckedAccount<'info>,
}
