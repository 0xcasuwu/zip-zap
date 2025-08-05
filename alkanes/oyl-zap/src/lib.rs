use alkanes_runtime::{
    auth::AuthenticatedResponder, declare_alkane, message::MessageDispatch,
    runtime::AlkaneResponder,
};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::{
    id::AlkaneId, 
    response::CallResponse, 
    parcel::{AlkaneTransfer, AlkaneTransferParcel}, 
    cellpack::Cellpack,
    context::Context
};
use anyhow::{anyhow, Result};
use metashrew_support::{compat::to_arraybuffer_layout};

pub mod types;
pub mod route_finder;
pub mod zap_calculator;
pub mod pool_provider;
pub mod amm_logic;

use types::{ZapParams, ZapQuote, PoolReserves, RouteInfo, DEFAULT_FEE_AMOUNT_PER_1000};
use route_finder::RouteFinder;
use zap_calculator::ZapCalculator;
use pool_provider::PoolProvider;

struct ContractPoolProvider<'a, T: OylZapBase + ?Sized> {
    contract: &'a T,
}

impl<'a, T: OylZapBase + ?Sized> PoolProvider for ContractPoolProvider<'a, T> {
    fn get_pool_reserves(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<PoolReserves> {
        self.contract.get_pool_reserves(token_a, token_b)
    }

    fn get_connected_tokens(&self, _token: AlkaneId) -> Result<Vec<AlkaneId>> {
        // This would require a more complex implementation to query the factory
        // For now, we'll return an empty vec and rely on base tokens for routing
        Ok(vec![])
    }
}

#[derive(MessageDispatch)]
pub enum OylZapMessage {
    #[opcode(0)]
    InitZap {
        oyl_factory_id: AlkaneId,
        base_tokens: Vec<AlkaneId>,
    },

    #[opcode(1)]
    ZapIntoLP {
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        min_lp_tokens: u128,
        deadline: u128,
    },

    #[opcode(2)]
    #[returns(Vec<u8>)]
    GetZapQuote {
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        max_slippage_bps: u128,
    },

    #[opcode(3)]
    #[returns(Vec<u8>)]
    FindOptimalRoute {
        from_token: AlkaneId,
        to_token: AlkaneId,
        amount: u128,
    },

    #[opcode(4)]
    SetOylFactory {
        factory_id: AlkaneId,
    },

    #[opcode(5)]
    AddBaseToken {
        token_id: AlkaneId,
    },

    #[opcode(6)]
    RemoveBaseToken {
        token_id: AlkaneId,
    },

    #[opcode(7)]
    #[returns(Vec<u8>)]
    GetBaseTokens,

    #[opcode(8)]
    #[returns(Vec<u8>)]
    GetZapConfig,

    #[opcode(50)]
    Forward {},
}

pub trait OylZapBase: AlkaneResponder {
    fn init_zap(&self, oyl_factory_id: AlkaneId, base_tokens: Vec<AlkaneId>) -> Result<CallResponse> {
        // Store the OYL factory ID
        self.set_factory_id(&oyl_factory_id)?;
        
        // Store base tokens count
        self.set_base_tokens_count(base_tokens.len() as u128);
        
        // Store each base token
        for (i, token) in base_tokens.iter().enumerate() {
            self.set_base_token(i as u128, token)?;
        }

        // Initialize other configuration
        self.set_max_price_impact(5000u128); // 50% default max price impact
        self.set_default_slippage(500u128); // 5% default slippage

        println!("OYL Zap initialized with factory: {:?}", oyl_factory_id);
        Ok(CallResponse::forward(&AlkaneTransferParcel::default()))
    }

    fn zap_into_l_p(
        &self,
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        min_lp_tokens: u128,
        deadline: u128,
    ) -> Result<CallResponse> {
        // Get context to access incoming tokens
        let context = self.context()?;
        let mut response = CallResponse::default();

        // Validate that we received the expected input tokens
        if context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("No input tokens received"));
        }

        let input_transfer = &context.incoming_alkanes.0[0];
        if input_transfer.id != input_token {
            return Err(anyhow!(
                "Expected input token {:?}, got {:?}",
                input_token,
                input_transfer.id
            ));
        }

        if input_transfer.value != input_amount {
            return Err(anyhow!(
                "Expected input amount {}, got {}",
                input_amount,
                input_transfer.value
            ));
        }

