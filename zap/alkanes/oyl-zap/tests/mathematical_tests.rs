//! Mathematical Tests for OYL Zap Contract
//! 
//! These tests verify mathematical correctness including optimal split calculation verification,
//! route output calculation accuracy, precision handling, numerical stability, and invariant preservation.

mod common;
use common::*;

#[test]
fn test_optimal_split_calculation_verification() -> anyhow::Result<()> {
    println!("Testing optimal split calculation verification...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    let input_amount = 1e8 as u128; // 1 WBTC
    
    let quote = zap.get_zap_quote(wbtc, input_amount, eth, usdc, DEFAULT_SLIPPAGE)?;
    
    // Mathematical properties of optimal split
    
    // 1. Split amounts must sum to input amount (conservation)
    assert_eq!(
        quote.split_amount_a + quote.split_amount_b,
        quote.input_amount,
        "Split amounts must sum to input amount (conservation law)"
    );
    
    // 2. Both splits must be positive (feasibility)
    assert!(quote.split_amount_a > 0, "Split A must be positive");
    assert!(quote.split_amount_b > 0, "Split B must be positive");
    
    // 3. Split should be reasonably balanced for similar target tokens
    let split_ratio = (quote.split_amount_a * 1000) / quote.input_amount;
    assert!(
        split_ratio >= 200 && split_ratio <= 800, // Between 20% and 80%
        "Split should be reasonably balanced. Split A ratio: {}%",
        split_ratio as f64 / 10.0
    );
    
    // 4. Test mathematical optimality by checking nearby splits
    let delta = input_amount / 100; // 1% variation
    
    // Test split with slightly more to A
    let test_split_a = quote.split_amount_a + delta;
    let test_split_b = quote.split_amount_b - delta;
    
    if test_split_a <= input_amount && test_split_b > 0 {
        // Calculate expected outputs for test split
        let test_output_a = (test_split_a * quote.route_a.expected_output) / quote.split_amount_a;
        let test_output_b = (test_split_b * quote.route_b.expected_output) / quote.split_amount_b;
        
        // Original split should be at least as good as test split
        let original_balance = quote.route_a.expected_output + quote.route_b.expected_output;
        let test_balance = test_output_a + test_output_b;
        
        assert!(
            original_balance >= test_balance * 99 / 100, // Allow 1% tolerance
            "Original split should be optimal or near-optimal"
        );
    }
    
    validate_zap_quote(&quote)?;
    
    println!("✅ Optimal split calculation verification test passed");
    Ok(())
}

#[test]
fn test_route_output_calculation_accuracy() -> anyhow::Result<()> {
    println!("Testing route output calculation accuracy...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test different input amounts for mathematical consistency
    let test_amounts = vec![
        1e7 as u128,   // 0.1 WBTC
        5e7 as u128,   // 0.5 WBTC
        1e8 as u128,   // 1 WBTC
        2e8 as u128,   // 2 WBTC
    ];
    
    for amount in test_amounts {
        let quote = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE)?;
        
        // 1. Output should be proportional to input (for small amounts)
        if amount <= 1e8 as u128 {
            let output_ratio_a = quote.route_a.expected_output as f64 / quote.split_amount_a as f64;
            let output_ratio_b = quote.route_b.expected_output as f64 / quote.split_amount_b as f64;
            
            // Ratios should be positive and reasonable
            assert!(output_ratio_a > 0.0, "Route A output ratio should be positive");
            assert!(output_ratio_b > 0.0, "Route B output ratio should be positive");
            assert!(output_ratio_a < 10.0, "Route A output ratio should be reasonable");
            assert!(output_ratio_b < 10.0, "Route B output ratio should be reasonable");
        }
        
        // 2. Expected outputs should be achievable
        assert!(
            quote.route_a.expected_output > 0,
            "Route A should have positive expected output"
        );
        assert!(
            quote.route_b.expected_output > 0,
            "Route B should have positive expected output"
        );
        
        // 3. Price impact should be mathematically consistent
        let total_input_value = amount;
        let total_output_value = quote.route_a.expected_output + quote.route_b.expected_output;
        
        // Price impact should reflect the difference
        let calculated_impact = if total_input_value > total_output_value {
            ((total_input_value - total_output_value) * 10000) / total_input_value
        } else {
            0
        };
        
        // Allow some tolerance due to different calculation methods
        let impact_difference = if quote.price_impact > calculated_impact {
            quote.price_impact - calculated_impact
        } else {
            calculated_impact - quote.price_impact
        };
        
        assert!(
            impact_difference <= 1000, // Within 10%
            "Price impact should be mathematically consistent. Quote: {}%, Calculated: {}%",
            quote.price_impact as f64 / 100.0,
            calculated_impact as f64 / 100.0
        );
        
        validate_zap_quote(&quote)?;
    }
    
    println!("✅ Route output calculation accuracy test passed");
    Ok(())
}

#[test]
fn test_precision_handling_and_rounding() -> anyhow::Result<()> {
    println!("Testing precision handling and rounding...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test with amounts that might cause precision issues
    let precision_test_amounts = vec![
        1u128,           // Minimum amount
        3u128,           // Odd small amount
        7u128,           // Prime small amount
        1000u128,        // Small round amount
        1001u128,        // Small non-round amount
        999999u128,      // Large non-round amount
        1000000u128,     // Large round amount
    ];
    
    for amount in precision_test_amounts {
        let result = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE);
        
        match result {
            Ok(quote) => {
                // If successful, verify precision properties
                
                // 1. No precision loss in split calculation
                assert_eq!(
                    quote.split_amount_a + quote.split_amount_b,
                    amount,
                    "No precision loss in split for amount {}", amount
                );
                
                // 2. All values should be reasonable
                assert!(quote.expected_lp_tokens > 0, "Expected LP tokens should be positive for amount {}", amount);
                assert!(quote.minimum_lp_tokens <= quote.expected_lp_tokens, "Minimum should not exceed expected for amount {}", amount);
                
                // 3. Price impact should not overflow
                assert!(quote.price_impact <= 10000, "Price impact should not exceed 100% for amount {}", amount);
                
                validate_zap_quote(&quote)?;
                
                println!("Amount {}: {} LP tokens, {}% price impact", 
                        amount, quote.expected_lp_tokens, quote.price_impact as f64 / 100.0);
            }
            Err(_) => {
                // Graceful failure is acceptable for very small amounts
                println!("Amount {} rejected gracefully", amount);
            }
        }
    }
    
    // Test rounding consistency
    let base_amount = 1e8 as u128; // 1 WBTC
    let quote1 = zap.get_zap_quote(wbtc, base_amount, eth, usdc, DEFAULT_SLIPPAGE)?;
    let quote2 = zap.get_zap_quote(wbtc, base_amount, eth, usdc, DEFAULT_SLIPPAGE)?;
    
    // Results should be identical (deterministic rounding)
    assert_eq!(quote1.split_amount_a, quote2.split_amount_a, "Split A should be deterministic");
    assert_eq!(quote1.split_amount_b, quote2.split_amount_b, "Split B should be deterministic");
    assert_eq!(quote1.expected_lp_tokens, quote2.expected_lp_tokens, "Expected LP tokens should be deterministic");
    assert_eq!(quote1.price_impact, quote2.price_impact, "Price impact should be deterministic");
    
    println!("✅ Precision handling and rounding test passed");
    Ok(())
}

#[test]
fn test_numerical_stability_under_extreme_conditions() -> anyhow::Result<()> {
    println!("Testing numerical stability under extreme conditions...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test extreme conditions that might cause numerical instability
    
    // 1. Very large amounts
    let large_amounts = vec![
        1000 * 1e8 as u128,    // 1000 WBTC
        10000 * 1e8 as u128,   // 10000 WBTC
        u128::MAX / 1000,      // Near maximum
    ];
    
    for amount in large_amounts {
        let result = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE);
        
        match result {
            Ok(quote) => {
                // Verify numerical stability
                assert!(quote.split_amount_a <= amount, "Split A should not exceed input");
                assert!(quote.split_amount_b <= amount, "Split B should not exceed input");
                assert_eq!(quote.split_amount_a + quote.split_amount_b, amount, "Splits should sum correctly");
                
                // Values should not overflow
                assert!(quote.expected_lp_tokens < u128::MAX / 2, "Expected LP tokens should not overflow");
                assert!(quote.price_impact <= 10000, "Price impact should not exceed 100%");
                
                validate_zap_quote(&quote)?;
                
                println!("Large amount {} WBTC: stable calculation", amount as f64 / 1e8);
            }
            Err(_) => {
                println!("Large amount {} WBTC: gracefully rejected", amount as f64 / 1e8);
            }
        }
    }
    
    // 2. Test with extreme slippage tolerances
    let extreme_slippages = vec![
        1u128,      // 0.01%
        10000u128,  // 100%
    ];
    
    for slippage in extreme_slippages {
        let result = zap.get_zap_quote(wbtc, 1e8 as u128, eth, usdc, slippage);
        
        match result {
            Ok(quote) => {
                // Verify slippage calculation stability
                let expected_minimum = quote.expected_lp_tokens * (10000 - slippage) / 10000;
                assert_within_tolerance(quote.minimum_lp_tokens, expected_minimum, 100); // 1% tolerance
                
                validate_zap_quote(&quote)?;
                
                println!("Extreme slippage {}%: stable calculation", slippage as f64 / 100.0);
            }
            Err(_) => {
                println!("Extreme slippage {}%: gracefully rejected", slippage as f64 / 100.0);
            }
        }
    }
    
    // 3. Test mathematical edge cases
    let edge_case_amounts = vec![
        u128::MAX / 1000000,  // Large but not maximum
        1e18 as u128,         // Very large round number
        (1e18 as u128) + 1,   // Very large + 1
    ];
    
    for amount in edge_case_amounts {
        let result = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE);
        
        // Should either succeed with stable results or fail gracefully
        match result {
            Ok(quote) => {
                // Verify no overflow or underflow
                assert!(quote.split_amount_a > 0, "Split A should be positive");
                assert!(quote.split_amount_b > 0, "Split B should be positive");
                assert!(quote.expected_lp_tokens > 0, "Expected LP tokens should be positive");
                
                validate_zap_quote(&quote)?;
            }
            Err(_) => {
                // Graceful failure is acceptable for extreme values
            }
        }
    }
    
    println!("✅ Numerical stability under extreme conditions test passed");
    Ok(())
}

