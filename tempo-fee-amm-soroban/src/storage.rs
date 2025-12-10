use soroban_sdk::{contracttype, Address, Env};

/// Storage keys for the contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Admin address
    Admin,
    /// Pool data for a token pair (user_token, validator_token)
    Pool(Address, Address),
    /// Total LP token supply for a pool
    TotalSupply(Address, Address),
    /// LP token balance for a user in a specific pool
    LPBalance(Address, Address, Address),
    /// Pending fee swap amount for a pool
    PendingFeeSwap(Address, Address),
}

/// Pool structure storing reserve balances
#[contracttype]
#[derive(Clone, Debug, Default)]
pub struct Pool {
    pub reserve_user_token: i128,
    pub reserve_validator_token: i128,
}

// Storage helper functions

const DAY_IN_LEDGERS: u32 = 17280; // ~24 hours at 5 seconds per ledger
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

// Admin storage
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

// Pool storage
pub fn set_pool(env: &Env, user_token: &Address, validator_token: &Address, pool: &Pool) {
    let key = DataKey::Pool(user_token.clone(), validator_token.clone());
    env.storage().persistent().set(&key, pool);
    extend_persistent_ttl(env, &key);
}

pub fn get_pool(env: &Env, user_token: &Address, validator_token: &Address) -> Pool {
    let key = DataKey::Pool(user_token.clone(), validator_token.clone());
    let pool = env.storage().persistent().get(&key).unwrap_or_default();
    if env.storage().persistent().has(&key) {
        extend_persistent_ttl(env, &key);
    }
    pool
}

pub fn has_pool(env: &Env, user_token: &Address, validator_token: &Address) -> bool {
    let key = DataKey::Pool(user_token.clone(), validator_token.clone());
    env.storage().persistent().has(&key)
}

// Total supply storage
pub fn set_total_supply(env: &Env, user_token: &Address, validator_token: &Address, supply: i128) {
    let key = DataKey::TotalSupply(user_token.clone(), validator_token.clone());
    env.storage().persistent().set(&key, &supply);
    extend_persistent_ttl(env, &key);
}

pub fn get_total_supply(env: &Env, user_token: &Address, validator_token: &Address) -> i128 {
    let key = DataKey::TotalSupply(user_token.clone(), validator_token.clone());
    let supply = env.storage().persistent().get(&key).unwrap_or(0);
    if env.storage().persistent().has(&key) {
        extend_persistent_ttl(env, &key);
    }
    supply
}

// LP balance storage
pub fn set_lp_balance(
    env: &Env,
    user_token: &Address,
    validator_token: &Address,
    user: &Address,
    balance: i128,
) {
    let key = DataKey::LPBalance(user_token.clone(), validator_token.clone(), user.clone());
    env.storage().persistent().set(&key, &balance);
    extend_persistent_ttl(env, &key);
}

pub fn get_lp_balance(
    env: &Env,
    user_token: &Address,
    validator_token: &Address,
    user: &Address,
) -> i128 {
    let key = DataKey::LPBalance(user_token.clone(), validator_token.clone(), user.clone());
    let balance = env.storage().persistent().get(&key).unwrap_or(0);
    if env.storage().persistent().has(&key) {
        extend_persistent_ttl(env, &key);
    }
    balance
}

// Pending fee swap storage
pub fn set_pending_fee_swap(
    env: &Env,
    user_token: &Address,
    validator_token: &Address,
    amount: i128,
) {
    let key = DataKey::PendingFeeSwap(user_token.clone(), validator_token.clone());
    env.storage().persistent().set(&key, &amount);
    extend_persistent_ttl(env, &key);
}

pub fn get_pending_fee_swap(env: &Env, user_token: &Address, validator_token: &Address) -> i128 {
    let key = DataKey::PendingFeeSwap(user_token.clone(), validator_token.clone());
    let amount = env.storage().persistent().get(&key).unwrap_or(0);
    if env.storage().persistent().has(&key) {
        extend_persistent_ttl(env, &key);
    }
    amount
}

pub fn clear_pending_fee_swap(env: &Env, user_token: &Address, validator_token: &Address) {
    let key = DataKey::PendingFeeSwap(user_token.clone(), validator_token.clone());
    env.storage().persistent().set(&key, &0i128);
}
