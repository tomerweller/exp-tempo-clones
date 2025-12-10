use crate::{
    orderbook::{tick_to_price, PRICE_SCALE, MIN_TICK, MAX_TICK, TICK_SPACING, MIN_ORDER_SIZE},
    Error, StablecoinExchange, StablecoinExchangeClient,
};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_address.address()),
        StellarAssetClient::new(env, &contract_address.address()),
    )
}

fn setup_test_env() -> (
    Env,
    StablecoinExchangeClient<'static>,
    Address,
    Address,
    TokenClient<'static>,
    TokenClient<'static>,
    StellarAssetClient<'static>,
    StellarAssetClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    // Deploy exchange
    let exchange_address = env.register(StablecoinExchange, ());
    let exchange = StablecoinExchangeClient::new(&env, &exchange_address);
    exchange.initialize(&admin);

    // Create tokens
    let (base_token, base_admin) = create_token(&env, &admin);
    let (quote_token, quote_admin) = create_token(&env, &admin);

    let user = Address::generate(&env);

    (
        env,
        exchange,
        admin,
        user,
        base_token,
        quote_token,
        base_admin,
        quote_admin,
    )
}

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let exchange_address = env.register(StablecoinExchange, ());
    let exchange = StablecoinExchangeClient::new(&env, &exchange_address);

    exchange.initialize(&admin);
    assert_eq!(exchange.admin(), admin);
}

#[test]
fn test_create_pair() {
    let (_env, exchange, admin, _user, base_token, quote_token, _, _) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);

    // Verify orderbook exists
    let orderbook = exchange.get_orderbook(&base_token.address, &quote_token.address);
    assert_eq!(orderbook.base_token, base_token.address);
    assert_eq!(orderbook.quote_token, quote_token.address);
}

#[test]
fn test_create_pair_same_token_fails() {
    let (_env, exchange, admin, _user, base_token, _quote_token, _, _) = setup_test_env();

    let result = exchange.try_create_pair(&admin, &base_token.address, &base_token.address);
    assert_eq!(result, Err(Ok(Error::SameToken)));
}

#[test]
fn test_create_pair_duplicate_fails() {
    let (_env, exchange, admin, _user, base_token, quote_token, _, _) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);

    let result = exchange.try_create_pair(&admin, &base_token.address, &quote_token.address);
    assert_eq!(result, Err(Ok(Error::PairAlreadyExists)));
}

#[test]
fn test_place_bid_order() {
    let (_env, exchange, admin, user, base_token, quote_token, _, quote_admin) = setup_test_env();

    // Create pair
    exchange.create_pair(&admin, &base_token.address, &quote_token.address);

    // Mint quote tokens to user
    quote_admin.mint(&user, &1_000_000_000);

    // Place bid order: buy 100 base at tick 0
    let amount = 100_000_000i128; // 100 base (6 decimals)
    let tick = 0i32;
    let order_id = exchange.place(
        &user,
        &base_token.address,
        &quote_token.address,
        &true,
        &tick,
        &amount,
    );

    assert!(order_id > 0);

    // Check pending order
    let pending = exchange.get_pending_order(&order_id);
    assert!(pending.is_some());
    let order = pending.unwrap();
    assert_eq!(order.maker, user);
    assert!(order.is_bid);
    assert_eq!(order.tick, tick);
    assert_eq!(order.amount, amount);
}

#[test]
fn test_place_ask_order() {
    let (_env, exchange, admin, user, base_token, quote_token, base_admin, _) = setup_test_env();

    // Create pair
    exchange.create_pair(&admin, &base_token.address, &quote_token.address);

    // Mint base tokens to user
    base_admin.mint(&user, &1_000_000_000);

    // Place ask order: sell 100 base at tick 100
    let amount = 100_000_000i128;
    let tick = 100i32;
    let order_id = exchange.place(
        &user,
        &base_token.address,
        &quote_token.address,
        &false,
        &tick,
        &amount,
    );

    assert!(order_id > 0);

    let pending = exchange.get_pending_order(&order_id);
    assert!(pending.is_some());
    let order = pending.unwrap();
    assert!(!order.is_bid);
}

