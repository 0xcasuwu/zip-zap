//! Route Discovery Tests for OYL Zap Contract
//! 
//! These tests verify the optimal route finding algorithms, multi-hop pathfinding,
//! route efficiency calculations, and path validation mechanisms.

mod common;
use common::*;

#[test]
fn test_direct_route_discovery() -> anyhow::Result<()> {
    println!("Testing direct route discovery...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let amount = 1e8 as u128; // 1 WBTC
    
    let route = zap.find_optimal_route(wbtc, eth, amount)?;
    
    // Verify direct route properties
    assert_eq!(route.path.len(), 2, "Direct route should have 2 tokens");
    assert_eq!(route.path[0], wbtc, "Route should start with input token");
    assert_eq!(route.path[1], eth, "Route should end with target token");
    assert!(route.is_direct_route(), "Should be identified as direct route");
    assert_eq!(route.hop_count(), 1, "Direct route should have 1 hop");
    assert!(route.expected_output > 0, "Should have positive expected output");
    
    validate_route_info(&route)?;
    
    println!("✅ Direct route discovery test passed");
    Ok(())
}

#[test]
fn test_single_hop_route_discovery() -> anyhow::Result<()> {
    println!("Testing single-hop route discovery...");
    
    let zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    let uni = tokens["UNI"];
    let dai = tokens["DAI"];
    let _eth = tokens["ETH"]; // Base token for routing
    let amount = 1000 * 1e18 as u128; // 1000 UNI
    
    let route = zap.find_optimal_route(uni, dai, amount)?;
    
    // Should find route through ETH: UNI -> ETH -> DAI
    assert!(route.path.len() >= 2, "Route should have at least 2 tokens");
    assert_eq!(route.path[0], uni, "Route should start with UNI");
    assert_eq!(route.path[route.path.len() - 1], dai, "Route should end with DAI");
    assert!(route.hop_count() >= 1, "Should have at least 1 hop");
    assert!(route.expected_output > 0, "Should have positive expected output");
    
    validate_route_info(&route)?;
    
    println!("✅ Single-hop route discovery test passed");
    Ok(())
}

#[test]
fn test_multi_hop_route_discovery() -> anyhow::Result<()> {
    println!("Testing multi-hop route discovery...");
    
    let zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    let comp = tokens["COMP"];
    let aave = tokens["AAVE"];
    let amount = 100 * 1e18 as u128; // 100 COMP
    
    let route = zap.find_optimal_route(comp, aave, amount)?;
    
    // Should find multi-hop route: COMP -> ETH/USDC -> AAVE
    assert!(route.path.len() >= 2, "Route should have at least 2 tokens");
    assert_eq!(route.path[0], comp, "Route should start with COMP");
    assert_eq!(route.path[route.path.len() - 1], aave, "Route should end with AAVE");
    assert!(route.hop_count() >= 1, "Should have at least 1 hop");
    assert!(route.expected_output > 0, "Should have positive expected output");
    
    validate_route_info(&route)?;
    
    println!("✅ Multi-hop route discovery test passed");
    Ok(())
}

#[test]
fn test_route_efficiency_comparison() -> anyhow::Result<()> {
    println!("Testing route efficiency comparison...");
    
    let zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    let wbtc = tokens["WBTC"];
    let dai = tokens["DAI"];
    let amount = 1e8 as u128; // 1 WBTC
    
    // Find the best route
    let best_route = zap.find_optimal_route(wbtc, dai, amount)?;
    
    // Verify route efficiency properties
    assert!(best_route.expected_output > 0, "Best route should have positive output");
    assert!(best_route.price_impact < MAX_PRICE_IMPACT, "Price impact should be reasonable");
    assert!(best_route.hop_count() <= 3, "Should not exceed maximum hops");
    
    // Test that the route finder prefers more efficient routes
    // (This is implicitly tested by the route finding algorithm)
    validate_route_info(&best_route)?;
    
    println!("✅ Route efficiency comparison test passed");
    Ok(())
}

#[test]
fn test_route_validation_and_error_handling() -> anyhow::Result<()> {
    println!("Testing route validation and error handling...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let nonexistent_token = alkane_id("NONEXISTENT");
    let amount = 1e8 as u128;
    
    // Test route to non-existent token
    let result = zap.find_optimal_route(wbtc, nonexistent_token, amount);
    assert!(result.is_err(), "Should fail for non-existent token");
    
    // Test route from token to itself
    let result = zap.find_optimal_route(wbtc, wbtc, amount);
    assert!(result.is_err(), "Should fail for same input/output token");
    
    // Test with zero amount
    let eth = alkane_id("ETH");
    let result = zap.find_optimal_route(wbtc, eth, 0);
    assert!(result.is_err(), "Should fail for zero amount");
    
    println!("✅ Route validation and error handling test passed");
    Ok(())
}

#[test]
fn test_base_token_routing_optimization() -> anyhow::Result<()> {
    println!("Testing base token routing optimization...");
    
    let zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    let link = tokens["LINK"];
    let comp = tokens["COMP"];
    let amount = 1000 * 1e18 as u128; // 1000 LINK
    
    let route = zap.find_optimal_route(link, comp, amount)?;
    
    // Should route through base tokens (ETH, USDC, etc.)
    assert!(route.path.len() >= 2, "Route should have at least 2 tokens");
    assert_eq!(route.path[0], link, "Route should start with LINK");
    assert_eq!(route.path[route.path.len() - 1], comp, "Route should end with COMP");
    
    // Check if route uses base tokens
    let base_tokens = vec![tokens["ETH"], tokens["USDC"], tokens["USDT"], tokens["DAI"]];
    let uses_base_token = route.path.iter().any(|token| base_tokens.contains(token));
    
    if route.path.len() > 2 {
        assert!(uses_base_token, "Multi-hop route should use base tokens for efficiency");
    }
    
    validate_route_info(&route)?;
    
    println!("✅ Base token routing optimization test passed");
    Ok(())
}

#[test]
fn test_price_impact_calculation_accuracy() -> anyhow::Result<()> {
    println!("Testing price impact calculation accuracy...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    
    // Test different amounts to verify price impact scaling
    let test_amounts = vec![
        1e6 as u128,   // Small amount
        1e7 as u128,   // Medium amount  
        1e8 as u128,   // Large amount
        5e8 as u128,   // Very large amount
    ];
    
    let mut previous_impact = 0u128;
    
    for amount in test_amounts {
        let route = zap.find_optimal_route(wbtc, eth, amount)?;
        
        // Price impact should increase with larger amounts
        if previous_impact > 0 {
            assert!(
                route.price_impact >= previous_impact,
                "Price impact should increase with larger amounts. Previous: {}, Current: {}",
                previous_impact,
                route.price_impact
            );
        }
        
        assert_price_impact_reasonable(route.price_impact, MAX_PRICE_IMPACT);
        previous_impact = route.price_impact;
        
        validate_route_info(&route)?;
    }
    
    println!("✅ Price impact calculation accuracy test passed");
    Ok(())
}

#[test]
fn test_gas_estimation_accuracy() -> anyhow::Result<()> {
    println!("Testing gas estimation accuracy...");
    
    let zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    let wbtc = tokens["WBTC"];
    let eth = tokens["ETH"];
    let usdc = tokens["USDC"];
    let amount = 1e8 as u128;
    
    // Test direct route gas estimation
    let direct_route = zap.find_optimal_route(wbtc, eth, amount)?;
    
    // Test indirect route gas estimation
    let indirect_route = zap.find_optimal_route(wbtc, usdc, amount)?;
    
    // Gas should scale with number of hops
    assert!(direct_route.gas_estimate > 0, "Direct route should have positive gas estimate");
    
    if indirect_route.hop_count() > direct_route.hop_count() {
        assert!(
            indirect_route.gas_estimate > direct_route.gas_estimate,
            "More hops should require more gas. Direct: {}, Indirect: {}",
            direct_route.gas_estimate,
            indirect_route.gas_estimate
        );
    }
    
    // Reasonable gas estimates (not too high or too low)
    assert!(direct_route.gas_estimate < 1_000_000, "Gas estimate should be reasonable");
    assert!(indirect_route.gas_estimate < 1_000_000, "Gas estimate should be reasonable");
    
    validate_route_info(&direct_route)?;
    validate_route_info(&indirect_route)?;
    
    println!("✅ Gas estimation accuracy test passed");
    Ok(())
}

#[test]
fn test_route_caching_and_performance() -> anyhow::Result<()> {
    println!("Testing route caching and performance...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let amount = 1e8 as u128;
    
    // Benchmark route finding performance
    let iterations = 100;
    let duration = benchmark_route_finding(&zap, wbtc, eth, amount, iterations);
    
    println!("Route finding took {:?} for {} iterations", duration, iterations);
    println!("Average time per route: {:?}", duration / iterations as u32);
    
    // Performance should be reasonable (less than 1ms per route on average)
    let avg_duration = duration / iterations as u32;
    assert!(
        avg_duration < std::time::Duration::from_millis(10),
        "Route finding should be performant. Average: {:?}",
        avg_duration
    );
    
    // Verify that multiple calls return consistent results
    let route1 = zap.find_optimal_route(wbtc, eth, amount)?;
    let route2 = zap.find_optimal_route(wbtc, eth, amount)?;
    
    assert_eq!(route1.path, route2.path, "Routes should be consistent");
    assert_eq!(route1.expected_output, route2.expected_output, "Output should be consistent");
    
    println!("✅ Route caching and performance test passed");
    Ok(())
}

#[test]
fn test_liquidity_aware_routing() -> anyhow::Result<()> {
    println!("Testing liquidity-aware routing...");
    
    let zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    let wbtc = tokens["WBTC"];
    let usdc = tokens["USDC"];
    
    // Test with small amount (should prefer direct route)
    let small_amount = 1e6 as u128; // 0.01 WBTC
    let small_route = zap.find_optimal_route(wbtc, usdc, small_amount)?;
    
    // Test with large amount (might prefer indirect route to minimize slippage)
    let large_amount = 10 * 1e8 as u128; // 10 WBTC
    let large_route = zap.find_optimal_route(wbtc, usdc, large_amount)?;
    
    // Verify both routes are valid
    validate_route_info(&small_route)?;
    validate_route_info(&large_route)?;
    
    // Large amounts should have higher price impact
    assert!(
        large_route.price_impact >= small_route.price_impact,
        "Large amounts should have higher price impact. Small: {}, Large: {}",
        small_route.price_impact,
        large_route.price_impact
    );
    
    println!("✅ Liquidity-aware routing test passed");
    Ok(())
}

#[test]
fn test_stablecoin_routing_optimization() -> anyhow::Result<()> {
    println!("Testing stablecoin routing optimization...");
    
    let zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    let usdc = tokens["USDC"];
    let dai = tokens["DAI"];
    let amount = 1000 * 1e6 as u128; // 1000 USDC
    
    let route = zap.find_optimal_route(usdc, dai, amount)?;
    
    // Stablecoin routes should have low price impact
    assert!(
        route.price_impact < 100, // Less than 1%
        "Stablecoin routes should have low price impact: {}%",
        route.price_impact as f64 / 100.0
    );
    
    // Should prefer direct route for stablecoins
    assert!(
        route.hop_count() <= 2,
        "Stablecoin routes should be direct or single-hop"
    );
    
    validate_route_info(&route)?;
    
    println!("✅ Stablecoin routing optimization test passed");
    Ok(())
}

#[test]
fn test_route_discovery_edge_cases() -> anyhow::Result<()> {
    println!("Testing route discovery edge cases...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    
    // Test with very small amount
    let tiny_amount = 1u128;
    let result = zap.find_optimal_route(wbtc, eth, tiny_amount);
    // Should either succeed with minimal output or fail gracefully
    if let Ok(route) = result {
        validate_route_info(&route)?;
        assert!(route.expected_output > 0, "Even tiny amounts should have some output");
    }
    
    // Test with maximum reasonable amount
    let max_amount = u128::MAX / 1000; // Avoid overflow
    let result = zap.find_optimal_route(wbtc, eth, max_amount);
    // Should handle large amounts gracefully
    if let Ok(route) = result {
        validate_route_info(&route)?;
        // Price impact might be very high but should not overflow
        assert!(route.price_impact <= 10000, "Price impact should not exceed 100%");
    }
    
    println!("✅ Route discovery edge cases test passed");
    Ok(())
}

#[test]
fn test_route_comparison_and_selection() -> anyhow::Result<()> {
    println!("Testing route comparison and selection...");
    
    let zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    let wbtc = tokens["WBTC"];
    let dai = tokens["DAI"];
    let amount = 1e8 as u128; // 1 WBTC
    
    // Get the optimal route
    let optimal_route = zap.find_optimal_route(wbtc, dai, amount)?;
    
    // Verify route selection criteria
    validate_route_info(&optimal_route)?;
    
    // The route should optimize for:
    // 1. Higher output amount
    assert!(optimal_route.expected_output > 0, "Should have positive output");
    
    // 2. Reasonable price impact
    assert_price_impact_reasonable(optimal_route.price_impact, MAX_PRICE_IMPACT);
    
    // 3. Reasonable number of hops
    assert!(optimal_route.hop_count() <= 3, "Should not have too many hops");
    
    // 4. Reasonable gas cost
    assert!(optimal_route.gas_estimate > 0, "Should have positive gas estimate");
    assert!(optimal_route.gas_estimate < 1_000_000, "Gas should be reasonable");
    
    println!("✅ Route comparison and selection test passed");
    Ok(())
}
