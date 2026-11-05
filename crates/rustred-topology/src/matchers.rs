//! Forcer's rule detection (generation/pathforcer.py findTopologyActionAndDots :1392-1500, findDiamond :1030-1061) in
//! Forcer's order Insertion > Carpet > Triangle > Diamond > Custom, with frozen choice rules (part of
//! [`crate::P_TOPOLOGY_VERSION`]). Line and vertex indices are those of the topology passed in: canonical for
//! [`crate::NodeInfo::topo`].

use std::collections::BTreeSet;

use smallvec::SmallVec;

use crate::graph::{adjacency, connected_without_edges, cycle_masks};
use crate::{GraphKey, Normalised, PTopo, canonical, normalise};

/// One-loop insertion: lines `keep < drop` are parallel (a bubble). Choice: the parallel pair with the smallest
/// `(eps(keep) + eps(drop), keep, drop)`. The rule output keeps `keep` with eps units `eps(keep) + eps(drop) + 1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ins {
    pub keep: u8,
    pub drop: u8,
}

/// Carpet: line `r` joins the two external vertices (the graph is simple here, so `r` is unique). `third` = the line
/// carrying `q - mom(r)` alone (the bridge of `Gamma - r`), if any; `blob` = every other line (sorted),
/// `blob_loops = L - 1`, `blob_eps` = the sum of the blob's eps units.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CarpetMatch {
    pub r: u8,
    pub third: Option<u8>,
    pub blob: SmallVec<[u8; 16]>,
    pub blob_loops: u8,
    pub blob_eps: u8,
}

/// Triangle: internal vertices `v_a`, `v_b` of degree 3 joined by line `b`, apex vertex `apex` adjacent to both
/// (`apex` may be external), `a1 = (v_a, apex)`, `a2 = (v_b, apex)`, `c1` / `c2` = the third line at `v_a` / `v_b`;
/// `b`, `c1`, `c2` are integer, `a1`, `a2` may carry eps. Choice: fewest Custom children (distinct nonzero
/// contractions of `b`, `c1`, `c2` whose action is Custom), then fewest distinct children, then the smallest
/// `(a1, a2, b, c1, c2)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TriMatch {
    pub a1: u8,
    pub a2: u8,
    pub b: u8,
    pub c1: u8,
    pub c2: u8,
    pub apex: u8,
    pub v_a: u8,
    pub v_b: u8,
}

/// Diamond (detection only in M2; the lane is M3): bottom vertex `bottom` (internal, degree >= 3), top vertex `top`
/// adjacent to every other neighbour `x` of `bottom` (the middle vertices, internal, degree 3); for every middle
/// vertex in increasing order: `a` = line (x, top), `b` = line (x, bottom) (integer), `c` = its third line (integer);
/// `spect` = the line (bottom, top) if present. Choice: the smallest `(bottom, top)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DiaMatch {
    pub a: SmallVec<[u8; 6]>,
    pub b: SmallVec<[u8; 6]>,
    pub c: SmallVec<[u8; 6]>,
    pub spect: SmallVec<[u8; 4]>,
    pub top: u8,
    pub bottom: u8,
}

/// Forcer's action of a normalised topology (the DAG label; not the RustRed lane).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Trivial,
    Insertion(Ins),
    Carpet(CarpetMatch),
    Triangle(TriMatch),
    Diamond(DiaMatch),
    Custom,
}

/// The kind of an [`Action`] without its match data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActionKind {
    Trivial,
    Insertion,
    Carpet,
    Triangle,
    Diamond,
    Custom,
}

impl Action {
    pub fn kind(&self) -> ActionKind {
        match self {
            Action::Trivial => ActionKind::Trivial,
            Action::Insertion(_) => ActionKind::Insertion,
            Action::Carpet(_) => ActionKind::Carpet,
            Action::Triangle(_) => ActionKind::Triangle,
            Action::Diamond(_) => ActionKind::Diamond,
            Action::Custom => ActionKind::Custom,
        }
    }
}