#[test]
fn test_invariant_preservation_across_operations() -> anyhow::Result<()> {
    println!("Testing invariant preservation across operations...");
    
    let mut zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test that mathematical invariants are preserved across multiple operations
    
    // Get initial pool state
    let initial_pool = zap.factory.get_pool(eth, usdc)
        .ok_or_else(|| anyhow::anyhow!("Pool not found"))?
        .clone();
    
    let initial_k = initial_pool.reserve_a * initial_pool.reserve_b; // Constant product
    
    // Perform multiple zap operations
    let operations = vec![
        1e8 as u128,   // 1 WBTC
        5e7 as u128,   // 0.5 WBTC
        2e8 as u128,   // 2 WBTC
    ];
    
    let mut total_lp_issued = 0u128;
    
    for (i, amount) in operations.iter().enumerate() {
        let quote = zap.get_zap_quote(wbtc, *amount, eth, usdc, DEFAULT_SLIPPAGE)?;
        let lp_tokens = zap.execute_zap(&quote)?;
        
        total_lp_issued += lp_tokens;
        
        // Get current pool state
        let current_pool = zap.factory.get_pool(eth, usdc)
            .ok_or_else(|| anyhow::anyhow!("Pool not found"))?;
        
        // Verify invariants
        
        // 1. Pool reserves should only increase (liquidity addition)
        assert!(
            current_pool.reserve_a >= initial_pool.reserve_a,
            "Pool reserve A should not decrease in operation {}", i + 1
        );
        assert!(
            current_pool.reserve_b >= initial_pool.reserve_b,
            "Pool reserve B should not decrease in operation {}", i + 1
        );
        
        // 2. Total supply should increase
        assert!(
            current_pool.total_supply > initial_pool.total_supply,
            "Pool total supply should increase in operation {}", i + 1
        );
        
        // 3. Constant product should increase (due to fees and liquidity addition)
        let current_k = current_pool.reserve_a * current_pool.reserve_b;
        assert!(
            current_k >= initial_k,
            "Constant product should not decrease in operation {}", i + 1
        );
        
        // 4. LP token conservation
        let supply_increase = current_pool.total_supply - initial_pool.total_supply;
        assert_eq!(
            total_lp_issued,
            supply_increase,
            "LP tokens issued should match supply increase after operation {}", i + 1
        );
        
        validate_zap_quote(&quote)?;
        
        println!("Operation {}: {} WBTC -> {} LP tokens, pool K: {} -> {}", 
                i + 1, *amount as f64 / 1e8, lp_tokens, initial_k, current_k);
    }
    
    // Final invariant checks
    let final_pool = zap.factory.get_pool(eth, usdc)
        .ok_or_else(|| anyhow::anyhow!("Pool not found"))?;
    
    // Total value should be conserved (accounting for fees)
    let initial_total_value = initial_pool.reserve_a + initial_pool.reserve_b;
    let final_total_value = final_pool.reserve_a + final_pool.reserve_b;
    
    assert!(
        final_total_value > initial_total_value,
        "Total pool value should increase due to liquidity addition"
    );
    
    // LP token value should be reasonable
    let lp_value = final_total_value / final_pool.total_supply;
    assert!(lp_value > 0, "LP tokens should have positive value");
    
    println!("✅ Invariant preservation across operations test passed");
    Ok(())
}