#[test]
fn test_order_too_small_fails() {
    let (_env, exchange, admin, user, base_token, quote_token, _, quote_admin) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);
    quote_admin.mint(&user, &1_000_000_000);

    // Try to place order below minimum
    let result = exchange.try_place(
        &user,
        &base_token.address,
        &quote_token.address,
        &true,
        &0,
        &(MIN_ORDER_SIZE - 1),
    );

    assert_eq!(result, Err(Ok(Error::OrderTooSmall)));
}

#[test]
fn test_invalid_tick_fails() {
    let (_env, exchange, admin, user, base_token, quote_token, _, quote_admin) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);
    quote_admin.mint(&user, &1_000_000_000);

    // Try tick outside range
    let result = exchange.try_place(
        &user,
        &base_token.address,
        &quote_token.address,
        &true,
        &(MAX_TICK + 1),
        &MIN_ORDER_SIZE,
    );

    assert_eq!(result, Err(Ok(Error::InvalidTick)));
}

#[test]
fn test_execute_block() {
    let (env, exchange, admin, user, base_token, quote_token, _, quote_admin) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);
    quote_admin.mint(&user, &1_000_000_000);

    let order_id = exchange.place(
        &user,
        &base_token.address,
        &quote_token.address,
        &true,
        &0,
        &MIN_ORDER_SIZE,
    );

    // Execute block to activate order
    exchange.execute_block(
        &base_token.address,
        &quote_token.address,
        &vec![&env, order_id],
    );

    // Pending order should be gone
    assert!(exchange.get_pending_order(&order_id).is_none());

    // Active order should exist (with new ID)
    // Note: active order gets a new ID, so we check orderbook state
    let orderbook = exchange.get_orderbook(&base_token.address, &quote_token.address);
    assert!(orderbook.has_bids());
}

#[test]
fn test_cancel_pending_order() {
    let (_env, exchange, admin, user, base_token, quote_token, _, quote_admin) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);
    quote_admin.mint(&user, &1_000_000_000);

    let order_id = exchange.place(
        &user,
        &base_token.address,
        &quote_token.address,
        &true,
        &0,
        &MIN_ORDER_SIZE,
    );

    // Cancel the order
    let refund = exchange.cancel(&user, &order_id);
    assert_eq!(refund, MIN_ORDER_SIZE);

    // Order should be gone
    assert!(exchange.get_pending_order(&order_id).is_none());
}

#[test]
fn test_place_flip_order() {
    let (_env, exchange, admin, user, base_token, quote_token, _, quote_admin) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);
    quote_admin.mint(&user, &1_000_000_000);

    // Place flip bid: buy at tick 0, flip to sell at tick 100
    let order_id = exchange.place_flip(
        &user,
        &base_token.address,
        &quote_token.address,
        &true,
        &0,
        &MIN_ORDER_SIZE,
        &100, // flip_tick must be > tick for bids
    );

    let pending = exchange.get_pending_order(&order_id);
    assert!(pending.is_some());
    let order = pending.unwrap();
    assert!(order.is_flip);
    assert_eq!(order.flip_tick, 100);
}

#[test]
fn test_invalid_flip_tick_bid() {
    let (_env, exchange, admin, user, base_token, quote_token, _, quote_admin) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);
    quote_admin.mint(&user, &1_000_000_000);

    // Flip tick must be > tick for bids
    let result = exchange.try_place_flip(
        &user,
        &base_token.address,
        &quote_token.address,
        &true,
        &100,
        &MIN_ORDER_SIZE,
        &0, // Invalid: flip_tick <= tick
    );

    assert_eq!(result, Err(Ok(Error::InvalidBidFlipTick)));
}

#[test]
fn test_invalid_flip_tick_ask() {
    let (_env, exchange, admin, user, base_token, quote_token, base_admin, _) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);
    base_admin.mint(&user, &1_000_000_000);

    // Flip tick must be < tick for asks
    let result = exchange.try_place_flip(
        &user,
        &base_token.address,
        &quote_token.address,
        &false,
        &0,
        &MIN_ORDER_SIZE,
        &100, // Invalid: flip_tick >= tick
    );

    assert_eq!(result, Err(Ok(Error::InvalidAskFlipTick)));
}

