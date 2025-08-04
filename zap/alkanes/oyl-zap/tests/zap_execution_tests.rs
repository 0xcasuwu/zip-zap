//! Zap Execution Tests for OYL Zap Contract
//!
//! These tests verify the successful execution of zap operations, including token transfers,
//! swaps, and liquidity provision. They also test failure scenarios during execution.

mod common;
use common::*;

#[test]
fn test_successful_zap_execution() -> anyhow::Result<()> {
    println!("Testing successful zap execution...");

    let mut zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();

    // Test a standard zap: UNI -> WBTC/DAI LP
    let uni = tokens["UNI"];
    let wbtc = tokens["WBTC"];
    let dai = tokens["DAI"];
    let input_amount = 1000 * 1e18 as u128; // 1000 UNI

    let quote = zap.get_zap_quote(uni, input_amount, wbtc, dai, DEFAULT_SLIPPAGE)?;
    validate_zap_quote(&quote)?;

    let lp_tokens = zap.execute_zap(&quote)?;

    assert!(lp_tokens > 0, "Should receive positive LP tokens");
    assert!(
        lp_tokens >= quote.minimum_lp_tokens,
        "Should receive at least minimum LP tokens"
    );

    println!("✅ Successful zap execution test passed");
    Ok(())
}

#[test]
fn test_zap_execution_with_insufficient_lp_tokens() -> anyhow::Result<()> {
    println!("Testing zap execution with insufficient LP tokens...");

    let mut zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();

    let uni = tokens["UNI"];
    let wbtc = tokens["WBTC"];
    let dai = tokens["DAI"];
    let input_amount = 1000 * 1e18 as u128; // 1000 UNI

    let mut quote = zap.get_zap_quote(uni, input_amount, wbtc, dai, DEFAULT_SLIPPAGE)?;
    
    // Artificially inflate the minimum required LP tokens to trigger a failure
    quote.minimum_lp_tokens = quote.expected_lp_tokens + 1;

    let result = zap.execute_zap(&quote);
    assert!(result.is_err(), "Should fail due to insufficient LP tokens");

    println!("✅ Zap execution with insufficient LP tokens test passed");
    Ok(())
}


#[test]
fn test_zap_execution_failure_on_swap() -> anyhow::Result<()> {
    println!("Testing zap execution failure on swap...");

    let mut zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();

    let uni = tokens["UNI"];
    let wbtc = tokens["WBTC"];
    let dai = tokens["DAI"];
    let input_amount = 1000 * 1e18 as u128; // 1000 UNI

    let quote = zap.get_zap_quote(uni, input_amount, wbtc, dai, DEFAULT_SLIPPAGE)?;
    validate_zap_quote(&quote)?;

    // Sabotage one of the pools to make the swap fail
    let eth = tokens["ETH"];
    zap.factory.pools.remove(&(uni, eth));

    let result = zap.execute_zap(&quote);
    assert!(result.is_err(), "Should fail if a swap fails during execution");

    println!("✅ Zap execution failure on swap test passed");
    Ok(())
}
