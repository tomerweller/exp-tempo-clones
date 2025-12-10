# Plan: Tempo Stablecoin CLOB Exchange → Stellar Soroban

## Overview

The Tempo Stablecoin Exchange is a **Central Limit Order Book (CLOB)** DEX optimized for stablecoin trading. It features tick-based pricing, flip orders (automatic opposite-side order creation), and multi-hop swap routing.

## Architecture Comparison

| Tempo Component | Soroban Equivalent |
|-----------------|-------------------|
| EVM Precompile (Rust) | Soroban Contract (Rust/Wasm) |
| `alloy::primitives::Address` | `soroban_sdk::Address` |
| `alloy::primitives::B256` | `soroban_sdk::BytesN<32>` |
| `U256` for amounts | `i128` (sufficient for stablecoins) |
| Storage slots | `env.storage().persistent()` |
| TIP20 tokens | SEP-41 Token Interface |
| Linked list in storage | Vec or Map-based structures |

## Core Data Structures

### Order
```rust
pub struct Order {
    pub order_id: u128,
    pub maker: Address,
    pub book_key: BytesN<32>,
    pub is_bid: bool,
    pub tick: i32,           // i16 in original, i32 for safety
    pub amount: i128,
    pub remaining: i128,
    pub prev: u128,          // Linked list pointer
    pub next: u128,          // Linked list pointer
    pub is_flip: bool,
    pub flip_tick: i32,
}
```

### TickLevel
```rust
pub struct TickLevel {
    pub head: u128,          // First order ID at this tick
    pub tail: u128,          // Last order ID at this tick
    pub total_liquidity: i128,
}
```

### Orderbook
```rust
pub struct Orderbook {
    pub base_token: Address,
    pub quote_token: Address,
    pub best_bid_tick: i32,
    pub best_ask_tick: i32,
}
```

## Key Constants

```rust
const MIN_TICK: i32 = -2000;
const MAX_TICK: i32 = 2000;
const TICK_SPACING: i32 = 10;
const PRICE_SCALE: i128 = 100_000;
const MIN_ORDER_SIZE: i128 = 10_000_000;  // $10 USD (6 decimals)
```

## Core Functions to Implement

### Order Management
| Function | Description |
|----------|-------------|
| `place()` | Place a limit order (bid or ask) |
| `place_flip()` | Place a flip order (auto-creates opposite side) |
| `cancel()` | Cancel a pending/active order |
| `execute_block()` | Process pending orders into orderbook |

### Trading
| Function | Description |
|----------|-------------|
| `swap_exact_amount_in()` | Swap with exact input amount |
| `swap_exact_amount_out()` | Swap with exact output amount |
| `quote_swap_in()` | Quote for exact input swap |
| `quote_swap_out()` | Quote for exact output swap |

### Orderbook Management
| Function | Description |
|----------|-------------|
| `create_pair()` | Create a new trading pair |
| `get_orderbook()` | Get orderbook state |
| `get_tick_level()` | Get liquidity at a price tick |

### Balance Management
| Function | Description |
|----------|-------------|
| `balance_of()` | Get user's exchange balance |
| `withdraw()` | Withdraw tokens from exchange |
| `deposit()` | Deposit tokens to exchange (implicit in place) |

### Price Utilities
| Function | Description |
|----------|-------------|
| `tick_to_price()` | Convert tick to price |
| `price_to_tick()` | Convert price to tick |

## Storage Keys (DataKey enum)

```rust
pub enum DataKey {
    // Admin
    Admin,

    // Order counters
    ActiveOrderId,
    PendingOrderId,

    // Orders
    Order(u128),                              // order_id -> Order
    PendingOrder(u128),                       // order_id -> Order

    // Orderbooks
    Orderbook(Address, Address),              // (base, quote) -> Orderbook

    // Tick levels
    BidTickLevel(BytesN<32>, i32),           // (book_key, tick) -> TickLevel
    AskTickLevel(BytesN<32>, i32),           // (book_key, tick) -> TickLevel

    // Tick bitmaps (for efficient tick discovery)
    BidBitmap(BytesN<32>, i32),              // (book_key, word_index) -> u256
    AskBitmap(BytesN<32>, i32),              // (book_key, word_index) -> u256

    // User balances
    Balance(Address, Address),                // (user, token) -> i128
}
```

