//! Eps-coloured canonical keys through graphica's `canonize()`.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use graphica::Graph;
use smallvec::SmallVec;

use crate::{P_TOPOLOGY_VERSION, PTopo, find};

/// Identity of a DAG node: the canonical bytes of a normalised topology. Compared and hashed as bytes; never derived
/// from graphica's struct layout.
///
/// Encoding (version [`P_TOPOLOGY_VERSION`] = 1): `[version, n, colour(0) .. colour(n-1), then for every adjacent
/// vertex pair in increasing canonical (u, v): u, v, m, e_1 .. e_m]` with colour 1 for the two external vertices and
/// 0 otherwise, `m` = number of parallel lines on the pair and `e_1 <= .. <= e_m` their eps units. Integer powers,
/// numerators and eps units on `q` are not in the key.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GraphKey(pub Arc<[u8]>);

impl GraphKey {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Lower-case hex of the key bytes.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Inverse of [`GraphKey::to_hex`].
    pub fn from_hex(hex: &str) -> Option<Self> {
        if !hex.len().is_multiple_of(2) {
            return None;
        }
        let bytes: Option<Vec<u8>> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
            .collect();
        bytes.map(|b| GraphKey(b.into()))
    }
}

impl fmt::Debug for GraphKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GraphKey({})", self.to_hex())
    }
}

/// Canonical labelling of a topology (see [`canonical`]).
///
/// - `vertex_map[v]`: canonical vertex of input vertex `v`;
/// - `line_map[i]`: canonical line of input line `i` (canonical lines are sorted by `(u, v, eps)` with `u < v`;
///   parallel lines with equal eps keep their input order);
/// - `line_orbits[c]`: the smallest canonical line in the orbit of canonical line `c` under the automorphisms of the
///   topology (vertex automorphisms fixing the external pair as a set, and permutations of parallel lines with equal
///   eps);
/// - `aut_gens`: generators of that group as permutations of canonical lines (`g[c]` = image of line `c`): the line
///   actions of graphica's vertex-automorphism generators that move a line, then the transpositions of neighbouring
///   parallel lines with equal eps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canon {
    pub vertex_map: Vec<u8>,
    pub line_map: Vec<u8>,
    pub line_orbits: Vec<u8>,
    pub aut_gens: Vec<Vec<u8>>,
}

impl Canon {
    /// The canonical labelling of `t` (the topology this `Canon` was computed from): line `i` becomes line
    /// `line_map[i]`, vertex `v` becomes `vertex_map[v]`, and `s < t`.
    pub fn apply(&self, t: &PTopo) -> PTopo {
        let vm = &self.vertex_map;
        let mut lines: SmallVec<[(u8, u8, u8); 20]> = SmallVec::from_elem((0, 0, 0), t.lines.len());
        for (i, &(u, v, e)) in t.lines.iter().enumerate() {
            let (a, b) = (vm[usize::from(u)], vm[usize::from(v)]);
            lines[usize::from(self.line_map[i])] = (a.min(b), a.max(b), e);
        }
        let (s, tt) = (vm[usize::from(t.s)], vm[usize::from(t.t)]);
        PTopo {
            n: t.n,
            s: s.min(tt),
            t: s.max(tt),
            lines,
        }
    }
}

/// Edge colour of a vertex pair: the sorted eps units of its parallel lines, one byte each (eps + 1, so that an empty
/// slot is 0), at most eight lines per pair (a pair with k parallel lines carries k - 1 loops).
fn pack(eps: &[u8]) -> u64 {
    assert!(
        eps.len() <= 8,
        "rustred-topology: more than 8 parallel lines between two vertices"
    );
    eps.iter().enumerate().fold(0u64, |acc, (k, &e)| {
        assert!(e < u8::MAX, "rustred-topology: eps units {e} out of range");
        acc | (u64::from(e + 1) << (8 * k))
    })
}

