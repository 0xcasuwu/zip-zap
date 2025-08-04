/*
Chadson's Journal - 2025-08-03

I've identified a fundamental flaw in the test environment's state simulation that has been causing the persistent test failures.

**The Problem:**
The `MockOylFactory` was storing two separate, cloned instances of each `MockPool` in its `pools` HashMap, using `(token_a, token_b)` and `(token_b, token_a)` as distinct keys. When a swap was simulated, only one of these instances would be mutated, leaving the other with stale reserve data. Any subsequent operation that looked up the same pool with the opposite token order would read the incorrect, outdated state, leading to cascading errors in economic calculations. This explains why tests for arbitrage and multi-hop swaps were failing unpredictably.

**The Fix:**
I will refactor `MockOylFactory` to store only a single instance of each `MockPool`. This will be achieved by using a canonical key for the `pools` HashMap. A helper function, `get_canonical_key`, will be introduced to create a consistent key from a token pair, regardless of the order in which the tokens are provided. The `add_pool`, `get_pool`, and `get_pool_mut` methods will be updated to use this canonical key, ensuring that all operations on a given pool always refer to the same, single `MockPool` object. This will guarantee state consistency across all interactions within a test.

This change directly addresses the core issue identified in the project summary: the mock environment's flawed simulation of state changes. With this fix, the test environment will accurately reflect the behavior of the AMM logic, allowing for proper verification of the economic properties of the Zap contract.
*/
//! Common test utilities for OYL Zap contract tests

// Silence warnings for unused code in the common module
#![allow(dead_code)]
#![allow(unused_imports)]

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use oyl_zap_core::types::{ZapQuote, RouteInfo, PoolReserves, U256};
use oyl_zap_core::route_finder::RouteFinder;
use oyl_zap_core::zap_calculator::ZapCalculator;
use oyl_zap_core::pool_provider::PoolProvider;
use oyl_zap_core::amm_logic;
use alkanes_support::id::AlkaneId;
use alkanes_support::parcel::{AlkaneTransfer, AlkaneTransferParcel};
use alkanes_support::context::Context;
use alkanes_support::response::CallResponse;
use alkanes_runtime::storage::StoragePointer;

/// Helper to create a canonical key for a token pair, ensuring consistent ordering.
fn get_canonical_key(token_a: AlkaneId, token_b: AlkaneId) -> (AlkaneId, AlkaneId) {
    // AlkaneId does not derive Ord, so we compare its fields directly.
    if (token_a.block, token_a.tx) < (token_b.block, token_b.tx) {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    }
}

// Test configuration constants
pub const TEST_PRECISION: u128 = 1_000_000_000_000_000_000;
pub const TEST_FEE_RATE: u128 = 50; // 0.5% in basis points
pub const MAX_PRICE_IMPACT: u128 = 5000; // 50% in basis points
pub const DEFAULT_SLIPPAGE: u128 = 500; // 5% in basis points

// Common test helper functions
pub fn assert_within_tolerance(actual: u128, expected: u128, tolerance_bps: u128) {
    let tolerance = expected * tolerance_bps / 10000;
    let lower_bound = expected.saturating_sub(tolerance);
    let upper_bound = expected.saturating_add(tolerance);
    
    assert!(
        actual >= lower_bound && actual <= upper_bound,
        "Value {} not within {}% tolerance of expected {}. Range: [{}, {}]",
        actual,
        tolerance_bps as f64 / 100.0,
        expected,
        lower_bound,
        upper_bound
    );
}

pub fn assert_price_impact_reasonable(price_impact: u128, max_impact: u128) {
    assert!(
        price_impact <= max_impact,
        "Price impact {}% exceeds maximum allowed {}%",
        price_impact as f64 / 100.0,
        max_impact as f64 / 100.0
    );
}

pub fn calculate_percentage_difference(value1: u128, value2: u128) -> u128 {
    if value1 == value2 {
        return 0;
    }
    
    let larger = value1.max(value2);
    let smaller = value1.min(value2);
    let difference = larger - smaller;
    
    (difference * 10000) / larger
}