impl ActionKind {
    /// Forcer's name of the action (`insertion`, `carpet`, `triangle`, `diamond`, `custom`, `trivial`).
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionKind::Trivial => "trivial",
            ActionKind::Insertion => "insertion",
            ActionKind::Carpet => "carpet",
            ActionKind::Triangle => "triangle",
            ActionKind::Diamond => "diamond",
            ActionKind::Custom => "custom",
        }
    }
}

/// Line incidences per vertex: `(neighbour, line)`.
struct Incidence {
    at: Vec<SmallVec<[(usize, usize); 6]>>,
}

impl Incidence {
    fn new(t: &PTopo) -> Self {
        let mut at = vec![SmallVec::new(); usize::from(t.n)];
        for (i, &(u, v, _)) in t.lines.iter().enumerate() {
            let (u, v) = (usize::from(u), usize::from(v));
            at[u].push((v, i));
            if u != v {
                at[v].push((u, i));
            }
        }
        Incidence { at }
    }

    fn degree(&self, v: usize) -> usize {
        self.at[v].len()
    }

    fn neighbours(&self, v: usize) -> BTreeSet<usize> {
        self.at[v].iter().map(|&(w, _)| w).collect()
    }

    /// The line between `u` and `v` (simple graph).
    fn line(&self, u: usize, v: usize) -> Option<usize> {
        self.at[u].iter().find(|&&(w, _)| w == v).map(|&(_, i)| i)
    }
}

fn is_external(t: &PTopo, v: usize) -> bool {
    v == usize::from(t.s) || v == usize::from(t.t)
}

pub(crate) fn insertion(t: &PTopo) -> Option<Ins> {
    let mut best: Option<(u16, usize, usize)> = None;
    for (i, &(u, v, ei)) in t.lines.iter().enumerate() {
        for (j, &(x, y, ej)) in t.lines.iter().enumerate().skip(i + 1) {
            if (u, v) == (x, y) || (u, v) == (y, x) {
                let cand = (u16::from(ei) + u16::from(ej), i, j);
                if best.is_none_or(|b| cand < b) {
                    best = Some(cand);
                }
            }
        }
    }
    best.map(|(_, i, j)| Ins {
        keep: i as u8,
        drop: j as u8,
    })
}

pub(crate) fn carpet(t: &PTopo) -> Option<CarpetMatch> {
    if t.lines.len() < 2 {
        return None;
    }
    let (s, tt) = (t.s, t.t);
    let r = t
        .lines
        .iter()
        .position(|&(u, v, _)| (u, v) == (s, tt) || (u, v) == (tt, s))?;
    // third: the line whose removal (with r) separates s from t, i.e. it carries q - mom(r)
    let n = usize::from(t.n);
    let edges: Vec<(usize, usize)> = t
        .lines
        .iter()
        .map(|&(u, v, _)| (usize::from(u), usize::from(v)))
        .collect();
    let rest: Vec<usize> = (0..edges.len()).filter(|&i| i != r).collect();
    let rest_edges: Vec<(usize, usize)> = rest.iter().map(|&i| edges[i]).collect();
    let masks = cycle_masks(n, &rest_edges);
    let separating: Vec<usize> = rest
        .iter()
        .zip(&masks)
        .filter(|&(&i, &m)| {
            m == 0 && !connected_without_edges(n, &edges, &[r, i], usize::from(s), usize::from(tt))
        })
        .map(|(&i, _)| i)
        .collect();
    let third = match separating.as_slice() {
        [x] => Some(*x as u8),
        _ => None,
    };
    let blob: SmallVec<[u8; 16]> = (0..t.lines.len())
        .filter(|&i| i != r && Some(i as u8) != third)
        .map(|i| i as u8)
        .collect();
    let blob_eps = blob.iter().map(|&i| t.lines[usize::from(i)].2).sum();
    Some(CarpetMatch {
        r: r as u8,
        third,
        blob,
        blob_loops: t.loops().saturating_sub(1),
        blob_eps,
    })
}

