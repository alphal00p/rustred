//! Presentation of optional-index admission, never coverage authority.
use serde_json::{Value, json};

use super::initial_overlap::{InitialOverlapBuildReport, MAX_ENTRY_BYTES, MAX_INITIAL_DOMAINS};

#[derive(Clone, Copy)]
pub(super) enum Scope {
    GlobalInitial,
    BucketLocal,
}

pub(super) fn render(report: Option<InitialOverlapBuildReport>, scope: Scope) -> Value {
    json!({
        "status":report.map_or("not_built", |r| r.status.as_str()),
        "identity_scope":match scope {
            Scope::GlobalInitial => "global_initial_domain_id",
            Scope::BucketLocal => "bucket_local_initial_domain_id",
        },
        "total_initial":report.map(|r|r.total_initial),
        "examined_initial":report.map(|r|r.examined_initial),
        "eligible_apply":report.map(|r|r.eligible_apply),
        "eligibility_complete":report.map(|r|r.eligibility_complete),
        "retained_membership":report.map(|r|r.retained_membership),
        "usable_anchors":report.map(|r|r.usable_anchors),
        "logical_bytes_per_eligible_entry":report.map(|r|r.logical_bytes_per_eligible_entry),
        "requested_logical_entry_bytes":report.and_then(|r|r.requested_logical_entry_bytes),
        "limits":{"max_initial_apply_domains":MAX_INITIAL_DOMAINS,
            "max_logical_entry_bytes":MAX_ENTRY_BYTES,"scope":"per_index"},
        "logical_charge_is_not_retained_allocation_or_rss":true,
        "route_entries_remain_in_queue_and_ledger":true,
        "coverage_authority":false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::routed_campaign::walking::initial_overlap::InitialOverlapBuildStatus;

    #[test]
    fn absent_index_is_not_reported_as_success_or_complete_eligibility() {
        let value = render(None, Scope::GlobalInitial);
        assert_eq!(value["status"], "not_built");
        assert!(value["eligible_apply"].is_null());
        assert!(value["eligibility_complete"].is_null());
        assert_eq!(value["coverage_authority"], false);
    }

    #[test]
    fn cancelled_prefix_and_bucket_local_scope_remain_explicit() {
        let report = InitialOverlapBuildReport {
            total_initial: 12,
            examined_initial: 3,
            eligible_apply: 1,
            eligibility_complete: false,
            retained_membership: 0,
            usable_anchors: 0,
            logical_bytes_per_eligible_entry: 640,
            requested_logical_entry_bytes: None,
            status: InitialOverlapBuildStatus::Cancelled,
        };
        let value = render(Some(report), Scope::BucketLocal);
        assert_eq!(value["status"], "cancelled");
        assert_eq!(value["examined_initial"], 3);
        assert_eq!(value["eligible_apply"], 1);
        assert_eq!(value["eligibility_complete"], false);
        assert!(value["requested_logical_entry_bytes"].is_null());
        assert_eq!(value["identity_scope"], "bucket_local_initial_domain_id");
        assert_eq!(value["limits"]["scope"], "per_index");
        assert_eq!(value["coverage_authority"], false);
    }
}
