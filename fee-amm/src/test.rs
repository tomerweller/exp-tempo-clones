use crate::{Error, TempoFeeAMM, TempoFeeAMMClient};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

fn create_token_contract<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_address.address()),
        StellarAssetClient::new(env, &contract_address.address()),
    )
}

fn setup_test_env() -> (
    Env,
    TempoFeeAMMClient<'static>,
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

    // Deploy AMM contract
    let amm_address = env.register(TempoFeeAMM, ());
    let amm_client = TempoFeeAMMClient::new(&env, &amm_address);

    // Initialize AMM
    amm_client.initialize(&admin);

    // Create token contracts
    let (user_token, user_token_admin) = create_token_contract(&env, &admin);
    let (validator_token, validator_token_admin) = create_token_contract(&env, &admin);

    // Generate user address before moving env
    let user = Address::generate(&env);

    (
        env,
        amm_client,
        admin,
        user,
        user_token,
        validator_token,
        user_token_admin,
        validator_token_admin,
    )
}

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let amm_address = env.register(TempoFeeAMM, ());
    let amm_client = TempoFeeAMMClient::new(&env, &amm_address);

    amm_client.initialize(&admin);

    assert_eq!(amm_client.admin(), admin);
}

#[test]
fn test_mint_identical_addresses() {
    let (env, amm_client, _, user, user_token, _, user_token_admin, _) = setup_test_env();

    // Mint tokens to user
    user_token_admin.mint(&user, &10000);

    // Try to mint with identical addresses
    let result = amm_client.try_mint(
        &user,
        &user_token.address,
        &user_token.address, // Same token
        &1000,
        &1000,
        &user,
    );

    assert_eq!(result, Err(Ok(Error::IdenticalAddresses)));
}

#[test]
fn test_mint_first_deposit() {
    let (env, amm_client, _, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Mint tokens to user
    user_token_admin.mint(&user, &100_000);
    validator_token_admin.mint(&user, &100_000);

    // First deposit
    let liquidity = amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &10_000,
        &10_000,
        &user,
    );

    // Expected: (10000 + 10000) / 2 - 1000 = 9000
    assert_eq!(liquidity, 9000);

    // Check pool state
    let pool = amm_client.get_pool(&user_token.address, &validator_token.address);
    assert_eq!(pool.reserve_user_token, 10_000);
    assert_eq!(pool.reserve_validator_token, 10_000);

    // Check LP balance
    let lp_balance = amm_client.get_lp_balance(&user_token.address, &validator_token.address, &user);
    assert_eq!(lp_balance, 9000);

    // Check total supply (9000 + 1000 MIN_LIQUIDITY)
    let total_supply = amm_client.get_total_supply(&user_token.address, &validator_token.address);
    assert_eq!(total_supply, 10000);
}

#[test]
fn test_mint_subsequent_deposit() {
    let (env, amm_client, _, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    let user2 = Address::generate(&env);

    // Mint tokens to users
    user_token_admin.mint(&user, &100_000);
    validator_token_admin.mint(&user, &100_000);
    user_token_admin.mint(&user2, &100_000);
    validator_token_admin.mint(&user2, &100_000);

    // First deposit
    amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &10_000,
        &10_000,
        &user,
    );

    // Second deposit (proportional)
    let liquidity2 = amm_client.mint(
        &user2,
        &user_token.address,
        &validator_token.address,
        &5_000,
        &5_000,
        &user2,
    );

    // Expected: 5000 * 10000 / 10000 = 5000
    assert_eq!(liquidity2, 5000);

    // Check pool state
    let pool = amm_client.get_pool(&user_token.address, &validator_token.address);
    assert_eq!(pool.reserve_user_token, 15_000);
    assert_eq!(pool.reserve_validator_token, 15_000);
}

#[test]
fn test_mint_with_validator_token_first_deposit() {
    let (env, amm_client, _, user, user_token, validator_token, _, validator_token_admin) =
        setup_test_env();

    // Mint validator tokens to user
    validator_token_admin.mint(&user, &100_000);

    // First single-sided deposit
    let liquidity = amm_client.mint_with_validator_token(
        &user,
        &user_token.address,
        &validator_token.address,
        &10_000,
        &user,
    );

    // Expected: (10000 / 2) - 1000 = 4000
    assert_eq!(liquidity, 4000);

    // Check pool state
    let pool = amm_client.get_pool(&user_token.address, &validator_token.address);
    assert_eq!(pool.reserve_user_token, 0);
    assert_eq!(pool.reserve_validator_token, 10_000);
}

