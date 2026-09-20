//! Bounded scalar snapshots, independent heartbeat and process-only resources.
//!
//! Presentation is not an event journal or mathematical authority. Intermediate
//! details may be coalesced; aggregate counters are updated before coalescing.

mod format;
mod presenter;
mod resources;
mod state;

pub(crate) use presenter::FamilyCloseProgressMonitor;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod adversarial_tests;
