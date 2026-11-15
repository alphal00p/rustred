//! THE PIECE PLANNER (TRICKS item 4): one chosen service per unserved key of a node (all open faces together), the
//! three pieces per face, the order in which pieces must be solved, and the keys left to the IBP core.
//!
//! Faces are coupled: the face graph can be cyclic while the key graph is acyclic (a face hands keys down into another
//! face's core and that face hands into the first one's core, or a face hands into its OWN core), so viability, order
//! and acyclicity are decided per KEY, never per face:
//!
//! 1. a candidate is viable iff every edge term (non-zero or unknown) strictly descends under the node's full order,
//!    and a hand-down has exactly one edge term;
//! 2. the chosen service is the best viable candidate under the caller's ranking (kind order, then fewer edge terms,
//!    then provenance and terms: total and deterministic);
//! 3. keys without a viable candidate are LEFT TO THE IBP CORE;
//! 4. pieces per face: (i) `rules` (composite, two_row, table_rule), (ii) `hand_downs`, (iii) `core` (core_recipe
//!    chosen + left open); piece order = Tarjan's completion order on the piece graph (dependencies first: a
//!    hand-down's target piece before the hand-down); a cyclic piece SCC is one group solved in key order;
//! 5. BOTH checks: the descent of every chosen edge AND Kahn over the plan's key graph; a cycle in the key graph is
//!    `PlanError::Inconsistent` (the order is not a strict order on these keys: comparator or frame bug).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::graph::{MemGraph, cycle_through, kahn, sccs, witness_cycle};
use super::order::{EdgeOrder, KeyOrder, edge_order};
use super::{NonZero, Term};

pub const PLAN_SCHEMA: &str = "rustred.campaign.pieces-plan.v1";

/// What a candidate service is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceKind {
    /// Symmetry element + catalogue rule(s) at the image term(s), composed (never the alias edge K -> gK).
    Composite,
    /// A closed form from two rows of one element (at K and a shifted K').
    TwoRow,
    /// A one-term image strictly lower (onto another open face, or into the face's own core).
    HandDown,
    /// A recipe (IBP) relation solved at the key.
    CoreRecipe,
    /// A catalogue rule applied at the key.
    TableRule,
}

impl ServiceKind {
    pub fn piece(self) -> PieceKind {
        match self {
            ServiceKind::Composite | ServiceKind::TwoRow | ServiceKind::TableRule => {
                PieceKind::Rules
            }
            ServiceKind::HandDown => PieceKind::HandDowns,
            ServiceKind::CoreRecipe => PieceKind::Core,
        }
    }
}

/// The three pieces of a face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PieceKind {
    /// (i) conditioned composite rules (and other rules).
    Rules,
    /// (ii) hand-downs.
    HandDowns,
    /// (iii) the IBP core: solved by a recipe, or left open.
    Core,
}

/// One candidate service of a key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub kind: ServiceKind,
    /// The right-hand side with exact non-zero-ness (zero terms are kept for the record but are not edges).
    pub terms: Vec<Term>,
    pub provenance: String,
    /// Regular at d = 4 at the key (`None`: not tested). With [`Ranking::singular_last`] a candidate known to be
    /// singular is chosen only when no other viable candidate exists (TRICKS 9: regular wins by default).
    #[serde(default)]
    pub regular_at_4: Option<bool>,
}

impl Candidate {
    fn edge_terms(&self) -> impl Iterator<Item = &Term> {
        self.terms.iter().filter(|t| t.nonzero != NonZero::Zero)
    }
    fn edge_count(&self) -> usize {
        self.edge_terms().count()
    }
}

/// An unserved key with its face label and candidates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnservedKey {
    pub key: Vec<i32>,
    /// Opaque face label (e.g. the support signature); keys of one face share it.
    pub face: String,
    pub candidates: Vec<Candidate>,
}

