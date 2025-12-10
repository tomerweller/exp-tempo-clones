#![no_std]

mod error;
mod events;
mod order;
mod orderbook;
mod storage;

use error::Error;
use order::Order;
use orderbook::{
    calculate_base_amount, calculate_quote_amount, find_next_ask_tick,
    find_next_bid_tick, get_ask_tick_level, get_bid_tick_level, get_orderbook, has_orderbook,
    save_ask_tick_level, save_bid_tick_level, save_orderbook, tick_to_price, update_best_ask_tick,
    update_best_bid_tick, validate_tick, Orderbook, TickLevel, MAX_TICK, MIN_ORDER_SIZE, MIN_TICK,
    PRICE_SCALE, TICK_SPACING,
};
use soroban_sdk::{contract, contractimpl, token, Address, Env};

#[contract]
pub struct StablecoinExchange;

#[contractimpl]
impl StablecoinExchange {
    // ============ Initialization ============

    /// Initialize the contract with an admin
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if storage::has_admin(&env) {
            return Err(Error::AlreadyInitialized);
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

    // ============ Trading Pair Management ============

    /// Create a new trading pair
    pub fn create_pair(
        env: Env,
        caller: Address,
        base_token: Address,
        quote_token: Address,
    ) -> Result<(), Error> {
        caller.require_auth();

        if base_token == quote_token {
            return Err(Error::SameToken);
        }

        if has_orderbook(&env, &base_token, &quote_token) {
            return Err(Error::PairAlreadyExists);
        }

        storage::extend_instance_ttl(&env);

        let orderbook = Orderbook::new(base_token.clone(), quote_token.clone());
        save_orderbook(&env, &orderbook);

        events::emit_pair_created(&env, &base_token, &quote_token);

        Ok(())
    }

    /// Get orderbook state
    pub fn get_orderbook(
        env: Env,
        base_token: Address,
        quote_token: Address,
    ) -> Result<Orderbook, Error> {
        storage::extend_instance_ttl(&env);
        get_orderbook(&env, &base_token, &quote_token).ok_or(Error::PairNotFound)
    }

    // ============ Order Placement ============

    /// Place a limit order
    pub fn place(
        env: Env,
        maker: Address,
        base_token: Address,
        quote_token: Address,
        is_bid: bool,
        tick: i32,
        amount: i128,
    ) -> Result<u128, Error> {
        maker.require_auth();
        validate_tick(tick)?;

        if amount < MIN_ORDER_SIZE {
            return Err(Error::OrderTooSmall);
        }

        storage::extend_instance_ttl(&env);

        // Verify pair exists
        let _orderbook =
            get_orderbook(&env, &base_token, &quote_token).ok_or(Error::PairNotFound)?;

        // Calculate and transfer deposit
        let deposit_token;
        let deposit_amount;

        if is_bid {
            // Buying base with quote: deposit quote tokens
            deposit_token = quote_token.clone();
            deposit_amount = calculate_quote_amount(amount, tick);
        } else {
            // Selling base for quote: deposit base tokens
            deposit_token = base_token.clone();
            deposit_amount = amount;
        }

        // Transfer tokens to contract
        let token_client = token::Client::new(&env, &deposit_token);
        token_client.transfer(&maker, &env.current_contract_address(), &deposit_amount);

        // Create pending order
        let order_id = storage::get_next_pending_order_id(&env);
        let new_order = if is_bid {
            Order::new_bid(order_id, maker.clone(), base_token.clone(), quote_token.clone(), tick, amount)
        } else {
            Order::new_ask(order_id, maker.clone(), base_token.clone(), quote_token.clone(), tick, amount)
        };

        order::save_pending_order(&env, &new_order);

        events::emit_order_placed(
            &env,
            order_id,
            &maker,
            &base_token,
            &quote_token,
            is_bid,
            tick,
            amount,
            false,
        );

        Ok(order_id)
    }

    /// Place a flip order (auto-creates opposite side when filled)
    pub fn place_flip(
        env: Env,
        maker: Address,
        base_token: Address,
        quote_token: Address,
        is_bid: bool,
        tick: i32,
        amount: i128,
        flip_tick: i32,
    ) -> Result<u128, Error> {
        maker.require_auth();
        validate_tick(tick)?;
        validate_tick(flip_tick)?;

        if amount < MIN_ORDER_SIZE {
            return Err(Error::OrderTooSmall);
        }

        storage::extend_instance_ttl(&env);

        // Verify pair exists
        let _orderbook =
            get_orderbook(&env, &base_token, &quote_token).ok_or(Error::PairNotFound)?;

        // Calculate and transfer deposit
        let deposit_token;
        let deposit_amount;

        if is_bid {
            deposit_token = quote_token.clone();
            deposit_amount = calculate_quote_amount(amount, tick);
        } else {
            deposit_token = base_token.clone();
            deposit_amount = amount;
        }

        // Transfer tokens to contract
        let token_client = token::Client::new(&env, &deposit_token);
        token_client.transfer(&maker, &env.current_contract_address(), &deposit_amount);

        // Create pending flip order
        let order_id = storage::get_next_pending_order_id(&env);
        let new_order = if is_bid {
            Order::new_flip_bid(order_id, maker.clone(), base_token.clone(), quote_token.clone(), tick, amount, flip_tick)?
        } else {
            Order::new_flip_ask(order_id, maker.clone(), base_token.clone(), quote_token.clone(), tick, amount, flip_tick)?
        };

        order::save_pending_order(&env, &new_order);

        events::emit_order_placed(
            &env,
            order_id,
            &maker,
            &base_token,
            &quote_token,
            is_bid,
            tick,
            amount,
            true,
        );

        Ok(order_id)
    }

    /// Execute pending orders (activate them into the orderbook)
    ///
    /// WARNING: In the original Tempo implementation, this function is privileged
    /// and can only be called by the protocol (Address::ZERO) during block finalization.
    /// This prevents front-running and selective order activation.
    /// In this Soroban port, the function is permissionless - any user can call it.
    /// Consider adding admin-only restriction for production use.
    pub fn execute_block(
        env: Env,
        base_token: Address,
        quote_token: Address,
        order_ids: soroban_sdk::Vec<u128>,
    ) -> Result<(), Error> {
        // TODO: Add access control - original Tempo requires sender == Address::ZERO
        storage::extend_instance_ttl(&env);

        let mut orderbook =
            get_orderbook(&env, &base_token, &quote_token).ok_or(Error::PairNotFound)?;

        for order_id in order_ids.iter() {
            if let Some(pending_order) = order::get_pending_order(&env, order_id) {
                // Move to active and link into orderbook
                Self::activate_order(&env, &mut orderbook, pending_order)?;
                order::delete_pending_order(&env, order_id);
            }
        }

        save_orderbook(&env, &orderbook);
        Ok(())
    }

    /// Cancel an order
    pub fn cancel(env: Env, maker: Address, order_id: u128) -> Result<i128, Error> {
        maker.require_auth();
        storage::extend_instance_ttl(&env);

        // Try pending order first
        if let Some(pending_order) = order::get_pending_order(&env, order_id) {
            if pending_order.maker != maker {
                return Err(Error::NotOrderOwner);
            }

            let refund = pending_order.remaining;
            order::delete_pending_order(&env, order_id);

            // Refund is handled by the caller through withdraw
            storage::add_balance(&env, &maker, &pending_order.maker, refund);

            events::emit_order_canceled(&env, order_id, &maker, refund);
            return Ok(refund);
        }

        // Try active order
        if let Some(active_order) = order::get_order(&env, order_id) {
            if active_order.maker != maker {
                return Err(Error::NotOrderOwner);
            }

            // Remove from orderbook linked list
            Self::remove_order_from_book(&env, &active_order)?;

            let refund = active_order.remaining;
            order::delete_order(&env, order_id);

            // Add to balance for withdrawal
            storage::add_balance(&env, &maker, &active_order.maker, refund);

            events::emit_order_canceled(&env, order_id, &maker, refund);
            return Ok(refund);
        }

        Err(Error::OrderNotFound)
    }

    // ============ Swap Execution ============

    /// Swap exact amount in (taker sells exact amount)
    pub fn swap_exact_in(
        env: Env,
        taker: Address,
        base_token: Address,
        quote_token: Address,
        is_buy: bool, // true = buy base with quote, false = sell base for quote
        amount_in: i128,
        min_amount_out: i128,
    ) -> Result<i128, Error> {
        taker.require_auth();
        storage::extend_instance_ttl(&env);

        let mut orderbook =
            get_orderbook(&env, &base_token, &quote_token).ok_or(Error::PairNotFound)?;

        // Transfer input tokens from taker
        let input_token = if is_buy {
            &quote_token
        } else {
            &base_token
        };
        let token_client = token::Client::new(&env, input_token);
        token_client.transfer(&taker, &env.current_contract_address(), &amount_in);

        let mut remaining_in = amount_in;
        let mut total_out: i128 = 0;

        if is_buy {
            // Buy base with quote: match against asks
            while remaining_in > 0 && orderbook.has_asks() {
                let tick = orderbook.best_ask_tick;
                let mut level = get_ask_tick_level(&env, &base_token, &quote_token, tick);

                if level.is_empty() {
                    // Find next ask tick
                    if let Some(next_tick) = find_next_ask_tick(&env, &base_token, &quote_token, tick + TICK_SPACING)
                    {
                        orderbook.best_ask_tick = next_tick;
                        continue;
                    } else {
                        break;
                    }
                }

                // Calculate how much base we can buy with remaining quote
                let base_available = calculate_base_amount(remaining_in, tick);
                let fill_amount = base_available.min(level.total_liquidity);

                if fill_amount == 0 {
                    break;
                }

                // Fill orders at this tick
                let (filled_base, filled_quote) =
                    Self::fill_tick_level(&env, &mut level, &base_token, &quote_token, tick, fill_amount, false)?;

                remaining_in -= filled_quote;
                total_out += filled_base;

                // Save updated level
                if level.is_empty() {
                    orderbook::delete_ask_tick_level(&env, &base_token, &quote_token, tick);
                    update_best_ask_tick(&env, &mut orderbook);
                } else {
                    save_ask_tick_level(&env, &base_token, &quote_token, tick, &level);
                }
            }
        } else {
            // Sell base for quote: match against bids
            while remaining_in > 0 && orderbook.has_bids() {
                let tick = orderbook.best_bid_tick;
                let mut level = get_bid_tick_level(&env, &base_token, &quote_token, tick);

                if level.is_empty() {
                    // Find next bid tick
                    if let Some(next_tick) = find_next_bid_tick(&env, &base_token, &quote_token, tick - TICK_SPACING)
                    {
                        orderbook.best_bid_tick = next_tick;
                        continue;
                    } else {
                        break;
                    }
                }

                let fill_amount = remaining_in.min(level.total_liquidity);

                if fill_amount == 0 {
                    break;
                }

                // Fill orders at this tick
                let (filled_base, filled_quote) =
                    Self::fill_tick_level(&env, &mut level, &base_token, &quote_token, tick, fill_amount, true)?;

                remaining_in -= filled_base;
                total_out += filled_quote;

                // Save updated level
                if level.is_empty() {
                    orderbook::delete_bid_tick_level(&env, &base_token, &quote_token, tick);
                    update_best_bid_tick(&env, &mut orderbook);
                } else {
                    save_bid_tick_level(&env, &base_token, &quote_token, tick, &level);
                }
            }
        }

        // Check slippage
        if total_out < min_amount_out {
            return Err(Error::SlippageExceeded);
        }

        // Refund unused input
        if remaining_in > 0 {
            token_client.transfer(&env.current_contract_address(), &taker, &remaining_in);
        }

        // Transfer output to taker
        let output_token = if is_buy {
            &base_token
        } else {
            &quote_token
        };
        let out_token_client = token::Client::new(&env, output_token);
        out_token_client.transfer(&env.current_contract_address(), &taker, &total_out);

        save_orderbook(&env, &orderbook);

        events::emit_trade(
            &env,
            &base_token,
            &quote_token,
            &taker,
            is_buy,
            if is_buy { total_out } else { amount_in - remaining_in },
            if is_buy { amount_in - remaining_in } else { total_out },
            orderbook.best_bid_tick,
        );

        Ok(total_out)
    }

    /// Quote swap exact amount in
    pub fn quote_swap_in(
        env: Env,
        base_token: Address,
        quote_token: Address,
        is_buy: bool,
        amount_in: i128,
    ) -> Result<i128, Error> {
        storage::extend_instance_ttl(&env);

        let orderbook =
            get_orderbook(&env, &base_token, &quote_token).ok_or(Error::PairNotFound)?;

        let mut remaining_in = amount_in;
        let mut total_out: i128 = 0;

        if is_buy {
            let mut tick = orderbook.best_ask_tick;
            while remaining_in > 0 && tick <= MAX_TICK {
                let level = get_ask_tick_level(&env, &base_token, &quote_token, tick);
                if level.is_empty() {
                    tick += TICK_SPACING;
                    continue;
                }

                let base_available = calculate_base_amount(remaining_in, tick);
                let fill_amount = base_available.min(level.total_liquidity);

                if fill_amount > 0 {
                    let quote_cost = calculate_quote_amount(fill_amount, tick);
                    remaining_in -= quote_cost;
                    total_out += fill_amount;
                }

                tick += TICK_SPACING;
            }
        } else {
            let mut tick = orderbook.best_bid_tick;
            while remaining_in > 0 && tick >= MIN_TICK {
                let level = get_bid_tick_level(&env, &base_token, &quote_token, tick);
                if level.is_empty() {
                    tick -= TICK_SPACING;
                    continue;
                }

                let fill_amount = remaining_in.min(level.total_liquidity);

                if fill_amount > 0 {
                    let quote_received = calculate_quote_amount(fill_amount, tick);
                    remaining_in -= fill_amount;
                    total_out += quote_received;
                }

                tick -= TICK_SPACING;
            }
        }

        Ok(total_out)
    }

    // ============ Balance Management ============

    /// Get user's exchange balance for a token
    pub fn balance_of(env: Env, user: Address, token: Address) -> i128 {
        storage::extend_instance_ttl(&env);
        storage::get_balance(&env, &user, &token)
    }

    /// Withdraw tokens from exchange balance
    pub fn withdraw(env: Env, user: Address, token: Address, amount: i128) -> Result<(), Error> {
        user.require_auth();
        storage::extend_instance_ttl(&env);

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        if !storage::sub_balance(&env, &user, &token, amount) {
            return Err(Error::InsufficientBalance);
        }

        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&env.current_contract_address(), &user, &amount);

        events::emit_withdraw(&env, &user, &token, amount);

        Ok(())
    }

