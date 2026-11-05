//! Forcer's `mergePropagators` (generation/pathforcer.py:1064-1114) expressed on graphs.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::graph::{biconnected, cycle_masks};
use crate::{PTopo, canonical};
use serde::{Deserialize, Serialize};

/// What became of one input line.
///
/// - `Kept(c)`: the line is output line `c`, and no other input line maps to `c`.
/// - `SeriesMergedInto(c)`: the line and at least one other input line became output line `c` (lines with equal
///   momentum: their integer powers add in the edge map, their eps units add in the topology). On a DAG rule edge the
///   integrated bubble pair of an insertion carries the fate of the line that replaces it.
/// - `AbsorbedIntoQ`: the line carried `q` alone (it was in series with the external line) and is now a factor of
///   `q`; on a carpet rule edge the integrated line `r` too.
/// - `Contracted`: the line was removed with its endpoints identified (a DAG contraction edge, power 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fate {
    Kept(u8),
    SeriesMergedInto(u8),
    AbsorbedIntoQ,
    Contracted,
}

impl Fate {
    /// The output line, if the input line survives in one.
    pub fn line(&self) -> Option<u8> {
        match *self {
            Fate::Kept(c) | Fate::SeriesMergedInto(c) => Some(c),
            Fate::AbsorbedIntoQ | Fate::Contracted => None,
        }
    }
}

/// Result of [`normalise`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Normalised {
    /// Scaleless: `s == t`, a self-loop, or the glued graph `Gamma + e0` is not 2-connected.
    Zero,
    /// No internal line left (Forcer's `d1`); every input line was absorbed into `q`.
    Trivial,
    /// The normalised topology (vertices relabelled compactly, not canonically) and one [`Fate`] per input line.
    Topo(PTopo, Vec<Fate>),
}

static NONLOCAL_MERGES: AtomicU64 = AtomicU64::new(0);
static NONLOCAL_AMBIGUOUS: AtomicU64 = AtomicU64::new(0);

/// `(events, ambiguous)`: non-local series merges performed so far in this process (a 2-edge cut of the glued graph
/// whose lines do not meet at a two-valent vertex), and those among them whose candidate survivors gave different
/// keys (where the minimal-key rule of [`normalise`] decided). Diagnostic only.
#[doc(hidden)]
pub fn nonlocal_merge_events() -> (u64, u64) {
    (
        NONLOCAL_MERGES.load(Ordering::Relaxed),
        NONLOCAL_AMBIGUOUS.load(Ordering::Relaxed),
    )
}

