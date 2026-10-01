//! Bounded diagnostic census of exact query images crossing result entries.
//! It never constructs native summaries, looks up targets, or yields authority.
//! Only already-successfully-resolved source rows are observed on the merge
//! coordinator. No hot atomics, worker timers, or process/global reuse cache.
use super::CompactDomain;
use serde::Serialize;

const MAX_ROWS: usize = 16_384;
const BUCKETS: usize = 32_768;
const MAX_CAPACITY_BYTES: usize = 8 * 1024 * 1024;
const MAX_COMPARISONS: usize = 262_144;
const RETAIN: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Context {
    target: Option<u32>,
    /// Presence matters: an unprobed miss is not a stale snapshot receipt.
    snapshot: Option<(u64, u32)>,
    class: usize,
}

impl Context {
    pub(super) fn new(
        target: Option<u32>,
        snapshot: Option<(u64, u32)>,
        version: u64,
        published_len: usize,
    ) -> Self {
        let class = if target.is_some() {
            0
        } else if snapshot.is_some_and(|(v, n)| v == version && n as usize == published_len) {
            1
        } else {
            2
        };
        Self {
            target,
            snapshot,
            class,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub(in super::super::super::super) struct Stratum {
    rows: usize,
    cross_entry_geometry_repeats: usize,
    cross_entry_same_context_repeats: usize,
    resolved_stored: usize,
    resolved_candidate: usize,
}

impl Stratum {
    fn add(&mut self, next: &Self) {
        self.rows = self.rows.saturating_add(next.rows);
        self.cross_entry_geometry_repeats = self
            .cross_entry_geometry_repeats
            .saturating_add(next.cross_entry_geometry_repeats);
        self.cross_entry_same_context_repeats = self
            .cross_entry_same_context_repeats
            .saturating_add(next.cross_entry_same_context_repeats);
        self.resolved_stored = self.resolved_stored.saturating_add(next.resolved_stored);
        self.resolved_candidate = self
            .resolved_candidate
            .saturating_add(next.resolved_candidate);
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub(in super::super::super::super) struct Sample {
    epoch_version: u64,
    store_len: usize,
    eligible_entries: usize,
    eligible_rows: usize,
    observed_entries: usize,
    observed_rows: usize,
    skipped_rows: usize,
    unique_images: usize,
    unique_contexts: usize,
    cross_entry_geometry_repeats: usize,
    same_entry_geometry_repeats: usize,
    cross_entry_same_context_repeats: usize,
    stored_proposal: Stratum,
    current_view_miss: Stratum,
    stale_or_unprobed_miss: Stratum,
    comparison_count: usize,
    /// Vec backing capacities only, excluding allocator bookkeeping/RSS.
    allocated_capacity_bytes: usize,
    truncated: bool,
    stop_reason: Option<&'static str>,
}

struct Seen<const N: usize> {
    image: CompactDomain<N>,
    digest: u64,
    context: Context,
    first_entry: usize,
    multiple_entries: bool,
    next: Option<usize>,
}

pub(super) struct Observer<const N: usize> {
    sample: Sample,
    heads: Vec<Option<usize>>,
    seen: Vec<Seen<N>>,
    previous_entry: Option<usize>,
}

impl<const N: usize> Observer<N> {
    pub(super) fn for_cut(
        enabled: bool,
        version: u64,
        store_len: usize,
        eligible_entries: usize,
        eligible_rows: usize,
    ) -> Option<Self> {
        // Epoch versions, not invocation-relative cut numbers after resume.
        if !enabled || !(version < 8 || version % 64 == 0) {
            return None;
        }
        Some(Self::new(
            version,
            store_len,
            eligible_entries,
            eligible_rows,
        ))
    }

    fn new(version: u64, store_len: usize, entries: usize, rows: usize) -> Self {
        let mut value = Self {
            sample: Sample {
                epoch_version: version,
                store_len,
                eligible_entries: entries,
                eligible_rows: rows,
                ..Sample::default()
            },
            heads: Vec::new(),
            seen: Vec::new(),
            previous_entry: None,
        };
        if rows == 0 {
            return value;
        }
        let count = rows.min(MAX_ROWS);
        let buckets = (count * 2).next_power_of_two().min(BUCKETS);
        let requested = buckets
            .checked_mul(std::mem::size_of::<Option<usize>>())
            .and_then(|n| {
                count
                    .checked_mul(std::mem::size_of::<Seen<N>>())?
                    .checked_add(n)
            });
        if requested.is_none_or(|n| n > MAX_CAPACITY_BYTES) {
            value.stop("capacity_budget");
            return value;
        }
        if value.heads.try_reserve_exact(buckets).is_err()
            || value.seen.try_reserve_exact(count).is_err()
        {
            value.heads = Vec::new();
            value.seen = Vec::new();
            value.stop("allocation_refused");
            return value;
        }
        value.sample.allocated_capacity_bytes = value.heads.capacity()
            * std::mem::size_of::<Option<usize>>()
            + value.seen.capacity() * std::mem::size_of::<Seen<N>>();
        if value.sample.allocated_capacity_bytes > MAX_CAPACITY_BYTES {
            value.heads = Vec::new();
            value.seen = Vec::new();
            value.sample.allocated_capacity_bytes = 0;
            value.stop("capacity_budget");
            return value;
        }
        value.heads.resize(buckets, None);
        value
    }

    fn stop(&mut self, reason: &'static str) {
        self.sample.stop_reason = Some(reason);
        self.sample.truncated = true;
    }

    pub(super) fn record(
        &mut self,
        entry: usize,
        image: CompactDomain<N>,
        digest: u64,
        context: Context,
        resolved_stored: bool,
    ) {
        if self.sample.stop_reason.is_some() {
            return;
        }
        if self.sample.observed_rows == MAX_ROWS {
            self.stop("source_prefix_rows");
            return;
        }
        if self.sample.observed_rows >= self.sample.eligible_rows {
            self.stop("eligible_row_count");
            return;
        }
        // Only validated rows reach this diagnostic; no additional validation
        // or failure can precede/replace the mathematical source resolver.
        let bucket = digest as usize & (self.heads.len() - 1);
        let mut next = self.heads[bucket];
        let mut image_seen = false;
        let mut other_entry_image = false;
        let mut same_context = None;
        let mut other_entry_context = false;
        while let Some(at) = next {
            if self.sample.comparison_count == MAX_COMPARISONS {
                self.stop("comparison_budget");
                return;
            }
            self.sample.comparison_count += 1;
            let old = &self.seen[at];
            if old.digest == digest && old.image == image {
                image_seen = true;
                let other = old.first_entry != entry || old.multiple_entries;
                other_entry_image |= other;
                if old.context == context {
                    same_context = Some(at);
                    other_entry_context = other;
                }
            }
            next = old.next;
        }
        if self.previous_entry != Some(entry) {
            self.sample.observed_entries += 1;
            self.previous_entry = Some(entry);
        }
        self.sample.observed_rows += 1;
        self.sample.unique_images += usize::from(!image_seen);
        self.sample.cross_entry_geometry_repeats += usize::from(other_entry_image);
        self.sample.same_entry_geometry_repeats += usize::from(image_seen && !other_entry_image);
        self.sample.cross_entry_same_context_repeats += usize::from(other_entry_context);
        let stratum = match context.class {
            0 => &mut self.sample.stored_proposal,
            1 => &mut self.sample.current_view_miss,
            _ => &mut self.sample.stale_or_unprobed_miss,
        };
        stratum.rows += 1;
        stratum.cross_entry_geometry_repeats += usize::from(other_entry_image);
        stratum.cross_entry_same_context_repeats += usize::from(other_entry_context);
        stratum.resolved_stored += usize::from(resolved_stored);
        stratum.resolved_candidate += usize::from(!resolved_stored);
        if let Some(at) = same_context {
            self.seen[at].multiple_entries |= self.seen[at].first_entry != entry;
        } else {
            // Capacity was reserved for every possible observed prefix row.
            let at = self.seen.len();
            self.seen.push(Seen {
                image,
                digest,
                context,
                first_entry: entry,
                multiple_entries: false,
                next: self.heads[bucket],
            });
            self.heads[bucket] = Some(at);
            self.sample.unique_contexts += 1;
        }
    }

    pub(super) fn finish(mut self) -> Sample {
        self.sample.skipped_rows = self
            .sample
            .eligible_rows
            .saturating_sub(self.sample.observed_rows);
        self.sample.truncated |= self.sample.skipped_rows != 0;
        self.sample
    }
}

#[derive(Clone, Debug, Serialize)]
pub(in super::super::super::super) struct Totals {
    scope: &'static str,
    sampling: &'static str,
    interpretation: &'static str,
    max_rows_per_cut: usize,
    max_capacity_bytes_per_cut: usize,
    max_comparisons_per_cut: usize,
    sampled_cuts: u64,
    truncated_cuts: u64,
    eligible_rows: u64,
    eligible_entries: u64,
    observed_entries: u64,
    observed_rows: u64,
    skipped_rows: u64,
    cross_entry_geometry_repeats: u64,
    cross_entry_same_context_repeats: u64,
    stored_proposal: Stratum,
    current_view_miss: Stratum,
    stale_or_unprobed_miss: Stratum,
    retained_samples: [Option<Sample>; RETAIN],
}

impl Default for Totals {
    fn default() -> Self {
        Self {
            scope: "successful P2 preparations observed this invocation; not necessarily published; failed/cancelled P2 excluded; never authority",
            sampling: "global epoch version <8 or divisible by64, not first invocation cuts; first16384 successful source rows; prefix truncation may hide cross-entry repeats; last16 sampled cuts retained",
            interpretation: "exact full image equality; same context additionally requires identical target and snapshot presence/version/published_len within this immutable current store; repeated validation context is not a proof or cache hit; capacity excludes allocator overhead; no per-operation timing; observation folding cost included in canonical_dedup_seconds",
            max_rows_per_cut: MAX_ROWS,
            max_capacity_bytes_per_cut: MAX_CAPACITY_BYTES,
            max_comparisons_per_cut: MAX_COMPARISONS,
            sampled_cuts: 0,
            truncated_cuts: 0,
            eligible_rows: 0,
            eligible_entries: 0,
            observed_entries: 0,
            observed_rows: 0,
            skipped_rows: 0,
            cross_entry_geometry_repeats: 0,
            cross_entry_same_context_repeats: 0,
            stored_proposal: Stratum::default(),
            current_view_miss: Stratum::default(),
            stale_or_unprobed_miss: Stratum::default(),
            retained_samples: std::array::from_fn(|_| None),
        }
    }
}

impl Totals {
    pub(in super::super::super::super) fn add(&mut self, sample: &Sample) {
        self.sampled_cuts = self.sampled_cuts.saturating_add(1);
        self.truncated_cuts = self
            .truncated_cuts
            .saturating_add(u64::from(sample.truncated));
        self.eligible_rows = self
            .eligible_rows
            .saturating_add(sample.eligible_rows as u64);
        self.eligible_entries = self
            .eligible_entries
            .saturating_add(sample.eligible_entries as u64);
        self.observed_entries = self
            .observed_entries
            .saturating_add(sample.observed_entries as u64);
        self.observed_rows = self
            .observed_rows
            .saturating_add(sample.observed_rows as u64);
        self.skipped_rows = self.skipped_rows.saturating_add(sample.skipped_rows as u64);
        self.cross_entry_geometry_repeats = self
            .cross_entry_geometry_repeats
            .saturating_add(sample.cross_entry_geometry_repeats as u64);
        self.cross_entry_same_context_repeats = self
            .cross_entry_same_context_repeats
            .saturating_add(sample.cross_entry_same_context_repeats as u64);
        self.stored_proposal.add(&sample.stored_proposal);
        self.current_view_miss.add(&sample.current_view_miss);
        self.stale_or_unprobed_miss
            .add(&sample.stale_or_unprobed_miss);
        self.retained_samples.rotate_left(1);
        self.retained_samples[RETAIN - 1] = Some(sample.clone());
    }
}

#[cfg(test)]
mod tests;
