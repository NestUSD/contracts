#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = collateral_config, seeds = [b"vault", vault.owner.as_ref(), collateral_config.key().as_ref()], bump = vault.bump)]
    pub vault: Box<Account<'info, Vault>>,
    pub xstock_price_update: Box<Account<'info, PriceUpdateV2>>,
    #[account(address = collateral_config.collateral_mint)]
    pub collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, address = collateral_config.collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, token::mint = collateral_mint, token::authority = liquidator, token::token_program = collateral_token_program)]
    pub liquidator_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, address = collateral_config.insurance_collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub insurance_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, address = protocol.nusd_mint)]
    pub nusd_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, token::mint = nusd_mint, token::authority = liquidator, token::token_program = nusd_token_program)]
    pub liquidator_nusd_account: Box<InterfaceAccount<'info, TokenAccount>>,
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
    pub liquidator: Signer<'info>,
    #[account(address = collateral_config.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct LiquidateWithOracle<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = collateral_config, seeds = [b"vault", vault.owner.as_ref(), collateral_config.key().as_ref()], bump = vault.bump)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(
        has_one = protocol,
        has_one = collateral_config,
        seeds = [b"oracle", collateral_config.key().as_ref()],
        bump = oracle.bump
    )]
    pub oracle: Box<Account<'info, OracleSnapshot>>,
    #[account(address = collateral_config.collateral_mint)]
    pub collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, address = collateral_config.collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, token::mint = collateral_mint, token::authority = liquidator, token::token_program = collateral_token_program)]
    pub liquidator_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, address = collateral_config.insurance_collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub insurance_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, address = protocol.nusd_mint)]
    pub nusd_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, token::mint = nusd_mint, token::authority = liquidator, token::token_program = nusd_token_program)]
    pub liquidator_nusd_account: Box<InterfaceAccount<'info, TokenAccount>>,
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
    pub liquidator: Signer<'info>,
    #[account(address = collateral_config.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct StartLiquidation<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = collateral_config, seeds = [b"vault", vault.owner.as_ref(), collateral_config.key().as_ref()], bump = vault.bump)]
    pub vault: Box<Account<'info, Vault>>,
    pub xstock_price_update: Box<Account<'info, PriceUpdateV2>>,
    #[account(address = collateral_config.collateral_mint)]
    pub collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, address = collateral_config.collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, token::mint = collateral_mint, token::authority = liquidator, token::token_program = collateral_token_program)]
    pub liquidator_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        init,
        payer = liquidator,
        space = 8 + LiquidationReceipt::INIT_SPACE,
        seeds = [b"liquidation", vault.key().as_ref()],
        bump
    )]
    pub liquidation_receipt: Box<Account<'info, LiquidationReceipt>>,
    #[account(mut)]
    pub liquidator: Signer<'info>,
    #[account(address = collateral_config.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StartLiquidationWithOracle<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(mut, has_one = collateral_config, seeds = [b"vault", vault.owner.as_ref(), collateral_config.key().as_ref()], bump = vault.bump)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(
        has_one = protocol,
        has_one = collateral_config,
        seeds = [b"oracle", collateral_config.key().as_ref()],
        bump = oracle.bump
    )]
    pub oracle: Box<Account<'info, OracleSnapshot>>,
    #[account(address = collateral_config.collateral_mint)]
    pub collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, address = collateral_config.collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, token::mint = collateral_mint, token::authority = liquidator, token::token_program = collateral_token_program)]
    pub liquidator_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        init,
        payer = liquidator,
        space = 8 + LiquidationReceipt::INIT_SPACE,
        seeds = [b"liquidation", vault.key().as_ref()],
        bump
    )]
    pub liquidation_receipt: Box<Account<'info, LiquidationReceipt>>,
    #[account(mut)]
    pub liquidator: Signer<'info>,
    #[account(address = collateral_config.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SettleLiquidationProceeds<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(
        mut,
        close = liquidator,
        has_one = protocol,
        has_one = liquidator,
        seeds = [b"liquidation", liquidation_receipt.vault.as_ref()],
        bump = liquidation_receipt.bump
    )]
    pub liquidation_receipt: Box<Account<'info, LiquidationReceipt>>,
    #[account(address = protocol.usdc_mint)]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, address = protocol.nusd_mint)]
    pub nusd_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, token::mint = usdc_mint, token::authority = liquidator, token::token_program = usdc_token_program)]
    pub liquidator_usdc_account: Box<InterfaceAccount<'info, TokenAccount>>,
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
    #[account(mut)]
    pub liquidator: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub usdc_token_program: Interface<'info, TokenInterface>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct WithdrawInsuranceCollateral<'info> {
    #[account(seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(address = collateral_config.collateral_mint)]
    pub collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, address = collateral_config.insurance_collateral_vault, token::mint = collateral_mint, token::authority = protocol, token::token_program = collateral_token_program)]
    pub insurance_collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, token::mint = collateral_mint, token::authority = authority, token::token_program = collateral_token_program)]
    pub authority_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,
    pub authority: Signer<'info>,
    #[account(address = collateral_config.token_program)]
    pub collateral_token_program: Interface<'info, TokenInterface>,
}
