//! Typed records of Epoch semantics4 and their diagnostic JSON projection.
//! Binary authority is separated from optional diagnostic payloads; existing
//! audit/oracle row names remain available at the explicit projection boundary.
//! Records are appended in merge order; the finalization annotations
//! (`final_representative_id`, `responsibility_status`,
//! `local_classification_discharged` of partial records,
//! `descendant_closed`) are applied when the report is written, exactly as
//! for the legacy lanes.
use super::super::delegation::{Resolution, ResolutionStatus};
use super::super::queue::CompactDomain;
#[cfg(test)]
use super::super::{mask, power_bounds_json, queue::Phase};
#[cfg(test)]
use super::anchors::{AnchorScope, Lent};
use super::ledger6::Entry6;
#[cfg(test)]
use super::merge::Class;
use super::merge::{CheckedResult, RecordBuilder};
use super::state::EpochState;
use serde_json::{Value, json};

pub(in super::super) mod typed;
pub(in super::super) mod wire;

/// The exact resolver-side counters P3 adds, not native stats that may have
/// already charged a breaking event which the resolver did not accept.
/// Shared by newly written epoch records and the fresh-only CP6 reader.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
#[serde(deny_unknown_fields)]
pub(in crate::application::routed_campaign::walking) struct ResolverCounters {
    pub version: u32,
    pub successors: u64,
    pub conditional: u64,
    /// Total, original, coalesced, matching JobResult::optional exactly.
    pub optional: [u64; 3],
    pub route_masks: u64,
    pub route_joint_pruned: u64,
}

impl Default for ResolverCounters {
    fn default() -> Self {
        Self {
            version: 1,
            successors: 0,
            conditional: 0,
            optional: [0; 3],
            route_masks: 0,
            route_joint_pruned: 0,
        }
    }
}

impl ResolverCounters {
    pub fn from_result<const N: usize>(result: &super::job::JobResult<N>) -> Self {
        Self {
            version: 1,
            successors: result.successors,
            conditional: result.conditional,
            optional: result.optional,
            route_masks: result.route_masks,
            route_joint_pruned: result.route_joint_pruned,
        }
    }

    /// Restore-only aggregate. Compute the complete small sum before mutation;
    /// a malformed version or overflow leaves the accumulator unchanged.
    pub fn add(&mut self, next: &Self) -> Result<(), &'static str> {
        if self.version != 1 || next.version != 1 {
            return Err("epoch resolver counter version");
        }
        let sum = |a: u64, b: u64| a.checked_add(b).ok_or("epoch resolver counter overflow");
        let value = Self {
            version: 1,
            successors: sum(self.successors, next.successors)?,
            conditional: sum(self.conditional, next.conditional)?,
            optional: [
                sum(self.optional[0], next.optional[0])?,
                sum(self.optional[1], next.optional[1])?,
                sum(self.optional[2], next.optional[2])?,
            ],
            route_masks: sum(self.route_masks, next.route_masks)?,
            route_joint_pruned: sum(self.route_joint_pruned, next.route_joint_pruned)?,
        };
        *self = value;
        Ok(())
    }

    pub fn agrees_with(&self, counters: &super::state::WalkCounters) -> bool {
        self.version == 1
            && self.successors == counters.successors
            && self.conditional == counters.conditional
            && self.optional
                == [
                    counters.optional_total,
                    counters.optional_original,
                    counters.optional_coalesced,
                ]
            && self.route_masks == counters.route_masks
            && self.route_joint_pruned == counters.route_joint_pruned
    }
}

pub(super) const CONTAINMENT_AUTHORITY: &str = "same_snapshot_phase_owner_native_summary";

#[cfg(test)]
fn geometry<const N: usize>(image: &CompactDomain<N>) -> Value {
    let domain = image.expand();
    json!({"phase":format!("{:?}", domain.phase),"owner":mask(&domain.owner),
        "lower":domain.lower,"upper":domain.upper,"rank":domain.rank,
        "power_bounds":power_bounds_json(domain.powers)})
}

#[cfg(test)]
fn parse(bytes: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(bytes).map_err(|e| format!("result part is not JSON: {e}"))
}

pub(super) struct Builder;

