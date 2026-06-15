#[derive(Accounts)]
pub struct MutateVault<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = owner, has_one = collateral_config)]
    pub vault: Box<Account<'info, Vault>>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = owner, has_one = collateral_config)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(address = collateral_config.collateral_mint)]
    pub collateral_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = collateral_mint, token::authority = owner, token::token_program = collateral_token_program)]
    pub owner_collateral_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, address = collateral_config.collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub collateral_vault: InterfaceAccount<'info, TokenAccount>,
    pub owner: Signer<'info>,
    #[account(address = collateral_config.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = owner, has_one = collateral_config)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(address = collateral_config.collateral_mint)]
    pub collateral_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, address = collateral_config.collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub collateral_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = collateral_mint, token::authority = owner, token::token_program = collateral_token_program)]
    pub owner_collateral_account: InterfaceAccount<'info, TokenAccount>,
    pub xstock_price_update: Box<Account<'info, PriceUpdateV2>>,
    pub owner: Signer<'info>,
    #[account(address = collateral_config.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct WithdrawWithOracle<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = owner, has_one = collateral_config)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(
        has_one = protocol,
        has_one = collateral_config,
        seeds = [b"oracle", collateral_config.key().as_ref()],
        bump = oracle.bump
    )]
    pub oracle: Box<Account<'info, OracleSnapshot>>,
    #[account(address = collateral_config.collateral_mint)]
    pub collateral_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, address = collateral_config.collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub collateral_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = collateral_mint, token::authority = owner, token::token_program = collateral_token_program)]
    pub owner_collateral_account: InterfaceAccount<'info, TokenAccount>,
    pub owner: Signer<'info>,
    #[account(address = collateral_config.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct MintNusd<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = owner, has_one = collateral_config)]
    pub vault: Box<Account<'info, Vault>>,
    pub xstock_price_update: Box<Account<'info, PriceUpdateV2>>,
    #[account(mut, address = protocol.nusd_mint)]
    pub nusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = nusd_mint, token::authority = owner, token::token_program = nusd_token_program)]
    pub owner_nusd_account: InterfaceAccount<'info, TokenAccount>,
    pub owner: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct MintNusdWithOracle<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = owner, has_one = collateral_config)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(
        has_one = protocol,
        has_one = collateral_config,
        seeds = [b"oracle", collateral_config.key().as_ref()],
        bump = oracle.bump
    )]
    pub oracle: Box<Account<'info, OracleSnapshot>>,
    #[account(mut, address = protocol.nusd_mint)]
    pub nusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = nusd_mint, token::authority = owner, token::token_program = nusd_token_program)]
    pub owner_nusd_account: InterfaceAccount<'info, TokenAccount>,
    pub owner: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct RepayNusd<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = owner, has_one = collateral_config)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, address = protocol.nusd_mint)]
    pub nusd_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, token::mint = nusd_mint, token::authority = owner, token::token_program = nusd_token_program)]
    pub owner_nusd_account: Box<InterfaceAccount<'info, TokenAccount>>,
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
    pub owner: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}