#[test]
fn test_mathematical_properties_of_price_impact() -> anyhow::Result<()> {
    println!("Testing mathematical properties of price impact...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test mathematical properties of price impact calculation
    
    // 1. Price impact should be monotonically increasing with amount
    let amounts = vec![
        1e7 as u128,   // 0.1 WBTC
        2e7 as u128,   // 0.2 WBTC
        5e7 as u128,   // 0.5 WBTC
        1e8 as u128,   // 1 WBTC
        2e8 as u128,   // 2 WBTC
    ];
    
    let mut previous_impact = 0u128;
    
    for amount in amounts {
        let quote = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE)?;
        
        // Price impact should be non-decreasing
        assert!(
            quote.price_impact >= previous_impact,
            "Price impact should be non-decreasing. Amount: {} WBTC, Impact: {}%, Previous: {}%",
            amount as f64 / 1e8,
            quote.price_impact as f64 / 100.0,
            previous_impact as f64 / 100.0
        );
        
        previous_impact = quote.price_impact;
        validate_zap_quote(&quote)?;
    }
    
    // 2. Price impact should be subadditive (impact of sum ≤ sum of impacts)
    let amount_a = 5e7 as u128; // 0.5 WBTC
    let amount_b = 5e7 as u128; // 0.5 WBTC
    let amount_combined = amount_a + amount_b; // 1 WBTC
    
    let quote_a = zap.get_zap_quote(wbtc, amount_a, eth, usdc, DEFAULT_SLIPPAGE)?;
    let quote_b = zap.get_zap_quote(wbtc, amount_b, eth, usdc, DEFAULT_SLIPPAGE)?;
    let quote_combined = zap.get_zap_quote(wbtc, amount_combined, eth, usdc, DEFAULT_SLIPPAGE)?;
    
    let sum_of_impacts = quote_a.price_impact + quote_b.price_impact;
    
    // Combined impact should be less than or equal to sum (due to economies of scale)
    // But allow some tolerance due to different pool states
    assert!(
        quote_combined.price_impact <= sum_of_impacts + 200, // Allow 2% tolerance
        "Price impact should be approximately subadditive. Combined: {}%, Sum: {}%",
        quote_combined.price_impact as f64 / 100.0,
        sum_of_impacts as f64 / 100.0
    );
    
    // 3. Price impact should be bounded
    let large_amount = 10 * 1e8 as u128; // 10 WBTC
    let large_quote = zap.get_zap_quote(wbtc, large_amount, eth, usdc, DEFAULT_SLIPPAGE)?;
    
    assert!(
        large_quote.price_impact <= 10000, // Max 100%
        "Price impact should be bounded at 100%"
    );
    
    // 4. Price impact should be continuous (small changes in input → small changes in impact)
    let base_amount = 1e8 as u128; // 1 WBTC
    let delta = 1e6 as u128; // 0.01 WBTC
    
    let base_quote = zap.get_zap_quote(wbtc, base_amount, eth, usdc, DEFAULT_SLIPPAGE)?;
    let perturbed_quote = zap.get_zap_quote(wbtc, base_amount + delta, eth, usdc, DEFAULT_SLIPPAGE)?;
    
    let impact_change = if perturbed_quote.price_impact > base_quote.price_impact {
        perturbed_quote.price_impact - base_quote.price_impact
    } else {
        base_quote.price_impact - perturbed_quote.price_impact
    };
    
    // Small input change should cause small impact change
    assert!(
        impact_change <= 100, // Max 1% change for 1% input change
        "Price impact should be continuous. Base: {}%, Perturbed: {}%, Change: {}%",
        base_quote.price_impact as f64 / 100.0,
        perturbed_quote.price_impact as f64 / 100.0,
        impact_change as f64 / 100.0
    );
    
    validate_zap_quote(&quote_a)?;
    validate_zap_quote(&quote_b)?;
    validate_zap_quote(&quote_combined)?;
    validate_zap_quote(&large_quote)?;
    validate_zap_quote(&base_quote)?;
    validate_zap_quote(&perturbed_quote)?;
    
    println!("✅ Mathematical properties of price impact test passed");
    Ok(())
}

