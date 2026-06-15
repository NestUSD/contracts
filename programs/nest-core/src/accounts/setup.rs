#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(init, payer = authority, space = 8 + Protocol::INIT_SPACE, seeds = [b"protocol"], bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    pub nusd_mint: InterfaceAccount<'info, Mint>,
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = usdc_mint, token::authority = protocol, token::token_program = usdc_token_program)]
    pub psm_usdc_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = nusd_mint, token::authority = protocol, token::token_program = nusd_token_program)]
    pub insurance_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = nusd_mint, token::authority = staker_revenue_authority, token::token_program = nusd_token_program)]
    pub staker_revenue_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = nusd_mint, token::authority = protocol, token::token_program = nusd_token_program)]
    pub protocol_revenue_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: Verified against the nest-stake staking PDA before it can own the staker revenue vault.
    pub staker_revenue_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub usdc_token_program: Interface<'info, TokenInterface>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(params: AddCollateralParams)]
pub struct AddCollateral<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(init, payer = authority, space = 8 + CollateralConfig::INIT_SPACE, seeds = [b"collateral", params.collateral_mint.as_ref()], bump)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(address = params.collateral_mint)]
    pub collateral_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub collateral_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub insurance_collateral_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(address = params.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(init, payer = owner, space = 8 + Vault::INIT_SPACE, seeds = [b"vault", owner.key().as_ref(), collateral_config.key().as_ref()], bump)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

