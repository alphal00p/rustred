//! The construction of a regular terminal set R on the band (interfaces/regbasis.md 3; notes THE MATHEMATICS):
//!
//! 1. BAND PASS (streaming, any order, chunks of `spec.chunk_keys`): per decided band key its composed vector on T
//!    (precision: every exponent `<= 0`, more when the published map is singular), pole orders on T (and on P), insertion
//!    of the POLAR part into the lattice of its rank (socle `t^q`, i.e. `+ O^n` = the closure's terminals finite), into
//!    the global lattice (pivot keys = the guaranteed fallback), into the variant lattice without the terminals (socle
//!    `t^(q+1)`: is T inside the band lattice?), caching of the candidate family's vectors.
//! 2. Per rank r: N_{<=r} (prefix sums), length z_r, elementary divisors, growth (a set regular on lower ranks fails
//!    exactly when N_{<=r} grows), complete ranks -> "regular through rank r".
//! 3. SELECTION (Nakayama): greedy over the candidates in the preference order, then the pivot keys, then the terminals:
//!    take K iff `t^q c(K)` is not in `t N + span(taken)` (membership in a Hermite form at socle `q + 1`).
//! 4. CERTIFICATES: (c1) the lattice of R equals N (index 1); (c3) v(det M) = z; (c2, streamed) every band key's
//!    coefficients `c(K) M` on R have pole order 0 (series product with M's exact expansion, a different code path).
//! 5. Needs on T, R, P; the export object (sealed, bound).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::{Value, json};
use symbolica::domains::rational::{Q, Rational};
use symbolica::poly::PolyVariable;

use rustred_campaign_core::key::Key;
use rustred_campaign_core::regbasis::need::{Availability, NeedTable, need_table};
use rustred_campaign_core::regbasis::record::{
    Binding, CONVENTION_BAND_PLUS_TERMINALS, MATRIX_CONVENTION, RECORD_SCHEMA, band_verdict, seal,
};
use rustred_campaign_core::regbasis::{
    Compose, ComposeError, ComposedVec, Preference, RegBasisSpec, candidate_family, rank_of,
    sort_by_preference,
};

use crate::valuation::D0;

use super::exact::{ExactMap, exact_map};
use super::lattice::HermiteLattice;
use super::series::{Laurent, QP, RF, expand_rf, parse_rf, parse_series, t_vars};

/// One line naming the method (records).
pub const METHOD: &str = "regbasis: band lattice N = O-span{c(K)} + O^n over O = Q[t]_(t), t = d - d0, from exact polar parts \
     (Hermite form over Q[t]/t^s, own logic on Symbolica polynomials); R = Nakayama greedy (t N + span membership) in the \
     preference order; M = A^-1 exact (numerica Matrix::inv over RationalPolynomialField); orders by lane poles' valuation";

/// Length cap of the example lists in a record (undecided keys, certificate failures): a record size bound, not data.
pub const MAX_EXAMPLES: usize = 20;

/// The published masters: `T = Q P` (`q[i][k]` = exact coefficient of P_k in T_i, text in d).
#[derive(Clone, Debug)]
pub struct Published {
    pub labels: Vec<String>,
    pub keys: Vec<Option<Key>>,
    pub q: Vec<Vec<String>>,
}

impl Published {
    /// From lane poles' master map (`valuation::basis::read_master_map`, THE reader): the map must be `T = Q P` with
    /// rows = the closure's terminals (key texts, compared exactly when they parse as keys) and columns = the masters.
    pub fn from_master_map(
        map: &crate::valuation::basis::MasterMap,
        terminals: &[Key],
    ) -> Result<Published, String> {
        if map.rows.len() != terminals.len() {
            return Err(format!(
                "master map has {} rows, the closure {} terminals (rows must be the terminals: T = Q P)",
                map.rows.len(),
                terminals.len()
            ));
        }
        for (i, r) in map.rows.iter().enumerate() {
            if let Ok(k) = rustred_campaign_core::key::parse_key(r)
                && k != terminals[i]
            {
                return Err(format!(
                    "master map row {i} is {r}, terminal {i} is {:?}",
                    terminals[i]
                ));
            }
        }
        let keys = map
            .cols
            .iter()
            .map(|c| rustred_campaign_core::key::parse_key(c).ok())
            .collect();
        Ok(Published {
            labels: map.cols.clone(),
            keys,
            q: map.entries.clone(),
        })
    }
}

/// Inputs of [`construct`].
pub struct BuildInput<'a> {
    /// Which slots are lines (the node frame).
    pub chart: &'a [bool],
    /// The closure's terminals, in the closure's order (the coordinates of every composed vector).
    pub terminals: &'a [Key],
    /// The band: the top-sector keys of the box (any order).
    pub band: &'a [Key],
    /// The box's highest top-sector rank (data); `None` = the highest rank present in `band`.
    pub band_r_max: Option<u32>,
    /// The node comparator.
    pub cmp: &'a dyn Fn(&[i32], &[i32]) -> Ordering,
    pub spec: &'a RegBasisSpec,
    pub published: Option<&'a Published>,
    pub availability: Option<&'a dyn Availability>,
    /// The export binding (node identity, box, closure statement, d0); `None` = no sealed object.
    pub binding: Option<Binding>,
}