    // ============ View Functions ============

    /// Get order by ID
    pub fn get_order(env: Env, order_id: u128) -> Option<Order> {
        storage::extend_instance_ttl(&env);
        order::get_order(&env, order_id)
    }

    /// Get pending order by ID
    pub fn get_pending_order(env: Env, order_id: u128) -> Option<Order> {
        storage::extend_instance_ttl(&env);
        order::get_pending_order(&env, order_id)
    }

    /// Get tick level
    pub fn get_tick_level(
        env: Env,
        base_token: Address,
        quote_token: Address,
        is_bid: bool,
        tick: i32,
    ) -> TickLevel {
        storage::extend_instance_ttl(&env);
        if is_bid {
            get_bid_tick_level(&env, &base_token, &quote_token, tick)
        } else {
            get_ask_tick_level(&env, &base_token, &quote_token, tick)
        }
    }

    /// Get constants
    pub fn min_tick() -> i32 {
        MIN_TICK
    }

    pub fn max_tick() -> i32 {
        MAX_TICK
    }

    pub fn tick_spacing() -> i32 {
        TICK_SPACING
    }

    pub fn price_scale() -> i128 {
        PRICE_SCALE
    }

    pub fn min_order_size() -> i128 {
        MIN_ORDER_SIZE
    }

