use soroban_sdk::{contracttype, Address, Env};

use crate::error::Error;
use crate::storage::{extend_persistent_ttl, DataKey};

/// Constants for the orderbook
pub const MIN_TICK: i32 = -2000;
pub const MAX_TICK: i32 = 2000;
pub const TICK_SPACING: i32 = 10;
pub const PRICE_SCALE: i128 = 100_000;
pub const MIN_ORDER_SIZE: i128 = 10_000_000; // $10 with 6 decimals

/// Represents liquidity at a specific price tick
#[contracttype]
#[derive(Clone, Debug, Default)]
pub struct TickLevel {
    /// First order ID at this tick (0 if empty)
    pub head: u128,
    /// Last order ID at this tick (0 if empty)
    pub tail: u128,
    /// Total liquidity at this tick
    pub total_liquidity: i128,
}

impl TickLevel {
    pub fn is_empty(&self) -> bool {
        self.head == 0
    }
}

/// Represents an orderbook for a trading pair
#[contracttype]
#[derive(Clone, Debug)]
pub struct Orderbook {
    /// Base token address
    pub base_token: Address,
    /// Quote token address
    pub quote_token: Address,
    /// Best (highest) bid tick
    pub best_bid_tick: i32,
    /// Best (lowest) ask tick
    pub best_ask_tick: i32,
}

impl Orderbook {
    pub fn new(base_token: Address, quote_token: Address) -> Self {
        Self {
            base_token,
            quote_token,
            best_bid_tick: MIN_TICK - 1, // No bids initially
            best_ask_tick: MAX_TICK + 1, // No asks initially
        }
    }

    pub fn has_bids(&self) -> bool {
        self.best_bid_tick >= MIN_TICK
    }

    pub fn has_asks(&self) -> bool {
        self.best_ask_tick <= MAX_TICK
    }
}


// ============ Orderbook Storage ============

pub fn save_orderbook(env: &Env, orderbook: &Orderbook) {
    let key = DataKey::Orderbook(
        orderbook.base_token.clone(),
        orderbook.quote_token.clone(),
    );
    env.storage().persistent().set(&key, orderbook);
    extend_persistent_ttl(env, &key);
}

pub fn get_orderbook(env: &Env, base_token: &Address, quote_token: &Address) -> Option<Orderbook> {
    let key = DataKey::Orderbook(base_token.clone(), quote_token.clone());
    let book = env.storage().persistent().get(&key);
    if book.is_some() {
        extend_persistent_ttl(env, &key);
    }
    book
}

pub fn has_orderbook(env: &Env, base_token: &Address, quote_token: &Address) -> bool {
    let key = DataKey::Orderbook(base_token.clone(), quote_token.clone());
    env.storage().persistent().has(&key)
}

// ============ Tick Level Storage ============

pub fn get_bid_tick_level(env: &Env, base_token: &Address, quote_token: &Address, tick: i32) -> TickLevel {
    let key = DataKey::BidTickLevel(base_token.clone(), quote_token.clone(), tick);
    let level = env.storage().persistent().get(&key).unwrap_or_default();
    if env.storage().persistent().has(&key) {
        extend_persistent_ttl(env, &key);
    }
    level
}

pub fn save_bid_tick_level(env: &Env, base_token: &Address, quote_token: &Address, tick: i32, level: &TickLevel) {
    let key = DataKey::BidTickLevel(base_token.clone(), quote_token.clone(), tick);
    env.storage().persistent().set(&key, level);
    extend_persistent_ttl(env, &key);
}

pub fn delete_bid_tick_level(env: &Env, base_token: &Address, quote_token: &Address, tick: i32) {
    let key = DataKey::BidTickLevel(base_token.clone(), quote_token.clone(), tick);
    env.storage().persistent().remove(&key);
}

pub fn get_ask_tick_level(env: &Env, base_token: &Address, quote_token: &Address, tick: i32) -> TickLevel {
    let key = DataKey::AskTickLevel(base_token.clone(), quote_token.clone(), tick);
    let level = env.storage().persistent().get(&key).unwrap_or_default();
    if env.storage().persistent().has(&key) {
        extend_persistent_ttl(env, &key);
    }
    level
}

pub fn save_ask_tick_level(env: &Env, base_token: &Address, quote_token: &Address, tick: i32, level: &TickLevel) {
    let key = DataKey::AskTickLevel(base_token.clone(), quote_token.clone(), tick);
    env.storage().persistent().set(&key, level);
    extend_persistent_ttl(env, &key);
}

pub fn delete_ask_tick_level(env: &Env, base_token: &Address, quote_token: &Address, tick: i32) {
    let key = DataKey::AskTickLevel(base_token.clone(), quote_token.clone(), tick);
    env.storage().persistent().remove(&key);
}