## Implementation Phases

### Phase 1: Core Infrastructure (~200 LOC)
- [ ] Storage layer (DataKey, helpers)
- [ ] Error types
- [ ] Event definitions
- [ ] Constants

### Phase 2: Order & Orderbook Structures (~300 LOC)
- [ ] Order struct and methods
- [ ] TickLevel struct and methods
- [ ] Orderbook struct and methods
- [ ] Linked list operations

### Phase 3: Price Mathematics (~100 LOC)
- [ ] `tick_to_price()` - exponential price curve
- [ ] `price_to_tick()` - inverse calculation
- [ ] Tick bitmap operations for efficient discovery

### Phase 4: Order Placement (~250 LOC)
- [ ] `place()` - create pending order
- [ ] `place_flip()` - create flip order
- [ ] `execute_block()` - activate pending orders
- [ ] `cancel()` - remove orders

### Phase 5: Order Matching & Filling (~300 LOC)
- [ ] `partial_fill_order()` - partial fills
- [ ] `fill_order()` - complete fills
- [ ] Flip order handling
- [ ] Linked list maintenance

### Phase 6: Swap Execution (~250 LOC)
- [ ] `swap_exact_amount_in()` - taker sells exact amount
- [ ] `swap_exact_amount_out()` - taker buys exact amount
- [ ] Quote functions for both swap types
- [ ] Multi-tick traversal

### Phase 7: Balance & Withdrawal (~100 LOC)
- [ ] `deposit()` (internal, via place)
- [ ] `withdraw()`
- [ ] `balance_of()`
- [ ] Token transfers

### Phase 8: Tests (~500 LOC)
- [ ] Order placement tests
- [ ] Order cancellation tests
- [ ] Swap execution tests
- [ ] Flip order tests
- [ ] Edge cases (empty book, partial fills)

## File Structure

```
stablecoin-exchange-soroban/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Contract entry point & main logic
│   ├── storage.rs       # Storage keys and helpers
│   ├── order.rs         # Order struct and operations
│   ├── orderbook.rs     # Orderbook and tick level management
│   ├── price.rs         # Price/tick conversion math
│   ├── error.rs         # Error types
│   ├── events.rs        # Event definitions
│   └── test.rs          # Unit tests
```

## Complexity Assessment

| Component | Lines | Complexity | Notes |
|-----------|-------|------------|-------|
| Storage | ~150 | Medium | Many key types |
| Order | ~200 | Medium | Linked list logic |
| Orderbook | ~250 | High | Bitmap operations |
| Price math | ~100 | Medium | Fixed-point arithmetic |
| Placement | ~250 | High | State coordination |
| Matching | ~300 | High | Complex fill logic |
| Swaps | ~250 | High | Multi-tick traversal |
| Balances | ~100 | Low | Simple accounting |
| Tests | ~500 | Medium | Coverage needed |
| **Total** | **~2100** | - | - |

## Key Differences from Tempo Implementation

1. **No hardfork logic** - Single implementation path
2. **Simplified bitmap** - Use Soroban's native storage patterns
3. **i128 vs U256** - Sufficient for stablecoin amounts
4. **Direct token transfers** - SEP-41 instead of TIP20
5. **No gas metering** - Soroban handles resource limits differently

## Risk Areas

1. **Linked list in storage** - Gas-expensive, may need optimization
2. **Bitmap operations** - Complex bit manipulation in Soroban
3. **Multi-tick swaps** - May hit instruction limits on deep books
4. **Order expiry** - Consider TTL for stale orders

## Estimated Effort

- **Development**: 4-6 hours
- **Testing**: 2-3 hours
- **Deployment & Verification**: 1 hour

## Next Steps

1. Implement Phase 1-2 (infrastructure + data structures)
2. Implement Phase 3-4 (price math + order placement)
3. Implement Phase 5-6 (matching + swaps)
4. Implement Phase 7-8 (balances + tests)
5. Deploy to testnet and verify functionality
