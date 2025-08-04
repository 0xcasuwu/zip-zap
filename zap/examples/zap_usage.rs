//! Example usage of the OYL Zap contract
//! 
//! This example demonstrates how to:
//! 1. Initialize the zap contract
//! 2. Get a quote for zapping into an LP position
//! 3. Execute the zap operation
//! 4. Manage base tokens

use alkanes_support::id::AlkaneId;
use oyl_zap_core::{types::*, OylZapBase, OylZap};

fn main() {
    println!("OYL Zap Contract Usage Example");
    println!("==============================");

    // Example token IDs (these would be real alkane IDs in practice)
    let oyl_factory_id = AlkaneId { block: 1, tx: 1 };
    let usdc_token = AlkaneId { block: 2, tx: 2 };
    let wbtc_token = AlkaneId { block: 3, tx: 3 };
    let eth_token = AlkaneId { block: 4, tx: 4 };
    let target_token_a = AlkaneId { block: 5, tx: 5 }; // Some token A
    let target_token_b = AlkaneId { block: 6, tx: 6 }; // Some token B

    // Base tokens for routing (commonly traded tokens)
    let base_tokens = vec![usdc_token, wbtc_token, eth_token];

    println!("\n1. Initializing Zap Contract");
    println!("   Factory ID: {:?}", oyl_factory_id);
    println!("   Base tokens: {:?}", base_tokens);

    // Create zap instance
    let mut zap = OylZap::default();

    // Initialize the zap contract (this would be done via message dispatch in practice)
    match zap.init_zap(oyl_factory_id, base_tokens.clone()) {
        Ok(_) => println!("   ✓ Zap contract initialized successfully"),
        Err(e) => println!("   ✗ Failed to initialize: {}", e),
    }

    println!("\n2. Getting Zap Quote");
    let input_token = usdc_token;
    let input_amount = 1000_000000; // 1000 USDC (6 decimals)
    let max_slippage_bps = 500; // 5% slippage tolerance

    println!("   Input: {} units of {:?}", input_amount, input_token);
    println!("   Target LP: {:?} / {:?}", target_token_a, target_token_b);
    println!("   Max slippage: {}%", max_slippage_bps as f64 / 100.0);

    // Get quote (this would return serialized data in practice)
    match zap.get_zap_quote(input_token, input_amount, target_token_a, target_token_b, max_slippage_bps) {
        Ok(_) => println!("   ✓ Quote generated successfully"),
        Err(e) => println!("   ✗ Failed to get quote: {}", e),
    }

    println!("\n3. Finding Optimal Routes");
    
    // Find route from USDC to target token A
    match zap.find_optimal_route(input_token, target_token_a, input_amount / 2) {
        Ok(_) => println!("   ✓ Route A found: {:?} -> {:?}", input_token, target_token_a),
        Err(e) => println!("   ✗ Failed to find route A: {}", e),
    }

    // Find route from USDC to target token B
    match zap.find_optimal_route(input_token, target_token_b, input_amount / 2) {
        Ok(_) => println!("   ✓ Route B found: {:?} -> {:?}", input_token, target_token_b),
        Err(e) => println!("   ✗ Failed to find route B: {}", e),
    }

    println!("\n4. Executing Zap Operation");
    let min_lp_tokens = 950_000000; // Minimum LP tokens expected (95% of estimated)
    let deadline = 1640995500; // Unix timestamp

    println!("   Minimum LP tokens: {}", min_lp_tokens);
    println!("   Deadline: {}", deadline);

    match zap.zap_into_lp(
        input_token,
        input_amount,
        target_token_a,
        target_token_b,
        min_lp_tokens,
        deadline,
    ) {
        Ok(_) => println!("   ✓ Zap executed successfully!"),
        Err(e) => println!("   ✗ Zap execution failed: {}", e),
    }

    println!("\n5. Managing Base Tokens");
    
    // Add a new base token
    let new_base_token = AlkaneId { block: 7, tx: 7 };
    match zap.add_base_token(new_base_token) {
        Ok(_) => println!("   ✓ Added base token: {:?}", new_base_token),
        Err(e) => println!("   ✗ Failed to add base token: {}", e),
    }

    // Get current base tokens
    match zap.get_base_tokens() {
        Ok(_) => println!("   ✓ Retrieved current base tokens"),
        Err(e) => println!("   ✗ Failed to get base tokens: {}", e),
    }

    // Remove a base token
    match zap.remove_base_token(new_base_token) {
        Ok(_) => println!("   ✓ Removed base token: {:?}", new_base_token),
        Err(e) => println!("   ✗ Failed to remove base token: {}", e),
    }

    println!("\n6. Getting Configuration");
    match zap.get_zap_config() {
        Ok(_) => println!("   ✓ Retrieved zap configuration"),
        Err(e) => println!("   ✗ Failed to get configuration: {}", e),
    }

    println!("\n7. Updating Factory");
    let new_factory_id = AlkaneId { block: 10, tx: 10 };
    match zap.set_oyl_factory(new_factory_id) {
        Ok(_) => println!("   ✓ Updated factory to: {:?}", new_factory_id),
        Err(e) => println!("   ✗ Failed to update factory: {}", e),
    }

    println!("\nExample completed!");
    println!("\nKey Features Demonstrated:");
    println!("- ✓ Contract initialization with factory and base tokens");
    println!("- ✓ Quote generation for zap operations");
    println!("- ✓ Optimal route discovery for token swaps");
    println!("- ✓ Single-sided LP provision execution");
    println!("- ✓ Base token management (add/remove)");
    println!("- ✓ Configuration retrieval and updates");
    
    println!("\nIntegration Notes:");
    println!("- In practice, these operations would be called via message dispatch");
    println!("- The contract would interact with real OYL factory and pool contracts");
    println!("- Route discovery would query actual pool reserves and liquidity");
    println!("- Swap execution would perform real token transfers and LP minting");
}

#[cfg(test)]
mod example_tests {
    use super::*;

    #[test]
    fn test_example_token_creation() {
        let token = AlkaneId { block: 1, tx: 1 };
        assert_eq!(token.block, 1);
        assert_eq!(token.tx, 1);
    }

    #[test]
    fn test_zap_params_creation() {
        let input_token = AlkaneId { block: 1, tx: 1 };
        let target_token_a = AlkaneId { block: 2, tx: 2 };
        let target_token_b = AlkaneId { block: 3, tx: 3 };
        
        let params = ZapParams::new(
            input_token,
            1000,
            target_token_a,
            target_token_b,
            950,
            1640995500,
        );
        
        assert_eq!(params.input_amount, 1000);
        assert_eq!(params.min_lp_tokens, 950);
        assert_eq!(params.max_slippage_bps, 500); // Default 5%
    }

    #[test]
    fn test_route_info_properties() {
        let token_a = AlkaneId { block: 1, tx: 1 };
        let token_b = AlkaneId { block: 2, tx: 2 };
        let token_c = AlkaneId { block: 3, tx: 3 };
        
        // Direct route
        let direct_route = RouteInfo::new(vec![token_a, token_b], 1000);
        assert!(direct_route.is_direct_route());
        assert_eq!(direct_route.hop_count(), 1);
        
        // Multi-hop route
        let multi_hop_route = RouteInfo::new(vec![token_a, token_b, token_c], 900);
        assert!(!multi_hop_route.is_direct_route());
        assert_eq!(multi_hop_route.hop_count(), 2);
    }
}