#[test]
fn test_burn() {
    let (env, amm_client, _, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Mint tokens and add liquidity
    user_token_admin.mint(&user, &100_000);
    validator_token_admin.mint(&user, &100_000);

    let liquidity = amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &10_000,
        &10_000,
        &user,
    );

    // Burn half the liquidity
    let burn_amount = liquidity / 2;
    let (amount_user, amount_validator) = amm_client.burn(
        &user,
        &user_token.address,
        &validator_token.address,
        &burn_amount,
        &user,
    );

    // Should receive proportional amounts
    // burn_amount * 10000 / 10000 = 4500
    assert_eq!(amount_user, 4500);
    assert_eq!(amount_validator, 4500);

    // Check remaining LP balance
    let remaining_balance = amm_client.get_lp_balance(&user_token.address, &validator_token.address, &user);
    assert_eq!(remaining_balance, liquidity - burn_amount);
}

#[test]
fn test_burn_identical_addresses() {
    let (env, amm_client, _, user, user_token, _, _, _) = setup_test_env();

    let result = amm_client.try_burn(
        &user,
        &user_token.address,
        &user_token.address,
        &1000,
        &user,
    );

    assert_eq!(result, Err(Ok(Error::IdenticalAddresses)));
}

#[test]
fn test_burn_insufficient_balance() {
    let (env, amm_client, _, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Mint tokens and add liquidity
    user_token_admin.mint(&user, &100_000);
    validator_token_admin.mint(&user, &100_000);

    let liquidity = amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &10_000,
        &10_000,
        &user,
    );

    // Try to burn more than balance
    let result = amm_client.try_burn(
        &user,
        &user_token.address,
        &validator_token.address,
        &(liquidity + 1),
        &user,
    );

    assert_eq!(result, Err(Ok(Error::InsufficientLiquidity)));
}

#[test]
fn test_reserve_and_execute_fee_swap() {
    let (env, amm_client, admin, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Setup pool with liquidity
    user_token_admin.mint(&user, &1_000_000);
    validator_token_admin.mint(&user, &1_000_000);

    amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &100_000,
        &100_000,
        &user,
    );

    // Reserve liquidity for fee swap
    let swap_amount = 10_000i128;
    amm_client.reserve_liquidity(&user_token.address, &validator_token.address, &swap_amount);

    // Check pending
    let pending = amm_client.get_pending_fee_swap(&user_token.address, &validator_token.address);
    assert_eq!(pending, swap_amount);

    // Execute pending swaps
    let amount_out = amm_client.execute_pending_fee_swaps(&user_token.address, &validator_token.address);

    // Expected: 10000 * 9970 / 10000 = 9970
    assert_eq!(amount_out, 9970);

    // Check pending cleared
    let pending_after = amm_client.get_pending_fee_swap(&user_token.address, &validator_token.address);
    assert_eq!(pending_after, 0);

    // Check reserves updated
    let pool = amm_client.get_pool(&user_token.address, &validator_token.address);
    assert_eq!(pool.reserve_user_token, 100_000 + swap_amount);
    assert_eq!(pool.reserve_validator_token, 100_000 - amount_out);
}

#[test]
fn test_reserve_liquidity_insufficient() {
    let (env, amm_client, admin, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Setup pool with small liquidity (need > 1000 for MIN_LIQUIDITY)
    user_token_admin.mint(&user, &10_000);
    validator_token_admin.mint(&user, &10_000);

    // Mint with 5000 each: mean = 5000, liquidity = 5000 - 1000 = 4000
    amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &5_000,
        &5_000,
        &user,
    );

    // Pool has 5000 validator tokens
    // Try to reserve more than available (5001 * 0.997 = 4985 out needed > 5000)
    let result = amm_client.try_reserve_liquidity(
        &user_token.address,
        &validator_token.address,
        &6_000,
    );

    assert_eq!(result, Err(Ok(Error::InsufficientLiquidity)));
}

#[test]
fn test_release_liquidity() {
    let (env, amm_client, admin, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Setup pool
    user_token_admin.mint(&user, &1_000_000);
    validator_token_admin.mint(&user, &1_000_000);

    amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &100_000,
        &100_000,
        &user,
    );

    // Reserve then release
    amm_client.reserve_liquidity(&user_token.address, &validator_token.address, &10_000);
    amm_client.release_liquidity(&user_token.address, &validator_token.address, &5_000);

    let pending = amm_client.get_pending_fee_swap(&user_token.address, &validator_token.address);
    assert_eq!(pending, 5_000);
}