/// Preference among the viable candidates of a key: candidates known singular at d = 4 last (if `singular_last`),
/// then the kinds in order (a kind not listed is never chosen), then fewer edge terms, then provenance and terms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ranking {
    pub kinds: Vec<ServiceKind>,
    #[serde(default = "yes")]
    pub singular_last: bool,
}

fn yes() -> bool {
    true
}

impl Default for Ranking {
    fn default() -> Self {
        Ranking {
            kinds: vec![
                ServiceKind::Composite,
                ServiceKind::TwoRow,
                ServiceKind::HandDown,
                ServiceKind::CoreRecipe,
                ServiceKind::TableRule,
            ],
            singular_last: true,
        }
    }
}

impl Ranking {
    fn rank(&self, k: ServiceKind) -> Option<usize> {
        self.kinds.iter().position(|&x| x == k)
    }
}

/// The planner's input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanInput {
    pub arity: usize,
    pub keys: Vec<UnservedKey>,
    pub ranking: Ranking,
    /// Witness cycles listed for the candidate graph (cyclic SCCs, smallest member first).
    pub max_candidate_cycles: usize,
}

impl PlanInput {
    pub fn new(arity: usize, keys: Vec<UnservedKey>) -> PlanInput {
        PlanInput {
            arity,
            keys,
            ranking: Ranking::default(),
            max_candidate_cycles: 20,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalReason {
    /// An edge term is higher than the key.
    NonDescending,
    /// An edge term is equal to the key under the order (an alias at equal rank, or the key itself).
    Equal,
    /// A hand-down with other than exactly one edge term.
    HandDownNotOneTerm,
    /// The kind is not in the ranking.
    KindNotRanked,
}

/// A refused candidate.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Refusal {
    pub key: Vec<i32>,
    /// Index in the key's CANONICAL candidate order (kind rank, edge terms, provenance, terms).
    pub candidate: usize,
    pub kind: ServiceKind,
    pub provenance: String,
    pub reason: RefusalReason,
    /// The first offending term.
    pub term: Option<Vec<i32>>,
}

/// Where a chosen hand-down lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandDownTarget {
    /// An unserved key of another face.
    OtherFace,
    /// The core of the key's own face.
    OwnCore,
    /// Another piece of the key's own face.
    OwnFaceOther,
    /// A key outside the unserved set (served by existing material).
    Served,
}

/// The chosen service of a key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub key: Vec<i32>,
    pub face: String,
    pub piece: PieceKind,
    /// `None`: no viable candidate, left to the IBP core.
    pub kind: Option<ServiceKind>,
    pub candidate: Option<usize>,
    pub provenance: Option<String>,
    pub edge_terms: usize,
    pub regular_at_4: Option<bool>,
    /// Edge terms that are themselves unserved keys (plan-internal dependencies).
    pub internal_edges: usize,
    pub hand_down_target: Option<HandDownTarget>,
}

/// A piece: (face, kind).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PieceId {
    pub face: String,
    pub kind: PieceKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    pub id: PieceId,
    pub keys: u64,
    /// Of a core piece: keys solved by a chosen recipe / left open.
    pub core_solved: u64,
    pub core_open: u64,
    /// Pieces this one depends on (a chosen edge from one of its keys into theirs), with edge counts.
    pub depends_on: BTreeMap<String, u64>,
}

impl PieceId {
    pub fn label(&self) -> String {
        let k = match self.kind {
            PieceKind::Rules => "i:rules",
            PieceKind::HandDowns => "ii:hand_downs",
            PieceKind::Core => "iii:core",
        };
        format!("{}#{k}", self.face)
    }
}

/// One edge of a reported cycle, with its relation to the order and whether the plan uses it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanCycleEdge {
    pub from: Vec<i32>,
    pub to: Vec<i32>,
    pub order: EdgeOrder,
    pub in_plan: bool,
}

