//! Read-only native consumer for versioned preparation progress.
//!
//! The producer owns scheduling and resources. This module only reads snapshots;
//! interrupting it never sends a signal to a solver or mutates campaign files.

mod model;
mod render;
mod viewer;

pub(crate) use viewer::run;