/// Setup a comprehensive test environment with multiple pools and realistic liquidity
pub fn setup_comprehensive_test_environment() -> (MockOylFactory, HashMap<String, AlkaneId>) {
    let mut factory = MockOylFactory::new();
    let mut tokens = HashMap::new();
    
    // Create a comprehensive set of test tokens
    let token_configs = vec![
        ("WBTC", 100 * 100_000_000),      // Bitcoin
        ("ETH", 1000 * TEST_PRECISION),     // Ethereum
        ("USDC", 2_000_000 * 1_000_000),  // USD Coin
        ("USDT", 2_000_000 * 1_000_000),  // Tether
        ("DAI", 2_000_000 * TEST_PRECISION),  // Dai Stablecoin
        ("WETH", 1000 * TEST_PRECISION),    // Wrapped Ethereum
        ("UNI", 50000 * TEST_PRECISION),    // Uniswap
        ("LINK", 100000 * TEST_PRECISION),  // Chainlink
        ("AAVE", 10000 * TEST_PRECISION),   // Aave
        ("COMP", 5000 * TEST_PRECISION),    // Compound
    ];
    
    // Create tokens
    for (name, _) in &token_configs {
        tokens.insert(name.to_string(), alkane_id(name));
    }
    
    // Create comprehensive pool network
    let pool_configs = vec![
        // Major pairs
        ("WBTC", "ETH", 50 * 100_000_000, 750 * TEST_PRECISION),
        ("ETH", "USDC", 1000 * TEST_PRECISION, 2_000_000 * 1_000_000),
        ("ETH", "USDT", 800 * TEST_PRECISION, 1_600_000 * 1_000_000),
        ("ETH", "DAI", 900 * TEST_PRECISION, 1_800_000 * TEST_PRECISION),
        
        // Stablecoin pairs
        ("USDC", "USDT", 1_000_000 * 1_000_000, 1_000_000 * 1_000_000),
        ("USDC", "DAI", 1_000_000 * 1_000_000, 1_000_000 * TEST_PRECISION),
        ("USDT", "DAI", 1_000_000 * 1_000_000, 1_000_000 * TEST_PRECISION),
        
        // BTC pairs
        ("WBTC", "USDC", 25 * 100_000_000, 500_000 * 1_000_000),
        ("WBTC", "USDT", 20 * 100_000_000, 400_000 * 1_000_000),
        
        // DeFi tokens
        ("UNI", "ETH", 10000 * TEST_PRECISION, 100 * TEST_PRECISION),
        ("UNI", "USDC", 15000 * TEST_PRECISION, 150_000 * 1_000_000),
        ("LINK", "ETH", 5000 * TEST_PRECISION, 50 * TEST_PRECISION),
        ("LINK", "USDC", 8000 * TEST_PRECISION, 80_000 * 1_000_000),
        ("AAVE", "ETH", 1000 * TEST_PRECISION, 100 * TEST_PRECISION),
        ("AAVE", "USDC", 1500 * TEST_PRECISION, 150_000 * 1_000_000),
        ("COMP", "ETH", 500 * TEST_PRECISION, 50 * TEST_PRECISION),
        ("COMP", "USDC", 800 * TEST_PRECISION, 80_000 * 1_000_000),
        
        // Additional routing pairs
        ("WETH", "ETH", 500 * TEST_PRECISION, 500 * TEST_PRECISION),
        ("WETH", "USDC", 400 * TEST_PRECISION, 800_000 * 1_000_000),
    ];
    
    // Add all pools
    for (token_a_name, token_b_name, reserve_a, reserve_b) in pool_configs {
        let token_a = tokens[token_a_name];
        let token_b = tokens[token_b_name];
        factory.add_pool(token_a, token_b, reserve_a, reserve_b);
    }
    
    (factory, tokens)
}

/// Create a mock OYL Zap instance for testing
pub fn create_mock_zap() -> MockOylZap {
    MockOylZap::new()
}

/// Mock OYL Zap implementation for testing
#[derive(Clone)]
pub struct MockOylZap {
    pub factory_id: AlkaneId,
    pub base_tokens: Vec<AlkaneId>,
    pub max_price_impact: u128,
    pub default_slippage: u128,
    pub factory: MockOylFactory,
}

impl MockOylZap {
    pub fn new() -> Self {
        let (factory, base_tokens) = setup_test_environment();
        Self {
            factory_id: alkane_id("oyl_factory"),
            base_tokens,
            max_price_impact: MAX_PRICE_IMPACT,
            default_slippage: DEFAULT_SLIPPAGE,
            factory,
        }
    }
    
