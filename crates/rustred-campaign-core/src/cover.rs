//! Coverage of keys by prioritized rule tables: first-match dispatch (the default) and, as a separate and labelled
//! mode, union coverage.
//!
//! - A [`Table`] is a named list of [`CaseDomain`]s in record order, each with a label (the record's `i`, `index` or
//!   bank id) so that every credit names its rule.
//! - The PRIORITY of the tables is data given by the caller ([`Dispatcher::new`]; DESIGN-COMMENTS B6). Dispatch of a key
//!   = the first table in priority order that has an applying rule, and in it the first applying record.
//! - A rule whose verdict is [`Verdict::Elided`] or [`Verdict::Error`] before any applying rule makes the dispatch
//!   [`Dispatch::Undecided`] (fail closed): the first match cannot be known.
//! - [`Dispatcher::union_cover`] lists EVERY applying rule (union semantics: no dispatch order). It is what the
//!   checker's partition tools call "covered"; it is NOT the engine's dispatch.
//!
//! Index: the records of all tables are flattened in dispatch order and grouped by their fixed-slot pattern; per
//! pattern a hash map from the fixed values to the record list. For a key the candidate records are the union of the
//! lists of its projections, evaluated in dispatch order until the first applying one.

use std::collections::HashMap;
use std::time::Instant;

use crate::domain::{CaseDomain, Verdict};

/// A named rule table.
#[derive(Clone, Debug)]
pub struct Table {
    pub name: String,
    pub domains: Vec<CaseDomain>,
    /// One label per domain (record `i`, `index` or bank id).
    pub labels: Vec<i64>,
}

impl Table {
    pub fn new(name: impl Into<String>) -> Self {
        Table { name: name.into(), domains: vec![], labels: vec![] }
    }
    pub fn push(&mut self, label: i64, d: CaseDomain) {
        self.labels.push(label);
        self.domains.push(d);
    }
    pub fn len(&self) -> usize {
        self.domains.len()
    }
    pub fn is_empty(&self) -> bool {
        self.domains.is_empty()
    }
}

/// A credited rule: table (index into the dispatcher's table list), record position in the table, record label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleRef {
    pub table: usize,
    pub record: usize,
    pub label: i64,
}

/// Result of first-match dispatch at one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dispatch {
    Covered(RuleRef),
    Uncovered,
    /// The rule at `at` could not be decided (elided witness loci, evaluation error) before any rule applied.
    Undecided { at: RuleRef, verdict: Verdict },
}

impl Dispatch {
    pub fn winner(&self) -> Option<RuleRef> {
        match self {
            Dispatch::Covered(r) => Some(*r),
            _ => None,
        }
    }
}

/// Errors of the dispatcher setup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoverError {
    /// The priority list is not a list of distinct valid table indices.
    BadPriority(String),
    /// Tables of different arity.
    Arity(String),
}

impl std::fmt::Display for CoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoverError::BadPriority(s) => write!(f, "bad priority: {s}"),
            CoverError::Arity(s) => write!(f, "arity: {s}"),
        }
    }
}
impl std::error::Error for CoverError {}

struct Pattern {
    slots: Vec<usize>,
    map: HashMap<Vec<i32>, Vec<u32>>,
}

/// Prioritized tables with a fixed-slot index.
pub struct Dispatcher {
    tables: Vec<Table>,
    priority: Vec<usize>,
    /// Flattened entries in dispatch order: (table, record).
    entries: Vec<(u32, u32)>,
    patterns: Vec<Pattern>,
    arity: usize,
}

/// Batch result.
#[derive(Clone, Debug)]
pub struct BatchReport {
    pub results: Vec<Dispatch>,
    /// Wins per table (indexed like the table list, not the priority).
    pub wins: Vec<usize>,
    pub covered: usize,
    pub uncovered: usize,
    pub undecided: usize,
    pub seconds: f64,
}

impl Dispatcher {
    /// `priority` lists table indices, highest priority first; every table listed exactly once is NOT required
    /// (tables not listed are not used), but no index may repeat.
    pub fn new(tables: Vec<Table>, priority: Vec<usize>) -> Result<Self, CoverError> {
        let mut seen = vec![false; tables.len()];
        for &p in &priority {
            if p >= tables.len() || seen[p] {
                return Err(CoverError::BadPriority(format!("{priority:?} for {} tables", tables.len())));
            }
            seen[p] = true;
        }
        let arity = tables.iter().flat_map(|t| t.domains.iter()).map(|d| d.arity).next().unwrap_or(0);
        if tables.iter().flat_map(|t| t.domains.iter()).any(|d| d.arity != arity) {
            return Err(CoverError::Arity("domains of different arity".into()));
        }
        let mut entries = vec![];
        for &t in &priority {
            for r in 0..tables[t].len() {
                entries.push((t as u32, r as u32));
            }
        }
        let mut pidx: HashMap<Vec<usize>, usize> = HashMap::new();
        let mut patterns: Vec<Pattern> = vec![];
        for (e, &(t, r)) in entries.iter().enumerate() {
            let d = &tables[t as usize].domains[r as usize];
            let slots = d.fixed_slots();
            let vals: Vec<i32> = slots.iter().map(|&s| d.fixed[s].expect("fixed")).collect();
            let pi = *pidx.entry(slots.clone()).or_insert_with(|| {
                patterns.push(Pattern { slots, map: HashMap::new() });
                patterns.len() - 1
            });
            patterns[pi].map.entry(vals).or_default().push(e as u32);
        }
        Ok(Dispatcher { tables, priority, entries, patterns, arity })
    }

