//! Source-ordered validation/resolution in bounded immutable blocks. Helpers
//! return tokens or unresolved canonical query descriptions; only the caller
//! performs global digest/image deduplication and assigns candidate ordinals.
use super::super::super::job::Miss;
use super::*;

pub(in super::super) mod observation;

const MISS_BLOCK: usize = 256;

#[derive(Clone, Copy)]
enum Task {
    Header(usize),
    Misses {
        entry: usize,
        first: usize,
        end: usize,
    },
}

struct Output<const N: usize> {
    entry: usize,
    first: usize,
    rows: ResolvedRows<N>,
    counters: P2Counters,
}

/// Transport verified Stored tokens without reserving a full query payload
/// for every row. None denotes exactly one payload, in source order. These
/// private buffers are populated together only after resolve_miss succeeds.
struct ResolvedRows<const N: usize> {
    tags: Vec<Option<Verified>>,
    candidates: Vec<(QueryImage<N>, Query<N>)>,
}

impl<const N: usize> ResolvedRows<N> {
    fn new(misses: &[Miss<N>]) -> Result<Self, Error> {
        let mut tags = Vec::new();
        let mut candidates = Vec::new();
        // Every supplied positive still passes resolve_miss. Its successful
        // result must be Stored; target-less rows may resolve either way.
        // This is an allocation upper bound, never a containment decision.
        // Reserve both buffers before the first resolver call: no row push
        // grows either allocation, including an entirely candidate-only block.
        reserve(&mut tags, misses.len())?;
        reserve(
            &mut candidates,
            misses.iter().filter(|miss| miss.target.is_none()).count(),
        )?;
        Ok(Self { tags, candidates })
    }

    fn push(&mut self, resolution: MissResolution<N>) {
        debug_assert!(self.tags.len() < self.tags.capacity());
        match resolution {
            MissResolution::Stored(token) => self.tags.push(Some(token)),
            MissResolution::Candidate { q, query } => {
                debug_assert!(self.candidates.len() < self.candidates.capacity());
                self.candidates.push((q, query));
                self.tags.push(None);
            }
        }
    }

    fn into_iter(self) -> ResolvedRowsIter<N> {
        ResolvedRowsIter {
            tags: self.tags.into_iter(),
            candidates: self.candidates.into_iter(),
        }
    }
}

struct ResolvedRowsIter<const N: usize> {
    tags: std::vec::IntoIter<Option<Verified>>,
    candidates: std::vec::IntoIter<(QueryImage<N>, Query<N>)>,
}