/// The face-level view next to the key- and piece-level verdicts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceReport {
    /// face -> face -> chosen edges (self entry: into the face itself).
    pub face_graph: BTreeMap<String, BTreeMap<String, u64>>,
    /// Cyclic face SCCs (size > 1 or a self loop), sorted labels.
    pub face_cycles: Vec<Vec<String>>,
    pub face_level_cyclic: bool,
    pub piece_level_acyclic: bool,
    pub key_level_acyclic: bool,
    pub verdict: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub schema: String,
    pub order: String,
    pub keys: u64,
    /// One entry per unserved key, sorted by key.
    pub chosen: Vec<Choice>,
    pub refused: Vec<Refusal>,
    /// Keys left to the IBP core (no viable candidate), sorted.
    pub core_open: Vec<Vec<i32>>,
    pub pieces: Vec<Piece>,
    /// Groups of pieces in solving order (dependencies first); a group of more than one piece is a cyclic piece SCC
    /// (solve its keys in `key_order`).
    pub piece_order: Vec<Vec<String>>,
    /// Unserved keys in solving order (dependencies first).
    pub key_order: Vec<Vec<i32>>,
    /// Chosen edges: total, into unserved keys, into served leaves.
    pub edges: [u64; 3],
    pub face_report: FaceReport,
    /// Cycles among ALL candidates' edges (before choice): "no plan through it".
    pub candidate_cycles: Vec<Vec<PlanCycleEdge>>,
    pub candidate_cyclic_sccs: u64,
    pub counts_by_kind: BTreeMap<String, u64>,
    pub hand_down_targets: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    Arity {
        expected: usize,
        key: Vec<i32>,
    },
    /// The same key given with two face labels.
    FaceConflict {
        key: Vec<i32>,
        faces: [String; 2],
    },
    /// Every chosen edge descends, yet the plan's key graph has a cycle: the order is not a strict order on these keys.
    Inconsistent {
        cycle: Vec<Vec<i32>>,
    },
    Graph(String),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::Arity { expected, key } => {
                write!(
                    f,
                    "pieces plan: key {key:?} has arity {} (expected {expected})",
                    key.len()
                )
            }
            PlanError::FaceConflict { key, faces } => {
                write!(
                    f,
                    "pieces plan: key {key:?} given on faces {} and {}",
                    faces[0], faces[1]
                )
            }
            PlanError::Inconsistent { cycle } => write!(
                f,
                "pieces plan: INCONSISTENT: every chosen edge descends under the order but the key graph has the cycle \
                 {cycle:?} (comparator or frame bug)"
            ),
            PlanError::Graph(e) => write!(f, "pieces plan: graph: {e}"),
        }
    }
}

impl std::error::Error for PlanError {}

/// A key with its face and canonical candidates.
type KeyEntry<'a> = (&'a Vec<i32>, &'a (String, Vec<Candidate>));

fn gerr(e: std::io::Error) -> PlanError {
    PlanError::Graph(e.to_string())
}

