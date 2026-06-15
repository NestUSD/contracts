#[derive(Accounts)]
pub struct ReleaseBuybackSurplus<'info> {
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump, has_one = buyback_authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(address = protocol.usdc_mint)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, address = protocol.nusd_mint)]
    pub nusd_mint: InterfaceAccount<'info, Mint>,
    #[account(mut, address = protocol.psm_usdc_vault, token::mint = usdc_mint, token::authority = protocol, token::token_program = usdc_token_program)]
    pub psm_usdc_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, address = protocol.protocol_revenue_nusd_vault, token::mint = nusd_mint, token::authority = protocol, token::token_program = nusd_token_program)]
    pub protocol_revenue_nusd_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, address = protocol.buyback_usdc_account, token::mint = usdc_mint, token::authority = buyback_authority, token::token_program = usdc_token_program)]
    pub buyback_usdc_account: InterfaceAccount<'info, TokenAccount>,
    pub buyback_authority: Signer<'info>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub usdc_token_program: Interface<'info, TokenInterface>,
    #[account(address = SPL_TOKEN_PROGRAM_ID)]
    pub nusd_token_program: Interface<'info, TokenInterface>,
}
