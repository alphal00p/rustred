//! The deliberately naive reference closure, written from the stated semantics only (ported from audit loop-B
//! reference.rs; adapted to the semantics fixed by lane loopfix, closure.md section 11: credits take the closure tier
//! with PENDING credits on unknown Tier C facts, regularity of the image counted like a direct key, no credit through a
//! terminal image, declared terminals inside a rule's domain give `terminal_in_rule_domain`; audit mutant hooks removed).
#![allow(
    dead_code,
    clippy::type_complexity,
    clippy::needless_range_loop,
    clippy::collapsible_if
)]
use super::spec::{NodeSpec, RuleSpec, Sl};
use rustred_campaign_core::order::NodeComparator;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cf {
    Zero,
    NonZero,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rf {
    Regular,
    Pole,
    Unknown,
}

/// Facts as seen by the reference: (table name, record, label, key, term) -> fact.
pub trait RefFacts {
    fn coef(&self, t: &str, rec: usize, label: i64, key: &[i32], term: usize) -> Cf;
    fn reg(&self, t: &str, rec: usize, label: i64, key: &[i32]) -> Rf;
}

#[derive(Clone, Debug, PartialEq)]
pub enum RD {
    Covered { t: usize, r: usize, pt: Vec<i32> },
    Uncovered,
    Blocked { t: usize, r: usize },
    Outside,
}

fn mutant() -> u32 {
    0
}
pub fn in_chart(sp: &NodeSpec, k: &[i32]) -> bool {
    k.iter()
        .enumerate()
        .all(|(i, &x)| if i < sp.nl { x >= 1 } else { x <= 0 })
}

/// rule point or None (not selected)
fn point(sp: &NodeSpec, r: &RuleSpec, k: &[i32]) -> Option<Vec<i32>> {
    for (i, f) in r.fixed.iter().enumerate() {
        if let Some(v) = f
            && k[i] != *v
        {
            return None;
        }
    }
    match &r.target {
        None => Some(k.to_vec()),
        Some(t) => {
            let mut pt: Vec<Option<i32>> = vec![None; sp.n];
            for (j, s) in t.iter().enumerate() {
                match *s {
                    Sl::C(c) => {
                        if k[j] != c {
                            return None;
                        }
                    }
                    Sl::P(a, o) => pt[a] = Some(k[j] - o),
                }
            }
            Some(
                pt.iter()
                    .enumerate()
                    .map(|(i, p)| p.unwrap_or(r.fixed[i].unwrap_or(0)))
                    .collect(),
            )
        }
    }
}

pub fn dispatch(sp: &NodeSpec, k: &[i32]) -> RD {
    if !in_chart(sp, k) {
        return RD::Outside;
    }
    for &t in &sp.priority {
        for (ri, r) in sp.tables[t].rules.iter().enumerate() {
            let Some(pt) = point(sp, r, k) else { continue };
            if r.eqs.iter().any(|e| e.eval(&pt) != 0) {
                continue;
            }
            if r.bounds.iter().any(|e| e.eval(&pt) < 0) {
                continue;
            }
            if mutant() != 2 && r.exc.iter().any(|b| b.iter().all(|e| e.eval(&pt) == 0)) {
                continue;
            }
            if r.poles.iter().any(|e| e.eval(&pt) == 0) || r.elided {
                return RD::Blocked { t, r: ri };
            }
            return RD::Covered { t, r: ri, pt };
        }
    }
    RD::Uncovered
}

pub fn instantiate(body: &[Vec<Sl>], pt: &[i32]) -> Vec<Vec<i32>> {
    body.iter()
        .map(|t| {
            t.iter()
                .map(|s| match *s {
                    Sl::P(a, o) => pt[a] + o,
                    Sl::C(c) => c,
                })
                .collect()
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cls {
    Top,
    Pn { top: bool },
    Sub(u64),
}
pub fn classify(sp: &NodeSpec, t: &[i32]) -> Cls {
    let lines_ok = (0..sp.nl).all(|i| t[i] >= 1);
    if (sp.nl..sp.n).any(|i| t[i] > 0) {
        return Cls::Pn { top: lines_ok };
    }
    if !lines_ok {
        let mut m = 0u64;
        for i in 0..sp.nl {
            if t[i] <= 0 {
                m |= 1 << i;
            }
        }
        return Cls::Sub(m);
    }
    Cls::Top
}

pub fn apply_g(sp: &NodeSpec, g: &(Vec<usize>, Option<Vec<usize>>), k: &[i32]) -> Option<Vec<i32>> {
    let mut out = k.to_vec();
    for (i, &j) in g.0.iter().enumerate() {
        out[j] = k[i];
    }
    match &g.1 {
        Some(m) => {
            for (i, &j) in m.iter().enumerate() {
                out[sp.nl + j] = k[sp.nl + i];
            }
        }
        None => {
            if k[sp.nl..].iter().any(|&x| x != 0) {
                return None;
            }
        }
    }
    Some(out)
}

#[derive(Clone, Debug, Default)]
pub struct RefOut {
    pub keys: BTreeSet<Vec<i32>>,
    pub order: Vec<Vec<i32>>,
    pub rounds: u32,
    pub edges: u64,
    pub nondesc: BTreeSet<(Vec<i32>, String, usize, usize, Vec<i32>)>,
    pub open: BTreeMap<Vec<i32>, (String, Option<(String, usize)>)>,
    pub used: BTreeMap<(String, usize), (u64, u64)>,
    pub hand: BTreeMap<u64, BTreeSet<Vec<i32>>>,
    pub terminals: BTreeMap<Vec<i32>, String>,
    pub credits: BTreeMap<Vec<i32>, (usize, Vec<i32>, (String, usize))>,
    /// all valid (element) choices per credited key
    pub credit_valid: BTreeMap<Vec<i32>, Vec<usize>>,
    pub has_cycle: bool,
    pub longest: Option<u64>,
    /// Tier K credits: followed top terms of an element that are not below the key (re-audit F1a)
    pub credit_rz: u64,
    pub top_terms: u64,
    pub pn_top: u64,
    pub pn_sub: u64,
    pub sub_terms: u64,
    pub requires_zero: u64,
    pub unrep: u64,
    pub pos_nonzero: u64,
    pub dropped_zero: u64,
    pub und_pn: u64,
    pub und_nl: u64,
    pub und_reg: u64,
    pub needed: BTreeSet<(String, usize, i64, Vec<i32>, String, Option<usize>)>,
    pub verdict: String,
    pub terminal_in_rule_domain: u64,
    pub succ: HashMap<Vec<i32>, Vec<Vec<i32>>>,
}

pub struct RefCfg<'a> {
    pub tier_c: bool,
    pub credits: bool,
    pub terminals: bool,
    pub require_regularity: bool,
    pub facts: Option<&'a dyn RefFacts>,
}

/// One credit attempt by the literal condition, with the checks of the closure tier. Returns every valid element
/// (clean or pending) as (element, image, table, record, point, pending).
#[allow(clippy::type_complexity)]
fn credit_try(
    sp: &NodeSpec,
    cmp: &NodeComparator,
    k: &[i32],
    cfg: &RefCfg,
    terminal: &dyn Fn(&[i32]) -> bool,
) -> Vec<(usize, Vec<i32>, usize, usize, Vec<i32>, bool)> {
    let mut valid = vec![];
    for (gi, g) in sp.group.iter().enumerate() {
        let Some(img) = apply_g(sp, g, k) else {
            continue;
        };
        if img == k || terminal(&img) {
            continue;
        }
        let RD::Covered { t, r, pt } = dispatch(sp, &img) else {
            continue;
        };
        let rule = &sp.tables[t].rules[r];
        let Some(body) = &rule.body else { continue };
        let terms = instantiate(body, &pt);
        let mut ok = true;
        let mut pending = false;
        if cfg.tier_c {
            match cfg
                .facts
                .unwrap()
                .reg(&sp.tables[t].name, r, rule.label, &img)
            {
                Rf::Pole => ok = false,
                Rf::Unknown => pending = true,
                Rf::Regular => {}
            }
        }
        for (j, tm) in terms.iter().enumerate() {
            if !ok {
                break;
            }
            let cls = classify(sp, tm);
            let f = if cfg.tier_c {
                cfg.facts
                    .unwrap()
                    .coef(&sp.tables[t].name, r, rule.label, &img, j)
            } else {
                Cf::Unknown
            };
            if f == Cf::Zero {
                continue;
            }
            match cls {
                Cls::Pn { .. } if cfg.tier_c => {
                    if f == Cf::NonZero {
                        ok = false;
                    } else {
                        pending = true;
                    }
                }
                Cls::Top if !cmp.lower(tm, k) => {
                    if !cfg.tier_c || f == Cf::NonZero {
                        ok = false;
                    } else {
                        pending = true;
                    }
                }
                _ => {}
            }
        }
        if ok {
            valid.push((gi, img, t, r, pt, pending));
        }
    }
    valid
}

/// Tier K (lane closurefix, re-audit F1a): every element not refused for a STRUCTURAL reason (image exists, differs,
/// not a terminal, its dispatch covers it with a rule that has a body): the superset follows all of them.
fn credit_candidates(
    sp: &NodeSpec,
    k: &[i32],
    terminal: &dyn Fn(&[i32]) -> bool,
) -> Vec<(usize, Vec<i32>, usize, usize, Vec<i32>)> {
    let mut v = vec![];
    for (gi, g) in sp.group.iter().enumerate() {
        let Some(img) = apply_g(sp, g, k) else {
            continue;
        };
        if img == k || terminal(&img) {
            continue;
        }
        let RD::Covered { t, r, pt } = dispatch(sp, &img) else {
            continue;
        };
        if sp.tables[t].rules[r].body.is_none() {
            continue;
        }
        v.push((gi, img, t, r, pt));
    }
    v
}

pub fn run(sp: &NodeSpec, cmp: &NodeComparator, cfg: &RefCfg) -> RefOut {
    let mut o = RefOut::default();
    let start: BTreeSet<Vec<i32>> = sp.start.iter().cloned().collect();
    let tp: HashSet<Vec<i32>> = if cfg.terminals {
        sp.term_pref.iter().cloned().collect()
    } else {
        HashSet::new()
    };
    let to: HashSet<Vec<i32>> = if cfg.terminals {
        sp.term_other.iter().cloned().collect()
    } else {
        HashSet::new()
    };
    let mut level: Vec<Vec<i32>> = start.iter().cloned().collect();
    for k in &level {
        o.keys.insert(k.clone());
    }
    let mut adj: HashMap<Vec<i32>, Vec<Vec<i32>>> = HashMap::new();
    while !level.is_empty() {
        o.rounds += 1;
        o.order.extend(level.iter().cloned());
        let mut next: Vec<Vec<i32>> = vec![];
        for k in &level {
            let ins = start.contains(k);
            if tp.contains(k) || to.contains(k) {
                o.terminals.insert(
                    k.clone(),
                    if tp.contains(k) { "preferred" } else { "other" }.into(),
                );
                if matches!(dispatch(sp, k), RD::Covered { .. } | RD::Blocked { .. }) {
                    o.terminal_in_rule_domain += 1;
                }
                continue;
            }
            let d = dispatch(sp, k);
            // the servers followed at k: (table, record, point, key the rule is applied at); one, except for a Tier K
            // credit (the union of its candidates)
            let servers: Vec<(usize, usize, Vec<i32>, Vec<i32>)> = match d {
                RD::Outside => {
                    o.open.insert(k.clone(), ("outside_chart".into(), None));
                    continue;
                }
                RD::Blocked { t, r } => {
                    o.open.insert(
                        k.clone(),
                        ("blocked".into(), Some((sp.tables[t].name.clone(), r))),
                    );
                    continue;
                }
                RD::Covered { t, r, pt } => {
                    if sp.tables[t].rules[r].body.is_none() {
                        o.open.insert(
                            k.clone(),
                            ("no_body".into(), Some((sp.tables[t].name.clone(), r))),
                        );
                        continue;
                    }
                    vec![(t, r, pt, k.clone())]
                }
                RD::Uncovered => {
                    let is_term = |x: &[i32]| tp.contains(x) || to.contains(x);
                    let v = if cfg.credits {
                        credit_try(sp, cmp, k, cfg, &is_term)
                    } else {
                        vec![]
                    };
                    let cands = if cfg.credits && !cfg.tier_c {
                        credit_candidates(sp, k, &is_term)
                    } else {
                        vec![]
                    };
                    if v.is_empty() {
                        o.open.insert(k.clone(), ("uncovered".into(), None));
                        if cands.is_empty() {
                            continue;
                        }
                        // Tier K: open, its candidates followed all the same
                        cands
                            .into_iter()
                            .map(|(_, img, t, r, pt)| (t, r, pt, img))
                            .collect()
                    } else {
                        let first = &v[0];
                        let (gi, img, t, r, pt, _) = first.clone();
                        o.credits
                            .insert(k.clone(), (gi, img.clone(), (sp.tables[t].name.clone(), r)));
                        o.credit_valid
                            .insert(k.clone(), v.iter().map(|x| x.0).collect());
                        let mut sv = vec![(t, r, pt, img)];
                        for (g2, img2, t2, r2, pt2) in cands {
                            if g2 != gi {
                                sv.push((t2, r2, pt2, img2));
                            }
                        }
                        sv
                    }
                }
            };
            let mut dist: BTreeSet<Vec<i32>> = BTreeSet::new();
            let mut used_here: BTreeSet<(String, usize)> = BTreeSet::new();
            for (t, r, pt, fkey) in servers {
            let rule = &sp.tables[t].rules[r];
            let tname = sp.tables[t].name.clone();
            let credited = fkey != *k;
            if cfg.tier_c && cfg.require_regularity {
                match cfg.facts.unwrap().reg(&tname, r, rule.label, &fkey) {
                    Rf::Pole if !credited => {
                        o.open
                            .insert(k.clone(), ("irregular".into(), Some((tname, r))));
                        continue;
                    }
                    Rf::Pole => unreachable!("a credit through a pole is refused"),
                    Rf::Unknown => {
                        o.und_reg += 1;
                        o.needed.insert((
                            tname.clone(),
                            r,
                            rule.label,
                            fkey.clone(),
                            "regularity".into(),
                            None,
                        ));
                    }
                    Rf::Regular => {}
                }
            }
            if used_here.insert((tname.clone(), r)) {
                let e = o.used.entry((tname.clone(), r)).or_default();
                e.0 += 1;
                if ins {
                    e.1 += 1;
                }
            }
            let terms = instantiate(rule.body.as_ref().unwrap(), &pt);
            for (j, tm) in terms.iter().enumerate() {
                let cls = classify(sp, tm);
                let f = if cfg.tier_c {
                    cfg.facts.unwrap().coef(&tname, r, rule.label, &fkey, j)
                } else {
                    Cf::Unknown
                };
                if f == Cf::Zero {
                    o.dropped_zero += 1;
                    continue;
                }
                if !matches!(cls, Cls::Pn { .. }) && tm.iter().any(|&x| !(-32..=31).contains(&x)) {
                    o.unrep += 1;
                    if cls == Cls::Top {
                        o.top_terms += 1;
                        if !cmp.lower(tm, k) {
                            if credited && !cfg.tier_c {
                                o.credit_rz += 1;
                            } else if !cfg.tier_c || f == Cf::NonZero {
                                o.nondesc
                                    .insert((k.clone(), tname.clone(), r, j, tm.clone()));
                            } else {
                                o.und_nl += 1;
                                o.needed.insert((
                                    tname.clone(),
                                    r,
                                    rule.label,
                                    fkey.clone(),
                                    "coefficient".into(),
                                    Some(j),
                                ));
                            }
                        }
                    }
                    continue;
                }
                match cls {
                    Cls::Pn { top } => {
                        if top {
                            o.pn_top += 1
                        } else {
                            o.pn_sub += 1
                        }
                        if !cfg.tier_c {
                            o.requires_zero += 1;
                        } else if f == Cf::NonZero {
                            o.pos_nonzero += 1;
                        } else {
                            o.und_pn += 1;
                            o.needed.insert((
                                tname.clone(),
                                r,
                                rule.label,
                                fkey.clone(),
                                "coefficient".into(),
                                Some(j),
                            ));
                        }
                    }
                    Cls::Sub(m) => {
                        o.sub_terms += 1;
                        o.hand.entry(m).or_default().insert(tm.clone());
                    }
                    Cls::Top => {
                        o.top_terms += 1;
                        dist.insert(tm.clone());
                        if !cmp.lower(tm, k) {
                            if credited && !cfg.tier_c {
                                o.credit_rz += 1;
                            } else if !cfg.tier_c || f == Cf::NonZero {
                                o.nondesc
                                    .insert((k.clone(), tname.clone(), r, j, tm.clone()));
                            } else {
                                o.und_nl += 1;
                                o.needed.insert((
                                    tname.clone(),
                                    r,
                                    rule.label,
                                    fkey.clone(),
                                    "coefficient".into(),
                                    Some(j),
                                ));
                            }
                        }
                        if o.keys.insert(tm.clone()) {
                            next.push(tm.clone());
                        }
                    }
                }
            }
            }
            o.edges += dist.len() as u64
                + if mutant() == 3 && dist.len() > 2 {
                    1
                } else {
                    0
                };
            let v: Vec<Vec<i32>> = dist.into_iter().collect();
            o.succ.insert(k.clone(), v.clone());
            adj.insert(k.clone(), v);
        }
        next.sort();
        level = next;
    }
    // explicit DFS cycle detection and longest chain
    let idx: HashMap<&Vec<i32>, usize> = o.order.iter().enumerate().map(|(i, k)| (k, i)).collect();
    let nn = o.order.len();
    let succ: Vec<Vec<usize>> = o
        .order
        .iter()
        .map(|k| {
            adj.get(k)
                .map(|v| v.iter().map(|t| idx[t]).collect())
                .unwrap_or_default()
        })
        .collect();
    let mut color = vec![0u8; nn];
    let mut depth = vec![0u64; nn];
    let mut cyc = false;
    for s in 0..nn {
        if color[s] != 0 {
            continue;
        }
        let mut st: Vec<(usize, usize)> = vec![(s, 0)];
        color[s] = 1;
        while let Some(&mut (u, ref mut ei)) = st.last_mut() {
            if *ei < succ[u].len() {
                let v = succ[u][*ei];
                *ei += 1;
                if color[v] == 1 {
                    cyc = true;
                } else if color[v] == 0 {
                    color[v] = 1;
                    st.push((v, 0));
                }
            } else {
                color[u] = 2;
                depth[u] = succ[u].iter().map(|&v| depth[v] + 1).max().unwrap_or(0);
                st.pop();
            }
        }
    }
    o.has_cycle = cyc;
    o.longest = if cyc {
        None
    } else {
        Some(depth.iter().copied().max().unwrap_or(0))
    };
    // a cycle needs a followed non-lower edge, already counted (lane closurefix, re-audit F3): never a failure by itself
    let failed = !o.nondesc.is_empty() || o.pos_nonzero > 0 || o.unrep > 0;
    o.verdict = if failed {
        "failed"
    } else if !o.open.is_empty() {
        "open"
    } else if o.terminal_in_rule_domain > 0 {
        "terminal_in_rule_domain"
    } else if !cfg.tier_c {
        "superset_clean"
    } else if o.und_pn + o.und_nl + o.und_reg > 0 {
        "undecided"
    } else {
        "closed"
    }
    .into();
    o
}
