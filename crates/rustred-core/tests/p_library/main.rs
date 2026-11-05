//! The p-library tests of rustred-core: one binary, one module per M2 task
//! (each module file names its owner). Shared helpers: `tests/support/`.
#[path = "../support/mod.rs"]
mod support;

mod assemble;
mod basis;
mod canary;
mod census;
mod class;
mod custom;
mod eps;
mod iface;
mod kernel;
mod lanes;
mod plib;
mod triangle;
mod twosided;
