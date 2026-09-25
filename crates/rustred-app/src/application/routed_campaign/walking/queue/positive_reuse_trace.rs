//! Test-only spectator of actual successful non-exact admission requests.
//! Never serves a hit, changes admission, or supplies completion authority.
use super::{Domain, Phase, Queue};
use serde::Serialize;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::rc::Rc;

const MAX_ENTRIES: usize = 65_536;
const DESTINATION_ENTRIES: usize = 1024;
const DESTINATION_BYTES: usize = 2 * 1024 * 1024;
const MEMO_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
const EXAMPLES: usize = 8;

thread_local! {
    static ACTIVE: RefCell<Option<Trace>> = const { RefCell::new(None) };
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct Source {
    id: usize,
    phase: Phase,
    owner: Vec<bool>,
}

#[derive(Debug)]
struct Key {
    phase: Phase,
    owner: Box<[bool]>,
    lower: Box<[u64]>,
    upper: Box<[Option<u64>]>,
    rank: Option<u32>,
    powers: rustred::solver::DomainPowerBounds,
}
impl Key {
    fn new<const N: usize>(d: &Domain<N>) -> Self {
        Self {
            phase: d.phase,
            owner: d.owner.to_vec().into_boxed_slice(),
            lower: d.lower.clone().into_boxed_slice(),
            upper: d.upper.clone().into_boxed_slice(),
            rank: d.rank,
            powers: d.powers,
        }
    }
    fn matches<const N: usize>(&self, d: &Domain<N>) -> bool {
        self.phase == d.phase
            && self.owner.as_ref() == d.owner
            && self.lower.as_ref() == d.lower
            && self.upper.as_ref() == d.upper
            && self.rank == d.rank
            && self.powers == d.powers
    }
    fn logical_bytes(&self) -> usize {
        size_of::<Self>() + 2 * size_of::<usize>() // Rc header, excluding allocator overhead.
            + size_of_val(self.owner.as_ref())
            + size_of_val(self.lower.as_ref())
            + size_of_val(self.upper.as_ref())
    }
}
fn fingerprint<const N: usize>(d: &Domain<N>) -> u64 {
    let mut h = DefaultHasher::new();
    d.hash(&mut h);
    h.finish()
}

#[derive(Default, Clone, Copy, Serialize)]
struct Counts {
    requests: usize,
    semantic_requests: usize,
    raw_requests: usize,
    forward_comparison_charges: usize,
    summary_builds: usize,
}
impl Counts {
    fn add(&mut self, semantic: bool, checks: usize, summaries: usize) {
        self.requests = self.requests.checked_add(1).unwrap();
        self.semantic_requests = self
            .semantic_requests
            .checked_add(usize::from(semantic))
            .unwrap();
        self.raw_requests = self
            .raw_requests
            .checked_add(usize::from(!semantic))
            .unwrap();
        self.forward_comparison_charges =
            self.forward_comparison_charges.checked_add(checks).unwrap();
        self.summary_builds = self.summary_builds.checked_add(summaries).unwrap();
    }
}

#[derive(Default, Clone, Copy)]
struct Destination {
    requests: usize,
    positive: usize,
    semantic: usize,
    forward_charges: usize,
}
struct Entry {
    key: Rc<Key>,
    first_source: Rc<Source>,
    previous_source: Rc<Source>,
    first_representative: usize,
    previous_representative: usize,
}
impl Entry {
    fn logical_estimated_bytes(&self) -> usize {
        // Count shared sources per entry conservatively; do not claim allocator RSS.
        self.key.logical_bytes()
            + 2 * (size_of::<Source>() + 2 * size_of::<usize>() + self.first_source.owner.len())
    }
}
struct Trace {
    arity: usize,
    requested_capacity: usize,
    capacity: usize,
    source: Option<Rc<Source>>,
    destinations: [HashMap<Box<[bool]>, Destination>; 2],
    destination_overflow: Destination,
    destination_entries: usize,
    destination_entry_limit: usize,
    destination_bytes: usize,
    destination_byte_limit: usize,
    buckets: HashMap<u64, Vec<Entry>>,
    fifo: VecDeque<(u64, Rc<Key>)>,
    all: Counts,
    same_source: Counts,
    cross_source: Counts,
    misses: Counts,
    source_changes: usize,
    insertions: usize,
    evictions: usize,
    representative_changes: usize,
    retained_payload_bytes: usize,
    bucket_entry_capacity: usize,
    peak_logical_bytes: usize,
    examples: Vec<Value>,
    index_counters_disabled_before: Option<bool>,
    index_counters_disabled_after: Option<bool>,
}
impl Trace {
    fn new(arity: usize, capacity: usize) -> Result<Self, &'static str> {
        Self::with_destination_limits(arity, capacity, DESTINATION_ENTRIES, DESTINATION_BYTES)
    }
    fn with_destination_limits(
        arity: usize,
        capacity: usize,
        destination_entry_limit: usize,
        destination_byte_limit: usize,
    ) -> Result<Self, &'static str> {
        if arity == 0 || capacity == 0 || capacity > MAX_ENTRIES || destination_entry_limit == 0 {
            return Err("spectator bounds");
        }
        let entry_bytes = arity
            .checked_mul(
                size_of::<bool>()
                    + size_of::<u64>()
                    + size_of::<Option<u64>>()
                    + 2 * size_of::<bool>(),
            )
            .and_then(|n| {
                n.checked_add(
                    size_of::<Key>()
                        + 2 * size_of::<Source>()
                        + size_of::<Entry>()
                        + 6 * size_of::<usize>(),
                )
            })
            .ok_or("memo byte overflow")?;
        let requested_capacity = capacity;
        let capacity = capacity.min(MEMO_PAYLOAD_BYTES / entry_bytes);
        if capacity == 0 {
            return Err("memo payload allowance");
        }
        let destinations = [HashMap::new(), HashMap::new()];
        let mut buckets = HashMap::new();
        buckets
            .try_reserve(capacity)
            .map_err(|_| "memo allocation")?;
        let mut fifo = VecDeque::new();
        fifo.try_reserve_exact(capacity)
            .map_err(|_| "FIFO allocation")?;
        let mut out = Self {
            arity,
            requested_capacity,
            capacity,
            source: None,
            destinations,
            destination_overflow: Destination::default(),
            destination_entries: 0,
            destination_entry_limit,
            destination_bytes: 0,
            destination_byte_limit,
            buckets,
            fifo,
            all: Counts::default(),
            same_source: Counts::default(),
            cross_source: Counts::default(),
            misses: Counts::default(),
            source_changes: 0,
            insertions: 0,
            evictions: 0,
            representative_changes: 0,
            retained_payload_bytes: 0,
            bucket_entry_capacity: 0,
            peak_logical_bytes: 0,
            examples: Vec::new(),
            index_counters_disabled_before: None,
            index_counters_disabled_after: None,
        };
        out.update_peak();
        Ok(out)
    }
    fn destination<const N: usize>(&mut self, d: &Domain<N>) -> &mut Destination {
        assert_eq!(self.arity, N);
        assert_eq!(d.lower.len(), N);
        assert_eq!(d.upper.len(), N);
        let phase = usize::from(d.phase == Phase::Route);
        if !self.destinations[phase].contains_key(d.owner.as_slice()) {
            let bytes = size_of::<(Box<[bool]>, Destination)>()
                .checked_add(N)
                .unwrap();
            if self.destination_entries == self.destination_entry_limit
                || self
                    .destination_bytes
                    .checked_add(bytes)
                    .is_none_or(|n| n > self.destination_byte_limit)
            {
                return &mut self.destination_overflow;
            }
            self.destinations[phase]
                .try_reserve(1)
                .expect("bounded destination allocation");
            self.destinations[phase]
                .insert(d.owner.to_vec().into_boxed_slice(), Destination::default());
            self.destination_entries += 1;
            self.destination_bytes += bytes;
        }
        self.destinations[phase]
            .get_mut(d.owner.as_slice())
            .unwrap()
    }
    fn set_source<const N: usize>(&mut self, id: usize, d: &Domain<N>) {
        assert_eq!(self.arity, N);
        if self
            .source
            .as_ref()
            .is_some_and(|s| s.id == id && s.phase == d.phase && s.owner == d.owner)
        {
            return;
        }
        self.source = Some(Rc::new(Source {
            id,
            phase: d.phase,
            owner: d.owner.to_vec(),
        }));
        self.source_changes = self.source_changes.checked_add(1).unwrap();
    }
    fn logical_estimate(&self) -> usize {
        self.retained_payload_bytes
            + self
                .destinations
                .iter()
                .map(|m| m.capacity() * (size_of::<(Box<[bool]>, Destination)>() + 1))
                .sum::<usize>()
            + self.destination_entries * self.arity * size_of::<bool>()
            + self.buckets.capacity() * (size_of::<(u64, Vec<Entry>)>() + 1)
            + self.bucket_entry_capacity * size_of::<Entry>()
            + self.fifo.capacity() * size_of::<(u64, Rc<Key>)>()
            + size_of::<Self>()
    }
    fn update_peak(&mut self) {
        self.peak_logical_bytes = self.peak_logical_bytes.max(self.logical_estimate());
    }
    fn positive<const N: usize>(
        &mut self,
        d: &Domain<N>,
        representative: usize,
        semantic: bool,
        checks: usize,
        summaries: usize,
        hash: u64,
    ) {
        let source = Rc::clone(self.source.as_ref().expect("source-bound observation"));
        self.all.add(semantic, checks, summaries);
        let dest = self.destination(d);
        dest.positive = dest.positive.checked_add(1).unwrap();
        dest.semantic = dest.semantic.checked_add(usize::from(semantic)).unwrap();
        dest.forward_charges = dest.forward_charges.checked_add(checks).unwrap();
        if let Some(entry) = self
            .buckets
            .get_mut(&hash)
            .and_then(|bucket| bucket.iter_mut().find(|e| e.key.matches(d)))
        {
            let cross = entry.previous_source.as_ref() != source.as_ref();
            if cross {
                self.cross_source.add(semantic, checks, summaries);
                if self.examples.len() < EXAMPLES {
                    self.examples
                        .push(json!({"previous_source":entry.previous_source.as_ref(),
                        "current_source":source.as_ref(),"first_source":entry.first_source.as_ref(),
                        "destination":{"phase":d.phase,"owner":d.owner.as_slice(),"lower":d.lower,
                            "upper":d.upper,"rank":d.rank,"powers":format!("{:?}", d.powers)},
                        "first_representative":entry.first_representative,
                        "previous_representative":entry.previous_representative,
                        "current_representative":representative,"semantic":semantic,
                        "forward_comparison_charges":checks}));
                }
            } else {
                self.same_source.add(semantic, checks, summaries);
            }
            self.representative_changes = self
                .representative_changes
                .checked_add(usize::from(entry.previous_representative != representative))
                .unwrap();
            entry.previous_source = source;
            entry.previous_representative = representative;
            // Update attribution, never FIFO position.
            return;
        }
        self.misses.add(semantic, checks, summaries);
        if self.fifo.len() == self.capacity {
            let (old_hash, old_key) = self.fifo.pop_front().unwrap();
            let bucket = self.buckets.get_mut(&old_hash).unwrap();
            let index = bucket
                .iter()
                .position(|e| Rc::ptr_eq(&e.key, &old_key))
                .unwrap();
            let removed = bucket.swap_remove(index);
            self.retained_payload_bytes -= removed.logical_estimated_bytes();
            if bucket.is_empty() {
                self.bucket_entry_capacity -= bucket.capacity();
                self.buckets.remove(&old_hash);
            }
            self.evictions = self.evictions.checked_add(1).unwrap();
        }
        let key = Rc::new(Key::new(d));
        let entry = Entry {
            key: Rc::clone(&key),
            first_source: Rc::clone(&source),
            previous_source: source,
            first_representative: representative,
            previous_representative: representative,
        };
        self.retained_payload_bytes += entry.logical_estimated_bytes();
        let bucket = self.buckets.entry(hash).or_default();
        let before_capacity = bucket.capacity();
        bucket.push(entry);
        self.bucket_entry_capacity += bucket.capacity() - before_capacity;
        self.fifo.push_back((hash, key));
        self.insertions = self.insertions.checked_add(1).unwrap();
        self.update_peak();
    }
    fn top(&self, positive: bool) -> Vec<Value> {
        let mut rows: Vec<_> = self
            .destinations
            .iter()
            .enumerate()
            .flat_map(|(phase, m)| m.iter().map(move |(owner, c)| (phase, owner, c)))
            .filter(|(_, _, c)| {
                if positive {
                    c.positive > 0
                } else {
                    c.requests > 0
                }
            })
            .collect();
        rows.sort_by(|(pa, oa, a), (pb, ob, b)| {
            let (a, b) = if positive {
                (a.positive, b.positive)
            } else {
                (a.requests, b.requests)
            };
            b.cmp(&a).then(pa.cmp(pb)).then(oa.cmp(ob))
        });
        rows.truncate(16);
        rows.into_iter().map(|(phase,owner,c)| {
            let owner: String=owner.iter().map(|b|if *b {'1'}else{'0'}).collect();
            json!({"phase":if phase == 0 { "Apply" } else { "Route" },
                "owner":owner,"queue_requests":c.requests,"positive_nonexact":c.positive,
                "semantic_positive":c.semantic,"positive_forward_comparison_charges":c.forward_charges})
        }).collect()
    }
    fn report(&self) -> Value {
        json!({"schema":"rustred.test-positive-inclusion-spectator.v1",
            "mode":"observation_only; no hit served", "arity":self.arity,
            "requested_entry_capacity":self.requested_capacity,"entry_capacity":self.capacity,
            "memo_entry_payload_allowance_bytes":MEMO_PAYLOAD_BYTES,
            "eviction":"FIFO insert order; hits do not refresh", "source_changes":self.source_changes,
            "source_identity":"actual Ordered publisher ID, phase and owner; unsplit only",
            "hit_source_definition":"compared with previous observed positive request for that retained full key",
            "positive_nonexact":self.all,"same_source_hits":self.same_source,"cross_source_hits":self.cross_source,
            "cold_or_evicted_misses":self.misses,"insertions":self.insertions,"evictions":self.evictions,
            "retained_entries":self.fifo.len(),"representative_changes_on_hit":self.representative_changes,
            "forward_cost_scope":"committed forward comparison charges, including reused preparation accounting; excludes maintenance; do not add speculative checks",
            "queue_request_scope":"actual admission calls with source identity; excludes job-local reuse and initial inputs; includes unsuccessful attempted calls",
            "queue_requests":self.destinations.iter().flat_map(|m|m.values()).map(|c|c.requests).sum::<usize>()+self.destination_overflow.requests,
            "destination_retained_keys":self.destination_entries,
            "destination_entry_limit":self.destination_entry_limit,"destination_key_payload_bytes":self.destination_bytes,
            "destination_key_byte_limit":self.destination_byte_limit,
            "destination_scope":"exact counts for first retained phase/owner keys; overflow aggregate; top lists are not global heavy hitters if overflow is nonzero",
            "destination_overflow":{"queue_requests":self.destination_overflow.requests,"positive_nonexact":self.destination_overflow.positive,
                "semantic_positive":self.destination_overflow.semantic,"positive_forward_comparison_charges":self.destination_overflow.forward_charges},
            "destination_top_by_requests":self.top(false),"destination_top_by_positive":self.top(true),
            "logical_retained_estimated_bytes":self.logical_estimate(),"peak_logical_estimated_bytes":self.peak_logical_bytes,
            "memory_scope":"owned payload/capacity estimate, not an allocation upper bound; shared Source charged per entry conservatively; excludes current-source metadata not retained by entries, allocator metadata, HashMap control padding and bounded example JSON; not RSS",
            "test_index_counters_disabled_and_zero_before":self.index_counters_disabled_before,
            "test_index_counters_disabled_and_zero_after":self.index_counters_disabled_after,
            "build_scope":"cfg(test) layout and spectator remain; no production timing inference",
            "cross_source_examples":self.examples,"cache_authority":false,"production_recommendation":false})
    }
}