    pub fn tick_to_price(tick: i32) -> i128 {
        tick_to_price(tick)
    }

    // ============ Internal Functions ============

    fn activate_order(
        env: &Env,
        orderbook: &mut Orderbook,
        mut pending_order: Order,
    ) -> Result<(), Error> {
        // Assign new active order ID
        let active_id = storage::get_next_active_order_id(env);
        pending_order.order_id = active_id;

        let base_token = &pending_order.base_token;
        let quote_token = &pending_order.quote_token;

        // Get appropriate tick level
        let mut level = if pending_order.is_bid {
            get_bid_tick_level(env, base_token, quote_token, pending_order.tick)
        } else {
            get_ask_tick_level(env, base_token, quote_token, pending_order.tick)
        };

        // Add to end of linked list at this tick
        if level.tail == 0 {
            // First order at this tick
            level.head = active_id;
            level.tail = active_id;
        } else {
            // Append to existing list
            if let Some(mut tail_order) = order::get_order(env, level.tail) {
                tail_order.next = active_id;
                order::save_order(env, &tail_order);
            }
            pending_order.prev = level.tail;
            level.tail = active_id;
        }

        level.total_liquidity += pending_order.remaining;

        // Save order and level
        order::save_order(env, &pending_order);

        if pending_order.is_bid {
            save_bid_tick_level(env, base_token, quote_token, pending_order.tick, &level);
            if pending_order.tick > orderbook.best_bid_tick {
                orderbook.best_bid_tick = pending_order.tick;
            }
        } else {
            save_ask_tick_level(env, base_token, quote_token, pending_order.tick, &level);
            if pending_order.tick < orderbook.best_ask_tick {
                orderbook.best_ask_tick = pending_order.tick;
            }
        }

        Ok(())
    }

