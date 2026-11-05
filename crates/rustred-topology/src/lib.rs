//! Massless propagator topologies for RustRed's p-integral library.
//!
//! A [`PTopo`] is the graph of a massless two-point (propagator) integral: vertices `0..n`, two distinct external
//! vertices `s` and `t` where the momentum `q` enters and leaves, and internal lines `(u, v, eps)` whose power is
//! `a + eps * epsilon` with an integer `a` that is not part of the topology (`eps = 0`: integer line; `eps > 0`: an
//! "eps line", drawn with crosses in Forcer's papers). Integer powers, numerators and an eps power on `q` itself are
//! never part of a topology: at `Q^2 = 1` the factor `(Q^2)^(-j eps)` is one.
//!
//! The crate provides, following Forcer's reduction tree (B. Ruijl, T. Ueda, J. Vermaseren, `benruijl/forcer`):
//! - [`normalise`]: Forcer's `mergePropagators` on graphs. Zero when `s == t`, a self-loop exists or the glued graph
//!   `Gamma + e0` (`e0` = the external line `s`-`t`) is not 2-connected; lines carrying `q` alone are absorbed into
//!   `q`; lines with equal momentum (a 2-edge cut of the glued graph) merge into one line, their eps units adding;
//!   no line left = [`Normalised::Trivial`]. Every input line gets a [`Fate`].
//! - [`canonical`]: the eps-coloured canonical key [`GraphKey`] (graphica `canonize()` of the simple graph with `s`
//!   and `t` in one colour and edge colour = the sorted multiset of eps units of the parallel lines), and the
//!   canonical labelling [`Canon`].
//! - [`forcer_action`]: Forcer's rule detection in Forcer's order Insertion > Carpet > Triangle > Diamond > Custom,
//!   with frozen, versioned choice rules; [`is_series_parallel`].
//! - [`TopologyService::reduction_graph`]: the reduction DAG ([`ChildMode::ForcerTree`] = production and golden,
//!   [`ChildMode::Demand`] = diagnostic) built through a cache of per-key analyses; [`topologies_json`].
//!
//! Everything is deterministic: no output depends on hash-map iteration order. Any change of the key encoding, of
//! [`normalise`], of a matcher or of a choice rule bumps [`P_TOPOLOGY_VERSION`] (the key bytes start with it).

use smallvec::SmallVec;

mod canon;
mod dag;
mod graph;
mod matchers;
mod normalise;

pub use canon::{Canon, GraphKey, canonical};
pub use dag::{
    ChildMode, Dag, DagEdge, DagEdgeKind, NodeInfo, TopologyService, reduction_graph,
    topologies_json,
};
pub use matchers::{
    Action, ActionKind, CarpetMatch, DiaMatch, Ins, TriMatch, forcer_action, is_series_parallel,
};
#[doc(hidden)]
pub use normalise::nonlocal_merge_events;
pub use normalise::{Fate, Normalised, normalise};

/// Version of everything that decides a [`GraphKey`] or a DAG: key encoding, [`normalise`], the matchers and their
/// choice rules. The first byte of every key.
pub const P_TOPOLOGY_VERSION: u32 = 1;
const _: () = assert!(
    P_TOPOLOGY_VERSION < 256,
    "the key stores the version in one byte"
);

/// Schema name of [`topologies_json`].
pub const P_TOPOLOGIES_SCHEMA: &str = "rustred.p-topologies.v1";

/// Eps units `j` of a line with power `a + j eps`.
pub type EpsUnits = u8;

/// A propagator topology: vertices `0..n`, external vertices `s != t` (where `q` enters and leaves), internal lines
/// `(u, v, eps units)`. Inputs may be arbitrary (unnormalised, any labelling); [`NodeInfo::topo`] is normalised and
/// canonically labelled (`u < v`, lines sorted by `(u, v, eps)`, `s < t`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PTopo {
    pub n: u8,
    pub s: u8,
    pub t: u8,
    pub lines: SmallVec<[(u8, u8, EpsUnits); 20]>,
}

impl PTopo {
    /// A topology from its parts; panics if an endpoint, `s` or `t` is not a vertex (`< n`).
    pub fn new(n: u8, s: u8, t: u8, lines: impl IntoIterator<Item = (u8, u8, EpsUnits)>) -> Self {
        let topo = PTopo {
            n,
            s,
            t,
            lines: lines.into_iter().collect(),
        };
        topo.assert_valid();
        topo
    }

    /// The trivial topology (Forcer's `d1`): two external vertices, no internal line.
    pub fn trivial() -> Self {
        PTopo {
            n: 2,
            s: 0,
            t: 1,
            lines: SmallVec::new(),
        }
    }

    /// Number of internal lines.
    pub fn line_count(&self) -> u8 {
        u8::try_from(self.lines.len()).expect("PTopo: more than 255 lines")
    }

    /// Number of independent loops of the internal lines (`E - V + components` over the vertices touched by a line;
    /// the external line is not counted).
    pub fn loops(&self) -> u8 {
        let n = usize::from(self.n);
        let mut parent: Vec<usize> = (0..n).collect();
        let mut touched = vec![false; n];
        let mut components = 0usize;
        for &(u, v, _) in &self.lines {
            let (u, v) = (usize::from(u), usize::from(v));
            for x in [u, v] {
                if !touched[x] {
                    touched[x] = true;
                    components += 1;
                }
            }
            let (a, b) = (find(&mut parent, u), find(&mut parent, v));
            if a != b {
                parent[a] = b;
                components -= 1;
            }
        }
        let vertices = touched.iter().filter(|&&x| x).count();
        u8::try_from(self.lines.len() + components - vertices).expect("PTopo: more than 255 loops")
    }

    /// Number of lines with a non-zero eps power.
    pub fn eps_lines(&self) -> u8 {
        u8::try_from(self.lines.iter().filter(|l| l.2 > 0).count())
            .expect("PTopo: more than 255 lines")
    }

    pub(crate) fn assert_valid(&self) {
        assert!(
            self.s < self.n && self.t < self.n,
            "PTopo: external vertex out of range (n = {}, s = {}, t = {})",
            self.n,
            self.s,
            self.t
        );
        for &(u, v, _) in &self.lines {
            assert!(
                u < self.n && v < self.n,
                "PTopo: line ({u}, {v}) has an endpoint outside 0..{}",
                self.n
            );
        }
    }
}

pub(crate) fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}
