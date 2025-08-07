use alkanes_runtime::{
    auth::AuthenticatedResponder, declare_alkane, message::MessageDispatch,
    runtime::AlkaneResponder, storage::StoragePointer,
};
use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};
use anyhow::{anyhow, Result};
use metashrew_support::compat::to_arraybuffer_layout;
use std::sync::Arc;

pub mod types;
pub mod amm_logic;
pub mod pool_provider;
pub mod route_finder;
pub mod zap_calculator;

// Re-export constants for tests
pub use types::{DEFAULT_FEE_AMOUNT_PER_1000, MAX_HOPS, BASIS_POINTS, MINIMUM_LIQUIDITY};

// Helper function for integer square root
fn integer_sqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    
    let mut x = n;
    let mut y = (x + 1) / 2;
    
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    
    x
}

#[derive(MessageDispatch)]
pub enum OylZapMessage {
    #[opcode(0)]
    InitializeZap {
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
    #[opcode(50)]
    Forward {},
}

pub trait ZapBase: AuthenticatedResponder {
    // Helper methods that need to be implemented
    fn get_pool_reserves_impl(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<(u128, u128)>;
    fn calculate_swap_output(&self, amount_in: u128, reserve_in: u128, reserve_out: u128) -> Result<u128>;
    fn execute_swap(&self, path: Vec<AlkaneId>, amount_in: u128, amount_out_min: u128, deadline: u128) -> Result<CallResponse>;
    fn add_liquidity(&self, token_a: AlkaneId, token_b: AlkaneId, amount_a: u128, amount_b: u128, amount_a_min: u128, amount_b_min: u128, deadline: u128) -> Result<CallResponse>;
    fn find_pool_id(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<AlkaneId>;

    fn initialize(&self, factory_id: AlkaneId, base_tokens: Vec<AlkaneId>) -> Result<CallResponse> {
        let context = self.context()?;
        // In a real implementation, this would store the factory_id and base_tokens
        // For now, just return success
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn add_pool(
        &self,
        token_a: AlkaneId,
        token_b: AlkaneId,
        reserve_a: u128,
        reserve_b: u128,
        total_supply: u128,
        fee_rate: u128,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        // In a real implementation, this would store pool data
        // For now, just return success
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn update_pool_reserves(
        &self,
        token_a: AlkaneId,
        token_b: AlkaneId,
        reserve_a: u128,
        reserve_b: u128,
        total_supply: u128,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        // In a real implementation, this would update stored pool data
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn get_zap_quote(
        &self,
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        max_slippage_bps: u128,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        // Get pool reserves for the target pair (call implementation method directly)
        let (reserve_a, reserve_b) = self.get_pool_reserves_impl(target_token_a, target_token_b)?;
        
        // Calculate optimal split (50/50 for simplicity, could be optimized)
        let split_amount = input_amount / 2;
        
        // Calculate swap outputs for each half
        let mut amount_a_out = 0u128;
        let mut amount_b_out = 0u128;
        
        if input_token == target_token_a {
            amount_a_out = split_amount;
            // Swap other half to token_b
            let (reserve_in, reserve_out) = self.get_pool_reserves_impl(input_token, target_token_b)?;
            amount_b_out = self.calculate_swap_output(split_amount, reserve_in, reserve_out)?;
        } else if input_token == target_token_b {
            amount_b_out = split_amount;
            // Swap other half to token_a
            let (reserve_in, reserve_out) = self.get_pool_reserves_impl(input_token, target_token_a)?;
            amount_a_out = self.calculate_swap_output(split_amount, reserve_in, reserve_out)?;
        } else {
            // Need to swap both halves
            let (reserve_in_a, reserve_out_a) = self.get_pool_reserves_impl(input_token, target_token_a)?;
            amount_a_out = self.calculate_swap_output(split_amount, reserve_in_a, reserve_out_a)?;
            
            let (reserve_in_b, reserve_out_b) = self.get_pool_reserves_impl(input_token, target_token_b)?;
            amount_b_out = self.calculate_swap_output(split_amount, reserve_in_b, reserve_out_b)?;
        }
        
        // Calculate expected LP tokens (simplified)
        let total_supply = reserve_a + reserve_b; // Simplified, should get actual total supply
        let expected_lp = if total_supply == 0 {
            integer_sqrt(amount_a_out * amount_b_out)
        } else {
            std::cmp::min(
                amount_a_out * total_supply / reserve_a,
                amount_b_out * total_supply / reserve_b
            )
        };
        
        // Apply slippage
        let min_lp_tokens = expected_lp * (10000 - max_slippage_bps) / 10000;
        
        // Pack quote data
        let mut data = Vec::new();
        data.extend_from_slice(&split_amount.to_le_bytes()); // split_amount
        data.extend_from_slice(&amount_a_out.to_le_bytes()); // expected_token_a
        data.extend_from_slice(&amount_b_out.to_le_bytes()); // expected_token_b
        data.extend_from_slice(&expected_lp.to_le_bytes());  // expected_lp_tokens
        data.extend_from_slice(&min_lp_tokens.to_le_bytes()); // min_lp_tokens
        
        response.data = data;
        Ok(response)
    }

    fn execute_zap(
        &self,
        input_token: AlkaneId,
        input_amount: u128,
        target_token_a: AlkaneId,
        target_token_b: AlkaneId,
        min_lp_tokens: u128,
        deadline: u128,
        max_slippage_bps: u128,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        
        // Basic deadline check
        if deadline != 0 && self.height() as u128 > deadline {
            return Err(anyhow!("Transaction deadline has passed"));
        }
        
        // Validate input amount from incoming alkanes
        if context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("No input tokens provided"));
        }
        
        let input_transfer = &context.incoming_alkanes.0[0];
        if input_transfer.id != input_token || input_transfer.value != input_amount {
            return Err(anyhow!("Input token mismatch"));
        }
        
        // Calculate optimal split (50/50 for simplicity)
        let split_amount = input_amount / 2;
        
        // Step 1: Execute swaps to get both target tokens
        let mut amount_a = 0u128;
        let mut amount_b = 0u128;
        
        if input_token == target_token_a {
            amount_a = split_amount;
            // Swap other half to token_b
            let swap_path = vec![input_token, target_token_b];
            let swap_result = self.execute_swap(swap_path, split_amount, 0, deadline)?;
            // Extract amount_b from swap result
            if !swap_result.alkanes.0.is_empty() {
                amount_b = swap_result.alkanes.0[0].value;
            }
        } else if input_token == target_token_b {
            amount_b = split_amount;
            // Swap other half to token_a
            let swap_path = vec![input_token, target_token_a];
            let swap_result = self.execute_swap(swap_path, split_amount, 0, deadline)?;
            // Extract amount_a from swap result
            if !swap_result.alkanes.0.is_empty() {
                amount_a = swap_result.alkanes.0[0].value;
            }
        } else {
            // Need to swap both halves
            let swap_path_a = vec![input_token, target_token_a];
            let swap_result_a = self.execute_swap(swap_path_a, split_amount, 0, deadline)?;
            if !swap_result_a.alkanes.0.is_empty() {
                amount_a = swap_result_a.alkanes.0[0].value;
            }
            
            let swap_path_b = vec![input_token, target_token_b];
            let swap_result_b = self.execute_swap(swap_path_b, split_amount, 0, deadline)?;
            if !swap_result_b.alkanes.0.is_empty() {
                amount_b = swap_result_b.alkanes.0[0].value;
            }
        }
        
        // Step 2: Add liquidity with the obtained tokens
        let amount_a_min = amount_a * (10000 - max_slippage_bps) / 10000;
        let amount_b_min = amount_b * (10000 - max_slippage_bps) / 10000;
        
        let liquidity_result = self.add_liquidity(
            target_token_a,
            target_token_b,
            amount_a,
            amount_b,
            amount_a_min,
            amount_b_min,
            deadline,
        )?;
        
        // Validate minimum LP tokens received
        let mut lp_tokens_received = 0u128;
        for transfer in &liquidity_result.alkanes.0 {
            // LP token should be the pool token
            if let Ok(pool_id) = self.find_pool_id(target_token_a, target_token_b) {
                if transfer.id == pool_id {
                    lp_tokens_received = transfer.value;
                    break;
                }
            }
        }
        
        if lp_tokens_received < min_lp_tokens {
            return Err(anyhow!(
                "Insufficient LP tokens received: {} < {}",
                lp_tokens_received,
                min_lp_tokens
            ));
        }
        
        Ok(liquidity_result)
    }

    fn get_best_route(
        &self,
        from_token: AlkaneId,
        to_token: AlkaneId,
        amount_in: u128,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        let route_data = format!(
            "BestRoute: from={:?}, to={:?}, amount_in={}",
            from_token, to_token, amount_in
        );
        
        response.data = route_data.as_bytes().to_vec();
        Ok(response)
    }

    fn get_pool_reserves(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        let reserves_data = format!(
            "PoolReserves: token_a={:?}, token_b={:?}",
            token_a, token_b
        );
        
        response.data = reserves_data.as_bytes().to_vec();
        Ok(response)
    }

    fn forward(&self) -> Result<CallResponse> {
        let context = self.context()?;
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }
}

#[derive(Default)]
pub struct OylZap();

impl AlkaneResponder for OylZap {}
impl AuthenticatedResponder for OylZap {}
impl ZapBase for OylZap {
    fn get_pool_reserves_impl(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<(u128, u128)> {
        OylZap::get_pool_reserves_impl(self, token_a, token_b)
    }

    fn calculate_swap_output(&self, amount_in: u128, reserve_in: u128, reserve_out: u128) -> Result<u128> {
        OylZap::calculate_swap_output(self, amount_in, reserve_in, reserve_out)
    }

    fn execute_swap(&self, path: Vec<AlkaneId>, amount_in: u128, amount_out_min: u128, deadline: u128) -> Result<CallResponse> {
        OylZap::execute_swap(self, path, amount_in, amount_out_min, deadline)
    }

    fn add_liquidity(&self, token_a: AlkaneId, token_b: AlkaneId, amount_a: u128, amount_b: u128, amount_a_min: u128, amount_b_min: u128, deadline: u128) -> Result<CallResponse> {
        OylZap::add_liquidity(self, token_a, token_b, amount_a, amount_b, amount_a_min, amount_b_min, deadline)
    }

    fn find_pool_id(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<AlkaneId> {
        OylZap::find_pool_id(self, token_a, token_b)
    }
}

impl OylZap {
    fn initialize_zap(&self, factory_id: AlkaneId, base_tokens: Vec<AlkaneId>) -> Result<CallResponse> {
        let context = self.context()?;
        self.observe_initialization()?;
        
        // Store the oyl-protocol factory ID for making AMM calls
        self.set_oyl_factory_id(&factory_id)?;
        
        // Store base tokens for routing
        self.set_base_tokens(&base_tokens)?;
        
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    // Storage functions
    fn oyl_factory_id(&self) -> Result<AlkaneId> {
        let bytes = self.load("/oyl_factory_id".as_bytes().to_vec());
        if bytes.len() < 32 {
            return Err(anyhow!("OYL factory ID not set"));
        }
        Ok(AlkaneId {
            block: u128::from_le_bytes(bytes[0..16].try_into().unwrap()),
            tx: u128::from_le_bytes(bytes[16..32].try_into().unwrap()),
        })
    }

    fn set_oyl_factory_id(&self, id: &AlkaneId) -> Result<()> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&id.block.to_le_bytes());
        bytes.extend_from_slice(&id.tx.to_le_bytes());
        self.store("/oyl_factory_id".as_bytes().to_vec(), bytes);
        Ok(())
    }

    fn base_tokens(&self) -> Result<Vec<AlkaneId>> {
        let bytes = self.load("/base_tokens".as_bytes().to_vec());
        if bytes.is_empty() {
            return Ok(Vec::new());
        }

        let mut tokens = Vec::new();
        let mut offset = 0;
        while offset + 32 <= bytes.len() {
            let block = u128::from_le_bytes(bytes[offset..offset+16].try_into().unwrap());
            let tx = u128::from_le_bytes(bytes[offset+16..offset+32].try_into().unwrap());
            tokens.push(AlkaneId { block, tx });
            offset += 32;
        }
        Ok(tokens)
    }

    fn set_base_tokens(&self, tokens: &[AlkaneId]) -> Result<()> {
        let mut bytes = Vec::new();
        for token in tokens {
            bytes.extend_from_slice(&token.block.to_le_bytes());
            bytes.extend_from_slice(&token.tx.to_le_bytes());
        }
        self.store("/base_tokens".as_bytes().to_vec(), bytes);
        Ok(())
    }

    // Real AMM interaction functions
    fn find_pool_id(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<AlkaneId> {
        let factory_id = self.oyl_factory_id()?;
        
        // Call oyl-protocol factory to find existing pool
        let cellpack = Cellpack {
            target: factory_id,
            inputs: vec![2, token_a.block, token_a.tx, token_b.block, token_b.tx], // FindExistingPoolId opcode
        };

        let response = self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;
        
        if response.data.len() < 32 {
            return Err(anyhow!("Pool not found for tokens {:?} and {:?}", token_a, token_b));
        }

        Ok(AlkaneId {
            block: u128::from_le_bytes(response.data[0..16].try_into().unwrap()),
            tx: u128::from_le_bytes(response.data[16..32].try_into().unwrap()),
        })
    }

    fn get_pool_reserves_impl(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<(u128, u128)> {
        let pool_id = self.find_pool_id(token_a, token_b)?;
        
        // Call pool to get reserves
        let cellpack = Cellpack {
            target: pool_id,
            inputs: vec![97], // GetReserves opcode
        };

        let response = self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;
        
        if response.data.len() < 32 {
            return Err(anyhow!("Failed to get pool reserves"));
        }

        let reserve_a = u128::from_le_bytes(response.data[0..16].try_into().unwrap());
        let reserve_b = u128::from_le_bytes(response.data[16..32].try_into().unwrap());
        
        Ok((reserve_a, reserve_b))
    }

    fn calculate_swap_output(&self, amount_in: u128, reserve_in: u128, reserve_out: u128) -> Result<u128> {
        if amount_in == 0 || reserve_in == 0 || reserve_out == 0 {
            return Ok(0);
        }

        // AMM formula: amount_out = (amount_in * 997 * reserve_out) / (reserve_in * 1000 + amount_in * 997)
        // Using 0.3% fee (997/1000)
        let amount_in_with_fee = amount_in * 997;
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * 1000 + amount_in_with_fee;
        
        Ok(numerator / denominator)
    }

    fn execute_swap(&self, path: Vec<AlkaneId>, amount_in: u128, amount_out_min: u128, deadline: u128) -> Result<CallResponse> {
        let factory_id = self.oyl_factory_id()?;
        
        // Call oyl-protocol factory to execute swap
        let cellpack = Cellpack {
            target: factory_id,
            inputs: vec![
                13, // SwapExactTokensForTokens opcode
                path.len() as u128,
            ],
        };

        // Add path tokens to inputs
        let mut inputs = cellpack.inputs;
        for token in &path {
            inputs.push(token.block);
            inputs.push(token.tx);
        }
        inputs.push(amount_in);
        inputs.push(amount_out_min);
        inputs.push(deadline);

        let swap_cellpack = Cellpack {
            target: factory_id,
            inputs,
        };

        // Create transfer parcel with input token
        let input_parcel = AlkaneTransferParcel(vec![AlkaneTransfer {
            id: path[0].clone(),
            value: amount_in,
        }]);

        self.call(&swap_cellpack, &input_parcel, self.fuel())
    }

    fn add_liquidity(&self, token_a: AlkaneId, token_b: AlkaneId, amount_a: u128, amount_b: u128, amount_a_min: u128, amount_b_min: u128, deadline: u128) -> Result<CallResponse> {
        let factory_id = self.oyl_factory_id()?;
        
        // Call oyl-protocol factory to add liquidity
        let cellpack = Cellpack {
            target: factory_id,
            inputs: vec![
                11, // AddLiquidity opcode
                token_a.block, token_a.tx,
                token_b.block, token_b.tx,
                amount_a, amount_b,
                amount_a_min, amount_b_min,
                deadline,
            ],
        };

        // Create transfer parcel with both tokens
        let liquidity_parcel = AlkaneTransferParcel(vec![
            AlkaneTransfer { id: token_a, value: amount_a },
            AlkaneTransfer { id: token_b, value: amount_b },
        ]);

        self.call(&cellpack, &liquidity_parcel, self.fuel())
    }
}

declare_alkane! {
    impl AlkaneResponder for OylZap {
        type Message = OylZapMessage;
    }
}