/// Per-rank band summary.
#[derive(Clone, Debug)]
pub struct RankSummary {
    pub rank: u32,
    pub keys: usize,
    pub decided: usize,
    pub keys_with_pole_on_t: usize,
    pub max_pole_on_t: Vec<Option<u32>>,
    /// Length of N_{<=r} / O^n on T (z_r).
    pub length_t: u64,
    /// Pole exponents of N_{<=r} over O^n (descending, positive only).
    pub elementary_t: Vec<u32>,
    /// N_{<=r} strictly larger than N_{<=r-1} (a set regular through r - 1 fails at r).
    pub grows: bool,
    /// A key of rank r outside N_{<=r-1} (first in the preference order among the rank's pivot keys).
    pub new_direction_key: Option<Key>,
    pub length_p: Option<u64>,
    pub max_pole_on_p: Option<Vec<Option<u32>>>,
    /// Keys of this rank with a pole on R (certificate c2; `None` = not run).
    pub keys_with_pole_on_r: Option<usize>,
}

/// The result of [`construct`].
#[derive(Clone, Debug)]
pub struct BuildResult {
    pub regular_set: Vec<Key>,
    /// Per element of R: `terminal`, `candidate` or `pivot_fallback`.
    pub sources: Vec<String>,
    pub kept_terminals: Vec<usize>,
    pub exact: Option<ExactMap>,
    pub exact_note: Option<String>,
    /// The band lattice of all decided keys, scaled by t^q (socle q).
    pub lattice: HermiteLattice,
    pub q: u32,
    /// N_{<=r} on T per rank (scaled by t^q, socle q): seeds of the tower test.
    pub prefix_t: Vec<HermiteLattice>,
    /// N_{<=r} on P per rank (scaled by t^q_p) and q_p, when the published map was given.
    pub prefix_p: Option<(Vec<HermiteLattice>, u32)>,
    pub ranks: Vec<RankSummary>,
    pub r_max: u32,
    pub r_complete: Option<u32>,
    pub cert_lattice: Option<bool>,
    pub cert_det: Option<bool>,
    /// c4: lane poles' independent master-map check agrees with the constructed M (`None` = M not exact).
    pub cert_poles_check: Option<bool>,
    /// c5: lane tadic's Smith exponents of the exact A agree with the lattice's elementary divisors.
    pub cert_tadic_smith: Option<bool>,
    pub cert_keys_checked: Option<usize>,
    pub cert_keys_with_pole: Option<usize>,
    pub max_pole_on_r: Option<Vec<u32>>,
    pub regular: bool,
    pub verdict: String,
    /// Terminals whose unit vector lies in t N (they can never be in a regular set).
    pub terminal_in_t_n: Vec<bool>,
    /// Variant without "+ O^n": terminal i lies in the lattice of the band keys alone.
    pub terminal_in_band_lattice: Vec<bool>,
    pub needs: Vec<NeedTable>,
    pub record: Value,
    /// Canonical text of the sealed record (`None` without binding).
    pub record_text: Option<String>,
    pub undecided_examples: Vec<(Key, String)>,
    pub keys_decided: usize,
    pub keys_with_pole_on_t: usize,
}

/// The published map expanded at d0 with a lazily raised precision.
pub(crate) struct QMap {
    rf: Vec<Vec<RF>>,
    exp: Vec<Vec<Laurent>>,
    prec: i64,
    d0: Rational,
    tv: Arc<Vec<PolyVariable>>,
    pub(crate) pole: u32,
    pub(crate) m: usize,
}

impl QMap {
    pub(crate) fn new(
        texts: &[Vec<String>],
        n: usize,
        d0: &Rational,
        tv: &Arc<Vec<PolyVariable>>,
    ) -> Result<QMap, String> {
        if texts.len() != n || texts.is_empty() {
            return Err(format!("published map: {} rows, expected {n}", texts.len()));
        }
        let m = texts[0].len();
        if m == 0 || texts.iter().any(|r| r.len() != m) {
            return Err("published map: rows of unequal or zero length".into());
        }
        let rf: Vec<Vec<RF>> = texts
            .iter()
            .map(|r| r.iter().map(|t| parse_rf(t)).collect())
            .collect::<Result<_, _>>()?;
        let mut pole = 0u32;
        for r in &rf {
            for x in r {
                if let Some(v) = super::series::rf_valuation(x, d0)
                    && v < 0
                {
                    pole = pole.max((-v) as u32);
                }
            }
        }
        let mut q = QMap {
            rf,
            exp: vec![],
            prec: i64::MIN,
            d0: d0.clone(),
            tv: tv.clone(),
            pole,
            m,
        };
        q.ensure(1);
        Ok(q)
    }

    fn ensure(&mut self, prec: i64) {
        if prec <= self.prec {
            return;
        }
        self.exp = self
            .rf
            .iter()
            .map(|r| {
                r.iter()
                    .map(|x| expand_rf(x, &self.d0, prec, &self.tv))
                    .collect()
            })
            .collect();
        self.prec = prec;
    }

