//! The search for small exact combinations of symmetry rows at a key (module documentation in `tworow/mod.rs`).
//!
//! Per key: (A) scan: the expanded rows of every element at the key and at every shifted seed of the neighbourhood,
//! keeping per row only whether it contains the key and its NON-LOWER terms (terms other than the key that are not
//! strictly lower under the comparator); (B) pool: the rows containing the key, then the rows sharing a non-lower term
//! with the pool, `max_rows - 1` times; (C) the pool rows in full; (D) depth-first enumeration of row subsets of at most
//! `max_rows` rows (at most `elements_per_combination` elements) in which every non-lower term occurs in at least two
//! rows (necessary for it to cancel), each decided by an exact RREF with the columns in comparator order (highest
//! first, the identity carried along for the weights): valid iff the key's column is a pivot. A valid subset is not
//! extended (minimal subsets). COMPLETENESS (for minimal combinations of at most `max_rows` rows in the neighbourhood):
//! in a minimal valid set every row has a non-lower term (else it is removable or a valid single row) and the rows are
//! linked to a row containing the key through shared non-lower terms (a component without the key could be removed);
//! the enumeration branches over every row containing a term that is not yet cancelled, so it reaches every such set.
//! Every kept relation is re-derived by [`apply_combination`] (the weighted sum computed directly, not through the
//! echelon form) and must agree term by term.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::rows::{RowsError, rows_pruned};
use super::{
    Combination, KeyOutcome, KeyResult, PairShift, RowRef, RowSpec, SearchStats, TwoRowPolicy,
};
use crate::import::symmetry::{RelationCheck, verify_relation_with};
use crate::replay_base::algebra::{CheckAlgebra, Q};
use crate::replay_base::symrows::VerifiedGroup;

/// The node comparator: `cmp(a, b) == Less` iff `a` is lower than `b`.
pub type Cmp<'a> = &'a dyn Fn(&[i32], &[i32]) -> Ordering;
/// The caller's zero-sector predicate, a DOWNSET in the positive lines (terms it accepts are pruned from every row
/// during the expansion, [`super::rows`]); `None` = keep every term.
pub type ZeroSector<'a> = Option<&'a dyn Fn(&[i32]) -> bool>;

/// Why a given shape does not give a relation at a key ([`apply_combination`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reject {
    /// Malformed shape: empty, weight count, a shift that does not apply or leaves the chart.
    Shape(String),
    /// The element gives no row at that seed (a numerator image not usable there, or the row cancels).
    NoRow {
        element: usize,
        seed: Vec<i32>,
    },
    OverCap {
        seed: Vec<i32>,
    },
    Undecided(String),
    /// The coefficient of the key in the combination is exactly 0.
    KeyCancels,
    NotLower {
        term: Vec<i32>,
    },
}

impl std::fmt::Display for Reject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reject::Shape(m) => write!(f, "shape: {m}"),
            Reject::NoRow { element, seed } => write!(f, "no row of g{element} at {seed:?}"),
            Reject::OverCap { seed } => write!(f, "row over the term cap at {seed:?}"),
            Reject::Undecided(m) => write!(f, "undecided: {m}"),
            Reject::KeyCancels => write!(f, "the key cancels"),
            Reject::NotLower { term } => write!(f, "term {term:?} not strictly lower"),
        }
    }
}

enum SeedRows {
    Rows(Vec<(usize, Vec<(Vec<i32>, Q)>)>),
    OverCap,
    Undecided(String),
}

/// The expanded rows at one seed (one per element that acts), zero-sector terms pruned by the caller's predicate
/// ([`super::rows::rows_pruned`]; without a predicate exactly `rows_at_with`'s rows).
fn rows_for_seed<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    seed: &[i32],
    max_terms: usize,
    zero: ZeroSector,
) -> SeedRows {
    match rows_pruned(alg, group, seed, max_terms, zero) {
        Ok(r) => SeedRows::Rows(r),
        Err(RowsError::OverCap { .. }) => SeedRows::OverCap,
        Err(RowsError::Undecided(e)) => SeedRows::Undecided(e),
    }
}

/// A key of the chart's top sector: every line >= 1, every other slot <= 0.
fn in_chart(k: &[i32], chart: &[bool]) -> bool {
    k.iter()
        .zip(chart)
        .all(|(&v, &l)| if l { v >= 1 } else { v <= 0 })
}

