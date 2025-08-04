//! # AMM Logic
//!
//! This module contains pure, authoritative functions for all Automated Market Maker (AMM)
//! calculations. By centralizing this logic, we ensure that predictions, simulations, and
//! actual contract execution all behave identically, preventing economic exploits and
//! inconsistencies.

use crate::types::U256;
use anyhow::{anyhow, Result};

/// Calculates the output amount for a swap, given input amount and reserves.
/// This is based on the constant product formula (x * y = k), adjusted for fees.
///
/// # Arguments
/// * `amount_in` - The amount of the input token.
/// * `reserve_in` - The reserve of the input token in the pool.
/// * `reserve_out` - The reserve of the output token in the pool.
/// * `fee_bps` - The swap fee in basis points (e.g., 30 for 0.3%).
///
/// # Returns
/// The calculated output amount of the target token.
pub fn calculate_swap_out(
    amount_in: u128,
    reserve_in: u128,
    reserve_out: u128,
    fee_bps: u128,
) -> Result<u128> {
    if amount_in == 0 {
        return Err(anyhow!("Input amount cannot be zero"));
    }
    if reserve_in == 0 || reserve_out == 0 {
        return Err(anyhow!("Insufficient liquidity"));
    }

    let amount_in_u256 = U256::from(amount_in);
    let reserve_in_u256 = U256::from(reserve_in);
    let reserve_out_u256 = U256::from(reserve_out);

    // Authoritative Uniswap v2 formula
    let amount_in_with_fee = amount_in_u256 * (U256::from(10000) - U256::from(fee_bps));
    let numerator = amount_in_with_fee * reserve_out_u256;
    let denominator = (reserve_in_u256 * U256::from(10000)) + amount_in_with_fee;

    if denominator.is_zero() {
        return Err(anyhow!("Denominator is zero in swap calculation"));
    }

    let amount_out = numerator / denominator;
    Ok(amount_out.try_into()?)
}

/// Calculates the number of LP tokens to mint for a given liquidity provision.
///
/// # Arguments
/// * `amount_a` - The amount of token A being added.
/// * `amount_b` - The amount of token B being added.
/// * `reserve_a` - The current reserve of token A.
/// * `reserve_b` - The current reserve of token B.
/// * `total_supply` - The current total supply of LP tokens.
///
/// # Returns
/// The number of LP tokens to be minted.
pub fn calculate_lp_tokens_minted(
    amount_a: u128,
    amount_b: u128,
    reserve_a: u128,
    reserve_b: u128,
    total_supply: u128,
) -> Result<u128> {
    if total_supply == 0 {
        // First liquidity provider, LP tokens are geometric mean of amounts
        let lp_tokens = integer_sqrt(U256::from(amount_a) * U256::from(amount_b));
        Ok(lp_tokens.try_into()?)
    } else {
        // Subsequent provider, LP tokens are proportional to the lesser of the two amounts
        let lp_from_a = U256::from(amount_a) * U256::from(total_supply) / U256::from(reserve_a);
        let lp_from_b = U256::from(amount_b) * U256::from(total_supply) / U256::from(reserve_b);
        let min_lp = if lp_from_a < lp_from_b { lp_from_a } else { lp_from_b };
        Ok(min_lp.try_into()?)
    }
}

/// Calculates the price impact of a trade in basis points.
///
/// # Arguments
/// * `amount_in` - The amount of the input token.
/// * `reserve_in` - The reserve of the input token before the trade.
/// * `amount_out` - The amount of the output token.
/// * `reserve_out` - The reserve of the output token before the trade.
///
/// # Returns
/// The price impact in basis points (e.g., 100 for 1%).
pub fn calculate_price_impact(
    amount_in: u128,
    reserve_in: u128,
    amount_out: u128,
    reserve_out: u128,
) -> Result<u128> {
    if reserve_in == 0 || reserve_out == 0 {
        return Ok(10000); // 100% impact if no liquidity
    }

    let amount_in_u256 = U256::from(amount_in);
    let reserve_in_u256 = U256::from(reserve_in);
    let reserve_out_u256 = U256::from(reserve_out);

    // Ideal amount out without slippage (mid-price), ignoring fees for impact calculation
    let ideal_out = (amount_in_u256 * reserve_out_u256) / reserve_in_u256;
    let actual_out = U256::from(amount_out);

    if ideal_out.is_zero() {
        return Ok(10000); // Cannot calculate impact if ideal output is zero
    }
    
    // The difference between the ideal output and the actual output
    let impact_diff = if ideal_out > actual_out {
        ideal_out - actual_out
    } else {
        U256::from(0)
    };

    // Price impact as a percentage of the ideal output
    let impact_bps = (impact_diff * U256::from(10000)) / ideal_out;

    Ok(impact_bps.try_into().unwrap_or(10000))
}


/// Integer square root implementation for U256, using Babylonian method.
fn integer_sqrt(n: U256) -> U256 {
    if n.is_zero() {
        return U256::from(0);
    }
    let mut x = n;
    let mut y = (x + U256::from(1)) / U256::from(2);
    while y < x {
        x = y;
        y = (x + n / x) / U256::from(2);
    }
    x
}