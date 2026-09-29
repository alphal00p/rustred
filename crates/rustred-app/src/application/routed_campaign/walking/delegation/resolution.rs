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
            logical_publications: self.published_count(),
            native_publications: self.native_publications,
            delegated: self.transfers,
            delegated_publications: self.delegated_publications,
            ..Summary::default()
        };
        let g2 = self.g2_statuses()?;
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
                    let mut status = local_status(local);
                    if let Some(resolved) = g2.as_ref().and_then(|g2| g2.status(self, id)) {
                        summary.g2_records += 1;
                        status = resolved;
                        if status != ResolutionStatus::Discharged {
                            summary.g2_blocked += 1;
                        }
                    } else if let Some(anchor) = entry.initial_anchor {
                        summary.partial_initial_inspections += 1;
                        let anchor = anchor.get() - 1;
                        let prefix = self
                            .protected_initial_prefix
                            .ok_or(Error::InvalidInitialAnchor)?;
                        if anchor >= prefix || id < prefix || anchor >= id {
                            return Err(Error::InvalidInitialAnchor);
                        }
                        let source = &self.entries[anchor];
                        let Responsibility::Local(anchor_local) = source.responsibility else {
                            return Err(Error::InvalidInitialAnchor);
                        };
                        if source.key != entry.key || source.initial_anchor.is_some() {
                            return Err(Error::InvalidInitialAnchor);
                        }
                        // Anchors are pinned, bypass partial inspection and have
                        // no backward links. Read their raw published status;
                        // no graph traversal or already-filled by_id is needed.
                        // Preserve the residual's own blocker first. Counts are
                        // source records, never fabricated unique-frontier sums.
                        if status == ResolutionStatus::Discharged {
                            status = local_status(anchor_local);
                        }
                        if status != ResolutionStatus::Discharged {
                            summary.partial_initial_blocked += 1;
                        }
                    }
                    match status {
                        ResolutionStatus::Discharged => {
                            summary.native_discharged += 1;
                        }
                        ResolutionStatus::UnresolvedFrontiers { .. } => {
                            summary.native_frontier_blocked += 1;
                        }
                        ResolutionStatus::Failed => {
                            summary.native_failed += 1;
                        }
                        ResolutionStatus::Cancelled => {
                            summary.native_cancelled += 1;
                        }
                        ResolutionStatus::Pending => {
                            summary.native_pending += 1;
                        }
                    }
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

    /// Resolved statuses of the G2' log's rows, in merge order: a G2' record
    /// is its own residual's status, else the first non-discharged status of
    /// its anchors (older rows, already resolved); an initial-D-band anchor
    /// carries its initial anchor's raw status like the reverse pass does.
    fn g2_statuses(&self) -> Result<Option<G2Statuses>, Error> {
        let Some(log) = self.g2.as_ref() else {
            return Ok(None);
        };
        let mut statuses = Vec::new();
        statuses
            .try_reserve_exact(log.rows.len())
            .map_err(|_| Error::Allocation)?;
        for row in &log.rows {
            let entry = self
                .entries
                .get(row.id as usize)
                .ok_or(Error::InvalidG2Anchor)?;
            let Responsibility::Local(local) = entry.responsibility else {
                return Err(Error::InvalidG2Anchor);
            };
            let mut status = local_status(local);
            if status == ResolutionStatus::Discharged
                && let Some(anchor) = entry.initial_anchor
            {
                let Responsibility::Local(anchor_local) =
                    self.entries[anchor.get() - 1].responsibility
                else {
                    return Err(Error::InvalidInitialAnchor);
                };
                status = local_status(anchor_local);
            }
            for &anchor in log.anchors_of(row) {
                if status != ResolutionStatus::Discharged {
                    break;
                }
                let index = self
                    .entries
                    .get(anchor as usize)
                    .map(|e| e.g2_row)
                    .filter(|&index| index != super::g2_log::NONE)
                    .ok_or(Error::InvalidG2Anchor)? as usize;
                status = *statuses.get(index).ok_or(Error::InvalidG2Anchor)?;
            }
            statuses.push(status);
        }
        Ok(Some(G2Statuses(statuses)))
    }
}

struct G2Statuses(Vec<ResolutionStatus>);
impl G2Statuses {
    /// The resolved status of a G2' record (None for any other entry).
    fn status<K: Copy + Eq>(&self, ledger: &Ledger<K>, id: usize) -> Option<ResolutionStatus> {
        let row = ledger.g2_row(id)?;
        matches!(
            row.kind,
            super::g2_log::kind::G2_RESIDUAL | super::g2_log::kind::G2_FULL_COVER
        )
        .then(|| self.0[ledger.entries[id].g2_row as usize])
    }
}

fn local_status(local: Local) -> ResolutionStatus {
    match local {
        Local::Published(NativeOutcome::Completed {
            unresolved_frontiers: 0,
        }) => ResolutionStatus::Discharged,
        Local::Published(NativeOutcome::Completed {
            unresolved_frontiers,
        }) => ResolutionStatus::UnresolvedFrontiers {
            count: unresolved_frontiers,
        },
        Local::Published(NativeOutcome::Failed) => ResolutionStatus::Failed,
        Local::Published(NativeOutcome::Cancelled) => ResolutionStatus::Cancelled,
        _ => ResolutionStatus::Pending,
    }
}