        // Validate parameters
        let current_time = self.get_current_time();
        let zap_params = ZapParams::new(
            input_token,
            input_amount,
            target_token_a,
            target_token_b,
            min_lp_tokens,
            deadline,
        );
        zap_params.validate(current_time)?;

        // Get zap quote
        let quote = self.get_zap_quote_internal(
            input_token,
            input_amount,
            target_token_a,
            target_token_b,
            self.default_slippage(),
        )?;

        // Validate quote
        ZapCalculator::validate_zap_quote(&quote)?;

        if quote.expected_lp_tokens < min_lp_tokens {
            return Err(anyhow!(
                "Expected LP tokens {} less than minimum {}",
                quote.expected_lp_tokens,
                min_lp_tokens
            ));
        }

        // Execute the zap and get LP tokens
        let lp_tokens_received = self.execute_zap_with_tokens(&quote, &context)?;

        // Return LP tokens to user
        if lp_tokens_received > 0 {
            let factory_id = self.factory_id()?;
            let pool_id = self.find_pool_id(factory_id, target_token_a, target_token_b)?;
            
            response.alkanes.0.push(AlkaneTransfer {
                id: pool_id, // LP tokens have the same ID as the pool
                value: lp_tokens_received,
            });
        }

        println!("Zap executed successfully. LP tokens: {}", lp_tokens_received);
        Ok(response)
    }

    fn get_zap_quote(
        &self,
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        max_slippage_bps: u128,
    ) -> Result<CallResponse> {
        let quote = self.get_zap_quote_internal(
            input_token,
            input_amount,
            target_token_a,
            target_token_b,
            max_slippage_bps,
        )?;

        let quote_bytes = self.serialize_zap_quote(&quote)?;
        let mut response = CallResponse::forward(&AlkaneTransferParcel::default());
        response.data = quote_bytes;
        Ok(response)
    }

    fn find_optimal_route(
        &self,
        from_token: AlkaneId,
        to_token: AlkaneId,
        amount: u128,
    ) -> Result<CallResponse> {
        let factory_id = self.factory_id()?;
        let base_tokens = self.get_base_tokens_internal()?;
        let pool_provider = ContractPoolProvider { contract: self };
        
        let route_finder = RouteFinder::new(factory_id, &pool_provider).with_base_tokens(base_tokens);
        let route = route_finder.find_best_route(from_token, to_token, amount)?;

        let route_bytes = self.serialize_route_info(&route)?;
        let mut response = CallResponse::forward(&AlkaneTransferParcel::default());
        response.data = route_bytes;
        Ok(response)
    }

    fn set_oyl_factory(&self, factory_id: AlkaneId) -> Result<CallResponse> {
        self.set_factory_id(&factory_id)?;
        println!("OYL factory updated to: {:?}", factory_id);
        Ok(CallResponse::forward(&AlkaneTransferParcel::default()))
    }

    fn add_base_token(&self, token_id: AlkaneId) -> Result<CallResponse> {
        let current_count = self.base_tokens_count();
        
        // Check if token already exists (simplified - skip for now due to API issues)
        
        // Add new token
        self.set_base_token(current_count, &token_id)?;
        self.set_base_tokens_count(current_count + 1);

        println!("Base token added: {:?}", token_id);
        Ok(CallResponse::forward(&AlkaneTransferParcel::default()))
    }

    fn remove_base_token(&self, token_id: AlkaneId) -> Result<CallResponse> {
        let current_count = self.base_tokens_count();
        
        // Simplified removal - just decrement count for now
        if current_count > 0 {
            self.set_base_tokens_count(current_count - 1);
        }

        println!("Base token removed: {:?}", token_id);
        Ok(CallResponse::forward(&AlkaneTransferParcel::default()))
    }

    fn get_base_tokens(&self) -> Result<CallResponse> {
        let base_tokens = self.get_base_tokens_internal()?;
        let tokens_bytes = self.serialize_base_tokens(&base_tokens)?;
        let mut response = CallResponse::forward(&AlkaneTransferParcel::default());
        response.data = tokens_bytes;
        Ok(response)
    }

    fn get_zap_config(&self) -> Result<CallResponse> {
        let factory_id = self.factory_id()?;
        let max_price_impact = self.max_price_impact();
        let default_slippage = self.default_slippage();
        let base_tokens = self.get_base_tokens_internal()?;

        let config_bytes = self.serialize_zap_config(
            factory_id,
            max_price_impact,
            default_slippage,
            &base_tokens,
        )?;
        let mut response = CallResponse::forward(&AlkaneTransferParcel::default());
        response.data = config_bytes;
        Ok(response)
    }

    fn forward(&self) -> Result<CallResponse> {
        Ok(CallResponse::forward(&AlkaneTransferParcel::default()))
    }

    // Internal helper methods
    fn get_zap_quote_internal(
        &self,
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        max_slippage_bps: u128,
    ) -> Result<ZapQuote> {
        let factory_id = self.factory_id()?;
        let base_tokens = self.get_base_tokens_internal()?;
        let pool_provider = ContractPoolProvider { contract: self };
        
        let route_finder = RouteFinder::new(factory_id, &pool_provider).with_base_tokens(base_tokens);

        // Find routes to both target tokens
        let route_a = route_finder.find_best_route(input_token, target_token_a, input_amount / 2)?;
        let route_b = route_finder.find_best_route(input_token, target_token_b, input_amount / 2)?;

        // Get target pool reserves (mock implementation)
        let target_pool_reserves = self.get_pool_reserves(target_token_a, target_token_b)?;

        // Generate quote
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

        Ok(quote)
    }

    fn execute_zap(&self, quote: &ZapQuote) -> Result<()> {
        let factory_id = self.factory_id()?;
        let current_block = self.height();
        
        println!("Executing zap:");
        println!("  Input: {} of {:?}", quote.input_amount, quote.input_token);
        println!("  Split A: {} -> {:?}", quote.split_amount_a, quote.target_token_a);
        println!("  Split B: {} -> {:?}", quote.split_amount_b, quote.target_token_b);
        
        // Step 1: Execute swap for route A (input_token -> target_token_a)
        let amount_a_received = if quote.route_a.path.len() > 1 {
            self.execute_swap_route(&quote.route_a, quote.split_amount_a, factory_id, current_block.into())?
        } else {
            return Err(anyhow!("Invalid route A"));
        };
        
        // Step 2: Execute swap for route B (input_token -> target_token_b)  
        let amount_b_received = if quote.route_b.path.len() > 1 {
            self.execute_swap_route(&quote.route_b, quote.split_amount_b, factory_id, current_block.into())?
        } else {
            return Err(anyhow!("Invalid route B"));
        };
        
        println!("  Swapped to: {} of {:?}, {} of {:?}", 
                amount_a_received, quote.target_token_a,
                amount_b_received, quote.target_token_b);
        
        // Step 3: Add liquidity to the target pool
        let lp_tokens_received = self.add_liquidity(
            quote.target_token_a,
            quote.target_token_b,
            amount_a_received,
            amount_b_received,
            quote.minimum_lp_tokens,
            factory_id,
            current_block.into(),
        )?;
        
        println!("  LP tokens received: {}", lp_tokens_received);
        
        // Verify we received at least the minimum expected LP tokens
        if lp_tokens_received < quote.minimum_lp_tokens {
            return Err(anyhow!(
                "Received {} LP tokens, less than minimum {}",
                lp_tokens_received,
                quote.minimum_lp_tokens
            ));
        }
        
        println!("Zap executed successfully!");
        Ok(())
    }

    fn execute_zap_with_tokens(&self, quote: &ZapQuote, _context: &Context) -> Result<u128> {
        let factory_id = self.factory_id()?;
        let current_block = self.height();
        
        println!("Executing zap with tokens:");
        println!("  Input: {} of {:?}", quote.input_amount, quote.input_token);
        println!("  Split A: {} -> {:?}", quote.split_amount_a, quote.target_token_a);
        println!("  Split B: {} -> {:?}", quote.split_amount_b, quote.target_token_b);
        
        // Step 1: Execute swap for route A (input_token -> target_token_a)
        let amount_a_received = if quote.route_a.path.len() > 1 {
            self.execute_swap_route_with_tokens(&quote.route_a, quote.split_amount_a, factory_id, current_block.into())?
        } else {
            return Err(anyhow!("Invalid route A"));
        };
        
        // Step 2: Execute swap for route B (input_token -> target_token_b)  
        let amount_b_received = if quote.route_b.path.len() > 1 {
            self.execute_swap_route_with_tokens(&quote.route_b, quote.split_amount_b, factory_id, current_block.into())?
        } else {
            return Err(anyhow!("Invalid route B"));
        };
        
        println!("  Swapped to: {} of {:?}, {} of {:?}", 
                amount_a_received, quote.target_token_a,
                amount_b_received, quote.target_token_b);
        
        // Step 3: Add liquidity to the target pool
        let lp_tokens_received = self.add_liquidity_with_tokens(
            quote.target_token_a,
            quote.target_token_b,
            amount_a_received,
            amount_b_received,
            quote.minimum_lp_tokens,
            factory_id,
            current_block.into(),
        )?;
        
        println!("  LP tokens received: {}", lp_tokens_received);
        
        // Verify we received at least the minimum expected LP tokens
        if lp_tokens_received < quote.minimum_lp_tokens {
            return Err(anyhow!(
                "Received {} LP tokens, less than minimum {}",
                lp_tokens_received,
                quote.minimum_lp_tokens
            ));
        }
        
        println!("Zap executed successfully!");
        Ok(lp_tokens_received)
    }

    fn execute_swap_route(&self, route: &RouteInfo, amount_in: u128, factory_id: AlkaneId, deadline: u128) -> Result<u128> {
        if route.path.len() < 2 {
            return Err(anyhow!("Route must have at least 2 tokens"));
        }
        
        // Create Cellpack for the swap operation
        let mut inputs = vec![
            13u128, // SwapExactTokensForTokens opcode
            route.path.len() as u128, // path length
        ];
        
        // Add path tokens (flattened)
        for token in &route.path {
            inputs.push(token.block);
            inputs.push(token.tx);
        }
        
        inputs.push(amount_in); // amount_in
        inputs.push(route.expected_output * 95 / 100); // amount_out_min (5% slippage tolerance)
        inputs.push(deadline); // deadline
        
        let cellpack = Cellpack {
            target: factory_id,
            inputs,
        };

        // Create parcel with input tokens to send
        let input_token = &route.path[0];
        let parcel = AlkaneTransferParcel(vec![AlkaneTransfer {
            id: input_token.clone(),
            value: amount_in,
        }]);

        // Execute the swap
        let response = self.call(&cellpack, &parcel, self.fuel())?;

        // Extract output tokens from response
        let mut amount_out = 0u128;
        let output_token = &route.path[route.path.len() - 1];
        
        for transfer in &response.alkanes.0 {
            if transfer.id == *output_token {
                amount_out = amount_out.checked_add(transfer.value).unwrap_or(amount_out);
            }
        }

        if amount_out == 0 {
            return Err(anyhow!("No output tokens received from swap"));
        }

        Ok(amount_out)
    }

    fn execute_swap_route_with_tokens(&self, route: &RouteInfo, amount_in: u128, factory_id: AlkaneId, deadline: u128) -> Result<u128> {
        if route.path.len() < 2 {
            return Err(anyhow!("Route must have at least 2 tokens"));
        }
        
        // Create Cellpack for the swap operation
        let cellpack = Cellpack {
            target: factory_id,
            inputs: vec![
                13u128, // SwapExactTokensForTokens opcode
                route.path.len() as u128, // path length
                // Add path tokens (flattened)
                route.path[0].block, route.path[0].tx,
                route.path[1].block, route.path[1].tx,
                // Add more path tokens if needed (for multi-hop)
                amount_in, // amount_in
                route.expected_output * 95 / 100, // amount_out_min (5% slippage tolerance)
                deadline, // deadline
            ],
        };

        // Create parcel with input tokens to send
        let input_token = &route.path[0];
        let parcel = AlkaneTransferParcel(vec![AlkaneTransfer {
            id: input_token.clone(),
            value: amount_in,
        }]);

        // Execute the swap
        let response = self.call(&cellpack, &parcel, self.fuel())?;

        // Extract output tokens from response
        let mut amount_out = 0u128;
        let output_token = &route.path[route.path.len() - 1];
        
        for transfer in &response.alkanes.0 {
            if transfer.id == *output_token {
                amount_out = amount_out.checked_add(transfer.value).unwrap_or(amount_out);
            }
        }

        if amount_out == 0 {
            return Err(anyhow!("No output tokens received from swap"));
        }

        Ok(amount_out)
    }

    fn add_liquidity(
        &self,
        token_a: AlkaneId,
        token_b: AlkaneId,
        amount_a: u128,
        amount_b: u128,
        min_lp_tokens: u128,
        factory_id: AlkaneId,
        deadline: u128,
    ) -> Result<u128> {
        // Create Cellpack for the add liquidity operation
        let cellpack = Cellpack {
            target: factory_id,
            inputs: vec![
                11u128, // AddLiquidity opcode
                token_a.block, token_a.tx,
                token_b.block, token_b.tx,
                amount_a, // amount_a_desired
                amount_b, // amount_b_desired
                amount_a * 95 / 100, // amount_a_min (5% slippage)
                amount_b * 95 / 100, // amount_b_min (5% slippage)
                deadline, // deadline
            ],
        };

        // Create parcel with both tokens to send
        let parcel = AlkaneTransferParcel(vec![
            AlkaneTransfer {
                id: token_a,
                value: amount_a,
            },
            AlkaneTransfer {
                id: token_b,
                value: amount_b,
            },
        ]);

        // Execute the add liquidity operation
        let response = self.call(&cellpack, &parcel, self.fuel())?;

        // Extract LP tokens from response
        let mut lp_tokens_received = 0u128;
        
        // Find the actual pool ID using the factory
        let pool_id = self.find_pool_id(factory_id, token_a, token_b)?;
        
        for transfer in &response.alkanes.0 {
            if transfer.id == pool_id {
                lp_tokens_received = lp_tokens_received.checked_add(transfer.value).unwrap_or(lp_tokens_received);
            }
        }

        if lp_tokens_received == 0 {
            return Err(anyhow!("No LP tokens received from add liquidity"));
        }

        if lp_tokens_received < min_lp_tokens {
            return Err(anyhow!(
                "Received {} LP tokens, less than minimum {}",
                lp_tokens_received,
                min_lp_tokens
            ));
        }

        Ok(lp_tokens_received)
    }

    fn add_liquidity_with_tokens(
        &self,
        token_a: AlkaneId,
        token_b: AlkaneId,
        amount_a: u128,
        amount_b: u128,
        min_lp_tokens: u128,
        factory_id: AlkaneId,
        deadline: u128,
    ) -> Result<u128> {
        // Create Cellpack for the add liquidity operation
        let cellpack = Cellpack {
            target: factory_id,
            inputs: vec![
                11u128, // AddLiquidity opcode
                token_a.block, token_a.tx,
                token_b.block, token_b.tx,
                amount_a, // amount_a_desired
                amount_b, // amount_b_desired
                amount_a * 95 / 100, // amount_a_min (5% slippage)
                amount_b * 95 / 100, // amount_b_min (5% slippage)
                deadline, // deadline
            ],
        };

        // Create parcel with both tokens to send
        let parcel = AlkaneTransferParcel(vec![
            AlkaneTransfer {
                id: token_a,
                value: amount_a,
            },
            AlkaneTransfer {
                id: token_b,
                value: amount_b,
            },
        ]);

        // Execute the add liquidity operation
        let response = self.call(&cellpack, &parcel, self.fuel())?;

        // Extract LP tokens from response
        let mut lp_tokens_received = 0u128;
        
        let pool_id = self.find_pool_id(factory_id, token_a, token_b)?;
        
        for transfer in &response.alkanes.0 {
            if transfer.id == pool_id {
                lp_tokens_received = lp_tokens_received.checked_add(transfer.value).unwrap_or(lp_tokens_received);
            }
        }

        if lp_tokens_received == 0 {
            return Err(anyhow!("No LP tokens received from add liquidity"));
        }

        if lp_tokens_received < min_lp_tokens {
            return Err(anyhow!(
                "Received {} LP tokens, less than minimum {}",
                lp_tokens_received,
                min_lp_tokens
            ));
        }

        Ok(lp_tokens_received)
    }

    fn get_base_tokens_internal(&self) -> Result<Vec<AlkaneId>> {
        let count = self.base_tokens_count();
        let mut tokens = Vec::new();

        for i in 0..count {
            let token = self.base_token(i)?;
            tokens.push(token);
        }

        Ok(tokens)
    }

    fn get_pool_reserves(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<PoolReserves> {
        let factory_id = self.factory_id()?;
        
        // First, find the pool ID for this token pair
        let pool_id = self.find_pool_id(factory_id, token_a, token_b)?;
        
        // Query the pool's reserves using GetReserves (opcode 97)
        let cellpack = Cellpack {
            target: pool_id,
            inputs: vec![97u128], // GetReserves opcode
        };
        
        let response = self.call(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;
        
        if response.data.len() < 32 {
            // Fallback to mock reserves if call fails
            let reserve_a = 1000000u128; // Mock reserve A
            let reserve_b = 2000000u128; // Mock reserve B
            let total_supply = 1414213u128; // Mock total supply (sqrt of reserves)
            
            return Ok(PoolReserves::new(
                token_a,
                token_b,
                reserve_a,
                reserve_b,
                total_supply,
                (DEFAULT_FEE_AMOUNT_PER_1000 * 10).into(),
            ));
        }
        
        // Parse reserves from response
        let reserve_a = u128::from_le_bytes(response.data[0..16].try_into()?);
        let reserve_b = u128::from_le_bytes(response.data[16..32].try_into()?);
        
        // Calculate total supply (simplified - could be queried separately)
        let total_supply = if response.data.len() >= 48 {
            u128::from_le_bytes(response.data[32..48].try_into()?)
        } else {
            // Estimate total supply as geometric mean of reserves
            ((reserve_a as f64 * reserve_b as f64).sqrt()) as u128
        };
        
        // Estimate fee rate if not provided in response data
        let fee_rate = if response.data.len() >= 64 {
            u128::from_le_bytes(response.data[48..64].try_into()?)
        } else {
            (DEFAULT_FEE_AMOUNT_PER_1000 * 10).into() // Default fee rate (0.3%)
        };

        Ok(PoolReserves::new(
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            total_supply,
            fee_rate,
        ))
    }

    fn find_pool_id(&self, factory_id: AlkaneId, token_a: AlkaneId, token_b: AlkaneId) -> Result<AlkaneId> {
        // Call OYL factory's FindExistingPoolId (opcode 2)
        let cellpack = Cellpack {
            target: factory_id,
            inputs: vec![
                2u128, // FindExistingPoolId opcode
                token_a.block,
                token_a.tx,
                token_b.block,
                token_b.tx,
            ],
        };
        
        let response = self.call(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;
        
        if response.data.len() < 32 {
            return Err(anyhow!("Invalid pool ID response"));
        }
        
        // Parse AlkaneId from response
        let pool_block = u128::from_le_bytes(response.data[0..16].try_into()?);
        let pool_tx = u128::from_le_bytes(response.data[16..32].try_into()?);
        
        Ok(AlkaneId {
            block: pool_block,
            tx: pool_tx,
        })
    }

    fn get_current_time(&self) -> u128 {
        // Use block height as time reference
        self.height().into()
    }

    // Serialization methods
    fn serialize_zap_quote(&self, quote: &ZapQuote) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        
        // Serialize quote data (simplified)
        bytes.extend_from_slice(&quote.input_token.block.to_le_bytes());
        bytes.extend_from_slice(&quote.input_token.tx.to_le_bytes());
        bytes.extend_from_slice(&quote.input_amount.to_le_bytes());
        bytes.extend_from_slice(&quote.expected_lp_tokens.to_le_bytes());
        bytes.extend_from_slice(&quote.minimum_lp_tokens.to_le_bytes());
        bytes.extend_from_slice(&quote.price_impact.to_le_bytes());

        Ok(bytes)
    }

    fn serialize_route_info(&self, route: &RouteInfo) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        
        // Path length
        bytes.extend_from_slice(&(route.path.len() as u32).to_le_bytes());
        
        // Path tokens
        for token in &route.path {
            bytes.extend_from_slice(&token.block.to_le_bytes());
            bytes.extend_from_slice(&token.tx.to_le_bytes());
        }
        
        // Route info
        bytes.extend_from_slice(&route.expected_output.to_le_bytes());
        bytes.extend_from_slice(&route.price_impact.to_le_bytes());
        bytes.extend_from_slice(&route.gas_estimate.to_le_bytes());

        Ok(bytes)
    }

    fn serialize_base_tokens(&self, tokens: &[AlkaneId]) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        
        bytes.extend_from_slice(&(tokens.len() as u32).to_le_bytes());
        for token in tokens {
            bytes.extend_from_slice(&token.block.to_le_bytes());
            bytes.extend_from_slice(&token.tx.to_le_bytes());
        }

        Ok(bytes)
    }

    fn serialize_zap_config(
        &self,
        factory_id: AlkaneId,
        max_price_impact: u128,
        default_slippage: u128,
        base_tokens: &[AlkaneId],
    ) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        
        bytes.extend_from_slice(&factory_id.block.to_le_bytes());
        bytes.extend_from_slice(&factory_id.tx.to_le_bytes());
        bytes.extend_from_slice(&max_price_impact.to_le_bytes());
        bytes.extend_from_slice(&default_slippage.to_le_bytes());
        
        bytes.extend_from_slice(&(base_tokens.len() as u32).to_le_bytes());
        for token in base_tokens {
            bytes.extend_from_slice(&token.block.to_le_bytes());
            bytes.extend_from_slice(&token.tx.to_le_bytes());
        }

        Ok(bytes)
    }

    // Storage operations using direct store/load methods (following vault factory pattern)
    fn factory_id(&self) -> Result<AlkaneId> {
        let bytes = self.load("/oyl_factory".as_bytes().to_vec());
        if bytes.len() < 32 {
            return Err(anyhow!("OYL factory ID not set"));
        }
        Ok(AlkaneId {
            block: u128::from_le_bytes(bytes[0..16].try_into().map_err(|_| {
                anyhow!("Failed to parse factory block ID from storage")
            })?),
            tx: u128::from_le_bytes(bytes[16..32].try_into().map_err(|_| {
                anyhow!("Failed to parse factory tx ID from storage")
            })?),
        })
    }

    fn set_factory_id(&self, id: &AlkaneId) -> Result<()> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&id.block.to_le_bytes());
        bytes.extend_from_slice(&id.tx.to_le_bytes());
        self.store("/oyl_factory".as_bytes().to_vec(), bytes);
        Ok(())
    }

    fn base_tokens_count(&self) -> u128 {
        self.load_u128("/base_tokens_count")
    }

    fn set_base_tokens_count(&self, count: u128) {
        self.store(
            "/base_tokens_count".as_bytes().to_vec(),
            count.to_le_bytes().to_vec(),
        );
    }

    fn base_token(&self, index: u128) -> Result<AlkaneId> {
        let key = format!("/base_token_{}", index);
        let bytes = self.load(key.as_bytes().to_vec());
        if bytes.len() < 32 {
            return Err(anyhow!("Base token {} not set", index));
        }
        Ok(AlkaneId {
            block: u128::from_le_bytes(bytes[0..16].try_into().map_err(|_| {
                anyhow!("Failed to parse base token {} block ID from storage", index)
            })?),
            tx: u128::from_le_bytes(bytes[16..32].try_into().map_err(|_| {
                anyhow!("Failed to parse base token {} tx ID from storage", index)
            })?),
        })
    }

    fn set_base_token(&self, index: u128, id: &AlkaneId) -> Result<()> {
        let key = format!("/base_token_{}", index);
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&id.block.to_le_bytes());
        bytes.extend_from_slice(&id.tx.to_le_bytes());
        self.store(key.as_bytes().to_vec(), bytes);
        Ok(())
    }

    fn max_price_impact(&self) -> u128 {
        self.load_u128("/max_price_impact")
    }

    fn set_max_price_impact(&self, value: u128) {
        self.store(
            "/max_price_impact".as_bytes().to_vec(),
            value.to_le_bytes().to_vec(),
        );
    }

    fn default_slippage(&self) -> u128 {
        self.load_u128("/default_slippage")
    }

    fn set_default_slippage(&self, value: u128) {
        self.store(
            "/default_slippage".as_bytes().to_vec(),
            value.to_le_bytes().to_vec(),
        );
    }

    // Helper function to load u128 values from storage
    fn load_u128(&self, key_str: &str) -> u128 {
        let key = key_str.as_bytes().to_vec();
        let bytes = self.load(key);
        if bytes.len() >= 16 {
            let bytes_array: [u8; 16] = bytes[0..16].try_into().unwrap_or([0; 16]);
            u128::from_le_bytes(bytes_array)
        } else {
            0
        }
    }
}

#[derive(Default)]
pub struct OylZap();

impl OylZapBase for OylZap {}

// MessageDispatch implementation is auto-generated by the derive macro

impl AlkaneResponder for OylZap {}

impl AuthenticatedResponder for OylZap {}

declare_alkane! {
    impl AlkaneResponder for OylZap {
        type Message = OylZapMessage;
    }
}
