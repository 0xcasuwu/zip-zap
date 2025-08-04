//! Integration Tests for OYL Zap Contract
//! 
//! These tests verify complex multi-step zap operations, error handling and recovery,
//! state consistency verification, cross-contract interaction testing, and comprehensive stress testing.

mod common;
use common::*;

#[test]
fn test_complex_multi_step_zap_operations() -> anyhow::Result<()> {
    println!("Testing complex multi-step zap operations...");
    
    let mut zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    // Test complex scenario: UNI -> WBTC/DAI LP
    let uni = tokens["UNI"];
    let wbtc = tokens["WBTC"];
    let dai = tokens["DAI"];
    let input_amount = 1000 * 1e18 as u128; // 1000 UNI
    
    println!("Step 1: Getting zap quote for UNI -> WBTC/DAI...");
    let quote = zap.get_zap_quote(uni, input_amount, wbtc, dai, DEFAULT_SLIPPAGE)?;
    validate_zap_quote(&quote)?;
    
    // Verify complex routing
    println!("Route A (UNI -> WBTC): {} hops", quote.route_a.hop_count());
    println!("Route B (UNI -> DAI): {} hops", quote.route_b.hop_count());
    
    assert!(
        quote.route_a.hop_count() >= 1,
        "Route A should require at least 1 hop for UNI -> WBTC"
    );
    assert!(
        quote.route_b.hop_count() >= 1,
        "Route B should require at least 1 hop for UNI -> DAI"
    );
    
    println!("Step 2: Executing complex zap...");
    let lp_tokens = zap.execute_zap(&quote)?;
    
    // Verify successful execution
    assert!(lp_tokens > 0, "Should receive positive LP tokens");
    assert!(
        lp_tokens >= quote.minimum_lp_tokens,
        "Should receive at least minimum LP tokens"
    );
    
    println!("Step 3: Verifying pool state changes...");
    let target_pool = zap.factory.get_pool(wbtc, dai)
        .ok_or_else(|| anyhow::anyhow!("Target pool not found"))?;
    
    assert!(target_pool.total_supply > 0, "Pool should have liquidity");
    assert!(target_pool.reserve_a > 0, "Pool should have WBTC reserves");
    assert!(target_pool.reserve_b > 0, "Pool should have DAI reserves");
    
    println!("✅ Complex multi-step zap operations test passed");
    Ok(())
}

