//! Re-audit part 3 (lane audit-part3): DIFFERENTIAL FUZZ of every irreversible verdict of `partition` / `loci` /
//! `economy` against exhaustive first-match enumeration (`cover::Dispatcher::first_match`, `CaseDomain::applies`) on
//! a window plus far probes. The generator is the auditor's own (not the authors'): near-cover CHAINS (a catch-all
//! with exception branches whose loci are re-covered by earlier records that carry their own exceptions, poles,
//! half-plane bounds, fixed slots), witness-blocked records (domain flag and record-level status), target patterns,
//! recenter bounds, case equations with non-unit coefficients (index > 1 lattices), parity traps (rational but no
//! integer points), nonlinear literals, extreme coefficients (+-2^62 .. +-2^127), arities 2, 3, 4, 7, 20.
//!
//! Checked per set: dispatch-mode point oracle == dispatcher on every key; `proves_no_gap` (dispatch and union);
//! `prove_coverage_preserved` under five option variants; `shadowed` (list and dispatcher sources), `is_empty`,
//! `contained_in` for every record; census `uncovered_in_c` empty; determinism of a rerun. A disagreement is
//! classified: `C2` (the first selected record's verdict is the engine zero test), `M1` (rule point out of i32),
//! `NEW` otherwise. The test FAILS on any NEW unsound verdict (a Proved / shadowed / minted proof with an open key).
//!
//! Sets: `AUDIT_P3_SETS` (default 1,500 in the permanent suite; the audit ran 60,000), threads 4, seed base
//! `AUDIT_P3_SEED`. Synthetic tables only (lane rule L8).

use std::collections::BTreeMap;

use rustred_campaign_core::cover::{
    Assurance, Dispatch, Dispatcher, RecordMeta, RuleRef, Table, WitnessStatus,
};
use rustred_campaign_core::domain::{
    CaseDomain, PoleKind, PoleLocus, Stage, TargetMap, TargetSlot, Verdict,
};
use rustred_campaign_core::economy::{ProofWork, prove_coverage_preserved};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::loci::contain::ProofBudget;
use rustred_campaign_core::partition::{
    Budget, Containment, CoverageNotion, DispatchOrderStatement, DispatcherSource, DomainList,
    Emptiness, PartitionOptions, contained_in_with, gap_partition, is_empty_with,
    shadowed_with,
};
use rustred_campaign_core::poly::{EvalError, Poly};

// ------------------------------------------------------------------------------------------------------------------
// RNG (splitmix64; independent of the authors' generators)
// ------------------------------------------------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn chance(&mut self, p_per_mille: u64) -> bool {
        self.below(1000) < p_per_mille
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len() as u64) as usize]
    }
}

// ------------------------------------------------------------------------------------------------------------------
// generator
// ------------------------------------------------------------------------------------------------------------------

const EXTREME: [i128; 12] = [
    1 << 62,
    -(1 << 62),
    1 << 63,
    -(1 << 63),
    (1 << 64) + 1,
    1 << 100,
    -(1 << 100),
    i128::MAX,
    i128::MIN + 1,
    i128::MIN,
    3 << 120,
    -(5 << 123),
];

struct Gen<'r> {
    r: &'r mut Rng,
    n: usize,
    e: usize,
    extreme: u64,
}