    fn remove_order_from_book(env: &Env, order_to_remove: &Order) -> Result<(), Error> {
        let base_token = &order_to_remove.base_token;
        let quote_token = &order_to_remove.quote_token;
        let tick = order_to_remove.tick;

        let mut level = if order_to_remove.is_bid {
            get_bid_tick_level(env, base_token, quote_token, tick)
        } else {
            get_ask_tick_level(env, base_token, quote_token, tick)
        };

        // Update linked list
        if order_to_remove.prev != 0 {
            if let Some(mut prev_order) = order::get_order(env, order_to_remove.prev) {
                prev_order.next = order_to_remove.next;
                order::save_order(env, &prev_order);
            }
        } else {
            level.head = order_to_remove.next;
        }

        if order_to_remove.next != 0 {
            if let Some(mut next_order) = order::get_order(env, order_to_remove.next) {
                next_order.prev = order_to_remove.prev;
                order::save_order(env, &next_order);
            }
        } else {
            level.tail = order_to_remove.prev;
        }

        level.total_liquidity -= order_to_remove.remaining;

        // Save or delete level
        if level.is_empty() {
            if order_to_remove.is_bid {
                orderbook::delete_bid_tick_level(env, base_token, quote_token, tick);
            } else {
                orderbook::delete_ask_tick_level(env, base_token, quote_token, tick);
            }
        } else if order_to_remove.is_bid {
            save_bid_tick_level(env, base_token, quote_token, tick, &level);
        } else {
            save_ask_tick_level(env, base_token, quote_token, tick, &level);
        }

        Ok(())
    }

