use super::{
    ledger::{Ledger, Local, Responsibility},
    types::{Error, NativeOutcome, Resolution, ResolutionReport, ResolutionStatus, Summary},
};

impl<K: Copy + Eq> Ledger<K> {
    /// Resolve direct provenance links in reverse ID order. Native rows, CAS and
    /// domain geometry are not touched. Allocation remains bounded by the
    /// admitted-domain allowance; allocation failure returns a typed error.
    pub fn resolve(&self) -> Result<ResolutionReport, Error> {
        let mut by_id = Vec::new();
        by_id
            .try_reserve_exact(self.entries.len())
            .map_err(|_| Error::Allocation)?;
        by_id.resize(
            self.entries.len(),
            Resolution {
                representative: 0,
                status: ResolutionStatus::Pending,
                delegated: false,
            },
        );
        let mut depth: Vec<usize> = Vec::new();
        depth
            .try_reserve_exact(self.entries.len())
            .map_err(|_| Error::Allocation)?;
        depth.resize(self.entries.len(), 0);
        let mut summary = Summary {
            admitted: self.entries.len(),
            logical_publications: self.cursor,
            native_publications: self.native_publications,
            delegated: self.transfers,
            delegated_publications: self.delegated_publications,
            ..Summary::default()
        };
        for id in (0..self.entries.len()).rev() {
            let entry = &self.entries[id];
            by_id[id] = match entry.responsibility {
                Responsibility::Delegate { to } => {
                    if to <= id || to >= self.entries.len() {
                        return Err(Error::InvalidForwardEdge);
                    }
                    if entry.key != self.entries[to].key {
                        return Err(Error::IdentityMismatch);
                    }
                    // Depth cannot exceed the number of admitted forward IDs.
                    depth[id] = depth[to] + 1;
                    summary.maximum_alias_depth = summary.maximum_alias_depth.max(depth[id]);
                    let target = by_id[to];
                    match target.status {
                        ResolutionStatus::Pending => summary.delegated_pending += 1,
                        ResolutionStatus::Discharged => summary.delegated_resolved += 1,
                        ResolutionStatus::UnresolvedFrontiers { .. } => {
                            summary.delegated_frontier_blocked += 1
                        }
                        ResolutionStatus::Failed => summary.delegated_failure_blocked += 1,
                        ResolutionStatus::Cancelled => summary.delegated_cancelled += 1,
                    }
                    Resolution {
                        delegated: true,
                        ..target
                    }
                }
                Responsibility::Local(local) => {
                    let status = match local {
                        Local::Published(NativeOutcome::Completed {
                            unresolved_frontiers: 0,
                        }) => {
                            summary.native_discharged += 1;
                            ResolutionStatus::Discharged
                        }
                        Local::Published(NativeOutcome::Completed {
                            unresolved_frontiers,
                        }) => {
                            summary.native_frontier_blocked += 1;
                            ResolutionStatus::UnresolvedFrontiers {
                                count: unresolved_frontiers,
                            }
                        }
                        Local::Published(NativeOutcome::Failed) => {
                            summary.native_failed += 1;
                            ResolutionStatus::Failed
                        }
                        Local::Published(NativeOutcome::Cancelled) => {
                            summary.native_cancelled += 1;
                            ResolutionStatus::Cancelled
                        }
                        _ => {
                            summary.native_pending += 1;
                            ResolutionStatus::Pending
                        }
                    };
                    Resolution {
                        representative: id,
                        status,
                        delegated: false,
                    }
                }
            };
        }
        Ok(ResolutionReport { by_id, summary })
    }
}