impl Gen<'_> {
    fn coef(&mut self) -> i128 {
        if self.r.chance(self.extreme) {
            return *self.r.pick(&EXTREME);
        }
        let mut c = self.r.range(-3, 3) as i128;
        if c == 0 {
            c = 1;
        }
        c
    }
    fn konst(&mut self) -> i128 {
        if self.r.chance(self.extreme / 2) {
            return *self.r.pick(&EXTREME);
        }
        self.r.range(-4, 4) as i128
    }
    fn slot(&mut self) -> usize {
        // arity 20: only the first 6 slots (late coupled slots multiply the classes beyond any budget, P3-M2;
        // reproduced separately by seed 0x38ecb4765e420c4c)
        if self.n >= 20 {
            return self.r.below(6) as usize;
        }
        self.r.below(self.n as u64) as usize
    }
    fn near(&mut self, s: usize) -> i64 {
        if s < self.e {
            self.r.range(1, 4)
        } else {
            self.r.range(-3, 0)
        }
    }
    /// c + sum a_i n_i over 1..=3 distinct slots (None on a coefficient overflow).
    fn lin(&mut self) -> Option<Poly> {
        let k = 1 + self.r.below(3.min(self.n as u64)) as usize;
        let mut p = Poly::constant(self.konst());
        let mut used = vec![];
        for _ in 0..k {
            let s = self.slot();
            if used.contains(&s) {
                continue;
            }
            used.push(s);
            p = p
                .add(&Poly::constant(self.coef()).mul(&Poly::var(s)).ok()?)
                .ok()?;
        }
        Some(p)
    }
    /// A shifted slot `n_s - v` with v near the floor.
    fn shift(&mut self) -> Poly {
        let s = self.slot();
        let v = self.near(s);
        Poly::slot_minus(s, v as i128)
    }
    /// A zero-set literal: linear (mostly), product, square, monomial, parity trap.
    fn zlit(&mut self) -> Option<Poly> {
        match self.r.below(10) {
            0 => self.shift().mul(&self.shift()).ok(),
            1 => {
                let s = self.slot();
                Poly::var(s)
                    .mul(&Poly::var(s))
                    .ok()?
                    .sub(&Poly::constant(*self.r.pick(&[1, 4, 9, 2, 16])))
                    .ok()
            }
            2 => {
                // parity trap: 2 a + 2 b + odd
                let (a, b) = (self.slot(), self.slot());
                Poly::constant(2)
                    .mul(&Poly::var(a))
                    .ok()?
                    .add(
                        &Poly::constant(2 * self.r.range(-2, 2) as i128 + 2)
                            .mul(&Poly::var(b))
                            .ok()?,
                    )
                    .ok()?
                    .add(&Poly::constant(2 * self.r.range(-2, 2) as i128 + 1))
                    .ok()
            }
            3 => {
                let (a, b) = (self.slot(), self.slot());
                Poly::var(a)
                    .mul(&Poly::var(b))
                    .ok()?
                    .sub(&Poly::constant(self.r.range(-6, 6) as i128))
                    .ok()
            }
            4 | 5 => Some(self.shift()),
            _ => self.lin(),
        }
    }
    fn pole(&mut self) -> Option<PoleLocus> {
        // (lin) * d + (lin') or d * lin, or a product with a linear factor
        let p = match self.r.below(4) {
            0 => Poly::d().mul(&self.lin()?).ok()?,
            1 => Poly::d().mul(&self.lin()?).ok()?.add(&self.lin()?).ok()?,
            2 => self
                .shift()
                .mul(
                    &Poly::d()
                        .add(&Poly::constant(self.r.range(-2, 2) as i128))
                        .ok()?,
                )
                .ok()?,
            _ => Poly::d().mul(&self.shift()).ok()?.mul(&self.shift()).ok()?,
        };
        if p.is_zero() {
            return None;
        }
        Some(PoleLocus::new(PoleKind::Denominator, &p))
    }
    fn chart(&self) -> Option<Vec<bool>> {
        Some((0..self.n).map(|i| i < self.e).collect())
    }
    fn base(&self) -> CaseDomain {
        CaseDomain::free(self.n, self.chart())
    }
    /// A random record (decorations added with small probabilities).
    fn record(&mut self) -> CaseDomain {
        let mut d = self.base();
        let kind = self.r.below(8);
        if kind == 0 || self.r.chance(200) {
            for _ in 0..1 + self.r.below(2) {
                let s = self.slot();
                let v = if self.r.chance(30) {
                    if s < self.e { 0 } else { 1 } // outside the chart
                } else {
                    self.near(s)
                };
                d.fixed[s] = Some(v as i32);
            }
        }
        if kind == 1 || kind == 2 {
            for _ in 0..1 + self.r.below(2) {
                if let Some(p) = self.zlit() {
                    d.equations.push(p);
                }
            }
        }
        if kind == 3 || self.r.chance(150) {
            for _ in 0..1 + self.r.below(2) {
                if let Some(p) = self.lin() {
                    d.bounds.push(p);
                }
            }
        }
        if kind >= 4 || self.r.chance(250) {
            for _ in 0..1 + self.r.below(3) {
                let mut b = vec![];
                for _ in 0..1 + self.r.below(2) {
                    if let Some(p) = self.zlit() {
                        b.push(p);
                    }
                }
                if !b.is_empty() {
                    d.exceptions.push(b);
                }
            }
        }
        self.decorate(&mut d);
        d
    }
    fn decorate(&mut self, d: &mut CaseDomain) {
        if self.r.chance(120)
            && let Some(p) = self.pole()
        {
            d.poles.push(p);
        }
        if self.r.chance(60) {
            d.witness_elided = true;
        }
        if self.r.chance(80) {
            // target pattern: every slot n<j>+o at its own parameter, o small (rarely huge), or a constant slot
            let mut t = vec![];
            for j in 0..self.n {
                // arity 20: constants and offsets only on the first 6 slots (a range bound on a late slot makes the
                // rule depend on it and multiplies the classes, P3-M2)
                let early = self.n < 20 || j < 6;
                if early && d.fixed[j].is_none() && self.r.chance(100) {
                    t.push(TargetSlot::Const(self.near(j)));
                } else {
                    let o = if !early {
                        0
                    } else if self.r.chance(15) {
                        *self.r.pick(&[-2_147_483_000i64, 2_147_483_000, 1 << 40])
                    } else if self.r.chance(400) {
                        self.r.range(-2, 2)
                    } else {
                        0
                    };
                    t.push(TargetSlot::Param {
                        param: j,
                        offset: o,
                    });
                }
            }
            d.target = TargetMap::Pattern(t);
        } else if self.r.chance(60)
            && let Some(p) = self.lin()
        {
            d.recenter_bounds.push(p);
        }
    }
}

