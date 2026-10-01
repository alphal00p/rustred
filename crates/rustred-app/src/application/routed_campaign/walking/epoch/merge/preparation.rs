//! Read-only, bounded P2 preparation. A private pool consumes only explicitly
//! reserved helper capacity; it never uses Rayon's global pool or native CAS.
//! All tasks join before this call returns. Neither tasks nor returned plans
//! retain store borrows/snapshot leases across authoritative P3 publication.
//!
//! Successful plans match the serial reference. Error order is now explicit:
//! initial source order, then sorted (phase, owner)/member order, then survivor
//! order. The former HashMap iteration did not specify a cross-bucket error.
use super::*;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

mod source;

const CANCELLED: &str = "epoch merge preparation cancelled";

/// Logical output allowances, not an RSS prediction. They bound the shipped
/// obligation slots/candidates and the total retirement IDs, including helper
/// outputs awaiting reduction. Exceeding either refuses the whole unpublished
/// cut; it never truncates work. The caller owns their resource-policy values.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct Limits {
    pub obligations: usize,
    pub retirements: usize,
}

#[derive(Debug)]
pub(in super::super) enum Error {
    Stopped,
    RamGuard(String),
    Fatal(Fatal),
}

impl From<Fatal> for Error {
    fn from(error: Fatal) -> Self {
        Self::Fatal(error)
    }
}

fn classify(error: impl Into<String>) -> Error {
    let error = error.into();
    if error == CANCELLED {
        Error::Stopped
    } else if error.contains("allocation") || error == "prepared retirement set limit" {
        Error::RamGuard(error)
    } else {
        Error::Fatal(fatal(format!("P2: {error}")))
    }
}

fn checkpoint(cancelled: &AtomicBool) -> Result<(), &'static str> {
    if cancelled.load(Ordering::Relaxed) {
        Err(CANCELLED)
    } else {
        Ok(())
    }
}

// A coordinator-local callback is intentionally neither Send nor Sync. Serial
// preparation polls it within scans; parallel tasks construct atomic-only
// callbacks on their already-reserved helper threads.
type Checkpoint<'a> = dyn FnMut() -> Result<(), &'static str> + 'a;

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), Error> {
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::RamGuard("merge preparation output allocation".into()))
}

/// Session-local measurements, never containment/replay authority. Timings are
/// sequential wall phases, not summed helper CPU or inspector utilization.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub(in super::super) struct Metrics {
    pub helper_limit: usize,
    pub obligations: usize,
    pub candidates: usize,
    pub buckets: usize,
    pub largest_bucket: usize,
    pub survivors: usize,
    pub retirements: usize,
    pub reverse_waves: usize,
    pub source_tasks: usize,
    pub source_waves: usize,
    pub source_resolution_seconds: f64,
    pub canonical_dedup_seconds: f64,
    pub antichain_seconds: f64,
    pub representative_seconds: f64,
    pub reverse_seconds: f64,
    pub transfer_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_entry_observation: Option<source::observation::Sample>,
}

pub(in super::super) struct Prepared<const N: usize> {
    pub plan: MergePlan<N>,
    pub metrics: Metrics,
}

/// Invocation-local totals; not replay or containment authority. Whole-phase
/// wall durations are accumulated once, never summed worker task durations.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub(in super::super) struct Totals {
    pub cuts: u64,
    pub source_tasks: u64,
    pub source_waves: u64,
    pub obligations: u64,
    pub retirements: u64,
    pub peak_candidates: usize,
    pub peak_bucket: usize,
    pub source_resolution_seconds: f64,
    pub canonical_dedup_seconds: f64,
    pub antichain_seconds: f64,
    pub representative_seconds: f64,
    pub reverse_seconds: f64,
    pub transfer_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_entry_observation: Option<source::observation::Totals>,
}
impl Totals {
    pub fn add(&mut self, m: &Metrics) {
        self.cuts += 1;
        self.source_tasks += m.source_tasks as u64;
        self.source_waves += m.source_waves as u64;
        self.obligations += m.obligations as u64;
        self.retirements += m.retirements as u64;
        self.peak_candidates = self.peak_candidates.max(m.candidates);
        self.peak_bucket = self.peak_bucket.max(m.largest_bucket);
        self.source_resolution_seconds += m.source_resolution_seconds;
        self.canonical_dedup_seconds += m.canonical_dedup_seconds;
        self.antichain_seconds += m.antichain_seconds;
        self.representative_seconds += m.representative_seconds;
        self.reverse_seconds += m.reverse_seconds;
        self.transfer_seconds += m.transfer_seconds;
        if let Some(sample) = &m.cross_entry_observation {
            self.cross_entry_observation
                .get_or_insert_with(Default::default)
                .add(sample);
        }
    }
}

