// Integration tests with indexer (similar to boiler testing suite)
pub mod zap_integration_test;
pub mod test_runner;

#[cfg(test)]
mod zap_tests {
    use super::*;
    use alkanes_support::id::AlkaneId;
    use oyl_zap_core::{types::*, route_finder::*, zap_calculator::*};

    fn create_test_alkane_id(block: u128, tx: u128) -> AlkaneId {
        AlkaneId { block, tx }
    }

    #[test]
    fn test_route_info_creation() {
        let token_a = create_test_alkane_id(1, 1);
        let token_b = create_test_alkane_id(2, 2);
        let path = vec![token_a, token_b];
        
        let route = RouteInfo::new(path.clone(), 1000);
        
        assert_eq!(route.path, path);
        assert_eq!(route.expected_output, 1000);
        assert_eq!(route.price_impact, 0);
        assert_eq!(route.gas_estimate, 0);
        assert!(route.is_direct_route());
        assert_eq!(route.hop_count(), 1);
    }

    #[test]
    fn test_zap_quote_creation() {
        let input_token = create_test_alkane_id(1, 1);
        let target_token_a = create_test_alkane_id(2, 2);
        let target_token_b = create_test_alkane_id(3, 3);
        
        let quote = ZapQuote::new(input_token, 1000, target_token_a, target_token_b);
        
        assert_eq!(quote.input_token, input_token);
        assert_eq!(quote.input_amount, 1000);
        assert_eq!(quote.target_token_a, target_token_a);
        assert_eq!(quote.target_token_b, target_token_b);
        assert_eq!(quote.expected_lp_tokens, 0);
    }

    #[test]
    fn test_pool_reserves() {
        let token_a = create_test_alkane_id(1, 1);
        let token_b = create_test_alkane_id(2, 2);
        
        let reserves = PoolReserves::new(token_a, token_b, 1000, 2000, 1414, 50);
        
        assert_eq!(reserves.get_reserve_for_token(&token_a), Some(1000));
        assert_eq!(reserves.get_reserve_for_token(&token_b), Some(2000));
        assert_eq!(reserves.get_reserve_for_token(&create_test_alkane_id(3, 3)), None);
        
        let price_ratio = reserves.get_price_ratio();
        assert!(price_ratio.is_ok());
    }

    #[test]
    fn test_zap_params_validation() {
        let input_token = create_test_alkane_id(1, 1);
        let target_token_a = create_test_alkane_id(2, 2);
        let target_token_b = create_test_alkane_id(3, 3);
        
        let params = ZapParams::new(input_token, 1000, target_token_a, target_token_b, 950, 1640995500);
        
        // Valid params should pass
        assert!(params.validate(1640995200).is_ok());
        
        // Expired deadline should fail
        assert!(params.validate(1640995600).is_err());
        
        // Same input and target token should fail
        let invalid_params = ZapParams::new(input_token, 1000, input_token, target_token_b, 950, 1640995500);
        assert!(invalid_params.validate(1640995200).is_err());
    }

    #[test]
    fn test_route_finder_creation() {
        let factory_id = create_test_alkane_id(1, 1);
        let base_tokens = vec![
            create_test_alkane_id(2, 2),
            create_test_alkane_id(3, 3),
        ];
        
        let pool_provider = MockPoolProvider::new();
        let finder = RouteFinder::new(factory_id, &pool_provider).with_base_tokens(base_tokens.clone());
        
        assert_eq!(finder.oyl_factory_id, factory_id);
        assert_eq!(finder.common_base_tokens, base_tokens);
    }

    #[test]
    fn test_zap_calculator_lp_tokens() {
        // Test new pool
        let result = ZapCalculator::calculate_expected_lp_tokens(
            1000,
            2000,
            &PoolReserves::new(
                create_test_alkane_id(1, 1),
                create_test_alkane_id(2, 2),
                0,
                0,
                0,
                50
            ),
        );
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);

        // Test existing pool
        let result = ZapCalculator::calculate_expected_lp_tokens(
            1000,
            2000,
            &PoolReserves::new(
                create_test_alkane_id(1, 1),
                create_test_alkane_id(2, 2),
                1000000,
                2000000,
                1414213,
                50
            ),
        );
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn test_minimum_lp_tokens_calculation() {
        let result = ZapCalculator::calculate_minimum_lp_tokens(1000, 500); // 5% slippage
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 950);

        // Test invalid slippage
        let result = ZapCalculator::calculate_minimum_lp_tokens(1000, 15000); // 150% slippage
        assert!(result.is_err());
    }

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_FEE_AMOUNT_PER_1000, 5);
        assert_eq!(MAX_HOPS, 3);
        assert_eq!(BASIS_POINTS, 10000);
        assert_eq!(MINIMUM_LIQUIDITY, 1000);
    }
}

// MockPoolProvider for testing RouteFinder
use oyl_zap_core::pool_provider::PoolProvider;
use std::collections::HashMap;
use anyhow::{anyhow, Result};
use alkanes_support::id::AlkaneId;
use oyl_zap_core::types::PoolReserves;

struct MockPoolProvider {
    pools: HashMap<(AlkaneId, AlkaneId), PoolReserves>,
}

impl MockPoolProvider {
    fn new() -> Self {
        Self {
            pools: HashMap::new(),
        }
    }
}

impl PoolProvider for MockPoolProvider {
    fn get_pool_reserves(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<PoolReserves> {
        let key1 = (token_a, token_b);
        let key2 = (token_b, token_a);
        self.pools.get(&key1).or_else(|| self.pools.get(&key2)).cloned().ok_or_else(|| anyhow!("Pool not found"))
    }

    fn get_connected_tokens(&self, _token: AlkaneId) -> Result<Vec<AlkaneId>> {
        Ok(vec![])
    }
}