    /// The same tables under another priority.
    pub fn with_priority(&self, priority: Vec<usize>) -> Result<Self, CoverError> {
        Dispatcher::new(self.tables.clone(), priority)
    }
    pub fn tables(&self) -> &[Table] {
        &self.tables
    }
    pub fn priority(&self) -> &[usize] {
        &self.priority
    }
    pub fn arity(&self) -> usize {
        self.arity
    }
    pub fn n_patterns(&self) -> usize {
        self.patterns.len()
    }

    fn candidates(&self, key: &[i32], out: &mut Vec<u32>) {
        out.clear();
        let mut proj = Vec::with_capacity(self.arity);
        for p in &self.patterns {
            proj.clear();
            proj.extend(p.slots.iter().map(|&s| key[s]));
            if let Some(v) = p.map.get(&proj) {
                out.extend_from_slice(v);
            }
        }
        out.sort_unstable();
    }

    fn rref(&self, e: u32) -> RuleRef {
        let (t, r) = self.entries[e as usize];
        RuleRef { table: t as usize, record: r as usize, label: self.tables[t as usize].labels[r as usize] }
    }

    /// First-match dispatch at `key`.
    pub fn first_match(&self, key: &[i32]) -> Dispatch {
        let mut c = Vec::new();
        self.first_match_with(key, &mut c)
    }

    fn first_match_with(&self, key: &[i32], c: &mut Vec<u32>) -> Dispatch {
        if key.len() != self.arity {
            return Dispatch::Uncovered;
        }
        self.candidates(key, c);
        for &e in c.iter() {
            let (t, r) = self.entries[e as usize];
            match self.tables[t as usize].domains[r as usize].applies(key) {
                Verdict::Applies => return Dispatch::Covered(self.rref(e)),
                v @ (Verdict::Elided | Verdict::Error(_)) => return Dispatch::Undecided { at: self.rref(e), verdict: v },
                _ => {}
            }
        }
        Dispatch::Uncovered
    }

    /// UNION coverage (not dispatch): every rule that applies at `key`, in priority order. Rules with an
    /// `Elided`/`Error` verdict are returned in the second list.
    pub fn union_cover(&self, key: &[i32]) -> (Vec<RuleRef>, Vec<(RuleRef, Verdict)>) {
        let mut c = Vec::new();
        let (mut yes, mut undecided) = (vec![], vec![]);
        if key.len() != self.arity {
            return (yes, undecided);
        }
        self.candidates(key, &mut c);
        for &e in &c {
            let (t, r) = self.entries[e as usize];
            match self.tables[t as usize].domains[r as usize].applies(key) {
                Verdict::Applies => yes.push(self.rref(e)),
                v @ (Verdict::Elided | Verdict::Error(_)) => undecided.push((self.rref(e), v)),
                _ => {}
            }
        }
        (yes, undecided)
    }

    /// First-match dispatch over a key list, `threads` worker threads (1..=4 recommended).
    pub fn batch(&self, keys: &[Vec<i32>], threads: usize) -> BatchReport {
        let t0 = Instant::now();
        let threads = threads.max(1);
        let chunk = keys.len().div_ceil(threads).max(1);
        let mut results: Vec<Dispatch> = Vec::with_capacity(keys.len());
        std::thread::scope(|s| {
            let hs: Vec<_> = keys
                .chunks(chunk)
                .map(|ks| {
                    s.spawn(move || {
                        let mut c = Vec::new();
                        ks.iter().map(|k| self.first_match_with(k, &mut c)).collect::<Vec<_>>()
                    })
                })
                .collect();
            for h in hs {
                results.extend(h.join().expect("worker"));
            }
        });
        let mut wins = vec![0; self.tables.len()];
        let (mut covered, mut uncovered, mut undecided) = (0, 0, 0);
        for r in &results {
            match r {
                Dispatch::Covered(w) => {
                    wins[w.table] += 1;
                    covered += 1;
                }
                Dispatch::Uncovered => uncovered += 1,
                Dispatch::Undecided { .. } => undecided += 1,
            }
        }
        BatchReport { results, wins, covered, uncovered, undecided, seconds: t0.elapsed().as_secs_f64() }
    }
}
