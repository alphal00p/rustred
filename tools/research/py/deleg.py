import re,sys
p='execution/delegation.rs'
s=open(p).read()
s=s.replace('''use super::super::delegation::{ResolutionStatus, SchedulingPolicy};''','''use super::super::delegation::{Resolution, ResolutionStatus, SchedulingPolicy};''')
s=s.replace('''        let domain = &self.queue.domains[id];
        self.records
            .try_reserve(1)
            .map_err(|_| "delegation record allocation")?;''','''        let domain = &self.queue.domains[id];
        self.records
            .get_mut()
            .reserve_one()
            .map_err(|_| "delegation record allocation")?;''')
s=s.replace('''            closure.finish(id, false, true);
        }
        self.records.push(record);''','''            closure.finish(id, false, true);
        }
        self.records.get_mut().push(record)?;''')
old_start=s.index('    /// One final O(N) resolution, never a hot progress/event scan.')
old_end=s.index('        let s = &report.summary;')
new='''    /// One final O(N) resolution, never a hot progress/event scan. Records
    /// are not touched: the per-ID resolutions are returned (only when the
    /// ledger resolved at the final cursor) and applied to each record as it
    /// is streamed into the report (`annotate_resolution`).
    pub(crate) fn finalize_delegation(&mut self) -> (Option<Value>, Option<Vec<Resolution>>) {
        let Some(ledger) = self.queue.delegation.as_ref() else {
            return (None, None);
        };
        let report = match ledger.resolve() {
            Ok(report) if ledger.cursor() == self.queue.next => report,
            Ok(_) => {
                self.error
                    .get_or_insert_with(|| "delegation final cursor mismatch".into());
                return (
                    Some(json!({"all_ledger_obligations_discharged":false,
                    "error":"delegation final cursor mismatch"})),
                    None,
                );
            }
            Err(error) => {
                let error = error.to_string();
                self.error.get_or_insert_with(|| error.clone());
                return (
                    Some(json!({"all_ledger_obligations_discharged":false, "error":error})),
                    None,
                );
            }
        };
'''
s=s[:old_start]+new+s[old_end:]
s=s.replace('''            "resolution_scope":"local_obligations_including_partial_anchor_dependencies; not_unique_native_failure_or_frontier_sources; global_frontiers_and_errors_are_separate"
        }))
    }
}''','''            "resolution_scope":"local_obligations_including_partial_anchor_dependencies; not_unique_native_failure_or_frontier_sources; global_frontiers_and_errors_are_separate"
        }));
        (summary, Some(report.by_id))
    }

    /// Test seam: the summary plus a read-back of the records with the ledger
    /// annotations applied, as the former in-place finalization left them.
    #[cfg(test)]
    pub(crate) fn finalized_records(&mut self) -> (Option<Value>, Vec<Value>) {
        let (summary, resolutions) = self.finalize_delegation();
        let mut records = self.records.borrow().snapshot();
        if let Some(by_id) = &resolutions {
            for record in &mut records {
                annotate_resolution(record, by_id);
            }
        }
        (summary, records)
    }
}

/// Ledger annotations of one delegated or partial record, with the exact
/// strings of the former in-place mutation; every other record is untouched.
pub(super) fn annotate_resolution(record: &mut Value, by_id: &[Resolution]) {
    let partial = record["record_kind"] == "partial_initial_overlap_inspection";
    if record["record_kind"] != "delegated_not_inspected" && !partial {
        return;
    }
    let Some(&resolution) = record["id"]
        .as_u64()
        .and_then(|id| usize::try_from(id).ok())
        .and_then(|id| by_id.get(id))
    else {
        return;
    };
    if partial {
        record["local_classification_discharged"] =
            json!(resolution.status == ResolutionStatus::Discharged);
        record["responsibility_status"] = match resolution.status {
            ResolutionStatus::Pending => json!("pending_residual_or_initial_anchor"),
            ResolutionStatus::Discharged => json!("discharged_by_residual_and_initial_anchor"),
            ResolutionStatus::UnresolvedFrontiers { count } => {
                json!({"blocked_by_residual_or_initial_anchor_frontiers":count})
            }
            ResolutionStatus::Failed => json!("blocked_by_residual_or_initial_anchor_failure"),
            ResolutionStatus::Cancelled => {
                json!("blocked_by_residual_or_initial_anchor_cancellation")
            }
        };
        return;
    }
    record["final_representative_id"] = json!(resolution.representative);
    record["responsibility_status"] = match resolution.status {
        ResolutionStatus::Pending => json!("pending"),
        ResolutionStatus::Discharged => json!("discharged_by_representative"),
        ResolutionStatus::UnresolvedFrontiers { count } => {
            json!({"blocked_by_representative_frontiers":count})
        }
        ResolutionStatus::Failed => json!("blocked_by_representative_failure"),
        ResolutionStatus::Cancelled => json!("blocked_by_representative_cancellation"),
    };
}''')
s=s.replace('''        let s = &report.summary;
        Some(json!({''','''        let s = &report.summary;
        let summary = Some(json!({''')
open(p,'w').write(s)
