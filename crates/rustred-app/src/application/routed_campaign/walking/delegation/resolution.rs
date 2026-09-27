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
        let mut memo: Vec<Option<ResolutionStatus>> = Vec::new();
        if self.g2_anchors {
            memo.try_reserve_exact(self.entries.len())
                .map_err(|_| Error::Allocation)?;
            memo.resize(self.entries.len(), None);
        }
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
                    if let Some(anchor) = entry.initial_anchor
                        && self.g2_anchors
                    {
                        // W0 G2' falsifier: anchors are committed Native
                        // entries of any ID, possibly partial themselves.
                        summary.partial_initial_inspections += 1;
                        if status == ResolutionStatus::Discharged {
                            status =
                                self.anchor_chain_status(anchor.get() - 1, entry.key, &mut memo)?;
                        }
                        if status != ResolutionStatus::Discharged {
                            summary.partial_initial_blocked += 1;
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
}

impl<K: Copy + Eq> Ledger<K> {
    /// W0 G2' falsifier: the final status of a committed anchor, following its
    /// own residual anchors. Anchor chains are strictly ordered in commit time,
    /// so a revisit (cycle) is an invariant violation.
    fn anchor_chain_status(
        &self,
        start: usize,
        key: K,
        memo: &mut [Option<ResolutionStatus>],
    ) -> Result<ResolutionStatus, Error> {
        let mut path = Vec::new();
        let mut current = start;
        let status = loop {
            if let Some(done) = memo.get(current).copied().flatten() {
                break done;
            }
            let entry = self
                .entries
                .get(current)
                .ok_or(Error::InvalidInitialAnchor)?;
            let Responsibility::Local(local) = entry.responsibility else {
                return Err(Error::InvalidInitialAnchor);
            };
            if entry.key != key || path.len() > self.entries.len() {
                return Err(Error::InvalidInitialAnchor);
            }
            let own = local_status(local);
            match entry.initial_anchor {
                Some(next) if own == ResolutionStatus::Discharged => {
                    path.push(current);
                    current = next.get() - 1;
                }
                _ => {
                    memo[current] = Some(own);
                    break own;
                }
            }
        };
        for id in path {
            memo[id] = Some(status);
        }
        Ok(status)
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