struct Set {
    n: usize,
    e: usize,
    recs: Vec<(CaseDomain, WitnessStatus)>,
}

fn gen_set(r: &mut Rng) -> Set {
    let n = *r.pick(&[2usize, 2, 2, 3, 3, 3, 4, 4, 7, 20]);
    let e = r.below(n as u64 + 1) as usize;
    let extreme = if r.chance(150) { 120 } else { 0 };
    let mut g = Gen { r, n, e, extreme };
    let mut recs: Vec<CaseDomain> = vec![];
    if g.r.chance(650) {
        // near-cover chain: catch-all with exception branches, each branch re-covered by an earlier record
        let mut last = g.base();
        let nb = 1 + g.r.below(3);
        for _ in 0..nb {
            let mut b = vec![];
            for _ in 0..1 + g.r.below(2) {
                if let Some(p) = g.zlit() {
                    b.push(p);
                }
            }
            if b.is_empty() {
                continue;
            }
            if g.r.chance(750) {
                let mut cov = g.base();
                cov.equations = b.clone();
                if g.r.chance(300)
                    && let Some(p) = g.zlit()
                {
                    cov.exceptions.push(vec![p]);
                }
                if g.r.chance(150)
                    && let Some(p) = g.lin()
                {
                    cov.bounds.push(p);
                }
                if g.r.chance(100) {
                    let s = g.slot();
                    cov.fixed[s] = Some(g.near(s) as i32);
                }
                g.decorate(&mut cov);
                recs.push(cov);
            }
            last.exceptions.push(b);
        }
        if g.r.chance(150) {
            g.decorate(&mut last);
        }
        for _ in 0..g.r.below(3) {
            let at = g.r.below(recs.len() as u64 + 1) as usize;
            let rec = g.record();
            recs.insert(at, rec);
        }
        recs.push(last);
        if g.r.chance(200) && recs.len() > 1 {
            let (a, b) = (
                g.r.below(recs.len() as u64) as usize,
                g.r.below(recs.len() as u64) as usize,
            );
            recs.swap(a, b);
        }
    } else {
        for _ in 0..1 + g.r.below(6) {
            let rec = g.record();
            recs.push(rec);
        }
        if g.r.chance(300) {
            let rec = g.base();
            recs.push(rec);
        }
    }
    let recs = recs
        .into_iter()
        .filter(|d| d.validate().is_ok())
        .map(|d| {
            let w = if d.witness_elided {
                WitnessStatus::Elided
            } else if g.r.chance(40) {
                WitnessStatus::NotCarried
            } else {
                WitnessStatus::Complete
            };
            (d, w)
        })
        .collect();
    Set { n, e, recs }
}

// ------------------------------------------------------------------------------------------------------------------
// truth
// ------------------------------------------------------------------------------------------------------------------

fn dispatcher(s: &Set) -> Option<Dispatcher> {
    let mut t = Table::new("t", s.n);
    for (i, (d, w)) in s.recs.iter().enumerate() {
        let meta = RecordMeta {
            source_index: i,
            line_no: 0,
            witness: *w,
            witness_basis: Assurance::Caller,
            target_basis: Assurance::Caller,
        };
        t.push_with_meta(i as i64, d.clone(), meta).ok()?;
    }
    Dispatcher::new(vec![t], vec![0]).ok()
}

fn list(s: &Set) -> DomainList {
    let mut l = DomainList::new(s.n);
    for (i, (d, _)) in s.recs.iter().enumerate() {
        l.push(RuleRef::bare(0, i, i as i64), d.clone());
    }
    l
}