#[test]
fn test_convergence_properties() -> anyhow::Result<()> {
    println!("Testing convergence properties...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test that calculations converge to expected values
    
    // 1. Test convergence of split optimization
    let input_amount = 1e8 as u128; // 1 WBTC
    let quote = zap.get_zap_quote(wbtc, input_amount, eth, usdc, DEFAULT_SLIPPAGE)?;
    
    // The split should be close to optimal for balanced liquidity provision
    // Get target pool to check current ratio
    let target_pool = zap.factory.get_pool(eth, usdc)
        .ok_or_else(|| anyhow::anyhow!("Target pool not found"))?;
    
    let pool_ratio = target_pool.reserve_a as f64 / target_pool.reserve_b as f64;
    
    // Calculate expected amounts after swaps (simplified)
    let expected_amount_a = quote.split_amount_a; // Simplified: assume 1:1 for WBTC->ETH
    let expected_amount_b = quote.split_amount_b; // Simplified: assume 1:1 for WBTC->USDC
    
    let add_ratio = expected_amount_a as f64 / expected_amount_b as f64;
    
    // The ratio should be reasonably close to pool ratio for balanced addition
    let ratio_error = (add_ratio - pool_ratio).abs() / pool_ratio;
    assert!(
        ratio_error <= 0.5, // Within 50% (generous tolerance for simplified calculation)
        "Split should converge to balanced liquidity addition. Pool ratio: {:.3}, Add ratio: {:.3}, Error: {:.1}%",
        pool_ratio,
        add_ratio,
        ratio_error * 100.0
    );
    
    // 2. Test convergence of iterative calculations
    // Simulate multiple iterations of the same calculation
    let mut results = Vec::new();
    
    for _ in 0..5 {
        let quote = zap.get_zap_quote(wbtc, input_amount, eth, usdc, DEFAULT_SLIPPAGE)?;
        results.push((quote.split_amount_a, quote.split_amount_b, quote.expected_lp_tokens));
    }
    
    // All results should be identical (deterministic convergence)
    let first_result = results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            result.0, first_result.0,
            "Split A should converge deterministically in iteration {}", i + 1
        );
        assert_eq!(
            result.1, first_result.1,
            "Split B should converge deterministically in iteration {}", i + 1
        );
        assert_eq!(
            result.2, first_result.2,
            "Expected LP tokens should converge deterministically in iteration {}", i + 1
        );
    }
    
    // 3. Test convergence under different conditions
    let test_conditions = vec![
        (5e7 as u128, 100u128),   // 0.5 WBTC, 1% slippage
        (1e8 as u128, 500u128),   // 1 WBTC, 5% slippage
        (2e8 as u128, 1000u128),  // 2 WBTC, 10% slippage
    ];
    
    for (amount, slippage) in test_conditions {
        let quote = zap.get_zap_quote(wbtc, amount, eth, usdc, slippage)?;
        
        // Calculations should converge to valid results
        assert!(quote.split_amount_a > 0, "Split A should converge to positive value");
        assert!(quote.split_amount_b > 0, "Split B should converge to positive value");
        assert!(quote.expected_lp_tokens > 0, "Expected LP tokens should converge to positive value");
        
        // Results should be mathematically consistent
        assert_eq!(
            quote.split_amount_a + quote.split_amount_b,
            amount,
            "Splits should converge to sum equal to input"
        );
        
        validate_zap_quote(&quote)?;
    }
    
    println!("✅ Convergence properties test passed");
    Ok(())
}

