//! The DOMAIN LOGIC of the campaign's A2 identity test (plan 3.1 (e), WP-I2): the engine's independent replay code,
//! COPIED, minus its algebra, plus new pieces. Provenance and every change: interfaces/ibp-rows.md section 5. Every
//! file here imports only std, serde_json and its siblings; ALL algebra is reached through the trait
//! [`algebra::CheckAlgebra`] (DEV-17 REVISED, rows3): the segregated Symbolica-free implementation is
//! `crate::independent_algebra` (read its header), the Symbolica-backed one is the crate `rustred-campaign-algebra`.
//!
//! - [`algebra`]: the trait and the data types exchanged through it (no algebra); `AlgebraOrigin`, `Recorder`.
//! - `certificate` (crate-private, rows4): the certificate binding `CertAlg` (Independent + optional recorder): the only
//!   backend value that yields objects of certificate origin.
//! - [`family`]: copy of `rustred-core/src/node/replay/family.rs` at b9f06de7 (domain parts; arithmetic via the trait).
//! - [`record`], [`rows`]: the listed items of `replay/mod.rs` at b9f06de7.
//! - [`primes`]: NEW verify-range prime policy and SplitMix64 streams (primality via the trait).
//! - [`sha256`]: NEW plain SHA-256 for the rule identity digest of the `replay` evidence record (not algebra).
//! - [`symrows`]: NEW symmetry group (data, derivation, exact verification, cross-check) and symmetry rows.
//!
//! MOVED to `crate::independent_algebra` (rows3): `arith` (field, rationals), `expr` (engine expression reader),
//! `coeff` (streaming coefficient evaluator), `span` (sparse elimination, now `elim`), Miller-Rabin, and the matrix
//! routines of `symrows` / `family` / `ibp_rows` (now `linalg`).

pub mod algebra;
pub(crate) mod certificate;
pub mod family;
pub mod primes;
pub mod record;
pub mod rows;
pub mod sha256;
pub mod symrows;
