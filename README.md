# Tempo Clones for Stellar Soroban

Soroban smart contract ports of [Tempo](https://github.com/tempoxyz/tempo) precompiles for the Stellar network.

## Contracts

### [Fee AMM](./fee-amm)

An Automated Market Maker with specialized fee swap mechanics, featuring:
- Constant product formula (x * y = k)
- Two fee tiers: 0.3% (standard) and 0.15% (rebalance)
- Pending fee swap reservation system for atomic operations
- LP token minting/burning

**Testnet**: `CD4HRU5WQSU2O4PWGRURWPO5J6XPM2O52ESFBRKUHIRMPHMNH5EEICWM`

### [Stablecoin Exchange](./stablecoin-exchange)

A Central Limit Order Book (CLOB) optimized for stablecoin pairs, featuring:
- Tick-based pricing (±20% range from parity)
- Limit orders with price-time priority
- Flip orders that auto-create opposite side when filled
- Internal balance management with withdrawals

**Testnet**: `CA4GR5VNEEN2MGLNNDDX326QKBCYVVMTEMXVPPOWHPOM2NWP5Q4FG5BY`

## Building

Each contract can be built independently:

```bash
cd fee-amm
cargo build --release --target wasm32-unknown-unknown

cd stablecoin-exchange
cargo build --release --target wasm32-unknown-unknown
```

## Testing

```bash
cd fee-amm && cargo test
cd stablecoin-exchange && cargo test
```

## Original Implementations

- [Tempo Fee AMM](https://github.com/tempoxyz/tempo/blob/main/crates/precompiles/src/tip_fee_manager/amm.rs)
- [Tempo Stablecoin Exchange](https://github.com/tempoxyz/tempo/tree/main/crates/precompiles/src/stablecoin_exchange)
