//! # rustred-campaign-algebra: the Symbolica-backed MODE of the campaign's identity test (DEV-17 REVISED)
//!
//! The identity test of `rustred-campaign-core` (`ibp_rows`, `replay_base`) is domain logic generic over the trait
//! `replay_base::algebra::CheckAlgebra`. Its certificate backend is the segregated, Symbolica-free
//! `rustred_campaign_core::independent_algebra::Independent` (read that module's header for why it exists). This
//! crate, the ONLY campaign crate that links Symbolica, provides:
//!
//! - [`SymbolicaAlgebra`] ([`backend`]): the same trait on Symbolica / numerica (finite fields `Zp64`, `Integer`,
//!   `Rational`, `Matrix`, `SparseRowReducer`, the atom parser + `try_to_rational_polynomial`, exactly the engine's
//!   coefficient path);
//! - the checker entry points with a mode switch ([`modes`]): [`identity_test_mode`] and [`replay_evidence_mode`] with
//!   [`IdentityMode::Independent`] (default, the certificate of record), [`IdentityMode::Symbolica`] (diagnostic) and
//!   [`IdentityMode::Both`] (the TRIAGE: `AgreePass` / `AgreeFail` = both backends and the shared-logic oracle agree
//!   the rule is not derived from its recorded seeds / `RecordInconsistent` / `SharedLogicDiffers` / `Differ` = an
//!   algebra disagreement localised to the first differing intermediate quantity, the outlier named by the
//!   [`referee`], a third implementation; see [`modes`]);
//! - the recording / planted-defect wrappers of the triage ([`wrap`]).
//!
//! The `replay` level is granted ONLY by the core's concrete certificate entry points (rows4: sealed `AlgebraOrigin`,
//! `ReplayRecord.certificate`); a Symbolica-mode record is `ReplayLevel::Diagnostic`; in `Both` anything but an
//! agreement blocks the level.
//!
//! LICENCE: Symbolica without a licence key makes concurrent processes of the same Unix user abort each other
//! host-wide. [`SymbolicaAlgebra::new`] refuses to run unless `SYMBOLICA_LICENSE` is set and valid (`source
//! WS/ops/env.sh`); the tests skip with a message otherwise. Never print the variable's value.
//!
//! SYMBOLICA ENTRY POINT NOT USED: `Token::parse_polynomial` (src/parser.rs:1909) has four defects found by the study
//! lane (symbolica-usage.md section 6: never returns on whitespace, `n0*n0` read as `n0`, panics on unknown variables,
//! quadratic on unsorted input); it is never used here for acceptance. PR candidates: WS/notes/implementer.rows3.md.

pub mod backend;
// lane epsdepth (interfaces/epsdepth.md): REQUEST TO I-INT, one line
pub mod epsdepth;
pub mod modes;
pub mod referee;
// lane poles (interfaces/pole-diagnostics.md, B0): REQUEST TO I-INT, one line
pub mod valuation;
pub mod wrap;

pub use backend::{SymbolicaAlgebra, SymbolicaBudget, licensed};
pub use modes::{
    Difference, IdentityMode, ModeRecord, ModeVerdict, Outlier, Triage, TriageFinding,
    TriageReport, identity_test_mode, replay_evidence_mode, replay_evidence_triage,
    triage_identity,
};
