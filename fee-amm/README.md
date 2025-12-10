# Fee AMM - Stellar Soroban Smart Contract

A Soroban port of the Tempo Fee AMM precompile, implementing an Automated Market Maker with specialized fee swap mechanics.

## Overview

This contract provides liquidity pool functionality with two distinct swap types:
- **Fee Swaps**: Standard swaps with 0.3% fee (multiplier M = 9970)
- **Rebalance Swaps**: Privileged swaps with 0.15% fee (multiplier N = 9985)

The AMM uses a constant product formula (x * y = k) and includes a pending fee swap reservation system for atomic operations.

## Features

| Feature | Description |
|---------|-------------|
| **Mint** | Add liquidity and receive LP tokens |
| **Burn** | Remove liquidity by burning LP tokens |
| **Fee Swap** | Standard swap with 0.3% fee |
| **Rebalance Swap** | Lower-fee swap (0.15%) for rebalancing |
| **Reserve Liquidity** | Lock liquidity for pending fee swaps |
| **Execute Pending** | Complete reserved fee swaps atomically |

## Constants

```
SCALE = 10000
M = 9970      (fee swap multiplier, 0.3% fee)
N = 9985      (rebalance multiplier, 0.15% fee)
MIN_LIQUIDITY = 1000
```

## Deployment

**Testnet Contract ID**: `CD4HRU5WQSU2O4PWGRURWPO5J6XPM2O52ESFBRKUHIRMPHMNH5EEICWM`

## Building

```bash
cargo build --release --target wasm32-unknown-unknown
```

## Testing

```bash
cargo test
```

All 17 tests pass.

## API

### Initialization
- `initialize(admin, token_a, token_b)` - Initialize the AMM with two tokens

### Liquidity
- `mint(to, amount_a, amount_b, min_liquidity)` - Add liquidity
- `burn(from, liquidity, min_a, min_b)` - Remove liquidity

### Swapping
- `fee_swap(amount_in, token_in, min_out)` - Standard swap (0.3% fee)
- `rebalance_swap(amount_in, token_in, min_out)` - Rebalance swap (0.15% fee)

### Fee Swap Reservations
- `reserve_liquidity(reserver, token, amount)` - Reserve for pending swap
- `execute_pending_fee_swaps(pending_swaps)` - Execute reserved swaps

### View Functions
- `get_reserves()` - Get current reserves
- `balance_of(address)` - Get LP token balance
- `total_supply()` - Get total LP tokens

## Known Limitations

### Access Control Differences from Original Tempo

In the original Tempo implementation, several functions are likely **system-level operations** called by the protocol during transaction processing or block finalization, not by regular users.

| Function | This Port | Original Tempo (likely) |
|----------|-----------|------------------------|
| `reserve_liquidity` | Admin-only | Protocol/system call |
| `execute_pending_fee_swaps` | Admin-only | Protocol/system call |
| `rebalance_swap` | Permissionless | Possibly validator-only |

**Implications:**
- `reserve_liquidity` and `execute_pending_fee_swaps` use admin-only access as an approximation of protocol-level access
- `rebalance_swap` is currently permissionless - anyone with validator tokens can rebalance. In Tempo, this may be restricted to validators.

For production use, consider integrating these functions with your fee collection and validator systems rather than exposing them directly.

### Pending Swaps Vector Growth

The `pending_swaps` are stored in a single `Vec<PendingFeeSwap>` ledger entry. Soroban enforces a ~64KB limit on individual ledger entries. Each pending swap is ~100+ bytes, meaning ~500-600 pending swaps could exceed this limit and cause transactions to fail.

**Potential mitigations:**
- Add a `MAX_PENDING_SWAPS` constant and reject new reservations when full
- Process pending swaps more frequently to prevent accumulation

## Original Implementation

Ported from [Tempo's Fee AMM Precompile](https://github.com/tempoxyz/tempo/blob/main/crates/precompiles/src/tip_fee_manager/amm.rs)