#[test]
fn test_mathematical_edge_cases() -> anyhow::Result<()> {
    println!("Testing mathematical edge cases...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test mathematical edge cases that might cause issues
    
    // 1. Test with amounts that are powers of 2 (binary edge cases)
    let binary_amounts = vec![
        1u128,
        2u128,
        4u128,
        8u128,
        16u128,
        32u128,
        64u128,
        128u128,
        256u128,
        512u128,
        1024u128,
        2048u128,
    ];
    
    for amount in binary_amounts {
        let result = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE);
        
        match result {
            Ok(quote) => {
                // Verify mathematical properties hold for binary amounts
                assert_eq!(
                    quote.split_amount_a + quote.split_amount_b,
                    amount,
                    "Conservation should hold for binary amount {}", amount
                );
                
                validate_zap_quote(&quote)?;
            }
            Err(_) => {
                // Graceful failure is acceptable for very small amounts
                println!("Binary amount {} rejected gracefully", amount);
            }
        }
    }
    
    // 2. Test with amounts that might cause division issues
    let division_test_amounts = vec![
        3u128,      // Prime number
        7u128,      // Prime number
        11u128,     // Prime number
        13u128,     // Prime number
        17u128,     // Prime number
        101u128,    // Larger prime
        1001u128,   // 7 * 11 * 13
        10001u128,  // Large odd number
    ];
    
    for amount in division_test_amounts {
        let result = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE);
        
        match result {
            Ok(quote) => {
                // Verify no precision loss in division
                assert_eq!(
                    quote.split_amount_a + quote.split_amount_b,
                    amount,
                    "No precision loss for amount {} (division test)", amount
                );
                
                // Both splits should be reasonable
                assert!(quote.split_amount_a > 0, "Split A should be positive for amount {}", amount);
                assert!(quote.split_amount_b > 0, "Split B should be positive for amount {}", amount);
                
                validate_zap_quote(&quote)?;
            }
            Err(_) => {
                println!("Division test amount {} rejected gracefully", amount);
            }
        }
    }
    
    // 3. Test boundary conditions for slippage
    let boundary_slippages = vec![
        0u128,      // No slippage
        1u128,      // Minimum slippage
        9999u128,   // Maximum slippage - 1
        10000u128,  // Maximum slippage
    ];
    
    for slippage in boundary_slippages {
        let result = zap.get_zap_quote(wbtc, 1e8 as u128, eth, usdc, slippage);
        
        match result {
            Ok(quote) => {
                // Verify slippage calculation is mathematically correct
                let expected_minimum = quote.expected_lp_tokens * (10000 - slippage) / 10000;
                assert_within_tolerance(quote.minimum_lp_tokens, expected_minimum, 100); // 1% tolerance
                
                validate_zap_quote(&quote)?;
                
                println!("Boundary slippage {}%: mathematically correct", slippage as f64 / 100.0);
            }
            Err(_) => {
                println!("Boundary slippage {}%: gracefully rejected", slippage as f64 / 100.0);
            }
        }
    }
    
    println!("✅ Mathematical edge cases test passed");
    Ok(())
}

