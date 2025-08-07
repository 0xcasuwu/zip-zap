# Zap Testing Suite

This directory contains a comprehensive testing suite for the zip-zap project, modeled after the boiler testing patterns. The testing suite provides both unit tests and integration tests that interact with the alkanes indexer.

## Overview

The testing suite is designed to verify:
- **Deployment Patterns**: Testing the specific deployment behaviors (3→4, 2→2, 6→4→2)
- **Zap Functionality**: End-to-end zap operations with trace analysis
- **Multi-User Scenarios**: Concurrent zap operations and proportional results
- **Route Finding**: Optimal path selection and multi-hop routing
- **Edge Cases**: Error handling and robustness testing

## Test Structure

### Unit Tests (`mod.rs`)
- Basic functionality tests for core types and calculations
- Mock-based testing for isolated component verification
- Fast execution, no external dependencies

### Integration Tests (`zap_integration_test.rs`)
- Full blockchain simulation using alkanes indexer
- Comprehensive trace analysis similar to boiler tests
- Real contract deployment and interaction testing
- Mathematical verification of zap calculations

### Test Runner (`test_runner.rs`)
- Structured test execution with detailed reporting
- Performance benchmarking capabilities
- JSON export for CI/CD integration
- Configurable test scenarios

## Key Features

### 1. Deployment Pattern Testing
Tests the specific deployment patterns mentioned in the requirements:
- Deploy to block 3 → outputs to block 4
- Deploy to block 2 → stays at block 2
- Target block 6 → looks for block 4 to spawn at block 2

### 2. Comprehensive Trace Analysis
Following the boiler testing patterns:
- Captures transaction traces at each step
- Analyzes contract responses and state changes
- Verifies mathematical calculations against expected results
- Provides detailed debugging information

### 3. Multi-User Testing
- Simulates concurrent zap operations
- Verifies proportional LP token distribution
- Tests different slippage tolerances
- Analyzes efficiency ratios between users

### 4. Mathematical Verification
- Validates AMM swap calculations
- Verifies LP token minting formulas
- Tests slippage protection mechanisms
- Confirms fee calculations (0.3% standard)

## Running Tests

### Basic Unit Tests
```bash
cd projects/zip-zap
cargo test
```

### Integration Tests (WASM)
```bash
cd projects/zip-zap
wasm-pack test --node
```

### Using the Test Runner
```rust
use crate::tests::test_runner::{ZapTestRunner, TestConfig};

// Run all tests with default configuration
let mut runner = ZapTestRunner::new(true);
runner.run_all_tests().unwrap();

// Run with custom configuration
let config = TestConfig {
    verbose: true,
    test_deployment_patterns: true,
    test_multi_user: true,
    test_edge_cases: true,
};
let runner = run_zap_tests_with_config(config).unwrap();

// Export results for CI/CD
let json_results = runner.export_results_json();
```

## Test Functions

### `test_zap_deployment_patterns()`
- **Purpose**: Verify deployment pattern behaviors
- **Tests**: 3→4, 2→2, 6→4→2 patterns
- **Verification**: Contract addresses match expected deployment locations

### `test_basic_zap_flow()`
- **Purpose**: End-to-end zap operation testing
- **Flow**: Quote → Execute → Verify LP tokens received
- **Verification**: Mathematical accuracy of zap calculations

### `test_multi_user_zap_scenarios()`
- **Purpose**: Concurrent user testing
- **Scenarios**: Multiple users with different parameters
- **Verification**: Proportional results and efficiency analysis

### `test_zap_route_finding()`
- **Purpose**: Route optimization testing
- **Scenarios**: Direct routes, indirect routes, multi-hop paths
- **Verification**: Optimal path selection and gas efficiency

### `test_zap_edge_cases()`
- **Purpose**: Error handling and robustness
- **Cases**: Zero amounts, expired deadlines, insufficient tokens
- **Verification**: Proper error responses and system stability

## Trace Analysis

The integration tests capture comprehensive trace data similar to the boiler testing suite:

```rust
// Example trace analysis
for vout in 0..5 {
    let trace_data = &view::trace(&OutPoint {
        txid: zap_block.txdata[0].compute_txid(),
        vout,
    })?;
    let trace_result: alkanes_support::trace::Trace = 
        alkanes_support::proto::alkanes::AlkanesTrace::parse_from_bytes(trace_data)?.into();
    let trace_guard = trace_result.0.lock().unwrap();
    if !trace_guard.is_empty() {
        println!("   • Zap vout {} trace: {:?}", vout, *trace_guard);
    }
}
```

## Mathematical Verification

The tests include comprehensive mathematical verification:

### AMM Swap Formula
```rust
// Uniswap V2 formula with 0.3% fee
let amount_in_with_fee = amount_in * 997;
let numerator = amount_in_with_fee * reserve_out;
let denominator = reserve_in * 1000 + amount_in_with_fee;
let amount_out = numerator / denominator;
```

### LP Token Calculation
```rust
// For new pools
let lp_tokens = integer_sqrt(amount_a * amount_b);

// For existing pools
let lp_tokens = std::cmp::min(
    amount_a * total_supply / reserve_a,
    amount_b * total_supply / reserve_b
);
```

### Slippage Protection
```rust
let min_lp_tokens = expected_lp * (10000 - slippage_bps) / 10000;
```

## Performance Benchmarking

The test runner includes performance benchmarking:

```rust
let mut benchmark = ZapBenchmark::new();
benchmark.record("zap_execution", duration_ms);
benchmark.record("route_finding", duration_ms);
benchmark.print_summary();
```

## CI/CD Integration

Export test results for automated systems:

```rust
let json_results = runner.export_results_json();
// JSON format includes:
// - total_tests, passed_tests, failed_tests
// - total_duration_ms
// - detailed results for each test
```

## Debugging

For debugging failed tests:

1. **Enable Verbose Output**: Set `verbose: true` in test configuration
2. **Analyze Traces**: Review the detailed trace output for each operation
3. **Check Mathematical Calculations**: Verify expected vs actual results
4. **Review Deployment Patterns**: Ensure contracts deployed to correct blocks

## Best Practices

1. **Run Tests Frequently**: Integration tests catch deployment issues early
2. **Monitor Performance**: Use benchmarking to detect performance regressions
3. **Analyze Traces**: Detailed trace analysis helps debug complex issues
4. **Test Edge Cases**: Comprehensive edge case testing ensures robustness
5. **Verify Mathematics**: Always validate calculations against expected formulas

## Comparison to Boiler Tests

This testing suite follows the same patterns as the boiler withdrawal verification tests:

| Feature | Boiler Tests | Zap Tests |
|---------|-------------|-----------|
| Ecosystem Setup | ✅ Multi-phase deployment | ✅ Multi-phase deployment |
| Trace Analysis | ✅ Comprehensive traces | ✅ Comprehensive traces |
| Mathematical Verification | ✅ Reward calculations | ✅ AMM calculations |
| Multi-User Testing | ✅ Overlapping stakes | ✅ Concurrent zaps |
| Edge Case Testing | ✅ QA scenarios | ✅ Error conditions |
| Performance Analysis | ✅ Time/stake weighting | ✅ Route efficiency |

## Future Enhancements

Potential improvements to the testing suite:

1. **Fuzzing Tests**: Random input generation for robustness testing
2. **Gas Optimization**: Detailed gas usage analysis and optimization
3. **Cross-Chain Testing**: Multi-chain zap operation testing
4. **Load Testing**: High-volume concurrent operation testing
5. **Integration with Real Pools**: Testing against actual AMM pools

## Contributing

When adding new tests:

1. Follow the existing patterns from boiler tests
2. Include comprehensive trace analysis
3. Add mathematical verification where applicable
4. Update this README with new test descriptions
5. Ensure tests are deterministic and repeatable

## Troubleshooting

Common issues and solutions:

### Test Failures
- **Deployment Pattern Issues**: Check block numbers in deployment configuration
- **Mathematical Mismatches**: Verify AMM formulas and precision handling
- **Trace Analysis Errors**: Ensure proper indexer setup and block indexing

### Performance Issues
- **Slow Test Execution**: Consider reducing test complexity or parallelization
- **Memory Usage**: Monitor WASM memory usage during complex operations

### Integration Issues
- **Indexer Problems**: Verify alkanes indexer is properly configured
- **Contract Deployment**: Check WASM compilation and deployment patterns
