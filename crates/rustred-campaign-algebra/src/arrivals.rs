//! CERTIFIED ARRIVAL SETS per DAG edge parent -> child, Symbolica part (lane arrivals; interfaces/arrivals.md;
//! definitions D1-D7 in WS/notes/implementer.arrivals.md). Included by `bin/rr_arrivals.rs` (and the tests) beside
//! `coef_facts.rs` and `zero_proofs.rs`.
//!
//! Pipeline for ONE parent (its import set, its closure rows, the edge files of its children):
//! A. EXACT CLOSURE CL(S): from the rows' start keys, follow top edges decided NON-ZERO at the key, lazily: an edge is
//!    decided only when it is the next candidate into a key not yet reached (modular witness first: a numerator value
//!    != 0 mod p at (rule point, d0) proves non-zero; else the exact per-key evaluation). Rounds and decisions are
//!    deterministic (sorted keys, candidates in (key, term) order); CL(S) does not depend on the order.
//! B. HAND-DOWN DECISIONS at every rule-served key of CL(S): structural first (zeros2's prover extended to hand-down
//!    loci: zero / non-zero on a plane or the domain), else a modular witness, else exact per key; with the exact level,
//!    the coefficient's values at two (prime, d0) points.
//! C. MAP + AGGREGATE: every non-zero hand-down term whose dropped mask an edge file routes is expanded into child keys
//!    (Symbolica polynomial arithmetic over Q: the route's linear forms raised to the numerator powers); per child key the
//!    distinct senders (support level), the first sender, and (exact level) the senders whose TOTAL coefficient
//!    sum_t c_t(u) T_{t,k} is proved non-zero (a non-zero value at one point is a witness; zero at both points ->
//!    exact symbolic sum of the univariate rational functions).
//! Nothing here names a node, a count or a frame (L8): edges, frames, primes and budgets are data / parameters.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::closure::CoefFact;
use rustred_campaign_core::import::ImportedSet;
use rustred_campaign_core::import::arrivals::edge::{EdgeFile, RoutedSector, SlotSub, mask_owners};
use rustred_campaign_core::import::arrivals::format::{ArrivalLine, Sender};
use rustred_campaign_core::import::arrivals::rows::ParentRows;
use rustred_campaign_core::key::NodeFrame;
use serde_json::{Value, json};
use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::{Q, Rational, RationalField};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::poly::polynomial::MultivariatePolynomial;

use super::coef_facts;
use super::zero_proofs::{ProverCfg, RecordProver};

type QPoly = MultivariatePolynomial<RationalField, u16>;
type PPoly = MultivariatePolynomial<Zp64, u16>;
type Fe = FiniteFieldElement<u64>;

/// The two evaluation points of the modular witnesses (primes verified at start; d values arbitrary: a non-zero value
/// is a proof at any point, a zero value only sends the pair to the exact evaluation).
pub const PRIMES: [(u64, u64); 2] = [
    (2_305_843_009_213_693_951, 1_000_003),        // 2^61 - 1
    (4_611_686_018_427_387_847, 7_777_777_777_777), // 2^62 - 57
];

/// Version of this module's semantics (recorded in every arrival file).
pub const TOOL_VERSION: &str = "rr_arrivals.v1";

fn int_mod(f: &Zp64, i: &Integer) -> Fe {
    let p = Integer::from(f.get_prime());
    let mut r = i % &p;
    if r < 0 {
        r += &p;
    }
    f.to_element(u64::try_from(r).expect("residue fits u64"))
}

fn small_mod(f: &Zp64, v: i64) -> Fe {
    let p = f.get_prime() as i128;
    f.to_element((v as i128).rem_euclid(p) as u64)
}

