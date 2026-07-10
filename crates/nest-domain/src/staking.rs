use crate::{
    checked_add, checked_sub, mul_div_down, NestError, Result, DEFAULT_COOLDOWN_SECONDS,
    DEFAULT_REVENUE_VESTING_SECONDS, MIN_INITIAL_STAKE_NUSD, UNSTAKE_CLAIM_WINDOW_SECONDS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StakingPool {
    pub total_shares: u128,
    pub staking_vault_nusd: u128,
    pub unvested_revenue: u128,
    pub reserved_pending_claims: u128,
    pub vesting_start_ts: i64,
    pub vesting_end_ts: i64,
    pub last_vesting_sync_ts: i64,
    pub cooldown_seconds: i64,
    pub revenue_vesting_seconds: i64,
}

impl Default for StakingPool {
    fn default() -> Self {
        Self {
            total_shares: 0,
            staking_vault_nusd: 0,
            unvested_revenue: 0,
            reserved_pending_claims: 0,
            vesting_start_ts: 0,
            vesting_end_ts: 0,
            last_vesting_sync_ts: 0,
            cooldown_seconds: DEFAULT_COOLDOWN_SECONDS,
            revenue_vesting_seconds: DEFAULT_REVENUE_VESTING_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingWithdrawal {
    pub shares: u128,
    pub request_ts: i64,
    /// Zero identifies a withdrawal created before claim windows were introduced.
    pub claim_deadline_ts: i64,
}

impl StakingPool {
    pub fn accounted_assets(&self) -> Result<u128> {
        let after_unvested = checked_sub(self.staking_vault_nusd, self.unvested_revenue)?;
        checked_sub(after_unvested, self.reserved_pending_claims)
    }

    pub fn stake_entry_assets(&self, pending_revenue: u128) -> Result<u128> {
        checked_add(
            checked_sub(self.staking_vault_nusd, self.reserved_pending_claims)?,
            pending_revenue,
        )
    }

    pub fn sync_vesting(&mut self, now: i64) -> Result<u128> {
        require_nonnegative_ts(now)?;
        require_nonnegative_ts(self.last_vesting_sync_ts)?;
        require_nonnegative_ts(self.vesting_end_ts)?;
        if self.unvested_revenue == 0 || now <= self.last_vesting_sync_ts {
            return Ok(0);
        }
        if now >= self.vesting_end_ts {
            let vested = self.unvested_revenue;
            self.unvested_revenue = 0;
            self.last_vesting_sync_ts = now;
            return Ok(vested);
        }
        let elapsed = checked_sub(now as u128, self.last_vesting_sync_ts as u128)?;
        let remaining_duration = checked_sub(
            self.vesting_end_ts as u128,
            self.last_vesting_sync_ts as u128,
        )?;
        let vested = mul_div_down(self.unvested_revenue, elapsed, remaining_duration)?;
        self.unvested_revenue = checked_sub(self.unvested_revenue, vested)?;
        self.last_vesting_sync_ts = now;
        Ok(vested)
    }

    pub fn stake(&mut self, amount: u128, pending_revenue: u128, now: i64) -> Result<u128> {
        if amount == 0 {
            return Err(NestError::InvalidParameter);
        }
        if self.total_shares == 0 && amount < MIN_INITIAL_STAKE_NUSD {
            return Err(NestError::InvalidParameter);
        }
        self.sync_vesting(now)?;
        let entry_assets = self.stake_entry_assets(pending_revenue)?;
        let shares = if self.total_shares == 0 {
            amount
        } else {
            if entry_assets == 0 {
                return Err(NestError::InvalidParameter);
            }
            mul_div_down(amount, self.total_shares, entry_assets)?
        };
        if shares == 0 {
            return Err(NestError::InvalidParameter);
        }
        self.staking_vault_nusd = checked_add(self.staking_vault_nusd, amount)?;
        self.total_shares = checked_add(self.total_shares, shares)?;
        Ok(shares)
    }

    pub fn harvest(&mut self, amount: u128, now: i64) -> Result<()> {
        require_nonnegative_ts(now)?;
        if amount == 0 || self.total_shares == 0 {
            return Err(NestError::InvalidParameter);
        }
        if self.unvested_revenue > 0 && now < self.vesting_end_ts {
            return Err(NestError::InvalidParameter);
        }
        self.sync_vesting(now)?;
        if self.unvested_revenue != 0 {
            return Err(NestError::InvalidParameter);
        }
        self.staking_vault_nusd = checked_add(self.staking_vault_nusd, amount)?;
        self.unvested_revenue = checked_add(self.unvested_revenue, amount)?;
        self.vesting_start_ts = now;
        self.last_vesting_sync_ts = now;
        self.vesting_end_ts = now
            .checked_add(self.revenue_vesting_seconds)
            .ok_or(NestError::MathOverflow)?;
        Ok(())
    }

    pub fn request_unstake(&mut self, shares: u128, now: i64) -> Result<PendingWithdrawal> {
        require_nonnegative_ts(now)?;
        require_nonnegative_ts(self.cooldown_seconds)?;
        self.sync_vesting(now)?;
        if shares == 0 || shares > self.total_shares {
            return Err(NestError::InvalidParameter);
        }
        let claim_deadline_ts = now
            .checked_add(self.cooldown_seconds)
            .and_then(|value| value.checked_add(UNSTAKE_CLAIM_WINDOW_SECONDS))
            .ok_or(NestError::MathOverflow)?;
        Ok(PendingWithdrawal {
            shares,
            request_ts: now,
            claim_deadline_ts,
        })
    }

    pub fn complete_unstake(&mut self, pending: PendingWithdrawal, now: i64) -> Result<u128> {
        require_nonnegative_ts(now)?;
        require_nonnegative_ts(pending.request_ts)?;
        require_nonnegative_ts(self.cooldown_seconds)?;
        let unlock_ts = pending
            .request_ts
            .checked_add(self.cooldown_seconds)
            .ok_or(NestError::MathOverflow)?;
        if now < unlock_ts {
            return Err(NestError::CooldownActive);
        }
        if pending.claim_deadline_ts != 0 {
            require_nonnegative_ts(pending.claim_deadline_ts)?;
            if now > pending.claim_deadline_ts {
                return Err(NestError::ClaimWindowExpired);
            }
        }
        self.sync_vesting(now)?;
        if pending.shares == 0 || pending.shares > self.total_shares {
            return Err(NestError::InvalidParameter);
        }
        let redeemable_assets = checked_sub(self.staking_vault_nusd, self.reserved_pending_claims)?;
        let assets = mul_div_down(pending.shares, redeemable_assets, self.total_shares)?;
        let unvested_redeemed = if pending.shares == self.total_shares {
            self.unvested_revenue
        } else {
            mul_div_down(pending.shares, self.unvested_revenue, self.total_shares)?
        };
        self.total_shares = checked_sub(self.total_shares, pending.shares)?;
        self.staking_vault_nusd = checked_sub(self.staking_vault_nusd, assets)?;
        self.unvested_revenue = checked_sub(self.unvested_revenue, unvested_redeemed)?;
        Ok(assets)
    }

    pub fn cancel_expired_unstake(&self, pending: PendingWithdrawal, now: i64) -> Result<u128> {
        require_nonnegative_ts(now)?;
        require_nonnegative_ts(pending.request_ts)?;
        if pending.shares == 0 || pending.shares > self.total_shares {
            return Err(NestError::InvalidParameter);
        }
        if pending.claim_deadline_ts == 0 {
            return Ok(pending.shares);
        }
        require_nonnegative_ts(pending.claim_deadline_ts)?;
        if now <= pending.claim_deadline_ts {
            return Err(NestError::ClaimWindowActive);
        }
        Ok(pending.shares)
    }

    pub fn realize_loss(&mut self, amount: u128) -> Result<()> {
        if amount > self.staking_vault_nusd {
            return Err(NestError::Insolvent);
        }
        self.staking_vault_nusd = checked_sub(self.staking_vault_nusd, amount)?;
        if self.unvested_revenue > self.staking_vault_nusd {
            self.unvested_revenue = self.staking_vault_nusd;
        }
        Ok(())
    }
}

fn require_nonnegative_ts(value: i64) -> Result<()> {
    if value < 0 {
        return Err(NestError::InvalidParameter);
    }
    Ok(())
}