pub(in super::super) struct Engine {
    pool: Option<rayon::ThreadPool>,
    helpers: usize,
    limits: Limits,
    observe_cross_entry: bool,
}

impl Engine {
    /// Create once per campaign/session, not once per cut. A spawn failure is
    /// an operational refusal before any merge publication or admitted work.
    pub fn new(helpers: usize, limits: Limits) -> Result<Self, Error> {
        let pool = if helpers == 0 {
            None
        } else {
            Some(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(helpers)
                    .thread_name(|index| format!("epoch-prepare-{index}"))
                    .build()
                    .map_err(|error| Error::RamGuard(format!("merge helper pool: {error}")))?,
            )
        };
        Ok(Self {
            pool,
            helpers,
            limits,
            observe_cross_entry: std::env::var_os(super::super::inspector::profile::ENVIRONMENT)
                .as_deref()
                == Some(std::ffi::OsStr::new("1")),
        })
    }

    /// Run the preparation coordinator on one of the already reserved helpers
    /// while the caller remains responsive to progress/operational stops.
    /// Nested bucket tasks use this same private pool. The scope joins every
    /// task before returning; no read lease survives into P3 mutation.
    pub fn prepare_observed<const N: usize>(
        &self,
        state: &EpochState<N>,
        checked: &Checked<N>,
        stop_requested: impl FnMut() -> bool,
    ) -> Result<Prepared<N>, Error> {
        self.prepare_observed_periodic(
            state,
            checked,
            stop_requested,
            std::time::Duration::from_millis(50),
        )
    }

