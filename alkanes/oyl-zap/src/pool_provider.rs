use crate::types::PoolReserves;
use alkanes_support::id::AlkaneId;
use anyhow::Result;

/// A trait for providing pool data. This allows for decoupling the routing logic
/// from the specific data source, making it easier to test with mock data or
/// connect to a live data source.
pub trait PoolProvider {
    /// Get the reserves for a specific pool.
    fn get_pool_reserves(&self, token_a: AlkaneId, token_b: AlkaneId) -> Result<PoolReserves>;

    /// Get all tokens connected to a given token through existing pools.
    fn get_connected_tokens(&self, token: AlkaneId) -> Result<Vec<AlkaneId>>;
}