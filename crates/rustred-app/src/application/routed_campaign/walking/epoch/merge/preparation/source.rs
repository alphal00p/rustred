//! Source-ordered validation/resolution in bounded immutable blocks. Helpers
//! return tokens or unresolved canonical query descriptions; only the caller
//! performs global digest/image deduplication and assigns candidate ordinals.
use super::*;

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
    rows: Vec<MissResolution<N>>,
    counters: P2Counters,
}

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
                            rows: Vec::new(),
                            counters: work,
                        })
                    }
                    Task::Misses { entry, first, end } => {
                        let result = &entries[entry].result;
                        let mut rows = Vec::new();
                        reserve(&mut rows, end - first)?;
                        for miss in &result.misses[first..end] {
                            rows.push(
                                resolve_miss(store, version, result, miss, &mut work, &mut *poll)
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
    Ok(CandidateBatch {
        slots_of,
        candidates,
        counters,
    })
}
