//! Reporting and publisher glue for the optional responsibility ledger.
use super::super::delegation::{ResolutionStatus, SchedulingPolicy};
use super::*;

pub(crate) fn scheduling_policy_json(policy: SchedulingPolicy) -> Value {
    match policy {
        SchedulingPolicy::InspectAll => json!({"kind":"inspect_all"}),
        SchedulingPolicy::TransferUnreserved { lookahead } => {
            json!({"kind":"transfer_unreserved", "lookahead":lookahead.get()})
        }
    }
}

impl<const N: usize> State<N> {
    pub(super) fn current_is_delegated(&self) -> bool {
        self.queue
            .delegation
            .as_ref()
            .is_some_and(|ledger| ledger.delegated_to(self.queue.next).is_some())
    }

    pub(super) fn note_native_started(&mut self, id: usize) -> Result<(), String> {
        if let Some(ledger) = &mut self.queue.delegation {
            ledger
                .native_started(id)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Called only in the ordinary serial/parallel loop, NEVER cleanup.
    pub(super) fn commit_delegated(&mut self) -> Result<(), String> {
        self.commit_delegated_id(self.queue.next)
    }
    pub(super) fn commit_delegated_id(&mut self, id: usize) -> Result<(), String> {
        let ready = self.ready();
        if self.error.is_some()
            || (!ready && (!self.details.is_empty() || !self.refusals.records.is_empty()))
        {
            return Err("delegation publication has unresolved publisher state".into());
        }
        let ledger = self
            .queue
            .delegation
            .as_mut()
            .ok_or("delegation policy not enabled")?;
        if !ready && ledger.cursor() != id {
            return Err("delegation publisher cursor mismatch".into());
        }
        let to = ledger
            .delegated_to(id)
            .ok_or("publisher has no delegation")?;
        let domain = &self.queue.domains[id];
        self.records
            .try_reserve(1)
            .map_err(|_| "delegation record allocation")?;
        let record = json!({"record_kind":"delegated_not_inspected", "id":id,
            "phase":format!("{:?}", domain.phase), "owner":mask(&domain.owner),
            "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
            "power_bounds":power_bounds_json(domain.powers),
            "representative_id":to, "local_inspection_finished":false,
            "responsibility_status":"pending",
            "containment_authority":"same_snapshot_phase_owner_native_summary"});
        let publication = ledger
            .publish_delegated(id)
            .map_err(|error| error.to_string())?;
        debug_assert_eq!(publication.native_publications(), 0);
        {
            let mut closure = self.closure.borrow_mut();
            closure.discovered(self.queue.domains.len());
            closure.edge(id, to);
            closure.finish(id, false, true);
        }
        self.records.push(record);
        self.queue.next = ledger.cursor();
        if ready {
            self.streams.initial_published += usize::from(id < self.initial_domain_count);
        }
        Ok(())
    }

    pub(in super::super) fn add_delegation_progress(&self, out: &mut Value) {
        let Some(ledger) = &self.queue.delegation else {
            return;
        };
        out["scheduling_policy"] = scheduling_policy_json(SchedulingPolicy::TransferUnreserved {
            lookahead: ledger.lookahead(),
        });
        out["native_processed_nodes"] = json!(self.native_records);
        if ledger.initial_prefix().is_some() {
            out["reuse_initial_d_bands"] = json!(true);
            out["partial_initial_inspections"] = json!(ledger.partial_initial_inspections());
        }
        out["delegation"] = json!({
            "dispatch_fence":ledger.dispatch_fence(),
            "transferred_obligations":ledger.transfer_count(),
            "delegated_publications":ledger.delegated_publications(),
            "native_publications":ledger.native_publications(),
            "pending_native_publications":ledger.len() - ledger.transfer_count() - ledger.native_publications(),
            "logical_publications":ledger.published_count(),
            "contiguous_publication_watermark":ledger.cursor(),
            "reservation_scan":ledger.reservation_scan(),
            "outstanding_native_parent_credits":ledger.outstanding_native(),
            "resolution_scope":"ledger_local_only; global_frontiers_and_errors_are_separate",
        });
    }

    /// One final O(N) resolution, never a hot progress/event scan.
    pub(crate) fn finalize_delegation(&mut self) -> Option<Value> {
        let ledger = self.queue.delegation.as_ref()?;
        let report = match ledger.resolve() {
            Ok(report) if ledger.cursor() == self.queue.next => report,
            Ok(_) => {
                self.error
                    .get_or_insert_with(|| "delegation final cursor mismatch".into());
                return Some(json!({"all_ledger_obligations_discharged":false,
                    "error":"delegation final cursor mismatch"}));
            }
            Err(error) => {
                let error = error.to_string();
                self.error.get_or_insert_with(|| error.clone());
                return Some(json!({"all_ledger_obligations_discharged":false, "error":error}));
            }
        };
        for record in &mut self.records {
            let partial = record["record_kind"] == "partial_initial_overlap_inspection";
            if record["record_kind"] != "delegated_not_inspected" && !partial {
                continue;
            }
            let id = record["id"]
                .as_u64()
                .and_then(|id| usize::try_from(id).ok())
                .expect("internally constructed delegated ID");
            let resolution = report.by_id[id];
            if partial {
                record["local_classification_discharged"] =
                    json!(resolution.status == ResolutionStatus::Discharged);
                record["responsibility_status"] = match resolution.status {
                    ResolutionStatus::Pending => json!("pending_residual_or_initial_anchor"),
                    ResolutionStatus::Discharged => {
                        json!("discharged_by_residual_and_initial_anchor")
                    }
                    ResolutionStatus::UnresolvedFrontiers { count } => {
                        json!({"blocked_by_residual_or_initial_anchor_frontiers":count})
                    }
                    ResolutionStatus::Failed => {
                        json!("blocked_by_residual_or_initial_anchor_failure")
                    }
                    ResolutionStatus::Cancelled => {
                        json!("blocked_by_residual_or_initial_anchor_cancellation")
                    }
                };
                continue;
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
        }
        let s = &report.summary;
        Some(json!({
            "all_ledger_obligations_discharged":s.all_ledger_obligations_discharged(),
            "transferred_obligations":s.delegated, "logical_publications":s.logical_publications,
            "native_publications":s.native_publications, "native_discharged":s.native_discharged,
            "native_frontier_blocked":s.native_frontier_blocked,
            "native_failed":s.native_failed, "native_cancelled":s.native_cancelled,
            "pending_native_publications":s.native_pending,
            "delegated_publications":s.delegated_publications,
            "delegated_resolved":s.delegated_resolved, "delegated_pending":s.delegated_pending,
            "delegated_frontier_blocked":s.delegated_frontier_blocked,
            "delegated_failure_blocked":s.delegated_failure_blocked,
            "delegated_cancelled":s.delegated_cancelled,
            "maximum_alias_depth":s.maximum_alias_depth,
            "partial_initial_inspections":s.partial_initial_inspections,
            "partial_initial_blocked":s.partial_initial_blocked,
            "resolution_scope":"local_obligations_including_partial_anchor_dependencies; not_unique_native_failure_or_frontier_sources; global_frontiers_and_errors_are_separate"
        }))
    }
}

#[cfg(test)]
mod tests;