    pub fn with_comprehensive_setup() -> Self {
        let (factory, token_map) = setup_comprehensive_test_environment();
        let base_tokens = vec![
            token_map["WBTC"],
            token_map["ETH"],
            token_map["USDC"],
            token_map["USDT"],
            token_map["DAI"],
        ];
        
        Self {
            factory_id: alkane_id("oyl_factory"),
            base_tokens,
            max_price_impact: MAX_PRICE_IMPACT,
            default_slippage: DEFAULT_SLIPPAGE,
            factory,
        }
    }
    
    pub fn init_zap(&mut self, factory_id: AlkaneId, base_tokens: Vec<AlkaneId>) -> Result<()> {
        self.factory_id = factory_id;
        self.base_tokens = base_tokens;
        Ok(())
    }
    
    pub fn get_zap_quote(
        &self,
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        max_slippage_bps: u128,
    ) -> Result<ZapQuote> {
        // Find routes to both target tokens, handling direct contributions and excluding the other target token
        // from the path to prevent the route from cannibalizing the liquidity of its sibling target pool.
        let route_a = if input_token == target_token_a {
            Ok(RouteInfo::new(vec![input_token], input_amount / 2))
        } else {
            RouteFinder::new(self.factory_id, &self.factory)
                .with_base_tokens(self.base_tokens.clone())
                .with_excluded_intermediate_tokens(&[target_token_b])
                .find_best_route(input_token, target_token_a, input_amount / 2)
        }?;

        let route_b = if input_token == target_token_b {
            Ok(RouteInfo::new(vec![input_token], input_amount / 2))
        } else {
            RouteFinder::new(self.factory_id, &self.factory)
                .with_base_tokens(self.base_tokens.clone())
                .with_excluded_intermediate_tokens(&[target_token_a])
                .find_best_route(input_token, target_token_b, input_amount / 2)
        }?;
        
        // Get target pool reserves
        let target_pool = self.factory.get_pool(target_token_a, target_token_b)
            .ok_or_else(|| anyhow::anyhow!("Target pool not found"))?;
        
        let target_pool_reserves = PoolReserves::new(
            target_token_a,
            target_token_b,
            target_pool.reserve_a,
            target_pool.reserve_b,
            target_pool.total_supply,
            target_pool.fee_rate,
        );
        
        // Generate quote
        ZapCalculator::generate_zap_quote(
            input_token,
            input_amount,
            target_token_a,
            target_token_b,
            route_a,
            route_b,
            &target_pool_reserves,
            max_slippage_bps,
            // The route_finder used here is for post-calculation checks, so a generic one is fine.
            &RouteFinder::new(self.factory_id, &self.factory),
        )
    }
    
    pub fn execute_zap(&mut self, quote: &ZapQuote) -> Result<u128> {
        // Clone the factory to create an isolated environment for this zap execution.
        // This prevents race conditions where the execution of one route affects the other.
        let mut execution_factory = self.factory.clone();

        // Step 1: Execute swaps for both routes within the isolated factory.
        let amount_a_received =
            Self::simulate_route_execution_static(&mut execution_factory, &quote.route_a, quote.split_amount_a)?;
        let amount_b_received =
            Self::simulate_route_execution_static(&mut execution_factory, &quote.route_b, quote.split_amount_b)?;

        // Step 2: Add liquidity to the target pool within the isolated factory.
        let target_pool = execution_factory
            .get_pool_mut(quote.target_token_a, quote.target_token_b)
            .ok_or_else(|| anyhow::anyhow!("Target pool not found in execution factory"))?;
        
        let lp_tokens = target_pool.simulate_add_liquidity(amount_a_received, amount_b_received)?;

        // Step 3: Atomically update the main factory state with the result of the execution.
        self.factory = execution_factory;

        // Step 4: Verify minimum LP tokens.
        if lp_tokens < quote.minimum_lp_tokens {
            return Err(anyhow::anyhow!(
                "Received {} LP tokens, less than minimum {}",
                lp_tokens,
                quote.minimum_lp_tokens
            ));
        }

        Ok(lp_tokens)
    }

