#[derive(Accounts)]
pub struct MutateProtocol<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetStakerYieldParams<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    /// CHECK: Owned by nest-stake; the helper verifies discriminator and nUSD mint before reading staking assets.
    #[account(owner = NEST_STAKE_PROGRAM_ID)]
    pub staking_state: UncheckedAccount<'info>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetPsmKaminoCollateralVault<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(token::authority = protocol, token::token_program = token_program)]
    pub protocol_kamino_collateral_vault: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct SetBuybackConfig<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(address = protocol.usdc_mint)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    #[account(token::mint = usdc_mint, token::token_program = usdc_token_program)]
    pub buyback_usdc_account: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub usdc_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct MutateCollateral<'info> {
    #[account(seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct TripCollateralVaultCoverageBreaker<'info> {
    #[account(seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(address = collateral_config.collateral_mint)]
    pub collateral_mint: InterfaceAccount<'info, Mint>,
    #[account(address = collateral_config.collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub collateral_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(address = collateral_config.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct TripCollateralEmergencyBreaker<'info> {
    #[account(seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(address = protocol.liquidation_authority)]
    pub liquidator: Signer<'info>,
}
