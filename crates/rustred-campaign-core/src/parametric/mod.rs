//! PARAMETRIC statements about rule tables (interfaces/parametric.md; plan/auto-campaign/reports/PARAMETRIC-ANSWER.md):
//! properties proved on the WHOLE domain of a rule, or on a stated cone of keys, not on a finite list of keys.
//!
//! - [`descent`] (B1): every right-hand-side term of one rule record against its target, on the record's whole case
//!   domain, under the node comparator: descends / descends except on stated sub-loci (positive-numerator planes
//!   without a zero proof) / does not descend (verified witness key) / undecided (never a silent pass).
//! - [`cone`] (B2): coverage at FIRST MATCH of a cone of keys (the top-sector chart orthant cut by linear constraints)
//!   by a dispatcher's rule tables, from the exact dispatch-mode partition; output covered / uncovered cells (with
//!   verified witnesses) / undecided cells, and a certificate.
//! - [`symalias`]: one-term symmetry aliases `I(k) = I(g k)` serving a gap piece (case + equations + bounds), pure
//!   verified element, exact descent on the whole piece (lane t162face).
//! - [`check`]: the independent checker of a cone certificate (re-dispatch at probe keys and witnesses, digests, cone
//!   arithmetic): different code, different algorithm (points instead of symbolic classes).
//!
//! Generic (L8): nothing here names a node, an arity, a slot or a count; orders, frames, cones and zero proofs are
//! input data. Evidence objects carry a version constant and the sha256 of the tables they are about.

pub mod check;
pub mod cone;
pub mod descent;
pub mod permuted;
pub mod prune;
pub mod symalias;

/// Schema of a B1 evidence file (JSONL: header, one line per record, summary).
pub const PARAMETRIC_DESCENT_SCHEMA: &str = "rustred.campaign.parametric-descent.v1";
/// Version of the B1 decision procedure (bump when a verdict can change for the same input).
pub const PARAMETRIC_DESCENT_VERSION: u32 = 1;
/// Schema of a B2 certificate.
pub const CONE_COVERAGE_SCHEMA: &str = "rustred.campaign.cone-coverage.v1";
/// Version of the B2 procedure.
pub const CONE_COVERAGE_VERSION: u32 = 1;