/// Interned term keys.
struct Terms {
    ids: HashMap<Vec<i32>, u32>,
    keys: Vec<Vec<i32>>,
}

impl Terms {
    fn id(&mut self, k: &[i32]) -> u32 {
        if let Some(&i) = self.ids.get(k) {
            return i;
        }
        let i = self.keys.len() as u32;
        self.keys.push(k.to_vec());
        self.ids.insert(k.to_vec(), i);
        i
    }
}

struct Info {
    element: usize,
    seed: usize,
    has_k: bool,
    bad: Vec<u32>,
}

struct PRow {
    element: usize,
    seed: usize,
    terms: Vec<(u32, Q)>,
    bad: Vec<u32>,
    has_k: bool,
    kcoef: Q,
}

/// One key (see the module documentation). `zero` drops zero-sector terms from every row (the caller's knowledge).
pub fn search_key<A: CheckAlgebra>(
    alg: &A,
    key: &[i32],
    group: &VerifiedGroup,
    cmp: Cmp,
    policy: &TwoRowPolicy,
    zero: ZeroSector,
) -> KeyResult {
    let mut stats = SearchStats::default();
    let refused = |reason: String| KeyResult {
        key: key.to_vec(),
        outcome: KeyOutcome::Refused { reason },
        stats: SearchStats::default(),
    };
    let g = group.group();
    if key.len() != g.arity {
        return refused(format!("key of length {} for arity {}", key.len(), g.arity));
    }
    if let Err(e) = policy.validate() {
        return refused(e);
    }
    let shifts = match policy.shifts(g.arity) {
        Ok(s) => s,
        Err(e) => return refused(e),
    };
    let nel = g
        .elements
        .len()
        .min(policy.max_elements.unwrap_or(usize::MAX));
    stats.elements = nel;
    stats.shifts = shifts.len();
    // seeds: the key, then the shifted seeds (distinct, in generation order)
    let mut seeds: Vec<(Option<PairShift>, Vec<i32>)> = vec![(None, key.to_vec())];
    let mut seen: HashSet<Vec<i32>> = HashSet::from([key.to_vec()]);
    if policy.max_rows > 1 || policy.single_rows {
        for s in shifts {
            let Some(k2) = s.apply(key) else { continue };
            if policy.keep_in_chart && !in_chart(&k2, &g.chart) {
                continue;
            }
            if seen.insert(k2.clone()) {
                seeds.push((Some(s), k2));
            }
        }
    }
    stats.seeds = seeds.len();

    // (A) scan
    let mut terms = Terms {
        ids: HashMap::new(),
        keys: vec![],
    };
    let kid = terms.id(key);
    let mut infos: Vec<Info> = vec![];
    let mut undecided_reason: Option<String> = None;
    for (si, (_, s)) in seeds.iter().enumerate() {
        match rows_for_seed(alg, group, s, policy.max_terms_per_row, zero) {
            SeedRows::OverCap => stats.seeds_over_cap += 1,
            SeedRows::Undecided(e) => {
                stats.undecided += 1;
                undecided_reason.get_or_insert(e);
            }
            SeedRows::Rows(rows) => {
                for (e, v) in rows {
                    if e >= nel {
                        continue;
                    }
                    stats.rows += 1;
                    let mut has_k = false;
                    let mut bad = vec![];
                    for (t, _) in &v {
                        if t.as_slice() == key {
                            has_k = true;
                        } else if cmp(t, key) != Ordering::Less {
                            bad.push(terms.id(t));
                        }
                    }
                    bad.sort_unstable();
                    bad.dedup();
                    infos.push(Info {
                        element: e,
                        seed: si,
                        has_k,
                        bad,
                    });
                }
            }
        }
    }

    // (B) pool: rows containing the key, then rows sharing a non-lower term, max_rows - 1 links
    let mut by_bad: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, r) in infos.iter().enumerate() {
        for &t in &r.bad {
            by_bad.entry(t).or_default().push(i);
        }
    }
    let mut in_pool = vec![false; infos.len()];
    let mut frontier: Vec<usize> = (0..infos.len()).filter(|&i| infos[i].has_k).collect();
    let mut pool: Vec<usize> = frontier.clone();
    for &i in &frontier {
        in_pool[i] = true;
    }
    let mut linked: HashSet<u32> = HashSet::new();
    for _ in 1..policy.max_rows {
        let mut next = vec![];
        for &i in &frontier {
            for &t in &infos[i].bad {
                if linked.insert(t) {
                    for &j in by_bad.get(&t).map(Vec::as_slice).unwrap_or(&[]) {
                        if !in_pool[j] {
                            in_pool[j] = true;
                            next.push(j);
                        }
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_unstable();
        pool.extend(&next);
        frontier = next;
    }
    if pool.len() > policy.max_pool_rows {
        pool.truncate(policy.max_pool_rows);
        stats.pool_truncated = true;
    }
    stats.pool_rows = pool.len();
    drop(by_bad);

    // (C) the pool rows in full (recomputed per seed: the scan kept only the non-lower terms)
    let mut need: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for &i in &pool {
        need.entry(infos[i].seed)
            .or_default()
            .insert(infos[i].element);
    }
    let mut full: HashMap<(usize, usize), Vec<(u32, Q)>> = HashMap::new();
    for (&si, els) in &need {
        match rows_for_seed(alg, group, &seeds[si].1, policy.max_terms_per_row, zero) {
            SeedRows::Rows(rows) => {
                for (e, v) in rows {
                    if els.contains(&e) {
                        let row = v.iter().map(|(t, c)| (terms.id(t), *c)).collect();
                        full.insert((si, e), row);
                    }
                }
            }
            // deterministic: the scan computed the same seed; fail closed if it does not repeat
            SeedRows::OverCap => stats.seeds_over_cap += 1,
            SeedRows::Undecided(e) => {
                stats.undecided += 1;
                undecided_reason.get_or_insert(e);
            }
        }
    }
    let prows: Vec<PRow> = pool
        .iter()
        .filter_map(|&i| {
            let r = &infos[i];
            let t = full.remove(&(r.seed, r.element))?;
            let kcoef = t
                .iter()
                .find(|(id, _)| *id == kid)
                .map(|(_, c)| *c)
                .unwrap_or(Q::ZERO);
            Some(PRow {
                element: r.element,
                seed: r.seed,
                terms: t,
                bad: r.bad.clone(),
                has_k: r.has_k,
                kcoef,
            })
        })
        .collect();
    drop(full);
    drop(infos);
    // global rank of every term of the pool (comparator order, lowest = 0)
    let mut used: Vec<u32> = prows
        .iter()
        .flat_map(|r| r.terms.iter().map(|(t, _)| *t))
        .collect();
    used.sort_unstable();
    used.dedup();
    used.sort_by(|&a, &b| cmp(&terms.keys[a as usize], &terms.keys[b as usize]));
    let mut rank = vec![u32::MAX; terms.keys.len()];
    for (r, &t) in used.iter().enumerate() {
        rank[t as usize] = r as u32;
    }

    // (D) enumeration
    let mut bad_index: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, r) in prows.iter().enumerate() {
        for &t in &r.bad {
            bad_index.entry(t).or_default().push(i);
        }
    }
    let mut en = Enumerator {
        alg,
        prows: &prows,
        bad_index: &bad_index,
        kid,
        rank: &rank,
        keys: &terms.keys,
        key,
        cmp,
        policy,
        seeds: &seeds,
        visited: HashSet::new(),
        found: vec![],
        seen_rel: HashSet::new(),
        stats: &mut stats,
    };
    for a in 0..prows.len() {
        if prows[a].has_k {
            en.dfs(vec![a]);
        }
    }
    let mut found = std::mem::take(&mut en.found);
    drop(en);
    stats.found = found.len();
    // ranking: fewest RHS terms, fewer rows, lower highest RHS key, elements, seeds (generation order)
    found.sort_by(|(sa, a), (sb, b)| {
        a.rhs
            .len()
            .cmp(&b.rhs.len())
            .then(a.rows.len().cmp(&b.rows.len()))
            .then_with(|| match (a.rhs.first(), b.rhs.first()) {
                (Some(x), Some(y)) => cmp(&x.0, &y.0),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
            .then_with(|| a.elements().cmp(&b.elements()))
            .then_with(|| sa.cmp(sb))
    });
    // re-derivation of every kept relation by the direct weighted sum (not the echelon form)
    let mut kept = vec![];
    for (_, c) in found {
        if kept.len() >= policy.max_relations {
            break;
        }
        match apply_combination(alg, group, key, &c.shape(), &c.weights, cmp, policy, zero) {
            Ok(d) if same_relation(&c, &d) => kept.push(c),
            Ok(_) => {
                stats.undecided += 1;
                undecided_reason
                    .get_or_insert("re-derivation differs from the echelon form".into());
            }
            Err(e) => {
                stats.undecided += 1;
                undecided_reason.get_or_insert(format!("re-derivation: {e}"));
            }
        }
    }
    let outcome = if !kept.is_empty() {
        KeyOutcome::Served { relations: kept }
    } else if stats.seeds_over_cap > 0 || stats.pool_truncated || stats.subsets_budget_hit {
        let mut what = vec![];
        if stats.seeds_over_cap > 0 {
            what.push(format!(
                "{} seed(s) with a row over max_terms_per_row {}",
                stats.seeds_over_cap, policy.max_terms_per_row
            ));
        }
        if stats.pool_truncated {
            what.push(format!(
                "pool truncated at max_pool_rows {}",
                policy.max_pool_rows
            ));
        }
        if stats.subsets_budget_hit {
            what.push(format!("max_subsets {} reached", policy.max_subsets));
        }
        KeyOutcome::OverCap {
            what: what.join("; "),
        }
    } else if stats.undecided > 0 {
        KeyOutcome::Undecided {
            reason: undecided_reason.unwrap_or_default(),
        }
    } else {
        KeyOutcome::NoneInNeighbourhood
    };
    KeyResult {
        key: key.to_vec(),
        outcome,
        stats,
    }
}

fn same_relation(a: &Combination, b: &Combination) -> bool {
    let s = |c: &Combination| -> BTreeMap<Vec<i32>, Q> { c.rhs.iter().cloned().collect() };
    s(a) == s(b) && a.weights == b.weights
}

struct Enumerator<'x, A: CheckAlgebra> {
    alg: &'x A,
    prows: &'x [PRow],
    bad_index: &'x HashMap<u32, Vec<usize>>,
    kid: u32,
    rank: &'x [u32],
    keys: &'x [Vec<i32>],
    key: &'x [i32],
    cmp: Cmp<'x>,
    policy: &'x TwoRowPolicy,
    seeds: &'x [(Option<PairShift>, Vec<i32>)],
    visited: HashSet<Vec<usize>>,
    /// (seed indices of the rows, relation)
    found: Vec<(Vec<usize>, Combination)>,
    seen_rel: HashSet<Vec<(u32, Q)>>,
    stats: &'x mut SearchStats,
}

impl<A: CheckAlgebra> Enumerator<'_, A> {
    fn elements_ok(&self, s: &[usize], j: usize) -> bool {
        let mut el: BTreeSet<usize> = s.iter().map(|&i| self.prows[i].element).collect();
        el.insert(self.prows[j].element);
        el.len() <= self.policy.elements_per_combination
    }

    fn dfs(&mut self, s: Vec<usize>) {
        if self.stats.subsets_budget_hit || self.visited.contains(&s) {
            return;
        }
        if self.visited.len() >= self.policy.max_subsets {
            self.stats.subsets_budget_hit = true;
            return;
        }
        self.visited.insert(s.clone());
        let mut count: BTreeMap<u32, usize> = BTreeMap::new();
        for &i in &s {
            for &t in &self.prows[i].bad {
                *count.entry(t).or_default() += 1;
            }
        }
        let uncovered: Vec<u32> = count
            .iter()
            .filter(|(_, c)| **c == 1)
            .map(|(t, _)| *t)
            .collect();
        let mut branch: BTreeSet<usize> = BTreeSet::new();
        if uncovered.is_empty() {
            if self.test(&s) {
                return; // valid: minimal, not extended
            }
            if s.len() < self.policy.max_rows {
                // a row linked to the set by a non-lower term, or containing the key (with a non-lower term)
                for t in count.keys() {
                    branch.extend(self.bad_index.get(t).into_iter().flatten().copied());
                }
                branch.extend(
                    (0..self.prows.len())
                        .filter(|&j| self.prows[j].has_k && !self.prows[j].bad.is_empty()),
                );
            }
        } else if s.len() < self.policy.max_rows {
            // the smallest term not yet cancelled must occur in a further row
            branch.extend(
                self.bad_index
                    .get(&uncovered[0])
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        for j in branch {
            if s.contains(&j) || !self.elements_ok(&s, j) {
                continue;
            }
            let mut s2 = s.clone();
            s2.push(j);
            s2.sort_unstable();
            self.dfs(s2);
        }
    }

    /// The exact RREF test of a subset; records a valid relation. Returns whether the subset is valid.
    fn test(&mut self, s: &[usize]) -> bool {
        let alg = self.alg;
        let mut cols: Vec<u32> = s
            .iter()
            .flat_map(|&i| self.prows[i].terms.iter().map(|(t, _)| *t))
            .collect();
        cols.sort_unstable();
        cols.dedup();
        cols.sort_by(|&a, &b| self.rank[b as usize].cmp(&self.rank[a as usize]));
        let pos: HashMap<u32, usize> = cols.iter().enumerate().map(|(i, &t)| (t, i)).collect();
        let Some(&tcol) = pos.get(&self.kid) else {
            return false;
        };
        let n = cols.len();
        let m = s.len();
        let dense: Vec<Vec<Q>> = s
            .iter()
            .enumerate()
            .map(|(r, &i)| {
                let mut row = vec![Q::ZERO; n + m];
                for (t, c) in &self.prows[i].terms {
                    row[pos[t]] = *c;
                }
                row[n + r] = Q::ONE;
                row
            })
            .collect();
        self.stats.subsets_tested += 1;
        let (rref, piv) = match alg.q_rref(&dense, n) {
            Ok(x) => x,
            Err(_) => {
                self.stats.undecided += 1;
                return false;
            }
        };
        let Some(pr) = piv.iter().position(|&p| p == tcol) else {
            return false;
        };
        let row = &rref[pr];
        if row[..tcol].iter().any(|c| !c.is_zero()) {
            self.stats.undecided += 1; // impossible in an echelon form: fail closed
            return false;
        }
        let mut rhs = vec![];
        let mut ident = vec![];
        for j in tcol + 1..n {
            if row[j].is_zero() {
                continue;
            }
            let k = &self.keys[cols[j] as usize];
            if (self.cmp)(k, self.key) != Ordering::Less {
                self.stats.not_lower += 1;
                return false;
            }
            let Ok(c) = alg.q_neg(row[j]) else {
                self.stats.undecided += 1;
                return false;
            };
            ident.push((cols[j], c));
            rhs.push((k.clone(), c));
        }
        if !self.seen_rel.insert(ident) {
            return true; // the same relation from another subset
        }
        if m == 1 && !self.policy.single_rows {
            return true;
        }
        let comb = Combination {
            key: self.key.to_vec(),
            rows: s
                .iter()
                .map(|&i| {
                    let r = &self.prows[i];
                    RowRef {
                        spec: RowSpec {
                            element: r.element,
                            shift: self.seeds[r.seed].0.clone(),
                        },
                        seed: self.seeds[r.seed].1.clone(),
                        terms: r.terms.len(),
                    }
                })
                .collect(),
            weights: row[n..].to_vec(),
            key_coefficients: s.iter().map(|&i| self.prows[i].kcoef).collect(),
            rhs,
        };
        let seeds_of: Vec<usize> = s.iter().map(|&i| self.prows[i].seed).collect();
        self.found.push((seeds_of, comb));
        true
    }
}

/// The search at every key (input order). See [`search_key`].
pub fn two_row_relations<A: CheckAlgebra>(
    alg: &A,
    keys: &[Vec<i32>],
    group: &VerifiedGroup,
    cmp: Cmp,
    policy: &TwoRowPolicy,
    zero: ZeroSector,
) -> Vec<KeyResult> {
    keys.iter()
        .map(|k| search_key(alg, k, group, cmp, policy, zero))
        .collect()
}

/// ONE shape with given weights at one key, by the direct weighted sum of the rows (no echelon form): the region
/// verification and the re-derivation of every found relation. The weights are renormalised so that the key has
/// coefficient exactly 1; `KeyCancels` when it is 0; `NotLower` names the first term that is not strictly lower.
#[allow(clippy::too_many_arguments)]
pub fn apply_combination<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    key: &[i32],
    shape: &[RowSpec],
    weights: &[Q],
    cmp: Cmp,
    policy: &TwoRowPolicy,
    zero: ZeroSector,
) -> Result<Combination, Reject> {
    if shape.is_empty() || shape.len() != weights.len() {
        return Err(Reject::Shape(format!(
            "{} rows, {} weights",
            shape.len(),
            weights.len()
        )));
    }
    let chart = &group.group().chart;
    if key.len() != chart.len() {
        return Err(Reject::Shape(format!(
            "key of length {} for arity {}",
            key.len(),
            chart.len()
        )));
    }
    let und = |e: String| Reject::Undecided(e);
    let mut acc: BTreeMap<Vec<i32>, Q> = BTreeMap::new();
    let mut rows = vec![];
    let mut kc = vec![];
    for (spec, w) in shape.iter().zip(weights) {
        let seed = match &spec.shift {
            None => key.to_vec(),
            Some(s) => s
                .apply(key)
                .ok_or_else(|| Reject::Shape(format!("shift {} does not apply", s.text())))?,
        };
        if spec.shift.is_some() && policy.keep_in_chart && !in_chart(&seed, chart) {
            return Err(Reject::Shape(format!(
                "shifted seed {seed:?} leaves the chart"
            )));
        }
        let row = match rows_for_seed(alg, group, &seed, policy.max_terms_per_row, zero) {
            SeedRows::OverCap => return Err(Reject::OverCap { seed }),
            SeedRows::Undecided(e) => return Err(und(e)),
            SeedRows::Rows(r) => r
                .into_iter()
                .find(|(e, _)| *e == spec.element)
                .map(|(_, v)| v)
                .ok_or(Reject::NoRow {
                    element: spec.element,
                    seed: seed.clone(),
                })?,
        };
        kc.push(
            row.iter()
                .find(|(t, _)| t.as_slice() == key)
                .map(|(_, c)| *c)
                .unwrap_or(Q::ZERO),
        );
        for (t, c) in &row {
            let v = alg.q_mul(*w, *c).map_err(und)?;
            let e = acc.entry(t.clone()).or_insert(Q::ZERO);
            *e = alg.q_add(*e, v).map_err(und)?;
        }
        rows.push(RowRef {
            spec: spec.clone(),
            seed,
            terms: row.len(),
        });
    }
    acc.retain(|_, c| !c.is_zero());
    let ck = acc.remove(key).unwrap_or(Q::ZERO);
    if ck.is_zero() {
        return Err(Reject::KeyCancels);
    }
    let mut rhs = vec![];
    for (t, c) in acc {
        if cmp(&t, key) != Ordering::Less {
            return Err(Reject::NotLower { term: t });
        }
        let v = alg.q_div(c, ck).map_err(und)?;
        rhs.push((t, alg.q_neg(v).map_err(und)?));
    }
    rhs.sort_by(|a, b| cmp(&b.0, &a.0));
    let weights = weights
        .iter()
        .map(|w| alg.q_div(*w, ck))
        .collect::<Result<Vec<Q>, _>>()
        .map_err(und)?;
    Ok(Combination {
        key: key.to_vec(),
        rows,
        weights,
        key_coefficients: kc,
        rhs,
    })
}

/// ROUTE (A) of the independent check: the import's relation verifier (`import::symmetry::verify_relation_with`) on
/// the combination's relation with the combination's seeds: the relation vector must lie in the span of the rows (of
/// every element) at those seeds. Shares the row derivation with the search; valid only for relations searched
/// WITHOUT a zero-sector drop (a dropped term is not in the row span).
pub fn import_check<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    comb: &Combination,
    max_terms: usize,
) -> RelationCheck {
    let mut seeds: Vec<Vec<i32>> = vec![];
    for s in comb.seeds() {
        if !seeds.contains(&s) {
            seeds.push(s);
        }
    }
    verify_relation_with(alg, group, &comb.key, &comb.rhs_text(), &seeds, max_terms)
}