#[test]
fn test_rebalance_swap() {
    let (env, amm_client, admin, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Setup pool
    user_token_admin.mint(&user, &1_000_000);
    validator_token_admin.mint(&user, &1_000_000);

    amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &100_000,
        &100_000,
        &user,
    );

    let pool_before = amm_client.get_pool(&user_token.address, &validator_token.address);

    // Rebalance swap: user wants to get user tokens by providing validator tokens
    let amount_out = 10_000i128;
    let amount_in = amm_client.rebalance_swap(
        &user,
        &user_token.address,
        &validator_token.address,
        &amount_out,
        &user,
    );

    // Expected: 10000 * 9985 / 10000 + 1 = 9986
    assert_eq!(amount_in, 9986);

    // Check reserves
    let pool_after = amm_client.get_pool(&user_token.address, &validator_token.address);
    assert_eq!(pool_after.reserve_user_token, pool_before.reserve_user_token - amount_out);
    assert_eq!(pool_after.reserve_validator_token, pool_before.reserve_validator_token + amount_in);
}

#[test]
fn test_rebalance_swap_insufficient_reserves() {
    let (env, amm_client, admin, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Setup pool with small reserves
    user_token_admin.mint(&user, &10_000);
    validator_token_admin.mint(&user, &10_000);

    amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &5_000,
        &5_000,
        &user,
    );

    // Try to swap more than available
    let result = amm_client.try_rebalance_swap(
        &user,
        &user_token.address,
        &validator_token.address,
        &10_000,
        &user,
    );

    assert_eq!(result, Err(Ok(Error::InsufficientReserves)));
}

#[test]
fn test_calculate_fee_swap_output() {
    // Test the pure calculation function
    let amount_in = 10_000i128;
    let result = TempoFeeAMM::calculate_fee_swap_output(amount_in);

    // Expected: 10000 * 9970 / 10000 = 9970
    assert_eq!(result, Ok(9970));
}

#[test]
fn test_calculate_rebalance_input() {
    // Test the pure calculation function
    let amount_out = 10_000i128;
    let result = TempoFeeAMM::calculate_rebalance_input(amount_out);

    // Expected: 10000 * 9985 / 10000 + 1 = 9986
    assert_eq!(result, Ok(9986));
}

#[test]
fn test_multiple_fee_swaps() {
    let (env, amm_client, admin, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Setup pool
    user_token_admin.mint(&user, &1_000_000);
    validator_token_admin.mint(&user, &1_000_000);

    amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &100_000,
        &100_000,
        &user,
    );

    // Multiple reservations
    amm_client.reserve_liquidity(&user_token.address, &validator_token.address, &1_000);
    amm_client.reserve_liquidity(&user_token.address, &validator_token.address, &2_000);
    amm_client.reserve_liquidity(&user_token.address, &validator_token.address, &3_000);

    // Check total pending
    let pending = amm_client.get_pending_fee_swap(&user_token.address, &validator_token.address);
    assert_eq!(pending, 6_000);

    // Execute all at once
    let total_out = amm_client.execute_pending_fee_swaps(&user_token.address, &validator_token.address);

    // Expected: 6000 * 9970 / 10000 = 5982
    assert_eq!(total_out, 5982);

    // Verify pending cleared
    let pending_after = amm_client.get_pending_fee_swap(&user_token.address, &validator_token.address);
    assert_eq!(pending_after, 0);
}

#[test]
fn test_burn_blocked_by_pending_swaps() {
    let (env, amm_client, admin, user, user_token, validator_token, user_token_admin, validator_token_admin) =
        setup_test_env();

    // Setup pool
    user_token_admin.mint(&user, &1_000_000);
    validator_token_admin.mint(&user, &1_000_000);

    let liquidity = amm_client.mint(
        &user,
        &user_token.address,
        &validator_token.address,
        &10_000,
        &10_000,
        &user,
    );

    // Reserve most of the validator tokens
    // Pool has 10000 validator tokens, reserve 9500 worth of swaps
    // 9500 * 0.997 = 9471.5 out needed
    amm_client.reserve_liquidity(&user_token.address, &validator_token.address, &9_500);

    // Try to burn all liquidity - should fail because validator tokens are reserved
    let result = amm_client.try_burn(
        &user,
        &user_token.address,
        &validator_token.address,
        &liquidity,
        &user,
    );

    assert_eq!(result, Err(Ok(Error::InsufficientReserves)));
}
