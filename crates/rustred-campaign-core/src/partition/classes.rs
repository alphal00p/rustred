//! Per-slot states and the memoised recursion that labels every class of the top sector Covered / Uncovered /
//! Fragile (the exact port of the checker's `uenum_g.py`, with exact integer tests for every linear literal).

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use super::exact::{Range, int_roots, linear_feasible, sign_breaks};
use super::nf::NfRules;
use super::{LiteralTests, PartitionError, Progress};

/// Label of a listed class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub enum Label {
    /// No rule alive: uncovered except possibly on coupled case-equation loci.
    Uncovered,
    /// Covered except possibly on the zero sets of live exception branches.
    Fragile,
}

/// Result tree of the recursion (shared between memo hits). `None` = every point covered.
#[derive(Debug)]
pub enum Node {
    Leaf(Label),
    Split { slot: usize, children: Vec<(u16, Rc<Node>)> },
}

/// Status of a zero-set literal on a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Identically zero on the class.
    Z0,
    /// Nonzero at every integer point of the class.
    NZ,
    /// Zero somewhere (or unknown): carried as a coupled condition.
    V,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Eq,
    Ge,
    Bad,
}

#[derive(Clone, Debug)]
struct Cond {
    rule: u32,
    kind: Kind,
    lits: Vec<u32>,
    sup: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CondVal {
    Ok,
    Kill,
    Live,
}

type Bits = Vec<u64>;

fn bit(b: &Bits, i: usize) -> bool {
    (b[i >> 6] >> (i & 63)) & 1 == 1
}
fn set(b: &mut Bits, i: usize) {
    b[i >> 6] |= 1 << (i & 63);
}
fn clr(b: &mut Bits, i: usize) {
    b[i >> 6] &= !(1 << (i & 63));
}
fn and(a: &Bits, b: &Bits) -> Bits {
    a.iter().zip(b).map(|(x, y)| x & y).collect()
}
fn any_andnot(a: &Bits, b: &Bits) -> bool {
    a.iter().zip(b).any(|(x, y)| x & !y != 0)
}
fn any(a: &Bits) -> bool {
    a.iter().any(|&x| x != 0)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct MemoKey {
    k: u16,
    alive: Bits,
    frag: Bits,
    pend: Vec<(u16, u16)>,
    off: u16,
}

/// Counters of one recursion.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct EnumStats {
    pub nodes: u64,
    pub memo_hits: u64,
    pub memo_size: u64,
    pub pruned_off_floor: u64,
    pub unknown_literal_tests: u64,
    pub status_cache: u64,
    pub aborted: bool,
}

/// Per-slot states in the general form: index 0 = the floor singleton, then outward; the last is the half-infinite
/// tail. Critical values: every integer root in chart range of every one-slot zero-set literal, and the sign
/// boundary of every one-slot GE literal.
pub fn slot_states(nf: &NfRules, extra: Option<&[Vec<i64>]>) -> Result<(Vec<Vec<Range>>, u64), PartitionError> {
    let n = nf.arity;
    let mut crit: Vec<Vec<i64>> = (0..n).map(|i| vec![nf.floor(i)]).collect();
    let mut unknown = 0u64;
    for l in &nf.lits {
        if l.support.len() != 1 {
            continue;
        }
        let s = l.support[0];
        let u = l.poly.univariate(s).expect("one-slot literal");
        let vals = if l.is_ge {
            match sign_breaks(&u) {
                Ok(v) => {
                    // keep the boundary values: for a linear a x + b >= 0 the value where it starts / stops holding
                    if u.len() == 2 {
                        let (b, a) = (u[0], u[1]);
                        let c = if a > 0 {
                            super::exact::div_ceil(-b, a)
                        } else {
                            super::exact::div_floor(-b, a)
                        };
                        vec![c as i64]
                    } else {
                        v
                    }
                }
                Err(_) => {
                    unknown += 1;
                    vec![]
                }
            }
        } else {
            match int_roots(&u) {
                Ok(v) => v,
                Err(_) => {
                    unknown += 1;
                    vec![]
                }
            }
        };
        for x in vals {
            if (nf.lines[s] && x >= 1) || (!nf.lines[s] && x <= 0) {
                crit[s].push(x);
            }
        }
    }
    if let Some(ex) = extra {
        for (i, v) in ex.iter().enumerate().take(n) {
            for &x in v {
                if (nf.lines[i] && x >= 1) || (!nf.lines[i] && x <= 0) {
                    crit[i].push(x);
                }
            }
        }
    }
    let mut out = vec![];
    for (i, c) in crit.iter_mut().enumerate() {
        c.sort_unstable();
        c.dedup();
        if !nf.lines[i] {
            c.reverse();
        }
        let mut st = vec![];
        for (j, &x) in c.iter().enumerate() {
            st.push(Range::point(x));
            if j + 1 < c.len() && (c[j + 1] - x).abs() > 1 {
                st.push(if nf.lines[i] {
                    Range {
                        lo: Some(x + 1),
                        hi: Some(c[j + 1] - 1),
                    }
                } else {
                    Range {
                        lo: Some(c[j + 1] + 1),
                        hi: Some(x - 1),
                    }
                });
            }
        }
        let last = *c.last().expect("floor");
        st.push(if nf.lines[i] {
            Range {
                lo: Some(last + 1),
                hi: None,
            }
        } else {
            Range {
                lo: None,
                hi: Some(last - 1),
            }
        });
        out.push(st);
    }
    Ok((out, unknown))
}

/// The recursion engine.
pub struct Engine<'a> {
    pub nf: &'a NfRules,
    pub states: Vec<Vec<Range>>,
    tests: LiteralTests,
    words: usize,
    conds: Vec<Cond>,
    kill: Vec<Vec<Bits>>,
    live: Vec<Vec<Bits>>,
    compl_upto: Vec<Bits>,
    univ_suffix: Vec<Bits>,
    multi_at: Vec<Vec<usize>>,
    pend: Vec<Vec<(u32, u64)>>,
    cache: HashMap<(u32, Vec<u16>), Status>,
    ge_cache: HashMap<(u32, Vec<u16>), CondVal>,
    memo: HashMap<MemoKey, Option<Rc<Node>>>,
    pub use_memo: bool,
    pub cap: Option<usize>,
    pub stats: EnumStats,
    pub max_nodes: Option<u64>,
    pub deadline: Option<Instant>,
    t0: Instant,
}

