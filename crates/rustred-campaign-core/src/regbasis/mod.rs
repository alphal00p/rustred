//! # Regular terminal set R at d0: construction, full band, export object, needs, tower test (lane regbasis)
//!
//! Interface: WS/plan/auto-campaign/interfaces/regbasis.md. Objective: POLES_OBJECTIVE.md 2 (B), 2 (C), 3 items 2 and 4.
//! The mathematics (with proofs) is in WS/notes/implementer.regbasis.md, section THE MATHEMATICS: a set R = A T of keys is
//! regular for the band at d0 (M = A^-1 regular and every band key finite on R) iff R is a basis, over the local ring O at
//! d0, of the lattice `N_B = O-span{c(K) : K in band} + O^n` of the composed coefficient vectors on the closure's terminals;
//! by Nakayama such an R always exists among the band keys and the terminals and is CONSTRUCTED by a greedy pass in the
//! preference order (never guessed).
//!
//! This crate holds the Symbolica-free half (no algebra here):
//! - [`compose`]: the [`compose::Compose`] trait (the composed reduction of keys onto the terminals as truncated Laurent
//!   series in `t = d - d0` with exact coefficients, plus exact coefficients for the few keys of R) and its text exchange
//!   type [`compose::SeriesText`]; lane epsdepth's composition is adapted to it;
//! - [`keys`]: band keys of a box, ranks / dots, the candidate family, dot-free tower keys, the preference order;
//! - [`spec`]: the spec section `regular_basis` with documented defaults (every bound is data, L8);
//! - [`need`]: need(X_i) = base order + max pole order, against an optional availability (the user's eps box);
//! - [`record`]: the export object `rustred.campaign.regular-basis.v1` (canonical JSON, BLAKE3 digest bound to the node
//!   identity, the box, the closure statement and d0) and the tower record.
//!
//! The exact algebra (Hermite form over the local ring, exact M and M^-1, the streaming construction, the tower test) is
//! in `rustred_campaign_algebra::regbasis` (Symbolica, L11).
//!
//! Nothing here changes a closure, a verdict or a statement (POLES_OBJECTIVE 3.5): closures keep their terminals; the
//! object is applied at export and in diagnostics. Generic (L8): no node, count, arity, family or box shape is known here;
//! five-loop data are test data.

pub mod compose;
pub mod keys;
pub mod need;
pub mod record;
pub mod spec;

pub use compose::{Compose, ComposeError, ComposedVec, SeriesText};
pub use keys::{
    Preference, band_keys, candidate_family, dot_free_keys, dots_of, rank_of, sort_by_preference,
};
pub use need::{Availability, MapAvailability, NeedRow, NeedTable, need_table};
pub use spec::{CandidateOrder, CertifyKeys, RegBasisSpec};