impl<const N: usize> Iterator for ResolvedRowsIter<N> {
    type Item = MissResolution<N>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.tags.next()? {
            Some(token) => MissResolution::Stored(token),
            None => {
                // One None is written with each payload by push. Never zip:
                // a missing internal payload must not truncate obligations.
                let (q, query) = self.candidates.next().expect("source candidate payload");
                MissResolution::Candidate { q, query }
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.tags.size_hint()
    }
}

// Keep the default nth/last implementations: skipping a tag must also consume
// its candidate payload. A Map over tags alone would not preserve that pairing.
impl<const N: usize> ExactSizeIterator for ResolvedRowsIter<N> {}

#[cfg(test)]
mod compact_rows_tests;

fn source_error(error: Fatal) -> Error {
    let detail = error.0.strip_prefix("P2: ").unwrap_or(&error.0);
    if detail == CANCELLED {
        Error::Stopped
    } else if detail.contains("allocation") {
        Error::RamGuard(error.0)
    } else {
        Error::Fatal(error)
    }
}

fn add_counters(total: &mut P2Counters, next: &P2Counters) {
    total.lookup.add(&next.lookup);
    total.verify.add(&next.verify);
    total.miss_requests += next.miss_requests;
    total.inspector.queries += next.inspector.queries;
    total.inspector.stored_hits += next.inspector.stored_hits;
    total.inspector.coordinator_miss_rechecks_skipped +=
        next.inspector.coordinator_miss_rechecks_skipped;
    // Header tasks appear in original entry order; floating timing attribution
    // is consequently folded in the same order as the serial reference too.
    total.inspector.seconds += next.inspector.seconds;
}

pub(super) fn prepare<const N: usize>(
    engine: &Engine,
    state: &EpochState<N>,
    checked: &Checked<N>,
    cancelled: &AtomicBool,
    control: &mut Checkpoint<'_>,
    metrics: &mut Metrics,
) -> Result<CandidateBatch<N>, Error> {
    let started = Instant::now();
    let mut tasks = Vec::new();
    let task_bound = checked
        .entries
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(metrics.obligations / MISS_BLOCK))
        .ok_or_else(|| Error::RamGuard("merge source task count overflow".into()))?;
    reserve(&mut tasks, task_bound)?;
    let mut slots_of = Vec::new();
    reserve(&mut slots_of, checked.entries.len())?;
    for (entry, value) in checked.entries.iter().enumerate() {
        control().map_err(classify)?;
        let mut slots = Vec::new();
        if value.class.merges() && !value.recurring_panic {
            reserve(&mut slots, value.result.misses.len())?;
            tasks.push(Task::Header(entry));
            for first in (0..value.result.misses.len()).step_by(MISS_BLOCK) {
                tasks.push(Task::Misses {
                    entry,
                    first,
                    end: first
                        .saturating_add(MISS_BLOCK)
                        .min(value.result.misses.len()),
                });
            }
        }
        slots_of.push(slots);
    }
    metrics.source_tasks = tasks.len();
    metrics.source_resolution_seconds += started.elapsed().as_secs_f64();
    let store: &super::super::super::store::Store<N> = &state.store;
    let version = state.k;
    let entries = &checked.entries;
    let mut candidates: Vec<Candidate<N>> = Vec::new();
    let mut by_digest: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut counters = P2Counters::default();
    // Opt-in diagnostic only. All observations are made after the ordinary
    // worker validation succeeds; no observation can supply a query or token.
    let mut observation = if engine.observe_cross_entry {
        observation::Observer::for_cut(
            true,
            state.k,
            store.len(),
            checked
                .entries
                .iter()
                .filter(|entry| entry.class.merges() && !entry.recurring_panic)
                .count(),
            metrics.obligations,
        )
    } else {
        None
    };
    // Only a bounded wave of query descriptions is held besides the final
    // plan. Task/input ordinals, not completion order, define every reduction.
    let wave_size = engine.helpers.max(1).saturating_mul(2);
    for (wave_number, wave) in tasks.chunks(wave_size).enumerate() {
        let started = Instant::now();
        let outputs = engine.map(
            wave,
            wave_number * wave_size,
            cancelled,
            control,
            |_, task, poll| {
                let mut work = P2Counters::default();
                match *task {
                    Task::Header(entry) => {
                        lookup_accounting(&entries[entry].result, store.len(), &mut work, poll)
                            .map_err(source_error)?;
                        Ok(Output {
                            entry,
                            first: 0,
                            rows: ResolvedRows::new(&[])?,
                            counters: work,
                        })
                    }
                    Task::Misses { entry, first, end } => {
                        let result = &entries[entry].result;
                        let mut rows = ResolvedRows::new(&result.misses[first..end])?;
                        let negative_prefix = checked.negative_prefix(result);
                        for miss in &result.misses[first..end] {
                            rows.push(
                                resolve_miss(
                                    store,
                                    version,
                                    result,
                                    miss,
                                    negative_prefix,
                                    &mut work,
                                    &mut *poll,
                                )
                                .map_err(source_error)?,
                            );
                        }
                        Ok(Output {
                            entry,
                            first,
                            rows,
                            counters: work,
                        })
                    }
                }
            },
        )?;
        metrics.source_resolution_seconds += started.elapsed().as_secs_f64();
        metrics.source_waves += 1;
        let started = Instant::now();
        for output in outputs {
            add_counters(&mut counters, &output.counters);
            let misses = &entries[output.entry].result.misses;
            let slots = &mut slots_of[output.entry];
            for (offset, resolution) in output.rows.into_iter().enumerate() {
                control().map_err(classify)?;
                if let Some(observation) = &mut observation {
                    let miss = &misses[output.first + offset];
                    let lookup = entries[output.entry].result.lookup.as_ref();
                    observation.record(
                        output.entry,
                        miss.image,
                        miss.digest,
                        observation::Context::new(
                            miss.target,
                            lookup.map(|work| (work.version, work.published_len)),
                            version,
                            store.len(),
                        ),
                        matches!(&resolution, MissResolution::Stored(_)),
                    );
                }
                let (q, query) = match resolution {
                    MissResolution::Stored(token) => {
                        slots.push(Ok(token));
                        continue;
                    }
                    MissResolution::Candidate { q, query } => (q, query),
                };
                let miss = &misses[output.first + offset];
                by_digest
                    .try_reserve(1)
                    .map_err(|_| Error::RamGuard("merge source digest allocation".into()))?;
                let same_digest = by_digest.entry(miss.digest).or_default();
                let mut known = None;
                for &slot in same_digest.iter() {
                    control().map_err(classify)?;
                    if candidates[slot as usize].q.image == miss.image {
                        known = Some(slot);
                        break;
                    }
                }
                let first = (output.entry as u32, miss.ordinal);
                let slot = match known {
                    Some(slot) => {
                        let candidate = &mut candidates[slot as usize];
                        candidate.first = candidate.first.min(first);
                        slot
                    }
                    None => {
                        candidates.try_reserve(1).map_err(|_| {
                            Error::RamGuard("merge source candidate allocation".into())
                        })?;
                        same_digest.try_reserve(1).map_err(|_| {
                            Error::RamGuard("merge source collision allocation".into())
                        })?;
                        let slot = candidates.len() as u32;
                        same_digest.push(slot);
                        candidates.push(Candidate {
                            key: (miss.digest, canonical_bytes(&miss.image)),
                            bucket: bucket_key(&miss.image),
                            first,
                            q,
                            query,
                        });
                        slot
                    }
                };
                slots.push(Err(slot));
            }
        }
        metrics.canonical_dedup_seconds += started.elapsed().as_secs_f64();
    }
    metrics.cross_entry_observation = observation.map(observation::Observer::finish);
    Ok(CandidateBatch {
        slots_of,
        candidates,
        counters,
    })
}