impl<'a> Engine<'a> {
    pub fn new(nf: &'a NfRules, states: Vec<Vec<Range>>, tests: LiteralTests) -> Result<Self, PartitionError> {
        let n = nf.arity;
        if n > 64 {
            return Err(PartitionError::Unsupported("arity above 64".into()));
        }
        let nr = nf.rules.len();
        let words = nr.div_ceil(64).max(1);
        let mut conds = vec![];
        for (ri, r) in nf.rules.iter().enumerate() {
            let sup_of = |ls: &[u32]| {
                let mut s: Vec<usize> = ls.iter().flat_map(|&l| nf.lits[l as usize].support.clone()).collect();
                s.sort_unstable();
                s.dedup();
                s
            };
            for &l in &r.eq {
                conds.push(Cond {
                    rule: ri as u32,
                    kind: Kind::Eq,
                    lits: vec![l],
                    sup: sup_of(&[l]),
                });
            }
            for &l in &r.ge {
                conds.push(Cond {
                    rule: ri as u32,
                    kind: Kind::Ge,
                    lits: vec![l],
                    sup: sup_of(&[l]),
                });
            }
            for b in &r.bad {
                conds.push(Cond {
                    rule: ri as u32,
                    kind: Kind::Bad,
                    lits: b.clone(),
                    sup: sup_of(b),
                });
            }
        }
        let full: Bits = {
            let mut b = vec![u64::MAX; words];
            let extra = words * 64 - nr;
            if extra > 0 {
                b[words - 1] = u64::MAX >> extra;
            }
            if nr == 0 {
                b[0] = 0;
            }
            b
        };
        let mut eng = Engine {
            nf,
            states,
            tests,
            words,
            conds,
            kill: vec![],
            live: vec![],
            compl_upto: vec![],
            univ_suffix: vec![],
            multi_at: vec![vec![]; n + 1],
            pend: vec![vec![]; n + 1],
            cache: HashMap::new(),
            ge_cache: HashMap::new(),
            memo: HashMap::new(),
            use_memo: true,
            cap: None,
            stats: EnumStats::default(),
            max_nodes: None,
            deadline: None,
            t0: Instant::now(),
        };
        // single-slot masks
        let mut kill = vec![];
        let mut live = vec![];
        for i in 0..n {
            let ns = eng.states[i].len();
            kill.push(vec![vec![0u64; words]; ns]);
            live.push(vec![vec![0u64; words]; ns]);
        }
        let mut support = vec![0u64; nr];
        let mut multi_max: Vec<i64> = vec![-1; nr];
        for ci in 0..eng.conds.len() {
            let c = eng.conds[ci].clone();
            for &s in &c.sup {
                support[c.rule as usize] |= 1 << s;
            }
            if c.sup.len() == 1 {
                let s = c.sup[0];
                for v in 0..eng.states[s].len() {
                    let mut st = vec![0u16; n];
                    st[s] = v as u16;
                    match eng.cond_eval(&c, &st) {
                        CondVal::Kill => set(&mut kill[s][v], c.rule as usize),
                        CondVal::Live => set(&mut live[s][v], c.rule as usize),
                        CondVal::Ok => {}
                    }
                }
            } else if c.sup.len() > 1 {
                let mx = *c.sup.last().expect("nonempty");
                let mn = c.sup[0];
                eng.multi_at[mx + 1].push(ci);
                multi_max[c.rule as usize] = multi_max[c.rule as usize].max(mx as i64);
                let mut mask = 0u64;
                for kk in (mn + 1)..=(mx + 1) {
                    for &s in &c.sup {
                        if s < kk {
                            mask |= 1 << s;
                        }
                    }
                    eng.pend[kk].push((c.rule, mask));
                }
            }
        }
        let mut complete = vec![vec![0u64; words]; n + 1];
        for (ri, &sup) in support.iter().enumerate() {
            let k = if sup == 0 { 0 } else { 64 - sup.leading_zeros() as usize };
            set(&mut complete[k], ri);
        }
        let mut compl_upto = vec![];
        let mut acc = vec![0u64; words];
        for c in &complete {
            for (a, b) in acc.iter_mut().zip(c) {
                *a |= b;
            }
            compl_upto.push(acc.clone());
        }
        let mut univ_slot = vec![];
        for j in 0..n {
            let mut m = full.clone();
            for v in 0..eng.states[j].len() {
                for w in 0..words {
                    m[w] &= !kill[j][v][w] & !live[j][v][w];
                }
            }
            univ_slot.push(m);
        }
        let mut univ_suffix = vec![];
        for k in 0..=n {
            let mut m = full.clone();
            for us in univ_slot.iter().skip(k) {
                m = and(&m, us);
            }
            for (ri, &mm) in multi_max.iter().enumerate() {
                if mm >= k as i64 {
                    clr(&mut m, ri);
                }
            }
            univ_suffix.push(m);
        }
        eng.kill = kill;
        eng.live = live;
        eng.compl_upto = compl_upto;
        eng.univ_suffix = univ_suffix;
        Ok(eng)
    }