/// Check the two primes once (Symbolica's primality test).
pub fn check_primes() -> Result<(), String> {
    for (p, _) in PRIMES {
        if !Integer::from(p).is_prime(32) {
            return Err(format!("{p} is not prime"));
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------------------------
// The route expansion
// ------------------------------------------------------------------------------------------------------------------

/// Expands a parent key of a routed sector into child keys (exact rationals).
pub struct Transporter {
    tpl: QPoly,
    arity: usize,
}

impl Transporter {
    pub fn new(alg: &SymbolicaAlgebra, arity: usize) -> Result<Transporter, String> {
        let z = alg.parse_side_polynomial("0", arity)?;
        let tpl = z.map_coeff(|c| Rational::from(c), Q);
        if tpl.nvars() != arity + 1 {
            return Err("variable map is not n0..n{N-1}, d".into());
        }
        Ok(Transporter { tpl, arity })
    }

    fn rat(r: &rustred_campaign_core::import::arrivals::edge::Rat) -> Rational {
        Rational::from((r.num, r.den))
    }

    /// The image of `key` (a parent key of sector `s`) as (child key, coefficient), child keys sorted, zero coefficients
    /// dropped. Refuses a key that is not of the sector (a kept line < 1, a dropped line or numerator > 0).
    pub fn apply(&self, s: &RoutedSector, key: &[i32]) -> Result<Vec<(Vec<i32>, Rational)>, String> {
        let n = self.arity;
        if key.len() != n || s.subst.len() != n {
            return Err(format!("key of arity {} for a route of arity {}", key.len(), s.subst.len()));
        }
        let mut base = vec![0i32; n];
        let mut poly = self.tpl.constant(Rational::from(1));
        for (t, sub) in s.subst.iter().enumerate() {
            match sub {
                SlotSub::Line(j) => {
                    if key[t] < 1 {
                        return Err(format!("kept line {t} at power {} (not this sector)", key[t]));
                    }
                    base[*j] = key[t];
                }
                SlotSub::Form { coefs, q2 } => {
                    if key[t] > 0 {
                        return Err(format!("slot {t} at power {} > 0 (not this sector)", key[t]));
                    }
                    let e = (-key[t]) as usize;
                    if e == 0 {
                        continue;
                    }
                    let mut l = self.tpl.constant(Self::rat(q2));
                    for (j, c) in coefs {
                        let mut ex = vec![0u16; n + 1];
                        ex[*j] = 1;
                        l = &l + &self.tpl.monomial(Self::rat(c), ex);
                    }
                    poly = &poly * &l.pow(e);
                }
            }
        }
        let mut out = Vec::with_capacity(poly.nterms());
        for m in &poly {
            let mut k = base.clone();
            for (j, e) in m.exponents.iter().take(n).enumerate() {
                k[j] -= i32::from(*e);
            }
            out.push((k, m.coefficient.clone()));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }
}

/// INDEPENDENT re-check of an expansion (verifier; DESIGN REASON D-AR-1, not a Symbolica gap): at a random point X of
/// Z_p^N, prod over form slots (sum_j c_j X_j + q2)^e == sum_k T_k prod_j X_j^(base_j - k_j), in plain u128 arithmetic
/// mod a 61-bit prime (no polynomial code, no Symbolica). `seed` picks the point.
pub fn transport_identity_mod_p(
    s: &RoutedSector,
    key: &[i32],
    image: &[(Vec<i32>, (i128, i128))],
    seed: u64,
) -> Result<bool, String> {
    const P: u128 = 2_305_843_009_213_693_951;
    let n = key.len();
    let mut x = vec![0u128; n];
    let mut st = seed ^ 0x9E37_79B9_7F4A_7C15;
    for v in x.iter_mut() {
        st = st.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        *v = (st >> 3) as u128 % P;
    }
    let md = |a: i128| -> u128 { a.rem_euclid(P as i128) as u128 };
    let mul = |a: u128, b: u128| a * b % P;
    let pw = |mut b: u128, mut e: u64| {
        let mut r = 1u128;
        while e > 0 {
            if e & 1 == 1 {
                r = mul(r, b);
            }
            b = mul(b, b);
            e >>= 1;
        }
        r
    };
    let inv = |a: u128| pw(a, (P - 2) as u64);
    let mut lhs = 1u128;
    let mut base = vec![0i64; n];
    for (t, sub) in s.subst.iter().enumerate() {
        match sub {
            SlotSub::Line(j) => base[*j] = key[t] as i64,
            SlotSub::Form { coefs, q2 } => {
                let e = (-key[t]).max(0) as u64;
                let mut l = mul(md(q2.num as i128), inv(md(q2.den as i128)));
                for (j, c) in coefs {
                    l = (l + mul(mul(md(c.num as i128), inv(md(c.den as i128))), x[*j])) % P;
                }
                lhs = mul(lhs, pw(l, e));
            }
        }
    }
    let mut rhs = 0u128;
    for (k, (num, den)) in image {
        let mut term = mul(md(*num), inv(md(*den)));
        for j in 0..n {
            let e = base[j] - k[j] as i64;
            if e < 0 {
                return Err(format!("child key {k:?} above the base at slot {j}"));
            }
            term = mul(term, pw(x[j], e as u64));
        }
        rhs = (rhs + term) % P;
    }
    Ok(lhs == rhs)
}

/// Rational -> (num, den) as i128 (None if too large).
pub fn rat_i128(r: &Rational) -> Option<(i128, i128)> {
    Some((
        i128::try_from(r.numerator()).ok()?,
        i128::try_from(r.denominator()).ok()?,
    ))
}

// ------------------------------------------------------------------------------------------------------------------
// Records and modular evaluation
// ------------------------------------------------------------------------------------------------------------------

/// Read and parse the record (table, record) of the set, its sha256 checked against the set.
pub fn read_record(
    set: &ImportedSet,
    frames: &Mutex<BTreeMap<String, NodeFrame>>,
    t: usize,
    r: usize,
) -> Result<(RuleText, NodeFrame), String> {
    let info = &set.rules[t][r];
    let path = Path::new(&info.path);
    let frame = {
        let mut g = frames.lock().expect("frames");
        if !g.contains_key(&info.path) {
            let (f, _) = coef_facts::table_frame(path)?;
            g.insert(info.path.clone(), f);
        }
        g[&info.path].clone()
    };
    let (line, sha) = coef_facts::line_at(path, info.offset)?;
    if sha != info.record_sha256 {
        return Err(format!(
            "{}@{}: record sha256 {sha} != the set's {}",
            info.path, info.offset, info.record_sha256
        ));
    }
    let rt = RuleText::parse(&line, &frame).map_err(|e| format!("{}@{}: {e:?}", info.path, info.offset))?;
    Ok((rt, frame))
}

/// One parsed record with its prover and modular evaluator. The record text is LEAKED to give the prover a 'static
/// borrow (a one-shot CLI walk keeps every used record until it exits; memory = the used records' text + parsed terms).
pub struct RecEntry {
    pub p: RecordProver<'static>,
    pub me: ModEval,
}

/// The parsed records of a walk, built on first use, shared by every phase.
pub struct RecCache<'s> {
    set: &'s ImportedSet,
    alg: &'static SymbolicaAlgebra,
    cfg: ProverCfg,
    frames: Mutex<BTreeMap<String, NodeFrame>>,
    slots: HashMap<(usize, usize), Mutex<Option<RecEntry>>>,
    /// Every record text read once (leaked, reused by later phases after a slot was released).
    texts: Mutex<HashMap<(usize, usize), (&'static RuleText, NodeFrame)>>,
}

impl<'s> RecCache<'s> {
    pub fn new(alg: &'static SymbolicaAlgebra, set: &'s ImportedSet, cfg: ProverCfg, recs: impl IntoIterator<Item = (usize, usize)>) -> RecCache<'s> {
        RecCache {
            set,
            alg,
            cfg,
            frames: Mutex::new(BTreeMap::new()),
            slots: recs.into_iter().map(|k| (k, Mutex::new(None))).collect(),
            texts: Mutex::new(HashMap::new()),
        }
    }

    /// Release the parsed terms of one record (its text stays; the next use re-parses only what it needs).
    pub fn release(&self, t: usize, r: usize) {
        if let Some(slot) = self.slots.get(&(t, r))
            && let Ok(mut g) = slot.lock()
        {
            *g = None;
        }
    }

    /// Release every record's parsed terms (bounded memory between phases).
    pub fn release_all(&self) {
        for slot in self.slots.values() {
            if let Ok(mut g) = slot.lock() {
                *g = None;
            }
        }
    }

    /// Run `f` on the record's entry (parsed on first use).
    pub fn with<R>(&self, t: usize, r: usize, f: impl FnOnce(&mut RecEntry) -> Result<R, String>) -> Result<R, String> {
        let slot = self.slots.get(&(t, r)).ok_or_else(|| format!("record ({t}, {r}) not in the cache"))?;
        let mut g = slot.lock().map_err(|_| "record cache lock poisoned".to_string())?;
        if g.is_none() {
            let known = self.texts.lock().map_err(|_| "texts lock".to_string())?.get(&(t, r)).cloned();
            let (rt, frame) = match known {
                Some(x) => x,
                None => {
                    let (rt, frame) = read_record(self.set, &self.frames, t, r)?;
                    let rt: &'static RuleText = Box::leak(Box::new(rt));
                    self.texts.lock().map_err(|_| "texts lock".to_string())?.insert((t, r), (rt, frame.clone()));
                    (rt, frame)
                }
            };
            let p = RecordProver::new(self.alg, self.cfg.clone(), rt, frame.mask())?;
            *g = Some(RecEntry { p, me: ModEval::new() });
        }
        f(g.as_mut().expect("set"))
    }
}

/// Per-record modular evaluator: the parsed terms reduced mod each prime once, evaluated with cached powers.
pub struct ModEval {
    fields: Vec<Zp64>,
    cache: HashMap<(usize, usize), (PPoly, Option<PPoly>)>,
}

impl Default for ModEval {
    fn default() -> Self {
        Self::new()
    }
}

impl ModEval {
    pub fn new() -> ModEval {
        ModEval {
            fields: PRIMES.iter().map(|(p, _)| Zp64::new(*p)).collect(),
            cache: HashMap::new(),
        }
    }

    fn reduced(&mut self, p: &mut RecordProver<'_>, j: usize, i: usize) -> Result<&(PPoly, Option<PPoly>), String> {
        if !self.cache.contains_key(&(j, i)) {
            let f = self.fields[i].clone();
            let (num, den) = p.term_polys(j)?;
            let rn = num.map_coeff(|c| int_mod(&f, c), f.clone());
            let rd = den.map(|d| d.map_coeff(|c| int_mod(&f, c), f.clone()));
            self.cache.insert((j, i), (rn, rd));
        }
        Ok(&self.cache[&(j, i)])
    }

    /// (numerator, denominator) of term j at (rule point, d_i) mod p_i; denominator 1 when the term has none.
    pub fn values(&mut self, p: &mut RecordProver<'_>, j: usize, pt: &[i32], i: usize) -> Result<(Fe, Fe), String> {
        let f = self.fields[i].clone();
        let d0 = PRIMES[i].1;
        let mut point: Vec<Fe> = pt.iter().map(|&v| small_mod(&f, v as i64)).collect();
        point.push(f.to_element(d0 % f.get_prime()));
        let (rn, rd) = self.reduced(p, j, i)?;
        let n = rn.replace_all(&point);
        let d = match rd {
            Some(d) => d.replace_all(&point),
            None => f.one(),
        };
        Ok((n, d))
    }

    /// The coefficient value at point i, None when the denominator vanishes mod p_i there.
    pub fn coef(&mut self, p: &mut RecordProver<'_>, j: usize, pt: &[i32], i: usize) -> Result<Option<u64>, String> {
        let f = self.fields[i].clone();
        let (n, d) = self.values(p, j, pt, i)?;
        if f.is_zero(&d) {
            return Ok(None);
        }
        Ok(Some(f.from_element(&f.mul(&n, &f.inv(&d)))))
    }

    pub fn field(&self, i: usize) -> &Zp64 {
        &self.fields[i]
    }
}

/// The exact word of term j at `pt` with a modular witness first: (word, by).
pub fn exact_word(p: &mut RecordProver<'_>, me: &mut ModEval, j: usize, pt: &[i32]) -> Result<(CoefFact, String), String> {
    let f = me.field(0).clone();
    let (n, d) = me.values(p, j, pt, 0)?;
    if !f.is_zero(&n) && !f.is_zero(&d) {
        return Ok((CoefFact::NonZero, "key:mod".into()));
    }
    Ok((p.word_at(j, pt), "key".into()))
}

// ------------------------------------------------------------------------------------------------------------------
// The walk
// ------------------------------------------------------------------------------------------------------------------

/// Which child keys get the exact (coefficient) level.
#[derive(Clone, Debug)]
pub enum ExactLevel {
    None,
    All,
    Keys(BTreeSet<Vec<i32>>),
}

impl ExactLevel {
    fn wants(&self, k: &[i32]) -> bool {
        match self {
            ExactLevel::None => false,
            ExactLevel::All => true,
            ExactLevel::Keys(s) => s.contains(k),
        }
    }
    fn any(&self) -> bool {
        !matches!(self, ExactLevel::None)
    }
    pub fn word(&self) -> String {
        match self {
            ExactLevel::None => "none".into(),
            ExactLevel::All => "all".into(),
            ExactLevel::Keys(s) => format!("keys:{}", s.len()),
        }
    }
}

/// Parameters of a walk.
#[derive(Clone, Debug)]
pub struct WalkCfg {
    pub threads: usize,
    pub prover: ProverCfg,
    pub exact: ExactLevel,
    /// Use the structural hand-down proofs (false: every hand-down term per key: the verifier's independent route).
    pub structural: bool,
    /// Max child-key entries kept in a thread's route cache.
    pub cache_entries: usize,
}

/// A decided hand-down term at a key.
#[derive(Clone, Debug)]
pub struct HandTerm {
    pub j: u32,
    pub tk: Vec<i32>,
    pub mask: u64,
    pub word: CoefFact,
    pub by: String,
    pub vals: [Option<u64>; 2],
}

/// Counters of a walk.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct WalkStats {
    pub rows_keys: u64,
    pub rows_served: u64,
    pub start_keys: u64,
    pub closure_keys: u64,
    pub closure_served: u64,
    pub keys_only_not_reached: u64,
    pub top_decisions: u64,
    pub top_zero: u64,
    pub top_unknown: u64,
    pub top_by_mod: u64,
    pub rounds: u64,
    pub records_walked: u64,
    pub handdown_terms: u64,
    pub handdown_zero: u64,
    pub handdown_nonzero: u64,
    pub handdown_unknown: u64,
    pub by_structural_zero: u64,
    pub by_structural_nonzero: u64,
    pub by_key_mod: u64,
    pub by_key_exact: u64,
    pub positive_numerator_terms: u64,
    pub seconds_closure: f64,
    pub seconds_handdowns: f64,
}

fn split_lines(frame_lines: &[bool], tk: &[i32]) -> (bool, u64) {
    let mut pos = false;
    let mut mask = 0u64;
    for (i, (&v, &l)) in tk.iter().zip(frame_lines).enumerate() {
        if l {
            if v <= 0 {
                mask |= 1 << i;
            }
        } else if v > 0 {
            pos = true;
        }
    }
    (pos, mask)
}

/// Run `f` over groups in parallel (dynamic scheduling), results in group order.
fn par_map<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out: Vec<Mutex<Option<R>>> = items.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(it) = items.get(i) else { break };
                    let r = f(it);
                    *out[i].lock().expect("lock") = Some(r);
                }
            });
        }
    });
    out.into_iter().map(|m| m.into_inner().expect("lock").expect("done")).collect()
}