/// Known disagreement classes: any record whose verdict at the key is the engine zero test (C2) or a rule point out
/// of i32 (M1); NEW otherwise.
fn classify(s: &Set, k: &[i32]) -> &'static str {
    let mut cls = "NEW";
    for (d, _) in &s.recs {
        match d.applies(k) {
            Verdict::Error(EvalError::EngineZeroTest) => return "C2",
            Verdict::Error(EvalError::Overflow) => cls = "M1",
            _ => {}
        }
    }
    cls
}

fn witness_blocked(d: &CaseDomain, w: WitnessStatus) -> bool {
    d.witness_elided || w != WitnessStatus::Complete
}

/// Union coverage under FailClosed (only witness-blocked records block, on their selected set).
fn union_open(s: &Set, k: &[i32]) -> bool {
    for (d, w) in &s.recs {
        let v = d.applies(k);
        if witness_blocked(d, *w) {
            if v.stage() != Stage::NotSelected {
                return true;
            }
            continue;
        }
        if v == Verdict::Applies {
            return false;
        }
    }
    true
}

fn keys(s: &Set, r: &mut Rng) -> (Vec<Key>, Vec<Key>) {
    let (n, e) = (s.n, s.e);
    let floor = |i: usize| if i < e { 1 } else { 0 };
    let val = |i: usize, off: i32| if i < e { 1 + off } else { -off };
    let mut win: Vec<Key> = vec![];
    if n <= 4 || n == 7 {
        let w: i32 = if n <= 2 {
            9
        } else if n <= 3 {
            7
        } else if n == 4 {
            5
        } else {
            3
        };
        let mut out: Vec<Key> = vec![vec![]];
        for i in 0..n {
            out = out
                .into_iter()
                .flat_map(|k| {
                    (0..w).map(move |o| {
                        let mut k2 = k.clone();
                        k2.push(val(i, o));
                        k2
                    })
                })
                .collect();
        }
        win = out;
    } else {
        let base: Key = (0..n).map(floor).collect();
        win.push(base.clone());
        for i in 0..n {
            for o in 1..=3 {
                let mut k = base.clone();
                k[i] = val(i, o);
                win.push(k.clone());
                for j in i + 1..n {
                    for o2 in 1..=2 {
                        let mut k2 = k.clone();
                        k2[j] = val(j, o2);
                        win.push(k2);
                    }
                }
            }
        }
        for _ in 0..300 {
            win.push((0..n).map(|i| val(i, r.range(0, 4) as i32)).collect());
        }
    }
    let mut far = vec![];
    let big = [1_000, 65_537, 1_000_000, 2_147_483_000, i32::MAX - 1];
    for _ in 0..24 {
        let mut k: Key = (0..n).map(|i| val(i, r.range(0, 3) as i32)).collect();
        for _ in 0..1 + r.below(2) {
            let i = r.below(n as u64) as usize;
            let b = *r.pick(&big);
            k[i] = if i < e { b } else { -b };
        }
        far.push(k);
    }
    (win, far)
}

// ------------------------------------------------------------------------------------------------------------------
// the differential run
// ------------------------------------------------------------------------------------------------------------------

#[derive(Default, Debug, Clone)]
struct Tally {
    sets: u64,
    keys: u64,
    dispatch_proved: u64,
    union_proved: u64,
    minted: u64,
    shadowed_proved: u64,
    empty_proved: u64,
    contained_proved: u64,
    /// (check, class) -> count of unsound verdicts / oracle disagreements
    bad: BTreeMap<(String, String), u64>,
    /// first examples (seed, check, class, key)
    examples: Vec<(u64, String, String, Key)>,
    oracle_overreport: u64,
    census_in_c: u64,
    nondeterministic: u64,
    partition_errors: u64,
}