    pub fn full(&self) -> Bits {
        let nr = self.nf.rules.len();
        let mut b = vec![0u64; self.words];
        for i in 0..nr {
            set(&mut b, i);
        }
        b
    }

    /// Status of zero-set literal `l` given the states of its support slots (read from `st`).
    pub fn status(&mut self, l: u32, st: &[u16]) -> Status {
        let lit = &self.nf.lits[l as usize];
        let key: Vec<u16> = lit.support.iter().map(|&s| st[s]).collect();
        if let Some(&s) = self.cache.get(&(l, key.clone())) {
            return s;
        }
        let ranges: Vec<(usize, Range)> = lit.support.iter().map(|&s| (s, self.states[s][st[s] as usize])).collect();
        let (s, unknown) = literal_status(&lit.poly, &ranges, self.tests);
        if unknown {
            self.stats.unknown_literal_tests += 1;
        }
        self.cache.insert((l, key), s);
        self.stats.status_cache += 1;
        s
    }

    fn ge_val(&mut self, l: u32, st: &[u16]) -> CondVal {
        let lit = &self.nf.lits[l as usize];
        let key: Vec<u16> = lit.support.iter().map(|&s| st[s]).collect();
        if let Some(&s) = self.ge_cache.get(&(l, key.clone())) {
            return s;
        }
        let ranges: Vec<(usize, Range)> = lit.support.iter().map(|&s| (s, self.states[s][st[s] as usize])).collect();
        let v = match ge_status(&lit.poly, &ranges) {
            Some(true) => CondVal::Ok,
            _ => CondVal::Kill,
        };
        self.ge_cache.insert((l, key), v);
        v
    }