/// The result of the walk of one parent.
pub struct Walk {
    /// Indices into `rows.served` of the rule-served keys of CL(S), sorted by key.
    pub cl_served: Vec<usize>,
    /// Hand-down terms per entry of `cl_served` (non-zero and unknown only; zero ones counted).
    pub hand: Vec<Vec<HandTerm>>,
    pub stats: WalkStats,
    pub closure_digest: String,
}

/// Phases A and B (module docs). `cache` holds every record that serves a row (built by the caller).
pub fn walk(set: &ImportedSet, rows: &ParentRows, cache: &RecCache<'_>, cfg: &WalkCfg) -> Result<Walk, String> {
    let t0 = std::time::Instant::now();
    let lines = set.frame.mask().to_vec();
    let mut st = WalkStats {
        rows_keys: rows.keys.len() as u64,
        rows_served: rows.served.len() as u64,
        ..Default::default()
    };
    let idx: HashMap<&[i32], usize> = rows.served.iter().enumerate().map(|(i, r)| (r.key.as_slice(), i)).collect();
    // --- A: the exact closure, lazily ---
    let mut reached: BTreeSet<Vec<i32>> = rows.keys.iter().filter(|(_, s)| **s).map(|(k, _)| k.clone()).collect();
    st.start_keys = reached.len() as u64;
    let n = set.arity();
    let tops = |i: usize| -> Vec<(u32, Vec<i32>)> {
        let r = &rows.served[i];
        let (mut buf, mut cm) = (vec![], vec![]);
        let Ok(Some(nt)) = set.bodies.instantiate(&set.disp, set.disp.rule_ref(r.table, r.record), &r.applied, &mut buf, &mut cm) else {
            return vec![];
        };
        (0..nt)
            .filter_map(|j| {
                let tk = &buf[j * n..(j + 1) * n];
                let (pos, mask) = split_lines(&lines, tk);
                (!pos && mask == 0).then(|| (j as u32, tk.to_vec()))
            })
            .collect()
    };
    let mut frontier: Vec<Vec<i32>> = reached.iter().cloned().collect();
    let mut cand: BTreeMap<Vec<i32>, Vec<(usize, u32)>> = BTreeMap::new();
    while !frontier.is_empty() {
        st.rounds += 1;
        for k in &frontier {
            if let Some(&i) = idx.get(k.as_slice()) {
                for (j, tk) in tops(i) {
                    if !reached.contains(&tk) {
                        if !rows.keys.contains_key(&tk) {
                            return Err(format!("the rows file is not closed: {tk:?} (top term {j} of {k:?}) is not a row"));
                        }
                        cand.entry(tk).or_default().push((i, j));
                    }
                }
            }
        }
        let mut new: Vec<Vec<i32>> = vec![];
        loop {
            let pend: Vec<(Vec<i32>, (usize, u32))> = cand.iter().filter_map(|(k, v)| v.first().map(|c| (k.clone(), *c))).collect();
            if pend.is_empty() {
                break;
            }
            let mut by_rec: BTreeMap<(usize, usize), Vec<(usize, u32, usize)>> = BTreeMap::new();
            for (pi, (_, (i, j))) in pend.iter().enumerate() {
                let r = &rows.served[*i];
                by_rec.entry((r.table, r.record)).or_default().push((*i, *j, pi));
            }
            let groups: Vec<((usize, usize), Vec<(usize, u32, usize)>)> = by_rec.into_iter().collect();
            let res = par_map(&groups, cfg.threads, |((t, r), items)| -> Result<Vec<(usize, CoefFact, bool)>, String> {
                cache.with(*t, *r, |e| {
                    let mut out = vec![];
                    for (i, j, pi) in items {
                        let row = &rows.served[*i];
                        let pt = e.p.point(&row.applied).ok_or_else(|| format!("{:?}: no rule point", row.applied))?;
                        let (w, by) = exact_word(&mut e.p, &mut e.me, *j as usize, &pt)?;
                        out.push((*pi, w, by == "key:mod"));
                    }
                    Ok(out)
                })
            });
            let mut decided = vec![CoefFact::Unknown; pend.len()];
            for r in res {
                for (pi, w, m) in r? {
                    decided[pi] = w;
                    st.top_decisions += 1;
                    st.top_by_mod += m as u64;
                }
            }
            for ((k, _), w) in pend.iter().zip(decided) {
                if w == CoefFact::Zero {
                    st.top_zero += 1;
                    cand.get_mut(k).expect("pending").remove(0);
                } else {
                    st.top_unknown += (w == CoefFact::Unknown) as u64;
                    cand.remove(k);
                    reached.insert(k.clone());
                    new.push(k.clone());
                }
            }
            cand.retain(|_, v| !v.is_empty());
        }
        cand.clear();
        new.sort();
        eprintln!("[arrivals] closure round {}: +{} keys, reached {}, top decisions {} ({:.0} s)", st.rounds, new.len(), reached.len(), st.top_decisions, t0.elapsed().as_secs_f64());
        frontier = new;
    }
    cache.release_all();
    st.closure_keys = reached.len() as u64;
    st.keys_only_not_reached = (rows.keys.len() - reached.len()) as u64;
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    for k in &reached {
        h.update(format!("{k:?}\n").as_bytes());
    }
    let closure_digest = format!("sha256:{}", h.finish_hex());
    let cl_served: Vec<usize> = (0..rows.served.len()).filter(|&i| reached.contains(&rows.served[i].key)).collect();
    st.closure_served = cl_served.len() as u64;
    st.seconds_closure = t0.elapsed().as_secs_f64();
    // --- B: hand-down decisions on CL(S) ---
    let t1 = std::time::Instant::now();
    let mut by_rec: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (ci, &i) in cl_served.iter().enumerate() {
        let r = &rows.served[i];
        by_rec.entry((r.table, r.record)).or_default().push(ci);
    }
    let mut groups: Vec<((usize, usize), Vec<usize>)> = by_rec.into_iter().collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.1.len()));
    st.records_walked = groups.len() as u64;
    type Out = (Vec<(usize, Vec<HandTerm>)>, WalkStats);
    let res = par_map(&groups, cfg.threads, |((t, r), items)| -> Result<Out, String> {
        cache.with(*t, *r, |e| {
            let (p, me) = (&mut e.p, &mut e.me);
            let mut s = WalkStats::default();
            let mut out = vec![];
            for &ci in items {
                let row = &rows.served[cl_served[ci]];
                let pt = p.point(&row.applied).ok_or_else(|| format!("{:?}: no rule point", row.applied))?;
                let mut hand = vec![];
                for j in 0..p.n_terms() {
                    let Some(tk) = p.term_key(j, &pt) else {
                        return Err(format!("{:?}: term {j} pattern", row.applied));
                    };
                    if tk.len() != n {
                        return Err("term key arity".into());
                    }
                    let (pos, mask) = split_lines(&lines, &tk);
                    if pos {
                        s.positive_numerator_terms += 1;
                        continue;
                    }
                    if mask == 0 {
                        continue;
                    }
                    s.handdown_terms += 1;
                    let structural = if cfg.structural { p.handdown_structural(j, &pt, &tk) } else { None };
                    let (word, by) = match structural {
                        Some((w, by)) => {
                            if w == CoefFact::Zero {
                                s.by_structural_zero += 1;
                            } else {
                                s.by_structural_nonzero += 1;
                            }
                            (w, by)
                        }
                        None => {
                            let (w, by) = exact_word(p, me, j, &pt)?;
                            if by == "key:mod" {
                                s.by_key_mod += 1;
                            } else {
                                s.by_key_exact += 1;
                            }
                            (w, by)
                        }
                    };
                    match word {
                        CoefFact::Zero => {
                            s.handdown_zero += 1;
                            continue;
                        }
                        CoefFact::NonZero => s.handdown_nonzero += 1,
                        CoefFact::Unknown => s.handdown_unknown += 1,
                    }
                    let mut vals = [None, None];
                    if cfg.exact.any() && word == CoefFact::NonZero {
                        for (i, v) in vals.iter_mut().enumerate() {
                            *v = me.coef(p, j, &pt, i)?;
                        }
                    }
                    hand.push(HandTerm { j: j as u32, tk, mask, word, by, vals });
                }
                out.push((ci, hand));
            }
            let _ = p.take_new();
            Ok((out, s))
        })
        .inspect(|_| cache.release(*t, *r))
    });
    let mut hand: Vec<Vec<HandTerm>> = vec![vec![]; cl_served.len()];
    for r in res {
        let (o, s) = r?;
        for (ci, h) in o {
            hand[ci] = h;
        }
        st.handdown_terms += s.handdown_terms;
        st.handdown_zero += s.handdown_zero;
        st.handdown_nonzero += s.handdown_nonzero;
        st.handdown_unknown += s.handdown_unknown;
        st.by_structural_zero += s.by_structural_zero;
        st.by_structural_nonzero += s.by_structural_nonzero;
        st.by_key_mod += s.by_key_mod;
        st.by_key_exact += s.by_key_exact;
        st.positive_numerator_terms += s.positive_numerator_terms;
    }
    st.seconds_handdowns = t1.elapsed().as_secs_f64();
    eprintln!("[arrivals] hand-down decisions: {} terms ({} zero, {} non-zero) at {} keys ({:.0} s)", st.handdown_terms, st.handdown_zero, st.handdown_nonzero, cl_served.len(), st.seconds_handdowns);
    Ok(Walk { cl_served, hand, stats: st, closure_digest })
}