impl<const N: usize> RecordBuilder<N> for Builder {
    fn native(
        &self,
        id: u32,
        image: &CompactDomain<N>,
        entry: &CheckedResult<N>,
        merge_epoch: u64,
        distinct_edges: u32,
        self_edge: bool,
    ) -> Result<typed::Record, String> {
        typed::Record::native(id, image, entry, merge_epoch, distinct_edges, self_edge)
    }
    fn alias(
        &self,
        id: u32,
        image: &CompactDomain<N>,
        to: u32,
        merge_epoch: u64,
        exhausted: bool,
    ) -> typed::Record {
        typed::Record::alias(id, image, to, merge_epoch, exhausted)
    }
}

/// Frozen pre-S5 JSON projection, retained only as an independent test oracle.
#[cfg(test)]
pub(super) struct JsonReference;

#[cfg(test)]
impl JsonReference {
    pub fn native<const N: usize>(
        &self,
        id: u32,
        image: &CompactDomain<N>,
        entry: &CheckedResult<N>,
        merge_epoch: u64,
        distinct_edges: u32,
        self_edge: bool,
    ) -> Result<Value, String> {
        let r = &entry.result;
        let abandoned = r.kind == super::job::NativeKind::Abandoned;
        let finished = matches!(entry.class, Class::C0 | Class::C4) && !abandoned;
        let error = if finished || abandoned {
            None
        } else {
            let detail = r.error.clone().unwrap_or_else(|| "native error".into());
            Some(match r.break_reason.name() {
                "none" => detail,
                reason => format!("{reason}: {detail}"),
            })
        };
        let mut record = geometry(image);
        record["id"] = json!(id);
        record["local_inspection_finished"] = json!(finished);
        record["stats"] = if r.stats_json.is_empty() {
            Value::Null
        } else {
            parse(&r.stats_json)?
        };
        record["seconds"] = json!(r.seconds);
        record["error"] = json!(error);
        record["accepted_events"] = json!(r.accepted);
        record["frontiers"] = Value::Array(
            r.frontiers
                .iter()
                .map(|bytes| parse(bytes))
                .collect::<Result<_, _>>()?,
        );
        record["record_kind"] = json!("native_inspection");
        if abandoned {
            record["rescue_abandoned"] = json!(true);
        }
        record["local_classification_discharged"] =
            json!(entry.class == Class::C0 && entry.anchors.is_none());
        if image.phase() == Phase::Apply {
            record["optional_refusal_provenance_truncated"] = json!(r.refusals_truncated);
            record["optional_refusal_provenance_scope"] = json!("first_per_phase_per_query");
            record["optional_refusals"] = Value::Array(
                r.refusals
                    .iter()
                    .map(|bytes| parse(bytes))
                    .collect::<Result<_, _>>()?,
            );
        } else {
            record["conservative_route_overcover"] = json!(true);
        }
        if let Some(anchors) = entry.anchors.as_ref().filter(|a| a.record.kind.is_g2()) {
            // A G2' residual record (W4 planner; tests only in S2).
            let pieces = match &anchors.record.scope {
                AnchorScope::Residual(pieces) => pieces
                    .iter()
                    .map(|p| json!({"d_lo":p.d_lo,"d_hi":p.d_hi,"lower":p.lower,"upper":p.upper}))
                    .collect::<Vec<_>>(),
                AnchorScope::DBandCut(_) => Vec::new(),
            };
            record["record_kind"] = json!("g2_residual_inspection");
            record["native_inspection_scope"] = json!("g2_residual_only");
            record["local_inspection_finished"] = json!(false);
            record["residual_inspection_finished"] = json!(finished);
            record["local_classification_discharged"] = json!(false);
            record["g2"] = json!({"kind":anchors.record.kind.name(),
                "dispatch_version":anchors.record.dispatch_version,
                "anchors":anchors.record.anchors.iter().map(|a| json!({"id":a.anchor,
                    "stamp":a.stamp,"lent":match a.lent {Lent::Full => "domain",
                    Lent::LowSlice => "inspected_low_D_slice"}})).collect::<Vec<_>>(),
                "residual":pieces,"authority":"exact_union_cover_lattice"});
        }
        if let (Some((anchor, cut)), Some(scope)) = (entry.d_band(), r.scope) {
            record["record_kind"] = json!("partial_initial_overlap_inspection");
            record["native_inspection_scope"] = json!("low_D_residual_only");
            record["local_inspection_finished"] = json!(false);
            record["residual_inspection_finished"] = json!(finished);
            record["local_classification_discharged"] = json!(false);
            record["initial_overlap"] = json!({"anchor_id":anchor,"cut":cut,
                "covered_slice":"original_intersect_D_ge_cut",
                "residual_power_bounds":power_bounds_json(scope.residual),
                "coordinates_and_rank_unchanged":true,
                "authority":CONTAINMENT_AUTHORITY});
        }
        record["epoch"] = json!({"merge_epoch":merge_epoch,"v0":r.v0,"refresh_points":[],
            "distinct_edge_count":distinct_edges,"self_edge":self_edge,
            "class":entry.class.name(),"break_reason":r.break_reason.name(),
            "panic":r.panic,"emitted_events":r.emitted,"error_kind":r.error_kind.name(),
            "job_local_duplicates":r.job_duplicates,"known_reuse":r.known_reuse,
            "resolver_counters":ResolverCounters::from_result(r)});
        if entry.class == Class::C2 {
            // The ledger6 NativeError class of this record (`err_class`).
            record["epoch"]["err_class"] = json!(super::ledger6::err_class::name(
                super::merge::error_class(entry)
            ));
        }
        Ok(record)
    }