/// Canonical key and labelling of a topology: graphica `canonize()` of the SIMPLE graph on the vertices of `t` with
/// colour 1 for `s` and `t` (one colour: `q -> -q` is a symmetry) and 0 for the others, one edge per adjacent vertex
/// pair coloured by the sorted multiset of eps units of its parallel lines. Meant for normalised topologies (the key
/// of a DAG node); any valid [`PTopo`] is accepted.
pub fn canonical(t: &PTopo) -> (GraphKey, Canon) {
    t.assert_valid();
    let n = usize::from(t.n);
    let colour = |v: usize| u8::from(v == usize::from(t.s) || v == usize::from(t.t));

    let mut pairs: BTreeMap<(usize, usize), SmallVec<[u8; 8]>> = BTreeMap::new();
    for &(u, v, e) in &t.lines {
        let (u, v) = (usize::from(u), usize::from(v));
        pairs.entry((u.min(v), u.max(v))).or_default().push(e);
    }
    for eps in pairs.values_mut() {
        eps.sort_unstable();
    }

    let mut g: Graph<u8, u64> = Graph::new();
    for v in 0..n {
        g.add_node(colour(v));
    }
    for (&(u, v), eps) in &pairs {
        g.add_edge(u, v, false, pack(eps)).expect("vertices exist");
    }
    let form = g.canonize();
    let vm: Vec<u8> = form.vertex_map.iter().map(|&x| x as u8).collect();

    // key bytes
    let mut bytes = Vec::with_capacity(2 + n + 3 * pairs.len() + t.lines.len());
    bytes.push(P_TOPOLOGY_VERSION as u8);
    bytes.push(t.n);
    let mut colours = vec![0u8; n];
    for v in 0..n {
        colours[usize::from(vm[v])] = colour(v);
    }
    bytes.extend_from_slice(&colours);
    let mut cpairs: Vec<(u8, u8, &SmallVec<[u8; 8]>)> = pairs
        .iter()
        .map(|(&(u, v), eps)| {
            let (a, b) = (vm[u], vm[v]);
            (a.min(b), a.max(b), eps)
        })
        .collect();
    cpairs.sort_unstable_by_key(|&(a, b, _)| (a, b));
    for (a, b, eps) in cpairs {
        bytes.extend_from_slice(&[a, b, eps.len() as u8]);
        bytes.extend_from_slice(eps);
    }

    // canonical lines
    let mut order: Vec<(u8, u8, u8, usize)> = t
        .lines
        .iter()
        .enumerate()
        .map(|(i, &(u, v, e))| {
            let (a, b) = (vm[usize::from(u)], vm[usize::from(v)]);
            (a.min(b), a.max(b), e, i)
        })
        .collect();
    order.sort_unstable();
    let mut line_map = vec![0u8; t.lines.len()];
    for (c, &(_, _, _, i)) in order.iter().enumerate() {
        line_map[i] = c as u8;
    }
    let clines: Vec<(u8, u8, u8)> = order.iter().map(|&(a, b, e, _)| (a, b, e)).collect();

    let (line_orbits, aut_gens) = line_symmetry(n, &clines, &form.orbit_generators);
    (
        GraphKey(bytes.into()),
        Canon {
            vertex_map: vm,
            line_map,
            line_orbits,
            aut_gens,
        },
    )
}