// ------------------------------------------------------------------------------------------------------------------
// Aggregation
// ------------------------------------------------------------------------------------------------------------------

/// Per-edge aggregation counters.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct EdgeStats {
    pub sending_parent_keys: u64,
    pub handdown_terms_routed: u64,
    pub image_terms: u64,
    pub lower_child_sector_terms: u64,
    pub exact_pairs: u64,
    pub exact_by_witness: u64,
    pub exact_by_symbolic: u64,
    pub exact_cancelled: u64,
    pub unknown_terms_sending: u64,
    pub seconds: f64,
}

/// (cl index, term j, hand index) of a sender.
type SenderRef = (usize, u32, usize);

struct Acc {
    senders: u64,
    first: SenderRef,
    exact_senders: u64,
    exact_level: bool,
    first_exact: Option<SenderRef>,
}

/// One edge's arrival lines (sorted) and counters.
pub struct EdgeResult {
    pub lines: Vec<ArrivalLine>,
    pub stats: EdgeStats,
}

/// Per child key at one parent key: exact accumulators, the first contributing term.
struct PerKey {
    acc: [Option<Fe>; 2],
    first: SenderRef,
}

fn min_ref(w: &Walk, a: SenderRef, b: SenderRef) -> SenderRef {
    if (w.cl_served[a.0], a.1) <= (w.cl_served[b.0], b.1) { a } else { b }
}

