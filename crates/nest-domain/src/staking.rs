use crate::{
    checked_add, checked_sub, mul_div_down, NestError, Result, DEFAULT_COOLDOWN_SECONDS,
    DEFAULT_REVENUE_VESTING_SECONDS,
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
}

impl StakingPool {
    pub fn accounted_assets(&self) -> Result<u128> {
        let after_unvested = checked_sub(self.staking_vault_nusd, self.unvested_revenue)?;
        checked_sub(after_unvested, self.reserved_pending_claims)
    }

    pub fn stake_entry_assets(&self) -> Result<u128> {
        checked_sub(self.staking_vault_nusd, self.reserved_pending_claims)
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

    pub fn stake(&mut self, amount: u128, now: i64) -> Result<u128> {
        if amount == 0 {
            return Err(NestError::InvalidParameter);
        }
        self.sync_vesting(now)?;
        let entry_assets = self.stake_entry_assets()?;
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
        self.sync_vesting(now)?;
        if shares == 0 || shares > self.total_shares {
            return Err(NestError::InvalidParameter);
        }
        Ok(PendingWithdrawal {
            shares,
            request_ts: now,
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
        self.sync_vesting(now)?;
        if pending.shares == 0 || pending.shares > self.total_shares {
            return Err(NestError::InvalidParameter);
        }
        if pending.shares == self.total_shares && self.unvested_revenue > 0 {
            return Err(NestError::CooldownActive);
        }
        let accounted = self.accounted_assets()?;
        let assets = mul_div_down(pending.shares, accounted, self.total_shares)?;
        self.total_shares = checked_sub(self.total_shares, pending.shares)?;
        self.staking_vault_nusd = checked_sub(self.staking_vault_nusd, assets)?;
        Ok(assets)
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