    fn prepare_observed_periodic<const N: usize>(
        &self,
        state: &EpochState<N>,
        checked: &Checked<N>,
        mut stop_requested: impl FnMut() -> bool,
        interval: std::time::Duration,
    ) -> Result<Prepared<N>, Error> {
        let cancelled = AtomicBool::new(stop_requested());
        let Some(pool) = &self.pool else {
            let mut last_poll = Instant::now();
            let mut until_clock = 0u16;
            let result = self.prepare_with_control(state, checked, &cancelled, &mut || {
                checkpoint(&cancelled)?;
                if until_clock == 0 {
                    until_clock = 255;
                    if last_poll.elapsed() >= interval {
                        if stop_requested() {
                            cancelled.store(true, Ordering::Release);
                        }
                        last_poll = Instant::now();
                    }
                } else {
                    until_clock -= 1;
                }
                checkpoint(&cancelled)
            });
            return if stop_requested() && !matches!(result, Err(Error::Fatal(_))) {
                Err(Error::Stopped)
            } else {
                result
            };
        };
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        pool.in_place_scope(|scope| {
            scope.spawn(|_| {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    self.prepare(state, checked, &cancelled)
                }))
                .unwrap_or_else(|_| {
                    Err(Error::Fatal(fatal("P2: preparation coordinator panicked")))
                });
                let _ = sender.send(result);
            });
            loop {
                if stop_requested() {
                    cancelled.store(true, Ordering::Release);
                }
                match receiver.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok(result) => return result,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(Error::Fatal(fatal("P2: preparation result channel closed")));
                    }
                }
            }
        })
    }

    /// Collect all results into their input slots before selecting the first
    /// error. An early error/panic never detaches a helper or publishes a prefix.
    fn map<T: Sync, U: Send>(
        &self,
        items: &[T],
        first_ordinal: usize,
        cancelled: &AtomicBool,
        control: &mut Checkpoint<'_>,
        prepare: impl Fn(usize, &T, &mut Checkpoint<'_>) -> Result<U, Error> + Sync,
    ) -> Result<Vec<U>, Error> {
        control().map_err(classify)?;
        let mut slots = Vec::new();
        reserve(&mut slots, items.len())?;
        slots.resize_with(items.len(), || None);
        // Reserve the reduction vector before launching. A later allocation
        // refusal must not replace an already-produced canonical task error.
        let mut outputs = Vec::new();
        reserve(&mut outputs, slots.len())?;
        let run = |ordinal, item, control: &mut Checkpoint<'_>| {
            catch_unwind(AssertUnwindSafe(|| {
                control().map_err(classify)?;
                prepare(ordinal, item, control)
            }))
            .unwrap_or_else(|_| {
                Err(Error::Fatal(fatal(format!(
                    "P2: preparation task {ordinal} panicked"
                ))))
            })
        };
        if let Some(pool) = &self.pool {
            pool.install(|| {
                slots
                    .par_iter_mut()
                    .zip(items.par_iter())
                    .enumerate()
                    .for_each(|(ordinal, (slot, item))| {
                        *slot = Some(run(first_ordinal + ordinal, item, &mut || {
                            checkpoint(cancelled)
                        }))
                    });
            });
        } else {
            for (ordinal, (slot, item)) in slots.iter_mut().zip(items).enumerate() {
                *slot = Some(run(first_ordinal + ordinal, item, control));
            }
        }
        for result in slots {
            outputs.push(result.expect("joined preparation slot")?);
        }
        Ok(outputs)
    }

    pub fn prepare<const N: usize>(
        &self,
        state: &EpochState<N>,
        checked: &Checked<N>,
        cancelled: &AtomicBool,
    ) -> Result<Prepared<N>, Error> {
        self.prepare_with_control(state, checked, cancelled, &mut || checkpoint(cancelled))
    }

    fn prepare_with_control<const N: usize>(
        &self,
        state: &EpochState<N>,
        checked: &Checked<N>,
        cancelled: &AtomicBool,
        control: &mut Checkpoint<'_>,
    ) -> Result<Prepared<N>, Error> {
        control().map_err(classify)?;
        if checked.entries.len() > u32::MAX as usize {
            return Err(Error::RamGuard("merge preparation entry ID range".into()));
        }
        let obligations = checked
            .entries
            .iter()
            .filter(|entry| entry.class.merges() && !entry.recurring_panic)
            .try_fold(0usize, |count, entry| {
                count.checked_add(entry.result.misses.len())
            })
            .filter(|&count| count <= self.limits.obligations && count <= u32::MAX as usize)
            .ok_or_else(|| Error::RamGuard("merge preparation obligation allowance".into()))?;
        let mut metrics = Metrics {
            helper_limit: self.helpers,
            obligations,
            ..Metrics::default()
        };
        let CandidateBatch {
            slots_of,
            candidates,
            mut counters,
        } = source::prepare(self, state, checked, cancelled, control, &mut metrics)?;
        metrics.candidates = candidates.len();
        control().map_err(classify)?;

        let mut by_bucket = BTreeMap::<(u8, u32), Vec<u32>>::new();
        for (slot, candidate) in candidates.iter().enumerate() {
            control().map_err(classify)?;
            by_bucket
                .entry(candidate.bucket)
                .or_default()
                .push(slot as u32);
        }
        let buckets: Vec<Vec<u32>> = by_bucket.into_values().collect();
        metrics.buckets = buckets.len();
        metrics.largest_bucket = buckets.iter().map(Vec::len).max().unwrap_or(0);
        let started = Instant::now();
        let flags = self.map(&buckets, 0, cancelled, control, |_, members, poll| {
            antichain(members, &candidates, poll)
        })?;
        metrics.antichain_seconds = started.elapsed().as_secs_f64();
        let mut survivor = vec![false; candidates.len()];
        for (members, flags) in buckets.iter().zip(flags) {
            for (&slot, flag) in members.iter().zip(flags) {
                control().map_err(classify)?;
                survivor[slot as usize] = flag;
            }
        }
        let mut order: Vec<u32> = (0..candidates.len() as u32)
            .filter(|&slot| survivor[slot as usize])
            .collect();
        order.sort_by_key(|&slot| candidates[slot as usize].first);
        let mut position_of = vec![u32::MAX; candidates.len()];
        for (position, &slot) in order.iter().enumerate() {
            control().map_err(classify)?;
            position_of[slot as usize] = position as u32;
        }
        let n_s = order.len();
        metrics.survivors = n_s;
        let started = Instant::now();
        let resolutions = self.map(&buckets, 0, cancelled, control, |_, members, poll| {
            resolve_bucket(members, &candidates, &survivor, &position_of, n_s, poll)
        })?;
        metrics.representative_seconds = started.elapsed().as_secs_f64();
        let mut resolved = vec![None; candidates.len()];
        for (tokens, work) in resolutions {
            counters.verify.add(&work);
            for (slot, token) in tokens {
                control().map_err(classify)?;
                resolved[slot as usize] = Some(token);
            }
        }
        let mut targets = Vec::new();
        reserve(&mut targets, slots_of.len())?;
        for slots in slots_of {
            let mut output = Vec::new();
            reserve(&mut output, slots.len())?;
            for slot in slots {
                control().map_err(classify)?;
                output.push(slot.unwrap_or_else(|c| resolved[c as usize].expect("resolved")));
            }
            targets.push(output);
        }
        counters.antichain_folded += (candidates.len() - n_s) as u64;

        // Admit reverse jobs in canonical waves. The index's live count is an
        // upper bound, not a guessed output size. Completed plus admitted
        // outputs never exceed the total retirement allowance; one job whose
        // upper bound exceeds remaining space is allowed only up to that space
        // and must fail rather than return a truncated result.
        let started = Instant::now();
        let mut retirements = Vec::new();
        reserve(&mut retirements, n_s)?;
        let mut used = 0usize;
        let mut at = 0;
        let store: &super::super::store::Store<N> = &state.store;
        while at < order.len() {
            control().map_err(classify)?;
            let remaining = self.limits.retirements - used;
            let mut allowances = Vec::new();
            let mut reserved = 0usize;
            while at + allowances.len() < order.len() && allowances.len() < self.helpers.max(1) {
                let slot = order[at + allowances.len()];
                let candidate = &candidates[slot as usize];
                let maximum = store.bucket_of.get(&candidate.bucket).map_or(0, |&bucket| {
                    store.buckets[bucket as usize].index.storage().live
                });
                if !allowances.is_empty() && maximum > remaining - reserved {
                    break;
                }
                let allowance = maximum.min(remaining - reserved);
                allowances.push((slot, allowance));
                reserved += allowance;
            }
            let wave = self.map(
                &allowances,
                at,
                cancelled,
                control,
                |_, &(slot, allowance), poll| {
                    let candidate = &candidates[slot as usize];
                    let mut work = LookupCounters::default();
                    let retire = store
                        .contained_live_bounded(
                            &candidate.q,
                            &candidate.query,
                            &mut work,
                            allowance,
                            poll,
                        )
                        .map_err(classify)?;
                    Ok((retire, work))
                },
            )?;
            metrics.reverse_waves += 1;
            at += wave.len();
            for (retire, work) in wave {
                used = used.checked_add(retire.len()).ok_or_else(|| {
                    Error::RamGuard("merge preparation retirement count overflow".into())
                })?;
                counters.lookup.add(&work);
                retirements.push(retire);
            }
        }
        metrics.reverse_seconds = started.elapsed().as_secs_f64();
        metrics.retirements = used;

        // Only this ordered fold chooses transfer representatives. Parallel
        // completion order cannot change the smallest containing position or
        // charge redundant positive verifies for already-assigned old IDs.
        let started = Instant::now();
        let mut assigned = HashMap::new();
        let mut survivors = Vec::new();
        reserve(&mut survivors, n_s)?;
        for (position, (&slot, retire)) in order.iter().zip(retirements).enumerate() {
            control().map_err(classify)?;
            let candidate = &candidates[slot as usize];
            for &old in &retire {
                control().map_err(classify)?;
                if assigned.contains_key(&old) {
                    continue;
                }
                let old_q = QueryImage::new(state.store.domains[old as usize]).map_err(classify)?;
                let token = verify(
                    Container::Planned {
                        pos: position as u32,
                        survivors: n_s,
                        image: &candidate.q.image,
                    },
                    &old_q,
                    &mut counters.verify,
                )
                .ok_or_else(|| {
                    Error::Fatal(fatal(format!("P2: reverse candidate {old} failed verify")))
                })?;
                assigned
                    .try_reserve(1)
                    .map_err(|_| Error::RamGuard("merge preparation transfer allocation".into()))?;
                assigned.insert(old, token);
            }
            survivors.push(Survivor {
                image: candidate.q.image,
                summary: candidate.query.compact,
                query: candidate.query.clone(),
                digest: candidate.key.0,
                retire,
            });
        }
        let mut transfers: Vec<_> = assigned.into_iter().collect();
        transfers.sort_by_key(|&(old, _)| old);
        metrics.transfer_seconds = started.elapsed().as_secs_f64();
        control().map_err(classify)?;
        Ok(Prepared {
            plan: MergePlan {
                survivors,
                targets,
                transfers,
                counters,
            },
            metrics,
        })
    }
}