/// Every triangle of a simple topology, both orientations of each.
pub(crate) fn triangles(t: &PTopo) -> Vec<TriMatch> {
    let inc = Incidence::new(t);
    let mut out = Vec::new();
    for (b, &(x, y, eb)) in t.lines.iter().enumerate() {
        let (x, y) = (usize::from(x), usize::from(y));
        if eb != 0
            || is_external(t, x)
            || is_external(t, y)
            || inc.degree(x) != 3
            || inc.degree(y) != 3
        {
            continue;
        }
        let nx = inc.neighbours(x);
        let ny = inc.neighbours(y);
        for &z in nx.intersection(&ny) {
            if z == x || z == y {
                continue;
            }
            let (Some(a1), Some(a2)) = (inc.line(x, z), inc.line(y, z)) else {
                continue;
            };
            let c1 = inc.at[x]
                .iter()
                .map(|&(_, i)| i)
                .find(|&i| i != b && i != a1);
            let c2 = inc.at[y]
                .iter()
                .map(|&(_, i)| i)
                .find(|&i| i != b && i != a2);
            let (Some(c1), Some(c2)) = (c1, c2) else {
                continue;
            };
            if t.lines[c1].2 != 0 || t.lines[c2].2 != 0 {
                continue;
            }
            let m = |a1: usize, a2: usize, c1: usize, c2: usize, va: usize, vb: usize| TriMatch {
                a1: a1 as u8,
                a2: a2 as u8,
                b: b as u8,
                c1: c1 as u8,
                c2: c2 as u8,
                apex: z as u8,
                v_a: va as u8,
                v_b: vb as u8,
            };
            out.push(m(a1, a2, c1, c2, x, y));
            out.push(m(a2, a1, c2, c1, y, x));
        }
    }
    out
}

pub(crate) fn diamond(t: &PTopo) -> Option<DiaMatch> {
    let inc = Incidence::new(t);
    let n = usize::from(t.n);
    for bottom in 0..n {
        if is_external(t, bottom) || inc.degree(bottom) < 3 {
            continue;
        }
        let nb = inc.neighbours(bottom);
        let mut top: Option<BTreeSet<usize>> = None;
        for &x in &nb {
            let mut closed = inc.neighbours(x);
            closed.insert(x);
            top = Some(match top {
                None => closed,
                Some(acc) => acc.intersection(&closed).copied().collect(),
            });
        }
        for &tv in top.unwrap_or_default().iter().filter(|&&v| v != bottom) {
            let mids: Vec<usize> = nb.iter().copied().filter(|&x| x != tv).collect();
            let ok = mids.iter().all(|&x| {
                if inc.degree(x) != 3 || is_external(t, x) {
                    return false;
                }
                let Some(bx) = inc.line(bottom, x) else {
                    return false;
                };
                if t.lines[bx].2 != 0 {
                    return false;
                }
                inc.at[x]
                    .iter()
                    .filter(|&&(w, _)| w != bottom && w != tv)
                    .all(|&(_, i)| t.lines[i].2 == 0)
            });
            if !ok {
                continue;
            }
            let mut m = DiaMatch {
                a: SmallVec::new(),
                b: SmallVec::new(),
                c: SmallVec::new(),
                spect: SmallVec::new(),
                top: tv as u8,
                bottom: bottom as u8,
            };
            for &x in &mids {
                m.a.push(inc.line(x, tv).expect("middle vertex adjacent to top") as u8);
                m.b.push(
                    inc.line(x, bottom)
                        .expect("middle vertex adjacent to bottom") as u8,
                );
                for &(w, i) in &inc.at[x] {
                    if w != bottom && w != tv {
                        m.c.push(i as u8);
                    }
                }
            }
            if let Some(sp) = inc.line(bottom, tv) {
                m.spect.push(sp as u8);
            }
            return Some(m);
        }
    }
    None
}

