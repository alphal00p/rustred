//! Build capabilities for runtime-to-const-generic dispatch.
//!
//! `RUSTRED_RUNTIME_ARITIES=1,2,13,14,15 cargo build ...` selects the finite
//! registry at build time when the `runtime-arity-selection` feature is enabled. Without it, the registry is `1..=16`. Cargo rebuilds
//! this crate when the variable changes. These are compiled entry points, not
//! mathematical limits: checked `solver::bridge::*_for::<N>` calls and the
//! explicit-list form of [`crate::dispatch_arity!`] do not use this registry.
//! Other APIs can have their own representation and resource limits.
//!
//! The independent `capacity-dispatch` feature shares solver storage capacities
//! 4, 8 and 16 while keeping all registered physical arities. Without it each
//! registered arity uses its own capacity. Exact `dispatch_arity!` calls retain
//! their meaning under both policies.
//!
//! A downstream host retaining its own `solve::<N>` implementation can use:
//!
//! ```
//! fn solve<const N: usize>(label: &str) -> Result<(usize, &str), String> {
//!     Ok((N, label))
//! }
//! let n = 17;
//! let result = rustred::dispatch_arity!(
//!     n, [1, 13, 14, 15, 17], solve("host"),
//!     other => Err(format!("host has not compiled arity {other}"))
//! );
//! assert_eq!(result.unwrap(), (17, "host"));
//! ```
//!
//! Omit `[1, 13, 14, 15, 17]` to use the library's compiled registry. This only
//! selects a monomorphization: the host remains responsible for authenticating
//! its family and preserving cuts, ordering, budgets, and solver options.

include!(concat!(env!("OUT_DIR"), "/runtime_arities.rs"));

/// Sorted, unique arities compiled into the dynamic bridge and default dispatcher.
pub fn compiled_runtime_arities() -> &'static [usize] {
    COMPILED_RUNTIME_ARITIES
}

/// Inline storage capacities compiled into the runtime bridge. With
/// `capacity-dispatch`, these are 4, 8 and 16; supported physical arities
/// remain available from `compiled_runtime_arities()`.
pub fn compiled_runtime_capacities() -> &'static [usize] {
    COMPILED_RUNTIME_CAPACITIES
}
