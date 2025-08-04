use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};
use ruint::Uint;

pub type U256 = Uint<256, 4>;

#[derive(Debug, Clone, PartialEq)]
pub struct RouteInfo {
    pub path: Vec<AlkaneId>,
    pub expected_output: u128,
    pub price_impact: u128, // in basis points (10000 = 100%)
    pub gas_estimate: u128,
}

impl RouteInfo {
    pub fn new(path: Vec<AlkaneId>, expected_output: u128) -> Self {
        Self {
            path,
            expected_output,
            price_impact: 0,
            gas_estimate: 0,
        }
    }

    pub fn with_price_impact(mut self, price_impact: u128) -> Self {
        self.price_impact = price_impact;
        self
    }

    pub fn with_gas_estimate(mut self, gas_estimate: u128) -> Self {
        self.gas_estimate = gas_estimate;
        self
    }

    pub fn is_direct_route(&self) -> bool {
        self.path.len() == 2
    }

    pub fn hop_count(&self) -> usize {
        if self.path.len() < 2 {
            0
        } else {
            self.path.len() - 1
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZapQuote {
    pub input_token: AlkaneId,
    pub input_amount: u128,
    pub target_token_a: AlkaneId,
    pub target_token_b: AlkaneId,
    pub route_a: RouteInfo,
    pub route_b: RouteInfo,
    pub split_amount_a: u128,
    pub split_amount_b: u128,
    pub expected_lp_tokens: u128,
    pub price_impact: u128,
    pub minimum_lp_tokens: u128,
}

impl ZapQuote {
    pub fn new(
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
    ) -> Self {
        Self {
            input_token,
            input_amount,
            target_token_a,
            target_token_b,
            route_a: RouteInfo::new(vec![], 0),
            route_b: RouteInfo::new(vec![], 0),
            split_amount_a: 0,
            split_amount_b: 0,
            expected_lp_tokens: 0,
            price_impact: 0,
            minimum_lp_tokens: 0,
        }
    }

    pub fn with_routes(mut self, route_a: RouteInfo, route_b: RouteInfo) -> Self {
        self.route_a = route_a;
        self.route_b = route_b;
        self
    }

    pub fn with_split(mut self, split_amount_a: u128, split_amount_b: u128) -> Self {
        self.split_amount_a = split_amount_a;
        self.split_amount_b = split_amount_b;
        self
    }

    pub fn with_lp_estimate(mut self, expected_lp_tokens: u128, minimum_lp_tokens: u128) -> Self {
        self.expected_lp_tokens = expected_lp_tokens;
        self.minimum_lp_tokens = minimum_lp_tokens;
        self
    }

    pub fn with_price_impact(mut self, price_impact: u128) -> Self {
        self.price_impact = price_impact;
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.input_amount == 0 {
            return Err(anyhow!("Input amount cannot be zero"));
        }

        if self.split_amount_a + self.split_amount_b != self.input_amount {
            return Err(anyhow!("Split amounts must sum to input amount"));
        }

        if self.route_a.path.is_empty() || self.route_b.path.is_empty() {
            return Err(anyhow!("Routes cannot be empty"));
        }

        if self.route_a.path[0] != self.input_token || self.route_b.path[0] != self.input_token {
            return Err(anyhow!("Routes must start with input token"));
        }

        let route_a_end = self.route_a.path.last().unwrap();
        let route_b_end = self.route_b.path.last().unwrap();

        if *route_a_end != self.target_token_a || *route_b_end != self.target_token_b {
            return Err(anyhow!("Routes must end with target tokens"));
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PoolReserves {
    pub token_a: AlkaneId,
    pub token_b: AlkaneId,
    pub reserve_a: u128,
    pub reserve_b: u128,
    pub total_supply: u128,
    pub fee_rate: u128,
}

impl PoolReserves {
    pub fn new(
        token_a: AlkaneId,
        token_b: AlkaneId,
        reserve_a: u128,
        reserve_b: u128,
        total_supply: u128,
        fee_rate: u128,
    ) -> Self {
        Self {
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            total_supply,
            fee_rate,
        }
    }

    pub fn get_reserve_for_token(&self, token: &AlkaneId) -> Option<u128> {
        if *token == self.token_a {
            Some(self.reserve_a)
        } else if *token == self.token_b {
            Some(self.reserve_b)
        } else {
            None
        }
    }

    pub fn get_price_ratio(&self) -> Result<U256> {
        if self.reserve_b == 0 {
            return Err(anyhow!("Cannot calculate price ratio with zero reserve"));
        }
        Ok(U256::from(self.reserve_a) * U256::from(1e18 as u128) / U256::from(self.reserve_b))
    }
}

#[derive(Debug, Clone)]
pub struct ZapParams {
    pub input_token: AlkaneId,
    pub input_amount: u128,
    pub target_token_a: AlkaneId,
    pub target_token_b: AlkaneId,
    pub min_lp_tokens: u128,
    pub deadline: u128,
    pub max_slippage_bps: u128, // basis points, 100 = 1%
}

impl ZapParams {
    pub fn new(
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        min_lp_tokens: u128,
        deadline: u128,
    ) -> Self {
        Self {
            input_token,
            input_amount,
            target_token_a,
            target_token_b,
            min_lp_tokens,
            deadline,
            max_slippage_bps: 500, // 5% default
        }
    }

    pub fn with_max_slippage(mut self, max_slippage_bps: u128) -> Self {
        self.max_slippage_bps = max_slippage_bps;
        self
    }

    pub fn validate(&self, current_time: u128) -> Result<()> {
        if self.input_amount == 0 {
            return Err(anyhow!("Input amount cannot be zero"));
        }

        if self.deadline <= current_time {
            return Err(anyhow!("Transaction deadline has passed"));
        }

        if self.max_slippage_bps > 10000 {
            return Err(anyhow!("Max slippage cannot exceed 100%"));
        }

        if self.input_token == self.target_token_a || self.input_token == self.target_token_b {
            return Err(anyhow!("Input token cannot be the same as target tokens"));
        }

        if self.target_token_a == self.target_token_b {
            return Err(anyhow!("Target tokens must be different"));
        }

        Ok(())
    }
}

// Constants for the zap contract
pub const DEFAULT_FEE_AMOUNT_PER_1000: u128 = 5; // 0.5% fee
pub const MAX_HOPS: usize = 3; // Maximum number of hops in a route
pub const BASIS_POINTS: u128 = 10000; // 100% in basis points
pub const MINIMUM_LIQUIDITY: u128 = 1000; // Minimum liquidity for new pools