/// The kind of Forcer's action, detection only (no choice rules, no children).
pub(crate) fn action_kind(t: &PTopo) -> ActionKind {
    if t.lines.is_empty() {
        ActionKind::Trivial
    } else if insertion(t).is_some() {
        ActionKind::Insertion
    } else if carpet(t).is_some() {
        ActionKind::Carpet
    } else if !triangles(t).is_empty() {
        ActionKind::Triangle
    } else if diamond(t).is_some() {
        ActionKind::Diamond
    } else {
        ActionKind::Custom
    }
}

/// What a contraction child contributes to the triangle choice: its key and whether its action is Custom.
pub(crate) type ChildSummary = Option<(GraphKey, bool)>;

/// Rank of a triangle candidate: (#Custom children, #distinct children, (a1, a2, b, c1, c2)); smallest wins.
type TriangleRank = (usize, usize, [u8; 5]);

/// Forcer's action with the frozen choice rules; `child(line)` summarises the contraction of an integer line.
pub(crate) fn action_with(t: &PTopo, child: &mut dyn FnMut(usize) -> ChildSummary) -> Action {
    if t.lines.is_empty() {
        return Action::Trivial;
    }
    if let Some(ins) = insertion(t) {
        return Action::Insertion(ins);
    }
    if let Some(c) = carpet(t) {
        return Action::Carpet(c);
    }
    let tris = triangles(t);
    if !tris.is_empty() {
        let mut best: Option<(TriangleRank, TriMatch)> = None;
        for m in tris {
            let mut keys: Vec<(GraphKey, bool)> = [m.b, m.c1, m.c2]
                .iter()
                .filter_map(|&l| child(usize::from(l)))
                .collect();
            keys.sort();
            keys.dedup();
            let custom = keys.iter().filter(|k| k.1).count();
            let rank = (custom, keys.len(), [m.a1, m.a2, m.b, m.c1, m.c2]);
            if best.as_ref().is_none_or(|b| rank < b.0) {
                best = Some((rank, m));
            }
        }
        return Action::Triangle(best.expect("non-empty").1);
    }
    if let Some(d) = diamond(t) {
        return Action::Diamond(d);
    }
    Action::Custom
}

/// The topology with integer line `line` contracted (endpoints identified), not normalised.
pub(crate) fn contracted(t: &PTopo, line: usize) -> PTopo {
    let (a, b, _) = t.lines[line];
    let lines = t
        .lines
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != line)
        .map(|(_, &(u, v, e))| (if u == b { a } else { u }, if v == b { a } else { v }, e))
        .collect();
    PTopo {
        n: t.n,
        s: if t.s == b { a } else { t.s },
        t: if t.t == b { a } else { t.t },
        lines,
    }
}

/// Key and Custom flag of the normalised contraction of `line`; `None` if it is zero.
pub(crate) fn summarise_contraction(t: &PTopo, line: usize) -> ChildSummary {
    match normalise(&contracted(t, line)) {
        Normalised::Zero => None,
        Normalised::Trivial => Some((canonical(&PTopo::trivial()).0, false)),
        Normalised::Topo(p, _) => {
            let custom = action_kind(&p) == ActionKind::Custom;
            Some((canonical(&p).0, custom))
        }
    }
}

/// Forcer's action of a normalised topology, Insertion > Carpet > Triangle > Diamond > Custom, with the frozen choice
/// rules of [`Ins`], [`TriMatch`] and [`DiaMatch`] applied to `t`'s own line and vertex indices (canonical for
/// [`crate::NodeInfo::topo`]). No line: [`Action::Trivial`].
pub fn forcer_action(t: &PTopo) -> Action {
    t.assert_valid();
    action_with(t, &mut |line| summarise_contraction(t, line))
}

