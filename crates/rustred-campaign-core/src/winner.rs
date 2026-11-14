//! Streaming first-match dispatch over a key stream, used rules, and the WINNER DIFF (plan 2.4.7 D0, D6; R20).
//! Interface: `interfaces/campaign-core-domain.md` section 7.8.
//!
//! Determinism: the key stream is cut into chunks of a fixed size (independent of the thread count), chunks are
//! dispatched in parallel and merged in chunk order, so every output is identical for every thread count.

use std::collections::BTreeMap;

use crate::cover::{CoverError, Dispatch, Dispatcher, RuleRef};
use crate::key::Key;

/// Parallelism and chunking of a stream pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamOptions {
    pub threads: usize,
    /// Keys per chunk (the unit of work; fixes the merge order).
    pub chunk: usize,
}

impl Default for StreamOptions {
    fn default() -> Self {
        StreamOptions {
            threads: 1,
            chunk: 16_384,
        }
    }
}

/// Cut `keys` into chunks of `o.chunk` keys, run `work` on up to `o.threads` chunks at once, and hand every
/// (chunk, result) to `sink` in stream order. Memory: `threads x chunk` keys plus the results of one batch.
pub fn for_each_chunk<T, I, W, S>(keys: I, o: &StreamOptions, work: W, mut sink: S)
where
    T: Send,
    I: IntoIterator<Item = Key>,
    W: Fn(&[Key]) -> T + Sync,
    S: FnMut(Vec<Key>, T),
{
    let threads = o.threads.max(1);
    let chunk = o.chunk.max(1);
    let mut it = keys.into_iter();
    loop {
        let mut batch: Vec<Vec<Key>> = Vec::with_capacity(threads);
        let mut exhausted = false;
        while batch.len() < threads {
            let c: Vec<Key> = it.by_ref().take(chunk).collect();
            if c.len() < chunk {
                exhausted = true;
            }
            if !c.is_empty() {
                batch.push(c);
            }
            if exhausted {
                break;
            }
        }
        if batch.is_empty() {
            return;
        }
        let results: Vec<T> = if batch.len() == 1 {
            vec![work(&batch[0])]
        } else {
            let work = &work;
            std::thread::scope(|s| {
                let hs: Vec<_> = batch.iter().map(|c| s.spawn(move || work(c))).collect();
                hs.into_iter().map(|h| h.join().expect("worker")).collect()
            })
        };
        for (c, r) in batch.into_iter().zip(results) {
            sink(c, r);
        }
        if exhausted {
            return;
        }
    }
}

/// Default rule identity across table sets: the table's NAME, the record position, the record label.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId {
    pub table: String,
    pub record: usize,
    pub label: i64,
}

/// The default identity of a credited rule.
pub fn rule_id(d: &Dispatcher, r: RuleRef) -> RuleId {
    RuleId {
        table: d.tables()[r.table].name.clone(),
        record: r.record,
        label: r.label,
    }
}

/// Verdict at a key with the winner (or the blocking rule) mapped through an identity function.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WinnerState<I> {
    Covered(I),
    Uncovered,
    Blocked(I),
    Undecided(I),
    OutsideChart,
}

/// Map a dispatch result to its [`WinnerState`].
pub fn winner_state<I>(
    d: &Dispatcher,
    x: &Dispatch,
    id: &impl Fn(&Dispatcher, RuleRef) -> I,
) -> WinnerState<I> {
    match x {
        Dispatch::Covered(r) => WinnerState::Covered(id(d, *r)),
        Dispatch::Uncovered => WinnerState::Uncovered,
        Dispatch::Blocked { at, .. } => WinnerState::Blocked(id(d, *at)),
        Dispatch::Undecided { at, .. } => WinnerState::Undecided(id(d, *at)),
        Dispatch::OutsideChart { .. } => WinnerState::OutsideChart,
    }
}

/// Result of [`usage`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsageReport {
    pub keys: u64,
    pub covered: u64,
    pub uncovered: u64,
    pub blocked: u64,
    pub undecided: u64,
    pub outside_chart: u64,
    /// Wins per table (indexed like the dispatcher's table list).
    pub wins_per_table: Vec<u64>,
    /// (table, record) -> number of keys it serves (first-match winner).
    pub served: BTreeMap<(usize, usize), u64>,
    /// The open keys with their verdict, in stream order (only when requested).
    pub open_keys: Vec<(Key, Dispatch)>,
}