    /// c_P = c Q (series), precise enough for every exponent below `want`.
    pub(crate) fn apply(&mut self, c: &[Laurent], want: i64) -> Vec<Laurent> {
        let cpole = c
            .iter()
            .map(|x| -x.valuation().unwrap_or(0).min(0))
            .max()
            .unwrap_or(0);
        self.ensure(want + cpole + 1);
        (0..self.m)
            .map(|k| {
                let mut acc = Laurent::zero(i64::MAX / 4, &self.tv);
                for (i, ci) in c.iter().enumerate() {
                    acc = acc.add(&ci.mul(&self.exp[i][k]));
                }
                acc
            })
            .collect()
    }
}

/// d0 text (`a` or `a/b`) as lane tadic's `Rat`.
fn d0_rat(text: &str) -> Option<rustred_campaign_core::tadic::Rat> {
    let (a, b) = text.split_once('/').unwrap_or((text, "1"));
    rustred_campaign_core::tadic::Rat::new(a.parse().ok()?, b.parse().ok()?)
}

/// Polar-or-more scaled entries: `t^q c` truncated below `s`.
pub(crate) fn scaled(c: &[Laurent], q: u32, s: u32) -> Result<Vec<QP>, String> {
    c.iter().map(|x| x.scaled_entry(i64::from(q), s)).collect()
}

fn pole_of(c: &[Laurent]) -> u32 {
    c.iter().map(Laurent::pole_order).max().unwrap_or(0)
}

/// Pole exponents of a lattice scaled by t^q over O^n (descending, positive only).
pub(crate) fn elementary(l: &HermiteLattice, q: u32) -> Vec<u32> {
    let mut e: Vec<u32> = l
        .smith_exponents()
        .into_iter()
        .filter(|&x| x < q)
        .map(|x| q - x)
        .collect();
    e.sort_unstable_by(|a, b| b.cmp(a));
    e
}

/// A family of lattices (one per rank, plus extras) kept at a common scale `t^q`.
pub(crate) struct Scaled {
    pub(crate) q: u32,
    /// extra socle above q (0: socle q; 1: socle q + 1)
    pub(crate) lats: Vec<(HermiteLattice, u32)>,
}

impl Scaled {
    pub(crate) fn raise(&mut self, p: u32) {
        if p > self.q {
            let e = p - self.q;
            for (l, _) in self.lats.iter_mut() {
                l.rescale(e);
            }
            self.q = p;
        }
    }
}

fn parse_vec(
    v: &ComposedVec,
    n: usize,
    need_prec: i64,
    tv: &Arc<Vec<PolyVariable>>,
) -> Result<Vec<Laurent>, ComposeError> {
    if v.len() != n {
        return Err(ComposeError::Undecided(format!(
            "vector of length {}, expected {n}",
            v.len()
        )));
    }
    let mut out = Vec::with_capacity(n);
    for s in v {
        let l = parse_series(s, tv).map_err(ComposeError::Undecided)?;
        if l.prec < need_prec {
            return Err(ComposeError::Undecided(format!(
                "precision {} below {need_prec}",
                l.prec
            )));
        }
        out.push(l);
    }
    Ok(out)
}

/// Stream `keys` through the composition in chunks; `f(index, parsed vector or reason)`.
pub(crate) fn stream_parsed(
    compose: &mut dyn Compose,
    keys: &[Key],
    chunk: usize,
    abs_prec: i64,
    tv: &Arc<Vec<PolyVariable>>,
    mut f: impl FnMut(usize, Result<Vec<Laurent>, ComposeError>) -> Result<(), String>,
) -> Result<(), String> {
    let n = compose.n_terminals();
    let mut base = 0usize;
    for ch in keys.chunks(chunk.max(1)) {
        let mut buf: Vec<Option<Result<ComposedVec, ComposeError>>> = vec![None; ch.len()];
        compose.stream(ch, abs_prec, &mut |i, r| {
            if i < buf.len() {
                buf[i] = Some(r);
            }
        })?;
        for (i, r) in buf.into_iter().enumerate() {
            let r = match r {
                None => Err(ComposeError::Undecided(
                    "no answer from the composition".into(),
                )),
                Some(Err(e)) => Err(e),
                Some(Ok(v)) => parse_vec(&v, n, abs_prec, tv),
            };
            f(base + i, r)?;
        }
        base += ch.len();
    }
    Ok(())
}

fn unit_laurent(n: usize, j: usize, prec: i64, tv: &Arc<Vec<PolyVariable>>) -> Vec<Laurent> {
    (0..n)
        .map(|k| {
            if k == j {
                let p = QP::new(&Q, None, tv.clone()).monomial(Rational::one(), vec![0]);
                Laurent::normalised(0, p, prec)
            } else {
                Laurent::zero(prec, tv)
            }
        })
        .collect()
}

fn max_opt(a: Option<u32>, b: Option<u32>) -> Option<u32> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(x), Some(y)) => Some(x.max(y)),
    }
}