/// Is the glued graph `Gamma + e0` free of a K4 minor (series-parallel)? Reduction: drop self-loops and vertices of
/// degree <= 1, collapse parallel edges, suppress vertices of degree 2, until nothing changes; series-parallel iff at
/// most two vertices remain.
pub fn is_series_parallel(t: &PTopo) -> bool {
    t.assert_valid();
    let n = usize::from(t.n);
    let mut edges: Vec<(usize, usize)> = t
        .lines
        .iter()
        .map(|&(u, v, _)| (usize::from(u), usize::from(v)))
        .collect();
    edges.push((usize::from(t.s), usize::from(t.t)));
    let mut alive = vec![true; n];
    loop {
        edges.retain(|&(u, v)| u != v);
        let mut simple: Vec<(usize, usize)> =
            edges.iter().map(|&(u, v)| (u.min(v), u.max(v))).collect();
        simple.sort_unstable();
        simple.dedup();
        edges = simple;
        let adj = adjacency(n, &edges);
        let deg = |v: usize| adj[v].count_ones() as usize;
        let mut changed = false;
        for v in 0..n {
            if !alive[v] {
                continue;
            }
            match deg(v) {
                0 | 1 => {
                    alive[v] = false;
                    edges.retain(|&(a, b)| a != v && b != v);
                    changed = true;
                }
                2 => {
                    let nb: Vec<usize> = (0..n).filter(|&w| adj[v] & (1 << w) != 0).collect();
                    edges.retain(|&(a, b)| a != v && b != v);
                    edges.push((nb[0], nb[1]));
                    alive[v] = false;
                    changed = true;
                }
                _ => {}
            }
            if changed {
                break;
            }
        }
        if !changed {
            return alive.iter().filter(|&&a| a).count() <= 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topo(n: u8, s: u8, t: u8, lines: &[(u8, u8, u8)]) -> PTopo {
        PTopo {
            n,
            s,
            t,
            lines: lines.iter().copied().collect(),
        }
    }

    #[test]
    fn bubble_insertion_prefers_the_smallest_eps_pair() {
        let sunset = topo(2, 0, 1, &[(0, 1, 2), (0, 1, 0), (0, 1, 1)]);
        assert_eq!(
            forcer_action(&sunset),
            Action::Insertion(Ins { keep: 1, drop: 2 })
        );
    }

    #[test]
    fn kite_is_a_triangle_and_its_eps_central_variant_is_custom() {
        // S=0, T=3, rung 1-2
        let kite = topo(
            4,
            0,
            3,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 0), (1, 3, 0), (2, 3, 0)],
        );
        let Action::Triangle(m) = forcer_action(&kite) else {
            panic!("kite is a triangle")
        };
        assert_eq!(m.b, 2);
        let t1star05 = topo(
            4,
            0,
            3,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 1), (1, 3, 0), (2, 3, 0)],
        );
        assert_eq!(forcer_action(&t1star05), Action::Custom);
        // eps on a side line: the triangle with that line as a1 or a2 survives
        let side = topo(
            4,
            0,
            3,
            &[(0, 1, 1), (0, 2, 0), (1, 2, 0), (1, 3, 0), (2, 3, 0)],
        );
        assert_eq!(forcer_action(&side).kind(), ActionKind::Triangle);
    }

    #[test]
    fn carpet_finds_r_and_third() {
        // S=0, T=1: r = (0,1), third = (0,2) (the only other line at S), blob = a kite between 2 and T
        let t = topo(
            5,
            0,
            1,
            &[
                (0, 1, 0),
                (0, 2, 0),
                (2, 3, 0),
                (2, 4, 0),
                (3, 4, 1),
                (3, 1, 0),
                (4, 1, 0),
            ],
        );
        let Action::Carpet(c) = forcer_action(&t) else {
            panic!("carpet")
        };
        assert_eq!(c.r, 0);
        assert_eq!(c.third, Some(1));
        assert_eq!(c.blob.as_slice(), &[2, 3, 4, 5, 6]);
        assert_eq!(c.blob_loops, 2);
        assert_eq!(c.blob_eps, 1);
    }

    #[test]
    fn series_parallel_test() {
        let kite = topo(
            4,
            0,
            3,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 0), (1, 3, 0), (2, 3, 0)],
        );
        assert!(!is_series_parallel(&kite)); // kite + e0 = K4
        let sunset = topo(2, 0, 1, &[(0, 1, 0), (0, 1, 0), (0, 1, 0)]);
        assert!(is_series_parallel(&sunset));
        assert!(is_series_parallel(&PTopo::trivial()));
    }
}