impl UsageReport {
    /// Number of used rules (rules that win at least one key).
    pub fn used_rules(&self) -> usize {
        self.served.len()
    }
    /// Keys that are not covered.
    pub fn open(&self) -> u64 {
        self.uncovered + self.blocked + self.undecided + self.outside_chart
    }
}

/// First-match dispatch over a key stream: verdict counts, wins per table, served keys per rule (the used set of
/// R20), and optionally the open keys.
pub fn usage<I: IntoIterator<Item = Key>>(
    d: &Dispatcher,
    keys: I,
    o: &StreamOptions,
    keep_open: bool,
) -> Result<UsageReport, CoverError> {
    let mut rep = UsageReport {
        wins_per_table: vec![0; d.tables().len()],
        ..Default::default()
    };
    let mut err = None;
    for_each_chunk(
        keys,
        o,
        |c| {
            c.iter()
                .map(|k| d.first_match(k))
                .collect::<Result<Vec<_>, _>>()
        },
        |c, r| {
            if err.is_some() {
                return;
            }
            let r = match r {
                Ok(r) => r,
                Err(e) => {
                    err = Some(e);
                    return;
                }
            };
            for (k, x) in c.into_iter().zip(r) {
                rep.keys += 1;
                match &x {
                    Dispatch::Covered(w) => {
                        rep.covered += 1;
                        rep.wins_per_table[w.table] += 1;
                        *rep.served.entry((w.table, w.record)).or_insert(0) += 1;
                        continue;
                    }
                    Dispatch::Uncovered => rep.uncovered += 1,
                    Dispatch::Blocked { .. } => rep.blocked += 1,
                    Dispatch::Undecided { .. } => rep.undecided += 1,
                    Dispatch::OutsideChart { .. } => rep.outside_chart += 1,
                }
                if keep_open {
                    rep.open_keys.push((k, x));
                }
            }
        },
    );
    match err {
        Some(e) => Err(e),
        None => Ok(rep),
    }
}

/// One (old state -> new state) group of a winner diff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiffGroup {
    pub count: u64,
    /// The keys in stream order (empty unless `keep_keys`).
    pub keys: Vec<Key>,
}

/// Result of [`winner_diff`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WinnerDiff<I: Ord> {
    pub keys: u64,
    pub changed: u64,
    pub groups: BTreeMap<(WinnerState<I>, WinnerState<I>), DiffGroup>,
}

/// The keys whose first-match verdict or winner identity differs between `old` and `new` (plan 2.4.7 D6), grouped
/// by (old state, new state). `id` maps a rule of either dispatcher to a comparable identity ([`rule_id`], a
/// table name for a piece-level diff, a canonical-domain id for a domain-level diff).
pub fn winner_diff<I, K, F>(
    old: &Dispatcher,
    new: &Dispatcher,
    keys: K,
    id: F,
    o: &StreamOptions,
    keep_keys: bool,
) -> Result<WinnerDiff<I>, CoverError>
where
    I: Ord + Clone + Send,
    K: IntoIterator<Item = Key>,
    F: Fn(&Dispatcher, RuleRef) -> I + Sync,
{
    let mut out = WinnerDiff {
        keys: 0,
        changed: 0,
        groups: BTreeMap::new(),
    };
    let mut err = None;
    type Changes<I> = Vec<(usize, WinnerState<I>, WinnerState<I>)>;
    for_each_chunk(
        keys,
        o,
        |c| -> Result<Changes<I>, CoverError> {
            let mut ch = vec![];
            for (i, k) in c.iter().enumerate() {
                let a = winner_state(old, &old.first_match(k)?, &id);
                let b = winner_state(new, &new.first_match(k)?, &id);
                if a != b {
                    ch.push((i, a, b));
                }
            }
            Ok(ch)
        },
        |c, r| {
            if err.is_some() {
                return;
            }
            out.keys += c.len() as u64;
            match r {
                Err(e) => err = Some(e),
                Ok(ch) => {
                    for (i, a, b) in ch {
                        out.changed += 1;
                        let g = out.groups.entry((a, b)).or_default();
                        g.count += 1;
                        if keep_keys {
                            g.keys.push(c[i].clone());
                        }
                    }
                }
            }
        },
    );
    match err {
        Some(e) => Err(e),
        None => Ok(out),
    }
}
