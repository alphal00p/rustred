//! Committed records serialized straight into their sidecar line.
//!
//! A record used to be a `serde_json::Value` tree built on the coordinator,
//! serialized once and dropped. serde_json's object is a `BTreeMap`, so its
//! line lists the keys in byte order. The records below emit the same keys,
//! in that order, with the same serde_json scalar encodings, so a sidecar
//! line is byte-identical to the former one and the Memory sink still holds
//! the identical `Value` (`Line::into_value`). Every object states its keys
//! in a `sorted_object!`, which rejects an unsorted key list at compile time.
//! Test builds also rebuild each committed record through the former `json!`
//! path (`legacy`) and require identical bytes and an identical `Value`.
use super::super::super::{
    diagnostics::OptionalRefusals,
    initial_overlap::InitialOverlapScope,
    inspection::NativeStats,
    queue::{Domain, Phase},
};
use rustred::solver::{CandidateDomainRouteStats, DomainPowerBounds, OwnerAppliedStats};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::Value;

/// One committed record: its sidecar line (`Serialize`) or, for the Memory
/// sink, the `Value` the line parses to.
pub(in super::super::super) trait Line: Serialize {
    fn into_value(self) -> Result<Value, String>;
}

impl Line for Value {
    fn into_value(self) -> Result<Value, String> {
        Ok(self)
    }
}

/// Byte order of UTF-8 keys, as `String`'s `Ord` (the `BTreeMap` order).
const fn sorted_keys(keys: &[&str]) -> bool {
    let mut i = 1;
    while i < keys.len() {
        let (a, b) = (keys[i - 1].as_bytes(), keys[i].as_bytes());
        let mut j = 0;
        while j < a.len() && j < b.len() && a[j] == b[j] {
            j += 1;
        }
        let less = if j < a.len() && j < b.len() {
            a[j] < b[j]
        } else {
            a.len() < b.len()
        };
        if !less {
            return false;
        }
        i += 1;
    }
    true
}

/// A JSON object with the given entries in the given (checked) key order.
/// `always` entries are present even when null; `some` entries take an
/// `Option` and are absent when it is `None`.
macro_rules! sorted_object {
    ($serializer:expr, { $($kind:ident $key:literal => $value:expr),+ $(,)? }) => {{
        const {
            assert!(
                sorted_keys(&[$($key),+]),
                "keys must be listed in serde_json's BTreeMap order"
            )
        };
        let mut map = $serializer.serialize_map(None)?;
        $(sorted_object!(@$kind map, $key, $value);)+
        map.end()
    }};
    (@always $map:ident, $key:literal, $value:expr) => {
        $map.serialize_entry($key, &$value)?
    };
    (@some $map:ident, $key:literal, $value:expr) => {
        if let Some(value) = $value {
            $map.serialize_entry($key, &value)?;
        }
    };
}

/// `format!("{:?}", phase)` without the allocation.
struct PhaseName(Phase);

impl Serialize for PhaseName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("{:?}", self.0))
    }
}

/// The owner mask string (`walking::mask`) without the allocation.
struct Mask<'a, const N: usize>(&'a [bool; N]);

impl<const N: usize> Serialize for Mask<'_, N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut bytes = [b'0'; N];
        for (byte, &bit) in bytes.iter_mut().zip(self.0) {
            if bit {
                *byte = b'1';
            }
        }
        serializer.serialize_str(std::str::from_utf8(&bytes).expect("ASCII mask"))
    }
}

/// `power_bounds_json`.
struct PowerBounds(DomainPowerBounds);

impl Serialize for PowerBounds {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let p = self.0;
        sorted_object!(serializer, {
            always "max_positive_power" => p.max_positive_power,
            always "max_power_difference" => p.max_power_difference,
            always "min_power_difference" => p.min_power_difference,
        })
    }
}

/// `stats_json` (applied phases) and `route_stats` (Route): the native
/// statistics of a record.
struct Stats(NativeStats);

impl Serialize for Stats {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            NativeStats::Apply(s) | NativeStats::ApplyPartial(s, _) => {
                AppliedStats(s).serialize(serializer)
            }
            NativeStats::Route(s) => RouteStats(s).serialize(serializer),
        }
    }
}