#[test]
fn test_tick_to_price() {
    // Tick 0 should give base price
    assert_eq!(tick_to_price(0), PRICE_SCALE);

    // Positive ticks increase price
    assert!(tick_to_price(100) > tick_to_price(0));

    // Negative ticks decrease price
    assert!(tick_to_price(-100) < tick_to_price(0));
}

#[test]
fn test_swap_exact_in_buy() {
    let (env, exchange, admin, user, base_token, quote_token, base_admin, quote_admin) =
        setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);

    // Maker provides liquidity (ask order - selling base)
    let maker = Address::generate(&env);
    base_admin.mint(&maker, &1_000_000_000);

    let ask_order_id = exchange.place(
        &maker,
        &base_token.address,
        &quote_token.address,
        &false,   // ask
        &0,       // tick
        &100_000_000, // 100 base
    );

    exchange.execute_block(
        &base_token.address,
        &quote_token.address,
        &vec![&env, ask_order_id],
    );

    // Taker buys base with quote
    quote_admin.mint(&user, &1_000_000_000);

    let quote_in = 50_000_000i128; // 50 quote
    let base_out = exchange.swap_exact_in(
        &user,
        &base_token.address,
        &quote_token.address,
        &true, // is_buy
        &quote_in,
        &0, // min_amount_out
    );

    // Should receive base tokens
    assert!(base_out > 0);
}

#[test]
fn test_swap_exact_in_sell() {
    let (env, exchange, admin, user, base_token, quote_token, base_admin, quote_admin) =
        setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);

    // Maker provides liquidity (bid order - buying base)
    let maker = Address::generate(&env);
    quote_admin.mint(&maker, &1_000_000_000);

    let bid_order_id = exchange.place(
        &maker,
        &base_token.address,
        &quote_token.address,
        &true,    // bid
        &0,       // tick
        &100_000_000, // 100 base worth
    );

    exchange.execute_block(
        &base_token.address,
        &quote_token.address,
        &vec![&env, bid_order_id],
    );

    // Taker sells base for quote
    base_admin.mint(&user, &1_000_000_000);

    let base_in = 50_000_000i128;
    let quote_out = exchange.swap_exact_in(
        &user,
        &base_token.address,
        &quote_token.address,
        &false, // is_buy = false means selling base
        &base_in,
        &0,
    );

    assert!(quote_out > 0);
}

#[test]
fn test_quote_swap() {
    let (env, exchange, admin, _user, base_token, quote_token, base_admin, _) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);

    // Add some liquidity
    let maker = Address::generate(&env);
    base_admin.mint(&maker, &1_000_000_000);

    let ask_order_id = exchange.place(
        &maker,
        &base_token.address,
        &quote_token.address,
        &false,
        &0,
        &100_000_000,
    );

    exchange.execute_block(
        &base_token.address,
        &quote_token.address,
        &vec![&env, ask_order_id],
    );

    // Quote the swap
    let quote_in = 50_000_000i128;
    let expected_out = exchange.quote_swap_in(
        &base_token.address,
        &quote_token.address,
        &true,
        &quote_in,
    );

    assert!(expected_out > 0);
}

#[test]
fn test_withdraw() {
    let (_env, exchange, admin, user, base_token, quote_token, base_admin, _) = setup_test_env();

    exchange.create_pair(&admin, &base_token.address, &quote_token.address);

    // Give user some balance (simulating filled order credit)
    // We'll do this by placing and canceling an order
    base_admin.mint(&user, &1_000_000_000);

    let order_id = exchange.place(
        &user,
        &base_token.address,
        &quote_token.address,
        &false,
        &0,
        &MIN_ORDER_SIZE,
    );

    // Cancel to get balance credit
    exchange.cancel(&user, &order_id);

    // Check balance
    let balance = exchange.balance_of(&user, &user); // Note: balance key uses maker address
    assert_eq!(balance, MIN_ORDER_SIZE);
}

#[test]
fn test_constants() {
    assert_eq!(StablecoinExchange::min_tick(), MIN_TICK);
    assert_eq!(StablecoinExchange::max_tick(), MAX_TICK);
    assert_eq!(StablecoinExchange::tick_spacing(), TICK_SPACING);
    assert_eq!(StablecoinExchange::price_scale(), PRICE_SCALE);
    assert_eq!(StablecoinExchange::min_order_size(), MIN_ORDER_SIZE);
}
