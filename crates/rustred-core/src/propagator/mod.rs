//! Massless propagator-type ("p") integral library: one library program per
//! release scope over vakint's families and Forcer's rule cascade (plan M2
//! sections 3-4). Every module is registered here once; each file names its
//! owner task.

// Interfaces and their self-contained implementations (M2-I0; class and
// census bodies: M2-T1 and M2-T2; library bodies: M2-T6; programs body: M2-T8a).
pub mod census;
pub mod class;
pub mod error;
pub mod families;
pub mod lane;
pub mod library;
pub mod presentation;
pub mod programs;
pub mod step;
pub mod tag;
pub mod terminal;

// Ordered adapter and test support (M2-T6).
pub mod engine;
pub mod testing;

// Kernels and the Insertion, SeriesParallel and Carpet lanes (M2-T4).
pub mod carpet;
pub mod insertion;
pub mod kernel;
pub mod series_parallel;

// Lane decision and presentations (M2-T3).
pub mod decide;
pub mod present;

// Triangle lane (M2-T5).
pub mod triangle;

// Custom owners (M2-T7c).
pub mod custom;

// Descent certificate (M2-T8a).
pub mod descent;

// Basis layer onto the Forcer masters (M2-T9).
pub mod basis;

// Version of the topology keys and the reduction DAG (integrator topology commit).
pub use rustred_topology::P_TOPOLOGY_VERSION;
