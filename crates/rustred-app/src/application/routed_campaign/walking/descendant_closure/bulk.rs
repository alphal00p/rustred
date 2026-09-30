//! Source-wise dependency publication. This is an optional monitoring graph,
//! not the authoritative edge log. Failure disables observation as in the
//! scalar path; it must never be interpreted as discharge of an obligation.
use super::{FLAG_SEALED, Tracker, edges::Edges};

impl Tracker {
    /// Preallocate log capacity for a prepared cut, without changing logical
    /// edges or revisions. This does not claim full P3 transactional preflight:
    /// a log-cap fold or an unsealed source's dedup storage may still allocate.
    pub fn reserve_edge_batch(&mut self, additional: usize) {
        if self.unavailable.is_none() && self.edges.reserve_append(additional).is_err() {
            self.disable("dependency edge allocation unavailable");
        }
    }

    /// Publish a prepared source's nondecreasing target IDs and completion.
    /// Adjacent duplicates and edges retained from earlier unsealed attempts
    /// are ignored exactly as by `edge` followed by `finish`. The caller keeps
    /// alias/native publication order; only the inside of one source is batched.
    ///
    /// A fresh successfully sealed source needs no temporary dedup hash table.
    /// Its usual strictly sorted targets are appended directly after one range
    /// validation and one storage reservation. All source/target IDs must have
    /// been registered by `discovered` before this call.
    pub fn finish_with_sorted_targets(
        &mut self,
        source: usize,
        targets: &[u32],
        inspected: bool,
        success: bool,
    ) {
        self.finish_with_sorted_targets_using(
            source,
            targets,
            inspected,
            success,
            Edges::push_source,
        );
    }

    fn finish_with_sorted_targets_using(
        &mut self,
        source: usize,
        targets: &[u32],
        inspected: bool,
        success: bool,
        append: impl FnOnce(&mut Edges, u32, &[u32]) -> Result<(), ()>,
    ) {
        if self.unavailable.is_some() {
            return;
        }
        if targets.is_empty() {
            self.finish(source, inspected, success);
            return;
        }
        // Invalid endpoints are exceptional, not the hot path. Replay the
        // scalar failure path so an earlier sealed-source/revision failure
        // and any successful prefix retain their original diagnostic/counters.
        if source >= self.flags.len()
            || targets
                .iter()
                .any(|&target| target as usize >= self.flags.len())
        {
            for &target in targets {
                self.edge(source, target as usize);
            }
            self.finish(source, inspected, success);
            return;
        }
        if self.flags[source] & FLAG_SEALED != 0 {
            self.edge(source, targets[0] as usize);
            return;
        }
        if targets.windows(2).any(|pair| pair[0] > pair[1]) {
            self.disable("dependency batch targets are not sorted");
            return;
        }
        let source_id = source as u32;
        let old = self.open_targets.get(&source_id);
        let strict = targets.windows(2).all(|pair| pair[0] != pair[1]);
        let mut filtered = Vec::new();
        let new_targets = if old.is_none() && strict {
            targets
        } else {
            if filtered.try_reserve(targets.len()).is_err() {
                self.disable("dependency edge allocation unavailable");
                return;
            }
            let mut previous = None;
            for &target in targets {
                if previous != Some(target) && !old.is_some_and(|set| set.contains(&target)) {
                    filtered.push(target);
                }
                previous = Some(target);
            }
            filtered.as_slice()
        };

        // If the scalar sequence overflows, its next changed() disables and
        // clears the optional graph at revision MAX before finish is reached.
        let Some(revision) = self.revision.checked_add(new_targets.len() as u64) else {
            self.revision = u64::MAX;
            self.disable("dependency graph revision overflow");
            return;
        };
        if !success && !new_targets.is_empty() {
            if !self.open_targets.contains_key(&source_id)
                && self.open_targets.try_reserve(1).is_err()
            {
                self.disable("dependency deduplication allocation unavailable");
                return;
            }
            if self
                .open_targets
                .entry(source_id)
                .or_default()
                .try_reserve(new_targets.len())
                .is_err()
            {
                self.disable("dependency edge allocation unavailable");
                return;
            }
        }
        let edges_before = self.edges.len();
        if append(&mut self.edges, source_id, new_targets).is_err() {
            // Earlier chunks may have succeeded before a log-cap fold or
            // reservation failed. disable() clears the optional graph, but
            // its revision still describes that committed prefix honestly.
            self.revision += (self.edges.len() - edges_before) as u64;
            self.disable("dependency edge allocation unavailable");
            return;
        }
        if !success && !new_targets.is_empty() {
            self.open_targets
                .get_mut(&source_id)
                .expect("reserved open-source target table")
                .extend(new_targets.iter().copied());
        }
        self.revision = revision;
        self.finish(source, inspected, success);
    }
}

#[cfg(test)]
mod tests;
