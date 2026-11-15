//! # Composed pole profiles, eps depth of terminals, the user's eps box (lane epsdepth; interfaces/epsdepth.md)
//!
//! pole-diagnostics.md ADDENDUM 8 (the user's words of 2026-11-15, binding): the quantity to keep small is the POLE
//! ORDER at d0 with which a terminal enters the reduced form of a box key (p orders of pole = p further eps orders of
//! that terminal, "spurious coefficients"); an OPTIONAL eps box supplied by the user states what is available / needed;
//! the solve and the closure never read it, and no measure here changes a closure verdict. TRICKS 12, 13, 14, 15, 16.
//!
//! The Symbolica-free half (this crate takes no Symbolica, tests/schema_deps.rs):
//! - [`profile`]: [`profile::Val`], the [`profile::Valuation`] trait (implemented in the algebra crate by lane poles'
//!   exact valuation), LOCAL profiles of one relation at one key;
//! - [`compose`]: the COMPOSED profile: the node's served relations composed down to its terminals with truncated
//!   Laurent series (exact; generic over [`compose::Laurent`] and [`compose::ReductionSource`]);
//! - [`report`]: verdicts (regular / spurious / inherent / basis-dependent / undecided), per-terminal maxima, blame,
//!   eps-box tables;
//! - [`epsbox`]: the user's eps box (parsing, validation, required depth vs available);
//! - [`spec`]: the spec section with documented defaults.
//!
//! The RANKING over alternatives with their profiles is `crate::poles::pole_objective` (lane poles' `select`
//! generalised: one function).
//! Generic (L8): no node, count, arity or family is known here; five-loop data are test data.

pub mod compose;
pub mod epsbox;
pub mod profile;
pub mod report;
pub mod spec;

pub use compose::{
    ComposeParams, ComposeStats, Composition, KeyResult, KeyStatus, Laurent, Order, PairResult,
    ReductionSource, Terms, VectorSink, compose, compose_streaming, onto_terminals,
};
pub use epsbox::{BoxStatus, EpsBox, EpsBoxChecked};
pub use profile::{LocalProfile, TermKind, TermVal, Val, Valuation};
pub use report::{PairVerdict, node_report, verdict};
pub use spec::{Arith, EpsDepthSpec, KeyMode, TerminalSetStatus, dots_rank};
