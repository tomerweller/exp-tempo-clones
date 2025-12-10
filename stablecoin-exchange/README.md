# Stablecoin Exchange - Stellar Soroban Smart Contract

A Soroban port of the Tempo Stablecoin Exchange precompile, implementing a Central Limit Order Book (CLOB) optimized for stablecoin pairs.

## Overview

This contract provides a tick-based order book exchange designed for stablecoin trading pairs where prices remain close to parity. Orders are organized at discrete price ticks, with a linked list of orders at each tick level.

## Features

| Feature | Description |
|---------|-------------|
| **Tick-based Pricing** | Prices quantized to ticks for efficient matching |
| **Limit Orders** | Place bid/ask orders at specific price ticks |
| **Flip Orders** | Orders that auto-create opposite side when filled |
| **Order Cancellation** | Cancel orders with automatic refunds |
| **Swap Execution** | Market orders with slippage protection |
| **Balance Management** | Internal balances with withdrawal support |

## Constants

```
MIN_TICK = -2000
MAX_TICK = 2000
TICK_SPACING = 10
PRICE_SCALE = 100,000
MIN_ORDER_SIZE = 10,000,000 (~$10 with 6 decimals)
```

The tick range of ±2000 allows for approximately ±20% price deviation from parity, suitable for stablecoin pairs.

## Price Formula

```
price = PRICE_SCALE + (tick * 10)
```

At tick 0, price = 100,000 (1:1 parity)
At tick +100, price = 101,000 (1% premium)
At tick -100, price = 99,000 (1% discount)

## Deployment

**Testnet Contract ID**: `CA4GR5VNEEN2MGLNNDDX326QKBCYVVMTEMXVPPOWHPOM2NWP5Q4FG5BY`

## Building

```bash
cargo build --release --target wasm32-unknown-unknown
```

## Testing

```bash
cargo test
```

All 19 tests pass.

## API

### Initialization
- `initialize(admin)` - Initialize the exchange
- `create_pair(base_token, quote_token)` - Create a trading pair (admin-only)

### Order Placement
- `place(maker, base_token, quote_token, is_bid, tick, amount)` - Place limit order
- `place_flip(maker, base_token, quote_token, is_bid, tick, amount, flip_tick)` - Place flip order
- `execute_block(base_token, quote_token, order_ids)` - Activate pending orders

### Order Management
- `cancel(maker, order_id)` - Cancel an order

### Swapping
- `swap_exact_in(taker, base_token, quote_token, is_buy, amount_in, min_amount_out)` - Market swap
- `quote_swap_in(base_token, quote_token, is_buy, amount_in)` - Quote expected output

### Balance Management
- `balance_of(user, token)` - Get exchange balance
- `withdraw(user, token, amount)` - Withdraw tokens

### View Functions
- `get_orderbook(base_token, quote_token)` - Get orderbook state
- `get_order(order_id)` - Get active order
- `get_pending_order(order_id)` - Get pending order
- `get_tick_level(base_token, quote_token, is_bid, tick)` - Get tick level info

## Order Flow

1. **Place Order**: User calls `place()` which creates a pending order and transfers tokens
2. **Execute Block**: Validator calls `execute_block()` to activate pending orders into the orderbook
3. **Match**: When a swap occurs, orders are filled in price-time priority
4. **Settlement**: Filled amounts credited to maker's balance; withdraw to claim tokens

## Flip Orders

Flip orders automatically create an opposite-side order when fully filled:
- A bid flip order at tick 0 with flip_tick 100 will, when filled, create an ask at tick 100
- Useful for market makers who want to continuously provide liquidity on both sides

## Known Limitations

### Missing Access Control on execute_block

In the original Tempo implementation, `execute_block` is a **privileged function** that can only be called by the protocol (`Address::ZERO`) during block finalization:

```rust
// Original Tempo implementation
if sender != Address::ZERO {
    return Err(StablecoinExchangeError::unauthorized().into());
}
```

This design prevents:
- **Front-running**: Users cannot selectively activate favorable orders
- **MEV extraction**: No manipulation of order activation sequencing
- **Selective execution**: All pending orders are processed fairly by the protocol

**In this Soroban port, `execute_block` is permissionless** - any user can call it and choose which orders to activate. For production use, consider adding admin-only restriction or integrating with a trusted sequencer.

### Soroban Resource Limits

Soroban enforces strict limits on computation and ledger access per transaction. This contract has several operations that could hit these limits under certain conditions.

### Order Traversal in Swaps

The `swap_exact_in` function iterates through orders in a linked list at each tick level:

```rust
while amount_to_fill > 0 && current_order_id != 0 {
    let current_order = order::get_order(env, current_order_id)?;  // Ledger read
    // ... process order
}
```

Each order is a separate ledger entry read. Soroban limits transactions to ~100 ledger entries. A swap that needs to fill many small orders could exceed this limit and fail.

### Best Tick Discovery

Finding the next tick with liquidity iterates through ticks:

```rust
while tick >= MIN_TICK {
    let level = get_bid_tick_level(env, ...);  // Ledger read each iteration
    tick -= TICK_SPACING;
}
```

With 400 possible ticks (range of 4000 / spacing of 10), a sparse orderbook could require many ledger reads to find liquidity.

### Batch Order Execution

`execute_block` processes multiple pending orders, each requiring:
- Read pending order
- Read/write tick level
- Read/write tail order (for linked list)
- Write new active order

Processing 20+ orders in a single call could exceed ledger access limits.

### Potential Mitigations

- **Max iterations**: Cap loops with explicit limits and return partial results
- **Batch size limits**: Limit orders per `execute_block` call
- **Tick bitmaps**: Use bitmap to track which ticks have liquidity (avoid iterating empty ticks)
- **Pagination**: Split large swaps across multiple transactions
- **Order size minimums**: Increase `MIN_ORDER_SIZE` to reduce order fragmentation

## Original Implementation

Ported from [Tempo's Stablecoin Exchange Precompile](https://github.com/tempoxyz/tempo/tree/main/crates/precompiles/src/stablecoin_exchange)
