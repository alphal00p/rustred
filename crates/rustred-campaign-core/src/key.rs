//! Arity-generic integral keys (skeleton; filled in by the core-domain package).

/// An integral key: one integer index per slot, line slots first, then numerator slots.
pub type Key = Vec<i32>;
