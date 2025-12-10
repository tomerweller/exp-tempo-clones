# Plan: Tempo Fee AMM → Stellar Soroban Smart Contract

## Overview

The Tempo Fee AMM is a specialized AMM for managing fee swaps between user tokens and validator tokens. It differs from standard constant-product AMMs by using fixed fee multipliers and supporting pending fee swap reservations.

## Key Components to Port

| Tempo Component | Soroban Equivalent |
|-----------------|-------------------|
| EVM Precompile (Rust) | Soroban Contract (Rust/Wasm) |
| `alloy::primitives::U256` | `soroban_sdk::U256` or `i128` |
| `alloy::primitives::Address` | `soroban_sdk::Address` |
| `keccak256` pool ID | `soroban_sdk::crypto` or composite key |
| Storage slots | `env.storage().persistent()` |
| TIP20 tokens | SEP-41 Token Interface |

## Contract Structure

```rust
// DataKey enum for storage
pub enum DataKey {
    Pool(Address, Address),           // (user_token, validator_token) -> Pool
    TotalSupply(Address, Address),    // LP token supply per pool
    LPBalance(Address, Address, Address), // (user_token, validator_token, user) -> balance
    PendingFeeSwap(Address, Address), // Pending swap amounts
}

pub struct Pool {
    pub reserve_user_token: i128,
    pub reserve_validator_token: i128,
}
```

## Core Functions to Implement

### 1. Initialization
```rust
fn initialize(env: Env, admin: Address) -> Result<(), Error>
```

### 2. Liquidity Management
| Function | Description |
|----------|-------------|
| `mint()` | Add liquidity with both tokens, receive LP tokens |
| `mint_with_validator_token()` | Single-sided deposit using validator tokens only |
| `burn()` | Withdraw liquidity, return both token types |

### 3. Fee Swap Operations
| Function | Description |
|----------|-------------|
| `reserve_liquidity()` | Reserve liquidity for pending fee swaps |
| `release_liquidity()` | Release reserved liquidity (refund) |
| `execute_pending_fee_swaps()` | Execute accumulated fee conversions |
| `rebalance_swap()` | Swap validator→user tokens to rebalance pool |

### 4. View Functions
| Function | Description |
|----------|-------------|
| `get_pool()` | Get pool reserves |
| `get_total_supply()` | Get LP token total supply |
| `get_lp_balance()` | Get user's LP token balance |
| `get_pending_fee_swap_in()` | Get pending swap amount |

## Key Constants

```rust
const M: i128 = 9970;      // Fee multiplier (0.997 scaled by 10000)
const N: i128 = 9985;      // Rebalance multiplier
const SCALE: i128 = 10000; // Scaling factor
const MIN_LIQUIDITY: i128 = 1000;
```

## Implementation Considerations

### 1. Numeric Precision
- Tempo uses `U256` for large numbers
- Soroban's native `i128` covers most use cases (up to ~170 undecillion)
- Use `soroban_sdk::U256` if larger ranges needed

### 2. Token Interaction
- Use `soroban_sdk::token::Client` for SEP-41 token calls
- Replace `system_transfer_from` with standard `transfer_from` (requires approval)

### 3. Pool Identification
- Tempo uses `keccak256(abi.encode(user_token, validator_token))`
- Soroban: Use composite `DataKey` tuple directly (more efficient)

### 4. Access Control
- Replace USD currency validation with admin-managed allowlists
- Consider adding `require_auth()` for privileged operations

### 5. Events
- Replace `emit_event` with Soroban's `env.events().publish()`

## File Structure

```
tempo-fee-amm-soroban/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Contract entry point
│   ├── storage.rs       # Storage keys and helpers
│   ├── pool.rs          # Pool struct and operations
│   ├── amm.rs           # AMM logic (mint, burn, swap)
│   ├── error.rs         # Custom error types
│   └── test.rs          # Unit tests
```

## Implementation Order

1. **Project setup** - Cargo.toml with `soroban-sdk` dependency
2. **Storage layer** - DataKey enum and storage helpers
3. **Pool struct** - Reserve management
4. **Core AMM functions** - `mint()`, `burn()`, `compute_amount_out()`
5. **Fee swap system** - `reserve_liquidity()`, `execute_pending_fee_swaps()`
6. **Rebalance logic** - `rebalance_swap()`
7. **Events** - Mint, Burn, FeeSwap, RebalanceSwap
8. **Tests** - Port the existing Rust test suite

## Estimated Complexity

| Component | Lines of Code | Complexity |
|-----------|---------------|------------|
| Storage layer | ~50 | Low |
| Pool operations | ~100 | Medium |
| Mint/Burn | ~200 | Medium |
| Fee swap system | ~150 | High |
| Rebalance | ~80 | Medium |
| Tests | ~400 | Medium |
| **Total** | **~980** | - |

## Sources

- [Stellar Soroban Overview](https://developers.stellar.org/docs/build/smart-contracts/overview)
- [Soroban Liquidity Pool Example](https://developers.stellar.org/docs/build/smart-contracts/example-contracts/liquidity-pool)
- [Soroban Rust SDK](https://github.com/stellar/rs-soroban-sdk)
- [Soroban Examples Repository](https://github.com/stellar/soroban-examples)
- [Soroswap Technical Reference](https://docs.soroswap.finance/01-protocol-overview/03-technical-reference)
