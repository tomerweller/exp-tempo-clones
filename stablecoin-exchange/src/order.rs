use soroban_sdk::{contracttype, Address, Env};

use crate::error::Error;
use crate::storage::{extend_persistent_ttl, DataKey};

/// Represents a limit order in the orderbook
#[contracttype]
#[derive(Clone, Debug)]
pub struct Order {
    /// Unique order identifier
    pub order_id: u128,
    /// Address of the order maker
    pub maker: Address,
    /// Base token address
    pub base_token: Address,
    /// Quote token address
    pub quote_token: Address,
    /// True for bid (buy base), false for ask (sell base)
    pub is_bid: bool,
    /// Price tick
    pub tick: i32,
    /// Original order amount
    pub amount: i128,
    /// Remaining unfilled amount
    pub remaining: i128,
    /// Previous order ID in the linked list (0 if head)
    pub prev: u128,
    /// Next order ID in the linked list (0 if tail)
    pub next: u128,
    /// Whether this is a flip order
    pub is_flip: bool,
    /// Target tick for the flipped order (only used if is_flip)
    pub flip_tick: i32,
}

impl Order {
    /// Create a new bid order
    pub fn new_bid(
        order_id: u128,
        maker: Address,
        base_token: Address,
        quote_token: Address,
        tick: i32,
        amount: i128,
    ) -> Self {
        Self {
            order_id,
            maker,
            base_token,
            quote_token,
            is_bid: true,
            tick,
            amount,
            remaining: amount,
            prev: 0,
            next: 0,
            is_flip: false,
            flip_tick: 0,
        }
    }

    /// Create a new ask order
    pub fn new_ask(
        order_id: u128,
        maker: Address,
        base_token: Address,
        quote_token: Address,
        tick: i32,
        amount: i128,
    ) -> Self {
        Self {
            order_id,
            maker,
            base_token,
            quote_token,
            is_bid: false,
            tick,
            amount,
            remaining: amount,
            prev: 0,
            next: 0,
            is_flip: false,
            flip_tick: 0,
        }
    }

    /// Create a new flip order (bid)
    pub fn new_flip_bid(
        order_id: u128,
        maker: Address,
        base_token: Address,
        quote_token: Address,
        tick: i32,
        amount: i128,
        flip_tick: i32,
    ) -> Result<Self, Error> {
        // For bids: flip_tick must be > tick (sell higher than buy)
        if flip_tick <= tick {
            return Err(Error::InvalidBidFlipTick);
        }
        Ok(Self {
            order_id,
            maker,
            base_token,
            quote_token,
            is_bid: true,
            tick,
            amount,
            remaining: amount,
            prev: 0,
            next: 0,
            is_flip: true,
            flip_tick,
        })
    }

    /// Create a new flip order (ask)
    pub fn new_flip_ask(
        order_id: u128,
        maker: Address,
        base_token: Address,
        quote_token: Address,
        tick: i32,
        amount: i128,
        flip_tick: i32,
    ) -> Result<Self, Error> {
        // For asks: flip_tick must be < tick (buy lower than sell)
        if flip_tick >= tick {
            return Err(Error::InvalidAskFlipTick);
        }
        Ok(Self {
            order_id,
            maker,
            base_token,
            quote_token,
            is_bid: false,
            tick,
            amount,
            remaining: amount,
            prev: 0,
            next: 0,
            is_flip: true,
            flip_tick,
        })
    }

    /// Fill a portion of the order
    pub fn fill(&mut self, amount: i128) -> Result<(), Error> {
        if amount > self.remaining {
            return Err(Error::FillExceedsRemaining);
        }
        self.remaining -= amount;
        Ok(())
    }

    /// Check if order is fully filled
    pub fn is_fully_filled(&self) -> bool {
        self.remaining == 0
    }

    /// Create the flipped order after this order is fully filled
    pub fn create_flipped_order(&self, new_order_id: u128) -> Result<Order, Error> {
        if !self.is_flip {
            return Err(Error::NotAFlipOrder);
        }
        if !self.is_fully_filled() {
            return Err(Error::OrderNotFullyFilled);
        }

        // Flip the side: bid becomes ask, ask becomes bid
        Ok(Order {
            order_id: new_order_id,
            maker: self.maker.clone(),
            base_token: self.base_token.clone(),
            quote_token: self.quote_token.clone(),
            is_bid: !self.is_bid,
            tick: self.flip_tick,
            amount: self.amount,
            remaining: self.amount,
            prev: 0,
            next: 0,
            is_flip: false, // Flipped orders are not recursive
            flip_tick: 0,
        })
    }
}

// ============ Order Storage Functions ============

pub fn save_order(env: &Env, order: &Order) {
    let key = DataKey::Order(order.order_id);
    env.storage().persistent().set(&key, order);
    extend_persistent_ttl(env, &key);
}

pub fn get_order(env: &Env, order_id: u128) -> Option<Order> {
    let key = DataKey::Order(order_id);
    let order = env.storage().persistent().get(&key);
    if order.is_some() {
        extend_persistent_ttl(env, &key);
    }
    order
}

pub fn delete_order(env: &Env, order_id: u128) {
    let key = DataKey::Order(order_id);
    env.storage().persistent().remove(&key);
}

pub fn save_pending_order(env: &Env, order: &Order) {
    let key = DataKey::PendingOrder(order.order_id);
    env.storage().persistent().set(&key, order);
    extend_persistent_ttl(env, &key);
}

pub fn get_pending_order(env: &Env, order_id: u128) -> Option<Order> {
    let key = DataKey::PendingOrder(order_id);
    let order = env.storage().persistent().get(&key);
    if order.is_some() {
        extend_persistent_ttl(env, &key);
    }
    order
}

pub fn delete_pending_order(env: &Env, order_id: u128) {
    let key = DataKey::PendingOrder(order_id);
    env.storage().persistent().remove(&key);
}
