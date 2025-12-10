use soroban_sdk::{symbol_short, Address, Env, Symbol};

// Event topics
const MINT: Symbol = symbol_short!("mint");
const BURN: Symbol = symbol_short!("burn");
const FEE_SWAP: Symbol = symbol_short!("fee_swap");
const REBALANCE: Symbol = symbol_short!("rebalance");

pub fn emit_mint(
    env: &Env,
    sender: &Address,
    user_token: &Address,
    validator_token: &Address,
    amount_user_token: i128,
    amount_validator_token: i128,
    liquidity: i128,
) {
    env.events().publish(
        (MINT, sender, user_token, validator_token),
        (amount_user_token, amount_validator_token, liquidity),
    );
}

pub fn emit_burn(
    env: &Env,
    sender: &Address,
    user_token: &Address,
    validator_token: &Address,
    amount_user_token: i128,
    amount_validator_token: i128,
    liquidity: i128,
    to: &Address,
) {
    env.events().publish(
        (BURN, sender, user_token, validator_token),
        (amount_user_token, amount_validator_token, liquidity, to),
    );
}

pub fn emit_fee_swap(
    env: &Env,
    user_token: &Address,
    validator_token: &Address,
    amount_in: i128,
    amount_out: i128,
) {
    env.events().publish(
        (FEE_SWAP, user_token, validator_token),
        (amount_in, amount_out),
    );
}

pub fn emit_rebalance_swap(
    env: &Env,
    user_token: &Address,
    validator_token: &Address,
    swapper: &Address,
    amount_in: i128,
    amount_out: i128,
) {
    env.events().publish(
        (REBALANCE, user_token, validator_token, swapper),
        (amount_in, amount_out),
    );
}
