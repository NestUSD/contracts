#[derive(Accounts)]
pub struct RealizePsmKaminoYield<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(address = protocol.authority @ CoreError::Unauthorized)]
    pub authority: Signer<'info>,
    #[account(mut, address = protocol.usdc_mint)]
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
    #[account(mut)]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, address = protocol.psm_kamino_collateral_vault, token::mint = reserve_collateral_mint, token::authority = protocol, token::token_program = usdc_token_program)]
    pub protocol_kamino_collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    /// CHECK: Raw Kamino reserve; `assert_kamino_psm_yield_accounts` verifies the configured Kamino program owns it.
    #[account(mut)]
    pub reserve: UncheckedAccount<'info>,
    /// CHECK: Raw Kamino market; `assert_kamino_psm_yield_accounts` verifies the configured Kamino program owns it.
    pub lending_market: UncheckedAccount<'info>,
    /// CHECK: Derived from `lending_market` by `assert_kamino_psm_yield_accounts`.
    pub lending_market_authority: UncheckedAccount<'info>,
    /// CHECK: Read from the reserve state by `assert_kamino_psm_yield_accounts`.
    #[account(mut)]
    pub reserve_liquidity_supply: UncheckedAccount<'info>,
    /// CHECK: Passed to Kamino refresh as a real oracle account or the disabled optional-account sentinel.
    pub pyth_oracle: UncheckedAccount<'info>,
    /// CHECK: Passed to Kamino refresh as a real oracle account or the disabled optional-account sentinel.
    pub switchboard_price_oracle: UncheckedAccount<'info>,
    /// CHECK: Passed to Kamino refresh as a real oracle account or the disabled optional-account sentinel.
    pub switchboard_twap_oracle: UncheckedAccount<'info>,
    /// CHECK: Passed to Kamino refresh as a real Scope account or the disabled optional-account sentinel.
    pub scope_prices: UncheckedAccount<'info>,
    /// CHECK: Constrained to `Protocol.kamino_program_id` before it is invoked as the CPI program.
    #[account(address = protocol.kamino_program_id)]
    pub kamino_program: UncheckedAccount<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub usdc_token_program: Interface<'info, TokenInterface>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
    /// CHECK: Constrained to the instructions sysvar address required by Kamino.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}
