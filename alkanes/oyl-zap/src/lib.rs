use alkanes_runtime::{declare_alkane, runtime::AlkaneResponder, message::MessageDispatch};
use metashrew_support::compat::to_arraybuffer_layout;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use alkanes_support::response::CallResponse;

mod types;
mod amm_logic;
mod pool_provider;
mod route_finder;
mod zap_calculator;

use types::{ZapParams, PoolReserves};
use pool_provider::PoolProvider;
use route_finder::RouteFinder;
use zap_calculator::ZapCalculator;
use std::collections::HashMap;

#[derive(Default)]
pub struct OylZap {
    pools: HashMap<(AlkaneId, AlkaneId), PoolReserves>,
    base_tokens: Vec<AlkaneId>,
}

impl AlkaneResponder for OylZap {}

#[derive(MessageDispatch)]
enum OylZapMessage {
    #[opcode(0)]
    Initialize {
        factory_id: AlkaneId,
        base_tokens: Vec<AlkaneId>,
    },
    #[opcode(1)]
    AddPool {
        token_a: AlkaneId,
        token_b: AlkaneId,
        reserve_a: u128,
        reserve_b: u128,
        total_supply: u128,
        fee_rate: u128,
    },
    #[opcode(2)]
    UpdatePoolReserves {
        token_a: AlkaneId,
        token_b: AlkaneId,
        reserve_a: u128,
        reserve_b: u128,
        total_supply: u128,
    },
    #[opcode(3)]
    GetZapQuote {
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        max_slippage_bps: u128,
    },
    #[opcode(4)]
    ExecuteZap {
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        min_lp_tokens: u128,
        deadline: u128,
        max_slippage_bps: u128,
    },
    #[opcode(5)]
    GetBestRoute {
        from_token: AlkaneId,
        to_token: AlkaneId,
        amount_in: u128,
    },
    #[opcode(6)]
    GetPoolReserves {
        token_a: AlkaneId,
        token_b: AlkaneId,
    },
}

