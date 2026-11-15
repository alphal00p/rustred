//! Re-audit part 4 (lane audit-part4): the auditor's OWN differential fuzzer of the minting function
//! `economy::prove_coverage_preserved` against the REAL core dispatcher (`cover::Dispatcher::first_match`), biased
//! towards what the earlier fuzzers (audit_part2_oracle, audit_part3_fuzz) did not stress: SPARSE / FAR zero sets
//! (gaps placed far from the floor where no probe key of `dispatch_probes` and no small window looks), intersections
//! of literal zero sets (solved exactly by the oracle's key generator), Pell-type quadratics with exponentially sparse
//! integer points, products of shifted slots, target patterns (permutations, offsets, constants), fixed slots,
//! engine-readable bounds, witness poles and witness statuses, near-cover chains (a catch-all whose exception loci are
//! re-covered by patches, then perturbed).
//!
//! For every MINTED proof the oracle checks every key of (a) the floor box `lines 1..=B, numerators -(B-1)..=0` and
//! (b) the product of per-slot CRITICAL values (closure of: floor values, every integer root of every literal solved
//! for one slot with the others at critical values, +-1 around each, 3 rounds; rule-point values mapped to key slots),
//! (c) random far keys. Any key that is not `Covered` is a FALSE PROOF (the test fails and prints the set and key).
//! Optionally (env `AUDIT_P4_ENGINE_DIR`) every set with a minted proof is written as a `table_dump.v1` file together
//! with its oracle keys and the core dispatcher's winning labels, for the engine binary's `rr_node first-match`.
//!
//! Env: `AUDIT_P4_SETS` (default 400 in the suite), `AUDIT_P4_SEED`, `AUDIT_P4_BOX` (default 9), `AUDIT_P4_THREADS`
//! (default 1, at most 4). Synthetic tables only (lane rule L8).

use std::collections::BTreeSet;
use std::io::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};

use rustred_campaign_core::cover::{
    Assurance, Dispatch, Dispatcher, RecordMeta, Table, WitnessStatus,
};
use rustred_campaign_core::domain::{CaseDomain, PoleKind, PoleLocus, TargetMap, TargetSlot};
use rustred_campaign_core::economy::{ProofWork, prove_coverage_preserved};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::partition::{DispatchOrderStatement, DispatcherSource};
use rustred_campaign_core::poly::Poly;

// ------------------------------------------------------------------------------------------------------------------
// RNG
// ------------------------------------------------------------------------------------------------------------------