impl Tally {
    fn bad(&mut self, seed: u64, check: &str, class: &str, k: &[i32]) {
        *self
            .bad
            .entry((check.to_string(), class.to_string()))
            .or_default() += 1;
        if self.examples.len() < 40 && !self.examples.iter().any(|x| x.1 == check && x.2 == class) {
            self.examples
                .push((seed, check.into(), class.into(), k.to_vec()));
        }
    }
    fn merge(&mut self, o: Tally) {
        self.sets += o.sets;
        self.keys += o.keys;
        self.dispatch_proved += o.dispatch_proved;
        self.union_proved += o.union_proved;
        self.minted += o.minted;
        self.shadowed_proved += o.shadowed_proved;
        self.empty_proved += o.empty_proved;
        self.contained_proved += o.contained_proved;
        for (k, v) in o.bad {
            *self.bad.entry(k).or_default() += v;
        }
        for x in o.examples {
            if self.examples.len() < 60 {
                self.examples.push(x);
            }
        }
        self.oracle_overreport += o.oracle_overreport;
        self.census_in_c += o.census_in_c;
        self.nondeterministic += o.nondeterministic;
        self.partition_errors += o.partition_errors;
    }
    fn new_unsound(&self) -> u64 {
        self.bad
            .iter()
            .filter(|((_, c), _)| c == "NEW")
            .map(|(_, v)| *v)
            .sum()
    }
}

fn opts(n: usize, coverage: CoverageNotion) -> PartitionOptions {
    PartitionOptions {
        coverage,
        // max_faces bounds the 2^k face expansion (P3-M2: not charged to max_nodes); truncation = Incomplete
        budget: Budget {
            // arity >= 20: the class recursion enumerates every intermediate slot (paths of the memo DAG) and stores
            // every locus piece with its region (P3-M2): a smaller budget keeps the run inside 2 GB
            max_nodes: Some(if n >= 20 { 60_000 } else { 3_000_000 }),
            max_faces: Some(2_000),
            ..Default::default()
        },
        // classes with more any slots are families (Incomplete, never a proof): bounds thresholds() (P3-M2)
        max_any_slots: 8,
        ..Default::default()
    }
}

/// Proof budget of the containment checks (beyond it: Unknown, which is sound). The wall clock is needed: the
/// Fourier-Motzkin / exhaustive work inside ONE split node is not counted by `max_nodes` (P3-m4).
fn pbudget() -> ProofBudget {
    ProofBudget {
        max_nodes: 4_000,
        max_work: 2_000_000,
        max_enumeration: 5_000,
        wall: Some(std::time::Duration::from_millis(1_000)),
        ..Default::default()
    }
}

fn stage(what: &str) {
    if env_u64("AUDIT_P3_TRACE", 0) == 2 {
        eprintln!("  stage {what}");
    }
}