    fn cond_eval(&mut self, c: &Cond, st: &[u16]) -> CondVal {
        match c.kind {
            Kind::Eq => {
                if self.status(c.lits[0], st) == Status::Z0 {
                    CondVal::Ok
                } else {
                    CondVal::Kill
                }
            }
            Kind::Ge => self.ge_val(c.lits[0], st),
            Kind::Bad => {
                let mut all_z = true;
                let mut all_zv = true;
                for &l in &c.lits {
                    match self.status(l, st) {
                        Status::Z0 => {}
                        Status::V => all_z = false,
                        Status::NZ => {
                            all_z = false;
                            all_zv = false;
                            break;
                        }
                    }
                }
                if all_z {
                    CondVal::Kill
                } else if all_zv {
                    CondVal::Live
                } else {
                    CondVal::Ok
                }
            }
        }
    }

    /// Run the recursion from the root.
    pub fn run(&mut self, progress: Option<&dyn Fn(&Progress)>) -> Option<Rc<Node>> {
        self.t0 = Instant::now();
        let mut st = vec![0u16; self.nf.arity];
        let a = self.full();
        let f = vec![0u64; self.words];
        self.rec(0, &mut st, a, f, 0, progress)
    }

    fn rec(
        &mut self,
        k: usize,
        st: &mut Vec<u16>,
        mut a: Bits,
        mut fr: Bits,
        off: usize,
        progress: Option<&dyn Fn(&Progress)>,
    ) -> Option<Rc<Node>> {
        self.stats.nodes += 1;
        if self.stats.nodes % 4096 == 0 {
            if self.max_nodes.is_some_and(|m| self.stats.nodes > m)
                || self.deadline.is_some_and(|d| Instant::now() > d)
            {
                self.stats.aborted = true;
            }
            if self.stats.nodes % (1 << 18) == 0
                && let Some(p) = progress
            {
                p(&Progress {
                    nodes: self.stats.nodes,
                    memo_size: self.memo.len() as u64,
                    depth: k,
                    seconds_x1000: self.t0.elapsed().as_millis() as u64,
                    finished_subtrees: 0,
                    total_subtrees: 0,
                });
            }
        }
        if self.stats.aborted {
            return None;
        }
        if let Some(c) = self.cap
            && off > c
        {
            self.stats.pruned_off_floor += 1;
            return None;
        }
        for (w, x) in fr.iter_mut().enumerate() {
            *x &= a[w];
        }
        let key = if self.use_memo {
            let mut need = 0u64;
            for &(ri, mask) in &self.pend[k] {
                if bit(&a, ri as usize) {
                    need |= mask;
                }
            }
            let pend: Vec<(u16, u16)> = (0..k).filter(|&s| (need >> s) & 1 == 1).map(|s| (s as u16, st[s])).collect();
            let key = MemoKey {
                k: k as u16,
                alive: a.clone(),
                frag: fr.clone(),
                pend,
                off: if self.cap.is_some() { off as u16 } else { 0 },
            };
            if let Some(r) = self.memo.get(&key) {
                self.stats.memo_hits += 1;
                return r.clone();
            }
            Some(key)
        } else {
            None
        };
        let multi = std::mem::take(&mut self.multi_at[k]);
        for &ci in &multi {
            let ri = self.conds[ci].rule as usize;
            if !bit(&a, ri) {
                continue;
            }
            let c = self.conds[ci].clone();
            match self.cond_eval(&c, st) {
                CondVal::Kill => clr(&mut a, ri),
                CondVal::Live => set(&mut fr, ri),
                CondVal::Ok => {}
            }
        }
        self.multi_at[k] = multi;
        let cov = and(&a, &self.compl_upto[k]);
        let res = if any_andnot(&cov, &fr) || any_andnot(&and(&a, &self.univ_suffix[k]), &fr) {
            None
        } else if any(&cov) {
            Some(Rc::new(Node::Leaf(Label::Fragile)))
        } else if !any(&a) {
            Some(Rc::new(Node::Leaf(Label::Uncovered)))
        } else {
            let mut children = vec![];
            for v in 0..self.states[k].len() {
                st[k] = v as u16;
                let ca = and(&a, &self.kill_not(k, v));
                let mut cf = fr.clone();
                for w in 0..self.words {
                    cf[w] |= self.live[k][v][w] & a[w];
                }
                let r = self.rec(k + 1, st, ca, cf, off + usize::from(v != 0), progress);
                if let Some(r) = r {
                    children.push((v as u16, r));
                }
            }
            st[k] = 0;
            if children.is_empty() {
                None
            } else {
                Some(Rc::new(Node::Split { slot: k, children }))
            }
        };
        if let Some(key) = key
            && !self.stats.aborted
        {
            self.memo.insert(key, res.clone());
            self.stats.memo_size = self.memo.len() as u64;
        }
        res
    }

