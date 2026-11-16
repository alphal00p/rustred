//! # Regular terminal set R at d0 (lane regbasis; interfaces/regbasis.md; Symbolica half)
//!
//! The Symbolica-free half (trait, spec, keys, needs, record) is `rustred_campaign_core::regbasis`. The mathematics and
//! its proofs: WS/notes/implementer.regbasis.md, THE MATHEMATICS. Here:
//! - [`series`]: exact truncated Laurent series in `t = d - d0` (Symbolica univariate polynomials over Q), parsing of the
//!   core's text exchange, exact expansion of rational functions of d;
//! - [`lattice`]: [`lattice::HermiteLattice`], the canonical reduced Hermite form over the local ring at d0 of a lattice
//!   containing `t^s O^n`, streaming insertion, membership, rescaling, Smith exponents (API FOR OTHER LANES);
//! - [`exact`]: A, M = A^-1 and det M exactly over Q(d) (numerica `Matrix::inv` / `det`), orders of every entry (lane
//!   poles' exact valuation);
//! - [`build`]: the construction (streaming band pass, per-rank lattices, Nakayama greedy, certificates, needs, export);
//! - [`tower`]: the tower test (dot-free keys, z per rank);
//! - [`standin`]: an exact stand-in of the composition for constructed tests and table-driven runs.
//!
//! L11: Symbolica has no Hermite / Smith form over a DVR or over Q[t]/t^s (verified, notes); the elimination LOGIC is
//! ours, every arithmetic step is a Symbolica polynomial operation.

pub mod adapter;
pub mod build;
pub mod exact;
pub mod lattice;
pub mod need;
pub mod package;
pub mod series;
pub mod standin;
pub mod tower;
