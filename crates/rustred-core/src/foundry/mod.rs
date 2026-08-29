//! Offline derivation of proof-bearing integral rules.
//!
//! The current capability is deliberately narrow: [`anchored`] derives one
//! exact rule at one concrete integer anchor. It is not a closure engine,
//! resumable workspace, published artifact, or integral reducer.

pub mod anchored;