struct AppliedStats(OwnerAppliedStats);

impl Serialize for AppliedStats {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = self.0;
        sorted_object!(serializer, {
            always "application_refinement_cells" => s.application_refinement_cells,
            always "application_refinement_steps" => s.application_refinement_steps,
            always "boundary_cells" => s.boundary_cells,
            always "cancelled_groups" => s.cancelled_groups,
            always "coalescing_additions" => s.coalescing_additions,
            always "conditional_successors" => s.conditional_successors,
            always "conditional_unsupported_support_successors" =>
                s.conditional_unsupported_support_successors,
            always "correlation_empty_cells" => s.correlation_empty_cells,
            always "events" => s.events,
            always "matching" => MatchStats(s),
            always "native_operations" => s.native_operations,
            always "optional_coalesced_refusals" => s.optional_coalesced_refusals,
            always "optional_coefficient_refusals" => s.optional_coefficient_refusals,
            always "optional_original_refusals" => s.optional_original_refusals,
            always "problems" => s.problems,
            always "same_support_successors" => s.same_support_successors,
            always "selected_pieces" => s.selected_pieces,
            always "shift_groups" => s.shift_groups,
            always "sign_splits" => s.sign_splits,
            always "strict_subsupport_successors" => s.strict_subsupport_successors,
            always "successors" => s.successors,
            always "term_visits" => s.term_visits,
            always "unsupported_support_successors" => s.unsupported_support_successors,
            always "zero_sector_groups" => s.zero_sector_groups,
            always "zero_terms" => s.zero_terms,
        })
    }
}

struct MatchStats(OwnerAppliedStats);

impl Serialize for MatchStats {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let m = self.0.matching;
        sorted_object!(serializer, {
            always "cells" => m.cells,
            always "coordinate_cells" => m.coordinate_cells,
            always "correlation_empty_cells" => m.correlation_empty_cells,
            always "pieces" => m.pieces,
            always "predicates" => m.predicates,
            always "rank_empty_cells" => m.rank_empty_cells,
            always "refinement_cells" => m.refinement_cells,
            always "refinement_steps" => m.refinement_steps,
            always "rules" => m.rules,
            always "split_operations" => m.split_operations,
            always "terminal_checks" => m.terminal_checks,
        })
    }
}

struct RouteStats(CandidateDomainRouteStats);

impl Serialize for RouteStats {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = self.0;
        sorted_object!(serializer, {
            always "apply_domains" => s.apply_domains,
            always "coordinate_cells" => s.coordinate_cells,
            always "events" => s.events,
            always "joint_support_masks_pruned" => s.joint_support_masks_pruned,
            always "masks_examined" => s.masks_examined,
            always "masks_pruned" => s.masks_pruned,
            always "missing_routes" => s.missing_routes,
            always "route_domains" => s.route_domains,
            always "zero_sectors" => s.zero_sectors,
        })
    }
}

/// The `initial_overlap` object of a partial initial-overlap record.
struct InitialOverlap(InitialOverlapScope);

impl Serialize for InitialOverlap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let scope = self.0;
        sorted_object!(serializer, {
            always "anchor_id" => scope.anchor_id,
            always "authority" => "same_snapshot_phase_owner_native_summary",
            always "coordinates_and_rank_unchanged" => true,
            always "covered_slice" => "original_intersect_D_ge_cut",
            always "cut" => scope.cut,
            always "residual_power_bounds" => PowerBounds(scope.residual_powers),
        })
    }
}

/// What `commit_result` knows when it publishes a native (or subdivided
/// parent) record; `native` turns it into the record.
#[derive(Clone)]
pub(in super::super::super) struct NativeInputs<const N: usize> {
    pub id: usize,
    pub domain: Domain<N>,
    /// The publisher error when the record is opened: `error` and
    /// `local_inspection_finished`.
    pub error: Option<String>,
    /// No publisher error after the accepted-events update: the ledger
    /// flags (`local_classification_discharged`, `residual_inspection_finished`).
    pub error_free: bool,
    pub stats: NativeStats,
    pub seconds: f64,
    /// Ready runs with a replay: the published stream's accepted events.
    pub accepted_events: Option<usize>,
    pub frontiers: Vec<Value>,
    pub delegation: bool,
    /// The inspection's refusal diagnostics (applied phases only).
    pub refusals: Option<OptionalRefusals>,
    pub physical_parts: Option<Vec<Value>>,
}