/// Line orbits and line-permutation generators from graphica's vertex generators (canonical numbering, given as
/// lists of cycles) plus the transpositions of identical parallel lines.
fn line_symmetry(
    n: usize,
    clines: &[(u8, u8, u8)],
    vertex_gens: &[Vec<Vec<usize>>],
) -> (Vec<u8>, Vec<Vec<u8>>) {
    let e = clines.len();
    // runs of identical canonical lines: (u, v, eps) -> (first index, length)
    let mut runs: BTreeMap<(u8, u8, u8), (usize, usize)> = BTreeMap::new();
    for (c, &l) in clines.iter().enumerate() {
        runs.entry(l).and_modify(|r| r.1 += 1).or_insert((c, 1));
    }
    let mut gens: Vec<Vec<u8>> = Vec::new();
    for cycles in vertex_gens {
        let mut p: Vec<usize> = (0..n).collect();
        for cycle in cycles {
            for (k, &x) in cycle.iter().enumerate() {
                p[x] = cycle[(k + 1) % cycle.len()];
            }
        }
        let mut g = vec![0u8; e];
        for (c, &(u, v, eps)) in clines.iter().enumerate() {
            let (start, _) = runs[&(u, v, eps)];
            let (a, b) = (p[usize::from(u)] as u8, p[usize::from(v)] as u8);
            let (img_start, img_len) = *runs
                .get(&(a.min(b), a.max(b), eps))
                .expect("graphica generator is an automorphism of the canonical graph");
            let k = c - start;
            assert!(
                k < img_len,
                "graphica generator is an automorphism of the canonical graph"
            );
            g[c] = (img_start + k) as u8;
        }
        if g.iter().enumerate().any(|(c, &x)| usize::from(x) != c) && !gens.contains(&g) {
            gens.push(g);
        }
    }
    for &(start, len) in runs.values() {
        for k in start..start + len - 1 {
            let mut g: Vec<u8> = (0..e as u8).collect();
            g.swap(k, k + 1);
            gens.push(g);
        }
    }
    let mut parent: Vec<usize> = (0..e).collect();
    for g in &gens {
        for (c, &x) in g.iter().enumerate() {
            let (a, b) = (find(&mut parent, c), find(&mut parent, usize::from(x)));
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        }
    }
    let orbits: Vec<u8> = (0..e).map(|c| find(&mut parent, c) as u8).collect();
    // the root of a union-find class is its minimal element by construction (min-rooted unions)
    (orbits, gens)
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
    fn keys_are_labelling_independent() {
        // 2L kite with an eps unit on one rung, two different labellings of the same graph
        let a = topo(
            4,
            0,
            3,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 1), (1, 3, 0), (2, 3, 0)],
        );
        let b = topo(
            4,
            2,
            1,
            &[(2, 0, 0), (2, 3, 0), (0, 3, 1), (0, 1, 0), (3, 1, 0)],
        );
        assert_eq!(canonical(&a).0, canonical(&b).0);
        // S and T swapped is the same key (q -> -q)
        let c = topo(
            4,
            3,
            0,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 1), (1, 3, 0), (2, 3, 0)],
        );
        assert_eq!(canonical(&a).0, canonical(&c).0);
    }

    #[test]
    fn eps_colours_distinguish_keys() {
        let int = topo(
            4,
            0,
            3,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 0), (1, 3, 0), (2, 3, 0)],
        );
        let rung = topo(
            4,
            0,
            3,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 1), (1, 3, 0), (2, 3, 0)],
        );
        let side = topo(
            4,
            0,
            3,
            &[(0, 1, 1), (0, 2, 0), (1, 2, 0), (1, 3, 0), (2, 3, 0)],
        );
        let side_other = topo(
            4,
            0,
            3,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 0), (1, 3, 0), (2, 3, 1)],
        );
        let k = |t: &PTopo| canonical(t).0;
        assert_ne!(k(&int), k(&rung));
        assert_ne!(k(&int), k(&side));
        assert_ne!(k(&rung), k(&side));
        assert_eq!(k(&side), k(&side_other)); // automorphic positions
    }

    #[test]
    fn canonical_labelling_reproduces_the_key() {
        let t = topo(
            5,
            4,
            0,
            &[
                (4, 1, 0),
                (4, 2, 0),
                (1, 2, 0),
                (1, 3, 2),
                (2, 3, 0),
                (3, 0, 0),
                (3, 0, 1),
            ],
        );
        let (key, canon) = canonical(&t);
        let c = canon.apply(&t);
        assert!(c.s < c.t);
        assert!(c.lines.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(canonical(&c).0, key);
        // relabelled input: identical canonical topology
        let perm = [2u8, 4, 0, 1, 3];
        let r = topo(
            5,
            perm[4],
            perm[0],
            &t.lines
                .iter()
                .rev()
                .map(|&(u, v, e)| (perm[usize::from(v)], perm[usize::from(u)], e))
                .collect::<Vec<_>>(),
        );
        let (key_r, canon_r) = canonical(&r);
        assert_eq!(key_r, key);
        assert_eq!(canon_r.apply(&r), c);
    }

    #[test]
    fn line_orbits_of_the_kite_and_the_sunset() {
        // integer kite: S=0, T=3; four side lines in one orbit, the rung alone
        let kite = topo(
            4,
            0,
            3,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 0), (1, 3, 0), (2, 3, 0)],
        );
        let (_, canon) = canonical(&kite);
        let rung = usize::from(canon.line_map[2]);
        let side_orbit = canon.line_orbits[usize::from(canon.line_map[0])];
        for i in [0, 1, 3, 4] {
            assert_eq!(
                canon.line_orbits[usize::from(canon.line_map[i])],
                side_orbit
            );
        }
        assert_eq!(usize::from(canon.line_orbits[rung]), rung);
        // sunset with one eps line: the two integer lines form an orbit
        let sunset = topo(2, 0, 1, &[(0, 1, 0), (0, 1, 1), (0, 1, 0)]);
        let (_, canon) = canonical(&sunset);
        assert_eq!(canon.line_orbits, vec![0, 0, 2]);
        assert_eq!(canon.aut_gens, vec![vec![1, 0, 2]]);
    }

    #[test]
    fn hex_round_trip() {
        let (key, _) = canonical(&PTopo::trivial());
        assert_eq!(key.as_bytes(), &[1, 2, 1, 1]);
        assert_eq!(GraphKey::from_hex(&key.to_hex()), Some(key));
        assert_eq!(GraphKey::from_hex("0"), None);
        assert_eq!(GraphKey::from_hex("zz"), None);
    }
}
