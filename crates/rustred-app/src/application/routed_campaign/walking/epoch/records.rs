//! Records of semantics 3, JSON view (typed binary records land in S3).
//! The keys the audit, the verifier and the legacy tooling read are kept
//! (§11.8 audit key mapping); the epoch keys (merge epoch, v0, distinct edge
//! count, class, break reason, panic) ride in a nested `epoch` object.
//! Records are appended in merge order; the finalization annotations
//! (`final_representative_id`, `responsibility_status`,
//! `local_classification_discharged` of partial records,
//! `descendant_closed`) are applied when the report is written, exactly as
//! for the legacy lanes.
use super::super::delegation::{Resolution, ResolutionStatus};
use super::super::queue::{CompactDomain, Phase};
use super::super::{mask, power_bounds_json};
use super::ledger6::Entry6;
use super::merge::{CheckedResult, Class, RecordBuilder};
use super::state::EpochState;
use serde_json::{Value, json};

pub(super) const CONTAINMENT_AUTHORITY: &str = "same_snapshot_phase_owner_native_summary";

fn geometry<const N: usize>(image: &CompactDomain<N>) -> Value {
    let domain = image.expand();
    json!({"phase":format!("{:?}", domain.phase),"owner":mask(&domain.owner),
        "lower":domain.lower,"upper":domain.upper,"rank":domain.rank,
        "power_bounds":power_bounds_json(domain.powers)})
}

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
    ) -> Result<Value, String> {
        let r = &entry.result;
        let finished = matches!(entry.class, Class::C0 | Class::C4);
        let error = if finished {
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
        record["local_classification_discharged"] =
            json!(entry.class == Class::C0 && entry.dband.is_none());
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
        if let (Some((anchor, cut)), Some(scope)) = (entry.dband, r.scope) {
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
            "job_local_duplicates":r.job_duplicates,"known_reuse":r.known_reuse});
        Ok(record)
    }

    fn alias(
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
    let anchor_of: std::collections::HashMap<u32, u32> = state
        .anchors
        .records()
        .iter()
        .filter_map(|record| {
            record
                .anchors
                .first()
                .map(|&(anchor, _)| (record.node, anchor))
        })
        .collect();
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
                let mut status = own(id);
                if let Some(&anchor) = anchor_of.get(&id) {
                    s.partial_initial_inspections += 1;
                    if status == ResolutionStatus::Discharged {
                        status = own(anchor);
                    }
                    if status != ResolutionStatus::Discharged {
                        s.partial_initial_blocked += 1;
                    }
                }
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
}
