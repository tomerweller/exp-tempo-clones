use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Tokens must have different addresses
    IdenticalAddresses = 1,
    /// Insufficient liquidity in the pool
    InsufficientLiquidity = 2,
    /// Insufficient reserves for the operation
    InsufficientReserves = 3,
    /// Invalid amount provided
    InvalidAmount = 4,
    /// Arithmetic overflow or underflow
    Overflow = 5,
    /// Division by zero
    DivisionByZero = 6,
    /// Pool not initialized
    PoolNotInitialized = 7,
    /// Unauthorized operation
    Unauthorized = 8,
    /// Invalid swap calculation
    InvalidSwapCalculation = 9,
    /// Slippage tolerance exceeded
    SlippageExceeded = 10,
}