#[derive(Clone)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64* (independent of the earlier auditors' splitmix)
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn chance(&mut self, pm: u64) -> bool {
        self.below(1000) < pm
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Small polynomials (own representation: exact i128, for the oracle's root solving and for the text)
// ------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct P {
    n: usize,
    terms: Vec<(Vec<u32>, i128)>,
}

impl P {
    fn konst(n: usize, c: i128) -> P {
        P {
            n,
            terms: vec![(vec![0; n], c)],
        }
        .norm()
    }
    fn var(n: usize, s: usize) -> P {
        let mut e = vec![0; n];
        e[s] = 1;
        P {
            n,
            terms: vec![(e, 1)],
        }
    }
    fn norm(mut self) -> P {
        self.terms.sort();
        let mut out: Vec<(Vec<u32>, i128)> = vec![];
        for (e, c) in self.terms {
            if let Some(l) = out.last_mut()
                && l.0 == e
            {
                l.1 += c;
                continue;
            }
            out.push((e, c));
        }
        out.retain(|t| t.1 != 0);
        P { n: self.n, terms: out }
    }
    fn add(&self, o: &P) -> P {
        let mut t = self.terms.clone();
        t.extend(o.terms.iter().cloned());
        P { n: self.n, terms: t }.norm()
    }
    fn scale(&self, k: i128) -> P {
        P {
            n: self.n,
            terms: self.terms.iter().map(|(e, c)| (e.clone(), c * k)).collect(),
        }
        .norm()
    }
    fn mul(&self, o: &P) -> P {
        let mut t = vec![];
        for (a, x) in &self.terms {
            for (b, y) in &o.terms {
                t.push((a.iter().zip(b).map(|(p, q)| p + q).collect(), x * y));
            }
        }
        P { n: self.n, terms: t }.norm()
    }
    fn is_const(&self) -> bool {
        self.terms.iter().all(|(e, _)| e.iter().all(|&x| x == 0))
    }
    fn support(&self) -> Vec<usize> {
        (0..self.n)
            .filter(|&s| self.terms.iter().any(|(e, _)| e[s] > 0))
            .collect()
    }
    fn eval(&self, pt: &[i128]) -> Option<i128> {
        let mut acc: i128 = 0;
        for (e, c) in &self.terms {
            let mut t = *c;
            for (s, &k) in e.iter().enumerate() {
                for _ in 0..k {
                    t = t.checked_mul(pt[s])?;
                }
            }
            acc = acc.checked_add(t)?;
        }
        Some(acc)
    }
    /// Coefficients (by degree) of the polynomial in slot `s`, the other slots at `pt`.
    fn univ(&self, s: usize, pt: &[i128]) -> Option<Vec<i128>> {
        let deg = self.terms.iter().map(|(e, _)| e[s]).max().unwrap_or(0) as usize;
        let mut c = vec![0i128; deg + 1];
        for (e, k) in &self.terms {
            let mut t = *k;
            for (v, &x) in e.iter().enumerate() {
                if v == s {
                    continue;
                }
                for _ in 0..x {
                    t = t.checked_mul(pt[v])?;
                }
            }
            c[e[s] as usize] = c[e[s] as usize].checked_add(t)?;
        }
        Some(c)
    }
    fn text(&self) -> String {
        if self.terms.is_empty() {
            return "0".into();
        }
        let mut out = String::new();
        for (i, (e, c)) in self.terms.iter().enumerate() {
            let mono: Vec<String> = e
                .iter()
                .enumerate()
                .filter(|(_, x)| **x > 0)
                .map(|(s, x)| {
                    if *x == 1 {
                        format!("n{s}")
                    } else {
                        format!("n{s}^{x}")
                    }
                })
                .collect();
            let (sign, a) = if *c < 0 { ("-", -c) } else { ("+", *c) };
            if i == 0 {
                if sign == "-" {
                    out.push('-');
                }
            } else {
                out.push_str(sign);
            }
            if mono.is_empty() {
                out.push_str(&a.to_string());
            } else if a == 1 {
                out.push_str(&mono.join("*"));
            } else {
                out.push_str(&format!("{a}*{}", mono.join("*")));
            }
        }
        out
    }
    fn poly(&self) -> Poly {
        Poly::parse_d_free_exact(&self.text(), self.n).expect("own literal text parses")
    }
}

fn isqrt(v: i128) -> Option<i128> {
    if v < 0 {
        return None;
    }
    let mut r = (v as f64).sqrt() as i128;
    while r > 0 && r.checked_mul(r)? > v {
        r -= 1;
    }
    while (r + 1).checked_mul(r + 1)? <= v {
        r += 1;
    }
    Some(r)
}

/// Integer roots of a univariate polynomial (degree <= 2 exactly; higher degree: divisors of a small constant term).
fn int_roots(c: &[i128]) -> Vec<i128> {
    let mut c = c.to_vec();
    while c.len() > 1 && *c.last().unwrap() == 0 {
        c.pop();
    }
    match c.len() {
        0 | 1 => vec![],
        2 => {
            if c[1] != 0 && c[0] % c[1] == 0 {
                vec![-c[0] / c[1]]
            } else {
                vec![]
            }
        }
        3 => {
            let (a, b, k) = (c[2], c[1], c[0]);
            let Some(disc) = b.checked_mul(b).and_then(|x| x.checked_sub(4 * a * k)) else {
                return vec![];
            };
            let Some(r) = isqrt(disc) else { return vec![] };
            if r * r != disc {
                return vec![];
            }
            let mut out = vec![];
            for num in [-b + r, -b - r] {
                if num % (2 * a) == 0 {
                    out.push(num / (2 * a));
                }
            }
            out
        }
        _ => {
            let k = c[0];
            if k == 0 {
                let mut r = int_roots(&c[1..]);
                r.push(0);
                return r;
            }
            if k.unsigned_abs() > 100_000 {
                return vec![];
            }
            let mut out = vec![];
            for d in 1..=k.unsigned_abs() as i128 {
                if k % d == 0 {
                    for x in [d, -d] {
                        let mut v: i128 = 0;
                        let mut ok = true;
                        for &a in c.iter().rev() {
                            match v.checked_mul(x).and_then(|y| y.checked_add(a)) {
                                Some(y) => v = y,
                                None => {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        if ok && v == 0 {
                            out.push(x);
                        }
                    }
                }
            }
            out
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Records and sets
// ------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Tgt {
    Canonical,
    /// per key slot: (param, offset) or a constant
    Pattern(Vec<Result<(usize, i64), i64>>),
}

#[derive(Clone, Debug)]
struct Rec {
    fixed: Vec<Option<i32>>,
    tgt: Tgt,
    eqs: Vec<P>,
    bounds: Vec<P>,
    exc: Vec<Vec<P>>,
    /// witness poles (text with d)
    poles: Vec<String>,
    witness: WitnessStatus,
}

#[derive(Clone, Debug)]
struct Set {
    n: usize,
    e: usize,
    recs: Vec<Rec>,
}

struct Gen<'a> {
    r: &'a mut Rng,
    n: usize,
    e: usize,
}

impl Gen<'_> {
    fn floor(&self, s: usize) -> i64 {
        if s < self.e { 1 } else { 0 }
    }
    /// a value of slot s, far with some probability
    fn val(&mut self, s: usize) -> i64 {
        let off = if self.r.chance(250) {
            self.r.range(8, 60)
        } else {
            self.r.range(0, 4)
        };
        if s < self.e { 1 + off } else { -off }
    }
    fn slot(&mut self) -> usize {
        self.r.below(self.n as u64) as usize
    }
    fn shift(&mut self) -> P {
        let s = self.slot();
        let v = self.val(s);
        P::var(self.n, s).add(&P::konst(self.n, -(v as i128)))
    }
    fn lin(&mut self) -> P {
        let n = self.n;
        let k = 1 + self.r.below(3.min(n as u64)) as usize;
        let mut p = P::konst(n, 0);
        let mut used = vec![];
        for _ in 0..k {
            let s = self.slot();
            if used.contains(&s) {
                continue;
            }
            used.push(s);
            let a = *[1i128, -1, 2, -2, 3, -3, 5, 7, -4]
                .get(self.r.below(9) as usize)
                .unwrap();
            p = p.add(&P::var(n, s).scale(a));
        }
        // constant: put the zero set far with some probability
        let c = if self.r.chance(350) {
            self.r.range(-80, 80) as i128
        } else {
            self.r.range(-5, 5) as i128
        };
        p.add(&P::konst(n, c))
    }
    /// a zero-set literal of various sparse shapes
    fn zlit(&mut self) -> P {
        let n = self.n;
        match self.r.below(12) {
            0 => self.shift().mul(&self.shift()),
            1 => {
                // Pell-type x^2 - D y^2 - 1 (D in {2, 3, 5, 6, 7}): integer points grow exponentially
                let (a, b) = (self.slot(), self.slot());
                let d = *[2i128, 3, 5, 6, 7].get(self.r.below(5) as usize).unwrap();
                let k = *[1i128, -1, 4, -2].get(self.r.below(4) as usize).unwrap();
                P::var(n, a)
                    .mul(&P::var(n, a))
                    .add(&P::var(n, b).mul(&P::var(n, b)).scale(-d))
                    .add(&P::konst(n, -k))
            }
            2 => {
                // x*y - N with N composite (few lattice points, some far)
                let (a, b) = (self.slot(), self.slot());
                let nn = *[6i128, 12, 30, 35, 77, 221, 391, 1001, -15, -42]
                    .get(self.r.below(10) as usize)
                    .unwrap();
                P::var(n, a).mul(&P::var(n, b)).add(&P::konst(n, -nn))
            }
            3 => {
                // x^2 - y - c (parabola: every x gives one y)
                let (a, b) = (self.slot(), self.slot());
                let c = self.r.range(-6, 6) as i128;
                P::var(n, a)
                    .mul(&P::var(n, a))
                    .add(&P::var(n, b).scale(-1))
                    .add(&P::konst(n, c))
            }
            4 => {
                // parity trap / lattice: 2a + 4b + odd (no integer point) or 3a - 6b + 3c
                let (a, b) = (self.slot(), self.slot());
                let odd = 2 * self.r.range(-20, 20) as i128 + 1;
                let lat = self.r.chance(500);
                let k = if lat { 3 } else { 2 };
                P::var(n, a)
                    .scale(k)
                    .add(&P::var(n, b).scale(2 * k))
                    .add(&P::konst(n, if lat { 3 * odd } else { odd }))
            }
            5..=7 => self.shift(),
            _ => self.lin(),
        }
    }
    /// an engine-readable bound (one slot: line n >= m, numerator n <= M, at most 64 excluded)
    fn bound(&mut self) -> P {
        let n = self.n;
        let s = self.slot();
        if s < self.e {
            let m = self.r.range(1, 12) as i128;
            P::var(n, s).add(&P::konst(n, -m))
        } else {
            let mm = -(self.r.range(0, 12) as i128);
            P::var(n, s).scale(-1).add(&P::konst(n, mm))
        }
    }
    fn target(&mut self, fixed: &[Option<i32>]) -> Tgt {
        if self.r.chance(800) {
            return Tgt::Canonical;
        }
        let n = self.n;
        // a permutation of the parameters (within line slots / numerator slots), offsets, rarely a constant
        let mut perm: Vec<usize> = (0..n).collect();
        if self.r.chance(500) && self.e >= 2 {
            perm.swap(0, 1);
        }
        let mut t = vec![];
        for j in 0..n {
            if self.r.chance(80) {
                let v = if j < self.e { self.r.range(1, 6) } else { -self.r.range(0, 5) };
                t.push(Err(v));
            } else {
                let o = if self.r.chance(500) { self.r.range(-3, 3) } else { 0 };
                t.push(Ok((perm[j], o)));
            }
        }
        let _ = fixed;
        Tgt::Pattern(t)
    }
    fn pole(&mut self) -> String {
        let l = self.lin().text();
        match self.r.below(3) {
            0 => format!("d*({l})"),
            1 => format!("(d-2)*({l})"),
            _ => format!("(d*({l})+({}))", self.shift().text()),
        }
    }
    fn record(&mut self, catch_all: bool) -> Rec {
        let n = self.n;
        let mut r = Rec {
            fixed: vec![None; n],
            tgt: Tgt::Canonical,
            eqs: vec![],
            bounds: vec![],
            exc: vec![],
            poles: vec![],
            witness: WitnessStatus::Complete,
        };
        if !catch_all {
            match self.r.below(5) {
                0 | 1 => {
                    let s = self.slot();
                    r.fixed[s] = Some(self.val(s) as i32);
                }
                2 | 3 => r.eqs.push(self.zlit()),
                _ => {
                    r.bounds.push(self.bound());
                }
            }
        }
        let nb = if catch_all {
            1 + self.r.below(3)
        } else {
            self.r.below(3)
        };
        for _ in 0..nb {
            let m = 1 + self.r.below(2);
            r.exc.push((0..m).map(|_| self.zlit()).collect());
        }
        if self.r.chance(120) {
            r.bounds.push(self.bound());
        }
        if self.r.chance(150) {
            r.tgt = self.target(&r.fixed);
        }
        if self.r.chance(80) {
            r.poles.push(self.pole());
        }
        if self.r.chance(25) {
            r.witness = if self.r.chance(500) {
                WitnessStatus::Elided
            } else {
                WitnessStatus::NotCarried
            };
        }
        r
    }
    /// a patch for an exception branch: applies on (a superset of) its zero set
    fn patch(&mut self, br: &[P]) -> Rec {
        let mut r = self.record(false);
        r.eqs.clear();
        r.fixed = vec![None; self.n];
        r.bounds.clear();
        r.tgt = Tgt::Canonical;
        // pick one member as the patch's equation (the patch applies on the member's zero set)
        let m = &br[self.r.below(br.len() as u64) as usize];
        // a one-slot shift -> fixed slot; otherwise an equation
        let sup = m.support();
        if sup.len() == 1 && m.terms.len() == 2 && self.r.chance(600) {
            let s = sup[0];
            let a = m.terms.iter().find(|(e, _)| e[s] == 1).map(|t| t.1);
            let c = m.terms.iter().find(|(e, _)| e[s] == 0).map(|t| t.1);
            if let (Some(a), Some(c)) = (a, c)
                && a != 0
                && c % a == 0
                && i32::try_from(-c / a).is_ok()
            {
                r.fixed[s] = Some((-c / a) as i32);
            } else {
                r.eqs.push(m.clone());
            }
        } else {
            r.eqs.push(m.clone());
        }
        // patches carry fewer exceptions
        if self.r.chance(500) {
            r.exc.clear();
        }
        r
    }
}

fn gen_set(seed: u64) -> Set {
    let mut rng = Rng(seed | 1);
    let r = &mut rng;
    let n = [2usize, 2, 3, 3, 3, 4][r.below(6) as usize];
    let e = r.range(1, n as i64) as usize;
    let mut g = Gen { r, n, e };
    let ca = g.record(true);
    let mut recs = vec![];
    // near-cover chain: patches for the catch-all's branches, patches of patches, perturbations
    let mut pending: Vec<Vec<P>> = ca.exc.clone();
    let mut depth = 0;
    while let Some(br) = pending.pop() {
        if depth > 6 {
            break;
        }
        depth += 1;
        if g.r.chance(80) {
            continue; // drop a patch: a gap (must be refused)
        }
        let p = g.patch(&br);
        pending.extend(p.exc.iter().cloned());
        recs.push(p);
    }
    // a few unrelated records
    for _ in 0..g.r.below(3) {
        let x = g.record(false);
        recs.push(x);
    }
    // order: patches first (reverse creation order), random swaps, the catch-all last or (rarely) earlier
    recs.reverse();
    for _ in 0..g.r.below(3) {
        let len = recs.len();
        if len >= 2 {
            let (a, b) = (g.r.below(len as u64) as usize, g.r.below(len as u64) as usize);
            recs.swap(a, b);
        }
    }
    if g.r.chance(100) && !recs.is_empty() {
        let at = g.r.below(recs.len() as u64) as usize;
        recs.insert(at, ca);
    } else {
        recs.push(ca);
    }
    // perturb one constant of one literal (a near miss)
    if g.r.chance(300) {
        let i = g.r.below(recs.len() as u64) as usize;
        let rec = &mut recs[i];
        let delta = if g.r.chance(500) { 1 } else { -1 };
        if let Some(br) = rec.exc.first_mut()
            && let Some(m) = br.first_mut()
        {
            *m = m.add(&P::konst(n, delta));
        } else if let Some(eq) = rec.eqs.first_mut() {
            *eq = eq.add(&P::konst(n, delta));
        }
    }
    Set { n, e, recs }
}

// ------------------------------------------------------------------------------------------------------------------
// Building the core objects and the table file
// ------------------------------------------------------------------------------------------------------------------

fn mask(s: &Set) -> Vec<bool> {
    (0..s.n).map(|i| i < s.e).collect()
}

fn target_map(r: &Rec, n: usize) -> TargetMap {
    match &r.tgt {
        Tgt::Canonical => TargetMap::Canonical,
        Tgt::Pattern(t) => {
            let _ = n;
            TargetMap::Pattern(
                t.iter()
                    .map(|x| match x {
                        Ok((p, o)) => TargetSlot::Param {
                            param: *p,
                            offset: *o,
                        },
                        Err(c) => TargetSlot::Const(*c),
                    })
                    .collect(),
            )
        }
    }
}

fn domain(s: &Set, r: &Rec) -> Option<CaseDomain> {
    let mut d = CaseDomain::free(s.n, Some(mask(s)));
    d.fixed = r.fixed.clone();
    d.target = target_map(r, s.n);
    d.equations = r.eqs.iter().map(P::poly).collect();
    d.bounds = r.bounds.iter().map(P::poly).collect();
    d.exceptions = r
        .exc
        .iter()
        .map(|b| b.iter().map(P::poly).collect())
        .collect();
    for p in &r.poles {
        let q = Poly::parse_exact(p, s.n, true).ok()?;
        if q.is_zero() {
            return None;
        }
        d.poles.push(PoleLocus::new(PoleKind::Witness, &q));
    }
    d.validate().ok()?;
    Some(d)
}

fn dispatcher(s: &Set) -> Option<Dispatcher> {
    let mut t = Table::new("fuzz", s.n);
    for (i, r) in s.recs.iter().enumerate() {
        let d = domain(s, r)?;
        let meta = RecordMeta {
            source_index: i,
            line_no: 0,
            witness: r.witness,
            witness_basis: Assurance::Record,
            target_basis: Assurance::Record,
        };
        t.push_with_meta(i as i64, d, meta).ok()?;
    }
    Dispatcher::new(vec![t], vec![0])
        .ok()?
        .with_chart(mask(s))
        .ok()
}

fn tgt_text(r: &Rec, n: usize) -> String {
    let args: Vec<String> = match &r.tgt {
        Tgt::Canonical => (0..n)
            .map(|j| match r.fixed[j] {
                Some(v) => v.to_string(),
                None => format!("n{j}"),
            })
            .collect(),
        Tgt::Pattern(t) => t
            .iter()
            .map(|x| match x {
                Ok((p, 0)) => format!("n{p}"),
                Ok((p, o)) if *o > 0 => format!("n{p}+{o}"),
                Ok((p, o)) => format!("n{p}{o}"),
                Err(c) => c.to_string(),
            })
            .collect(),
    };
    format!("I({})", args.join(","))
}

/// The set as an engine `table_dump.v1` file (bounds as `domain_bounds` where they are one-slot min / max).
fn table_text(s: &Set) -> String {
    let m: String = mask(s).iter().map(|&b| if b { '1' } else { '0' }).collect();
    let mut out = vec![
        format!(
            r#"{{"type":"header","schema":"table_dump.v1","family":"audit-p4-fuzz","denominators":{},"mask":"{m}"}}"#,
            s.n
        ),
        {
            let g: Vec<String> = mask(s)
                .iter()
                .map(|&b| if b { "\"n>=1\"".into() } else { "\"n<=0\"".into() })
                .collect();
            format!(r#"{{"type":"chart","chart":"{m}","guards":[{}]}}"#, g.join(","))
        },
    ];
    let js = |v: &[String]| -> String {
        format!(
            "[{}]",
            v.iter()
                .map(|x| serde_json::to_string(x).unwrap())
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    for (i, r) in s.recs.iter().enumerate() {
        let fixed: Vec<String> = r
            .fixed
            .iter()
            .map(|f| f.map_or("null".into(), |v| v.to_string()))
            .collect();
        let eqs: Vec<String> = r.eqs.iter().map(P::text).collect();
        let exc: Vec<String> = r
            .exc
            .iter()
            .map(|b| js(&b.iter().map(P::text).collect::<Vec<_>>()))
            .collect();
        let mut extra = String::new();
        if !r.bounds.is_empty() {
            let mut mn = vec!["null".to_string(); s.n];
            let mut mx = vec!["null".to_string(); s.n];
            for b in &r.bounds {
                let sup = b.support();
                let sl = sup[0];
                let a = b.terms.iter().find(|(e, _)| e[sl] == 1).unwrap().1;
                let c = b.terms.iter().find(|(e, _)| e[sl] == 0).map_or(0, |t| t.1);
                if a > 0 {
                    mn[sl] = (-c).to_string();
                } else {
                    mx[sl] = c.to_string();
                }
            }
            extra = format!(
                r#","domain_bounds":{{"min":[{}],"max":[{}]}}"#,
                mn.join(","),
                mx.join(",")
            );
        }
        let wl = match r.witness {
            WitnessStatus::Complete => format!(r#","witness_loci":{}"#, js(&r.poles)),
            WitnessStatus::Elided => r#","witness_loci":["elided"]"#.to_string(),
            WitnessStatus::NotCarried => String::new(),
        };
        out.push(format!(
            r#"{{"type":"rule","chart":"{m}","index":{i},"fixed":[{}],"equations":{},"exceptions":[{}],"target":"{}","rhs":[["I({})","1"]],"dispatch":"Partition"{wl}{extra}}}"#,
            fixed.join(","),
            js(&eqs),
            exc.join(","),
            tgt_text(r, s.n),
            vec!["0"; s.n].join(","),
        ));
    }
    out.join("\n") + "\n"
}

// ------------------------------------------------------------------------------------------------------------------
// Oracle keys
// ------------------------------------------------------------------------------------------------------------------

fn in_chart(s: &Set, k: usize, v: i128) -> bool {
    if k < s.e { v >= 1 } else { v <= 0 }
}

/// Per-slot critical values: floor box, roots of every literal solved for one slot with the others at critical values
/// (3 rounds), rule-point values mapped to key slots through every target pattern.
fn oracle_keys(s: &Set, bx: i64, rng: &mut Rng) -> Vec<Vec<i32>> {
    let n = s.n;
    let mut vals: Vec<BTreeSet<i128>> = (0..n)
        .map(|k| {
            (0..bx)
                .map(|o| if k < s.e { 1 + o as i128 } else { -(o as i128) })
                .collect()
        })
        .collect();
    let lits: Vec<(&Rec, &P)> = s
        .recs
        .iter()
        .flat_map(|r| {
            r.eqs
                .iter()
                .chain(r.bounds.iter())
                .chain(r.exc.iter().flatten())
                .map(move |p| (r, p))
        })
        .collect();
    // rule point <-> key slot maps of a record
    let to_key = |r: &Rec, a: usize, v: i128| -> Vec<(usize, i128)> {
        match &r.tgt {
            Tgt::Canonical => vec![(a, v)],
            Tgt::Pattern(t) => t
                .iter()
                .enumerate()
                .filter_map(|(j, x)| match x {
                    Ok((p, o)) if *p == a => Some((j, v + *o as i128)),
                    _ => None,
                })
                .collect(),
        }
    };
    for _round in 0..3 {
        let snapshot = vals.clone();
        for (r, p) in &lits {
            let sup = p.support();
            for &sl in &sup {
                let others: Vec<usize> = sup.iter().copied().filter(|&x| x != sl).collect();
                // assignments of the other support slots from the critical values (capped)
                let mut assigns: Vec<Vec<(usize, i128)>> = vec![vec![]];
                for &o in &others {
                    let cand: Vec<i128> = snapshot[o].iter().copied().collect();
                    let mut next = vec![];
                    for a in &assigns {
                        for &v in cand.iter().take(40) {
                            let mut b = a.clone();
                            b.push((o, v));
                            next.push(b);
                        }
                    }
                    assigns = next;
                    if assigns.len() > 1600 {
                        assigns.truncate(1600);
                    }
                }
                for a in assigns {
                    let mut pt = vec![0i128; n];
                    for (o, v) in &a {
                        pt[*o] = *v;
                    }
                    if let Some(u) = p.univ(sl, &pt) {
                        for x in int_roots(&u) {
                            for d in [-1i128, 0, 1] {
                                let y = x + d;
                                for (ks, kv) in to_key(r, sl, y) {
                                    if in_chart(s, ks, kv) && i32::try_from(kv).is_ok() {
                                        vals[ks].insert(kv);
                                    }
                                }
                                if in_chart(s, sl, y) && i32::try_from(y).is_ok() {
                                    vals[sl].insert(y);
                                }
                            }
                        }
                    }
                }
            }
        }
        // fixed values / constants and their neighbours
        for r in &s.recs {
            for (k, f) in r.fixed.iter().enumerate() {
                if let Some(v) = f {
                    for d in [-1i128, 0, 1] {
                        let y = *v as i128 + d;
                        if in_chart(s, k, y) {
                            vals[k].insert(y);
                        }
                    }
                }
            }
        }
        for v in vals.iter_mut() {
            if v.len() > 120 {
                // keep the floor box and a spread of the rest
                let all: Vec<i128> = v.iter().copied().collect();
                let keep: BTreeSet<i128> = all
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i < bx as usize || i % (all.len() / 100 + 1) == 0)
                    .map(|(_, x)| *x)
                    .collect();
                *v = keep;
            }
        }
    }
    let total: f64 = vals.iter().map(|v| v.len() as f64).product();
    let mut keys: Vec<Vec<i32>> = vec![];
    if total <= 300_000.0 {
        keys.push(vec![]);
        for v in &vals {
            let mut next = vec![];
            for k in &keys {
                for &x in v {
                    let mut kk = k.clone();
                    kk.push(x as i32);
                    next.push(kk);
                }
            }
            keys = next;
        }
    } else {
        // the floor box in full (capped), then random combinations of critical values
        let mut boxk: Vec<Vec<i32>> = vec![vec![]];
        for k in 0..n {
            let mut next = vec![];
            for b in &boxk {
                for o in 0..bx {
                    let mut bb = b.clone();
                    bb.push(if k < s.e { 1 + o as i32 } else { -(o as i32) });
                    next.push(bb);
                }
            }
            boxk = next;
        }
        keys.extend(boxk);
        let lists: Vec<Vec<i128>> = vals.iter().map(|v| v.iter().copied().collect()).collect();
        for _ in 0..100_000 {
            keys.push(
                lists
                    .iter()
                    .map(|l| l[rng.below(l.len() as u64) as usize] as i32)
                    .collect(),
            );
        }
    }
    // random far keys
    for _ in 0..300 {
        keys.push(
            (0..n)
                .map(|k| {
                    let m = *[10i64, 100, 1000, 100_000, 1 << 30]
                        .get(rng.below(5) as usize)
                        .unwrap();
                    let v = rng.range(0, m);
                    if k < s.e { 1 + v as i32 } else { -(v as i32) }
                })
                .collect(),
        );
    }
    keys
}

// ------------------------------------------------------------------------------------------------------------------
// Driver
// ------------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Tally {
    sets: AtomicU64,
    built: AtomicU64,
    minted: AtomicU64,
    refused: AtomicU64,
    keys: AtomicU64,
    false_proofs: AtomicU64,
    refused_but_covered_on_oracle: AtomicU64,
    written: AtomicU64,
}

fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

fn run_one(seed: u64, bx: i64, t: &Tally, engine_dir: Option<&std::path::Path>) {
    t.sets.fetch_add(1, Ordering::Relaxed);
    let s = gen_set(seed);
    let Some(disp) = dispatcher(&s) else { return };
    t.built.fetch_add(1, Ordering::Relaxed);
    let frame = NodeFrame::without_header_lines_first(s.n, s.e);
    let src = DispatcherSource::new(&disp);
    let work = ProofWork {
        max_units: 3_000_000,
        wall: Some(std::time::Duration::from_secs(4)),
        max_memo: Some(50_000),
    };
    let res = prove_coverage_preserved(&src, &src, &frame, &work, &DispatchOrderStatement::SameOrder);
    let mut rng = Rng(seed ^ 0x9E37_79B9_7F4A_7C15);
    match res {
        Ok(_) => {
            t.minted.fetch_add(1, Ordering::Relaxed);
            let keys = oracle_keys(&s, bx, &mut rng);
            t.keys.fetch_add(keys.len() as u64, Ordering::Relaxed);
            let mut labels = vec![];
            for k in &keys {
                match disp.first_match(k).unwrap() {
                    Dispatch::Covered(r) => labels.push(r.label),
                    other => {
                        t.false_proofs.fetch_add(1, Ordering::Relaxed);
                        eprintln!(
                            "FALSE PROOF seed {seed:#x}: key {k:?} is {other:?}\nset: {s:#?}\ntable:\n{}",
                            table_text(&s)
                        );
                        return;
                    }
                }
            }
            if let Some(dir) = engine_dir {
                let w = t.written.fetch_add(1, Ordering::Relaxed);
                if w < 4000 {
                    let base = dir.join(format!("s{seed:016x}"));
                    std::fs::write(base.with_extension("table.jsonl"), table_text(&s)).unwrap();
                    let mut f = std::fs::File::create(base.with_extension("keys.jsonl")).unwrap();
                    let mut g = std::fs::File::create(base.with_extension("labels.txt")).unwrap();
                    // the engine reads keys as i64 JSON arrays; keep at most 3000 keys per set
                    for (k, l) in keys.iter().zip(&labels).take(3000) {
                        writeln!(f, "{}", serde_json::to_string(k).unwrap()).unwrap();
                        writeln!(g, "{l}").unwrap();
                    }
                }
            }
        }
        Err(_) => {
            t.refused.fetch_add(1, Ordering::Relaxed);
            // usefulness probe: is some oracle key actually open? (counted only)
            if rng.chance(100) {
                let keys = oracle_keys(&s, bx.min(6), &mut rng);
                if keys
                    .iter()
                    .all(|k| matches!(disp.first_match(k).unwrap(), Dispatch::Covered(_)))
                {
                    t.refused_but_covered_on_oracle.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
}

#[test]
fn p4_fuzz_minted_proofs_vs_first_match_on_far_and_critical_keys() {
    let n_sets = env_u64("AUDIT_P4_SETS", 400);
    let seed0 = env_u64("AUDIT_P4_SEED", 0xA4D1_7000_0000_0001);
    let bx = env_u64("AUDIT_P4_BOX", 9) as i64;
    let threads = env_u64("AUDIT_P4_THREADS", 1).clamp(1, 4);
    let engine_dir = std::env::var_os("AUDIT_P4_ENGINE_DIR").map(std::path::PathBuf::from);
    if let Some(d) = &engine_dir {
        std::fs::create_dir_all(d).unwrap();
    }
    let t = Tally::default();
    let next = AtomicU64::new(0);
    let t0 = std::time::Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n_sets {
                        break;
                    }
                    let seed = seed0.wrapping_add(i.wrapping_mul(0x1000_0000_01B3));
                    run_one(seed, bx, &t, engine_dir.as_deref());
                }
            });
        }
    });
    eprintln!(
        "P4 FUZZ: sets {} built {} minted {} refused {} oracle keys {} FALSE PROOFS {} refused-but-oracle-covered {} engine files {} ({:.1} s)",
        t.sets.load(Ordering::Relaxed),
        t.built.load(Ordering::Relaxed),
        t.minted.load(Ordering::Relaxed),
        t.refused.load(Ordering::Relaxed),
        t.keys.load(Ordering::Relaxed),
        t.false_proofs.load(Ordering::Relaxed),
        t.refused_but_covered_on_oracle.load(Ordering::Relaxed),
        t.written.load(Ordering::Relaxed),
        t0.elapsed().as_secs_f64()
    );
    assert_eq!(t.false_proofs.load(Ordering::Relaxed), 0, "false proofs");
}

#[test]
fn p4_fuzz_replay_seed() {
    let Some(seed) = std::env::var("AUDIT_P4_REPLAY")
        .ok()
        .and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok())
    else {
        return;
    };
    let s = gen_set(seed);
    eprintln!("{s:#?}\n{}", table_text(&s));
    let t = Tally::default();
    run_one(seed, env_u64("AUDIT_P4_BOX", 9) as i64, &t, None);
    eprintln!(
        "minted {} false {}",
        t.minted.load(Ordering::Relaxed),
        t.false_proofs.load(Ordering::Relaxed)
    );
}
