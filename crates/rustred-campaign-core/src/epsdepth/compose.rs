//! The COMPOSED pole profile (interfaces/epsdepth.md 4): compose a node's served relations down to its terminal set
//! with truncated Laurent series in `t = d - d0` (exact coefficients, tracked absolute precision), memoised per key.
//!
//! Generic over two traits (this crate has no algebra, no Symbolica): [`ReductionSource`] (keys, statuses, the top-sector
//! relations of keys: what the closing loop served) and [`Laurent`] (exact valuation and expansion of one coefficient,
//! series arithmetic; the algebra crate implements it on Symbolica's `Series`).
//!
//! Phases: (1) cone discovery from the selected keys with the EXACT valuation of every top edge (record-grouped batches);
//! (2) heights (cycle check) and the required precision top-down, R(k) = max over parents (R(K) - v(K->k)), capped by
//! `max_depth`; (3) bottom-up by height layers: vec(K) = sum c(K->k) * vec(k), c expanded to R(K) - w(k). The lemma of
//! the interface (section 4) makes every selected key decided in one pass unless a budget stopped it; the series itself
//! says whether it is decided (precision), so a budget can only produce `undecided`, never a wrong order.

use std::collections::HashMap;
use std::time::Instant;

use super::profile::Val;

/// Exact Laurent arithmetic at d0.
pub trait Laurent {
    /// An exact coefficient at a key (a rational function of d).
    type Coeff;
    /// A truncated Laurent series in t = d - d0 with exact coefficients and a known absolute precision.
    type Series: Clone;
    /// The exact order at d0 (`Val::Zero` for the zero coefficient).
    fn valuation(&mut self, c: &Self::Coeff) -> Result<Val, String>;
    /// `c` expanded at d0, known for every exponent `< abs_prec` (at least its leading term); `None` = exactly zero.
    fn expand(&mut self, c: &Self::Coeff, abs_prec: i64) -> Result<Option<Self::Series>, String>;
    fn add(&self, a: &Self::Series, b: &Self::Series) -> Self::Series;
    fn mul(&self, a: &Self::Series, b: &Self::Series) -> Self::Series;
    /// (lowest known non-zero exponent, or `None` when zero to its precision; absolute precision).
    fn order(&self, s: &Self::Series) -> (Option<i64>, i64);
    /// The leading coefficient as text (records), `None` when zero to its precision.
    fn leading_text(&self, s: &Self::Series) -> Option<String>;
    /// The constant 1, known for every exponent `< abs_prec` (at least its constant term): a terminal's own vector.
    fn one(&mut self, abs_prec: i64) -> Self::Series;
}

/// Receives the composed vector of every selected key as soon as it is computed (box streaming): `Ok(vector)` on the
/// terminals (component j = coefficient of terminal j; `None` = exactly zero; each series exact to its precision, which
/// is >= the target unless a depth budget capped it) or `Err(reason)` when the key is undecided as a whole.
pub type VectorSink<'a, S> = dyn FnMut(usize, Result<&[Option<S>], &str>) + 'a;

/// What a key is in the reduction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyStatus {
    /// The j-th terminal.
    Terminal(usize),
    /// Served by a relation (rule at the key, credit through an image, relation set).
    Served,
    /// Not served (a partly closed node), with the reason.
    Open(String),
}

/// The top-sector terms of one relation at its key: (term key index, exact coefficient). Exact zeros may be omitted.
pub type Terms<C> = Vec<(usize, C)>;

/// The node's reduction as served (verified readers behind it; interfaces/epsdepth.md 2).
pub trait ReductionSource {
    type Coeff;
    fn n_keys(&self) -> usize;
    fn key(&self, i: usize) -> Vec<i32>;
    fn status(&self, i: usize) -> KeyStatus;
    /// The server of a served key as a stable label (blame lists).
    fn server(&self, i: usize) -> String;
    /// Visit the relations of `keys` (any order: the source groups by server). Per key: its top terms or an error
    /// (the key is then `undecided(source: ..)`, nothing is guessed). `Err` = the source itself failed.
    #[allow(clippy::type_complexity)]
    fn relations(
        &mut self,
        keys: &[usize],
        visit: &mut dyn FnMut(usize, Result<Terms<Self::Coeff>, String>),
    ) -> Result<(), String>;
}