    fn fill_tick_level(
        env: &Env,
        level: &mut TickLevel,
        base_token: &Address,
        quote_token: &Address,
        tick: i32,
        mut amount_to_fill: i128,
        is_bid: bool,
    ) -> Result<(i128, i128), Error> {
        let mut total_base_filled: i128 = 0;
        let mut total_quote_filled: i128 = 0;

        let mut current_order_id = level.head;

        while amount_to_fill > 0 && current_order_id != 0 {
            let mut current_order = order::get_order(env, current_order_id)
                .ok_or(Error::OrderNotFound)?;

            let fill_amount = amount_to_fill.min(current_order.remaining);
            current_order.fill(fill_amount)?;

            let base_amount = fill_amount;
            let quote_amount = calculate_quote_amount(fill_amount, tick);

            total_base_filled += base_amount;
            total_quote_filled += quote_amount;
            amount_to_fill -= fill_amount;
            level.total_liquidity -= fill_amount;

            // Credit maker with the appropriate token
            let credit_token = if is_bid {
                base_token // Maker bid gets base
            } else {
                quote_token // Maker ask gets quote
            };
            let credit_amount = if is_bid {
                base_amount
            } else {
                quote_amount
            };
            storage::add_balance(env, &current_order.maker, credit_token, credit_amount);

            events::emit_order_filled(
                env,
                current_order_id,
                &current_order.maker,
                fill_amount,
                current_order.remaining,
            );

            let next_order_id = current_order.next;

            if current_order.is_fully_filled() {
                // Handle flip order
                if current_order.is_flip {
                    let flipped = current_order
                        .create_flipped_order(storage::get_next_pending_order_id(env))?;
                    order::save_pending_order(env, &flipped);
                }

                // Remove from list
                level.head = next_order_id;
                if next_order_id == 0 {
                    level.tail = 0;
                } else if let Some(mut next_order) = order::get_order(env, next_order_id) {
                    next_order.prev = 0;
                    order::save_order(env, &next_order);
                }

                order::delete_order(env, current_order_id);
            } else {
                order::save_order(env, &current_order);
            }

            current_order_id = next_order_id;
        }

        Ok((total_base_filled, total_quote_filled))
    }
}

#[cfg(test)]
mod test;
