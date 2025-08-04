# OYL Zap Contract

A smart contract that enables single-sided liquidity provision for the OYL AMM protocol. Users can provide any single token and the zap contract will automatically:

1. Find optimal swap routes to the target LP pair tokens
2. Split the input token optimally between the two target tokens
3. Execute the swaps
4. Provide liquidity to the target pool
5. Return LP tokens to the user

## Features

- **Single-Sided Entry**: Deposit any token to get LP tokens for any pair
- **Optimal Routing**: Finds the best swap paths with minimal price impact
- **Smart Splitting**: Calculates optimal token allocation for balanced LP provision
- **Slippage Protection**: Configurable minimum LP token output
- **Gas Optimized**: Efficient execution with minimal transaction overhead

## Architecture

```
Input Token → [Route Discovery] → [Optimal Split] → [Dual Swaps] → [LP Provision] → LP Tokens
```

## Usage

```rust
// Zap USDC into ETH/BTC LP
ZapIntoLP {
    input_token: USDC_ID,
    input_amount: 1000_000000, // 1000 USDC
    target_token_a: ETH_ID,
    target_token_b: BTC_ID,
    min_lp_tokens: 950_000000, // 95% slippage tolerance
    deadline: block_time + 300, // 5 minute deadline
}
```

## Contract Structure

- `alkanes/oyl-zap/`: Core zap contract implementation
- `src/lib.rs`: Main contract interface
- `src/tests/`: Comprehensive test suite

## Integration

The zap contract integrates with existing OYL protocol contracts:
- Uses OYL Factory for pool discovery and liquidity provision
- Leverages OYL Pool contracts for swapping
- Utilizes OYL Library for AMM calculations
