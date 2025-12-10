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
- `create_pair(caller, base_token, quote_token)` - Create a trading pair

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

## Original Implementation

Ported from [Tempo's Stablecoin Exchange Precompile](https://github.com/tempoxyz/tempo/tree/main/crates/precompiles/src/stablecoin_exchange)