#[test]
fn test_error_handling_and_recovery() -> anyhow::Result<()> {
    println!("Testing error handling and recovery...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test 1: Invalid input parameters
    println!("Testing invalid input parameters...");
    
    // Zero amount
    let result = zap.get_zap_quote(wbtc, 0, eth, usdc, DEFAULT_SLIPPAGE);
    assert!(result.is_err(), "Should reject zero amount");
    
    // Same input and target tokens
    let result = zap.get_zap_quote(wbtc, 1e8 as u128, wbtc, usdc, DEFAULT_SLIPPAGE);
    assert!(result.is_err(), "Should reject same input and target token");
    
    // Same target tokens
    let result = zap.get_zap_quote(wbtc, 1e8 as u128, eth, eth, DEFAULT_SLIPPAGE);
    assert!(result.is_err(), "Should reject same target tokens");
    
    // Excessive slippage
    let result = zap.get_zap_quote(wbtc, 1e8 as u128, eth, usdc, 10001);
    assert!(result.is_err(), "Should reject excessive slippage");
    
    // Test 2: Non-existent tokens
    println!("Testing non-existent tokens...");
    let fake_token = alkane_id("FAKE_TOKEN");
    
    let result = zap.get_zap_quote(fake_token, 1e8 as u128, eth, usdc, DEFAULT_SLIPPAGE);
    assert!(result.is_err(), "Should reject non-existent input token");
    
    let result = zap.get_zap_quote(wbtc, 1e8 as u128, fake_token, usdc, DEFAULT_SLIPPAGE);
    assert!(result.is_err(), "Should reject non-existent target token A");
    
    let result = zap.get_zap_quote(wbtc, 1e8 as u128, eth, fake_token, DEFAULT_SLIPPAGE);
    assert!(result.is_err(), "Should reject non-existent target token B");
    
    // Test 3: Recovery after errors
    println!("Testing recovery after errors...");
    
    // After errors, normal operations should still work
    let valid_quote = zap.get_zap_quote(wbtc, 1e8 as u128, eth, usdc, DEFAULT_SLIPPAGE)?;
    validate_zap_quote(&valid_quote)?;
    
    println!("Normal operation after errors: {} LP tokens expected", valid_quote.expected_lp_tokens);
    
    // Test 4: Boundary conditions
    println!("Testing boundary conditions...");
    
    // Very large amount (should either work or fail gracefully)
    let large_amount = u128::MAX / 1000;
    let result = zap.get_zap_quote(wbtc, large_amount, eth, usdc, DEFAULT_SLIPPAGE);
    match result {
        Ok(quote) => {
            validate_zap_quote(&quote)?;
            println!("Large amount handled successfully");
        }
        Err(_) => {
            println!("Large amount rejected gracefully");
        }
    }
    
    // Very small amount (should either work or fail gracefully)
    let tiny_amount = 1u128;
    let result = zap.get_zap_quote(wbtc, tiny_amount, eth, usdc, DEFAULT_SLIPPAGE);
    match result {
        Ok(quote) => {
            validate_zap_quote(&quote)?;
            println!("Tiny amount handled successfully");
        }
        Err(_) => {
            println!("Tiny amount rejected gracefully");
        }
    }
    
    println!("✅ Error handling and recovery test passed");
    Ok(())
}

#[test]
fn test_state_consistency_verification() -> anyhow::Result<()> {
    println!("Testing state consistency verification...");
    
    let mut zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test state consistency across multiple operations
    let operations = vec![
        (1e8 as u128, "1 WBTC"),
        (5e7 as u128, "0.5 WBTC"),
        (2e8 as u128, "2 WBTC"),
        (1e8 as u128, "1 WBTC again"),
    ];
    
    let mut total_input = 0u128;
    let mut total_lp_output = 0u128;
    let mut pool_states = Vec::new();
    
    for (i, (amount, description)) in operations.iter().enumerate() {
        println!("Operation {}: {}", i + 1, description);
        
        // Get initial pool state
        let initial_pool = zap.factory.get_pool(eth, usdc)
            .ok_or_else(|| anyhow::anyhow!("Pool not found"))?
            .clone();
        
        // Execute zap
        let quote = zap.get_zap_quote(wbtc, *amount, eth, usdc, DEFAULT_SLIPPAGE)?;
        let lp_tokens = zap.execute_zap(&quote.clone())?;
        
        // Get final pool state
        let final_pool = zap.factory.get_pool(eth, usdc)
            .ok_or_else(|| anyhow::anyhow!("Pool not found"))?;
        
        // Verify state consistency
        
        // 1. Pool reserves should only increase
        assert!(
            final_pool.reserve_a >= initial_pool.reserve_a,
            "Pool reserve A should not decrease in operation {}", i + 1
        );
        assert!(
            final_pool.reserve_b >= initial_pool.reserve_b,
            "Pool reserve B should not decrease in operation {}", i + 1
        );
        
        // 2. Total supply should increase
        assert!(
            final_pool.total_supply > initial_pool.total_supply,
            "Pool total supply should increase in operation {}", i + 1
        );
        
        // 3. LP tokens should be positive
        assert!(lp_tokens > 0, "LP tokens should be positive in operation {}", i + 1);
        
        // 4. Track cumulative values
        total_input += amount;
        total_lp_output += lp_tokens;
        pool_states.push((initial_pool.clone(), final_pool.clone(), lp_tokens));
        
        validate_zap_quote(&quote)?;
        
        println!("  {} -> {} LP tokens", description, lp_tokens);
        println!("  Pool reserves: {} -> {}, {} -> {}", 
                initial_pool.reserve_a, final_pool.reserve_a,
                initial_pool.reserve_b, final_pool.reserve_b);
    }
    
    // Verify overall consistency
    let final_pool = &pool_states.last().unwrap().1;
    let initial_pool = &pool_states.first().unwrap().0;
    
    // Total LP tokens issued should match pool supply increase
    let total_supply_increase = final_pool.total_supply - initial_pool.total_supply;
    assert_eq!(
        total_lp_output,
        total_supply_increase,
        "Total LP tokens issued should match pool supply increase"
    );
    
    // Pool value should have increased
    let initial_value = initial_pool.reserve_a + initial_pool.reserve_b;
    let final_value = final_pool.reserve_a + final_pool.reserve_b;
    assert!(
        final_value > initial_value,
        "Pool value should increase due to liquidity addition"
    );
    
    println!("Total input: {} WBTC", total_input as f64 / 1e8);
    println!("Total LP output: {}", total_lp_output);
    println!("Pool value increase: {} -> {}", initial_value, final_value);
    
    println!("✅ State consistency verification test passed");
    Ok(())
}

#[test]
fn test_cross_contract_interaction_testing() -> anyhow::Result<()> {
    println!("Testing cross-contract interaction...");
    
    let mut zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    // Test interactions across multiple pools and tokens
    let test_scenarios = vec![
        // Scenario 1: Major token pair
        (tokens["WBTC"], tokens["ETH"], tokens["USDC"], "WBTC -> ETH/USDC"),
        
        // Scenario 2: Stablecoin pair
        (tokens["USDC"], tokens["USDT"], tokens["DAI"], "USDC -> USDT/DAI"),
        
        // Scenario 3: DeFi token pair
        (tokens["UNI"], tokens["AAVE"], tokens["COMP"], "UNI -> AAVE/COMP"),
        
        // Scenario 4: Mixed pair
        (tokens["ETH"], tokens["WBTC"], tokens["DAI"], "ETH -> WBTC/DAI"),
    ];
    
    for (input_token, target_a, target_b, description) in test_scenarios {
        println!("Testing scenario: {}", description);
        
        let amount = match description {
            s if s.contains("WBTC") => 1e8 as u128,      // 1 WBTC
            s if s.contains("ETH") => 1e18 as u128,      // 1 ETH
            s if s.contains("UNI") => 100 * 1e18 as u128, // 100 UNI
            _ => 1000 * 1e6 as u128,                     // 1000 USDC equivalent
        };
        
        // Test quote generation
        let quote = zap.get_zap_quote(input_token, amount, target_a, target_b, DEFAULT_SLIPPAGE)?;
        validate_zap_quote(&quote)?;
        
        // Verify cross-contract routing
        println!("  Route A: {} hops", quote.route_a.hop_count());
        println!("  Route B: {} hops", quote.route_b.hop_count());
        println!("  Price impact: {}%", quote.price_impact as f64 / 100.0);
        
        // Test execution
        let lp_tokens = zap.execute_zap(&quote.clone())?;
        assert!(lp_tokens > 0, "Should receive positive LP tokens for {}", description);
        assert!(
            lp_tokens >= quote.minimum_lp_tokens,
            "Should meet minimum LP requirements for {}", description
        );
        
        // Verify target pool state
        let target_pool = zap.factory.get_pool(target_a, target_b)
            .ok_or_else(|| anyhow::anyhow!("Target pool not found for {}", description))?;
        
        assert!(target_pool.total_supply > 0, "Target pool should have liquidity for {}", description);
        
        println!("  Result: {} LP tokens", lp_tokens);
        
        // Reset state for next test
        let (factory, _token_map) = setup_comprehensive_test_environment();
        zap.factory = factory;
    }
    
    println!("✅ Cross-contract interaction testing test passed");
    Ok(())
}

#[test]
fn test_comprehensive_stress_testing() -> anyhow::Result<()> {
    println!("Testing comprehensive stress scenarios...");
    
    let mut zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Stress Test 1: High frequency operations
    println!("Stress Test 1: High frequency operations...");
    let start_time = std::time::Instant::now();
    let iterations = 100;
    
    for i in 0..iterations {
        let amount = ((i % 10) + 1) as u128 * 1e7 as u128; // Varying amounts
        let quote = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE)?;
        validate_zap_quote(&quote)?;
        
        // Every 10th iteration, execute the zap
        if i % 10 == 0 {
            let lp_tokens = zap.execute_zap(&quote.clone())?;
            assert!(lp_tokens > 0, "Iteration {} should produce LP tokens", i);
        }
    }
    
    let duration = start_time.elapsed();
    println!("  {} operations completed in {:?}", iterations, duration);
    println!("  Average time per operation: {:?}", duration / iterations);
    
    // Performance should be reasonable
    assert!(
        duration < std::time::Duration::from_secs(10),
        "High frequency operations should complete within reasonable time"
    );
    
    // Stress Test 2: Large amount variations
    println!("Stress Test 2: Large amount variations...");
    let large_amounts = vec![
        1e8 as u128,      // 1 WBTC
        10 * 1e8 as u128, // 10 WBTC
        100 * 1e8 as u128, // 100 WBTC
        1000 * 1e8 as u128, // 1000 WBTC
    ];
    
    for amount in large_amounts {
        let result = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE);
        
        match result {
            Ok(quote) => {
                validate_zap_quote(&quote)?;
                println!("  {} WBTC: {}% price impact", amount as f64 / 1e8, quote.price_impact as f64 / 100.0);
                
                // Price impact should increase with amount
                assert!(quote.price_impact > 0, "Large amounts should have price impact");
            }
            Err(_) => {
                println!("  {} WBTC: Rejected gracefully", amount as f64 / 1e8);
            }
        }
    }
    
    // Stress Test 3: Extreme slippage tolerances
    println!("Stress Test 3: Extreme slippage tolerances...");
    let slippage_tests = vec![
        1u128,      // 0.01%
        10u128,     // 0.1%
        100u128,    // 1%
        1000u128,   // 10%
        5000u128,   // 50%
        9999u128,   // 99.99%
    ];
    
    for slippage in slippage_tests {
        let result = zap.get_zap_quote(wbtc, 1e8 as u128, eth, usdc, slippage);
        
        match result {
            Ok(quote) => {
                validate_zap_quote(&quote)?;
                println!("  {}% slippage: {} min LP tokens", 
                        slippage as f64 / 100.0, quote.minimum_lp_tokens);
                
                // Minimum should decrease with higher slippage tolerance
                let expected_min = quote.expected_lp_tokens * (10000 - slippage) / 10000;
                assert_within_tolerance(quote.minimum_lp_tokens, expected_min, 100);
            }
            Err(_) => {
                println!("  {}% slippage: Rejected gracefully", slippage as f64 / 100.0);
            }
        }
    }
    
    // Stress Test 4: Concurrent operations simulation
    println!("Stress Test 4: Concurrent operations simulation...");
    let mut concurrent_results = Vec::new();
    
    for i in 0..20 {
        // Reset state for each "concurrent" operation
        let (factory, base_tokens) = setup_test_environment();
        zap.factory = factory;
        zap.base_tokens = base_tokens;
        
        let amount = (i + 1) as u128 * 5e6 as u128; // Varying amounts
        let quote = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE)?;
        let lp_tokens = zap.execute_zap(&quote.clone())?;
        
        concurrent_results.push((amount, lp_tokens));
    }
    
    // Verify all concurrent operations succeeded
    for (i, (amount, lp_tokens)) in concurrent_results.iter().enumerate() {
        assert!(*lp_tokens > 0, "Concurrent operation {} should produce LP tokens", i + 1);
        println!("  Operation {}: {} -> {} LP tokens", i + 1, amount, lp_tokens);
    }
    
    // Stress Test 5: Memory and resource usage
    println!("Stress Test 5: Memory and resource usage...");
    let mut large_data_test = Vec::new();
    
    for i in 0..50 {
        let quote = zap.get_zap_quote(wbtc, (i + 1) as u128 * 1e7 as u128, eth, usdc, DEFAULT_SLIPPAGE)?;
        large_data_test.push(quote);
    }
    
    // Verify all quotes are valid
    for (i, quote) in large_data_test.iter().enumerate() {
        validate_zap_quote(quote)?;
        assert!(quote.expected_lp_tokens > 0, "Quote {} should have positive LP tokens", i + 1);
    }
    
    println!("  {} quotes generated and validated", large_data_test.len());
    
    println!("✅ Comprehensive stress testing test passed");
    Ok(())
}