fn antichain<const N: usize>(
    members: &[u32],
    candidates: &[Candidate<N>],
    control: &mut Checkpoint<'_>,
) -> Result<Vec<bool>, Error> {
    let index = (members.len() > INDEXED_ANTICHAIN)
        .then(|| TempIndex::build_controlled(members, candidates, &mut *control))
        .transpose()
        .map_err(classify)?;
    let mut flags = Vec::new();
    reserve(&mut flags, members.len())?;
    for (local_j, &j) in members.iter().enumerate() {
        control().map_err(classify)?;
        let cj = &candidates[j as usize];
        let dominates = |local_i: usize| {
            local_i != local_j && {
                let ci = &candidates[members[local_i] as usize];
                contains(ci, cj) && (!contains(cj, ci) || ci.key < cj.key)
            }
        };
        let dominated = match &index {
            Some(index) => index
                .first_controlled(cj, dominates, &mut *control)
                .map_err(classify)?
                .is_some(),
            None => (0..members.len()).any(dominates),
        };
        flags.push(!dominated);
    }
    Ok(flags)
}

fn resolve_bucket<const N: usize>(
    members: &[u32],
    candidates: &[Candidate<N>],
    survivor: &[bool],
    position_of: &[u32],
    n_s: usize,
    control: &mut Checkpoint<'_>,
) -> Result<(Vec<(u32, Verified)>, VerifyCounters), Error> {
    let mut bucket_survivors: Vec<u32> = members
        .iter()
        .copied()
        .filter(|&slot| survivor[slot as usize])
        .collect();
    bucket_survivors.sort_by_key(|&slot| position_of[slot as usize]);
    let index = (bucket_survivors.len() > INDEXED_ANTICHAIN)
        .then(|| TempIndex::build_controlled(&bucket_survivors, candidates, &mut *control))
        .transpose()
        .map_err(classify)?;
    let mut tokens = Vec::new();
    reserve(&mut tokens, members.len())?;
    let mut work = VerifyCounters::default();
    for &j in members {
        control().map_err(classify)?;
        let cj = &candidates[j as usize];
        let s = if survivor[j as usize] {
            j
        } else {
            match &index {
                Some(index) => index
                    .first_controlled(
                        cj,
                        |local| contains(&candidates[bucket_survivors[local] as usize], cj),
                        &mut *control,
                    )
                    .map_err(classify)?
                    .map(|local| bucket_survivors[local]),
                None => bucket_survivors
                    .iter()
                    .copied()
                    .find(|&s| contains(&candidates[s as usize], cj)),
            }
            .ok_or_else(|| Error::Fatal(fatal("P2: a non-survivor has no containing survivor")))?
        };
        let token = verify(
            Container::Planned {
                pos: position_of[s as usize],
                survivors: n_s,
                image: &candidates[s as usize].q.image,
            },
            &cj.q,
            &mut work,
        )
        .ok_or_else(|| Error::Fatal(fatal("P2: antichain token failed verify")))?;
        tokens.push((j, token));
    }
    Ok((tokens, work))
}

#[cfg(test)]
mod tests;
