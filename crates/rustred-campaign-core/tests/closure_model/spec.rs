//! Node specifications (serialisable), random generation and construction of the crate objects.
//! Ported from audit loop-B (implementer-scratch/audit-loop-B/.../examples/audit_b/spec.rs, bin_v16), unchanged except
//! for this header.
#![allow(dead_code)]
use rustred_campaign_core::closure::{KeyCodec, KeySet, RuleBodies};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::{CaseDomain, PoleKind, PoleLocus, TargetMap, parse_target};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::poly::Poly;
use rustred_campaign_core::symmetry::GroupElement;
use serde::{Deserialize, Serialize};

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    pub fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            v.swap(i, j);
        }
    }
}

/// Linear form sum c_i n_i + c.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lin {
    pub t: Vec<(usize, i64)>,
    pub c: i64,
}
impl Lin {
    pub fn eval(&self, p: &[i32]) -> i64 {
        self.t.iter().map(|&(i, a)| a * p[i] as i64).sum::<i64>() + self.c
    }
    pub fn text(&self) -> String {
        let mut s = format!("{}", self.c);
        for &(i, a) in &self.t {
            if a >= 0 {
                s += &format!("+{a}*n{i}");
            } else {
                s += &format!("-{}*n{i}", -a);
            }
        }
        s
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Sl {
    P(usize, i32),
    C(i32),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuleSpec {
    pub label: i64,
    pub fixed: Vec<Option<i32>>,
    pub bounds: Vec<Lin>,
    pub eqs: Vec<Lin>,
    pub exc: Vec<Vec<Lin>>,
    pub poles: Vec<Lin>,
    pub elided: bool,
    pub target: Option<Vec<Sl>>,
    pub body: Option<Vec<Vec<Sl>>>,
    /// hidden truth: per term, percent chance that the coefficient is zero
    pub zp: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TableSpec {
    pub name: String,
    pub rules: Vec<RuleSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NodeSpec {
    pub n: usize,
    pub nl: usize,
    pub tables: Vec<TableSpec>,
    pub priority: Vec<usize>,
    pub ties: Option<Vec<usize>>,
    pub start: Vec<Vec<i32>>,
    pub group: Vec<(Vec<usize>, Option<Vec<usize>>)>,
    pub term_pref: Vec<Vec<i32>>,
    pub term_other: Vec<Vec<i32>>,
    pub fact_seed: u64,
}

pub fn slot_text(s: &Sl) -> String {
    match *s {
        Sl::P(a, 0) => format!("n{a}"),
        Sl::P(a, o) if o > 0 => format!("n{a}+{o}"),
        Sl::P(a, o) => format!("n{a}-{}", -o),
        Sl::C(c) => format!("{c}"),
    }
}
pub fn pat_text(v: &[Sl]) -> String {
    format!("I({})", v.iter().map(slot_text).collect::<Vec<_>>().join(","))
}

pub fn build_domain(sp: &NodeSpec, r: &RuleSpec) -> CaseDomain {
    let n = sp.n;
    let chart: Vec<bool> = (0..n).map(|s| s < sp.nl).collect();
    let mut d = CaseDomain::free(n, Some(chart));
    d.fixed = r.fixed.clone();
    let p = |l: &Lin| Poly::parse(&l.text(), n, false).unwrap();
    d.bounds = r.bounds.iter().map(p).collect();
    d.equations = r.eqs.iter().map(p).collect();
    d.exceptions = r.exc.iter().map(|b| b.iter().map(p).collect()).collect();
    d.poles = r.poles.iter().map(|l| PoleLocus::new(PoleKind::Denominator, &p(l))).collect();
    d.witness_elided = r.elided;
    if let Some(t) = &r.target {
        d.target = TargetMap::Pattern(parse_target(&pat_text(t), n).unwrap());
    }
    d
}

pub struct Built {
    pub disp: Dispatcher,
    pub bodies: RuleBodies,
    pub cmp: NodeComparator,
    pub start: KeySet,
}

pub fn build(sp: &NodeSpec) -> Result<Built, String> {
    let mut tabs = vec![];
    for t in &sp.tables {
        let mut tab = Table::new(t.name.clone(), sp.n);
        for r in &t.rules {
            tab.push(r.label, build_domain(sp, r)).map_err(|e| format!("push: {e:?}"))?;
        }
        tabs.push(tab);
    }
    let disp = Dispatcher::new(tabs, sp.priority.clone()).map_err(|e| format!("disp: {e:?}"))?;
    let mut bodies = RuleBodies::new(&disp);
    for (ti, t) in sp.tables.iter().enumerate() {
        for (ri, r) in t.rules.iter().enumerate() {
            if let Some(b) = &r.body {
                let pats: Vec<String> = b.iter().map(|x| pat_text(x)).collect();
                bodies.set(&disp, ti, ri, &pats).map_err(|e| format!("body: {e:?}"))?;
            }
        }
    }
    let cmp = NodeComparator::legacy(sp.n, sp.ties.clone()).map_err(|e| format!("{e:?}"))?;
    let start = keyset(sp.n, &sp.start);
    Ok(Built { disp, bodies, cmp, start })
}

pub fn keyset(n: usize, keys: &[Vec<i32>]) -> KeySet {
    KeySet::from_keys(KeyCodec::new(n, 6).unwrap(), keys.iter().map(|k| k.as_slice())).unwrap()
}

pub fn group_elems(sp: &NodeSpec) -> Vec<GroupElement> {
    sp.group.iter().map(|(l, m)| GroupElement { lines: l.clone(), numerators: m.clone() }).collect()
}

// ------------------------------------------------------------------------------------------------- generation

pub struct GenOpts {
    pub sparse: bool,
    pub asc_rate: u64,
    pub group: bool,
    pub terminals: bool,
}

fn gen_lin(rng: &mut Rng, n: usize, nvars: usize, crange: i64) -> Lin {
    let mut t = vec![];
    let mut used = vec![];
    for _ in 0..nvars {
        let i = rng.below(n as u64) as usize;
        if used.contains(&i) {
            continue;
        }
        used.push(i);
        let a = if rng.chance(50) { 1 } else { -1 };
        t.push((i, a));
    }
    Lin { t, c: rng.range(-crange, crange) }
}

pub fn gen_body(rng: &mut Rng, n: usize, nl: usize, asc: u64) -> (Vec<Vec<Sl>>, Vec<u8>) {
    let nt = rng.below(6) as usize + if rng.chance(90) { 1 } else { 0 };
    let mut terms = vec![];
    let mut zp = vec![];
    for _ in 0..nt {
        let mut t: Vec<Sl> = (0..n).map(|i| Sl::P(i, 0)).collect();
        let roll = if asc == 0 { rng.below(75) } else { rng.below(100) };
        let mut z = 10u8;
        let line = rng.below(nl as u64) as usize;
        let num = if n > nl { Some(nl + rng.below((n - nl) as u64) as usize) } else { None };
        if roll < asc {
            // ascend
            t[line] = Sl::P(line, 1);
            z = 85;
        } else if roll < asc + asc / 2 + 1 && rng.chance(50) {
            // self-loop (lands on the rule point)
            z = 85;
        } else if roll < 45 {
            t[line] = Sl::P(line, -1);
            if rng.chance(30) {
                if let Some(m) = num {
                    t[m] = Sl::P(m, -1);
                }
            }
        } else if roll < 60 {
            t[line] = if rng.chance(50) { Sl::C(0) } else { Sl::P(line, -(rng.range(1, 3) as i32)) };
            if rng.chance(20) {
                if let Some(m) = num {
                    t[m] = Sl::P(m, 1);
                }
            }
        } else if roll < 75 {
            if let Some(m) = num {
                t[m] = if rng.chance(70) { Sl::P(m, 1) } else { Sl::C(rng.range(0, 1) as i32) };
                t[line] = Sl::P(line, -(rng.range(0, 1) as i32));
            } else {
                t[line] = Sl::P(line, -1);
            }
            z = 85;
        } else if roll < 85 {
            // equal-rank style: move one unit between two lines
            let l2 = rng.below(nl as u64) as usize;
            t[line] = Sl::P(line, -1);
            t[l2] = Sl::P(l2, if l2 == line { -1 } else { 1 });
            z = 50;
        } else {
            for (i, s) in t.iter_mut().enumerate() {
                if rng.chance(40) {
                    *s = Sl::P(i, rng.range(-2, 2) as i32);
                }
                if rng.chance(4) {
                    *s = Sl::C(if i < nl { rng.range(0, 3) } else { rng.range(-2, 1) } as i32);
                }
            }
            z = 50;
        }
        // occasionally a permuted parameter
        if rng.chance(5) && n >= 2 {
            let a = rng.below(n as u64) as usize;
            let b = rng.below(n as u64) as usize;
            if let (Sl::P(_, oa), Sl::P(_, ob)) = (t[a], t[b]) {
                t[a] = Sl::P(b, oa);
                t[b] = Sl::P(a, ob);
            }
        }
        terms.push(t);
        zp.push(z);
    }
    (terms, zp)
}

pub fn gen_rule(rng: &mut Rng, n: usize, nl: usize, m: i32, asc: u64, label: i64, fixp: u64) -> RuleSpec {
    let mut fixed = vec![None; n];
    for (i, f) in fixed.iter_mut().enumerate() {
        if rng.chance(fixp) {
            *f = Some(if i < nl { rng.range(1, m as i64) as i32 } else { -(rng.range(0, m as i64) as i32) });
        }
    }
    let mut bounds = vec![];
    for i in 0..n {
        if i < nl {
            bounds.push(Lin { t: vec![(i, -1)], c: m as i64 });
        } else {
            bounds.push(Lin { t: vec![(i, 1)], c: m as i64 });
        }
    }
    let mut eqs = vec![];
    if rng.chance(15) {
        let nv = 1 + rng.below(2) as usize;
        eqs.push(gen_lin(rng, n, nv, 2));
    }
    let mut exc = vec![];
    if rng.chance(30) {
        for _ in 0..1 + rng.below(2) {
            let mut br = vec![];
            for _ in 0..1 + rng.below(2) {
                let i = rng.below(n as u64) as usize;
                let v = if i < nl { rng.range(1, m as i64) } else { -rng.range(0, 2) };
                br.push(Lin { t: vec![(i, 1)], c: -v });
            }
            exc.push(br);
        }
    }
    let mut poles = vec![];
    if rng.chance(5) {
        let i = rng.below(n as u64) as usize;
        let v = if i < nl { rng.range(1, m as i64) } else { -rng.range(0, 1) };
        poles.push(Lin { t: vec![(i, 1)], c: -v });
    }
    let elided = rng.chance(3);
    let target = if rng.chance(15) {
        let mut t: Vec<Sl> = (0..n).map(|i| Sl::P(i, 0)).collect();
        for (i, s) in t.iter_mut().enumerate() {
            if rng.chance(30) {
                *s = Sl::P(i, rng.range(-1, 1) as i32);
            }
        }
        if rng.chance(30) && n >= 2 {
            let a = rng.below(n as u64) as usize;
            let b = rng.below(n as u64) as usize;
            if let (Sl::P(_, oa), Sl::P(_, ob)) = (t[a], t[b]) {
                t[a] = Sl::P(b, oa);
                t[b] = Sl::P(a, ob);
            }
        }
        if rng.chance(20) {
            let i = rng.below(n as u64) as usize;
            t[i] = Sl::C(if i < nl { rng.range(1, 3) } else { -rng.range(0, 1) } as i32);
        }
        Some(t)
    } else {
        None
    };
    let (mut body, zp) = gen_body(rng, n, nl, asc);
    // constants equal to a fixed value at their slot (exercise the body's "fixed" handling)
    for t in body.iter_mut() {
        for (i, s) in t.iter_mut().enumerate() {
            if let Some(f) = fixed[i]
                && rng.chance(25)
            {
                *s = Sl::C(f);
            }
        }
    }
    let body = if rng.chance(2) { None } else { Some(body) };
    RuleSpec { label, fixed, bounds, eqs, exc, poles, elided, target, body, zp }
}

pub fn gen_node(rng: &mut Rng, o: &GenOpts) -> NodeSpec {
    let n = rng.range(3, 8) as usize;
    let nl = rng.range(1, (n as i64).min(5)) as usize;
    let cap = (150_000f64).powf(1.0 / n as f64).floor() as i64;
    let m = rng.range(3, 5).min(cap.max(2)) as i32;
    let nt = rng.range(1, 4) as usize;
    let total = match rng.below(100) {
        0..70 => rng.range(5, 30),
        70..95 => rng.range(30, 80),
        _ => rng.range(80, 200),
    } as usize;
    let mut tables = vec![];
    let mut label = 0i64;
    for t in 0..nt {
        let cnt = if t + 1 == nt { total - total / nt * (nt - 1) } else { total / nt };
        let mut rules = vec![];
        for _ in 0..cnt {
            label += 1 + rng.below(3) as i64;
            rules.push(gen_rule(rng, n, nl, m, o.asc_rate, label, if o.sparse { 45 } else { 18 }));
        }
        // a free catch-all at the end sometimes
        if !o.sparse && rng.chance(50) {
            label += 1;
            let mut r = gen_rule(rng, n, nl, m, o.asc_rate, label, 0);
            r.fixed = vec![None; n];
            r.eqs.clear();
            r.target = None;
            r.elided = false;
            r.poles.clear();
            rules.push(r);
        }
        tables.push(TableSpec { name: format!("T{t}"), rules });
    }
    let mut priority: Vec<usize> = (0..nt).collect();
    rng.shuffle(&mut priority);
    let ties = if rng.chance(30) {
        let mut p: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut p);
        Some(p)
    } else {
        None
    };
    // start set: a box (lines 1..=b, numerators -c..=0), capped
    let mut start = vec![];
    let mut hi = vec![0i32; n];
    let mut lo = vec![0i32; n];
    for i in 0..n {
        if i < nl {
            lo[i] = 1;
            hi[i] = rng.range(1, (m - 1) as i64) as i32;
        } else {
            lo[i] = -(rng.range(0, 1) as i32);
            hi[i] = 0;
        }
    }
    let mut k = lo.clone();
    loop {
        start.push(k.clone());
        let mut i = 0;
        while i < n {
            if k[i] < hi[i] {
                k[i] += 1;
                break;
            }
            k[i] = lo[i];
            i += 1;
        }
        if i == n || start.len() > 400 {
            break;
        }
    }
    if rng.chance(10) {
        // a few odd start keys: subsector, positive numerator
        let mut s = start[0].clone();
        s[0] = 0;
        start.push(s);
        if n > nl {
            let mut s = start[0].clone();
            s[nl] = 1;
            start.push(s);
        }
    }
    let mut group = vec![];
    if o.group {
        let g = rng.range(2, 12) as usize;
        for gi in 0..g {
            let mut l: Vec<usize> = (0..nl).collect();
            if gi > 0 {
                rng.shuffle(&mut l);
            }
            let nm = if n > nl && rng.chance(60) {
                let mut p: Vec<usize> = (0..n - nl).collect();
                rng.shuffle(&mut p);
                Some(p)
            } else {
                None
            };
            group.push((l, nm));
        }
    }
    let (mut term_pref, mut term_other) = (vec![], vec![]);
    if o.terminals {
        for _ in 0..rng.below(4) {
            let k: Vec<i32> =
                (0..n).map(|i| if i < nl { rng.range(1, m as i64) as i32 } else { -(rng.range(0, 1) as i32) }).collect();
            if rng.chance(50) { term_pref.push(k) } else { term_other.push(k) }
        }
    }
    NodeSpec { n, nl, tables, priority, ties, start, group, term_pref, term_other, fact_seed: rng.next() }
}
