use crate::types::{RouteInfo, ZapQuote, PoolReserves, U256, BASIS_POINTS};
use crate::pool_provider::PoolProvider;
use crate::route_finder::RouteFinder;
use crate::amm_logic;
use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};

pub struct ZapCalculator;

impl ZapCalculator {
    /// Calculate optimal split of input token for balanced LP provision
    pub fn calculate_optimal_split<P: PoolProvider>(
        input_amount: u128,
        route_a: &RouteInfo,
        route_b: &RouteInfo,
        target_pool_reserves: &PoolReserves,
        route_finder: &RouteFinder<P>,
    ) -> Result<(u128, u128)> {
        if input_amount == 0 {
            return Err(anyhow!("Input amount cannot be zero"));
        }

        // Get the current ratio of the target pool
        let pool_ratio = Self::get_pool_ratio(target_pool_reserves)?;
        
        // Use binary search to find optimal split
        Self::binary_search_optimal_split(
            input_amount,
            route_a,
            route_b,
            pool_ratio,
            route_finder,
        )
    }

    /// Get the ratio of token A to token B in the target pool
    fn get_pool_ratio(pool_reserves: &PoolReserves) -> Result<U256> {
        if pool_reserves.reserve_b == 0 {
            return Err(anyhow!("Pool reserve B cannot be zero"));
        }

        Ok(U256::from(pool_reserves.reserve_a) * U256::from(1_000_000_000_000_000_000u128) / U256::from(pool_reserves.reserve_b))
    }

    /// Use binary search to find the optimal split that results in balanced LP provision
    fn binary_search_optimal_split<P: PoolProvider>(
        input_amount: u128,
        route_a: &RouteInfo,
        route_b: &RouteInfo,
        target_ratio: U256,
        route_finder: &RouteFinder<P>,
    ) -> Result<(u128, u128)> {
        let mut left = 0u128;
        let mut right = input_amount;
        let mut best_split = (input_amount / 2, input_amount / 2);
        let mut best_balance_score = U256::MAX;

        // Binary search for optimal split
        for _ in 0..50 { // Limit iterations to prevent infinite loops
            let mid = (left + right) / 2;
            let split_a = mid;
            let split_b = input_amount - mid;

            if split_a == 0 || split_b == 0 {
                if left >= right {
                    break;
                }
                if split_a == 0 {
                    left = mid + 1;
                } else {
                    right = mid - 1;
                }
                continue;
            }

            // Calculate expected outputs
            let expected_a = Self::calculate_route_output(split_a, route_a, route_finder)?;
            let expected_b = Self::calculate_route_output(split_b, route_b, route_finder)?;

            // Calculate how balanced this split would be
            let balance_score = Self::calculate_balance_score(expected_a, expected_b, target_ratio)?;

            if balance_score < best_balance_score {
                best_balance_score = balance_score;
                best_split = (split_a, split_b);
            }

            // Adjust search range based on balance
            let current_ratio = if expected_b == 0 {
                U256::MAX
            } else {
                U256::from(expected_a) * U256::from(1_000_000_000_000_000_000u128) / U256::from(expected_b)
            };

            if current_ratio > target_ratio {
                // Too much A, reduce split_a
                right = mid.saturating_sub(1);
            } else {
                // Too little A, increase split_a
                left = mid + 1;
            }

            if left >= right {
                break;
            }
        }

        Ok(best_split)
    }

    /// Calculate how balanced the outputs are compared to the target ratio
    fn calculate_balance_score(output_a: u128, output_b: u128, target_ratio: U256) -> Result<U256> {
        if output_b == 0 {
            return Ok(U256::MAX);
        }

        let actual_ratio = U256::from(output_a) * U256::from(1_000_000_000_000_000_000u128) / U256::from(output_b);
        
        let diff = if actual_ratio > target_ratio {
            actual_ratio - target_ratio
        } else {
            target_ratio - actual_ratio
        };

        Ok(diff)
    }

    /// Calculate expected LP tokens from adding liquidity
    pub fn calculate_expected_lp_tokens(
        amount_a: u128,
        amount_b: u128,
        pool_reserves: &PoolReserves,
    ) -> Result<u128> {
        amm_logic::calculate_lp_tokens_minted(
            amount_a,
            amount_b,
            pool_reserves.reserve_a,
            pool_reserves.reserve_b,
            pool_reserves.total_supply,
        )
    }

    /// Calculate minimum LP tokens considering slippage
    pub fn calculate_minimum_lp_tokens(
        expected_lp_tokens: u128,
        slippage_tolerance_bps: u128,
    ) -> Result<u128> {
        if slippage_tolerance_bps > BASIS_POINTS {
            return Err(anyhow!("Slippage tolerance cannot exceed 100%"));
        }

        let slippage_multiplier = BASIS_POINTS - slippage_tolerance_bps;
        let minimum_lp = U256::from(expected_lp_tokens) * U256::from(slippage_multiplier) / U256::from(BASIS_POINTS);
        
        Ok(minimum_lp.try_into().map_err(|_| anyhow!("Minimum LP token amount exceeds u128"))?)
    }