/// Plans the pieces. Deterministic: the result does not depend on the order of keys or candidates in the input.
pub fn plan<O: KeyOrder + ?Sized>(order: &O, input: &PlanInput) -> Result<Plan, PlanError> {
    let a = input.arity;
    let chk = |k: &[i32]| -> Result<(), PlanError> {
        if k.len() != a {
            return Err(PlanError::Arity {
                expected: a,
                key: k.to_vec(),
            });
        }
        Ok(())
    };
    // canonical input: keys sorted, candidates merged and sorted
    let mut by_key: BTreeMap<Vec<i32>, (String, Vec<Candidate>)> = BTreeMap::new();
    for u in &input.keys {
        chk(&u.key)?;
        for c in &u.candidates {
            for t in &c.terms {
                chk(&t.key)?;
            }
        }
        let e = by_key
            .entry(u.key.clone())
            .or_insert_with(|| (u.face.clone(), vec![]));
        if e.0 != u.face {
            return Err(PlanError::FaceConflict {
                key: u.key.clone(),
                faces: [e.0.clone(), u.face.clone()],
            });
        }
        e.1.extend(u.candidates.iter().cloned());
    }
    let rank_key = |c: &Candidate| {
        (
            input.ranking.singular_last && c.regular_at_4 == Some(false),
            input.ranking.rank(c.kind).unwrap_or(usize::MAX),
            c.kind,
            c.edge_count(),
            c.provenance.clone(),
            c.terms
                .iter()
                .map(|t| (t.key.clone(), t.nonzero))
                .collect::<Vec<_>>(),
        )
    };
    for (_, cs) in by_key.values_mut() {
        cs.sort_by_cached_key(rank_key);
        cs.dedup();
    }
    let keys: Vec<Vec<i32>> = by_key.keys().cloned().collect();
    let id_of: BTreeMap<&[i32], u32> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_slice(), i as u32))
        .collect();
    let n = keys.len();

    // 1-3: choice per key
    let mut refused = Vec::new();
    let mut pick: Vec<Option<usize>> = vec![None; n];
    let mut cand_edges: Vec<(u32, u32)> = Vec::new();
    for (i, (k, (_, cs))) in by_key.iter().enumerate() {
        for (ci, c) in cs.iter().enumerate() {
            for t in c.edge_terms() {
                if let Some(&j) = id_of.get(t.key.as_slice()) {
                    cand_edges.push((i as u32, j));
                }
            }
            if pick[i].is_some() {
                continue;
            }
            let refuse = |reason, term: Option<Vec<i32>>| Refusal {
                key: k.clone(),
                candidate: ci,
                kind: c.kind,
                provenance: c.provenance.clone(),
                reason,
                term,
            };
            if input.ranking.rank(c.kind).is_none() {
                refused.push(refuse(RefusalReason::KindNotRanked, None));
                continue;
            }
            let bad = c
                .edge_terms()
                .map(|t| (edge_order(order, k, &t.key), t))
                .find(|(o, _)| *o != EdgeOrder::Lower);
            if let Some((o, t)) = bad {
                let r = if o == EdgeOrder::Equal {
                    RefusalReason::Equal
                } else {
                    RefusalReason::NonDescending
                };
                refused.push(refuse(r, Some(t.key.clone())));
                continue;
            }
            if c.kind == ServiceKind::HandDown && c.edge_count() != 1 {
                refused.push(refuse(RefusalReason::HandDownNotOneTerm, None));
                continue;
            }
            pick[i] = Some(ci);
        }
    }
    refused.sort();

    let entries: Vec<KeyEntry<'_>> = by_key.iter().collect();
    let pieces_k: Vec<PieceKind> = (0..n)
        .map(|i| match pick[i] {
            Some(ci) => entries[i].1.1[ci].kind.piece(),
            None => PieceKind::Core,
        })
        .collect();

    // plan graph (key -> unserved rhs keys of its chosen service), both checks
    let mut plan_edges: Vec<(u32, u32)> = Vec::new();
    let mut edges = [0u64; 3];
    let mut chosen = Vec::with_capacity(n);
    let mut hd_targets: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_kind: BTreeMap<String, u64> = BTreeMap::new();
    for i in 0..n {
        let (k, (face, cs)) = entries[i];
        let mut ch = Choice {
            key: k.clone(),
            face: face.clone(),
            piece: pieces_k[i],
            kind: None,
            candidate: None,
            provenance: None,
            edge_terms: 0,
            regular_at_4: None,
            internal_edges: 0,
            hand_down_target: None,
        };
        if let Some(ci) = pick[i] {
            let c = &cs[ci];
            ch.kind = Some(c.kind);
            ch.candidate = Some(ci);
            ch.provenance = Some(c.provenance.clone());
            ch.regular_at_4 = c.regular_at_4;
            for t in c.edge_terms() {
                // the descent check, again, on the CHOSEN edge (the plan's own guard; viability already demanded it)
                if edge_order(order, k, &t.key) != EdgeOrder::Lower {
                    return Err(PlanError::Graph(format!(
                        "chosen edge {k:?} -> {:?} does not descend (internal error)",
                        t.key
                    )));
                }
                ch.edge_terms += 1;
                edges[0] += 1;
                match id_of.get(t.key.as_slice()) {
                    Some(&j) => {
                        ch.internal_edges += 1;
                        edges[1] += 1;
                        plan_edges.push((i as u32, j));
                    }
                    None => edges[2] += 1,
                }
            }
            if c.kind == ServiceKind::HandDown {
                let t = c.edge_terms().next().expect("one edge term");
                let tc = match id_of.get(t.key.as_slice()) {
                    None => HandDownTarget::Served,
                    Some(&j) if entries[j as usize].1.0 != *face => HandDownTarget::OtherFace,
                    Some(&j) if pieces_k[j as usize] == PieceKind::Core => HandDownTarget::OwnCore,
                    Some(_) => HandDownTarget::OwnFaceOther,
                };
                ch.hand_down_target = Some(tc);
                *hd_targets
                    .entry(
                        serde_json::to_value(tc)
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string(),
                    )
                    .or_default() += 1;
            }
            *by_kind
                .entry(
                    serde_json::to_value(c.kind)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string(),
                )
                .or_default() += 1;
        } else {
            *by_kind.entry("left_to_core".into()).or_default() += 1;
        }
        chosen.push(ch);
    }

    // the independent graph check: Kahn on the REVERSED plan graph (dependencies first)
    let fwd = MemGraph::from_edges(n, &plan_edges);
    let rev = fwd.reversed();
    let mut indeg = rev.in_degrees();
    let mut order_ids = Vec::with_capacity(n);
    let done = kahn(&rev, &mut indeg, Some(&mut order_ids)).map_err(gerr)?;
    if (done as usize) < n {
        let residual: Vec<bool> = indeg.iter().map(|&d| d > 0).collect();
        let mut c = witness_cycle(&rev, &residual).map_err(gerr)?;
        c.reverse();
        return Err(PlanError::Inconsistent {
            cycle: c.iter().map(|&v| keys[v as usize].clone()).collect(),
        });
    }
    let key_order: Vec<Vec<i32>> = order_ids
        .iter()
        .map(|&v| keys[v as usize].clone())
        .collect();

    // pieces
    let mut piece_ids: BTreeMap<PieceId, u32> = BTreeMap::new();
    for i in 0..n {
        piece_ids.insert(
            PieceId {
                face: entries[i].1.0.clone(),
                kind: pieces_k[i],
            },
            0,
        );
    }
    for (j, v) in piece_ids.values_mut().enumerate() {
        *v = j as u32;
    }
    let plist: Vec<PieceId> = piece_ids.keys().cloned().collect();
    let pid = |i: usize| -> u32 {
        piece_ids[&PieceId {
            face: entries[i].1.0.clone(),
            kind: pieces_k[i],
        }]
    };
    let mut pieces: Vec<Piece> = plist
        .iter()
        .map(|id| Piece {
            id: id.clone(),
            keys: 0,
            core_solved: 0,
            core_open: 0,
            depends_on: BTreeMap::new(),
        })
        .collect();
    for i in 0..n {
        let p = &mut pieces[pid(i) as usize];
        p.keys += 1;
        if pieces_k[i] == PieceKind::Core {
            if pick[i].is_some() {
                p.core_solved += 1;
            } else {
                p.core_open += 1;
            }
        }
    }
    let mut piece_edges: BTreeSet<(u32, u32)> = BTreeSet::new();
    let mut face_graph: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for &(s, t) in &plan_edges {
        let (ps, pt) = (pid(s as usize), pid(t as usize));
        if ps != pt {
            piece_edges.insert((ps, pt));
            *pieces[ps as usize]
                .depends_on
                .entry(plist[pt as usize].label())
                .or_default() += 1;
        }
        *face_graph
            .entry(entries[s as usize].1.0.clone())
            .or_default()
            .entry(entries[t as usize].1.0.clone())
            .or_default() += 1;
    }
    let pe: Vec<(u32, u32)> = piece_edges.into_iter().collect();
    let pg = MemGraph::from_edges(plist.len(), &pe);
    let mut piece_order: Vec<Vec<String>> = Vec::new();
    let all = vec![true; plist.len()];
    let psum = sccs(&pg, &all, |c, _| {
        piece_order.push(c.iter().map(|&p| plist[p as usize].label()).collect());
    })
    .map_err(gerr)?;

    // face graph and its cycles
    let faces: Vec<String> = {
        let mut f: BTreeSet<String> = entries.iter().map(|e| e.1.0.clone()).collect();
        for m in face_graph.values() {
            f.extend(m.keys().cloned());
        }
        f.into_iter().collect()
    };
    let fid: BTreeMap<&str, u32> = faces
        .iter()
        .enumerate()
        .map(|(i, f)| (f.as_str(), i as u32))
        .collect();
    let mut fe = Vec::new();
    for (s, m) in &face_graph {
        for t in m.keys() {
            fe.push((fid[s.as_str()], fid[t.as_str()]));
        }
    }
    let fg = MemGraph::from_edges(faces.len(), &fe);
    let mut face_cycles = Vec::new();
    let fall = vec![true; faces.len()];
    sccs(&fg, &fall, |c, cyclic| {
        if cyclic {
            face_cycles.push(c.iter().map(|&f| faces[f as usize].clone()).collect());
        }
    })
    .map_err(gerr)?;
    face_cycles.sort();
    let face_level_cyclic = !face_cycles.is_empty();
    let piece_level_acyclic = psum.cyclic == 0;
    let verdict = format!(
        "face level {}, piece level {}, key level acyclic (descent on every chosen edge and Kahn agree)",
        if face_level_cyclic {
            "CYCLIC"
        } else {
            "acyclic"
        },
        if piece_level_acyclic {
            "acyclic"
        } else {
            "CYCLIC (solve those pieces in key order)"
        }
    );

    // candidate graph cycles (before choice)
    let cg = MemGraph::from_edges(n, &cand_edges);
    let mut comps: Vec<Vec<u32>> = Vec::new();
    let csum = sccs(&cg, &vec![true; n], |c, cyclic| {
        if cyclic {
            comps.push(c.to_vec());
        }
    })
    .map_err(gerr)?;
    comps.sort();
    let in_plan: BTreeSet<(u32, u32)> = plan_edges.iter().copied().collect();
    let mut candidate_cycles = Vec::new();
    for c in comps.iter().take(input.max_candidate_cycles) {
        let mut member = vec![false; n];
        for &v in c {
            member[v as usize] = true;
        }
        let cyc = cycle_through(&cg, &member, c[0]).map_err(gerr)?;
        candidate_cycles.push(
            (0..cyc.len())
                .map(|i| {
                    let (s, t) = (cyc[i], cyc[(i + 1) % cyc.len()]);
                    let (from, to) = (keys[s as usize].clone(), keys[t as usize].clone());
                    PlanCycleEdge {
                        order: edge_order(order, &from, &to),
                        in_plan: in_plan.contains(&(s, t)),
                        from,
                        to,
                    }
                })
                .collect(),
        );
    }

    let core_open: Vec<Vec<i32>> = (0..n)
        .filter(|&i| pick[i].is_none())
        .map(|i| keys[i].clone())
        .collect();
    Ok(Plan {
        schema: PLAN_SCHEMA.into(),
        order: order.order_label(),
        keys: n as u64,
        chosen,
        refused,
        core_open,
        pieces,
        piece_order,
        key_order,
        edges,
        face_report: FaceReport {
            face_graph,
            face_cycles,
            face_level_cyclic,
            piece_level_acyclic,
            key_level_acyclic: true,
            verdict,
        },
        candidate_cycles,
        candidate_cyclic_sccs: csum.cyclic,
        counts_by_kind: by_kind,
        hand_down_targets: hd_targets,
    })
}
