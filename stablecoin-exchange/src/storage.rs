use soroban_sdk::{contracttype, Address, Env};

/// Storage keys for the contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Admin address
    Admin,
    /// Next active order ID counter
    ActiveOrderId,
    /// Next pending order ID counter
    PendingOrderId,
    /// Active order by ID
    Order(u128),
    /// Pending order by ID
    PendingOrder(u128),
    /// Orderbook for a trading pair (base_token, quote_token)
    Orderbook(Address, Address),
    /// Bid tick level (base_token, quote_token, tick)
    BidTickLevel(Address, Address, i32),
    /// Ask tick level (base_token, quote_token, tick)
    AskTickLevel(Address, Address, i32),
    /// User balance (user, token)
    Balance(Address, Address),
}

// TTL constants
const DAY_IN_LEDGERS: u32 = 17280;
const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;
const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

pub fn extend_persistent_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

// ============ Admin Storage ============

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

// ============ Order ID Counters ============

pub fn get_next_active_order_id(env: &Env) -> u128 {
    let key = DataKey::ActiveOrderId;
    let id: u128 = env.storage().instance().get(&key).unwrap_or(1);
    env.storage().instance().set(&key, &(id + 1));
    id
}

pub fn get_next_pending_order_id(env: &Env) -> u128 {
    let key = DataKey::PendingOrderId;
    let id: u128 = env.storage().instance().get(&key).unwrap_or(1);
    env.storage().instance().set(&key, &(id + 1));
    id
}

pub fn get_current_active_order_id(env: &Env) -> u128 {
    env.storage()
        .instance()
        .get(&DataKey::ActiveOrderId)
        .unwrap_or(1)
}

pub fn get_current_pending_order_id(env: &Env) -> u128 {
    env.storage()
        .instance()
        .get(&DataKey::PendingOrderId)
        .unwrap_or(1)
}

// ============ Balance Storage ============

pub fn get_balance(env: &Env, user: &Address, token: &Address) -> i128 {
    let key = DataKey::Balance(user.clone(), token.clone());
    let balance = env.storage().persistent().get(&key).unwrap_or(0);
    if env.storage().persistent().has(&key) {
        extend_persistent_ttl(env, &key);
    }
    balance
}

pub fn set_balance(env: &Env, user: &Address, token: &Address, amount: i128) {
    let key = DataKey::Balance(user.clone(), token.clone());
    env.storage().persistent().set(&key, &amount);
    extend_persistent_ttl(env, &key);
}

pub fn add_balance(env: &Env, user: &Address, token: &Address, amount: i128) {
    let current = get_balance(env, user, token);
    set_balance(env, user, token, current + amount);
}

pub fn sub_balance(env: &Env, user: &Address, token: &Address, amount: i128) -> bool {
    let current = get_balance(env, user, token);
    if current < amount {
        return false;
    }
    set_balance(env, user, token, current - amount);
    true
}
