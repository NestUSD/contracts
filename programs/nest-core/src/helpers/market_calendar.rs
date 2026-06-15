fn validate_market_calendar_params(params: &MarketCalendarParams, now: i64) -> Result<()> {
    require!(now >= 0, CoreError::InvalidParameter);
    require!(params.valid_until_ts >= now, CoreError::InvalidParameter);
    require!(
        params.closed_days.len() <= MAX_CALENDAR_CLOSED_DAYS,
        CoreError::InvalidParameter
    );
    require!(
        params.extended_open_seconds < params.extended_close_seconds
            && params.extended_close_seconds <= SECONDS_PER_DAY as u32
            && params.extended_open_seconds <= params.regular_open_seconds
            && params.regular_open_seconds < params.regular_close_seconds
            && params.regular_close_seconds <= params.extended_close_seconds,
        CoreError::InvalidParameter
    );
    for (index, day) in params.closed_days.iter().enumerate() {
        require!(*day >= 0, CoreError::InvalidParameter);
        if index > 0 {
            require!(
                params.closed_days[index - 1] < *day,
                CoreError::InvalidParameter
            );
        }
    }
    Ok(())
}

fn apply_market_calendar_params(
    calendar: &mut MarketCalendar,
    protocol: Pubkey,
    params: MarketCalendarParams,
    bump: u8,
) {
    calendar.protocol = protocol;
    calendar.valid_until_ts = params.valid_until_ts;
    calendar.regular_open_seconds = params.regular_open_seconds;
    calendar.regular_close_seconds = params.regular_close_seconds;
    calendar.extended_open_seconds = params.extended_open_seconds;
    calendar.extended_close_seconds = params.extended_close_seconds;
    calendar.closed_days = params.closed_days;
    calendar.bump = bump;
}

#[cfg(test)]
fn derive_market_state_from_calendar(
    calendar: &MarketCalendar,
    now: i64,
) -> Result<MarketStateAccount> {
    require!(now >= 0, CoreError::InvalidParameter);
    require!(
        now <= calendar.valid_until_ts,
        CoreError::MarketCalendarExpired
    );
    let day = now.div_euclid(SECONDS_PER_DAY);
    if is_weekend_utc_day(day)
        || calendar
            .closed_days
            .iter()
            .any(|closed_day| *closed_day == day)
    {
        return Ok(MarketStateAccount::Closed);
    }
    let seconds = now.rem_euclid(SECONDS_PER_DAY) as u32;
    if seconds >= calendar.regular_open_seconds && seconds < calendar.regular_close_seconds {
        Ok(MarketStateAccount::Regular)
    } else if seconds >= calendar.extended_open_seconds && seconds < calendar.extended_close_seconds
    {
        Ok(MarketStateAccount::Extended)
    } else {
        Ok(MarketStateAccount::Closed)
    }
}

#[cfg(test)]
fn is_weekend_utc_day(day: i64) -> bool {
    let weekday = (day + 4).rem_euclid(7);
    weekday == 0 || weekday == 6
}
