#![no_std]

mod error;
mod events;
mod storage;

use error::Error;
use soroban_sdk::{contract, contractimpl, token, Address, Env};
use storage::Pool;

/// Fee multiplier: m = 0.9970 (scaled by 10000)
/// Used in fee swaps: amount_out = amount_in * M / SCALE
const M: i128 = 9970;

/// Rebalance multiplier: n = 0.9985 (scaled by 10000)
/// Used in rebalance swaps
const N: i128 = 9985;

/// Scaling factor for fixed-point arithmetic
const SCALE: i128 = 10000;

/// Minimum liquidity locked forever to prevent division by zero
const MIN_LIQUIDITY: i128 = 1000;

/// Compute amount out for a fee swap
/// Returns: amount_in * M / SCALE
#[inline]
fn compute_amount_out(amount_in: i128) -> Result<i128, Error> {
    amount_in
        .checked_mul(M)
        .and_then(|product| product.checked_div(SCALE))
        .ok_or(Error::Overflow)
}

/// Integer square root using Newton's method
fn sqrt(x: i128) -> i128 {
    if x == 0 {
        return 0;
    }
    let mut z = (x + 1) / 2;
    let mut y = x;
    while z < y {
        y = z;
        z = (x / z + z) / 2;
    }
    y
}

#[contract]
pub struct TempoFeeAMM;