/// Phase C for all edges of the parent at once: the arrival lines per edge and the masks no edge routes (mask -> terms).
pub fn aggregate(
    set: &ImportedSet,
    rows: &ParentRows,
    w: &Walk,
    edges: &[EdgeFile],
    cache: &RecCache<'_>,
    cfg: &WalkCfg,
) -> Result<(Vec<EdgeResult>, BTreeMap<u64, u64>), String> {
    let t0 = std::time::Instant::now();
    let owners = mask_owners(edges)?;
    let n = set.arity();
    let alg = cache.alg;
    let child_lines: Vec<Vec<usize>> = edges.iter().map(|e| e.child.table_lines.clone()).collect();
    let chunk = w.cl_served.len().div_ceil(cfg.threads.max(1) * 4).max(1);
    let parts: Vec<(usize, usize)> = (0..w.cl_served.len()).step_by(chunk).map(|a| (a, (a + chunk).min(w.cl_served.len()))).collect();
    type Part = (Vec<HashMap<Vec<i32>, Acc>>, Vec<EdgeStats>, BTreeMap<u64, u64>, Vec<(usize, usize, Vec<i32>, SenderRef)>);
    let res = par_map(&parts, cfg.threads, |&(a, b)| -> Result<Part, String> {
        let tr = Transporter::new(alg, n)?;
        let fields: Vec<Zp64> = PRIMES.iter().map(|(p, _)| Zp64::new(*p)).collect();
        let mut maps: Vec<HashMap<Vec<i32>, Acc>> = edges.iter().map(|_| HashMap::new()).collect();
        let mut est: Vec<EdgeStats> = edges.iter().map(|_| EdgeStats::default()).collect();
        let mut other: BTreeMap<u64, u64> = BTreeMap::new();
        let mut fallback = vec![];
        // expansion per (edge, parent term key): child keys with the coefficient reduced mod each prime
        let mut tcache: HashMap<(usize, Vec<i32>), std::sync::Arc<Vec<(Vec<i32>, [Fe; 2])>>> = HashMap::new();
        let mut cached = 0usize;
        for ci in a..b {
            let mut per: Vec<HashMap<Vec<i32>, PerKey>> = edges.iter().map(|_| HashMap::new()).collect();
            let mut sent = vec![false; edges.len()];
            for (hi, h) in w.hand[ci].iter().enumerate() {
                let Some(&e) = owners.get(&h.mask) else {
                    *other.entry(h.mask).or_default() += 1;
                    continue;
                };
                let sec = edges[e].sector(h.mask).expect("owner");
                est[e].handdown_terms_routed += 1;
                est[e].unknown_terms_sending += (h.word == CoefFact::Unknown) as u64;
                sent[e] = true;
                let key = (e, h.tk.clone());
                let img = match tcache.get(&key) {
                    Some(x) => x.clone(),
                    None => {
                        let red: Vec<(Vec<i32>, [Fe; 2])> = tr
                            .apply(sec, &h.tk)?
                            .into_iter()
                            .map(|(k, c)| {
                                let t = |f: &Zp64| f.mul(&int_mod(f, &c.numerator()), &f.inv(&int_mod(f, &c.denominator())));
                                (k, [t(&fields[0]), t(&fields[1])])
                            })
                            .collect();
                        let v = std::sync::Arc::new(red);
                        cached += v.len();
                        if cached > cfg.cache_entries {
                            tcache.clear();
                            cached = v.len();
                        }
                        tcache.insert(key, v.clone());
                        v
                    }
                };
                let me = (ci, h.j, hi);
                for (k, c) in img.iter() {
                    est[e].image_terms += 1;
                    if child_lines[e].iter().any(|&j| k[j] < 1) {
                        est[e].lower_child_sector_terms += 1;
                        continue;
                    }
                    let ent = per[e].entry(k.clone()).or_insert_with(|| PerKey { acc: [Some(fields[0].zero()), Some(fields[1].zero())], first: me });
                    if cfg.exact.wants(k) {
                        for (i, f) in fields.iter().enumerate() {
                            let acc = &mut ent.acc[i];
                            match (acc.as_ref(), h.vals[i]) {
                                (Some(av), Some(v)) => {
                                    *acc = Some(f.add(av, &f.mul(&c[i], &f.to_element(v))));
                                }
                                _ => *acc = None,
                            }
                        }
                    }
                }
            }
            for e in 0..edges.len() {
                est[e].sending_parent_keys += sent[e] as u64;
                for (k, pk) in per[e].drain() {
                    let wants = cfg.exact.wants(&k);
                    let m = maps[e].entry(k.clone()).or_insert(Acc { senders: 0, first: pk.first, exact_senders: 0, exact_level: false, first_exact: None });
                    m.senders += 1;
                    m.first = min_ref(w, m.first, pk.first);
                    if !wants {
                        continue;
                    }
                    est[e].exact_pairs += 1;
                    m.exact_level = true;
                    let witness = pk.acc.iter().enumerate().any(|(i, v)| v.as_ref().is_some_and(|v| !fields[i].is_zero(v)));
                    if witness {
                        est[e].exact_by_witness += 1;
                        m.exact_senders += 1;
                        m.first_exact = Some(m.first_exact.map_or(pk.first, |f| min_ref(w, f, pk.first)));
                    } else {
                        fallback.push((ci, e, k, pk.first));
                    }
                }
            }
        }
        Ok((maps, est, other, fallback))
    });
    let mut maps: Vec<HashMap<Vec<i32>, Acc>> = edges.iter().map(|_| HashMap::new()).collect();
    let mut est: Vec<EdgeStats> = edges.iter().map(|_| EdgeStats::default()).collect();
    let mut other: BTreeMap<u64, u64> = BTreeMap::new();
    let mut fallback = vec![];
    for r in res {
        let (ms, es, ot, fb) = r?;
        for (e, m) in ms.into_iter().enumerate() {
            for (k, a) in m {
                match maps[e].get_mut(&k) {
                    None => {
                        maps[e].insert(k, a);
                    }
                    Some(b) => {
                        b.senders += a.senders;
                        b.first = min_ref(w, a.first, b.first);
                        b.exact_senders += a.exact_senders;
                        b.exact_level |= a.exact_level;
                        b.first_exact = match (a.first_exact, b.first_exact) {
                            (Some(x), Some(y)) => Some(min_ref(w, x, y)),
                            (x, y) => x.or(y),
                        };
                    }
                }
            }
        }
        for (e, s) in es.into_iter().enumerate() {
            let t = &mut est[e];
            t.sending_parent_keys += s.sending_parent_keys;
            t.handdown_terms_routed += s.handdown_terms_routed;
            t.image_terms += s.image_terms;
            t.lower_child_sector_terms += s.lower_child_sector_terms;
            t.exact_pairs += s.exact_pairs;
            t.exact_by_witness += s.exact_by_witness;
            t.unknown_terms_sending += s.unknown_terms_sending;
        }
        for (m, c) in ot {
            *other.entry(m).or_default() += c;
        }
        fallback.extend(fb);
    }
    // the exact symbolic fallback (zero at both points or a denominator zero mod p), grouped by record
    if !fallback.is_empty() {
        let mut by_rec: BTreeMap<(usize, usize), Vec<(usize, usize, Vec<i32>, SenderRef)>> = BTreeMap::new();
        for x in fallback {
            let r = &rows.served[w.cl_served[x.0]];
            by_rec.entry((r.table, r.record)).or_default().push(x);
        }
        let groups: Vec<_> = by_rec.into_iter().collect();
        let res = par_map(&groups, cfg.threads, |((t, r), items)| -> Result<Vec<(usize, Vec<i32>, SenderRef, bool)>, String> {
            let tr = Transporter::new(alg, n)?;
            cache.with(*t, *r, |ent| {
                let p = &mut ent.p;
                let mut out = vec![];
                for (ci, e, k, first) in items {
                    let row = &rows.served[w.cl_served[*ci]];
                    let pt = p.point(&row.applied).ok_or("no rule point")?;
                    let vals: Vec<Integer> = pt.iter().map(|&x| Integer::from(x as i64)).collect();
                    let reps: Vec<(usize, &Integer)> = vals.iter().enumerate().collect();
                    let mut parts: Vec<(Rational, QPoly, Option<QPoly>)> = vec![];
                    for h in &w.hand[*ci] {
                        if owners.get(&h.mask) != Some(e) {
                            continue;
                        }
                        let sec = edges[*e].sector(h.mask).expect("owner");
                        let img = tr.apply(sec, &h.tk)?;
                        let Ok(pos) = img.binary_search_by(|x| x.0.as_slice().cmp(k.as_slice())) else { continue };
                        let (num, den) = p.term_polys(h.j as usize)?;
                        let nq = num.replace_multiple(&reps, true).map_coeff(|c| Rational::from(c), Q);
                        let dq = den.map(|d| d.replace_multiple(&reps, true).map_coeff(|c| Rational::from(c), Q));
                        parts.push((img[pos].1.clone(), nq, dq));
                    }
                    // a contributing term whose denominator vanishes identically at the key (0/0) is undecided: it sends
                    if parts.iter().any(|(_, _, d)| d.as_ref().is_some_and(|d| d.is_zero())) {
                        out.push((*e, k.clone(), *first, true));
                        continue;
                    }
                    // sum_t T_t N_t prod_{s != t} D_s == 0 ?
                    let mut total: Option<QPoly> = None;
                    for (a, (c, nq, _)) in parts.iter().enumerate() {
                        let mut term = nq.clone().mul_coeff(c.clone());
                        for (b, (_, _, d)) in parts.iter().enumerate() {
                            if a != b
                                && let Some(d) = d
                            {
                                term = &term * d;
                            }
                        }
                        total = Some(match total {
                            None => term,
                            Some(t) => &t + &term,
                        });
                    }
                    out.push((*e, k.clone(), *first, total.is_some_and(|t| !t.is_zero())));
                }
                Ok(out)
            })
            .inspect(|_| cache.release(*t, *r))
        });
        for r in res {
            for (e, k, first, nz) in r? {
                let m = maps[e].get_mut(&k).expect("acc");
                if nz {
                    est[e].exact_by_symbolic += 1;
                    m.exact_senders += 1;
                    m.first_exact = Some(m.first_exact.map_or(first, |f| min_ref(w, f, first)));
                } else {
                    est[e].exact_cancelled += 1;
                }
            }
        }
    }
    let sender = |(ci, _j, hi): SenderRef| -> Sender {
        let row = &rows.served[w.cl_served[ci]];
        let h = &w.hand[ci][hi];
        Sender {
            parent_key: row.key.clone(),
            applied: row.applied.clone(),
            record_sha256: set.rules[row.table][row.record].record_sha256.clone(),
            term: h.j,
            term_key: h.tk.clone(),
            by: h.by.clone(),
        }
    };
    let mut out = vec![];
    for (e, m) in maps.into_iter().enumerate() {
        let mut lines: Vec<ArrivalLine> = m
            .into_iter()
            .map(|(k, a)| ArrivalLine {
                key: k,
                senders: a.senders,
                exact_senders: a.exact_level.then_some(a.exact_senders),
                first: sender(a.first),
                first_exact: a.first_exact.map(sender),
            })
            .collect();
        lines.sort_by(|a, b| a.key.cmp(&b.key));
        let mut s = est[e].clone();
        s.seconds = t0.elapsed().as_secs_f64();
        out.push(EdgeResult { lines, stats: s });
    }
    Ok((out, other))
}