/// Forcer's normal form of a propagator topology.
///
/// Zero if `s == t`, a self-loop exists, or the glued graph `Gamma + e0` (`e0` = the external line `s`-`t`) is not
/// 2-connected (eps lines do not change this: scaleless stays scaleless). Otherwise repeat until nothing applies:
/// a line in series with `e0` (a bridge of `Gamma`, it carries `q` alone) is contracted and absorbed into `q` (its eps
/// units go to the external line, which is not part of a topology); two lines in series at an internal two-valent
/// vertex merge into one line, eps units added (the lower input index survives). No internal line left: `Trivial`.
///
/// Lines with equal momentum that do not meet at a two-valent vertex (a non-local 2-edge cut: a chain of two-point
/// blobs inside a larger graph, which an insertion output can create) have no labelling-independent survivor: keeping
/// one line or the other moves the blobs along the chain, which leaves the integral unchanged but can change the graph.
/// There every choice is normalised further and the one with the smallest [`crate::GraphKey`] is taken (ties: the
/// lowest index). Measured: 4 such merges in the 4L ForcerTree DAG, 2 with candidates of different keys (the insertion
/// outputs of Forcer's d351 and d268, where Forcer's own `mergePropagators` keeps the other line: d131 and d68 instead
/// of our d114 and d69; all four are nodes of the 438 either way); 109 / 84 in the 5L DAGs.
pub fn normalise(t: &PTopo) -> Normalised {
    t.assert_valid();
    let mut w = Work::new(t);
    match w.run() {
        Outcome::Zero => Normalised::Zero,
        Outcome::Trivial => Normalised::Trivial,
        Outcome::Topo => {
            let topo = w.topo();
            let mut count = vec![0usize; w.lines.len()];
            for tr in &w.track {
                if let Track::Line(i) = *tr {
                    count[i] += 1;
                }
            }
            let fates = w
                .track
                .iter()
                .map(|tr| match *tr {
                    Track::Line(i) if count[i] == 1 => Fate::Kept(i as u8),
                    Track::Line(i) => Fate::SeriesMergedInto(i as u8),
                    Track::Absorbed => Fate::AbsorbedIntoQ,
                })
                .collect();
            Normalised::Topo(topo, fates)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Track {
    Line(usize),
    Absorbed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Zero,
    Trivial,
    Topo,
}

#[derive(Clone, Debug)]
struct Work {
    n: usize,
    s: usize,
    t: usize,
    lines: Vec<(usize, usize, u8)>,
    track: Vec<Track>,
}

impl Work {
    fn new(t: &PTopo) -> Self {
        Work {
            n: usize::from(t.n),
            s: usize::from(t.s),
            t: usize::from(t.t),
            lines: t
                .lines
                .iter()
                .map(|&(u, v, e)| (usize::from(u), usize::from(v), e))
                .collect(),
            track: (0..t.lines.len()).map(Track::Line).collect(),
        }
    }

    fn topo(&self) -> PTopo {
        PTopo {
            n: self.n as u8,
            s: self.s as u8,
            t: self.t as u8,
            lines: self
                .lines
                .iter()
                .map(|&(u, v, e)| (u as u8, v as u8, e))
                .collect(),
        }
    }

    /// Remove line `j`, identifying its endpoints (the higher vertex disappears). If `into` is `Some(i)`, line `i`
    /// receives `j`'s eps units and input lines tracked at `j`; otherwise they are absorbed into `q`.
    fn contract(&mut self, j: usize, into: Option<usize>) {
        let (a, b, e) = self.lines[j];
        debug_assert_ne!(a, b);
        if let Some(i) = into {
            self.lines[i].2 = self.lines[i]
                .2
                .checked_add(e)
                .expect("rustred-topology: eps units overflow u8");
        }
        for tr in &mut self.track {
            if *tr == Track::Line(j) {
                *tr = match into {
                    Some(i) => Track::Line(i),
                    None => Track::Absorbed,
                };
            }
        }
        self.lines.remove(j);
        for tr in &mut self.track {
            if let Track::Line(x) = tr
                && *x > j
            {
                *x -= 1;
            }
        }
        let (keep, gone) = (a.min(b), a.max(b));
        let relabel = |x: usize| {
            let x = if x == gone { keep } else { x };
            if x > gone { x - 1 } else { x }
        };
        for l in &mut self.lines {
            l.0 = relabel(l.0);
            l.1 = relabel(l.1);
        }
        self.s = relabel(self.s);
        self.t = relabel(self.t);
        self.n -= 1;
    }

    /// Drop vertices that are neither external nor on a line.
    fn compact(&mut self) {
        let mut used = vec![false; self.n];
        used[self.s] = true;
        used[self.t] = true;
        for &(u, v, _) in &self.lines {
            used[u] = true;
            used[v] = true;
        }
        if used.iter().all(|&x| x) {
            return;
        }
        let mut map = vec![usize::MAX; self.n];
        let mut next = 0;
        for (v, &u) in used.iter().enumerate() {
            if u {
                map[v] = next;
                next += 1;
            }
        }
        for l in &mut self.lines {
            l.0 = map[l.0];
            l.1 = map[l.1];
        }
        self.s = map[self.s];
        self.t = map[self.t];
        self.n = next;
    }

    fn absorb_all(&mut self) {
        self.track.fill(Track::Absorbed);
        self.lines.clear();
    }

    fn run(&mut self) -> Outcome {
        loop {
            if self.s == self.t || self.lines.iter().any(|l| l.0 == l.1) {
                return Outcome::Zero;
            }
            if self.lines.is_empty() {
                return Outcome::Trivial;
            }
            self.compact();
            let mut edges: Vec<(usize, usize)> = self.lines.iter().map(|l| (l.0, l.1)).collect();
            edges.push((self.s, self.t));
            if !biconnected(self.n, &edges) {
                return Outcome::Zero;
            }
            let masks = cycle_masks(self.n, &edges);
            let e0 = masks[self.lines.len()];
            // a line in series with the external line carries q alone
            if let Some(i) = (0..self.lines.len()).find(|&i| masks[i] == e0) {
                if self.lines.len() == 1 {
                    self.absorb_all();
                    return Outcome::Trivial;
                }
                self.contract(i, None);
                continue;
            }
            // two lines in series at an internal two-valent vertex
            if let Some((i, j)) = self.local_series_pair() {
                self.contract(j, Some(i));
                continue;
            }
            // lines with equal momentum elsewhere: non-local 2-edge cuts
            let mut classes: Vec<(u64, Vec<usize>)> = Vec::new();
            for (i, &m) in masks[..self.lines.len()].iter().enumerate() {
                match classes.iter_mut().find(|c| c.0 == m) {
                    Some(c) => c.1.push(i),
                    None => classes.push((m, vec![i])),
                }
            }
            if let Some((_, class)) = classes.into_iter().find(|c| c.1.len() > 1) {
                return self.merge_nonlocal(&class);
            }
            return Outcome::Topo;
        }
    }

    fn local_series_pair(&self) -> Option<(usize, usize)> {
        for w in 0..self.n {
            if w == self.s || w == self.t {
                continue;
            }
            let mut at = self
                .lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.0 == w || l.1 == w)
                .map(|(i, _)| i);
            if let (Some(i), Some(j), None) = (at.next(), at.next(), at.next()) {
                return Some((i, j));
            }
        }
        None
    }

    /// Keep each line of the series class in turn, merge the others into it, normalise further, and continue with
    /// the result of smallest key (ties: the lowest kept index).
    fn merge_nonlocal(&mut self, class: &[usize]) -> Outcome {
        NONLOCAL_MERGES.fetch_add(1, Ordering::Relaxed);
        let mut best: Option<(Vec<u8>, Work, Outcome)> = None;
        let mut ambiguous = false;
        for &keep in class {
            let mut w = self.clone();
            // contract the others from the highest index down so that lower indices stay valid
            let mut others: Vec<usize> = class.iter().copied().filter(|&j| j != keep).collect();
            others.sort_unstable_by(|a, b| b.cmp(a));
            let mut keep_now = keep;
            for j in others {
                w.contract(j, Some(keep_now));
                if j < keep_now {
                    keep_now -= 1;
                }
            }
            let outcome = w.run();
            let key = match outcome {
                Outcome::Zero => continue,
                Outcome::Trivial => canonical(&PTopo::trivial()).0,
                Outcome::Topo => canonical(&w.topo()).0,
            };
            let key = key.as_bytes().to_vec();
            if let Some(b) = &best
                && b.0 != key
            {
                ambiguous = true;
            }
            if best.as_ref().is_none_or(|b| key < b.0) {
                best = Some((key, w, outcome));
            }
        }
        if ambiguous {
            NONLOCAL_AMBIGUOUS.fetch_add(1, Ordering::Relaxed);
        }
        match best {
            Some((_, w, outcome)) => {
                *self = w;
                outcome
            }
            None => Outcome::Zero,
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
    fn scaleless_topologies_are_zero() {
        // tadpole (self-loop), S == T, and a bubble hanging off the S-T line (cut vertex)
        assert_eq!(
            normalise(&topo(2, 0, 1, &[(0, 1, 0), (1, 1, 0)])),
            Normalised::Zero
        );
        assert_eq!(
            normalise(&topo(2, 0, 0, &[(0, 1, 0), (0, 1, 0)])),
            Normalised::Zero
        );
        assert_eq!(
            normalise(&topo(
                3,
                0,
                1,
                &[(0, 1, 0), (0, 1, 0), (1, 2, 0), (1, 2, 0)]
            )),
            Normalised::Zero
        );
        // eps lines do not make a scaleless graph scaleful
        assert_eq!(
            normalise(&topo(
                3,
                0,
                1,
                &[(0, 1, 1), (0, 1, 0), (1, 2, 2), (1, 2, 0)]
            )),
            Normalised::Zero
        );
    }

    #[test]
    fn single_line_and_chain_are_trivial() {
        assert_eq!(normalise(&topo(2, 0, 1, &[(0, 1, 3)])), Normalised::Trivial);
        assert_eq!(
            normalise(&topo(4, 0, 3, &[(0, 1, 0), (1, 2, 1), (2, 3, 0)])),
            Normalised::Trivial
        );
        assert_eq!(normalise(&PTopo::trivial()), Normalised::Trivial);
    }

    #[test]
    fn bubble_with_legs_absorbs_the_legs() {
        // S -a- x =bubble= y -b- T: a and b carry q alone
        let t = topo(4, 0, 3, &[(0, 1, 1), (1, 2, 0), (1, 2, 2), (2, 3, 0)]);
        let Normalised::Topo(p, fates) = normalise(&t) else {
            panic!()
        };
        assert_eq!(p.lines.len(), 2);
        assert_eq!(p.loops(), 1);
        assert_eq!(fates[0], Fate::AbsorbedIntoQ);
        assert_eq!(fates[3], Fate::AbsorbedIntoQ);
        assert!(matches!(fates[1], Fate::Kept(_)) && matches!(fates[2], Fate::Kept(_)));
        let eps: Vec<u8> = p.lines.iter().map(|l| l.2).collect();
        assert_eq!(eps.iter().copied().max(), Some(2));
    }

    #[test]
    fn series_lines_merge_and_add_eps_units() {
        // 2L sunset-like: S-T via (S-x-T) and two direct lines; S-x and x-T are in series
        let t = topo(3, 0, 2, &[(0, 1, 1), (1, 2, 2), (0, 2, 0), (0, 2, 0)]);
        let Normalised::Topo(p, fates) = normalise(&t) else {
            panic!()
        };
        assert_eq!(p.lines.len(), 3);
        assert_eq!(fates[0], Fate::SeriesMergedInto(0));
        assert_eq!(fates[1], Fate::SeriesMergedInto(0));
        assert_eq!(p.lines[0].2, 3);
        assert_eq!(fates[2], Fate::Kept(1));
        assert_eq!(fates[3], Fate::Kept(2));
    }

    #[test]
    fn nonlocal_series_merge_is_labelling_independent() {
        // S=0, T=1, a=2 form a triangle; a chain S -l- y1 =bubble= y2 -m- a hangs between S and a, so l and m carry
        // the same momentum without meeting at a two-valent vertex. Keeping l or keeping m gives non-isomorphic graphs
        // (the bubble next to a or next to S); the minimal key decides, whatever the labelling.
        let base = [
            (0u8, 1u8, 0u8),
            (0, 2, 0),
            (2, 1, 0),
            (0, 3, 1),
            (3, 4, 0),
            (3, 4, 0),
            (4, 2, 0),
        ];
        let key_of = |t: &PTopo| match normalise(t) {
            Normalised::Topo(p, fates) => {
                assert_eq!(p.lines.len(), 6);
                assert_eq!(
                    fates
                        .iter()
                        .filter(|f| matches!(f, Fate::SeriesMergedInto(_)))
                        .count(),
                    2
                );
                canonical(&p).0
            }
            other => panic!("unexpected {other:?}"),
        };
        let before = nonlocal_merge_events();
        let reference = key_of(&topo(5, 0, 1, &base));
        for perm in [[4u8, 3, 2, 1, 0], [1, 0, 3, 4, 2], [2, 4, 0, 3, 1]] {
            let lines: Vec<(u8, u8, u8)> = base
                .iter()
                .rev()
                .map(|&(u, v, e)| (perm[usize::from(v)], perm[usize::from(u)], e))
                .collect();
            assert_eq!(key_of(&topo(5, perm[0], perm[1], &lines)), reference);
        }
        let after = nonlocal_merge_events();
        assert!(after.0 >= before.0 + 4 && after.1 >= before.1 + 4);
        // the two candidates differ: the merged eps line sits between S and the bubble in the chosen one or not
        let keep_l = canonical(&topo(
            4,
            0,
            1,
            &[
                (0, 1, 0),
                (0, 2, 0),
                (2, 1, 0),
                (0, 3, 1),
                (3, 2, 0),
                (3, 2, 0),
            ],
        ))
        .0;
        let keep_m = canonical(&topo(
            4,
            0,
            1,
            &[
                (0, 1, 0),
                (0, 2, 0),
                (2, 1, 0),
                (3, 2, 1),
                (0, 3, 0),
                (0, 3, 0),
            ],
        ))
        .0;
        assert_ne!(keep_l, keep_m);
        assert_eq!(reference, keep_l.min(keep_m));
    }
}