// ============ Tick Validation ============

pub fn validate_tick(tick: i32) -> Result<(), Error> {
    if tick < MIN_TICK || tick > MAX_TICK {
        return Err(Error::InvalidTick);
    }
    if tick % TICK_SPACING != 0 {
        return Err(Error::TickNotAligned);
    }
    Ok(())
}

pub fn align_tick_down(tick: i32) -> i32 {
    tick - (tick.rem_euclid(TICK_SPACING))
}

pub fn align_tick_up(tick: i32) -> i32 {
    let rem = tick.rem_euclid(TICK_SPACING);
    if rem == 0 {
        tick
    } else {
        tick + (TICK_SPACING - rem)
    }
}

// ============ Price/Tick Conversion ============

/// Convert tick to price
/// Price = PRICE_SCALE * (1.0001 ^ tick)
/// Approximation using integer math
pub fn tick_to_price(tick: i32) -> i128 {
    // Base price at tick 0 is PRICE_SCALE (100,000)
    // Each tick multiplies by 1.0001
    // We use a simplified linear approximation for small tick ranges
    // price = PRICE_SCALE * (1 + tick * 0.0001)
    // price = PRICE_SCALE + tick * 10

    // For a more accurate exponential, we'd need more complex math
    // But for stablecoins with small tick range, linear is reasonable
    let adjustment = (tick as i128) * 10;
    let price = PRICE_SCALE + adjustment;

    // Ensure price is always positive
    if price < 1 {
        1
    } else {
        price
    }
}

/// Convert price to tick (inverse of tick_to_price)
pub fn price_to_tick(price: i128) -> i32 {
    if price <= 0 {
        return MIN_TICK;
    }

    // Inverse of: price = PRICE_SCALE + tick * 10
    // tick = (price - PRICE_SCALE) / 10
    let tick = ((price - PRICE_SCALE) / 10) as i32;

    // Clamp to valid range
    if tick < MIN_TICK {
        MIN_TICK
    } else if tick > MAX_TICK {
        MAX_TICK
    } else {
        align_tick_down(tick)
    }
}

/// Calculate quote amount from base amount and tick (for bids: buying base with quote)
pub fn calculate_quote_amount(base_amount: i128, tick: i32) -> i128 {
    let price = tick_to_price(tick);
    // quote = base * price / PRICE_SCALE
    (base_amount * price) / PRICE_SCALE
}

/// Calculate base amount from quote amount and tick (for asks: selling base for quote)
pub fn calculate_base_amount(quote_amount: i128, tick: i32) -> i128 {
    let price = tick_to_price(tick);
    if price == 0 {
        return 0;
    }
    // base = quote * PRICE_SCALE / price
    (quote_amount * PRICE_SCALE) / price
}

// ============ Best Tick Discovery ============

/// Find the next initialized bid tick at or below the given tick
pub fn find_next_bid_tick(
    env: &Env,
    base_token: &Address,
    quote_token: &Address,
    from_tick: i32,
) -> Option<i32> {
    let mut tick = align_tick_down(from_tick);

    while tick >= MIN_TICK {
        let level = get_bid_tick_level(env, base_token, quote_token, tick);
        if !level.is_empty() {
            return Some(tick);
        }
        tick -= TICK_SPACING;
    }

    None
}

/// Find the next initialized ask tick at or above the given tick
pub fn find_next_ask_tick(
    env: &Env,
    base_token: &Address,
    quote_token: &Address,
    from_tick: i32,
) -> Option<i32> {
    let mut tick = align_tick_up(from_tick);

    while tick <= MAX_TICK {
        let level = get_ask_tick_level(env, base_token, quote_token, tick);
        if !level.is_empty() {
            return Some(tick);
        }
        tick += TICK_SPACING;
    }

    None
}

/// Update the best bid tick after an order is added or removed
pub fn update_best_bid_tick(env: &Env, orderbook: &mut Orderbook) {
    if let Some(tick) = find_next_bid_tick(env, &orderbook.base_token, &orderbook.quote_token, MAX_TICK) {
        orderbook.best_bid_tick = tick;
    } else {
        orderbook.best_bid_tick = MIN_TICK - 1;
    }
}

/// Update the best ask tick after an order is added or removed
pub fn update_best_ask_tick(env: &Env, orderbook: &mut Orderbook) {
    if let Some(tick) = find_next_ask_tick(env, &orderbook.base_token, &orderbook.quote_token, MIN_TICK) {
        orderbook.best_ask_tick = tick;
    } else {
        orderbook.best_ask_tick = MAX_TICK + 1;
    }
}