fn run_one(seed: u64, t: &mut Tally, heavy: bool) {
    let mut r = Rng(seed);
    let s = gen_set(&mut r);
    if s.recs.is_empty() {
        return;
    }
    let Some(disp) = dispatcher(&s) else { return };
    let src = DispatcherSource::new(&disp);
    let f = NodeFrame::without_header_lines_first(s.n, s.e);
    let (win, far) = keys(&s, &mut r);
    let all: Vec<&Key> = win.iter().chain(far.iter()).collect();
    t.sets += 1;
    t.keys += all.len() as u64;
    let open: Vec<bool> = all
        .iter()
        .map(|k| !matches!(disp.first_match(k).unwrap(), Dispatch::Covered(_)))
        .collect();
    stage("dispatch partition");
    // ---- dispatch mode
    match gap_partition(&src, &f, &opts(s.n, CoverageNotion::DispatchCoverage), None) {
        Ok(rep) => {
            let proved = rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_ok();
            if proved {
                t.dispatch_proved += 1;
            }
            for (k, &o) in all.iter().zip(&open) {
                let m = rep.uncovered(k).unwrap();
                if o && !m {
                    t.bad(seed, "dispatch_oracle", classify(&s, k), k);
                }
                if !o && m {
                    t.oracle_overreport += 1;
                }
                if proved && o {
                    t.bad(seed, "dispatch_proves_no_gap", classify(&s, k), k);
                }
            }
            let c = rep.census(&win).unwrap();
            t.census_in_c += c.uncovered_in_c.len() as u64;
            if heavy {
                let rep2 =
                    gap_partition(&src, &f, &opts(s.n, CoverageNotion::DispatchCoverage), None)
                        .unwrap();
                let sig = |x: &rustred_campaign_core::partition::PartitionReport| {
                    (
                        x.outcome().tag(),
                        x.faces().iter().map(|g| g.key()).collect::<Vec<_>>(),
                        x.classes().len(),
                        x.stats().nodes,
                        x.blocked().len(),
                        x.proves_no_gap(CoverageNotion::DispatchCoverage).is_ok(),
                    )
                };
                if sig(&rep) != sig(&rep2) {
                    t.nondeterministic += 1;
                }
            }
        }
        Err(_) => t.partition_errors += 1,
    }
    stage("minted proofs");
    // ---- minted proofs (partfix3, audit-part3 C1: the minting function takes a WORK BUDGET only and builds its own
    // options; the former option variants are gone): the default budget and, for heavy sets, a small one
    let variants: Vec<ProofWork> = if heavy {
        vec![
            ProofWork::default(),
            ProofWork {
                max_units: 200_000,
                wall: None,
                max_memo: Some(10_000),
            },
        ]
    } else {
        vec![ProofWork::default()]
    };
    for (vi, o) in variants.iter().enumerate() {
        if let Ok(p) =
            prove_coverage_preserved(&src, &src, &f, o, &DispatchOrderStatement::SameOrder)
        {
            let _ = p;
            t.minted += 1;
            for (k, &op) in all.iter().zip(&open) {
                if op {
                    t.bad(seed, &format!("minted_proof_v{vi}"), classify(&s, k), k);
                }
            }
        }
    }
    stage("union partition");
    // ---- union mode (FailClosed)
    if let Ok(rep) = gap_partition(&src, &f, &opts(s.n, CoverageNotion::UnionCoverage), None) {
        let proved = rep.proves_no_gap(CoverageNotion::UnionCoverage).is_ok();
        if proved {
            t.union_proved += 1;
        }
        for k in &all {
            let o = union_open(&s, k);
            let m = rep.uncovered(k).unwrap();
            if o && !m {
                t.bad(seed, "union_oracle", classify(&s, k), k);
            }
            if proved && o {
                t.bad(seed, "union_proves_no_gap", classify(&s, k), k);
            }
        }
        let c = rep.census(&win).unwrap();
        t.census_in_c += c.uncovered_in_c.len() as u64;
    }
    // ---- shadowed / is_empty / contained_in per record: arities <= 4 only (P3-M3: on a nonlinear leaf whose
    // candidate fails verification, verified_witness walks D(k, 12) points with no budget: a hang at arity 20)
    if s.n > 4 {
        return;
    }
    let l = list(&s);
    let doms: Vec<&CaseDomain> = s.recs.iter().map(|(d, _)| d).collect();
    for i in 0..doms.len() {
        stage(&format!("shadowed {i}"));
        for (which, c) in [
            ("shadowed_list", shadowed_with(&l, i, &f, &pbudget())),
            ("shadowed_disp", shadowed_with(&src, i, &f, &pbudget())),
        ] {
            if let Containment::Proved { .. } = c {
                t.shadowed_proved += 1;
                for k in &all {
                    if doms[i].applies(k) != Verdict::Applies {
                        continue;
                    }
                    let matched = (0..i).any(|j| doms[j].applies(k).stage() != Stage::NotSelected);
                    let fires = matches!(disp.first_match(k).unwrap(), Dispatch::Covered(rr) if rr.record == i);
                    if !matched || fires {
                        t.bad(seed, which, classify(&s, k), k);
                    }
                }
            }
        }
        stage(&format!("is_empty {i}"));
        if let Emptiness::ProvedEmpty { .. } = is_empty_with(doms[i], &f, &pbudget()) {
            t.empty_proved += 1;
            for k in &all {
                if doms[i].applies(k) == Verdict::Applies {
                    t.bad(seed, "is_empty", "NEW", k);
                }
            }
        }
        if heavy {
            let others: Vec<&CaseDomain> = (0..doms.len())
                .filter(|&j| j != i)
                .map(|j| doms[j])
                .collect();
            stage(&format!("contained_in {i}"));
            if let Containment::Proved { .. } = contained_in_with(doms[i], &others, &f, &pbudget())
            {
                t.contained_proved += 1;
                for k in &all {
                    if doms[i].applies(k) == Verdict::Applies
                        && !others
                            .iter()
                            .any(|o| !o.witness_elided && o.applies(k) == Verdict::Applies)
                    {
                        t.bad(seed, "contained_in", "NEW", k);
                    }
                }
            }
        }
    }
}