    /// Generate a complete zap quote
    pub fn generate_zap_quote<P: PoolProvider>(
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        route_a: RouteInfo,
        route_b: RouteInfo,
        target_pool_reserves: &PoolReserves,
        slippage_tolerance_bps: u128,
        route_finder: &RouteFinder<P>,
    ) -> Result<ZapQuote> {
        // Calculate optimal split
        let (split_a, split_b) = Self::calculate_optimal_split(
            input_amount,
            &route_a,
            &route_b,
            target_pool_reserves,
            route_finder,
        )?;

        // Calculate expected outputs after swaps
        let expected_output_a = Self::calculate_route_output(split_a, &route_a, route_finder)?;
        let expected_output_b = Self::calculate_route_output(split_b, &route_b, route_finder)?;

        // Calculate expected LP tokens
        let expected_lp_tokens = Self::calculate_expected_lp_tokens(
            expected_output_a,
            expected_output_b,
            target_pool_reserves,
        )?;

        // Calculate minimum LP tokens with slippage protection
        let minimum_lp_tokens = Self::calculate_minimum_lp_tokens(
            expected_lp_tokens,
            slippage_tolerance_bps,
        )?;

        // Calculate overall price impact
        let price_impact = Self::calculate_overall_price_impact(&route_a, &route_b, split_a, split_b, route_finder)?;

        Ok(ZapQuote::new(input_token, input_amount, target_token_a, target_token_b)
            .with_routes(route_a, route_b)
            .with_split(split_a, split_b)
            .with_lp_estimate(expected_lp_tokens, minimum_lp_tokens)
            .with_price_impact(price_impact))
    }

    /// Calculate the actual output for a route given an input amount
    fn calculate_route_output<P: PoolProvider>(
        input_amount: u128,
        route: &RouteInfo,
        route_finder: &RouteFinder<P>,
    ) -> Result<u128> {
        if route.path.is_empty() {
            return Err(anyhow!("Route path cannot be empty"));
        }

        if route.path.len() == 1 {
            return Ok(input_amount);
        }

        let mut current_amount = input_amount;
        for i in 0..route.path.len() - 1 {
            let token_in = route.path[i];
            let token_out = route.path[i + 1];
            let pool = route_finder
                .pool_provider
                .get_pool_reserves(token_in, token_out)?;

            let (reserve_in, reserve_out) = if pool.token_a == token_in {
                (pool.reserve_a, pool.reserve_b)
            } else {
                (pool.reserve_b, pool.reserve_a)
            };

            current_amount = amm_logic::calculate_swap_out(current_amount, reserve_in, reserve_out, pool.fee_rate)?;
        }

        Ok(current_amount)
    }

    /// Calculate overall price impact from both routes
    fn calculate_overall_price_impact<P: PoolProvider>(
        route_a: &RouteInfo,
        route_b: &RouteInfo,
        split_a: u128,
        split_b: u128,
        route_finder: &RouteFinder<P>,
    ) -> Result<u128> {
        let total_input = U256::from(split_a) + U256::from(split_b);
        if total_input.is_zero() {
            return Ok(0);
        }

        let impact_a = Self::calculate_route_price_impact(split_a, route_a, route_finder)?;
        let impact_b = Self::calculate_route_price_impact(split_b, route_b, route_finder)?;

        // Weight the price impacts by the split amounts
        let weighted_impact_a = U256::from(impact_a) * U256::from(split_a) / total_input;
        let weighted_impact_b = U256::from(impact_b) * U256::from(split_b) / total_input;
        
        let total_impact = weighted_impact_a + weighted_impact_b;
        Ok(total_impact.try_into().map_err(|_| anyhow!("Price impact amount exceeds u128"))?)
    }

    fn calculate_route_price_impact<P: PoolProvider>(
        input_amount: u128,
        route: &RouteInfo,
        route_finder: &RouteFinder<P>,
    ) -> Result<u128> {
        let mut total_impact = U256::from(0);
        let mut current_amount = input_amount;

        for i in 0..route.path.len() - 1 {
            let token_in = route.path[i];
            let token_out = route.path[i + 1];
            let pool = route_finder.pool_provider.get_pool_reserves(token_in, token_out)?;

            let (reserve_in, reserve_out) = if pool.token_a == token_in {
                (pool.reserve_a, pool.reserve_b)
            } else {
                (pool.reserve_b, pool.reserve_a)
            };

            let amount_out = amm_logic::calculate_swap_out(current_amount, reserve_in, reserve_out, pool.fee_rate)?;
            let impact = amm_logic::calculate_price_impact(current_amount, reserve_in, amount_out, reserve_out)?;
            total_impact += U256::from(impact);
            current_amount = amount_out;
        }

        Ok(total_impact.try_into()?)
    }