    fn kill_not(&self, k: usize, v: usize) -> Bits {
        self.kill[k][v].iter().map(|x| !x).collect()
    }
}

/// Status of a zero-set literal on a product of ranges (slots not listed are irrelevant). Returns (status, unknown).
pub fn literal_status(
    p: &super::exact::MPoly,
    ranges: &[(usize, Range)],
    tests: LiteralTests,
) -> (Status, bool) {
    let mut q = p.clone();
    let mut free: Vec<(usize, Range)> = vec![];
    for &(s, r) in ranges {
        if let Some(v) = r.singleton() {
            match q.subst(s, v as i128) {
                Ok(x) => q = x,
                Err(_) => return (Status::V, true),
            }
        } else {
            free.push((s, r));
        }
    }
    let vars = q.support();
    if vars.is_empty() {
        return (
            if q.as_constant() == Some(0) {
                Status::Z0
            } else {
                Status::NZ
            },
            false,
        );
    }
    let range_of = |s: usize| free.iter().find(|(x, _)| *x == s).map(|(_, r)| *r).expect("free slot");
    if vars.len() == 1 {
        let s = vars[0];
        let r = range_of(s);
        let u = q.univariate(s).expect("univariate");
        let roots = match int_roots(&u) {
            Ok(v) => v,
            Err(_) => return (Status::V, true),
        };
        let inside = roots.iter().filter(|&&x| r.contains(x)).count();
        if inside == 0 {
            return (Status::NZ, false);
        }
        if let (Some(lo), Some(hi)) = (r.lo, r.hi)
            && (hi - lo + 1) as usize == inside
        {
            return (Status::Z0, false);
        }
        return (Status::V, false);
    }
    if let Some((coef, c)) = q.linear_form() {
        let terms: Vec<(i128, Range)> = vars.iter().map(|&s| (coef[s], range_of(s))).collect();
        return match tests {
            LiteralTests::Exact => match linear_feasible(&terms, c) {
                Ok(true) => (Status::V, false),
                Ok(false) => (Status::NZ, false),
                Err(_) => (Status::V, true),
            },
            LiteralTests::ReferenceCompatible => (reference_lin_status(&terms, c), false),
        };
    }
    (Status::V, false)
}

