//! Per-slot states and the memoised recursion that labels every class of the top sector Covered / Uncovered /
//! Fragile (the exact port of the checker's `uenum_g.py`, with exact integer tests for every linear literal).
//!
//! Overflow safety (re-audit RA2-C1 / M1 / m3): every coefficient operation is checked (`exact`'s primitives); an
//! overflow makes the affected test undecided (literal V, bound soft-killed), never a decided verdict, never a panic.
//! Clippy refuses new unchecked operators or truncating casts in this module (local allows state why).
//!
//! Undecided bounds (re-audit RA2-M2): a bound whose sign is not constant on a class (or cannot be decided) does not
//! label the class Uncovered: the rule is SOFT-killed, and a class left with no alive rule but a soft-killed one is
//! Fragile, so the locus step splits it exactly on the bound. A one-slot bound of any degree is decided exactly on
//! each state ([`super::exact::univ_ge_on`]).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use super::exact::{
    Range, ceil_neg_div, floor_neg_div, int_roots, linear_feasible, sign_breaks, univ_ge_on,
};
use super::nf::NfRules;
use super::{LiteralTests, PartitionError, Progress, WorkMeter};

/// Label of a listed class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub enum Label {
    /// No rule alive: uncovered except possibly on coupled case-equation loci.
    Uncovered,
    /// Covered except possibly on the zero sets of live exception branches.
    Fragile,
    /// Pruned (`max_off_floor`) or not explored (budget abort): NOTHING is known about the class (audit-part-A M2).
    Unexplored,
}

/// Result tree of the recursion (shared between memo hits). `None` = every point covered.
#[derive(Debug)]
pub enum Node {
    Leaf(Label),
    Split {
        slot: usize,
        children: Vec<(u16, Rc<Node>)>,
    },
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
    /// The rule does not cover the WHOLE class, but may cover part of it (a bound that varies on the class or cannot
    /// be decided): removed from the alive set, and the class is Fragile if nothing else is alive (re-audit RA2-M2).
    Soft,
}

type Bits = Vec<u64>;

// Bit-index arithmetic: `i >> 6` and `1 << (i & 63)` never overflow (shift amount < 64).
#[allow(clippy::arithmetic_side_effects)]
fn bit(b: &Bits, i: usize) -> bool {
    (b[i >> 6] >> (i & 63)) & 1 == 1
}
#[allow(clippy::arithmetic_side_effects)]
fn set(b: &mut Bits, i: usize) {
    b[i >> 6] |= 1 << (i & 63);
}
#[allow(clippy::arithmetic_side_effects)]
fn clr(b: &mut Bits, i: usize) {
    b[i >> 6] &= !(1 << (i & 63));
}
#[allow(clippy::arithmetic_side_effects)]
fn slot_bit(s: usize) -> u64 {
    1u64 << (s & 63)
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
    /// A rule was soft-killed on the path (the leaf with no alive rule is then Fragile, not Uncovered).
    soft: bool,
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
    pub memo_clears: u64,
    /// Classes labelled Fragile only because a bound was undecided on them (re-audit RA2-M2; resolved by the loci).
    pub soft_fragile: u64,
    pub aborted: bool,
}