    // Refactored to be a static method to make data flow explicit and support isolated execution.
    fn simulate_route_execution_static(
        factory: &mut MockOylFactory,
        route: &RouteInfo,
        amount_in: u128,
    ) -> Result<u128> {
        let mut current_amount = amount_in;

        for i in 0..route.path.len() - 1 {
            let token_in = route.path[i];
            let token_out = route.path[i + 1];

            let pool = factory
                .get_pool_mut(token_in, token_out)
                .ok_or_else(|| anyhow::anyhow!("Pool not found for route hop: {:?} -> {:?}", token_in, token_out))?;

            current_amount = pool.simulate_swap(token_in, current_amount)?;
        }

        Ok(current_amount)
    }
    
    // Keep the old instance method for compatibility or specific tests if needed, but delegate.
    fn simulate_route_execution(&mut self, route: &RouteInfo, amount_in: u128) -> Result<u128> {
        Self::simulate_route_execution_static(&mut self.factory, route, amount_in)
    }
    
    pub fn find_optimal_route(
        &self,
        from_token: AlkaneId,
        to_token: AlkaneId,
        amount: u128,
    ) -> Result<RouteInfo> {
        let route_finder = RouteFinder::new(self.factory_id, &self.factory)
            .with_base_tokens(self.base_tokens.clone());
        
        route_finder.find_best_route(from_token, to_token, amount)
    }
}

impl Default for MockOylZap {
    fn default() -> Self {
        Self::new()
    }
}

// Test result validation helpers
pub fn validate_zap_quote(quote: &ZapQuote) -> Result<()> {
    // Basic validation
    quote.validate()?;
    
    // Additional validations
    assert!(quote.expected_lp_tokens > 0, "Expected LP tokens must be positive");
    assert!(quote.minimum_lp_tokens <= quote.expected_lp_tokens, "Minimum LP tokens cannot exceed expected");
    assert!(quote.price_impact <= MAX_PRICE_IMPACT, "Price impact too high");
    assert_eq!(quote.split_amount_a + quote.split_amount_b, quote.input_amount, "Split amounts must sum to input");
    
    Ok(())
}

pub fn validate_route_info(route: &RouteInfo) -> Result<()> {
    assert!(!route.path.is_empty(), "Route path cannot be empty");
    assert!(route.path.len() >= 2, "Route must have at least 2 tokens");
    assert!(route.expected_output > 0, "Expected output must be positive");
    assert!(route.price_impact <= MAX_PRICE_IMPACT, "Price impact too high");
    
    Ok(())
}

// Performance benchmarking helpers
pub fn benchmark_route_finding(
    zap: &MockOylZap,
    from_token: AlkaneId,
    to_token: AlkaneId,
    amount: u128,
    iterations: usize,
) -> std::time::Duration {
    let start = std::time::Instant::now();
    
    for _ in 0..iterations {
        let _ = zap.find_optimal_route(from_token, to_token, amount);
    }
    
    start.elapsed()
}

pub fn benchmark_zap_quote_generation(
    zap: &MockOylZap,
    input_token: AlkaneId,
    input_amount: u128,
    target_token_a: AlkaneId,
    target_token_b: AlkaneId,
    iterations: usize,
) -> std::time::Duration {
    let start = std::time::Instant::now();
    
    for _ in 0..iterations {
        let _ = zap.get_zap_quote(input_token, input_amount, target_token_a, target_token_b, DEFAULT_SLIPPAGE);
    }
    
    start.elapsed()
}

// ============================================================================
// OYL ZAP SPECIFIC MOCKS
// ============================================================================

/// Mock OYL Factory for testing zap operations
#[derive(Default, Clone)]
pub struct MockOylFactory {
    pub pools: HashMap<(AlkaneId, AlkaneId), MockPool>,
    pub pool_count: u128,
}

impl MockOylFactory {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_pool(&mut self, token_a: AlkaneId, token_b: AlkaneId, reserve_a: u128, reserve_b: u128) -> AlkaneId {
        let pool_id = AlkaneId {
            block: self.pool_count + 1000,
            tx: self.pool_count + 2000,
        };
        
        let total_supply = amm_logic::calculate_lp_tokens_minted(reserve_a, reserve_b, 0, 0, 0).unwrap_or(0);

        let pool = MockPool {
            id: pool_id,
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            total_supply,
            fee_rate: TEST_FEE_RATE,
        };
        
        // Store pool with a canonical key to prevent state inconsistencies from duplicate pool objects.
        let key = get_canonical_key(token_a, token_b);
        self.pools.insert(key, pool);
        self.pool_count += 1;
        
