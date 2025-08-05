//! Direct Contribution Tests for OYL Zap Contract
//!
//! These tests verify scenarios where the input token is one of the target LP tokens.

mod common;
use common::*;

#[test]
fn test_zap_with_direct_contribution_of_token_a() -> anyhow::Result<()> {
    println!("Testing zap with direct contribution of token A...");

    let mut zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();

    // Scenario: Zap from ETH into a WBTC/ETH pool.
    let eth = tokens["ETH"];
    let wbtc = tokens["WBTC"];
    let input_amount = 10 * 1e18 as u128; // 10 ETH

    // Get the quote. The input token `eth` is one of the target tokens.
    let quote = zap.get_zap_quote(eth, input_amount, wbtc, eth, DEFAULT_SLIPPAGE)?;
    validate_zap_quote(&quote)?;

    // In this scenario, route_b should be a direct contribution, meaning it has a path of length 1.
    assert_eq!(quote.route_b.path.len(), 1, "Route B should be a direct contribution of ETH");
    assert_eq!(quote.route_b.path[0], eth, "Route B path should be ETH");

    // Route A should involve a swap from ETH to WBTC.
    assert!(quote.route_a.path.len() > 1, "Route A should be a swap from ETH to WBTC");

    // Execute the zap
    let lp_tokens = zap.execute_zap(&quote)?;

    assert!(lp_tokens > 0, "Should receive positive LP tokens");
    assert!(
        lp_tokens >= quote.minimum_lp_tokens,
        "Should receive at least minimum LP tokens"
    );

    println!("✅ Zap with direct contribution of token A test passed");
    Ok(())
}

#[test]
fn test_zap_with_direct_contribution_of_token_b() -> anyhow::Result<()> {
    println!("Testing zap with direct contribution of token B...");

    let mut zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();

    // Scenario: Zap from WBTC into a WBTC/ETH pool.
    let wbtc = tokens["WBTC"];
    let eth = tokens["ETH"];
    let input_amount = 1 * 1e8 as u128; // 1 WBTC

    // Get the quote. The input token `wbtc` is one of the target tokens.
    let quote = zap.get_zap_quote(wbtc, input_amount, wbtc, eth, DEFAULT_SLIPPAGE)?;
    validate_zap_quote(&quote)?;

    // In this scenario, route_a should be a direct contribution, meaning it has a path of length 1.
    assert_eq!(quote.route_a.path.len(), 1, "Route A should be a direct contribution of WBTC");
    assert_eq!(quote.route_a.path[0], wbtc, "Route A path should be WBTC");

    // Route B should involve a swap from WBTC to ETH.
    assert!(quote.route_b.path.len() > 1, "Route B should be a swap from WBTC to ETH");

    // Execute the zap
    let lp_tokens = zap.execute_zap(&quote)?;

    assert!(lp_tokens > 0, "Should receive positive LP tokens");
    assert!(
        lp_tokens >= quote.minimum_lp_tokens,
        "Should receive at least minimum LP tokens"
    );

    println!("✅ Zap with direct contribution of token B test passed");
    Ok(())
}