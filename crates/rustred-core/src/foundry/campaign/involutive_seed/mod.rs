//! Proposal-only bridge from a sealed ordinary source module to requested
//! domain support.
//!
//! This module deliberately stops before source-incidence expansion, modular
//! discovery, exact replay, owner compilation, ledger mutation, or artifact
//! publication.  In particular, [`InvolutiveSeedStatus`] reports exhaustion
//! of one bounded Janet queue only; it is never compiler closure.
//!
//! Progress follow-up: after the audited completion loop owns a read-only
//! epoch/census snapshot callback, thread that snapshot through
//! `InvolutiveSeedProgram::try_run`. A phase-only callback here would hide the
//! expensive prolongation/autoreduction work and is therefore intentionally
//! not presented as useful progress telemetry.

mod error;
mod model;
mod run;

pub(crate) use error::InvolutiveSeedError;
pub(crate) use model::{
    InvolutiveSeedCensus, InvolutiveSeedComplementDiagnostics, InvolutiveSeedLimits,
    InvolutiveSeedLocalizationCensus, InvolutiveSeedProgram, InvolutiveSeedReport,
    InvolutiveSeedStatus, InvolutiveSeedWorkCensus,
};

#[cfg(test)]
mod tests;