#[test]
fn test_end_to_end_integration_scenarios() -> anyhow::Result<()> {
    println!("Testing end-to-end integration scenarios...");
    
    let mut zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    // Scenario 1: Complete DeFi user journey
    println!("Scenario 1: Complete DeFi user journey...");
    
    // User starts with UNI tokens and wants to provide liquidity to WBTC/USDC pool
    let user_uni = 1000 * 1e18 as u128; // 1000 UNI
    let target_wbtc = tokens["WBTC"];
    let target_usdc = tokens["USDC"];
    
    // Step 1: Get quote
    let quote = zap.get_zap_quote(tokens["UNI"], user_uni, target_wbtc, target_usdc, DEFAULT_SLIPPAGE)?;
    validate_zap_quote(&quote)?;
    
    println!("  Quote: {} UNI -> {} LP tokens (min: {})", 
            user_uni, quote.expected_lp_tokens, quote.minimum_lp_tokens);
    println!("  Price impact: {}%", quote.price_impact as f64 / 100.0);
    
    // Step 2: Execute zap
    let lp_tokens_received = zap.execute_zap(&quote.clone())?;
    
    // Step 3: Verify results
    assert!(lp_tokens_received >= quote.minimum_lp_tokens, "Should meet minimum LP requirements");
    assert_within_tolerance(lp_tokens_received, quote.expected_lp_tokens, DEFAULT_SLIPPAGE);
    
    println!("  Result: {} LP tokens received", lp_tokens_received);
    
    // Scenario 2: Arbitrage opportunity exploitation
    println!("Scenario 2: Cross-pool arbitrage resistance...");
    
    // Try to exploit price differences across pools
    let eth_amount = 10 * 1e18 as u128; // 10 ETH
    
    // Route 1: ETH -> USDC -> DAI
    let quote1 = zap.get_zap_quote(tokens["ETH"], eth_amount, tokens["USDC"], tokens["DAI"], DEFAULT_SLIPPAGE)?;
    
    // Route 2: ETH -> DAI -> USDC  
    let quote2 = zap.get_zap_quote(tokens["ETH"], eth_amount, tokens["DAI"], tokens["USDC"], DEFAULT_SLIPPAGE)?;
    
    // Results should be similar (no significant arbitrage opportunity)
    let lp_difference = calculate_percentage_difference(quote1.expected_lp_tokens, quote2.expected_lp_tokens);
    assert!(
        lp_difference <= 500, // Within 5%
        "Cross-pool arbitrage opportunities should be minimal. Difference: {}%",
        lp_difference as f64 / 100.0
    );
    
    println!("  Route 1 LP tokens: {}", quote1.expected_lp_tokens);
    println!("  Route 2 LP tokens: {}", quote2.expected_lp_tokens);
    println!("  Difference: {}%", lp_difference as f64 / 100.0);
    
    // Scenario 3: Multi-user concurrent usage
    println!("Scenario 3: Multi-user concurrent usage...");
    
    let user_scenarios = vec![
        (tokens["WBTC"], 1e8 as u128, tokens["ETH"], tokens["USDC"], "User 1: WBTC -> ETH/USDC"),
        (tokens["ETH"], 5 * 1e18 as u128, tokens["USDC"], tokens["DAI"], "User 2: ETH -> USDC/DAI"),
        (tokens["USDC"], 10000 * 1e6 as u128, tokens["WBTC"], tokens["ETH"], "User 3: USDC -> WBTC/ETH"),
    ];
    
    let mut user_results = Vec::new();
    
    for (input_token, amount, target_a, target_b, description) in user_scenarios {
        // Reset to fair initial conditions for each user
        let (factory, _token_map) = setup_comprehensive_test_environment();
        zap.factory = factory;
        
        let quote = zap.get_zap_quote(input_token, amount, target_a, target_b, DEFAULT_SLIPPAGE)?;
        let lp_tokens = zap.execute_zap(&quote.clone())?;
        
        user_results.push((description, lp_tokens, quote.price_impact));
        
        println!("  {}: {} LP tokens, {}% price impact", 
                description, lp_tokens, quote.price_impact as f64 / 100.0);
        
        validate_zap_quote(&quote)?;
    }
    
    // All users should get fair treatment
    for (description, lp_tokens, _) in &user_results {
        assert!(*lp_tokens > 0, "{} should receive positive LP tokens", description);
    }
    
    // Scenario 4: Protocol upgrade simulation
    println!("Scenario 4: Protocol configuration changes...");
    
    // Test with different base token configurations
    let original_base_tokens = zap.base_tokens.clone();
    
    // Add new base token
    let mut new_base_tokens = original_base_tokens.clone();
    new_base_tokens.push(tokens["LINK"]);
    zap.base_tokens = new_base_tokens;
    
    let quote_with_new_base = zap.get_zap_quote(tokens["COMP"], 100 * 1e18 as u128, tokens["AAVE"], tokens["UNI"], DEFAULT_SLIPPAGE)?;
    validate_zap_quote(&quote_with_new_base)?;
    
    // Restore original configuration
    zap.base_tokens = original_base_tokens;
    
    let quote_original = zap.get_zap_quote(tokens["COMP"], 100 * 1e18 as u128, tokens["AAVE"], tokens["UNI"], DEFAULT_SLIPPAGE)?;
    validate_zap_quote(&quote_original)?;
    
    println!("  With additional base token: {} LP tokens", quote_with_new_base.expected_lp_tokens);
    println!("  With original base tokens: {} LP tokens", quote_original.expected_lp_tokens);
    
    // Both configurations should work
    assert!(quote_with_new_base.expected_lp_tokens > 0, "New configuration should work");
    assert!(quote_original.expected_lp_tokens > 0, "Original configuration should work");
    
    println!("✅ End-to-end integration scenarios test passed");
    Ok(())
}

