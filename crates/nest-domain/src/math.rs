use crate::{NestError, Result};

pub fn checked_add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or(NestError::MathOverflow)
}

pub fn checked_sub(a: u128, b: u128) -> Result<u128> {
    a.checked_sub(b).ok_or(NestError::MathOverflow)
}

pub fn checked_mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b).ok_or(NestError::MathOverflow)
}

pub fn mul_div_down(a: u128, b: u128, denominator: u128) -> Result<u128> {
    if denominator == 0 {
        return Err(NestError::DivisionByZero);
    }
    Ok(checked_mul(a, b)? / denominator)
}

pub fn mul_div_up(a: u128, b: u128, denominator: u128) -> Result<u128> {
    if denominator == 0 {
        return Err(NestError::DivisionByZero);
    }
    let product = checked_mul(a, b)?;
    Ok(if product == 0 {
        0
    } else {
        (product - 1)
            .checked_div(denominator)
            .ok_or(NestError::DivisionByZero)?
            .checked_add(1)
            .ok_or(NestError::MathOverflow)?
    })
}

pub fn pow10(exp: u8) -> Result<u128> {
    let mut value = 1u128;
    for _ in 0..exp {
        value = value.checked_mul(10).ok_or(NestError::MathOverflow)?;
    }
    Ok(value)
}