/// A native, partial or subdivided-parent record. Field names are the keys;
/// `None` means the key is absent (`seconds` is `null` instead).
pub(in super::super::super) struct NativeRecord<const N: usize> {
    id: usize,
    domain: Domain<N>,
    error: Option<String>,
    local_inspection_finished: bool,
    stats: NativeStats,
    seconds: Option<f64>,
    frontiers: Vec<Value>,
    accepted_events: Option<usize>,
    conservative_route_overcover: Option<bool>,
    initial_overlap: Option<InitialOverlapScope>,
    local_classification_discharged: Option<bool>,
    native_inspection_scope: Option<&'static str>,
    optional_refusal_provenance_scope: Option<&'static str>,
    optional_refusal_provenance_truncated: Option<bool>,
    optional_refusals: Option<Vec<Value>>,
    physical_inspections: Option<usize>,
    physical_parts: Option<Vec<Value>>,
    physical_parts_expected: Option<u8>,
    physical_seconds_scope: Option<&'static str>,
    physical_seconds_sum: Option<f64>,
    record_kind: Option<&'static str>,
    residual_inspection_finished: Option<bool>,
    stats_scope: Option<&'static str>,
    unreturned_physical_parts: Option<Vec<u8>>,
}

/// The record of one native publication; each step mirrors the former
/// in-place mutation of the `json!` object (later steps overwrite keys).
pub(in super::super::super) fn native<const N: usize>(input: NativeInputs<N>) -> NativeRecord<N> {
    let NativeInputs {
        id,
        domain,
        error,
        error_free,
        stats,
        seconds,
        accepted_events,
        frontiers,
        delegation,
        refusals,
        physical_parts,
    } = input;
    let frontier_count = frontiers.len();
    let mut record = NativeRecord {
        id,
        domain,
        local_inspection_finished: error.is_none(),
        error,
        stats,
        seconds: Some(seconds),
        frontiers,
        accepted_events,
        conservative_route_overcover: None,
        initial_overlap: None,
        local_classification_discharged: None,
        native_inspection_scope: None,
        optional_refusal_provenance_scope: None,
        optional_refusal_provenance_truncated: None,
        optional_refusals: None,
        physical_inspections: None,
        physical_parts: None,
        physical_parts_expected: None,
        physical_seconds_scope: None,
        physical_seconds_sum: None,
        record_kind: None,
        residual_inspection_finished: None,
        stats_scope: None,
        unreturned_physical_parts: None,
    };
    if delegation {
        record.record_kind = Some("native_inspection");
        record.local_classification_discharged = Some(error_free && frontier_count == 0);
    }
    if let NativeStats::ApplyPartial(_, scope) = stats {
        record.record_kind = Some("partial_initial_overlap_inspection");
        record.native_inspection_scope = Some("low_D_residual_only");
        record.local_inspection_finished = false;
        record.residual_inspection_finished = Some(error_free);
        // Filled from the typed ledger at finalization, never inferred from
        // residual Finished alone or the existence of an initial anchor.
        record.local_classification_discharged = Some(false);
        record.initial_overlap = Some(scope);
    }
    match (refusals, stats) {
        (Some(refusals), NativeStats::Apply(applied) | NativeStats::ApplyPartial(applied, _)) => {
            record.optional_refusal_provenance_truncated = Some(refusals.truncated(applied));
            record.optional_refusal_provenance_scope = Some("first_per_phase_per_query");
            record.optional_refusals = Some(refusals.records);
        }
        _ => record.conservative_route_overcover = Some(true),
    }
    if let Some(parts) = physical_parts {
        record.record_kind = Some("subdivided_native_inspection");
        record.native_inspection_scope = Some("disjoint_exact_source_partition");
        record.stats_scope = Some("checked_sum_of_actual_physical_calls");
        record.physical_inspections = Some(parts.len());
        record.physical_parts_expected = Some(2);
        record.unreturned_physical_parts = Some(
            (0..2u8)
                .filter(|&part| !parts.iter().any(|p| p["part"] == part))
                .collect(),
        );
        record.physical_seconds_sum = Some(seconds);
        record.physical_seconds_scope =
            Some("sum_of_physical_call_wall_seconds; not_parent_wall_or_CPU");
        record.seconds = None;
        record.optional_refusal_provenance_scope = Some("first_per_phase_per_physical_part");
        record.optional_refusals = Some(
            parts
                .iter()
                .flat_map(|p| {
                    p["optional_refusals"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .cloned()
                })
                .collect(),
        );
        record.optional_refusal_provenance_truncated = Some(
            parts
                .iter()
                .any(|p| p["optional_refusal_provenance_truncated"] == true),
        );
        record.physical_parts = Some(parts);
    }
    record
}

impl<const N: usize> Serialize for NativeRecord<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let domain = &self.domain;
        sorted_object!(serializer, {
            some "accepted_events" => self.accepted_events,
            some "conservative_route_overcover" => self.conservative_route_overcover,
            always "error" => self.error,
            always "frontiers" => self.frontiers,
            always "id" => self.id,
            some "initial_overlap" => self.initial_overlap.map(InitialOverlap),
            some "local_classification_discharged" => self.local_classification_discharged,
            always "local_inspection_finished" => self.local_inspection_finished,
            always "lower" => domain.lower,
            some "native_inspection_scope" => self.native_inspection_scope,
            some "optional_refusal_provenance_scope" => self.optional_refusal_provenance_scope,
            some "optional_refusal_provenance_truncated" =>
                self.optional_refusal_provenance_truncated,
            some "optional_refusals" => self.optional_refusals.as_ref(),
            always "owner" => Mask(&domain.owner),
            always "phase" => PhaseName(domain.phase),
            some "physical_inspections" => self.physical_inspections,
            some "physical_parts" => self.physical_parts.as_ref(),
            some "physical_parts_expected" => self.physical_parts_expected,
            some "physical_seconds_scope" => self.physical_seconds_scope,
            some "physical_seconds_sum" => self.physical_seconds_sum,
            always "power_bounds" => PowerBounds(domain.powers),
            always "rank" => domain.rank,
            some "record_kind" => self.record_kind,
            some "residual_inspection_finished" => self.residual_inspection_finished,
            always "seconds" => self.seconds,
            always "stats" => Stats(self.stats),
            some "stats_scope" => self.stats_scope,
            some "unreturned_physical_parts" => self.unreturned_physical_parts.as_ref(),
            always "upper" => domain.upper,
        })
    }
}