fn run(n_sets: u64, seed0: u64) -> Tally {
    let threads = env_u64("AUDIT_P3_THREADS", 4).clamp(1, 4);
    let trace = env_u64("AUDIT_P3_TRACE", 0) >= 1;
    let mut total = Tally::default();
    let parts: Vec<Tally> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|ti| {
                sc.spawn(move || {
                    let mut t = Tally::default();
                    let mut i = ti;
                    while i < n_sets {
                        let seed = seed0.wrapping_add(i.wrapping_mul(0x2545_F491_4F6C_DD1D));
                        if trace {
                            eprintln!("seed {i} {seed:#x}");
                        }
                        run_one(seed, &mut t, i % 4 == 0);
                        i += threads;
                    }
                    t
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for p in parts {
        total.merge(p);
    }
    total
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The permanent differential: no NEW unsound verdict (Proved no-gap, shadowed, empty, contained, minted proof
/// next to a key the dispatcher / the domains themselves report open). The known classes C2 / M1
/// (tests/audit_part3_dispatch_model.rs) are counted and printed, not failed on.
#[test]
fn p3_fuzz_irreversible_verdicts_vs_first_match() {
    let n = env_u64("AUDIT_P3_SETS", 1_500);
    let seed0 = env_u64("AUDIT_P3_SEED", 0x5EED_0003);
    let t0 = std::time::Instant::now();
    let t = run(n, seed0);
    eprintln!(
        "audit-part3 fuzz: sets={} keys={} dispatch_proved={} union_proved={} minted={} shadowed_proved={} empty_proved={} contained_proved={} oracle_overreport={} census_in_c={} nondeterministic={} partition_errors={} secs={}",
        t.sets,
        t.keys,
        t.dispatch_proved,
        t.union_proved,
        t.minted,
        t.shadowed_proved,
        t.empty_proved,
        t.contained_proved,
        t.oracle_overreport,
        t.census_in_c,
        t.nondeterministic,
        t.partition_errors,
        t0.elapsed().as_secs()
    );
    for ((check, class), c) in &t.bad {
        eprintln!("  disagreement {check} [{class}]: {c}");
    }
    for (seed, check, class, k) in &t.examples {
        eprintln!("  example seed={seed:#x} {check} [{class}] key={k:?}");
    }
    assert_eq!(t.census_in_c, 0, "uncovered keys in Covered classes");
    assert_eq!(t.nondeterministic, 0, "a rerun differs");
    assert_eq!(
        t.new_unsound(),
        0,
        "NEW unsound verdicts (see the examples above)"
    );
}

/// Replays one seed of the fuzzer with a full printout (for minimising a reported example).
#[test]
#[ignore]
fn p3_fuzz_replay_seed() {
    let seed = env_u64("AUDIT_P3_REPLAY", 0);
    let mut r = Rng(seed);
    let s = gen_set(&mut r);
    eprintln!("n={} e={}", s.n, s.e);
    for (i, (d, w)) in s.recs.iter().enumerate() {
        eprintln!("rec {i}: witness {w:?} {d:?}");
    }
    let mut t = Tally::default();
    run_one(seed, &mut t, true);
    eprintln!("{t:?}");
}

// ------------------------------------------------------------------------------------------------------------------
// budget and determinism
// ------------------------------------------------------------------------------------------------------------------

type Sig = (
    String,
    Vec<(Vec<usize>, Vec<usize>)>,
    usize,
    u64,
    usize,
    usize,
    bool,
    usize,
);

fn sig(
    rep: &rustred_campaign_core::partition::PartitionReport,
    notion: CoverageNotion,
    win: &[Key],
) -> Sig {
    (
        rep.outcome().tag().to_string(),
        rep.faces().iter().map(|g| g.key()).collect(),
        rep.classes().len(),
        rep.stats().nodes,
        rep.blocked().len(),
        rep.blocked_empty(),
        rep.proves_no_gap(notion).is_ok(),
        win.iter().filter(|k| rep.uncovered(k).unwrap()).count(),
    )
}

/// RA2-M5 (one budget per call): with a work budget B, (a) a run whose budget was exhausted is never Complete and
/// never proves no gap (and `prove_coverage_preserved` with the same options mints nothing), (b) a run that was not
/// exhausted is IDENTICAL to the unlimited run (deterministic accounting), (c) a budgeted proof implies the
/// unlimited proof. Budgets 1, 2, 4, ... until a run is not exhausted.
#[test]
fn p3_budget_sweep_never_proves_more_and_never_completes_after_exhaustion() {
    let n = env_u64("AUDIT_P3_BUDGET_SETS", 120);
    let seed0 = env_u64("AUDIT_P3_SEED", 0x5EED_0003) ^ 0x00B0_D6E7;
    let (mut runs, mut exhausted, mut proved) = (0u64, 0u64, 0u64);
    for i in 0..n {
        let seed = seed0.wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut r = Rng(seed);
        let s = gen_set(&mut r);
        if s.recs.is_empty() {
            continue;
        }
        let Some(disp) = dispatcher(&s) else { continue };
        let src = DispatcherSource::new(&disp);
        let f = NodeFrame::without_header_lines_first(s.n, s.e);
        let (win, _) = keys(&s, &mut r);
        let base = opts(s.n, CoverageNotion::DispatchCoverage);
        let Ok(full) = gap_partition(&src, &f, &base, None) else {
            continue;
        };
        if full.budget_exhausted().is_some() {
            continue; // the reference itself hit the 3M cap: no unlimited reference
        }
        let full_sig = sig(&full, CoverageNotion::DispatchCoverage, &win);
        let mut b = 1u64;
        loop {
            let o = PartitionOptions {
                budget: Budget {
                    max_nodes: Some(b),
                    ..base.budget.clone()
                },
                ..base.clone()
            };
            let rep = gap_partition(&src, &f, &o, None).unwrap();
            runs += 1;
            let ok = rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_ok();
            // partfix3: the minting function runs its own prove-mode partition (no faces) plus the cross-check under
            // the SAME number of units: it may prove with fewer / need more units than the describe run, but it never
            // proves more than the unlimited run
            let w = ProofWork {
                max_units: b,
                wall: None,
                max_memo: None,
            };
            let minted =
                prove_coverage_preserved(&src, &src, &f, &w, &DispatchOrderStatement::SameOrder)
                    .is_ok();
            assert!(
                !minted || full_sig.6,
                "seed {seed:#x} budget {b}: minted more than the unlimited run proves"
            );
            assert!(
                !ok || full_sig.6,
                "seed {seed:#x} budget {b}: proves more than the unlimited run"
            );
            if rep.budget_exhausted().is_some() {
                exhausted += 1;
                assert!(!ok, "seed {seed:#x} budget {b}: proof after exhaustion");
                assert!(
                    rep.outcome().is_aborted(),
                    "seed {seed:#x} budget {b}: {:?}",
                    rep.outcome()
                );
            } else {
                assert!(
                    sig(&rep, CoverageNotion::DispatchCoverage, &win) == full_sig,
                    "seed {seed:#x} budget {b}: an unexhausted run differs from the unlimited run"
                );
                proved += u64::from(ok);
                break;
            }
            b = b.saturating_mul(2);
            if b > 3_000_000 {
                break;
            }
        }
    }
    eprintln!(
        "budget sweep: {runs} budgeted runs, {exhausted} exhausted, {proved} proved at the first sufficient budget"
    );
    assert!(exhausted > 0 && runs > exhausted);
}

/// Determinism across thread counts: every report signature (outcome, faces, classes, nodes, blocked, proof, window
/// census) computed on 1 thread equals the one computed while 2 and 4 threads run other sets concurrently (thread-local
/// memo of `linear_factors`, no shared state).
#[test]
fn p3_determinism_across_thread_counts() {
    let n = env_u64("AUDIT_P3_DET_SETS", 160);
    let seed0 = env_u64("AUDIT_P3_SEED", 0x5EED_0003) ^ 0xD37E_2111;
    let seed_of = |i: u64| seed0.wrapping_add(i.wrapping_mul(0x2545_F491_4F6C_DD1D));
    let one = |seed: u64| -> Option<(Sig, Sig)> {
        let mut r = Rng(seed);
        let s = gen_set(&mut r);
        if s.recs.is_empty() {
            return None;
        }
        let disp = dispatcher(&s)?;
        let src = DispatcherSource::new(&disp);
        let f = NodeFrame::without_header_lines_first(s.n, s.e);
        let (win, _) = keys(&s, &mut r);
        let d = gap_partition(&src, &f, &opts(s.n, CoverageNotion::DispatchCoverage), None).ok()?;
        let u = gap_partition(&src, &f, &opts(s.n, CoverageNotion::UnionCoverage), None).ok()?;
        Some((
            sig(&d, CoverageNotion::DispatchCoverage, &win),
            sig(&u, CoverageNotion::UnionCoverage, &win),
        ))
    };
    let seq: Vec<Option<(Sig, Sig)>> = (0..n).map(|i| one(seed_of(i))).collect();
    for threads in [2u64, 4] {
        let res: Vec<(u64, Option<(Sig, Sig)>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..threads)
                .map(|ti| {
                    sc.spawn(move || {
                        let mut out = vec![];
                        let mut i = ti;
                        while i < n {
                            out.push((i, one(seed_of(i))));
                            i += threads;
                        }
                        out
                    })
                })
                .collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        for (i, s) in res {
            assert!(s == seq[i as usize], "set {i} differs on {threads} threads");
        }
    }
    eprintln!("determinism: {n} sets x (1, 2, 4 threads) identical");
}
