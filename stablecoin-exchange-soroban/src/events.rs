use soroban_sdk::{symbol_short, Address, Env, Symbol};

// Event topics
const ORDER_PLACED: Symbol = symbol_short!("placed");
const ORDER_CANCELED: Symbol = symbol_short!("canceled");
const ORDER_FILLED: Symbol = symbol_short!("filled");
const TRADE: Symbol = symbol_short!("trade");
const WITHDRAW: Symbol = symbol_short!("withdraw");
const PAIR_CREATED: Symbol = symbol_short!("pair");

pub fn emit_order_placed(
    env: &Env,
    order_id: u128,
    maker: &Address,
    base_token: &Address,
    quote_token: &Address,
    is_bid: bool,
    tick: i32,
    amount: i128,
    is_flip: bool,
) {
    env.events().publish(
        (ORDER_PLACED, maker, base_token, quote_token),
        (order_id, is_bid, tick, amount, is_flip),
    );
}

pub fn emit_order_canceled(env: &Env, order_id: u128, maker: &Address, refund_amount: i128) {
    env.events()
        .publish((ORDER_CANCELED, maker), (order_id, refund_amount));
}

pub fn emit_order_filled(
    env: &Env,
    order_id: u128,
    maker: &Address,
    filled_amount: i128,
    remaining: i128,
) {
    env.events().publish(
        (ORDER_FILLED, maker),
        (order_id, filled_amount, remaining),
    );
}

pub fn emit_trade(
    env: &Env,
    base_token: &Address,
    quote_token: &Address,
    taker: &Address,
    is_buy: bool,
    base_amount: i128,
    quote_amount: i128,
    tick: i32,
) {
    env.events().publish(
        (TRADE, base_token, quote_token, taker),
        (is_buy, base_amount, quote_amount, tick),
    );
}

pub fn emit_withdraw(env: &Env, user: &Address, token: &Address, amount: i128) {
    env.events()
        .publish((WITHDRAW, user, token), amount);
}

pub fn emit_pair_created(env: &Env, base_token: &Address, quote_token: &Address) {
    env.events()
        .publish((PAIR_CREATED,), (base_token, quote_token));
}