impl<const N: usize> Line for NativeRecord<N> {
    /// The retained arrays move into the `Value` instead of being copied.
    fn into_value(mut self) -> Result<Value, String> {
        let frontiers = std::mem::take(&mut self.frontiers);
        let refusals = self.optional_refusals.as_mut().map(std::mem::take);
        let parts = self.physical_parts.as_mut().map(std::mem::take);
        let mut value = serde_json::to_value(&self).map_err(|e| e.to_string())?;
        value["frontiers"] = Value::Array(frontiers);
        if let Some(refusals) = refusals {
            value["optional_refusals"] = Value::Array(refusals);
        }
        if let Some(parts) = parts {
            value["physical_parts"] = Value::Array(parts);
        }
        Ok(value)
    }
}

/// The record of one delegated (transferred) publication.
pub(in super::super::super) struct DelegatedRecord<const N: usize> {
    pub id: usize,
    pub domain: Domain<N>,
    pub representative: usize,
}

impl<const N: usize> Serialize for DelegatedRecord<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let domain = &self.domain;
        sorted_object!(serializer, {
            always "containment_authority" => "same_snapshot_phase_owner_native_summary",
            always "id" => self.id,
            always "local_inspection_finished" => false,
            always "lower" => domain.lower,
            always "owner" => Mask(&domain.owner),
            always "phase" => PhaseName(domain.phase),
            always "power_bounds" => PowerBounds(domain.powers),
            always "rank" => domain.rank,
            always "record_kind" => "delegated_not_inspected",
            always "representative_id" => self.representative,
            always "responsibility_status" => "pending",
            always "upper" => domain.upper,
        })
    }
}