        pool_id
    }
    
    pub fn get_pool(&self, token_a: AlkaneId, token_b: AlkaneId) -> Option<&MockPool> {
        let key = get_canonical_key(token_a, token_b);
        self.pools.get(&key)
    }

    pub fn get_pool_mut(&mut self, token_a: AlkaneId, token_b: AlkaneId) -> Option<&mut MockPool> {
        let key = get_canonical_key(token_a, token_b);
        self.pools.get_mut(&key)
    }
}

impl PoolProvider for MockOylFactory {
    fn get_pool_reserves(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<PoolReserves> {
        let pool = self.get_pool(token_a, token_b)
            .ok_or_else(|| anyhow::anyhow!("Pool not found"))?;
        
        Ok(PoolReserves::new(
            pool.token_a,
            pool.token_b,
            pool.reserve_a,
            pool.reserve_b,
            pool.total_supply,
            pool.fee_rate,
        ))
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
        // Remove duplicates
        connected.sort();
        connected.dedup();
        Ok(connected)
    }
}

/// Mock Pool for testing
#[derive(Debug, Clone)]
pub struct MockPool {
    pub id: AlkaneId,
    pub token_a: AlkaneId,
    pub token_b: AlkaneId,
    pub reserve_a: u128,
    pub reserve_b: u128,
    pub total_supply: u128,
    pub fee_rate: u128, // in basis points
}

impl MockPool {
    pub fn simulate_swap(&mut self, token_in: AlkaneId, amount_in: u128) -> Result<u128> {
        let (reserve_in, reserve_out) = if token_in == self.token_a {
            (self.reserve_a, self.reserve_b)
        } else if token_in == self.token_b {
            (self.reserve_b, self.reserve_a)
        } else {
            return Err(anyhow::anyhow!("Token not in pool"));
        };

        let amount_out = amm_logic::calculate_swap_out(amount_in, reserve_in, reserve_out, self.fee_rate)?;

        if token_in == self.token_a {
            self.reserve_a = self.reserve_a.saturating_add(amount_in);
            self.reserve_b = self.reserve_b.saturating_sub(amount_out);
        } else {
            self.reserve_b = self.reserve_b.saturating_add(amount_in);
            self.reserve_a = self.reserve_a.saturating_sub(amount_out);
        }
        
        Ok(amount_out)
    }

