//! Restore-critical record summaries in run order, through the same reader
//! which authenticates each sealed body. Diagnostic payloads are skipped,
//! never collected into a whole record/section; cold reinspection is separate.
use super::super::super::anchors::{AnchorMap, cell_of, low_slice};
use super::super::super::job::{BreakReason, ErrorKind, JobResult, NativeKind};
use super::super::super::ledger6::{Entry6, Ledger6, err_class};
use super::super::super::merge::{self, CheckedResult, Class};
use super::super::super::records::{CONTAINMENT_AUTHORITY, ResolverCounters};
use super::super::super::state::WalkCounters;
use super::super::invalid;
use super::super::publication::FileRef;
use super::super::read::Budget;
use super::{EdgeStore, Store, record_segments};
use crate::application::routed_campaign::walking::checkpoint::manifest::Segment;
use crate::application::routed_campaign::walking::queue::Phase;
use crate::application::routed_campaign::walking::{mask, power_bounds_json};
use fields::Fields;
use serde::de::DeserializeSeed;
use serde_json::{Value, json};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

mod fields;

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
fn integer(value: &Value, key: &str) -> io::Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("epoch record metadata integer"))
}
fn text<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("epoch record metadata text"))
}
fn boolean(value: &Value, key: &str) -> io::Result<bool> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("epoch record metadata bool"))
}