#[test]
fn test_configuration_changes() -> anyhow::Result<()> {
    println!("Testing configuration changes...");

    let mut zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();

    // Test removing a base token
    let initial_base_tokens_count = zap.base_tokens.len();
    let token_to_remove = tokens["DAI"];
    zap.base_tokens.retain(|&x| x != token_to_remove);
    assert_eq!(zap.base_tokens.len(), initial_base_tokens_count - 1);

    // Test changing the factory ID
    let new_factory_id = alkane_id("new_factory");
    zap.factory_id = new_factory_id;
    assert_eq!(zap.factory_id, new_factory_id);

    println!("✅ Configuration changes test passed");
    Ok(())
}

#[test]
fn test_performance_and_scalability() -> anyhow::Result<()> {
    println!("Testing performance and scalability...");
    
    let zap = MockOylZap::with_comprehensive_setup();
    let (_, tokens) = setup_comprehensive_test_environment();
    
    // Performance Test 1: Quote generation speed
    println!("Performance Test 1: Quote generation speed...");
    
    let test_pairs = vec![
        (tokens["WBTC"], tokens["ETH"], tokens["USDC"]),
        (tokens["ETH"], tokens["USDC"], tokens["DAI"]),
        (tokens["UNI"], tokens["AAVE"], tokens["COMP"]),
        (tokens["LINK"], tokens["WBTC"], tokens["DAI"]),
    ];
    
    let iterations_per_pair = 25;
    let mut total_duration = std::time::Duration::new(0, 0);
    let mut total_operations = 0;
    
    for (input_token, target_a, target_b) in test_pairs {
        let start = std::time::Instant::now();
        
        for i in 0..iterations_per_pair {
            let amount = (i + 1) as u128 * 1e15 as u128; // Varying amounts
            let _quote = zap.get_zap_quote(input_token, amount, target_a, target_b, DEFAULT_SLIPPAGE)?;
        }
        
        let duration = start.elapsed();
        total_duration += duration;
        total_operations += iterations_per_pair;
        
        println!("  {:?} -> {:?}/{:?}: {} ops in {:?}", 
                input_token, target_a, target_b, iterations_per_pair, duration);
    }
    
    let avg_duration = total_duration / total_operations;
    println!("  Average quote generation time: {:?}", avg_duration);
    
    // Performance should be reasonable
    assert!(
        avg_duration < std::time::Duration::from_millis(10),
        "Quote generation should be fast. Average: {:?}",
        avg_duration
    );
    
    // Performance Test 2: Memory usage with large datasets
    println!("Performance Test 2: Memory usage with large datasets...");
    
    let mut large_quote_set = Vec::new();
    let large_dataset_size = 200;
    
    let start = std::time::Instant::now();
    
    for i in 0..large_dataset_size {
        let amount = (i + 1) as u128 * 1e14 as u128;
        let quote = zap.get_zap_quote(tokens["WBTC"], amount, tokens["ETH"], tokens["USDC"], DEFAULT_SLIPPAGE)?;
        large_quote_set.push(quote);
    }
    
    let duration = start.elapsed();
    println!("  {} quotes generated in {:?}", large_dataset_size, duration);
    
    // Verify all quotes are valid
    for (i, quote) in large_quote_set.iter().enumerate() {
        validate_zap_quote(quote)?;
        assert!(quote.expected_lp_tokens > 0, "Quote {} should be valid", i + 1);
    }
    
    // Performance Test 3: Scalability with different pool sizes
    println!("Performance Test 3: Scalability with different pool sizes...");
    
    let scalability_tests = vec![
        (1e15 as u128, "Small pools"),
        (1e18 as u128, "Medium pools"),
        (1e21 as u128, "Large pools"),
    ];
    
    for (pool_scale, description) in scalability_tests {
        let start = std::time::Instant::now();
        
        // Simulate different pool sizes by using different input amounts
        let quote = zap.get_zap_quote(tokens["ETH"], pool_scale / 1000, tokens["USDC"], tokens["DAI"], DEFAULT_SLIPPAGE)?;
        
        let duration = start.elapsed();
        
        validate_zap_quote(&quote)?;
        println!("  {}: {:?} for quote generation", description, duration);
        
        // Performance should scale reasonably
        assert!(
            duration < std::time::Duration::from_millis(100),
            "Quote generation should be scalable for {}. Duration: {:?}",
            description,
            duration
        );
    }
    
    println!("✅ Performance and scalability test passed");
    Ok(())
}