/// The composed order of one (key, terminal) pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Order {
    /// The coefficient of the terminal is exactly zero (no path contributes).
    Zero,
    /// Decided: pole order `p` (0 = regular).
    Pole(u32),
    /// Not decided: `budget:depth`, `budget:keys`, `budget:time`, `open: ..`, `source: ..`.
    Undecided(String),
}

/// One (selected key, terminal) result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairResult {
    pub order: Order,
    /// Lowest known non-zero exponent of the composed coefficient.
    pub valuation: Option<i64>,
    /// Absolute precision reached.
    pub precision: Option<i64>,
    /// Exact upper bound: max over paths of summed edge pole orders (`None`: the terminal is not reachable).
    pub bound: Option<i64>,
    /// The key (index) whose relation introduces the surviving pole (interfaces/epsdepth.md 4, BLAME).
    pub blame: Option<usize>,
    pub blame_server: Option<String>,
    /// Leading Laurent coefficient (text) when non-zero.
    pub leading: Option<String>,
}

/// Results of one selected key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyResult {
    pub key: usize,
    pub pairs: Vec<PairResult>,
    /// Some edge of the key's cone carries a local pole (a decided p = 0 is then `spurious`).
    pub local_pole_in_cone: bool,
    /// The key's own relation: max top pole order and number of singular top terms (its local profile, top part).
    pub local_pole: u32,
    pub local_singular: u32,
}

/// Composition budgets and target (from the spec).
#[derive(Clone, Debug)]
pub struct ComposeParams {
    pub target: i64,
    pub max_depth: i64,
    pub max_keys: Option<usize>,
    pub max_seconds: Option<f64>,
}

impl Default for ComposeParams {
    fn default() -> Self {
        ComposeParams {
            target: 0,
            max_depth: 32,
            max_keys: None,
            max_seconds: None,
        }
    }
}

/// Counters of a run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComposeStats {
    pub selected: usize,
    pub cone_keys: usize,
    pub edges: usize,
    pub singular_edges: usize,
    pub singular_relations: usize,
    pub max_edge_pole: u32,
    pub max_height: usize,
    pub max_required: i64,
    pub capped_keys: usize,
    pub truncated_keys: usize,
    pub open_keys: usize,
    pub failed_keys: usize,
    pub expansions: u64,
    pub time_decided: bool,
    pub seconds_discover: f64,
    pub seconds_compose: f64,
}

/// The result of [`compose`].
#[derive(Clone, Debug)]
pub struct Composition {
    pub terminals: usize,
    pub results: Vec<KeyResult>,
    pub stats: ComposeStats,
    /// Per cone key with a local pole on a top term: (key index, max pole order, singular top terms).
    pub singular_relations: Vec<(usize, u32, u32)>,
}

const K_SERVED: u8 = 0;
const K_TERMINAL: u8 = 1;
const K_OPEN: u8 = 2;
const K_FAILED: u8 = 3;
const K_TRUNCATED: u8 = 4;
const NONE: u32 = u32::MAX;

enum VecR<S> {
    Unknown(String),
    Comps(Vec<Option<S>>),
}

/// Compose the reduction of `selected` keys onto the `nterm` terminals (the source's `KeyStatus::Terminal(j)`). See
/// the module documentation; `Err` only for a cycle, a source failure or an inconsistent source.
pub fn compose<S, L>(
    src: &mut S,
    lau: &mut L,
    selected: &[usize],
    nterm: usize,
    params: &ComposeParams,
) -> Result<Composition, String>
where
    S: ReductionSource,
    L: Laurent<Coeff = S::Coeff>,
{
    compose_streaming(src, lau, selected, nterm, params, None)
}