impl PoolProvider for OylZap {
    fn get_pool_reserves(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<PoolReserves> {
        let key1 = (token_a, token_b);
        let key2 = (token_b, token_a);
        
        self.pools.get(&key1)
            .or_else(|| self.pools.get(&key2))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Pool not found for tokens {:?} and {:?}", token_a, token_b))
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

impl OylZap {
    fn initialize(&mut self, factory_id: AlkaneId, base_tokens: Vec<AlkaneId>) -> Result<CallResponse> {
        self.base_tokens = base_tokens;
        self.pools.clear();
        
        Ok(CallResponse::forward(
            factory_id,
            &to_arraybuffer_layout(&b"Zap contract initialized successfully"[..])?,
        ))
    }

    fn add_pool(
        &mut self,
        token_a: AlkaneId,
        token_b: AlkaneId,
        reserve_a: u128,
        reserve_b: u128,
        total_supply: u128,
        fee_rate: u128,
    ) -> Result<CallResponse> {
        let pool = PoolReserves::new(token_a, token_b, reserve_a, reserve_b, total_supply, fee_rate);
        
        // Store pool with both key combinations for easy lookup
        self.pools.insert((token_a, token_b), pool.clone());
        self.pools.insert((token_b, token_a), pool);
        
        Ok(CallResponse::forward(
            token_a,
            &to_arraybuffer_layout(&format!("Pool added: {:?} - {:?}", token_a, token_b).as_bytes())?,
        ))
    }

    fn update_pool_reserves(
        &mut self,
        token_a: AlkaneId,
        token_b: AlkaneId,
        reserve_a: u128,
        reserve_b: u128,
        total_supply: u128,
    ) -> Result<CallResponse> {
        let key1 = (token_a, token_b);
        let key2 = (token_b, token_a);
        
        if let Some(pool) = self.pools.get_mut(&key1) {
            pool.reserve_a = reserve_a;
            pool.reserve_b = reserve_b;
            pool.total_supply = total_supply;
            
            // Update the reverse key as well
            if let Some(reverse_pool) = self.pools.get_mut(&key2) {
                reverse_pool.reserve_a = reserve_b;
                reverse_pool.reserve_b = reserve_a;
                reverse_pool.total_supply = total_supply;
            }
        } else if let Some(pool) = self.pools.get_mut(&key2) {
            pool.reserve_a = reserve_b;
            pool.reserve_b = reserve_a;
            pool.total_supply = total_supply;
            
            // Update the reverse key as well
            if let Some(reverse_pool) = self.pools.get_mut(&key1) {
                reverse_pool.reserve_a = reserve_a;
                reverse_pool.reserve_b = reserve_b;
                reverse_pool.total_supply = total_supply;
            }
        } else {
            return Err(anyhow::anyhow!("Pool not found for update"));
        }
        
        Ok(CallResponse::forward(
            token_a,
            &to_arraybuffer_layout(&format!("Pool reserves updated: {:?} - {:?}", token_a, token_b).as_bytes())?,
        ))
    }

    fn get_zap_quote(
        &self,
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        max_slippage_bps: u128,
    ) -> Result<CallResponse> {
        // Create route finder
        let factory_id = AlkaneId { block: 0, tx: 0 }; // Default factory ID
        let route_finder = RouteFinder::new(factory_id, self)
            .with_base_tokens(self.base_tokens.clone());

        // Find optimal routes
        let route_a = route_finder.find_best_route(input_token, target_token_a, input_amount / 2)?;
        let route_b = route_finder.find_best_route(input_token, target_token_b, input_amount / 2)?;

        // Get target pool reserves
        let target_pool_reserves = self.get_pool_reserves(target_token_a, target_token_b)?;

        // Generate zap quote
        let quote = ZapCalculator::generate_zap_quote(
            input_token,
            input_amount,
            target_token_a,
            target_token_b,
            route_a,
            route_b,
            &target_pool_reserves,
            max_slippage_bps,
            &route_finder,
        )?;

        // Validate the quote
        ZapCalculator::validate_zap_quote(&quote)?;

        // Serialize quote for response
        let quote_data = format!(
            "ZapQuote: input_amount={}, split_a={}, split_b={}, expected_lp={}, min_lp={}, price_impact={}bps",
            quote.input_amount,
            quote.split_amount_a,
            quote.split_amount_b,
            quote.expected_lp_tokens,
            quote.minimum_lp_tokens,
            quote.price_impact
        );

        Ok(CallResponse::forward(
            input_token,
            &to_arraybuffer_layout(&quote_data.as_bytes())?,
        ))
    }

    fn execute_zap(&mut self, params: ZapParams) -> Result<CallResponse> {
        // Validate parameters
        let current_time = 0u128; // In production, this would be the current block timestamp
        params.validate(current_time)?;

        // Get zap quote first
        let quote_response = self.get_zap_quote(
            params.input_token,
            params.input_amount,
            params.target_token_a,
            params.target_token_b,
            params.max_slippage_bps,
        )?;

        // In a real implementation, this would:
        // 1. Execute the swaps according to the optimal routes
        // 2. Add liquidity to the target pool
        // 3. Return LP tokens to the user
        // 4. Handle slippage protection and deadline checks

        Ok(CallResponse::forward(
            params.input_token,
            &to_arraybuffer_layout(&format!("Zap executed successfully for {} tokens", params.input_amount).as_bytes())?,
        ))
    }

    fn get_best_route(
        &self,
        from_token: AlkaneId,
        to_token: AlkaneId,
        amount_in: u128,
    ) -> Result<CallResponse> {
        let factory_id = AlkaneId { block: 0, tx: 0 };
        let route_finder = RouteFinder::new(factory_id, self)
            .with_base_tokens(self.base_tokens.clone());

        let route = route_finder.find_best_route(from_token, to_token, amount_in)?;

        let route_data = format!(
            "BestRoute: path_length={}, expected_output={}, price_impact={}bps, gas_estimate={}",
            route.path.len(),
            route.expected_output,
            route.price_impact,
            route.gas_estimate
        );

        Ok(CallResponse::forward(
            from_token,
            &to_arraybuffer_layout(&route_data.as_bytes())?,
        ))
    }

    fn get_pool_reserves_info(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<CallResponse> {
        let reserves = self.get_pool_reserves(token_a, token_b)?;
        
        let reserves_data = format!(
            "PoolReserves: token_a={:?}, reserve_a={}, token_b={:?}, reserve_b={}, total_supply={}, fee_rate={}",
            reserves.token_a,
            reserves.reserve_a,
            reserves.token_b,
            reserves.reserve_b,
            reserves.total_supply,
            reserves.fee_rate
        );

        Ok(CallResponse::forward(
            token_a,
            &to_arraybuffer_layout(&reserves_data.as_bytes())?,
        ))
    }
}

declare_alkane! {
    impl AlkaneResponder for OylZap {
        type Message = OylZapMessage;
    }
}
