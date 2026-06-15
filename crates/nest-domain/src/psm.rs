use crate::{mul_div_down, NestError, Result};

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