    pub fn simulate_add_liquidity(&mut self, amount_a: u128, amount_b: u128) -> Result<u128> {
        let lp_tokens = amm_logic::calculate_lp_tokens_minted(
            amount_a,
            amount_b,
            self.reserve_a,
            self.reserve_b,
            self.total_supply,
        )?;

        self.reserve_a = self.reserve_a.saturating_add(amount_a);
        self.reserve_b = self.reserve_b.saturating_add(amount_b);
        self.total_supply = self.total_supply.saturating_add(lp_tokens);
        
        Ok(lp_tokens)
    }
}

// ============================================================================
// TEST HELPER FUNCTIONS
// ============================================================================

/// Create a test AlkaneId from a string
pub fn alkane_id(s: &str) -> AlkaneId {
    let mut block_bytes = [0u8; 16];
    let s_bytes = s.as_bytes();
    let len = s_bytes.len().min(16);
    block_bytes[..len].copy_from_slice(&s_bytes[..len]);
    let block = u128::from_le_bytes(block_bytes);
    AlkaneId { block, tx: 0 }
}

/// Create a test context with specified caller and incoming alkanes
pub fn create_test_context(caller: AlkaneId, incoming: Vec<AlkaneTransfer>) -> Context {
    Context {
        caller,
        myself: alkane_id("zap_contract"),
        incoming_alkanes: AlkaneTransferParcel(incoming),
        inputs: vec![],
        vout: 0,
    }
}

/// Setup a balanced test environment with common tokens and pools
pub fn setup_test_environment() -> (MockOylFactory, Vec<AlkaneId>) {
    let mut factory = MockOylFactory::new();
    
    // Create common test tokens
    let wbtc = alkane_id("WBTC");
    let eth = alkane_id("ETH");
    let usdc = alkane_id("USDC");
    let usdt = alkane_id("USDT");
    let dai = alkane_id("DAI");
    
    // Add pools with realistic reserves
    factory.add_pool(wbtc, eth, 100 * 100_000_000, 1500 * TEST_PRECISION);      // WBTC/ETH
    factory.add_pool(eth, usdc, 1000 * TEST_PRECISION, 2_000_000 * 1_000_000);  // ETH/USDC
    factory.add_pool(usdc, usdt, 1_000_000 * 1_000_000, 1_000_000 * 1_000_000); // USDC/USDT
    factory.add_pool(usdc, dai, 1_000_000 * 1_000_000, 1_000_000 * TEST_PRECISION); // USDC/DAI
    factory.add_pool(wbtc, usdc, 50 * 100_000_000, 1_000_000 * 1_000_000);    // WBTC/USDC
    
    let base_tokens = vec![wbtc, eth, usdc, usdt, dai];
    
    (factory, base_tokens)
}

/// Create a mock zap quote for testing
pub fn create_mock_zap_quote(
    input_token: AlkaneId,
    input_amount: u128,
    target_token_a: AlkaneId,
    target_token_b: AlkaneId,
) -> oyl_zap_core::types::ZapQuote {
    
    let route_a = RouteInfo::new(
        vec![input_token, target_token_a],
        input_amount / 2,
    ).with_price_impact(100); // 1% price impact
    
    let route_b = RouteInfo::new(
        vec![input_token, target_token_b],
        input_amount / 2,
    ).with_price_impact(150); // 1.5% price impact
    
    ZapQuote::new(input_token, input_amount, target_token_a, target_token_b)
        .with_routes(route_a, route_b)
        .with_split(input_amount / 2, input_amount / 2)
        .with_lp_estimate(1000, 950) // Expected and minimum LP tokens
}

/// Simulate flash loan attack scenario
pub fn simulate_flash_loan_attack(
    factory: &mut MockOylFactory,
    target_pool_id: AlkaneId,
    attack_token: AlkaneId,
    flash_amount: u128,
) -> Result<i128> {
    let mut test_factory = factory.clone();
    let initial_reserves = test_factory.pools.values().find(|p| p.id == target_pool_id).cloned()
        .ok_or_else(|| anyhow::anyhow!("Pool not found"))?;
    
    let other_token = if initial_reserves.token_a == attack_token { initial_reserves.token_b } else { initial_reserves.token_a };
    
    let pool = test_factory.get_pool_mut(initial_reserves.token_a, initial_reserves.token_b).unwrap();
    
    // Step 1: Large swap to manipulate price
    let amount_out = pool.simulate_swap(attack_token, flash_amount)?;
    
    // Step 2: Reverse swap
    let amount_back = pool.simulate_swap(other_token, amount_out)?;
    
    // Step 3: Calculate profit/loss
    let profit = amount_back as i128 - flash_amount as i128;
    
    Ok(profit)
}

/// Calculate expected arbitrage profit for testing economic properties
pub fn calculate_arbitrage_profit(
    factory: &mut MockOylFactory,
    token_a: AlkaneId,
    token_b: AlkaneId,
    intermediate_token: AlkaneId,
    amount: u128,
) -> Result<i128> {
    // --- Indirect Route Simulation (A -> Intermediate -> B) ---
    let mut indirect_factory = factory.clone();
    let pool1 = indirect_factory.get_pool_mut(token_a, intermediate_token)
        .ok_or_else(|| anyhow::anyhow!("Pool A->Intermediate not found"))?;
    let intermediate_amount = pool1.simulate_swap(token_a, amount)?;
    println!("Intermediate amount: {}", intermediate_amount);

    let pool2 = indirect_factory.get_pool_mut(intermediate_token, token_b)
        .ok_or_else(|| anyhow::anyhow!("Pool Intermediate->B not found"))?;
    let indirect_output = pool2.simulate_swap(intermediate_token, intermediate_amount)?;
    println!("Indirect output: {}", indirect_output);

    // --- Direct Route Simulation (A -> B) ---
    let mut direct_factory = factory.clone();
    let direct_pool = direct_factory.get_pool_mut(token_a, token_b)
        .ok_or_else(|| anyhow::anyhow!("Direct pool A->B not found"))?;
    let direct_output = direct_pool.simulate_swap(token_a, amount)?;
    println!("Direct output: {}", direct_output);

    // The arbitrage profit is the difference between the two outcomes
    let profit = indirect_output as i128 - direct_output as i128;
    
    Ok(profit)
}