    /// Validate that a zap quote is reasonable
    pub fn validate_zap_quote(quote: &ZapQuote) -> Result<()> {
        quote.validate()?;

        // Additional validations
        if quote.expected_lp_tokens == 0 {
            return Err(anyhow!("Expected LP tokens cannot be zero"));
        }

        if quote.minimum_lp_tokens > quote.expected_lp_tokens {
            return Err(anyhow!("Minimum LP tokens cannot exceed expected LP tokens"));
        }

        if quote.price_impact > 5000 { // 50% price impact threshold
            return Err(anyhow!("Price impact too high: {}%", quote.price_impact as f64 / 100.0));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route_finder::RouteFinder;
    use crate::pool_provider::PoolProvider;
    use std::collections::HashMap;

    struct MockPoolProvider {
        pools: HashMap<(AlkaneId, AlkaneId), PoolReserves>,
    }

    impl PoolProvider for MockPoolProvider {
        fn get_pool_reserves(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<PoolReserves> {
            let key1 = (token_a, token_b);
            let key2 = (token_b, token_a);
            self.pools.get(&key1).or_else(|| self.pools.get(&key2)).cloned().ok_or_else(|| anyhow!("Pool not found"))
        }

        fn get_connected_tokens(&self, token: AlkaneId) -> Result<Vec<AlkaneId>> {
            let mut connected = Vec::new();
            for (pool_tokens, _) in &self.pools {
                if pool_tokens.0 == token {
                    connected.push(pool_tokens.1);
                } else if pool_tokens.1 == token {
                    connected.push(pool_tokens.0);
                }
            }
            Ok(connected)
        }
    }

    fn create_mock_route(output: u128) -> RouteInfo {
        RouteInfo::new(
            vec![
                AlkaneId { block: 1, tx: 1 },
                AlkaneId { block: 2, tx: 2 },
            ],
            output,
        )
    }

    fn create_mock_pool_reserves() -> PoolReserves {
        PoolReserves::new(
            AlkaneId { block: 1, tx: 1 },
            AlkaneId { block: 2, tx: 2 },
            1_000_000 * 1_000_000_000_000_000_000,
            2_000_000 * 1_000_000_000_000_000_000,
            1_414_213 * 1_000_000_000_000_000_000,
            50, // fee_rate
        )
    }

    #[test]
    fn test_calculate_expected_lp_tokens_new_pool() {
        let result = ZapCalculator::calculate_expected_lp_tokens(
            1000,
            2000,
            &PoolReserves::new(
                AlkaneId { block: 1, tx: 1 },
                AlkaneId { block: 2, tx: 2 },
                0,
                0,
                0,
                50,
            ),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1414);
    }

    #[test]
    fn test_calculate_expected_lp_tokens_existing_pool() {
        let pool_reserves = create_mock_pool_reserves();
        let result = ZapCalculator::calculate_expected_lp_tokens(1000, 2000, &pool_reserves);
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn test_calculate_minimum_lp_tokens() {
        let result = ZapCalculator::calculate_minimum_lp_tokens(1000, 500); // 5% slippage
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 950);
    }

    #[test]
    fn test_calculate_optimal_split() {
        let route_a = create_mock_route(1000);
        let route_b = create_mock_route(2000);
        let pool_reserves = create_mock_pool_reserves();
        let mut pools = HashMap::new();
        pools.insert(
            (
                AlkaneId { block: 1, tx: 1 },
                AlkaneId { block: 2, tx: 2 },
            ),
            pool_reserves.clone(),
        );
        let mock_pool_provider = MockPoolProvider { pools };
        let factory_id = AlkaneId { block: 1, tx: 0 };
        let route_finder = RouteFinder::new(factory_id, &mock_pool_provider);

        let result = ZapCalculator::calculate_optimal_split(1000, &route_a, &route_b, &pool_reserves, &route_finder);
        assert!(result.is_ok());
        
        let (split_a, split_b) = result.unwrap();
        assert_eq!(split_a + split_b, 1000);
        assert!(split_a > 0);
        assert!(split_b > 0);
    }

    #[test]
    fn test_generate_zap_quote() {
        let input_token = AlkaneId { block: 1, tx: 1 };
        let target_token_a = AlkaneId { block: 2, tx: 2 };
        let target_token_b = AlkaneId { block: 3, tx: 3 };
        let route_a = create_mock_route(1000);
        let route_b = create_mock_route(2000);
        let pool_reserves = create_mock_pool_reserves();

        let factory_id = AlkaneId { block: 1, tx: 0 };
        let mut pools = HashMap::new();
        pools.insert(
            (
                AlkaneId { block: 1, tx: 1 },
                AlkaneId { block: 2, tx: 2 },
            ),
            pool_reserves.clone(),
        );
        let mock_pool_provider = MockPoolProvider { pools };
        let route_finder = RouteFinder::new(factory_id, &mock_pool_provider);
        let result = ZapCalculator::generate_zap_quote(
            input_token,
            1000,
            target_token_a,
            target_token_b,
            route_a,
            route_b,
            &pool_reserves,
            500, // 5% slippage
            &route_finder,
        );

        assert!(result.is_ok());
        let quote = result.unwrap();
        assert_eq!(quote.input_amount, 1000);
        assert!(quote.expected_lp_tokens > 0);
        assert!(quote.minimum_lp_tokens > 0);
        assert!(quote.minimum_lp_tokens <= quote.expected_lp_tokens);
    }
}