/// THE API FOR OTHER LANES (regbasis): the exact Laurent vector of ONE key on the terminals, known for every exponent
/// `< params.target` (the depth control: raise `target` for deeper coefficients). Outer `Err`: cycle / source failure;
/// inner `Err`: the key is undecided (open key in its cone, budget).
#[allow(clippy::type_complexity)]
pub fn onto_terminals<S, L>(
    src: &mut S,
    lau: &mut L,
    key: usize,
    nterm: usize,
    params: &ComposeParams,
) -> Result<Result<Vec<Option<L::Series>>, String>, String>
where
    S: ReductionSource,
    L: Laurent<Coeff = S::Coeff>,
{
    let mut out: Option<Result<Vec<Option<L::Series>>, String>> = None;
    compose_streaming(
        src,
        lau,
        &[key],
        nterm,
        params,
        Some(&mut |_, v| {
            out = Some(v.map(|v| v.to_vec()).map_err(|e| e.to_string()));
        }),
    )?;
    Ok(out.unwrap_or_else(|| Err("not computed".into())))
}

/// [`compose`] with a [`VectorSink`]: the iteration over the box (every selected key's vector is handed over once, in
/// bottom-up order, and freed when no parent needs it any more).
pub fn compose_streaming<S, L>(
    src: &mut S,
    lau: &mut L,
    selected: &[usize],
    nterm: usize,
    params: &ComposeParams,
    mut sink: Option<&mut VectorSink<'_, L::Series>>,
) -> Result<Composition, String>
where
    S: ReductionSource,
    L: Laurent<Coeff = S::Coeff>,
{
    let t0 = Instant::now();
    let over_time = |t: &Instant| {
        params
            .max_seconds
            .is_some_and(|m| t.elapsed().as_secs_f64() > m)
    };
    let mut stats = ComposeStats {
        selected: selected.len(),
        ..Default::default()
    };
    // ---- phase 1: cone discovery with exact edge valuations
    let mut lid: HashMap<usize, u32> = HashMap::new();
    let mut glob: Vec<usize> = vec![];
    let mut kind: Vec<u8> = vec![];
    let mut term_of: Vec<u32> = vec![];
    let mut edges: Vec<Vec<(u32, i32)>> = vec![];
    let mut reason: HashMap<u32, String> = HashMap::new();
    let mut frontier: Vec<u32> = vec![];
    let intern = |g: usize,
                  lid: &mut HashMap<usize, u32>,
                  glob: &mut Vec<usize>,
                  kind: &mut Vec<u8>,
                  term_of: &mut Vec<u32>,
                  edges: &mut Vec<Vec<(u32, i32)>>,
                  next: &mut Vec<u32>|
     -> u32 {
        if let Some(&l) = lid.get(&g) {
            return l;
        }
        let l = glob.len() as u32;
        lid.insert(g, l);
        glob.push(g);
        kind.push(K_SERVED);
        term_of.push(NONE);
        edges.push(vec![]);
        next.push(l);
        l
    };
    for &g in selected {
        intern(
            g,
            &mut lid,
            &mut glob,
            &mut kind,
            &mut term_of,
            &mut edges,
            &mut frontier,
        );
    }
    let mut stopped_time = false;
    while !frontier.is_empty() {
        let mut batch: Vec<usize> = vec![];
        for &l in &frontier {
            let g = glob[l as usize];
            if params.max_keys.is_some_and(|m| l as usize >= m) {
                kind[l as usize] = K_TRUNCATED;
                reason.insert(l, "budget:keys".into());
                continue;
            }
            match src.status(g) {
                KeyStatus::Terminal(j) => {
                    if j >= nterm {
                        return Err(format!("terminal index {j} >= {nterm}"));
                    }
                    kind[l as usize] = K_TERMINAL;
                    term_of[l as usize] = j as u32;
                }
                KeyStatus::Open(r) => {
                    kind[l as usize] = K_OPEN;
                    reason.insert(l, format!("open: {r}"));
                }
                KeyStatus::Served => batch.push(g),
            }
        }
        if stopped_time || over_time(&t0) {
            stopped_time = true;
            for g in batch {
                let l = lid[&g];
                kind[l as usize] = K_TRUNCATED;
                reason.insert(l, "budget:time".into());
            }
            frontier.clear();
            continue;
        }
        let mut next: Vec<u32> = vec![];
        let mut failures: Vec<(u32, String)> = vec![];
        {
            let lid = &mut lid;
            let glob = &mut glob;
            let kind = &mut kind;
            let term_of = &mut term_of;
            let edges = &mut edges;
            let next = &mut next;
            let failures = &mut failures;
            let lau = &mut *lau;
            src.relations(&batch, &mut |g, res| {
                let Some(&l) = lid.get(&g) else {
                    return;
                };
                match res {
                    Err(e) => failures.push((l, format!("source: {e}"))),
                    Ok(terms) => {
                        let mut out = Vec::with_capacity(terms.len());
                        for (cg, c) in terms {
                            match lau.valuation(&c) {
                                Ok(Val::Zero) => {}
                                Ok(Val::Order(v)) => {
                                    let cl = intern(cg, lid, glob, kind, term_of, edges, next);
                                    out.push((
                                        cl,
                                        v.clamp(i32::MIN as i64 / 2, i32::MAX as i64 / 2) as i32,
                                    ));
                                }
                                Err(e) => {
                                    failures.push((l, format!("source: valuation: {e}")));
                                    out.clear();
                                    break;
                                }
                            }
                        }
                        edges[l as usize] = out;
                    }
                }
            })?;
        }
        for (l, e) in failures {
            kind[l as usize] = K_FAILED;
            edges[l as usize].clear();
            reason.insert(l, e);
        }
        frontier = next;
    }
    let m = glob.len();
    stats.cone_keys = m;
    for k in &kind {
        match *k {
            K_OPEN => stats.open_keys += 1,
            K_FAILED => stats.failed_keys += 1,
            K_TRUNCATED => stats.truncated_keys += 1,
            _ => {}
        }
    }
    let mut singular_relations = vec![];
    for (l, es) in edges.iter().enumerate() {
        stats.edges += es.len();
        let mut mx = 0u32;
        let mut ns = 0u32;
        for &(_, v) in es {
            if v < 0 {
                ns += 1;
                mx = mx.max(v.unsigned_abs());
            }
        }
        stats.singular_edges += ns as usize;
        if ns > 0 {
            singular_relations.push((glob[l], mx, ns));
        }
        stats.max_edge_pole = stats.max_edge_pole.max(mx);
    }
    stats.singular_relations = singular_relations.len();
    stats.seconds_discover = t0.elapsed().as_secs_f64();
    // ---- phase 2: heights (iterative post-order; a cycle is an error) and the required precision
    let mut height = vec![0usize; m];
    let mut state = vec![0u8; m];
    for &g in selected {
        let s = lid[&g];
        if state[s as usize] != 0 {
            continue;
        }
        let mut stack: Vec<(u32, usize)> = vec![(s, 0)];
        state[s as usize] = 1;
        while let Some(top) = stack.last_mut() {
            let (l, i) = *top;
            let es = &edges[l as usize];
            if i < es.len() {
                top.1 += 1;
                let c = es[i].0;
                match state[c as usize] {
                    0 => {
                        state[c as usize] = 1;
                        stack.push((c, 0));
                    }
                    1 => {
                        return Err(format!(
                            "cycle in the reduction: key {:?} reaches itself",
                            src.key(glob[c as usize])
                        ));
                    }
                    _ => {}
                }
            } else {
                let h = es
                    .iter()
                    .map(|&(c, _)| height[c as usize] + 1)
                    .max()
                    .unwrap_or(0);
                height[l as usize] = h;
                state[l as usize] = 2;
                stack.pop();
            }
        }
    }
    let max_h = height.iter().copied().max().unwrap_or(0);
    stats.max_height = max_h;
    let mut layers: Vec<Vec<u32>> = vec![vec![]; max_h + 1];
    for l in 0..m {
        layers[height[l]].push(l as u32);
    }
    let cap = params.target.saturating_add(params.max_depth);
    let mut req = vec![i64::MIN; m];
    let mut rem = vec![0u32; m];
    for &g in selected {
        req[lid[&g] as usize] = params.target;
    }
    for h in (0..=max_h).rev() {
        for &l in &layers[h] {
            let l = l as usize;
            if req[l] > cap && kind[l] != K_TERMINAL {
                req[l] = cap;
                stats.capped_keys += 1;
            }
            stats.max_required = stats.max_required.max(req[l]);
            for &(c, v) in &edges[l] {
                let r = req[l].saturating_sub(i64::from(v));
                let c = c as usize;
                if r > req[c] {
                    req[c] = r;
                }
                rem[c] += 1;
            }
        }
    }
    // ---- phase 3: bottom-up composition
    let t1 = Instant::now();
    let mut is_sel = vec![false; m];
    for &g in selected {
        is_sel[lid[&g] as usize] = true;
    }
    let mut vecs: Vec<Option<VecR<L::Series>>> = (0..m).map(|_| None).collect();
    let mut pole = vec![-1i32; m * nterm];
    let mut origin = vec![NONE; m * nterm];
    let mut bound = vec![i32::MIN; m * nterm];
    let mut lp = vec![false; m];
    let mut sel_res: HashMap<u32, KeyResult> = HashMap::new();
    let mut expansions = 0u64;
    // height 0: terminals, open / failed / truncated keys, served keys without top terms
    for &l in &layers[0] {
        let lu = l as usize;
        match kind[lu] {
            K_TERMINAL => {
                let j = term_of[lu] as usize;
                pole[lu * nterm + j] = 0;
                bound[lu * nterm + j] = 0;
            }
            K_SERVED => vecs[lu] = Some(VecR::Comps((0..nterm).map(|_| None).collect())),
            _ => vecs[lu] = Some(VecR::Unknown(reason.get(&l).cloned().unwrap_or_default())),
        }
        if is_sel[lu] {
            if kind[lu] != K_TERMINAL {
                sel_res.insert(
                    l,
                    key_result(
                        lu, &vecs, &pole, &origin, &bound, &lp, &edges, nterm, &*lau, &glob, src,
                    ),
                );
            }
            if let Some(f) = sink.as_mut() {
                match &vecs[lu] {
                    Some(VecR::Comps(c)) => f(glob[lu], Ok(c)),
                    Some(VecR::Unknown(r)) => f(glob[lu], Err(r)),
                    None => {
                        let j = term_of[lu] as usize;
                        let v: Vec<Option<L::Series>> = (0..nterm)
                            .map(|i| (i == j).then(|| lau.one(params.target)))
                            .collect();
                        f(glob[lu], Ok(&v));
                    }
                }
            }
        }
    }
    let mut stop_time = false;
    for layer in layers.iter().skip(1) {
        if over_time(&t0) {
            stop_time = true;
            break;
        }
        let batch: Vec<usize> = layer.iter().map(|&l| glob[l as usize]).collect();
        let mut errors: Vec<String> = vec![];
        let mut done: Vec<u32> = vec![];
        {
            let lid = &lid;
            let kind = &kind;
            let term_of = &term_of;
            let edges = &edges;
            let req = &req;
            let vecs_r = &mut vecs;
            let pole = &mut pole;
            let origin = &mut origin;
            let bound = &mut bound;
            let lp = &mut lp;
            let lau = &mut *lau;
            let errors = &mut errors;
            let done = &mut done;
            let expansions = &mut expansions;
            src.relations(&batch, &mut |g, res| {
                let Some(&k) = lid.get(&g) else {
                    return;
                };
                let ku = k as usize;
                done.push(k);
                let terms = match res {
                    Ok(t) => t,
                    Err(e) => {
                        vecs_r[ku] = Some(VecR::Unknown(format!("source: {e}")));
                        return;
                    }
                };
                let rk = req[ku];
                let mut comps: Vec<Option<L::Series>> = (0..nterm).map(|_| None).collect();
                let mut maxchild = vec![-1i64; nterm];
                let mut argchild = vec![NONE; nterm];
                let mut unknown: Option<String> = None;
                let mut my_lp = edges[ku].iter().any(|&(_, v)| v < 0);
                // the edge valuations of phase 1, by child (bounds)
                let vals: HashMap<u32, i32> = edges[ku].iter().copied().collect();
                for (cg, c) in terms {
                    let Some(&cl) = lid.get(&cg) else {
                        match lau.valuation(&c) {
                            Ok(Val::Zero) => continue,
                            _ => {
                                errors.push(format!("term {cg} of key {g} was not met in phase 1"));
                                return;
                            }
                        }
                    };
                    let clu = cl as usize;
                    let Some(&v) = vals.get(&cl) else {
                        continue; // exact zero coefficient (phase 1 dropped it)
                    };
                    if kind[clu] == K_TERMINAL {
                        let j = term_of[clu] as usize;
                        match lau.expand(&c, rk) {
                            Ok(Some(s)) => {
                                *expansions += 1;
                                comps[j] = Some(match comps[j].take() {
                                    None => s,
                                    Some(a) => lau.add(&a, &s),
                                });
                                if maxchild[j] < 0 {
                                    maxchild[j] = 0;
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                unknown = Some(format!("source: expansion: {e}"));
                                break;
                            }
                        }
                        let b = -i64::from(v);
                        bound[ku * nterm + j] = bound[ku * nterm + j].max(b as i32);
                        continue;
                    }
                    my_lp |= lp[clu];
                    for j in 0..nterm {
                        let cb = bound[clu * nterm + j];
                        if cb != i32::MIN {
                            bound[ku * nterm + j] = bound[ku * nterm + j].max(cb.saturating_sub(v));
                        }
                    }
                    match &vecs_r[clu] {
                        None => {
                            errors.push(format!(
                                "key {cg} needed after it was freed (height order broken)"
                            ));
                            return;
                        }
                        Some(VecR::Unknown(r)) => {
                            unknown = Some(r.clone());
                        }
                        Some(VecR::Comps(cv)) => {
                            let w = cv
                                .iter()
                                .flatten()
                                .map(|s| {
                                    let (o, p) = lau.order(s);
                                    o.unwrap_or(p)
                                })
                                .min();
                            let Some(w) = w else {
                                continue; // vec(child) is exactly zero modulo subsectors
                            };
                            let s = match lau.expand(&c, rk.saturating_sub(w)) {
                                Ok(Some(s)) => s,
                                Ok(None) => continue,
                                Err(e) => {
                                    unknown = Some(format!("source: expansion: {e}"));
                                    break;
                                }
                            };
                            *expansions += 1;
                            for (j, sj) in cv.iter().enumerate() {
                                let Some(sj) = sj else { continue };
                                let prod = lau.mul(&s, sj);
                                comps[j] = Some(match comps[j].take() {
                                    None => prod,
                                    Some(a) => lau.add(&a, &prod),
                                });
                                let cp = i64::from(pole[clu * nterm + j].max(0));
                                if cp > maxchild[j] || (cp == maxchild[j] && cl < argchild[j]) {
                                    maxchild[j] = cp;
                                    argchild[j] = cl;
                                }
                            }
                        }
                    }
                }
                lp[ku] = my_lp;
                if let Some(r) = unknown {
                    vecs_r[ku] = Some(VecR::Unknown(r));
                    return;
                }
                for j in 0..nterm {
                    let Some(s) = &comps[j] else { continue };
                    let (o, p) = lau.order(s);
                    let pj = match o {
                        Some(v) => (-v).max(0),
                        None if p >= 0 => 0,
                        None => -1,
                    };
                    pole[ku * nterm + j] = pj.clamp(-1, i32::MAX as i64) as i32;
                    if pj > 0 {
                        origin[ku * nterm + j] = if pj > maxchild[j] || argchild[j] == NONE {
                            k
                        } else {
                            origin[argchild[j] as usize * nterm + j]
                        };
                    }
                }
                vecs_r[ku] = Some(VecR::Comps(comps));
            })?;
        }
        if let Some(e) = errors.into_iter().next() {
            return Err(e);
        }
        // results of selected keys of this layer, then free children whose parents are all done
        for &k in &done {
            let ku = k as usize;
            if is_sel[ku] {
                sel_res.insert(
                    k,
                    key_result(
                        ku, &vecs, &pole, &origin, &bound, &lp, &edges, nterm, &*lau, &glob, src,
                    ),
                );
                if let Some(f) = sink.as_mut() {
                    match &vecs[ku] {
                        Some(VecR::Comps(c)) => f(glob[ku], Ok(c)),
                        Some(VecR::Unknown(r)) => f(glob[ku], Err(r)),
                        None => f(glob[ku], Err("not computed")),
                    }
                }
            }
        }
        for &k in &done {
            for &(c, _) in &edges[k as usize] {
                let cu = c as usize;
                rem[cu] = rem[cu].saturating_sub(1);
                if rem[cu] == 0 && kind[cu] != K_TERMINAL {
                    vecs[cu] = None;
                }
            }
        }
    }
    stats.expansions = expansions;
    stats.time_decided = stop_time || stopped_time;
    stats.seconds_compose = t1.elapsed().as_secs_f64();
    let mut results = Vec::with_capacity(selected.len());
    for &g in selected {
        let l = lid[&g];
        let lu = l as usize;
        if let Some(r) = sel_res.remove(&l) {
            results.push(r);
            continue;
        }
        // height 0 or not computed (time budget)
        let r = match kind[lu] {
            K_TERMINAL => {
                let j = term_of[lu] as usize;
                KeyResult {
                    key: g,
                    pairs: (0..nterm)
                        .map(|i| PairResult {
                            order: if i == j { Order::Pole(0) } else { Order::Zero },
                            valuation: (i == j).then_some(0),
                            precision: None,
                            bound: (i == j).then_some(0),
                            blame: None,
                            blame_server: None,
                            leading: (i == j).then(|| "1".to_string()),
                        })
                        .collect(),
                    local_pole_in_cone: false,
                    local_pole: 0,
                    local_singular: 0,
                }
            }
            _ => {
                let why = reason.get(&l).cloned().unwrap_or_else(|| {
                    if stats.time_decided {
                        "budget:time".into()
                    } else {
                        "not computed".into()
                    }
                });
                undecided_result(g, nterm, &why)
            }
        };
        results.push(r);
    }
    Ok(Composition {
        terminals: nterm,
        results,
        stats,
        singular_relations,
    })
}

fn undecided_result(g: usize, nterm: usize, why: &str) -> KeyResult {
    KeyResult {
        key: g,
        pairs: (0..nterm)
            .map(|_| PairResult {
                order: Order::Undecided(why.to_string()),
                valuation: None,
                precision: None,
                bound: None,
                blame: None,
                blame_server: None,
                leading: None,
            })
            .collect(),
        local_pole_in_cone: false,
        local_pole: 0,
        local_singular: 0,
    }
}

#[allow(clippy::too_many_arguments)]
fn key_result<L: Laurent, S: ReductionSource>(
    ku: usize,
    vecs: &[Option<VecR<L::Series>>],
    pole: &[i32],
    origin: &[u32],
    bound: &[i32],
    lp: &[bool],
    edges: &[Vec<(u32, i32)>],
    nterm: usize,
    lau: &L,
    glob: &[usize],
    src: &S,
) -> KeyResult {
    let g = glob[ku];
    let (local_pole, local_singular) = edges[ku].iter().fold((0u32, 0u32), |(m, n), &(_, v)| {
        if v < 0 {
            (m.max(v.unsigned_abs()), n + 1)
        } else {
            (m, n)
        }
    });
    let comps = match &vecs[ku] {
        Some(VecR::Comps(c)) => c,
        Some(VecR::Unknown(r)) => {
            let mut r = undecided_result(g, nterm, r);
            r.local_pole = local_pole;
            r.local_singular = local_singular;
            return r;
        }
        None => return undecided_result(g, nterm, "not computed"),
    };
    let pairs = (0..nterm)
        .map(|j| {
            let b = bound[ku * nterm + j];
            let bound = (b != i32::MIN).then_some(i64::from(b));
            match &comps[j] {
                None => PairResult {
                    order: Order::Zero,
                    valuation: None,
                    precision: None,
                    bound,
                    blame: None,
                    blame_server: None,
                    leading: None,
                },
                Some(s) => {
                    let (o, p) = lau.order(s);
                    let pj = pole[ku * nterm + j];
                    let order = if pj < 0 {
                        Order::Undecided("budget:depth".into())
                    } else {
                        Order::Pole(pj as u32)
                    };
                    let oi = origin[ku * nterm + j];
                    let blame = (oi != NONE).then(|| glob[oi as usize]);
                    PairResult {
                        order,
                        valuation: o,
                        precision: Some(p),
                        bound,
                        blame,
                        blame_server: blame.map(|b| src.server(b)),
                        leading: lau.leading_text(s),
                    }
                }
            }
        })
        .collect();
    KeyResult {
        key: g,
        pairs,
        local_pole_in_cone: lp[ku],
        local_pole,
        local_singular,
    }
}