/// The header of an arrival file (bindings).
#[allow(clippy::too_many_arguments)]
pub fn header(
    parent: &str,
    spec_tables: &[(String, String)],
    rows: &ParentRows,
    w: &Walk,
    edge: &EdgeFile,
    edge_producer: &str,
    frame_checks: &Value,
    cfg: &WalkCfg,
) -> Value {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    let mut ts: Vec<&(String, String)> = spec_tables.iter().collect();
    ts.sort();
    for (p, s) in &ts {
        h.update(format!("{p}\t{s}\n").as_bytes());
    }
    json!({"semantics": rustred_campaign_core::import::arrivals::ARRIVAL_SEMANTICS,
        "parent": {"node": parent, "tables": ts.iter().map(|(p, s)| json!({"path": p, "sha256": s})).collect::<Vec<_>>(),
            "tables_digest": h.finish_hex(), "rows": {"path": rows.path, "sha256": rows.sha256,
            "keys": rows.keys.len(), "served": rows.served.len()},
            "start_keys": w.stats.start_keys, "closure_keys": w.stats.closure_keys, "closure_digest": w.closure_digest},
        "child": {"label": edge.child.label, "lines": edge.child.table_lines},
        "edge": edge.identity(), "edge_producer": edge_producer, "frame_checks": frame_checks,
        "decision": {"structural": cfg.structural, "exact_level": cfg.exact.word(),
            "primes": PRIMES.iter().map(|(p, d)| json!({"p": p, "d": d})).collect::<Vec<_>>(),
            "max_planes": cfg.prover.max_planes, "max_factor_terms": cfg.prover.max_factor_terms},
        "level": "support (per term, no cancellation assumed) + exact per sender (total coefficient)",
        "tool": {"program": "rr_arrivals", "version": TOOL_VERSION, "crate": env!("CARGO_PKG_VERSION"),
            "symbolica": super::zero_proofs::SYMBOLICA_REV}})
}