fn validate<const N: usize>(
    fields: Fields,
    source: u32,
    targets: &[u32],
    view: &View<'_, N>,
    totals: &mut Totals,
) -> io::Result<()> {
    let image = view
        .store
        .domains
        .get(source as usize)
        .ok_or_else(|| invalid("epoch record ID range"))?;
    let domain = image.expand();
    for (key, expected) in [
        ("id", json!(source)),
        ("phase", json!(format!("{:?}", domain.phase))),
        ("owner", json!(mask(&domain.owner))),
        ("lower", json!(domain.lower)),
        ("upper", json!(domain.upper)),
        ("rank", json!(domain.rank)),
        ("power_bounds", power_bounds_json(domain.powers)),
    ] {
        if fields.value(key)? != &expected {
            return Err(invalid("epoch record canonical geometry differs"));
        }
    }
    let entry = view
        .ledger
        .get(source)
        .map_err(|_| invalid("epoch record ledger source"))?;
    let epoch = fields.value("epoch")?;
    let merge_epoch = integer(epoch, "merge_epoch")?;
    if merge_epoch == 0 || merge_epoch > view.k || merge_epoch < totals.merge_epoch {
        return Err(invalid("epoch record merge version"));
    }
    totals.merge_epoch = merge_epoch;
    if let Entry6::Alias { to } = entry {
        if fields.text("record_kind")? != "delegated_not_inspected"
            || fields.u64("representative_id")? != u64::from(to)
            || fields.boolean("local_inspection_finished")?
            || fields.text("containment_authority")? != CONTAINMENT_AUTHORITY
            || fields.text("responsibility_status")? != "pending"
            || !matches!(text(epoch, "transition")?, "T3" | "T10")
            || fields.frontiers.is_some()
            || fields.has_error.is_some()
        {
            return Err(invalid("epoch alias record differs from ledger"));
        }
        return Ok(());
    }
    let (class, stored_epoch) = match entry {
        Entry6::Native { epoch, .. } => (Class::C0, epoch),
        Entry6::NativeFrontier { epoch } => (Class::C4, epoch),
        Entry6::NativeError { epoch, .. } => (Class::C2, epoch),
        _ => return Err(invalid("epoch record belongs to unmerged ID")),
    };
    let v0 = integer(epoch, "v0")?;
    let frontiers = fields
        .frontiers
        .ok_or_else(|| invalid("epoch native frontier array missing"))?;
    let error = fields
        .has_error
        .ok_or_else(|| invalid("epoch native error field missing"))?;
    let anchored = view.anchors.get(source);
    let expected_kind = if anchored.is_some() {
        "partial_initial_overlap_inspection"
    } else {
        "native_inspection"
    };
    if fields.text("record_kind")? != expected_kind
        || merge_epoch != stored_epoch
        || v0.checked_add(1) != Some(merge_epoch)
        || integer(epoch, "distinct_edge_count")? != targets.len() as u64
        || boolean(epoch, "self_edge")? != targets.binary_search(&source).is_ok()
        || text(epoch, "class")? != class.name()
        || epoch.get("refresh_points") != Some(&json!([]))
        || error != (class == Class::C2)
        || (class == Class::C0 && frontiers != 0)
        || (class == Class::C4
            && view.frontier_counts.get(&source).copied().map(u64::from) != Some(frontiers))
        || fields.boolean("local_inspection_finished")?
            != (class != Class::C2 && anchored.is_none())
        || fields.boolean("local_classification_discharged")?
            != (class == Class::C0 && anchored.is_none())
    {
        return Err(invalid("epoch native record class, scope or run differs"));
    }
    if let Some(anchor) = anchored {
        if anchor.dispatch_version != v0 {
            return Err(invalid(
                "epoch anchor and native record dispatch versions differ",
            ));
        }
        let (anchor_id, cut) = anchor
            .d_band()
            .ok_or_else(|| invalid("epoch record unsupported anchor"))?;
        let residual =
            low_slice(cell_of(image), cut).ok_or_else(|| invalid("epoch record residual cut"))?;
        let expected = json!({"anchor_id":anchor_id,"cut":cut,"covered_slice":"original_intersect_D_ge_cut",
            "residual_power_bounds":power_bounds_json(residual.powers),"coordinates_and_rank_unchanged":true,
            "authority":CONTAINMENT_AUTHORITY});
        if fields.value("initial_overlap")? != &expected
            || fields.text("native_inspection_scope")? != "low_D_residual_only"
            || fields.boolean("residual_inspection_finished")? != (class != Class::C2)
        {
            return Err(invalid("epoch partial record scope differs from anchor"));
        }
    } else if fields.has("initial_overlap")
        || fields.has("residual_inspection_finished")
        || fields.has("native_inspection_scope")
    {
        return Err(invalid("epoch plain record carries partial scope"));
    }
    let delta: ResolverCounters = serde_json::from_value(
        epoch
            .get("resolver_counters")
            .ok_or_else(|| invalid("epoch resolver counter provenance missing"))?
            .clone(),
    )
    .map_err(io::Error::other)?;
    match image.phase() {
        Phase::Apply if delta.route_masks != 0 || delta.route_joint_pruned != 0 => {
            return Err(invalid("epoch Apply record carries Route counters"));
        }
        Phase::Route if delta.optional != [0; 3] => {
            return Err(invalid("epoch Route record carries Apply counters"));
        }
        _ => {}
    }
    let panic = boolean(epoch, "panic")?;
    let stats = fields.value("stats")?;
    let stats_events = if panic {
        if !stats.is_null() || !targets.is_empty() {
            return Err(invalid("epoch recurring panic has stats or edges"));
        }
        u64::MAX
    } else {
        integer(stats, "events")?
    };
    let error_name = text(epoch, "error_kind")?;
    let error_kind = ErrorKind::of(error_name);
    if error_kind.name() != error_name {
        return Err(invalid("epoch record error kind"));
    }
    let break_name = text(epoch, "break_reason")?;
    let break_reason = [
        BreakReason::None,
        BreakReason::Allowance,
        BreakReason::ResolverRange,
        BreakReason::ResolverSummary,
        BreakReason::ResolverDiagnostic,
        BreakReason::SpillIo,
        BreakReason::Alloc,
        BreakReason::Cancel,
        BreakReason::Protocol,
    ]
    .into_iter()
    .find(|reason| reason.name() == break_name)
    .ok_or_else(|| invalid("epoch record break reason"))?;
    // Reuse P1's source of truth for parity/class/error. It consults only
    // frontier emptiness, so one empty marker substitutes for an arbitrary
    // diagnostic array; nothing proportional to frontier count is allocated.
    let observation = JobResult::<N> {
        seq: 0,
        parent: source,
        v0,
        kind: if image.phase() == Phase::Route {
            NativeKind::Route
        } else if anchored.is_some() {
            NativeKind::ApplyPartial
        } else {
            NativeKind::Apply
        },
        error_kind,
        break_reason,
        panic,
        emitted: integer(epoch, "emitted_events")?,
        accepted: fields.u64("accepted_events")?,
        stats_events,
        successors: delta.successors,
        conditional: delta.conditional,
        known_reuse: integer(epoch, "known_reuse")?,
        job_duplicates: integer(epoch, "job_local_duplicates")?,
        optional: delta.optional,
        route_masks: delta.route_masks,
        route_joint_pruned: delta.route_joint_pruned,
        seconds: 0.0,
        stats_json: Vec::new(),
        error: None,
        frontiers: if frontiers == 0 {
            Vec::new()
        } else {
            vec![Vec::new()]
        },
        refusals: Vec::new(),
        refusals_truncated: false,
        scope: None,
        g2: None,
        misses: Vec::new(),
    };
    // Merged C2 may be a recurring C3. The old retry word no longer exists;
    // persisted terminal error class, not invented retry accounting, is checked.
    let (derived, _) =
        merge::classify(&observation, 4).map_err(|_| invalid("epoch record P1 parity/protocol"))?;
    if derived != class {
        return Err(invalid("epoch record P1 class differs from ledger"));
    }
    if let Entry6::NativeError { err, .. } = entry {
        let checked = CheckedResult {
            recurring_panic: panic,
            result: observation.clone(),
            class,
            cause: None,
            anchors: None,
        };
        if merge::error_class(&checked) != err || text(epoch, "err_class")? != err_class::name(err)
        {
            return Err(invalid("epoch record terminal error class differs"));
        }
    }
    if image.phase() == Phase::Route {
        if !fields.boolean("conservative_route_overcover")? {
            return Err(invalid("epoch Route record scope"));
        }
        add(&mut totals.routed, 1)?;
    }
    totals.resolver.add(&delta).map_err(io::Error::other)?;
    add(&mut totals.events, observation.emitted)?;
    add(&mut totals.frontiers, frontiers)?;
    add(&mut totals.known_reuse, observation.known_reuse)?;
    add(&mut totals.job_duplicates, observation.job_duplicates)?;
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
            let budget = Cell::new(128);
            let mut decoder = serde_json::Deserializer::from_reader(Budget {
                input: reader,
                remaining: &budget,
                reason: "epoch record authority field exceeds fixed schema",
            });
            for _ in 0..segment.count {
                let (source, targets) = runs
                    .next()
                    .ok_or_else(|| invalid("epoch more records than runs"))?;
                let fields = fields::Seed(&budget)
                    .deserialize(&mut decoder)
                    .map_err(io::Error::other)?;
                validate(fields, source, targets, &view, &mut totals)?;
            }
            budget.set(128);
            decoder.end().map_err(io::Error::other)
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