    pub fn alias<const N: usize>(
        &self,
        id: u32,
        image: &CompactDomain<N>,
        to: u32,
        merge_epoch: u64,
        exhausted: bool,
    ) -> Value {
        let mut record = geometry(image);
        record["record_kind"] = json!("delegated_not_inspected");
        record["id"] = json!(id);
        record["representative_id"] = json!(to);
        record["local_inspection_finished"] = json!(false);
        record["responsibility_status"] = json!("pending");
        record["containment_authority"] = json!(CONTAINMENT_AUTHORITY);
        record["epoch"] = json!({"merge_epoch":merge_epoch,
            "transition":if exhausted {"T10"} else {"T3"},
            "authority":"verify_chokepoint: explicit phase and owner, raw inclusion or native summary on canonical images"});
        record
    }
}

/// Final per-ID resolutions (the legacy `Ledger::resolve` rules on ledger6)
/// and the delegation summary the audit reads.
pub(super) fn resolve<const N: usize>(state: &EpochState<N>) -> (Vec<Resolution>, Value) {
    #[cfg(test)]
    super::assert_large_finalization_allowed();
    let total = state.store.len();
    let mut by_id = vec![
        Resolution {
            representative: 0,
            status: ResolutionStatus::Pending,
            delegated: false,
        };
        total
    ];
    let mut depth = vec![0usize; total];
    let mut s = Summary::default();
    let own = |id: u32| -> ResolutionStatus {
        match state.ledger.get(id).expect("valid ledger entry") {
            Entry6::Native { .. } => ResolutionStatus::Discharged,
            Entry6::NativeFrontier { .. } => ResolutionStatus::UnresolvedFrontiers {
                count: state.frontier_counts.get(&id).copied().unwrap_or(0) as usize,
            },
            Entry6::NativeError { .. } => ResolutionStatus::Failed,
            _ => ResolutionStatus::Pending,
        }
    };
    // Anchored nodes: a node is discharged only if it and EVERY anchor are
    // (an anchor's own anchors included). Resolved in ascending merge epoch:
    // G2' anchors merged strictly earlier; an InitialDBand anchor (< P0) is
    // anchor-free, so its own status is final.
    let epoch_of = |id: u32| match state.ledger.get(id) {
        Ok(Entry6::Native { epoch, .. } | Entry6::NativeFrontier { epoch })
        | Ok(Entry6::NativeError { epoch, .. }) => epoch,
        _ => u64::MAX,
    };
    let mut anchored: Vec<&super::anchors::AnchorRecord> = state.anchors.records().iter().collect();
    anchored.sort_by_key(|record| (epoch_of(record.node), record.node));
    let mut anchored_status: std::collections::HashMap<u32, ResolutionStatus> =
        std::collections::HashMap::new();
    for record in anchored {
        let mut status = own(record.node);
        if status == ResolutionStatus::Discharged {
            for a in &record.anchors {
                let anchor = anchored_status
                    .get(&a.anchor)
                    .copied()
                    .unwrap_or_else(|| own(a.anchor));
                if anchor != ResolutionStatus::Discharged {
                    status = anchor;
                    break;
                }
            }
        }
        if record.kind.is_g2() {
            s.g2_records += 1;
            s.g2_blocked += usize::from(status != ResolutionStatus::Discharged);
        } else {
            s.partial_initial_inspections += 1;
            s.partial_initial_blocked += usize::from(status != ResolutionStatus::Discharged);
        }
        anchored_status.insert(record.node, status);
    }
    for id in (0..total as u32).rev() {
        let entry = state.ledger.get(id).expect("valid ledger entry");
        by_id[id as usize] = match entry {
            Entry6::Alias { to } => {
                depth[id as usize] = depth[to as usize] + 1;
                s.maximum_alias_depth = s.maximum_alias_depth.max(depth[id as usize]);
                let target = by_id[to as usize];
                match target.status {
                    ResolutionStatus::Pending => s.delegated_pending += 1,
                    ResolutionStatus::Discharged => s.delegated_resolved += 1,
                    ResolutionStatus::UnresolvedFrontiers { .. } => {
                        s.delegated_frontier_blocked += 1
                    }
                    ResolutionStatus::Failed => s.delegated_failure_blocked += 1,
                    ResolutionStatus::Cancelled => s.delegated_cancelled += 1,
                }
                Resolution {
                    delegated: true,
                    ..target
                }
            }
            _ => {
                let status = anchored_status.get(&id).copied().unwrap_or_else(|| own(id));
                match status {
                    ResolutionStatus::Discharged => s.native_discharged += 1,
                    ResolutionStatus::UnresolvedFrontiers { .. } => s.native_frontier_blocked += 1,
                    ResolutionStatus::Failed => s.native_failed += 1,
                    ResolutionStatus::Cancelled => s.native_cancelled += 1,
                    ResolutionStatus::Pending => s.native_pending += 1,
                }
                Resolution {
                    representative: id as usize,
                    status,
                    delegated: false,
                }
            }
        };
    }
    let counts = state.ledger.counts();
    use super::ledger6::Tag;
    let natives =
        counts.get(Tag::Native) + counts.get(Tag::NativeFrontier) + counts.get(Tag::NativeError);
    let aliases = counts.get(Tag::Alias);
    let logical = natives + aliases;
    let all = logical == total as u64
        && s.native_pending == 0
        && s.native_frontier_blocked == 0
        && s.native_failed == 0
        && s.native_cancelled == 0
        && s.delegated_pending == 0
        && s.delegated_frontier_blocked == 0
        && s.delegated_failure_blocked == 0
        && s.delegated_cancelled == 0;
    // `native_pending` counts unmerged IDs as the legacy summary counts
    // unpublished locals; the audit requires it (and every blocker) be 0.
    let summary = json!({
        "all_ledger_obligations_discharged":all,
        "transferred_obligations":state.counters.transfers,"logical_publications":logical,
        "native_publications":natives,"native_discharged":s.native_discharged,
        "native_frontier_blocked":s.native_frontier_blocked,
        "native_failed":s.native_failed,"native_cancelled":s.native_cancelled,
        "pending_native_publications":s.native_pending,
        "delegated_publications":aliases,"delegated_resolved":s.delegated_resolved,
        "delegated_pending":s.delegated_pending,
        "delegated_frontier_blocked":s.delegated_frontier_blocked,
        "delegated_failure_blocked":s.delegated_failure_blocked,
        "delegated_cancelled":s.delegated_cancelled,
        "maximum_alias_depth":s.maximum_alias_depth,
        "partial_initial_inspections":s.partial_initial_inspections,
        "partial_initial_blocked":s.partial_initial_blocked,
        "g2_residual_records":s.g2_records,"g2_residual_blocked":s.g2_blocked,
        "ledger6":counts.json(),
        "resolution_scope":"ledger6 tags (semantics 3); local obligations including partial anchor dependencies; global frontiers and errors are separate"
    });
    (by_id, summary)
}

#[derive(Default)]
struct Summary {
    native_discharged: usize,
    native_frontier_blocked: usize,
    native_failed: usize,
    native_cancelled: usize,
    native_pending: usize,
    delegated_resolved: usize,
    delegated_pending: usize,
    delegated_frontier_blocked: usize,
    delegated_failure_blocked: usize,
    delegated_cancelled: usize,
    maximum_alias_depth: usize,
    partial_initial_inspections: usize,
    partial_initial_blocked: usize,
    g2_records: usize,
    g2_blocked: usize,
}
