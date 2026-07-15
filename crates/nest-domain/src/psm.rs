use crate::{mul_div_down, mul_div_up, NestError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KaminoPrincipalRedemption {
    pub principal_removed: u128,
    pub principal_received: u128,
    pub principal_loss: u128,
}

pub fn account_kamino_principal_redemption(
    deployed_principal: u128,
    ctokens_before: u128,
    redeemed_ctokens: u128,
    usdc_received: u128,
) -> Result<KaminoPrincipalRedemption> {
    if deployed_principal == 0
        || ctokens_before == 0
        || redeemed_ctokens == 0
        || redeemed_ctokens > ctokens_before
    {
        return Err(NestError::InvalidParameter);
    }

    // Round the redeemed basis up so the principal left behind is never
    // overstated relative to the remaining share of the cToken position.
    let principal_removed = if redeemed_ctokens == ctokens_before {
        deployed_principal
    } else {
        mul_div_up(deployed_principal, redeemed_ctokens, ctokens_before)?
    };
    let principal_received = core::cmp::min(usdc_received, principal_removed);
    let principal_loss = principal_removed
        .checked_sub(principal_received)
        .ok_or(NestError::MathOverflow)?;

    Ok(KaminoPrincipalRedemption {
        principal_removed,
        principal_received,
        principal_loss,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KaminoProfitSliceProof {
    pub remaining_value_lower_bound: u128,
}

pub fn verify_kamino_profit_slice(
    redeemed_ctokens: u128,
    remaining_ctokens: u128,
    usdc_received: u128,
    deployed_principal: u128,
) -> Result<KaminoProfitSliceProof> {
    if redeemed_ctokens == 0
        || remaining_ctokens == 0
        || usdc_received == 0
        || deployed_principal == 0
    {
        return Err(NestError::InvalidParameter);
    }

    // The observed redeem rate is a conservative lower bound for the cTokens
    // left in Kamino. If that lower bound cannot cover principal, the slice is
    // not treated as profit.
    let remaining_value_lower_bound =
        mul_div_down(usdc_received, remaining_ctokens, redeemed_ctokens)?;
    if remaining_value_lower_bound < deployed_principal {
        return Err(NestError::PrincipalNotCovered);
    }

    Ok(KaminoProfitSliceProof {
        remaining_value_lower_bound,
    })
}