pub(in super::super) fn enabled() -> bool {
    ACTIVE.with(|t| t.borrow().is_some())
}
pub(in super::super) fn begin_run<const N: usize>(q: &mut Queue<N>) {
    q.disable_index_work_counters(); // Existing preference propagates to future buckets.
    let zero = q.index_work_counters_disabled_and_zero();
    ACTIVE.with(|t| {
        t.borrow_mut()
            .as_mut()
            .unwrap()
            .index_counters_disabled_before = Some(zero)
    });
    assert!(
        zero,
        "spectator starts with disabled, zero test index counters"
    );
}
pub(in super::super) fn end_run<const N: usize>(q: &Queue<N>) {
    let zero = q.index_work_counters_disabled_and_zero();
    ACTIVE.with(|t| {
        t.borrow_mut()
            .as_mut()
            .unwrap()
            .index_counters_disabled_after = Some(zero)
    });
    assert!(
        zero,
        "spectator must not execute test index counter atomics"
    );
}
pub(in super::super) fn set_source<const N: usize>(id: usize, d: &Domain<N>) {
    ACTIVE.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut() {
            t.set_source(id, d);
        }
    });
}
pub(super) struct Observation {
    checks: usize,
    maintenance: usize,
    summaries: usize,
    source: usize,
}
pub(super) fn begin<const N: usize>(q: &Queue<N>, d: &Domain<N>) -> Option<Observation> {
    ACTIVE.with(|t| {
        let mut t = t.borrow_mut();
        let t = t.as_mut()?;
        let source = t.source.as_ref()?.id;
        let destination = t.destination(d);
        destination.requests = destination.requests.checked_add(1).unwrap();
        t.update_peak();
        Some(Observation {
            checks: q.containment_checks,
            maintenance: q.containment_maintenance_checks,
            summaries: q.containment_summary_builds,
            source,
        })
    })
}
pub(super) fn positive<const N: usize>(
    q: &Queue<N>,
    d: &Domain<N>,
    id: usize,
    semantic: bool,
    observation: Option<Observation>,
) {
    let Some(o) = observation else {
        return;
    };
    assert_eq!(q.containment_maintenance_checks, o.maintenance);
    ACTIVE.with(|t| {
        let mut t = t.borrow_mut();
        let t = t.as_mut().unwrap();
        assert_eq!(t.source.as_ref().unwrap().id, o.source);
        t.positive(
            d,
            id,
            semantic,
            q.containment_checks.checked_sub(o.checks).unwrap(),
            q.containment_summary_builds
                .checked_sub(o.summaries)
                .unwrap(),
            fingerprint(d),
        );
    });
}
struct Session(PhantomData<Rc<()>>);
impl Session {
    fn start(arity: usize, capacity: usize) -> Result<Self, &'static str> {
        let trace = Trace::new(arity, capacity)?;
        ACTIVE.with(|t| {
            assert!(t.borrow().is_none(), "one spectator per coordinator");
            *t.borrow_mut() = Some(trace);
        });
        Ok(Self(PhantomData))
    }
    fn finish(self) -> Value {
        ACTIVE.with(|t| t.borrow_mut().take().unwrap().report())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        ACTIVE.with(|t| {
            t.borrow_mut().take();
        });
    }
}

#[cfg(test)]
mod tests;
