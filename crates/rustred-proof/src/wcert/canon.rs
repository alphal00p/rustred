//! Binding MODULO zero sectors and the symmetry group (check (c) when the plain comparison fails).
//!
//! The engine's canonical form of a lifted row drops integrals of ZERO SECTORS (scaleless) and maps integrals onto
//! representatives under the node's SYMMETRY GROUP (I(a) = s I(g a)). Both are family properties carried by the
//! generating run's table (`zero_sectors` record: sector masks; `symmetry_group` record: per element a slot action, as
//! read by the instance-span mode, `span::parse_group`), each with its own proof elsewhere; this module takes them as
//! STATED inputs (their sha256 goes into the record) and decides
//!     sum over the row (stored frame) of c_I I  ==  stored relation   modulo  { I = 0 : I in a zero sector } and
//!                                                                             { I(a) = s I(g a) : g exact on a }.
//! Normal form: an integral pattern is dropped when its sector is a listed zero sector for EVERY key of the domain (a
//! symbolic slot's sign must be decided by the domain box), else replaced by the minimum of its connected class under the
//! exact actions (BFS closure; an action applies to a pattern when every slot whose power may be nonzero has a target, and
//! it fixes every symbolic slot without a flip: the images stay patterns of the same variables). Two relations are equal
//! modulo these identities when their class sums agree (a class reached with both signs is zero).
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use super::Vars;
use super::alg::Bound;
use crate::core::{J, Pat, TABLE_KINDS, jparse, read_lines_matching, record_kind};

pub struct Canon {
    pub zero: HashSet<Vec<u8>>,
    pub actions: Vec<Vec<Option<(usize, bool)>>>,
    /// per slot: bounds of the stored-frame variable (symbolic slots), from the domain box
    pub bounds: Vec<Bound>,
    pub zero_source: Option<(String, String)>,
    pub group_source: Option<(String, String, usize)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Class {
    /// in a zero sector at every key of the domain
    Zero,
    /// representative and sign (true: I(p) = - I(rep))
    Rep(Pat, bool),
}

/// Read the first `zero_sectors` record and the `symmetry_group` record of `chart` from a file (a table or a side file;
/// reading stops at the first rule record). Returns (masks + sha256 of the record line), (actions + sha + order).
pub fn read_records(
    path: &str,
    chart: &str,
) -> (
    Option<(HashSet<Vec<u8>>, String)>,
    Option<(Vec<Vec<Option<(usize, bool)>>>, String, usize)>,
) {
    let (mut zero, mut group) = (None, None);
    let mut ln = 0usize;
    read_lines_matching(path, |l| {
        ln += 1;
        let kind = record_kind(l, path, ln, TABLE_KINDS);
        if kind == Some(b"rule") {
            return false;
        }
        if kind == Some(b"zero_sectors") && zero.is_none() {
            let j = jparse(l);
            let masks: HashSet<Vec<u8>> = j
                .get(l, "masks")
                .map(|m| m.arr().iter().map(|x| x.s(l).to_vec()).collect())
                .unwrap_or_default();
            zero = Some((masks, super::sha_hex(l)));
        }
        if kind == Some(b"symmetry_group") && group.is_none() {
            let (c, actions) = crate::span::parse_group(l);
            if c.as_deref().is_none_or(|c| c == chart.as_bytes()) {
                group = Some((actions.clone(), super::sha_hex(l), actions.len()));
            }
        }
        let _ = J::Null;
        true
    });
    (zero, group)
}

impl Canon {
    /// Sign of slot `i` of pattern `p` on the domain: Some(true) = power >= 1 at every key, Some(false) = <= 0 at every
    /// key, None = not decided.
    fn positive(&self, p: &Pat, i: usize) -> Option<bool> {
        let (sym, v) = p[i];
        if !sym {
            return Some(v > 0);
        }
        let b = self.bounds.get(i).copied().unwrap_or_default();
        if b.lo.is_some_and(|lo| lo + v >= 1) {
            return Some(true);
        }
        if b.hi.is_some_and(|hi| hi + v <= 0) {
            return Some(false);
        }
        None
    }

    fn may_be_nonzero(&self, p: &Pat, i: usize) -> bool {
        let (sym, v) = p[i];
        sym || v != 0
    }

    pub fn is_zero_sector(&self, p: &Pat) -> bool {
        if self.zero.is_empty() {
            return false;
        }
        let mut mask = Vec::with_capacity(p.len());
        for i in 0..p.len() {
            match self.positive(p, i) {
                Some(true) => mask.push(b'1'),
                Some(false) => mask.push(b'0'),
                None => return false,
            }
        }
        self.zero.contains(&mask)
    }

    /// Image of `p` under element `e` when the action is exact on it and keeps every symbolic slot in place unflipped.
    fn image(&self, e: usize, p: &Pat) -> Option<(Pat, bool)> {
        let act = &self.actions[e];
        let mut out: Pat = p
            .iter()
            .map(|&(s, v)| if s { (true, v) } else { (false, 0) })
            .collect();
        let mut neg = false;
        for i in 0..p.len() {
            if !self.may_be_nonzero(p, i) {
                continue;
            }
            let (t, flip) = (*act.get(i)?)?;
            if t >= p.len() {
                return None;
            }
            if p[i].0 {
                if t != i || flip {
                    return None;
                }
                continue;
            }
            if out[t].0 {
                return None;
            }
            out[t] = p[i];
            if flip && p[i].1 % 2 != 0 {
                neg = !neg;
            }
        }
        Some((out, neg))
    }

    /// Normal form of one integral pattern.
    pub fn class(&self, p: &Pat) -> Class {
        if self.is_zero_sector(p) {
            return Class::Zero;
        }
        let mut seen: HashMap<Pat, bool> = HashMap::new();
        seen.insert(p.clone(), false);
        let mut q = VecDeque::from([p.clone()]);
        let mut conflict = false;
        while let Some(x) = q.pop_front() {
            let sx = seen[&x];
            for e in 0..self.actions.len() {
                let Some((y, neg)) = self.image(e, &x) else {
                    continue;
                };
                let sy = sx ^ neg;
                match seen.get(&y) {
                    Some(&s) => {
                        if s != sy {
                            conflict = true;
                        }
                    }
                    None => {
                        seen.insert(y.clone(), sy);
                        q.push_back(y);
                    }
                }
            }
        }
        if conflict {
            return Class::Zero;
        }
        let (rep, s) = seen.into_iter().min_by(|a, b| a.0.cmp(&b.0)).unwrap();
        Class::Rep(rep, s)
    }
}

/// Bounds per slot of the stored-frame variables (domain_bounds and the chart condition on the target), as in loci.
pub fn slot_bounds(dom: &super::loci::Domain, arity: usize) -> Vec<Bound> {
    (0..arity)
        .map(|i| dom.bounds.get(&Vars::slot(i)).copied().unwrap_or_default())
        .collect()
}

/// Class sets of the stored relation: target class + rhs classes (counts of zero-sector terms).
pub fn stored_classes(
    canon: &Canon,
    target: &Pat,
    rhs: &[Pat],
) -> (Class, BTreeMap<Pat, Vec<(usize, bool)>>, usize) {
    let tc = canon.class(target);
    let mut m: BTreeMap<Pat, Vec<(usize, bool)>> = BTreeMap::new();
    let mut zero = 0;
    for (i, p) in rhs.iter().enumerate() {
        match canon.class(p) {
            Class::Zero => zero += 1,
            Class::Rep(r, s) => m.entry(r).or_default().push((i, s)),
        }
    }
    (tc, m, zero)
}
