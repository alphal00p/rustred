//! The engine's independent replay code, COPIED for the campaign's A2 identity test (plan 3.1 (e), WP-I2), plus two
//! new pieces. Provenance and every change: interfaces/ibp-rows.md section 5. Every file here imports only std,
//! serde_json and its siblings (no Symbolica, no rustred-core).
//!
//! - [`arith`], [`expr`], [`family`]: whole copies of `rustred-core/src/node/replay/{arith,expr,family}.rs` at b9f06de7.
//! - [`record`], [`span`], [`rows`]: the listed items of `replay/mod.rs` at b9f06de7 (`span` takes the prime).
//! - [`coeff`]: NEW streaming evaluator of rule coefficients "(P)/(Q)" modulo several primes at once (no tree).
//! - [`primes`]: NEW verify-range primes (deterministic Miller-Rabin) and SplitMix64 streams.
//! - [`sha256`]: NEW plain SHA-256 for the rule identity digest of the `replay` evidence record.
//! - [`symrows`]: NEW symmetry group (data, derivation, exact verification, cross-check) and symmetry rows.

pub mod arith;
pub mod coeff;
pub mod expr;
pub mod family;
pub mod primes;
pub mod record;
pub mod rows;
pub mod sha256;
pub mod span;
pub mod symrows;