impl<const N: usize> Line for DelegatedRecord<N> {
    fn into_value(self) -> Result<Value, String> {
        serde_json::to_value(&self).map_err(|e| e.to_string())
    }
}

/// The former `json!` records, kept verbatim as the reference of the
/// differential checks.
#[cfg(test)]
pub(in super::super::super) mod legacy {
    use super::super::super::{mask, power_bounds_json, route_stats, stats_json};
    use super::*;
    use serde_json::json;

    pub fn native<const N: usize>(input: NativeInputs<N>) -> Value {
        let NativeInputs {
            id,
            domain,
            error,
            error_free,
            stats: native_stats,
            seconds,
            accepted_events,
            frontiers,
            delegation,
            refusals,
            physical_parts,
        } = input;
        let partial_scope = match native_stats {
            NativeStats::ApplyPartial(_, scope) => Some(scope),
            _ => None,
        };
        let (stats, optional, truncated) = match native_stats {
            NativeStats::Apply(stats) | NativeStats::ApplyPartial(stats, _) => {
                (stats_json(stats), refusals, Some(stats))
            }
            NativeStats::Route(stats) => (route_stats(stats), None, None),
        };
        let frontier_count = frontiers.len();
        let mut record = json!({"id":id, "phase":format!("{:?}", domain.phase), "owner":mask(&domain.owner),
            "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
            "power_bounds":power_bounds_json(domain.powers),
            "local_inspection_finished":error.is_none(), "stats":stats, "seconds":seconds,
            "error":error});
        if let Some(accepted) = accepted_events {
            record["accepted_events"] = json!(accepted);
        }
        record["frontiers"] = Value::Array(frontiers);
        if delegation {
            record["record_kind"] = json!("native_inspection");
            record["local_classification_discharged"] = json!(error_free && frontier_count == 0);
        }
        if let Some(scope) = partial_scope {
            record["record_kind"] = json!("partial_initial_overlap_inspection");
            record["native_inspection_scope"] = json!("low_D_residual_only");
            record["local_inspection_finished"] = json!(false);
            record["residual_inspection_finished"] = json!(error_free);
            record["local_classification_discharged"] = json!(false);
            record["initial_overlap"] = json!({"anchor_id":scope.anchor_id,"cut":scope.cut,
                "covered_slice":"original_intersect_D_ge_cut",
                "residual_power_bounds":power_bounds_json(scope.residual_powers),
                "coordinates_and_rank_unchanged":true,
                "authority":"same_snapshot_phase_owner_native_summary"});
        }
        if let (Some(optional), Some(stats)) = (optional, truncated) {
            record["optional_refusal_provenance_truncated"] = json!(optional.truncated(stats));
            record["optional_refusal_provenance_scope"] = json!("first_per_phase_per_query");
            record["optional_refusals"] = Value::Array(optional.records);
        } else {
            record["conservative_route_overcover"] = json!(true);
        }
        if let Some(parts) = physical_parts {
            record["record_kind"] = json!("subdivided_native_inspection");
            record["native_inspection_scope"] = json!("disjoint_exact_source_partition");
            record["stats_scope"] = json!("checked_sum_of_actual_physical_calls");
            record["physical_inspections"] = json!(parts.len());
            record["physical_parts_expected"] = json!(2);
            record["unreturned_physical_parts"] = json!(
                (0..2)
                    .filter(|part| !parts.iter().any(|p| p["part"] == *part))
                    .collect::<Vec<_>>()
            );
            record["physical_seconds_sum"] = json!(seconds);
            record["physical_seconds_scope"] =
                json!("sum_of_physical_call_wall_seconds; not_parent_wall_or_CPU");
            record["seconds"] = Value::Null;
            record["optional_refusal_provenance_scope"] =
                json!("first_per_phase_per_physical_part");
            record["optional_refusals"] = json!(
                parts
                    .iter()
                    .flat_map(|p| p["optional_refusals"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .cloned())
                    .collect::<Vec<_>>()
            );
            record["optional_refusal_provenance_truncated"] = json!(
                parts
                    .iter()
                    .any(|p| p["optional_refusal_provenance_truncated"] == true)
            );
            record["physical_parts"] = Value::Array(parts);
        }
        record
    }

    pub fn delegated<const N: usize>(record: &DelegatedRecord<N>) -> Value {
        let (id, domain, to) = (record.id, &record.domain, record.representative);
        json!({"record_kind":"delegated_not_inspected", "id":id,
            "phase":format!("{:?}", domain.phase), "owner":mask(&domain.owner),
            "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
            "power_bounds":power_bounds_json(domain.powers),
            "representative_id":to, "local_inspection_finished":false,
            "responsibility_status":"pending",
            "containment_authority":"same_snapshot_phase_owner_native_summary"})
    }

    /// Record variants the differential checks have seen on this thread (a
    /// walk's coordinator is the thread that runs it).
    pub mod seen {
        use std::cell::Cell;
        pub const DELEGATED: u32 = 1 << 0;
        pub const NATIVE: u32 = 1 << 1;
        pub const ROUTE: u32 = 1 << 2;
        pub const PARTIAL: u32 = 1 << 3;
        pub const SUBDIVIDED: u32 = 1 << 4;
        pub const FRONTIERS: u32 = 1 << 5;
        pub const REFUSALS: u32 = 1 << 6;
        pub const ACCEPTED_EVENTS: u32 = 1 << 7;
        pub const ERROR: u32 = 1 << 8;
        pub const DELEGATION_LEDGER: u32 = 1 << 9;
        thread_local!(static SEEN: Cell<u32> = const { Cell::new(0) });
        pub fn add(flags: u32) {
            SEEN.with(|seen| seen.set(seen.get() | flags));
        }
        /// The flags seen on this thread since the previous call.
        pub fn take() -> u32 {
            SEEN.with(|seen| seen.replace(0))
        }
    }

    fn same(line: &impl Line, reference: &Value, what: &str) {
        let bytes = serde_json::to_vec(line).expect("typed record serializes");
        let expected = serde_json::to_vec(reference).expect("reference record serializes");
        assert_eq!(
            String::from_utf8_lossy(&bytes),
            String::from_utf8_lossy(&expected),
            "{what} record line differs from the former json! record"
        );
    }

    /// Every test-build native publication: the typed line and `Value` must
    /// equal the former `json!` record's.
    pub fn check_native<const N: usize>(input: &NativeInputs<N>) {
        let reference = native(input.clone());
        let record = super::native(input.clone());
        same(&record, &reference, "native");
        assert_eq!(record.into_value().unwrap(), reference);
        let mut flags = 0;
        let applied = !matches!(input.stats, NativeStats::Route(_));
        flags |= if applied { seen::NATIVE } else { seen::ROUTE };
        if matches!(input.stats, NativeStats::ApplyPartial(..)) {
            flags |= seen::PARTIAL;
        }
        if input.physical_parts.is_some() {
            flags |= seen::SUBDIVIDED;
        }
        if !input.frontiers.is_empty() {
            flags |= seen::FRONTIERS;
        }
        if input
            .refusals
            .as_ref()
            .is_some_and(|r| !r.records.is_empty())
            || input.physical_parts.as_ref().is_some_and(|parts| {
                parts.iter().any(|p| {
                    p["optional_refusals"]
                        .as_array()
                        .is_some_and(|r| !r.is_empty())
                })
            })
        {
            flags |= seen::REFUSALS;
        }
        if input.accepted_events.is_some() {
            flags |= seen::ACCEPTED_EVENTS;
        }
        if input.error.is_some() || !input.error_free {
            flags |= seen::ERROR;
        }
        if input.delegation {
            flags |= seen::DELEGATION_LEDGER;
        }
        seen::add(flags);
    }
    pub fn check_delegated<const N: usize>(record: &DelegatedRecord<N>) {
        let reference = delegated(record);
        same(record, &reference, "delegated");
        let value = serde_json::to_value(record).unwrap();
        assert_eq!(value, reference);
        seen::add(seen::DELEGATED);
    }
}

#[cfg(test)]
mod tests;
