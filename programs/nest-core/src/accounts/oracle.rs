#[derive(Accounts)]
pub struct RefreshLazerOracle<'info> {
    #[account(seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(init_if_needed, payer = payer, space = 8 + OracleSnapshot::INIT_SPACE, seeds = [b"oracle", collateral_config.key().as_ref()], bump)]
    pub oracle: Box<Account<'info, OracleSnapshot>>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Address constrained to the Pyth Lazer verifier program.
    #[account(address = pyth_lazer_solana_contract::ID)]
    pub pyth_lazer_program: AccountInfo<'info>,
    #[account(address = pyth_lazer_solana_contract::STORAGE_ID)]
    pub pyth_lazer_storage: Account<'info, pyth_lazer_solana_contract::Storage>,
    /// CHECK: Pyth Lazer verifies this against storage.has_one = treasury.
    #[account(mut)]
    pub pyth_lazer_treasury: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: The sysvar loader validates this account, and the address is constrained here.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RefreshSignedOracle<'info> {
    #[account(seeds = [b"protocol"], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(has_one = protocol)]
    pub collateral_config: Box<Account<'info, CollateralConfig>>,
    #[account(
        seeds = [b"nest_price_signer", protocol.key().as_ref()],
        bump = nest_price_signer.bump,
        has_one = protocol
    )]
    pub nest_price_signer: Box<Account<'info, NestPriceSignerConfig>>,
    #[account(init_if_needed, payer = payer, space = 8 + OracleSnapshot::INIT_SPACE, seeds = [b"oracle", collateral_config.key().as_ref()], bump)]
    pub oracle: Box<Account<'info, OracleSnapshot>>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: The sysvar loader validates this account, and the address is constrained here.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
}