/// Per-slot states in the general form: index 0 = the floor singleton, then outward; the last is the half-infinite
/// tail. Critical values: every integer root in chart range of every one-slot zero-set literal, and the sign
/// boundary of every one-slot GE literal; values outside the `i32` key range are not used (the state tests stay exact
/// on coarser states). Returns the states and the number of literals whose critical values could not be computed
/// (overflow / work cap: their states are coarser, their tests stay exact). At most `u16::MAX - 1` states per slot
/// (state indices are `u16`, `u16::MAX` = [`super::ANY_OFF`]): more is a typed refusal, never a truncation.
pub fn slot_states(
    nf: &NfRules,
    extra: Option<&[Vec<i64>]>,
) -> Result<(Vec<Vec<Range>>, u64), PartitionError> {
    let n = nf.arity;
    let mut crit: Vec<Vec<i64>> = (0..n).map(|i| vec![nf.floor(i)]).collect();
    let mut unknown = 0u64;
    let in_chart = |s: usize, x: i64| (nf.lines[s] && x >= 1) || (!nf.lines[s] && x <= 0);
    let key_range = |x: i128| -> Option<i64> {
        i32::try_from(x).ok().map(i64::from) // keys are i32: x +- 1 below never overflows i64
    };
    for l in &nf.lits {
        if l.support.len() != 1 {
            continue;
        }
        let s = l.support[0];
        let Some(u) = l.poly.univariate(s) else {
            unknown = unknown.saturating_add(1);
            continue;
        };
        let vals: Result<Vec<i128>, _> = if l.is_ge {
            if let [b, a] = *u.as_slice() {
                // a x + b >= 0: the value where it starts (a > 0) / stops (a < 0) holding, checked
                let c = if a > 0 {
                    ceil_neg_div(b, a)
                } else {
                    floor_neg_div(b, a)
                };
                c.map(|c| vec![c])
            } else {
                sign_breaks(&u)
            }
        } else {
            int_roots(&u)
        };
        match vals {
            Ok(v) => {
                for x in v {
                    match key_range(x) {
                        Some(x) if in_chart(s, x) => crit[s].push(x),
                        Some(_) => {}
                        None => {} // beyond the i32 key range: not a boundary (coarser, still exact)
                    }
                }
            }
            Err(_) => unknown = unknown.saturating_add(1),
        }
    }
    if let Some(ex) = extra {
        for (i, v) in ex.iter().enumerate().take(n) {
            for &x in v {
                if i32::try_from(x).is_ok() && in_chart(i, x) {
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
        // every value is within i32, so +- 1 never overflows i64
        let step = |x: i64, d: i64| {
            x.checked_add(d)
                .ok_or(PartitionError::Exact(super::ExactError::Overflow))
        };
        let mut st = vec![];
        for (j, &x) in c.iter().enumerate() {
            st.push(Range::point(x));
            if let Some(&y) = c.get(j.saturating_add(1))
                && y.abs_diff(x) > 1
            {
                st.push(if nf.lines[i] {
                    Range {
                        lo: Some(step(x, 1)?),
                        hi: Some(step(y, -1)?),
                    }
                } else {
                    Range {
                        lo: Some(step(y, 1)?),
                        hi: Some(step(x, -1)?),
                    }
                });
            }
        }
        let last = *c
            .last()
            .ok_or(PartitionError::Unsupported("no floor value".into()))?;
        st.push(if nf.lines[i] {
            Range {
                lo: Some(step(last, 1)?),
                hi: None,
            }
        } else {
            Range {
                lo: None,
                hi: Some(step(last, -1)?),
            }
        });
        if st.len() >= usize::from(u16::MAX) {
            return Err(PartitionError::Unsupported(format!(
                "slot {i}: {} states (state indices are u16)",
                st.len()
            )));
        }
        out.push(st);
    }
    Ok((out, unknown))
}

/// The recursion engine.
pub struct Engine<'a> {
    pub nf: &'a NfRules,
    pub states: Vec<Vec<Range>>,
    /// Number of states per slot (validated `< u16::MAX` by [`slot_states`] / [`Engine::new`]).
    nstates: Vec<u16>,
    tests: LiteralTests,
    words: usize,
    conds: Vec<Cond>,
    kill: Vec<Vec<Bits>>,
    live: Vec<Vec<Bits>>,
    soft: Vec<Vec<Bits>>,
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
    /// The ONE work budget of the whole partition call (re-audit RA2-M5): every recursion node is one unit, checked
    /// at EVERY node (wall clock every [`WorkMeter::WALL_EVERY`] units); shared with the blocked sub-partitions and
    /// the locus resolution. Default: unlimited.
    pub meter: Rc<WorkMeter>,
    /// Memo entries kept at most (the memo is cleared beyond: results stay exact, only work is repeated).
    pub max_memo: Option<usize>,
    t0: Instant,
    /// The shared `Unexplored` leaf.
    unexplored: Rc<Node>,
}

fn idx_u32(i: usize) -> Result<u32, PartitionError> {
    u32::try_from(i).map_err(|_| PartitionError::Unsupported(format!("index {i} beyond u32")))
}

impl<'a> Engine<'a> {
    pub fn new(
        nf: &'a NfRules,
        states: Vec<Vec<Range>>,
        tests: LiteralTests,
    ) -> Result<Self, PartitionError> {
        let n = nf.arity;
        if n > 64 {
            return Err(PartitionError::Unsupported("arity above 64".into()));
        }
        if states.len() != n {
            return Err(PartitionError::Arity {
                expected: n,
                got: states.len(),
            });
        }
        let nstates: Vec<u16> = states
            .iter()
            .map(|st| {
                u16::try_from(st.len())
                    .ok()
                    .filter(|&m| m < u16::MAX && m > 0)
                    .ok_or_else(|| PartitionError::Unsupported("state count per slot".into()))
            })
            .collect::<Result<_, _>>()?;
        let nr = nf.rules.len();
        let words = nr.div_ceil(64).max(1);
        let mut conds = vec![];
        for (ri, r) in nf.rules.iter().enumerate() {
            let rule = idx_u32(ri)?;
            let sup_of = |ls: &[u32]| {
                let mut s: Vec<usize> = ls
                    .iter()
                    .flat_map(|&l| nf.lits[l as usize].support.clone())
                    .collect();
                s.sort_unstable();
                s.dedup();
                s
            };
            for &l in &r.eq {
                conds.push(Cond {
                    rule,
                    kind: Kind::Eq,
                    lits: vec![l],
                    sup: sup_of(&[l]),
                });
            }
            for &l in &r.ge {
                conds.push(Cond {
                    rule,
                    kind: Kind::Ge,
                    lits: vec![l],
                    sup: sup_of(&[l]),
                });
            }
            for b in &r.bad {
                conds.push(Cond {
                    rule,
                    kind: Kind::Bad,
                    lits: b.clone(),
                    sup: sup_of(b),
                });
            }
        }
        let full: Bits = {
            let mut b = vec![u64::MAX; words];
            let extra = words.saturating_mul(64).saturating_sub(nr);
            if extra > 0
                && let Some(last) = b.last_mut()
            {
                *last = u64::MAX
                    .checked_shr(u32::try_from(extra).unwrap_or(64))
                    .unwrap_or(0);
            }
            if nr == 0 {
                b[0] = 0;
            }
            b
        };
        let n1 = n.saturating_add(1);
        let mut eng = Engine {
            nf,
            states,
            nstates,
            tests,
            words,
            conds,
            kill: vec![],
            live: vec![],
            soft: vec![],
            compl_upto: vec![],
            univ_suffix: vec![],
            multi_at: vec![vec![]; n1],
            pend: vec![vec![]; n1],
            cache: HashMap::new(),
            ge_cache: HashMap::new(),
            memo: HashMap::new(),
            use_memo: true,
            cap: None,
            stats: EnumStats::default(),
            meter: Rc::new(WorkMeter::unlimited()),
            max_memo: None,
            t0: Instant::now(),
            unexplored: Rc::new(Node::Leaf(Label::Unexplored)),
        };
        // single-slot masks
        let mut kill = vec![];
        let mut live = vec![];
        let mut soft = vec![];
        for i in 0..n {
            let ns = eng.states[i].len();
            kill.push(vec![vec![0u64; words]; ns]);
            live.push(vec![vec![0u64; words]; ns]);
            soft.push(vec![vec![0u64; words]; ns]);
        }
        let mut support = vec![0u64; nr];
        let mut multi_max: Vec<Option<usize>> = vec![None; nr];
        for ci in 0..eng.conds.len() {
            let c = eng.conds[ci].clone();
            let rule = c.rule as usize;
            for &s in &c.sup {
                support[rule] |= slot_bit(s);
            }
            if c.sup.len() == 1 {
                let s = c.sup[0];
                for v in 0..eng.nstates[s] {
                    let mut st = vec![0u16; n];
                    st[s] = v;
                    let vi = usize::from(v);
                    match eng.cond_eval(&c, &st) {
                        CondVal::Kill => set(&mut kill[s][vi], rule),
                        CondVal::Live => set(&mut live[s][vi], rule),
                        CondVal::Soft => set(&mut soft[s][vi], rule),
                        CondVal::Ok => {}
                    }
                }
            } else if let (Some(&mn), Some(&mx)) = (c.sup.first(), c.sup.last())
                && c.sup.len() > 1
            {
                let mx1 = mx.saturating_add(1); // mx < n <= 64
                eng.multi_at[mx1].push(ci);
                multi_max[rule] = Some(multi_max[rule].map_or(mx, |m| m.max(mx)));
                let mut mask = 0u64;
                for kk in mn.saturating_add(1)..=mx1 {
                    for &s in &c.sup {
                        if s < kk {
                            mask |= slot_bit(s);
                        }
                    }
                    eng.pend[kk].push((c.rule, mask));
                }
            }
        }
        let mut complete = vec![vec![0u64; words]; n1];
        for (ri, &sup) in support.iter().enumerate() {
            // the slot after the last slot the rule depends on (0 = no slot)
            let k = usize::try_from(u64::BITS.saturating_sub(sup.leading_zeros())).unwrap_or(n);
            set(&mut complete[k.min(n)], ri);
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
                    m[w] &= !kill[j][v][w] & !live[j][v][w] & !soft[j][v][w];
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
                if mm.is_some_and(|m| m >= k) {
                    clr(&mut m, ri);
                }
            }
            univ_suffix.push(m);
        }
        eng.kill = kill;
        eng.live = live;
        eng.soft = soft;
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
        let ranges: Vec<(usize, Range)> = lit
            .support
            .iter()
            .map(|&s| (s, self.states[s][usize::from(st[s])]))
            .collect();
        let (s, unknown) = literal_status(&lit.poly, &ranges, self.tests);
        if unknown {
            self.stats.unknown_literal_tests = self.stats.unknown_literal_tests.saturating_add(1);
        }
        self.cache.insert((l, key), s);
        self.stats.status_cache = self.stats.status_cache.saturating_add(1);
        s
    }

    fn ge_val(&mut self, l: u32, st: &[u16]) -> CondVal {
        let lit = &self.nf.lits[l as usize];
        let key: Vec<u16> = lit.support.iter().map(|&s| st[s]).collect();
        if let Some(&s) = self.ge_cache.get(&(l, key.clone())) {
            return s;
        }
        let ranges: Vec<(usize, Range)> = lit
            .support
            .iter()
            .map(|&s| (s, self.states[s][usize::from(st[s])]))
            .collect();
        let v = match ge_status(&lit.poly, &ranges) {
            Some(true) => CondVal::Ok,
            Some(false) => CondVal::Kill,
            // varies on the class or undecided: the rule may cover PART of the class (re-audit RA2-M2)
            None => CondVal::Soft,
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
        self.rec(0, &mut st, a, f, 0, false, progress)
    }

    #[allow(clippy::too_many_arguments)]
    fn rec(
        &mut self,
        k: usize,
        st: &mut Vec<u16>,
        mut a: Bits,
        mut fr: Bits,
        off: usize,
        mut soft: bool,
        progress: Option<&dyn Fn(&Progress)>,
    ) -> Option<Rc<Node>> {
        if self.stats.aborted {
            return Some(self.unexplored.clone()); // not counted: no work after the abort
        }
        self.stats.nodes = self.stats.nodes.saturating_add(1);
        // the budget is checked at EVERY node (re-audit RA2-m5; the wall clock every WorkMeter::WALL_EVERY units)
        if !self.meter.tick(1) {
            self.stats.aborted = true;
        }
        if self.stats.nodes.is_multiple_of(1 << 18)
            && let Some(p) = progress
        {
            p(&Progress {
                nodes: self.stats.nodes,
                memo_size: u64::try_from(self.memo.len()).unwrap_or(u64::MAX),
                depth: k,
                seconds_x1000: u64::try_from(self.t0.elapsed().as_millis()).unwrap_or(u64::MAX),
                finished_subtrees: 0,
                total_subtrees: 0,
            });
        }
        if self.stats.aborted {
            return Some(self.unexplored.clone());
        }
        if let Some(c) = self.cap
            && off > c
        {
            self.stats.pruned_off_floor = self.stats.pruned_off_floor.saturating_add(1);
            return Some(self.unexplored.clone());
        }
        for (w, x) in fr.iter_mut().enumerate() {
            *x &= a[w];
        }
        let small = |x: usize| u16::try_from(x).unwrap_or(u16::MAX); // depth, slot, off-floor count: <= 64
        let key = if self.use_memo {
            let mut need = 0u64;
            for &(ri, mask) in &self.pend[k] {
                if bit(&a, ri as usize) {
                    need |= mask;
                }
            }
            let pend: Vec<(u16, u16)> = (0..k)
                .filter(|&s| need & slot_bit(s) != 0)
                .map(|s| (small(s), st[s]))
                .collect();
            let key = MemoKey {
                k: small(k),
                alive: a.clone(),
                frag: fr.clone(),
                pend,
                off: if self.cap.is_some() { small(off) } else { 0 },
                soft,
            };
            if let Some(r) = self.memo.get(&key) {
                self.stats.memo_hits = self.stats.memo_hits.saturating_add(1);
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
                CondVal::Soft => {
                    clr(&mut a, ri);
                    soft = true;
                }
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
            if soft {
                // no rule covers the whole class, but a soft-killed rule may cover part of it: exact split by loci
                self.stats.soft_fragile = self.stats.soft_fragile.saturating_add(1);
                Some(Rc::new(Node::Leaf(Label::Fragile)))
            } else {
                Some(Rc::new(Node::Leaf(Label::Uncovered)))
            }
        } else {
            let mut children = vec![];
            let k1 = k.saturating_add(1); // k < arity here
            for v in 0..self.nstates[k] {
                st[k] = v;
                let vi = usize::from(v);
                let mut ca = a.clone();
                let mut child_soft = soft;
                for ((cw, &kw), &sw) in ca.iter_mut().zip(&self.kill[k][vi]).zip(&self.soft[k][vi])
                {
                    if sw & *cw != 0 {
                        child_soft = true;
                    }
                    *cw &= !kw & !sw;
                }
                let mut cf = fr.clone();
                for w in 0..self.words {
                    cf[w] |= self.live[k][vi][w] & a[w];
                }
                let r = self.rec(
                    k1,
                    st,
                    ca,
                    cf,
                    off.saturating_add(usize::from(v != 0)),
                    child_soft,
                    progress,
                );
                if let Some(r) = r {
                    children.push((v, r));
                }
            }
            st[k] = 0;
            if children.is_empty() {
                None
            } else if children
                .iter()
                .all(|(_, c)| Rc::ptr_eq(c, &self.unexplored))
            {
                // no U / F below, only covered and unexplored states: the node is one Unexplored class
                // (conservative: its covered keys read Unexplored too; pruned subtrees are not materialised)
                Some(self.unexplored.clone())
            } else {
                Some(Rc::new(Node::Split { slot: k, children }))
            }
        };
        if let Some(key) = key
            && !self.stats.aborted
        {
            if self.max_memo.is_some_and(|m| self.memo.len() >= m) {
                self.memo.clear();
                self.stats.memo_clears = self.stats.memo_clears.saturating_add(1);
            }
            self.memo.insert(key, res.clone());
            self.stats.memo_size = u64::try_from(self.memo.len()).unwrap_or(u64::MAX);
        }
        res
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
            match q.subst(s, i128::from(v)) {
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
    let range_of = |s: usize| free.iter().find(|(x, _)| *x == s).map(|(_, r)| *r);
    if let [s] = *vars.as_slice() {
        let (Some(r), Some(u)) = (range_of(s), q.univariate(s)) else {
            return (Status::V, true);
        };
        let roots = match int_roots(&u) {
            Ok(v) => v,
            Err(_) => return (Status::V, true),
        };
        let inside = roots.iter().filter(|&&x| r.contains_wide(x)).count();
        if inside == 0 {
            return (Status::NZ, false);
        }
        if r.size()
            .is_some_and(|sz| u128::try_from(inside).is_ok_and(|i| i == sz))
        {
            return (Status::Z0, false);
        }
        return (Status::V, false);
    }
    if let Some((coef, c)) = q.linear_form() {
        let mut terms: Vec<(i128, Range)> = vec![];
        for &s in &vars {
            let Some(r) = range_of(s) else {
                return (Status::V, true);
            };
            terms.push((coef[s], r));
        }
        return match tests {
            LiteralTests::Exact => match linear_feasible(&terms, c) {
                Ok(true) => (Status::V, false),
                Ok(false) => (Status::NZ, false),
                Err(_) => (Status::V, true),
            },
            LiteralTests::ReferenceCompatible => match reference_lin_status(&terms, c) {
                Some(st) => (st, false),
                None => (Status::V, true), // overflow: conservative, counted
            },
        };
    }
    (Status::V, false)
}

/// The checker's `lin_status` (uenum_g.py): gcd test, interval range test, exact reachability only when every
/// variable is a tail state and the outward coefficients have one sign; otherwise V. Checked arithmetic: `None` on
/// an i128 overflow (the caller then answers V and counts the test as unknown; audit-part-A m3).
fn reference_lin_status(terms: &[(i128, Range)], c: i128) -> Option<Status> {
    let g = super::exact::gcd_all(terms.iter().map(|t| t.0)).ok()?;
    if g != 0 && c.checked_rem(g)? != 0 {
        return Some(Status::NZ);
    }
    // range of c + sum a x over the box (None = infinite)
    let mut lo: Option<i128> = Some(c);
    let mut hi: Option<i128> = Some(c);
    for &(a, r) in terms {
        let l = match r.lo {
            Some(x) => Some(a.checked_mul(i128::from(x))?),
            None => None,
        };
        let h = match r.hi {
            Some(x) => Some(a.checked_mul(i128::from(x))?),
            None => None,
        };
        let (mn, mx) = if a >= 0 { (l, h) } else { (h, l) };
        lo = match (lo, mn) {
            (Some(x), Some(y)) => Some(x.checked_add(y)?),
            _ => None,
        };
        hi = match (hi, mx) {
            (Some(x), Some(y)) => Some(x.checked_add(y)?),
            _ => None,
        };
    }
    if lo.is_some_and(|x| x > 0) || hi.is_some_and(|x| x < 0) {
        return Some(Status::NZ);
    }
    let tails = terms.iter().all(|(_, r)| r.lo.is_none() || r.hi.is_none());
    if tails {
        let mut cc = c;
        let mut cs = vec![];
        for &(a, r) in terms {
            match (r.lo, r.hi) {
                (Some(l), None) => {
                    cc = cc.checked_add(a.checked_mul(i128::from(l))?)?;
                    cs.push(a);
                }
                (None, Some(h)) => {
                    cc = cc.checked_add(a.checked_mul(i128::from(h))?)?;
                    cs.push(a.checked_neg()?);
                }
                _ => return Some(Status::V),
            }
        }
        if cs.iter().all(|&a| a > 0) || cs.iter().all(|&a| a < 0) {
            if cs.first().is_some_and(|&a| a < 0) {
                for a in cs.iter_mut() {
                    *a = a.checked_neg()?;
                }
                cc = cc.checked_neg()?;
            }
            let t = cc.checked_neg()?;
            if t < 0 {
                return Some(Status::NZ);
            }
            if t > 10_000_000 {
                return Some(Status::V);
            }
            let t = usize::try_from(t).ok()?;
            let mut reach = vec![false; t.checked_add(1)?];
            reach[0] = true;
            for x in 1..=t {
                let xi = i128::try_from(x).ok()?;
                reach[x] = cs.iter().any(|&a| {
                    xi.checked_sub(a)
                        .and_then(|d| usize::try_from(d).ok())
                        .is_some_and(|d| reach[d])
                });
            }
            return Some(if reach[t] { Status::V } else { Status::NZ });
        }
    }
    Some(Status::V)
}

/// The largest number of integers a one-slot NONLINEAR bound is evaluated at to decide its sign on a state
/// ([`univ_ge_on`]): beyond, the bound is undecided (soft) and the locus step splits it.
const UNIV_GE_CAP: u128 = 400_000;

/// Sign of a GE literal on a product of ranges: Some(true) = >= 0 everywhere, Some(false) = < 0 everywhere, None =
/// varies or unknown. One free slot (any degree): exact ([`univ_ge_on`]); several: exact interval bound of a linear
/// form, else None.
pub fn ge_status(p: &super::exact::MPoly, ranges: &[(usize, Range)]) -> Option<bool> {
    let mut q = p.clone();
    let mut free: Vec<(usize, Range)> = vec![];
    for &(s, r) in ranges {
        if let Some(v) = r.singleton() {
            q = q.subst(s, i128::from(v)).ok()?;
        } else {
            free.push((s, r));
        }
    }
    if let Some(c) = q.as_constant() {
        return Some(c >= 0);
    }
    let vars = q.support();
    if let [s] = *vars.as_slice()
        && q.total_degree() > 1
    {
        // one slot, nonlinear (re-audit RA2-M2): decided exactly on the state
        let r = free.iter().find(|(x, _)| *x == s)?.1;
        let u = q.univariate(s)?;
        return univ_ge_on(&u, r.lo.map(i128::from), r.hi.map(i128::from), UNIV_GE_CAP);
    }
    let (coef, c) = q.linear_form()?;
    let mut lo: Option<i128> = Some(c);
    let mut hi: Option<i128> = Some(c);
    for (s, r) in &free {
        let a = coef[*s];
        if a == 0 {
            continue;
        }
        let l = r.lo.map(|x| a.checked_mul(i128::from(x)));
        let h = r.hi.map(|x| a.checked_mul(i128::from(x)));
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

/// Every root-to-leaf path of the tree, as classes (unlimited work; see [`collect_classes_with`]).
pub fn collect_classes(root: &Option<Rc<Node>>, arity: usize) -> Vec<Class> {
    collect_classes_with(root, arity, &WorkMeter::unlimited())
}

/// [`collect_classes`] charged to `meter`, one unit per visited tree node (audit-part3 M2: the memo makes the tree a
/// DAG whose PATHS can be exponentially many). When the meter is exhausted the listing stops; the caller's run is
/// then aborted (the meter is sticky and every outcome reads it), so a truncated list never supports a verdict.
pub fn collect_classes_with(
    root: &Option<Rc<Node>>,
    arity: usize,
    meter: &WorkMeter,
) -> Vec<Class> {
    let mut out = vec![];
    let mut prefix: Vec<Option<u16>> = vec![None; arity];
    fn walk(n: &Node, prefix: &mut Vec<Option<u16>>, out: &mut Vec<Class>, meter: &WorkMeter) {
        if !meter.tick(1) {
            return;
        }
        match n {
            Node::Leaf(l) => out.push(Class {
                label: *l,
                states: prefix.clone(),
            }),
            Node::Split { slot, children } => {
                for (v, c) in children {
                    if meter.exhausted() {
                        break;
                    }
                    prefix[*slot] = Some(*v);
                    walk(c, prefix, out, meter);
                }
                prefix[*slot] = None;
            }
        }
    }
    if let Some(r) = root {
        walk(r, &mut prefix, &mut out, meter);
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
