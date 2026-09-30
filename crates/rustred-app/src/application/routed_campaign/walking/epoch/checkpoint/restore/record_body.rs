//! Restore-critical binary authorities in run order. Decode and authenticate
//! the same sealed stream; optional diagnostics are skipped with fixed scratch.
use super::super::super::anchors::{AnchorMap, AnchorScope, Lent, cell_of, low_slice};
use super::super::super::job::{JobResult, NativeKind};
use super::super::super::ledger6::{Entry6, Ledger6};
use super::super::super::merge::{self, CheckedResult, Class};
use super::super::super::records::{ResolverCounters, wire};
use super::super::super::state::WalkCounters;
#[cfg(test)]
use super::super::super::{
    job::{BreakReason, ErrorKind},
    ledger6::err_class,
};
use super::super::invalid;
use super::super::publication::FileRef;
use super::{EdgeStore, Store, record_segments};
use crate::application::routed_campaign::walking::checkpoint::manifest::Segment;
use crate::application::routed_campaign::walking::queue::Phase;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

mod authority;

pub(super) struct View<'a, const N: usize> {
    pub store: &'a Store<N>,
    pub ledger: &'a Ledger6,
    pub edges: &'a EdgeStore,
    pub anchors: &'a AnchorMap,
    pub frontier_counts: &'a BTreeMap<u32, u32>,
    pub counters: &'a WalkCounters,
    pub k: u64,
    pub input_frontiers: usize,
}

#[derive(Default)]
struct Totals {
    merge_epoch: u64,
    events: u64,
    frontiers: u64,
    known_reuse: u64,
    job_duplicates: u64,
    routed: u64,
    resolver: ResolverCounters,
}
fn add(total: &mut u64, value: u64) -> io::Result<()> {
    *total = total
        .checked_add(value)
        .ok_or_else(|| invalid("epoch record aggregate overflow"))?;
    Ok(())
}

pub(super) fn read<const N: usize>(
    directory: &Path,
    file: &FileRef,
    generation: u64,
    view: View<'_, N>,
) -> io::Result<Vec<Segment>> {
    let mut runs = view.edges.run_iter();
    let mut totals = Totals {
        frontiers: view.input_frontiers as u64,
        ..Default::default()
    };
    let segments = record_segments::read_with(
        directory,
        file,
        generation,
        view.edges.runs(),
        |segment, reader| {
            for _ in 0..segment.count {
                let (source, targets) = runs
                    .next()
                    .ok_or_else(|| invalid("epoch more records than runs"))?;
                let (record, diagnostics) = wire::authority(reader)?;
                authority::validate(record, source, targets, &view, &mut totals)?;
                wire::skip_diagnostics(reader, diagnostics)?;
            }
            Ok(())
        },
    )?;
    if runs.next().is_some()
        || totals.events != view.counters.events
        || totals.frontiers != view.counters.frontiers
        || totals.known_reuse != view.counters.known_reuse
        || totals.job_duplicates != view.counters.job_duplicates
        || totals.routed != view.counters.routed
        || !totals.resolver.agrees_with(view.counters)
    {
        return Err(invalid("epoch record-derived aggregate counters differ"));
    }
    Ok(segments)
}

#[cfg(test)]
mod tests;
