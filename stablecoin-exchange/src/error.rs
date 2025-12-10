use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract already initialized
    AlreadyInitialized = 1,
    /// Unauthorized operation
    Unauthorized = 2,
    /// Invalid tick value (out of bounds)
    InvalidTick = 3,
    /// Invalid flip tick for bid (must be > tick)
    InvalidBidFlipTick = 4,
    /// Invalid flip tick for ask (must be < tick)
    InvalidAskFlipTick = 5,
    /// Order not found
    OrderNotFound = 6,
    /// Cannot cancel - order not owned by caller
    NotOrderOwner = 7,
    /// Order amount too small (minimum $10)
    OrderTooSmall = 8,
    /// Insufficient balance for operation
    InsufficientBalance = 9,
    /// Trading pair already exists
    PairAlreadyExists = 10,
    /// Trading pair not found
    PairNotFound = 11,
    /// Cannot fill more than remaining
    FillExceedsRemaining = 12,
    /// Order not fully filled (for flip)
    OrderNotFullyFilled = 13,
    /// Not a flip order
    NotAFlipOrder = 14,
    /// Arithmetic overflow
    Overflow = 15,
    /// Division by zero
    DivisionByZero = 16,
    /// Invalid amount (zero or negative)
    InvalidAmount = 17,
    /// Slippage exceeded
    SlippageExceeded = 18,
    /// No liquidity available
    NoLiquidity = 19,
    /// Same token for base and quote
    SameToken = 20,
    /// Tick not aligned to spacing
    TickNotAligned = 21,
}
