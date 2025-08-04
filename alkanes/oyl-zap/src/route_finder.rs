use crate::pool_provider::PoolProvider;
use crate::types::{RouteInfo, U256, MAX_HOPS};
use crate::amm_logic;
use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};
use std::collections::{HashSet, VecDeque};

pub struct RouteFinder<'a, P: PoolProvider> {
    pub oyl_factory_id: AlkaneId,
    pub common_base_tokens: Vec<AlkaneId>,
    pub pool_provider: &'a P,
    pub excluded_intermediate_tokens: HashSet<AlkaneId>,
}

impl<'a, P: PoolProvider> RouteFinder<'a, P> {
    pub fn new(oyl_factory_id: AlkaneId, pool_provider: &'a P) -> Self {
        Self {
            oyl_factory_id,
            common_base_tokens: Vec::new(),
            pool_provider,
            excluded_intermediate_tokens: HashSet::new(),
        }
    }

    pub fn with_base_tokens(mut self, base_tokens: Vec<AlkaneId>) -> Self {
        self.common_base_tokens = base_tokens;
        self
    }

    /// Exclude these tokens from being used as intermediate hops in a route.
    pub fn with_excluded_intermediate_tokens(mut self, tokens: &[AlkaneId]) -> Self {
        self.excluded_intermediate_tokens = tokens.iter().cloned().collect();
        self
    }

    /// Find the best route from input token to target token
    pub fn find_best_route(
        &self,
        from_token: AlkaneId,
        to_token: AlkaneId,
        amount_in: u128,
    ) -> Result<RouteInfo> {
        if from_token == to_token {
            return Err(anyhow!("Cannot route from token to itself"));
        }
        if amount_in == 0 {
            return Err(anyhow!("Input amount cannot be zero"));
        }

        let all_routes = self.find_all_routes(from_token, to_token, amount_in)?;
        
        all_routes
            .into_iter()
            .max_by(|a, b| a.expected_output.cmp(&b.expected_output))
            .ok_or_else(|| anyhow!("No route found from {:?} to {:?}", from_token, to_token))
    }

    fn find_all_routes(
        &self,
        from_token: AlkaneId,
        to_token: AlkaneId,
        amount_in: u128,
    ) -> Result<Vec<RouteInfo>> {
        let mut routes = Vec::new();

        // Direct route
        if let Ok(reserves) = self.pool_provider.get_pool_reserves(from_token, to_token) {
            let (reserve_in, reserve_out) = if reserves.token_a == from_token {
                (reserves.reserve_a, reserves.reserve_b)
            } else {
                (reserves.reserve_b, reserves.reserve_a)
            };
            if let Ok(amount_out) = amm_logic::calculate_swap_out(amount_in, reserve_in, reserve_out, 500) {
                let impact = amm_logic::calculate_price_impact(amount_in, reserve_in, amount_out, reserve_out)?;
                routes.push(RouteInfo::new(vec![from_token, to_token], amount_out).with_price_impact(impact));
            }
        }

        // Single-hop routes
        for base_token in &self.common_base_tokens {
            if *base_token == from_token || *base_token == to_token {
                continue;
            }
            // Ensure the intermediate base token is not in the exclusion list.
            if self.excluded_intermediate_tokens.contains(base_token) {
                continue;
            }
            if let Ok(route) = self.find_single_hop_route(from_token, to_token, *base_token, amount_in) {
                routes.push(route);
            }
        }
        
        // Multi-hop routes
        if let Ok(multi_hop_routes) = self.find_multi_hop_routes(from_token, to_token, amount_in) {
            routes.extend(multi_hop_routes);
        }

        Ok(routes)
    }