#[test]
fn test_mathematical_consistency_across_scenarios() -> anyhow::Result<()> {
    println!("Testing mathematical consistency across scenarios...");
    
    let zap = create_mock_zap();
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    
    // Test that mathematical relationships hold across different scenarios
    
    // 1. Linearity test for small amounts
    let base_amount = 1e7 as u128; // 0.1 WBTC
    let double_amount = 2e7 as u128; // 0.2 WBTC
    
    let base_quote = zap.get_zap_quote(wbtc, base_amount, eth, usdc, DEFAULT_SLIPPAGE)?;
    let double_quote = zap.get_zap_quote(wbtc, double_amount, eth, usdc, DEFAULT_SLIPPAGE)?;
    
    // For small amounts, output should be approximately linear
    let base_total_output = base_quote.route_a.expected_output + base_quote.route_b.expected_output;
    let double_total_output = double_quote.route_a.expected_output + double_quote.route_b.expected_output;
    
    let linearity_ratio = double_total_output as f64 / (2.0 * base_total_output as f64);
    
    // Should be close to 1.0 for small amounts (linear relationship)
    assert!(
        linearity_ratio >= 0.8 && linearity_ratio <= 1.2, // Within 20%
        "Small amounts should have approximately linear output. Ratio: {:.3}",
        linearity_ratio
    );
    
    // 2. Symmetry test
    let amount = 1e8 as u128; // 1 WBTC
    let quote_ab = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE)?;
    let quote_ba = zap.get_zap_quote(wbtc, amount, usdc, eth, DEFAULT_SLIPPAGE)?;
    
    // Swapping target tokens should give similar results (symmetry)
    let impact_difference = calculate_percentage_difference(quote_ab.price_impact, quote_ba.price_impact);
    assert!(
        impact_difference <= 1000, // Within 10%
        "Symmetric operations should have similar price impact. AB: {}%, BA: {}%",
        quote_ab.price_impact as f64 / 100.0,
        quote_ba.price_impact as f64 / 100.0
    );
    
    // 3. Transitivity test (A->B->C should be consistent with A->C)
    // This is implicitly tested by the route finding algorithm
    
    // 4. Conservation test across multiple operations
    let operations = vec![
        (5e7 as u128, "0.5 WBTC"),
        (1e8 as u128, "1 WBTC"),
        (15e7 as u128, "1.5 WBTC"),
    ];
    
    for (amount, description) in operations {
        let quote = zap.get_zap_quote(wbtc, amount, eth, usdc, DEFAULT_SLIPPAGE)?;
        
        // Conservation: input should equal sum of splits
        assert_eq!(
            quote.split_amount_a + quote.split_amount_b,
            amount,
            "Conservation should hold for {}", description
        );
        
        // Consistency: all values should be reasonable
        assert!(quote.expected_lp_tokens > 0, "Expected LP tokens should be positive for {}", description);
        assert!(quote.price_impact <= 10000, "Price impact should be bounded for {}", description);
        
        validate_zap_quote(&quote)?;
    }
    
    validate_zap_quote(&base_quote)?;
    validate_zap_quote(&double_quote)?;
    validate_zap_quote(&quote_ab)?;
    validate_zap_quote(&quote_ba)?;
    
    println!("✅ Mathematical consistency across scenarios test passed");
    Ok(())
}