/// Run the construction (see the module documentation).
pub fn construct(compose: &mut dyn Compose, inp: &BuildInput<'_>) -> Result<BuildResult, String> {
    let n = inp.terminals.len();
    if n == 0 {
        return Err("no terminals".into());
    }
    if compose.n_terminals() != n {
        return Err(format!(
            "composition has {} terminals, closure {n}",
            compose.n_terminals()
        ));
    }
    let spec = inp.spec;
    let d0 = D0::parse(&spec.d0).map_err(|e| e.to_string())?;
    let tv = t_vars();
    let mut qmap = match inp.published {
        Some(p) => Some(QMap::new(&p.q, n, d0.value(), &tv)?),
        None => None,
    };
    let abs_prec: i64 = 1.max(qmap.as_ref().map_or(0, |q| i64::from(q.pole)));
    let m = qmap.as_ref().map_or(0, |q| q.m);

    // ranks of the band
    let ranks: Vec<u32> = inp
        .band
        .iter()
        .map(|k| {
            rank_of(inp.chart, k)
                .map(|r| r as u32)
                .ok_or_else(|| format!("band key {k:?} is not a top-sector chart key"))
        })
        .collect::<Result<_, _>>()?;
    let r_list = ranks.iter().copied().max().unwrap_or(0);
    let r_max = inp.band_r_max.unwrap_or(r_list);
    let nr = r_list.max(r_max) as usize + 1;

    // candidate family (restricted to band keys and terminals)
    let band_set: HashSet<&[i32]> = inp.band.iter().map(|k| k.as_slice()).collect();
    let term_pos: HashMap<&[i32], usize> = inp
        .terminals
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_slice(), i))
        .collect();
    let mut cands = candidate_family(inp.chart, spec, &|k| band_set.contains(k));
    for t in inp.terminals {
        cands.push(t.clone());
    }
    let pref = Preference {
        chart: inp.chart,
        order: spec.order,
        terminals: inp.terminals,
        terminals_first: spec.terminals_first,
        cmp: inp.cmp,
    };
    sort_by_preference(&mut cands, &pref);
    let cand_set: HashSet<Key> = cands.iter().cloned().collect();

    // lattices: T per rank (socle q), global (socle q), variant (socle q+1); P per rank (socle q_p)
    let mut lt = Scaled {
        q: 0,
        lats: (0..nr)
            .map(|_| (HermiteLattice::new(n, 0, tv.clone()), 0))
            .collect(),
    };
    lt.lats.push((HermiteLattice::new(n, 0, tv.clone()), 0)); // global
    lt.lats.push((HermiteLattice::new(n, 1, tv.clone()), 1)); // variant: band keys alone + t O^n
    let gi = nr;
    let vi = nr + 1;
    let mut lp = Scaled {
        q: 0,
        lats: (0..if m > 0 { nr } else { 0 })
            .map(|_| (HermiteLattice::new(m, 0, tv.clone()), 0))
            .collect(),
    };

    let mut keys_per_rank = vec![0usize; nr];
    let mut decided_per_rank = vec![0usize; nr];
    let mut polar_t_per_rank = vec![0usize; nr];
    let mut maxpole_t: Vec<Vec<Option<u32>>> = vec![vec![None; n]; nr];
    let mut maxpole_p: Vec<Vec<Option<u32>>> = vec![vec![None; m]; nr];
    let mut p_undecided = 0usize;
    let mut cache: HashMap<Key, Vec<Laurent>> = HashMap::new();
    // pivot keys (global: the greedy's fallback) and per-rank pivot keys (they generate the rank's lattice), with vectors
    let mut pivots: Vec<(Key, Vec<Laurent>)> = vec![];
    let mut rank_pivots: Vec<Vec<(Key, Vec<Laurent>)>> = vec![vec![]; nr];
    let mut undecided_examples: Vec<(Key, String)> = vec![];
    let mut keys_decided = 0usize;
    let mut keys_polar = 0usize;

    for &r in &ranks {
        keys_per_rank[r as usize] += 1;
    }

    stream_parsed(
        compose,
        inp.band,
        spec.chunk_keys,
        abs_prec,
        &tv,
        |i, res| {
            let key = &inp.band[i];
            let r = ranks[i] as usize;
            let c = match res {
                Ok(c) => c,
                Err(e) => {
                    if undecided_examples.len() < MAX_EXAMPLES {
                        undecided_examples.push((key.clone(), e.to_string()));
                    }
                    return Ok(());
                }
            };
            keys_decided += 1;
            decided_per_rank[r] += 1;
            let mut polar = false;
            for (j, x) in c.iter().enumerate() {
                if x.valuation().is_some() {
                    let p = x.pole_order();
                    maxpole_t[r][j] = max_opt(maxpole_t[r][j], Some(p));
                    if p > 0 {
                        polar = true;
                    }
                }
            }
            if polar {
                polar_t_per_rank[r] += 1;
                keys_polar += 1;
            }
            lt.raise(pole_of(&c));
            let q = lt.q;
            let polar_part = scaled(&c, q, q)?;
            if lt.lats[r].0.insert(&polar_part)? > 0 {
                rank_pivots[r].push((key.clone(), c.clone()));
            }
            let g = lt.lats[gi].0.insert(&polar_part)?;
            if g > 0 {
                pivots.push((key.clone(), c.clone()));
            }
            let var_entries = scaled(&c, q, q + 1)?;
            lt.lats[vi].0.insert(&var_entries)?;
            if cand_set.contains(key) {
                cache.insert(key.clone(), c.clone());
            }
            if let Some(qm) = qmap.as_mut() {
                let cp = qm.apply(&c, 0);
                if cp.iter().all(|x| x.decides_pole()) {
                    for (k, x) in cp.iter().enumerate() {
                        if x.valuation().is_some() {
                            maxpole_p[r][k] = max_opt(maxpole_p[r][k], Some(x.pole_order()));
                        }
                    }
                    lp.raise(pole_of(&cp));
                    let qp = lp.q;
                    lp.lats[r].0.insert(&scaled(&cp, qp, qp)?)?;
                } else {
                    p_undecided += 1;
                }
            }
            Ok(())
        },
    )?;
    if keys_decided == 0 {
        return Err("no band key has a decided composition".into());
    }
    let q = lt.q;

    // prefix lattices and per-rank summaries
    let mut ranks_out = Vec::with_capacity(nr);
    let mut acc = HermiteLattice::new(n, q, tv.clone());
    let mut acc_p = if m > 0 {
        Some(HermiteLattice::new(m, lp.q, tv.clone()))
    } else {
        None
    };
    let mut prev_len = 0u64;
    let mut prefix_t = Vec::with_capacity(nr);
    let mut prefix_p = Vec::with_capacity(nr);
    for r in 0..nr {
        let before = acc.clone();
        for row in lt.lats[r].0.rows().to_vec() {
            acc.insert(&row)?;
        }
        let len = acc.length();
        let grows = r > 0 && len > prev_len;
        let mut new_key = None;
        if grows {
            // the rank's pivot keys generate its lattice (with O^n): one of them lies outside N_{<=r-1}
            let mut cands_r: Vec<Key> = vec![];
            for (k, c) in &rank_pivots[r] {
                if !before.contains(&scaled(c, q, q)?) {
                    cands_r.push(k.clone());
                }
            }
            sort_by_preference(&mut cands_r, &pref);
            new_key = cands_r.first().cloned();
        }
        let (length_p, mp) = match acc_p.as_mut() {
            Some(ap) => {
                for row in lp.lats[r].0.rows().to_vec() {
                    ap.insert(&row)?;
                }
                prefix_p.push(ap.clone());
                (Some(ap.length()), Some(maxpole_p[r].clone()))
            }
            None => (None, None),
        };
        ranks_out.push(RankSummary {
            rank: r as u32,
            keys: keys_per_rank[r],
            decided: decided_per_rank[r],
            keys_with_pole_on_t: polar_t_per_rank[r],
            max_pole_on_t: maxpole_t[r].clone(),
            length_t: len,
            elementary_t: elementary(&acc, q),
            grows,
            new_direction_key: new_key,
            length_p,
            max_pole_on_p: mp,
            keys_with_pole_on_r: None,
        });
        prev_len = len;
        prefix_t.push(acc.clone());
    }
    let n_lat = acc.clone();
    if !n_lat.equals(&lt.lats[gi].0) {
        return Err("internal: prefix lattice differs from the streamed global lattice".into());
    }
    let mut r_complete: Option<u32> = None;
    for r in 0..=r_max as usize {
        if r < nr && keys_per_rank[r] > 0 && decided_per_rank[r] == keys_per_rank[r] {
            r_complete = Some(r as u32);
        } else {
            break;
        }
    }

    // terminals in t N (never selectable) and the variant (band keys alone contain T?)
    let t_n = n_lat.scaled(1);
    let terminal_in_t_n: Vec<bool> = (0..n).map(|i| t_n.contains(&n_lat.unit(i, q))).collect();
    let terminal_in_band_lattice: Vec<bool> = (0..n)
        .map(|i| lt.lats[vi].0.contains(&n_lat.unit(i, q)))
        .collect();

    // SELECTION (Nakayama greedy)
    let mut g = t_n.clone();
    let mut chosen: Vec<(Key, Vec<Laurent>, String)> = vec![];
    let mut considered: HashSet<Key> = HashSet::new();
    let try_take = |k: &Key,
                    c: &Vec<Laurent>,
                    src: &str,
                    g: &mut HermiteLattice,
                    chosen: &mut Vec<(Key, Vec<Laurent>, String)>|
     -> Result<(), String> {
        if chosen.len() >= n {
            return Ok(());
        }
        let polar = scaled(c, q, q)?;
        if !n_lat.contains(&polar) {
            return Ok(()); // not an element of N (cannot happen for a band key)
        }
        let x = scaled(c, q, q + 1)?;
        if !g.contains(&x) {
            g.insert(&x)?;
            chosen.push((k.clone(), c.clone(), src.to_string()));
        }
        Ok(())
    };
    for k in &cands {
        if !considered.insert(k.clone()) {
            continue;
        }
        let c = if let Some(&i) = term_pos.get(k.as_slice()) {
            unit_laurent(n, i, abs_prec, &tv)
        } else if let Some(c) = cache.get(k) {
            c.clone()
        } else {
            continue;
        };
        let src = if term_pos.contains_key(k.as_slice()) {
            "terminal"
        } else {
            "candidate"
        };
        try_take(k, &c, src, &mut g, &mut chosen)?;
    }
    if chosen.len() < n {
        let mut pk: Vec<Key> = pivots.iter().map(|(k, _)| k.clone()).collect();
        sort_by_preference(&mut pk, &pref);
        let pm: HashMap<&Key, &Vec<Laurent>> = pivots.iter().map(|(k, c)| (k, c)).collect();
        for k in &pk {
            if considered.insert(k.clone()) {
                let c = pm[k].clone();
                try_take(k, &c, "pivot_fallback", &mut g, &mut chosen)?;
            }
        }
        for (i, t) in inp.terminals.iter().enumerate() {
            if considered.insert(t.clone()) {
                try_take(
                    t,
                    &unit_laurent(n, i, abs_prec, &tv),
                    "terminal",
                    &mut g,
                    &mut chosen,
                )?;
            }
        }
    }
    if chosen.len() < n {
        return Err(format!(
            "internal: the greedy found only {} of {n} elements (Theorem 2 guarantees n); data inconsistent",
            chosen.len()
        ));
    }
    // order R by the preference order
    let order_of: HashMap<Key, usize> = {
        let mut all: Vec<Key> = chosen.iter().map(|x| x.0.clone()).collect();
        sort_by_preference(&mut all, &pref);
        all.into_iter().enumerate().map(|(i, k)| (k, i)).collect()
    };
    chosen.sort_by_key(|x| order_of[&x.0]);
    let regular_set: Vec<Key> = chosen.iter().map(|x| x.0.clone()).collect();
    let sources: Vec<String> = chosen.iter().map(|x| x.2.clone()).collect();
    let kept_terminals: Vec<usize> = regular_set
        .iter()
        .filter_map(|k| term_pos.get(k.as_slice()).copied())
        .collect();

    // c1: the lattice of R equals N
    let cert_lattice = if spec.certify_lattice {
        let mut lr = HermiteLattice::new(n, q, tv.clone());
        for (_, c, _) in &chosen {
            lr.insert(&scaled(c, q, q)?)?;
        }
        Some(lr.equals(&n_lat) && lr.length() == n_lat.length())
    } else {
        None
    };

    // exact map
    let mut exact_note = None;
    let mut a_rows: Vec<Vec<String>> = vec![];
    for k in &regular_set {
        if let Some(&i) = term_pos.get(k.as_slice()) {
            a_rows.push(
                (0..n)
                    .map(|j| {
                        if i == j {
                            "1".to_string()
                        } else {
                            "0".to_string()
                        }
                    })
                    .collect(),
            );
        } else {
            match compose.exact(k) {
                Ok(v) if v.len() == n => a_rows.push(v),
                Ok(v) => {
                    exact_note = Some(format!(
                        "exact coefficients of {k:?}: {} entries, expected {n}",
                        v.len()
                    ));
                    break;
                }
                Err(e) => {
                    exact_note = Some(format!("exact coefficients of {k:?}: {e}"));
                    break;
                }
            }
        }
    }
    let exact = if a_rows.len() == n {
        match exact_map(&a_rows, &d0) {
            Ok(x) => Some(x),
            Err(e) => {
                exact_note = Some(e);
                None
            }
        }
    } else {
        None
    };
    // c4: lane poles' exact master-map check of the constructed M (independent code path); a disagreement is a finding
    let c4 = exact.as_ref().map(|x| {
        let map = crate::valuation::basis::MasterMap {
            rows: inp.terminals.iter().map(|k| rustred_campaign_core::key::key_text(k)).collect(),
            cols: regular_set.iter().map(|k| rustred_campaign_core::key::key_text(k)).collect(),
            entries: x.m_text.clone(),
        };
        let chk = crate::valuation::basis::check_master_map(&map, &d0);
        let val = |v: &crate::valuation::Valuation| match v {
            crate::valuation::Valuation::Zero => None,
            crate::valuation::Valuation::Order(o) => Some(*o),
        };
        let det_ok = chk.det_order.as_ref().map(val) == Some(x.det_m_order);
        let m_ok = chk.entry_orders.iter().map(|r| r.iter().map(val).collect::<Vec<_>>()).collect::<Vec<_>>() == x.m_orders;
        let inv_ok = chk.inverse_orders.as_ref().map(|io| io.iter().map(|r| r.iter().map(val).collect::<Vec<_>>()).collect::<Vec<_>>())
            == Some(x.a_orders.clone());
        let max_inv_pole = x.a_orders.iter().flatten().filter_map(|o| *o).filter(|&o| o < 0).map(|o| (-o) as u64).max();
        let verdict_ok = match max_inv_pole {
            None => chk.verdict == "regular",
            Some(p) => chk.verdict == "degenerate" && chk.order == Some(p),
        };
        let agree = det_ok && m_ok && inv_ok && verdict_ok;
        json!({"agree": agree, "det_order_agrees": det_ok, "M_orders_agree": m_ok, "M_inv_orders_agree": inv_ok,
               "verdict_agrees": verdict_ok, "poles_verdict": chk.verdict, "poles_order": chk.order,
               "rank_at_d0": chk.rank_at_d0, "null_directions_of_M_at_d0": chk.null_directions, "reason": chk.reason})
    });
    let c4_agree = c4.as_ref().map(|v| v["agree"].as_bool() == Some(true));
    // c5: lane tadic's Smith exponents of the exact A (rows of R on T, the module generated AS GIVEN) must be minus the
    // elementary divisors of N, padded with zeros (an independent code path: fraction-free elimination over Z[t])
    let c5 = exact.as_ref().and_then(|x| {
        let rat = d0_rat(d0.text())?;
        let rows: Vec<Vec<(u32, String)>> = x
            .a_text
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .filter(|(_, t)| t.as_str() != "0")
                    .map(|(j, t)| (j as u32, t.clone()))
                    .collect()
            })
            .collect();
        let got = crate::tadic::lattice::smith_exponents(&rows, rat);
        let mut want: Vec<i64> = elementary(&n_lat, q)
            .iter()
            .map(|&k| -i64::from(k))
            .collect();
        want.resize(n, 0);
        want.sort_unstable();
        Some(match got {
            Ok(g) => json!({"agree": g == want, "tadic_smith_exponents": g, "expected": want}),
            Err(e) => json!({"agree": null, "error": e}),
        })
    });
    let c5_agree = c5.as_ref().and_then(|v| v["agree"].as_bool());
    let cert_det = exact.as_ref().map(|x| {
        x.det_m_order == Some(n_lat.length() as i64)
            || (n_lat.length() == 0 && x.det_m_order == Some(0))
    });
    let m_regular = exact.as_ref().map(|x| {
        x.m_orders
            .iter()
            .flatten()
            .all(|o| o.is_none_or(|v| v >= 0))
    });

    // c2: per band key on R (streamed)
    let mut cert_keys_checked = None;
    let mut cert_keys_with_pole = None;
    let mut max_pole_on_r: Option<Vec<u32>> = None;
    let mut failures: Vec<Key> = vec![];
    if spec.certify_keys == rustred_campaign_core::regbasis::CertifyKeys::All
        && let Some(x) = exact.as_ref()
    {
        let mexp: Vec<Vec<Laurent>> =
            x.m.iter()
                .map(|r| {
                    r.iter()
                        .map(|e| expand_rf(e, d0.value(), i64::from(q) + 1, &tv))
                        .collect()
                })
                .collect();
        let mut checked = 0usize;
        let mut with_pole = 0usize;
        let mut mx = vec![0u32; n];
        let mut per_rank = vec![0usize; nr];
        stream_parsed(
            compose,
            inp.band,
            spec.chunk_keys,
            abs_prec,
            &tv,
            |i, res| {
                let Ok(c) = res else { return Ok(()) };
                checked += 1;
                let mut polar = false;
                for j in 0..n {
                    let mut acc = Laurent::zero(i64::MAX / 4, &tv);
                    for (ii, ci) in c.iter().enumerate() {
                        acc = acc.add(&ci.mul(&mexp[ii][j]));
                    }
                    if !acc.decides_pole() {
                        return Err(format!(
                            "certificate c2: precision does not decide the pole of {:?}",
                            inp.band[i]
                        ));
                    }
                    let p = acc.pole_order();
                    mx[j] = mx[j].max(p);
                    if p > 0 {
                        polar = true;
                    }
                }
                if polar {
                    with_pole += 1;
                    per_rank[ranks[i] as usize] += 1;
                    if failures.len() < MAX_EXAMPLES {
                        failures.push(inp.band[i].clone());
                    }
                }
                Ok(())
            },
        )?;
        for (r, s) in ranks_out.iter_mut().enumerate() {
            s.keys_with_pole_on_r = Some(per_rank[r]);
        }
        cert_keys_checked = Some(checked);
        cert_keys_with_pole = Some(with_pole);
        max_pole_on_r = Some(mx);
    }

    let regular = cert_lattice.unwrap_or(true)
        && cert_det.unwrap_or(true)
        && c4_agree.unwrap_or(true)
        && c5_agree.unwrap_or(true)
        && m_regular.unwrap_or(true)
        && cert_keys_with_pole.is_none_or(|w| w == 0);
    let verdict = band_verdict(r_complete, r_max, regular);

    // needs
    let tmax: Vec<Option<u32>> = (0..n)
        .map(|j| {
            ranks_out
                .iter()
                .fold(None, |a, s| max_opt(a, s.max_pole_on_t[j]))
        })
        .collect();
    let tlabels: Vec<String> = inp
        .terminals
        .iter()
        .map(|k| rustred_campaign_core::key::key_text(k))
        .collect();
    let tkeys: Vec<Option<Key>> = inp.terminals.iter().cloned().map(Some).collect();
    let mut needs = vec![need_table(
        "T",
        spec.base_order,
        &tlabels,
        &tkeys,
        &tmax,
        inp.availability,
        &verdict,
    )];
    let rlabels: Vec<String> = regular_set
        .iter()
        .map(|k| rustred_campaign_core::key::key_text(k))
        .collect();
    let rkeys: Vec<Option<Key>> = regular_set.iter().cloned().map(Some).collect();
    let rmax: Vec<Option<u32>> = match &max_pole_on_r {
        Some(v) => v.iter().map(|&p| Some(p)).collect(),
        None => vec![Some(0); n],
    };
    needs.push(need_table(
        "R",
        spec.base_order,
        &rlabels,
        &rkeys,
        &rmax,
        inp.availability,
        &verdict,
    ));
    if let Some(p) = inp.published {
        let pmax: Vec<Option<u32>> = (0..m)
            .map(|k| {
                ranks_out.iter().fold(None, |a, s| {
                    max_opt(a, s.max_pole_on_p.as_ref().and_then(|v| v[k]))
                })
            })
            .collect();
        needs.push(need_table(
            "P",
            spec.base_order,
            &p.labels,
            &p.keys,
            &pmax,
            inp.availability,
            &verdict,
        ));
    }

    // export object
    let lat_len = n_lat.length();
    let mut obj = json!({
        "schema": RECORD_SCHEMA,
        "binding": inp.binding.as_ref().map(|b| b.to_value()),
        "spec": spec.to_value(),
        "convention": MATRIX_CONVENTION,
        "lattice_convention": CONVENTION_BAND_PLUS_TERMINALS,
        "terminals": inp.terminals,
        "regular_set": regular_set,
        "regular_set_sources": sources,
        "kept_terminals": kept_terminals,
        "M": exact.as_ref().map(|x| x.m_text.clone()),
        "M_inv": exact.as_ref().map(|x| x.a_text.clone()),
        "M_order": exact.as_ref().map(|x| x.m_orders.clone()),
        "M_inv_order": exact.as_ref().map(|x| x.a_orders.clone()),
        "det_M": exact.as_ref().map(|x| x.det_m_text.clone()),
        "det_M_order": exact.as_ref().and_then(|x| x.det_m_order),
        "exact_note": exact_note,
        "lattice": {
            "q_max_pole_on_T": q,
            "length_z": lat_len,
            "elementary_divisors_on_T": elementary(&n_lat, q),
            "hermite_scaled_by_t_q": n_lat.rows_text(),
            "terminal_in_t_N": terminal_in_t_n,
            "variant_without_terminals": {
                "terminal_in_band_lattice": terminal_in_band_lattice,
                "equal": terminal_in_band_lattice.iter().all(|&b| b),
                "meaning": "true for terminal i: T_i is in the lattice of the band keys alone (the +O^n of the construction is automatic for it)",
            },
        },
        "band": {
            "r_max": r_max,
            "r_complete": r_complete,
            "verdict": verdict,
            "keys": inp.band.len(),
            "decided": keys_decided,
            "keys_with_pole_on_T": keys_polar,
            "undecided_examples": undecided_examples.iter().map(|(k, e)| json!({"key": k, "reason": e})).collect::<Vec<_>>(),
            "published_undecided": if m > 0 { Some(p_undecided) } else { None },
            "per_rank": ranks_out.iter().map(|s| json!({
                "rank": s.rank, "keys": s.keys, "decided": s.decided, "keys_with_pole_on_T": s.keys_with_pole_on_t,
                "max_pole_on_T": s.max_pole_on_t, "length_T": s.length_t, "elementary_T": s.elementary_t,
                "grows": s.grows, "new_direction_key": s.new_direction_key, "length_P": s.length_p,
                "max_pole_on_P": s.max_pole_on_p, "keys_with_pole_on_R": s.keys_with_pole_on_r,
            })).collect::<Vec<_>>(),
        },
        "certificate": {
            "c1_lattice_index_one": cert_lattice,
            "c3_det_order_equals_length": cert_det,
            "c4_poles_master_map_check": c4,
            "c5_tadic_smith_exponents": c5,
            "M_regular": m_regular,
            "c2_keys_checked": cert_keys_checked,
            "c2_keys_with_pole_on_R": cert_keys_with_pole,
            "c2_max_pole_on_R": max_pole_on_r,
            "c2_failures": failures,
        },
        "need": needs.iter().map(|t| t.to_value()).collect::<Vec<_>>(),
        "published": inp.published.map(|p| json!({"labels": p.labels, "keys": p.keys, "Q": p.q})),
        "selection": {
            "candidates": cands.len(),
            "fallback_used": sources.iter().any(|s| s == "pivot_fallback"),
            "pivot_keys": pivots.len(),
        },
        "method": METHOD,
        "regular": regular,
    });
    let mut record_text = None;
    if let Some(b) = inp.binding.as_ref() {
        b.validate()?;
        let (v, t) = seal(obj)?;
        obj = v;
        record_text = Some(t);
    }

    Ok(BuildResult {
        regular_set,
        sources,
        kept_terminals,
        exact,
        exact_note: obj["exact_note"].as_str().map(str::to_string),
        lattice: n_lat,
        q,
        prefix_t,
        prefix_p: if m > 0 { Some((prefix_p, lp.q)) } else { None },
        ranks: ranks_out,
        r_max,
        r_complete,
        cert_lattice,
        cert_det,
        cert_poles_check: c4_agree,
        cert_tadic_smith: c5_agree,
        cert_keys_checked,
        cert_keys_with_pole,
        max_pole_on_r,
        regular,
        verdict,
        terminal_in_t_n,
        terminal_in_band_lattice,
        needs,
        record: obj,
        record_text,
        undecided_examples,
        keys_decided,
        keys_with_pole_on_t: keys_polar,
    })
}