    /// Find single-hop route through a base token
    fn find_single_hop_route(
        &self,
        from_token: AlkaneId,
        to_token: AlkaneId,
        base_token: AlkaneId,
        amount_in: u128,
    ) -> Result<RouteInfo> {
        // First hop: from_token -> base_token
        let reserves1 = self
            .pool_provider
            .get_pool_reserves(from_token, base_token)?;

        let (reserve1_in, reserve1_out) = if reserves1.token_a == from_token {
            (reserves1.reserve_a, reserves1.reserve_b)
        } else {
            (reserves1.reserve_b, reserves1.reserve_a)
        };

        let intermediate_amount = amm_logic::calculate_swap_out(amount_in, reserve1_in, reserve1_out, 500)?;

        // Second hop: base_token -> to_token
        let reserves2 = self
            .pool_provider
            .get_pool_reserves(base_token, to_token)?;

        let (reserve2_in, reserve2_out) = if reserves2.token_a == base_token {
            (reserves2.reserve_a, reserves2.reserve_b)
        } else {
            (reserves2.reserve_b, reserves2.reserve_a)
        };

        let final_amount =
            amm_logic::calculate_swap_out(intermediate_amount, reserve2_in, reserve2_out, 500)?;

        // Calculate combined price impact
        let price_impact = self.calculate_path_price_impact(&[from_token, base_token, to_token], amount_in)?;

        Ok(
            RouteInfo::new(vec![from_token, base_token, to_token], final_amount)
                .with_price_impact(price_impact)
                .with_gas_estimate(100_000), // Estimated gas for two swaps
        )
    }

    /// Find multi-hop routes using BFS
    fn find_multi_hop_routes(
        &self,
        from_token: AlkaneId,
        to_token: AlkaneId,
        amount_in: u128,
    ) -> Result<Vec<RouteInfo>> {
        let mut routes = Vec::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        // Initialize with direct connections from from_token
        queue.push_back((vec![from_token], amount_in));
        visited.insert(from_token);

        while let Some((current_path, current_amount)) = queue.pop_front() {
            if current_path.len() > MAX_HOPS {
                continue;
            }

            let current_token = *current_path.last().unwrap();

            // Get all tokens that have pools with current_token
            if let Ok(connected_tokens) = self.pool_provider.get_connected_tokens(current_token) {
                for next_token in connected_tokens {
                    if visited.contains(&next_token) {
                        continue;
                    }

                    // Prevent routing through an excluded token, unless it's the final destination.
                    if self.excluded_intermediate_tokens.contains(&next_token) && next_token != to_token {
                        continue;
                    }

                    let mut new_path = current_path.clone();
                    new_path.push(next_token);

                    // Calculate amount out for this hop
                    if let Ok(reserves) = self
                        .pool_provider
                        .get_pool_reserves(current_token, next_token)
                    {
                        let (reserve_in, reserve_out) = if reserves.token_a == current_token {
                            (reserves.reserve_a, reserves.reserve_b)
                        } else {
                            (reserves.reserve_b, reserves.reserve_a)
                        };

                        if let Ok(amount_out) =
                            amm_logic::calculate_swap_out(current_amount, reserve_in, reserve_out, 500)
                        {
                            if next_token == to_token {
                                // Found a complete route
                                let price_impact =
                                    self.calculate_path_price_impact(&new_path, amount_in)?;
                                let gas_estimate = (new_path.len() - 1) as u128 * 50_000;

                                let route = RouteInfo::new(new_path, amount_out)
                                    .with_price_impact(price_impact)
                                    .with_gas_estimate(gas_estimate);
                                routes.push(route);
                            } else {
                                // Continue searching
                                queue.push_back((new_path, amount_out));
                                visited.insert(next_token);
                            }
                        }
                    }
                }
            }
        }

        Ok(routes)
    }

    /// Calculate price impact for a complete path
    fn calculate_path_price_impact(&self, path: &[AlkaneId], amount_in: u128) -> Result<u128> {
        let mut remaining_fraction = U256::from(10000);
        let mut current_amount = amount_in;

        for i in 0..path.len() - 1 {
            let from_token = path[i];
            let to_token = path[i + 1];

            let reserves = self
                .pool_provider
                .get_pool_reserves(from_token, to_token)?;

            let (reserve_in, reserve_out) = if reserves.token_a == from_token {
                (reserves.reserve_a, reserves.reserve_b)
            } else {
                (reserves.reserve_b, reserves.reserve_a)
            };

            let amount_out = amm_logic::calculate_swap_out(current_amount, reserve_in, reserve_out, 500)?;
            let impact = amm_logic::calculate_price_impact(
                current_amount,
                reserve_in,
                amount_out,
                reserve_out,
            )?;

            remaining_fraction = remaining_fraction * (U256::from(10000) - U256::from(impact)) / U256::from(10000);
            current_amount = amount_out;
        }

        Ok((U256::from(10000) - remaining_fraction).try_into()?)
    }
}