/// The checker's `lin_status` (uenum_g.py): gcd test, interval range test, exact reachability only when every
/// variable is a tail state and the outward coefficients have one sign; otherwise V.
fn reference_lin_status(terms: &[(i128, Range)], c: i128) -> Status {
    let mut g = 0;
    for &(a, _) in terms {
        g = super::exact::gcd(g, a);
    }
    if g != 0 && c % g != 0 {
        return Status::NZ;
    }
    // range of c + sum a x over the box (None = infinite)
    let mut lo: Option<i128> = Some(c);
    let mut hi: Option<i128> = Some(c);
    for &(a, r) in terms {
        let (l, h) = (r.lo.map(|x| a * x as i128), r.hi.map(|x| a * x as i128));
        let (mn, mx) = if a >= 0 { (l, h) } else { (h, l) };
        lo = match (lo, mn) {
            (Some(x), Some(y)) => Some(x + y),
            _ => None,
        };
        hi = match (hi, mx) {
            (Some(x), Some(y)) => Some(x + y),
            _ => None,
        };
    }
    if lo.is_some_and(|x| x > 0) || hi.is_some_and(|x| x < 0) {
        return Status::NZ;
    }
    let tails = terms.iter().all(|(_, r)| r.lo.is_none() || r.hi.is_none());
    if tails {
        let mut cc = c;
        let mut cs = vec![];
        for &(a, r) in terms {
            match (r.lo, r.hi) {
                (Some(l), None) => {
                    cc += a * l as i128;
                    cs.push(a);
                }
                (None, Some(h)) => {
                    cc += a * h as i128;
                    cs.push(-a);
                }
                _ => return Status::V,
            }
        }
        if cs.iter().all(|&a| a > 0) || cs.iter().all(|&a| a < 0) {
            if cs[0] < 0 {
                for a in cs.iter_mut() {
                    *a = -*a;
                }
                cc = -cc;
            }
            let t = -cc;
            if t < 0 {
                return Status::NZ;
            }
            if t > 10_000_000 {
                return Status::V;
            }
            let t = t as usize;
            let mut reach = vec![false; t + 1];
            reach[0] = true;
            for x in 1..=t {
                reach[x] = cs.iter().any(|&a| x as i128 >= a && reach[x - a as usize]);
            }
            return if reach[t] { Status::V } else { Status::NZ };
        }
    }
    Status::V
}

/// Sign of a GE literal on a product of ranges: Some(true) = >= 0 everywhere, Some(false) = < 0 everywhere, None =
/// varies or unknown.
pub fn ge_status(p: &super::exact::MPoly, ranges: &[(usize, Range)]) -> Option<bool> {
    let mut q = p.clone();
    let mut free: Vec<(usize, Range)> = vec![];
    for &(s, r) in ranges {
        if let Some(v) = r.singleton() {
            q = q.subst(s, v as i128).ok()?;
        } else {
            free.push((s, r));
        }
    }
    if let Some(c) = q.as_constant() {
        return Some(c >= 0);
    }
    let (coef, c) = q.linear_form()?;
    let mut lo: Option<i128> = Some(c);
    let mut hi: Option<i128> = Some(c);
    for (s, r) in &free {
        let a = coef[*s];
        if a == 0 {
            continue;
        }
        let l = r.lo.map(|x| a.checked_mul(x as i128));
        let h = r.hi.map(|x| a.checked_mul(x as i128));
        let (mn, mx) = if a > 0 { (l, h) } else { (h, l) };
        lo = match (lo, mn) {
            (Some(x), Some(Some(y))) => x.checked_add(y),
            _ => None,
        };
        hi = match (hi, mx) {
            (Some(x), Some(Some(y))) => x.checked_add(y),
            _ => None,
        };
    }
    if lo.is_some_and(|x| x >= 0) {
        return Some(true);
    }
    if hi.is_some_and(|x| x < 0) {
        return Some(false);
    }
    None
}

/// A listed class: label and per slot a state index (`None` = any value of that slot).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Class {
    pub label: Label,
    pub states: Vec<Option<u16>>,
}

/// Every root-to-leaf path of the tree, as classes.
pub fn collect_classes(root: &Option<Rc<Node>>, arity: usize) -> Vec<Class> {
    let mut out = vec![];
    let mut prefix: Vec<Option<u16>> = vec![None; arity];
    fn walk(n: &Node, prefix: &mut Vec<Option<u16>>, out: &mut Vec<Class>) {
        match n {
            Node::Leaf(l) => out.push(Class {
                label: *l,
                states: prefix.clone(),
            }),
            Node::Split { slot, children } => {
                for (v, c) in children {
                    prefix[*slot] = Some(*v);
                    walk(c, prefix, out);
                }
                prefix[*slot] = None;
            }
        }
    }
    if let Some(r) = root {
        walk(r, &mut prefix, &mut out);
    }
    out
}

/// The class hit by a key (state indices per slot): `None` = in a covered class.
pub fn lookup<'n>(root: &'n Option<Rc<Node>>, idx: &[u16]) -> Option<&'n Node> {
    let mut n: &Node = root.as_deref()?;
    loop {
        match n {
            Node::Leaf(_) => return Some(n),
            Node::Split { slot, children } => {
                let v = idx[*slot];
                n = children.iter().find(|(x, _)| *x == v).map(|(_, c)| &**c)?;
            }
        }
    }
}
