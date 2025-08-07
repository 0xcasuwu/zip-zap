// Re-export the core zap functionality
pub use oyl_zap_core::*;

// Precompiled WASM modules for testing
pub mod precompiled;

#[cfg(test)]
mod tests;
