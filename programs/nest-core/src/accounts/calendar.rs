#[derive(Accounts)]
pub struct InitializeMarketCalendar<'info> {
    #[account(seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(init, payer = authority, space = 8 + MarketCalendar::INIT_SPACE, seeds = [b"market_calendar", protocol.key().as_ref()], bump)]
    pub market_calendar: Box<Account<'info, MarketCalendar>>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MutateMarketCalendar<'info> {
    #[account(seeds = [b"protocol"], bump = protocol.bump, has_one = authority)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, has_one = protocol, seeds = [b"market_calendar", protocol.key().as_ref()], bump = market_calendar.bump)]
    pub market_calendar: Box<Account<'info, MarketCalendar>>,
    pub authority: Signer<'info>,
}