#[contractimpl]
impl TempoFeeAMM {
    /// Initialize the contract with an admin
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if storage::has_admin(&env) {
            return Err(Error::Unauthorized);
        }
        storage::set_admin(&env, &admin);
        storage::extend_instance_ttl(&env);
        Ok(())
    }

    /// Get the admin address
    pub fn admin(env: Env) -> Address {
        storage::extend_instance_ttl(&env);
        storage::get_admin(&env)
    }

    /// Get pool reserves for a token pair
    pub fn get_pool(env: Env, user_token: Address, validator_token: Address) -> Pool {
        storage::extend_instance_ttl(&env);
        storage::get_pool(&env, &user_token, &validator_token)
    }

    /// Get total LP token supply for a pool
    pub fn get_total_supply(env: Env, user_token: Address, validator_token: Address) -> i128 {
        storage::extend_instance_ttl(&env);
        storage::get_total_supply(&env, &user_token, &validator_token)
    }

    /// Get LP token balance for a user
    pub fn get_lp_balance(
        env: Env,
        user_token: Address,
        validator_token: Address,
        user: Address,
    ) -> i128 {
        storage::extend_instance_ttl(&env);
        storage::get_lp_balance(&env, &user_token, &validator_token, &user)
    }

    /// Get pending fee swap amount for a pool
    pub fn get_pending_fee_swap(env: Env, user_token: Address, validator_token: Address) -> i128 {
        storage::extend_instance_ttl(&env);
        storage::get_pending_fee_swap(&env, &user_token, &validator_token)
    }

    /// Mint LP tokens by providing both user and validator tokens
    pub fn mint(
        env: Env,
        sender: Address,
        user_token: Address,
        validator_token: Address,
        amount_user_token: i128,
        amount_validator_token: i128,
        to: Address,
    ) -> Result<i128, Error> {
        // Verify sender authorization
        sender.require_auth();

        // Tokens must be different
        if user_token == validator_token {
            return Err(Error::IdenticalAddresses);
        }

        // Amounts must be positive
        if amount_user_token <= 0 || amount_validator_token <= 0 {
            return Err(Error::InvalidAmount);
        }

        storage::extend_instance_ttl(&env);

        let mut pool = storage::get_pool(&env, &user_token, &validator_token);
        let total_supply = storage::get_total_supply(&env, &user_token, &validator_token);

        let liquidity = if total_supply == 0 {
            // First deposit: liquidity = mean(amount_user, amount_validator) - MIN_LIQUIDITY
            // Using addition mean: (a + b) / 2
            let mean = amount_user_token
                .checked_add(amount_validator_token)
                .and_then(|sum| sum.checked_div(2))
                .ok_or(Error::Overflow)?;

            if mean <= MIN_LIQUIDITY {
                return Err(Error::InsufficientLiquidity);
            }

            // Lock MIN_LIQUIDITY forever
            storage::set_total_supply(&env, &user_token, &validator_token, MIN_LIQUIDITY);

            mean.checked_sub(MIN_LIQUIDITY)
                .ok_or(Error::InsufficientLiquidity)?
        } else {
            // Subsequent deposits: calculate proportional liquidity
            let liquidity_user = if pool.reserve_user_token > 0 {
                amount_user_token
                    .checked_mul(total_supply)
                    .and_then(|num| num.checked_div(pool.reserve_user_token))
                    .ok_or(Error::Overflow)?
            } else {
                i128::MAX
            };

            let liquidity_validator = if pool.reserve_validator_token > 0 {
                amount_validator_token
                    .checked_mul(total_supply)
                    .and_then(|num| num.checked_div(pool.reserve_validator_token))
                    .ok_or(Error::Overflow)?
            } else {
                i128::MAX
            };

            liquidity_user.min(liquidity_validator)
        };

        if liquidity <= 0 {
            return Err(Error::InsufficientLiquidity);
        }

        // Transfer tokens from sender to contract
        let user_token_client = token::Client::new(&env, &user_token);
        let validator_token_client = token::Client::new(&env, &validator_token);

        user_token_client.transfer(&sender, &env.current_contract_address(), &amount_user_token);
        validator_token_client.transfer(
            &sender,
            &env.current_contract_address(),
            &amount_validator_token,
        );

        // Update reserves
        pool.reserve_user_token = pool
            .reserve_user_token
            .checked_add(amount_user_token)
            .ok_or(Error::Overflow)?;
        pool.reserve_validator_token = pool
            .reserve_validator_token
            .checked_add(amount_validator_token)
            .ok_or(Error::Overflow)?;

        storage::set_pool(&env, &user_token, &validator_token, &pool);

        // Mint LP tokens
        let current_supply = storage::get_total_supply(&env, &user_token, &validator_token);
        storage::set_total_supply(
            &env,
            &user_token,
            &validator_token,
            current_supply.checked_add(liquidity).ok_or(Error::Overflow)?,
        );

        let current_balance = storage::get_lp_balance(&env, &user_token, &validator_token, &to);
        storage::set_lp_balance(
            &env,
            &user_token,
            &validator_token,
            &to,
            current_balance
                .checked_add(liquidity)
                .ok_or(Error::Overflow)?,
        );

        // Emit event
        events::emit_mint(
            &env,
            &sender,
            &user_token,
            &validator_token,
            amount_user_token,
            amount_validator_token,
            liquidity,
        );

        Ok(liquidity)
    }

    /// Mint LP tokens using only validator tokens (single-sided deposit)
    pub fn mint_with_validator_token(
        env: Env,
        sender: Address,
        user_token: Address,
        validator_token: Address,
        amount_validator_token: i128,
        to: Address,
    ) -> Result<i128, Error> {
        sender.require_auth();

        if user_token == validator_token {
            return Err(Error::IdenticalAddresses);
        }

        if amount_validator_token <= 0 {
            return Err(Error::InvalidAmount);
        }

        storage::extend_instance_ttl(&env);

        let mut pool = storage::get_pool(&env, &user_token, &validator_token);
        let mut total_supply = storage::get_total_supply(&env, &user_token, &validator_token);

        let liquidity =
            if pool.reserve_user_token == 0 && pool.reserve_validator_token == 0 {
                // First deposit: liquidity = (amount / 2) - MIN_LIQUIDITY
                let half_amount = amount_validator_token
                    .checked_div(2)
                    .ok_or(Error::Overflow)?;

                if half_amount <= MIN_LIQUIDITY {
                    return Err(Error::InsufficientLiquidity);
                }

                total_supply = total_supply
                    .checked_add(MIN_LIQUIDITY)
                    .ok_or(Error::Overflow)?;
                storage::set_total_supply(&env, &user_token, &validator_token, total_supply);

                half_amount
                    .checked_sub(MIN_LIQUIDITY)
                    .ok_or(Error::InsufficientLiquidity)?
            } else {
                // Subsequent deposits: liquidity = amount * totalSupply / (V + n * U / SCALE)
                let n_times_u = N
                    .checked_mul(pool.reserve_user_token)
                    .and_then(|prod| prod.checked_div(SCALE))
                    .ok_or(Error::InvalidSwapCalculation)?;

                let denom = pool
                    .reserve_validator_token
                    .checked_add(n_times_u)
                    .ok_or(Error::Overflow)?;

                if denom == 0 {
                    return Err(Error::DivisionByZero);
                }

                amount_validator_token
                    .checked_mul(total_supply)
                    .and_then(|num| num.checked_div(denom))
                    .ok_or(Error::InvalidSwapCalculation)?
            };

        if liquidity <= 0 {
            return Err(Error::InsufficientLiquidity);
        }

        // Transfer validator tokens from sender
        let validator_token_client = token::Client::new(&env, &validator_token);
        validator_token_client.transfer(
            &sender,
            &env.current_contract_address(),
            &amount_validator_token,
        );

        // Update reserves (only validator token increases)
        pool.reserve_validator_token = pool
            .reserve_validator_token
            .checked_add(amount_validator_token)
            .ok_or(Error::Overflow)?;

        storage::set_pool(&env, &user_token, &validator_token, &pool);

        // Mint LP tokens
        storage::set_total_supply(
            &env,
            &user_token,
            &validator_token,
            total_supply.checked_add(liquidity).ok_or(Error::Overflow)?,
        );

        let current_balance = storage::get_lp_balance(&env, &user_token, &validator_token, &to);
        storage::set_lp_balance(
            &env,
            &user_token,
            &validator_token,
            &to,
            current_balance
                .checked_add(liquidity)
                .ok_or(Error::Overflow)?,
        );

        // Emit event
        events::emit_mint(
            &env,
            &sender,
            &user_token,
            &validator_token,
            0,
            amount_validator_token,
            liquidity,
        );

        Ok(liquidity)
    }

    /// Burn LP tokens and withdraw both tokens proportionally
    pub fn burn(
        env: Env,
        sender: Address,
        user_token: Address,
        validator_token: Address,
        liquidity: i128,
        to: Address,
    ) -> Result<(i128, i128), Error> {
        sender.require_auth();

        if user_token == validator_token {
            return Err(Error::IdenticalAddresses);
        }

        if liquidity <= 0 {
            return Err(Error::InvalidAmount);
        }

        storage::extend_instance_ttl(&env);

        // Check sender has sufficient LP balance
        let balance = storage::get_lp_balance(&env, &user_token, &validator_token, &sender);
        if balance < liquidity {
            return Err(Error::InsufficientLiquidity);
        }

        let mut pool = storage::get_pool(&env, &user_token, &validator_token);
        let total_supply = storage::get_total_supply(&env, &user_token, &validator_token);

        if total_supply == 0 {
            return Err(Error::PoolNotInitialized);
        }

        // Calculate amounts to return
        let amount_user_token = liquidity
            .checked_mul(pool.reserve_user_token)
            .and_then(|prod| prod.checked_div(total_supply))
            .ok_or(Error::Overflow)?;

        let amount_validator_token = liquidity
            .checked_mul(pool.reserve_validator_token)
            .and_then(|prod| prod.checked_div(total_supply))
            .ok_or(Error::Overflow)?;

        // Check withdrawal doesn't violate pending swaps
        let pending = storage::get_pending_fee_swap(&env, &user_token, &validator_token);
        let pending_out = compute_amount_out(pending)?;
        let effective_validator_reserve = pool
            .reserve_validator_token
            .checked_sub(pending_out)
            .ok_or(Error::Overflow)?;

        if amount_validator_token > effective_validator_reserve {
            return Err(Error::InsufficientReserves);
        }

        // Burn LP tokens
        storage::set_lp_balance(
            &env,
            &user_token,
            &validator_token,
            &sender,
            balance.checked_sub(liquidity).ok_or(Error::Overflow)?,
        );

        storage::set_total_supply(
            &env,
            &user_token,
            &validator_token,
            total_supply
                .checked_sub(liquidity)
                .ok_or(Error::Overflow)?,
        );

        // Update reserves
        pool.reserve_user_token = pool
            .reserve_user_token
            .checked_sub(amount_user_token)
            .ok_or(Error::InsufficientReserves)?;
        pool.reserve_validator_token = pool
            .reserve_validator_token
            .checked_sub(amount_validator_token)
            .ok_or(Error::InsufficientReserves)?;

        storage::set_pool(&env, &user_token, &validator_token, &pool);

        // Transfer tokens to recipient
        if amount_user_token > 0 {
            let user_token_client = token::Client::new(&env, &user_token);
            user_token_client.transfer(&env.current_contract_address(), &to, &amount_user_token);
        }

        if amount_validator_token > 0 {
            let validator_token_client = token::Client::new(&env, &validator_token);
            validator_token_client.transfer(
                &env.current_contract_address(),
                &to,
                &amount_validator_token,
            );
        }

        // Emit event
        events::emit_burn(
            &env,
            &sender,
            &user_token,
            &validator_token,
            amount_user_token,
            amount_validator_token,
            liquidity,
            &to,
        );

        Ok((amount_user_token, amount_validator_token))
    }

    /// Reserve liquidity for pending fee swaps
    /// Called before executing fee transactions to ensure liquidity is available
    ///
    /// NOTE: In the original Tempo implementation, this is likely a system-level function
    /// called by the protocol during transaction processing. Here we use admin-only access
    /// as an approximation. In production, consider integrating with the fee collection system.
    pub fn reserve_liquidity(
        env: Env,
        caller: Address,
        user_token: Address,
        validator_token: Address,
        max_amount: i128,
    ) -> Result<(), Error> {
        // Only admin can reserve liquidity (typically called by fee system)
        caller.require_auth();
        let admin = storage::get_admin(&env);
        if caller != admin {
            return Err(Error::Unauthorized);
        }

        if max_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        storage::extend_instance_ttl(&env);

        let current_pending =
            storage::get_pending_fee_swap(&env, &user_token, &validator_token);

        let new_total_pending = current_pending
            .checked_add(max_amount)
            .ok_or(Error::Overflow)?;

        // Check that total output needed is within reserves
        let total_out_needed = compute_amount_out(new_total_pending)?;

        let pool = storage::get_pool(&env, &user_token, &validator_token);
        if total_out_needed > pool.reserve_validator_token {
            return Err(Error::InsufficientLiquidity);
        }

        storage::set_pending_fee_swap(&env, &user_token, &validator_token, new_total_pending);

        Ok(())
    }

    /// Release reserved liquidity (refund unused reservation)
    pub fn release_liquidity(
        env: Env,
        caller: Address,
        user_token: Address,
        validator_token: Address,
        refund_amount: i128,
    ) -> Result<(), Error> {
        caller.require_auth();
        let admin = storage::get_admin(&env);
        if caller != admin {
            return Err(Error::Unauthorized);
        }

        if refund_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        storage::extend_instance_ttl(&env);

        let current_pending =
            storage::get_pending_fee_swap(&env, &user_token, &validator_token);

        let new_pending = current_pending
            .checked_sub(refund_amount)
            .ok_or(Error::Overflow)?;

        storage::set_pending_fee_swap(&env, &user_token, &validator_token, new_pending);

        Ok(())
    }

    /// Execute all pending fee swaps for a pool
    /// Converts accumulated user tokens to validator tokens
    ///
    /// NOTE: In the original Tempo implementation, this is likely a system-level function
    /// called by the protocol during block finalization. Here we use admin-only access
    /// as an approximation. In production, consider protocol-level integration.
    pub fn execute_pending_fee_swaps(
        env: Env,
        caller: Address,
        user_token: Address,
        validator_token: Address,
    ) -> Result<i128, Error> {
        caller.require_auth();
        let admin = storage::get_admin(&env);
        if caller != admin {
            return Err(Error::Unauthorized);
        }

        storage::extend_instance_ttl(&env);

        let amount_in = storage::get_pending_fee_swap(&env, &user_token, &validator_token);
        if amount_in == 0 {
            return Ok(0);
        }

        let pending_out = compute_amount_out(amount_in)?;

        let mut pool = storage::get_pool(&env, &user_token, &validator_token);

        // Update reserves: user tokens go in, validator tokens go out
        pool.reserve_user_token = pool
            .reserve_user_token
            .checked_add(amount_in)
            .ok_or(Error::Overflow)?;

        pool.reserve_validator_token = pool
            .reserve_validator_token
            .checked_sub(pending_out)
            .ok_or(Error::Overflow)?;

        storage::set_pool(&env, &user_token, &validator_token, &pool);
        storage::clear_pending_fee_swap(&env, &user_token, &validator_token);

        // Emit event
        events::emit_fee_swap(&env, &user_token, &validator_token, amount_in, pending_out);

        Ok(pending_out)
    }

    /// Rebalance swap: exchange validator tokens for user tokens
    /// Used to rebalance pools when they become imbalanced
    ///
    /// NOTE: In the original Tempo implementation, this function may be intended for
    /// validators or privileged actors to rebalance pools. Currently permissionless -
    /// any user with validator tokens can call it. Consider adding access control
    /// if rebalancing should be restricted.
    pub fn rebalance_swap(
        env: Env,
        sender: Address,
        user_token: Address,
        validator_token: Address,
        amount_out: i128,
        to: Address,
    ) -> Result<i128, Error> {
        sender.require_auth();

        if amount_out <= 0 {
            return Err(Error::InvalidAmount);
        }

        storage::extend_instance_ttl(&env);

        let mut pool = storage::get_pool(&env, &user_token, &validator_token);

        // Check sufficient user token reserves
        if amount_out > pool.reserve_user_token {
            return Err(Error::InsufficientReserves);
        }

        // Calculate input: amount_in = amount_out * N / SCALE + 1
        let amount_in = amount_out
            .checked_mul(N)
            .and_then(|prod| prod.checked_div(SCALE))
            .and_then(|res| res.checked_add(1))
            .ok_or(Error::Overflow)?;

        // Update reserves: validator tokens in, user tokens out
        pool.reserve_validator_token = pool
            .reserve_validator_token
            .checked_add(amount_in)
            .ok_or(Error::Overflow)?;

        pool.reserve_user_token = pool
            .reserve_user_token
            .checked_sub(amount_out)
            .ok_or(Error::InsufficientReserves)?;

        storage::set_pool(&env, &user_token, &validator_token, &pool);

        // Transfer tokens
        let validator_token_client = token::Client::new(&env, &validator_token);
        validator_token_client.transfer(&sender, &env.current_contract_address(), &amount_in);

        let user_token_client = token::Client::new(&env, &user_token);
        user_token_client.transfer(&env.current_contract_address(), &to, &amount_out);

        // Emit event
        events::emit_rebalance_swap(
            &env,
            &user_token,
            &validator_token,
            &sender,
            amount_in,
            amount_out,
        );

        Ok(amount_in)
    }

    /// Calculate the output amount for a given input (view function)
    pub fn calculate_fee_swap_output(amount_in: i128) -> Result<i128, Error> {
        compute_amount_out(amount_in)
    }

    /// Calculate the input amount for a rebalance swap (view function)
    pub fn calculate_rebalance_input(amount_out: i128) -> Result<i128, Error> {
        amount_out
            .checked_mul(N)
            .and_then(|prod| prod.checked_div(SCALE))
            .and_then(|res| res.checked_add(1))
            .ok_or(Error::Overflow)
    }
}

#[cfg(test)]
mod test;
