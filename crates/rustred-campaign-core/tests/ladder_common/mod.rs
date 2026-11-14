//! Shared helpers of the ladder replay tests: the recorded reference attempts (tests/data/ladder/, converted from the
//! checker's read-only records, see tests/data/ladder/README.md) answered by a SCRIPTED case solver, scripted facts
//! and a prover, driven by the library's own sequential driver `repair::drive_tree`.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::{NodeComparator, NodeOrderRecord, SourceOrder};
use rustred_campaign_core::poly::Poly;
use rustred_campaign_core::repair::{
    CaseRequest, CaseSolver, Descent, FactSource, KeyProver, TreeLimits, TreeReport, drive_tree,
};
use rustred_campaign_core::worklist::affine::{AffineEq, fit_loci};
use rustred_campaign_core::worklist::items::{
    CaseSpec, ChildOrigin, EvidenceLevel, Face, Item, ItemKind, Key, NodeParams, RootSource,
};
use rustred_campaign_core::worklist::ladder::{
    CandidateRule, Disposition, KeyEvidence, PointFacts, SolveAnswer, SolveOutcome, Tri,
};
use rustred_campaign_core::worklist::seed::{AttemptBudget, CentreKind, SeedSetting};
use serde::Deserialize;

pub const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/ladder");

#[derive(Clone, Debug, Deserialize)]
pub struct RecCase {
    pub sd: Vec<usize>,
    pub sn: Vec<usize>,
    pub fixed: BTreeMap<String, i32>,
    pub eqs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rec {
    pub item: i64,
    pub source: String,
    pub case: RecCase,
    pub offsets: Vec<BTreeMap<String, i32>>,
    pub floor: bool,
    pub depth: u8,
    pub meas: String,
    pub exists: bool,
    pub rhs_terms: Option<u64>,
    pub rows: Option<u64>,
    pub tie_break: Option<bool>,
    pub cpu_ms: u64,
    pub points: Vec<Vec<i32>>,
    pub verdicts: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RecItem {
    pub item: i64,
    pub sd: Option<Vec<usize>>,
    pub sn: Option<Vec<usize>>,
    pub census: Vec<Vec<i32>>,
    pub closure_keys: Vec<Vec<i32>>,
    pub worker: String,
    pub stages_line: Option<u64>,
}

pub fn read_jsonl<T: for<'de> Deserialize<'de>>(name: &str) -> Vec<T> {
    let text = std::fs::read_to_string(format!("{DATA}/{name}")).expect("fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("record"))
        .collect()
}

pub fn t178_comparator() -> NodeComparator {
    NodeComparator::from_node_order(
        &NodeOrderRecord {
            name: "orderB".into(),
            slot_permutation: None,
            coordinate_priority: Some(vec![
                6, 4, 9, 8, 5, 2, 1, 0, 7, 3, 11, 12, 10, 13, 15, 14, 16, 17, 18, 19,
            ]),
            source_order: SourceOrder::Input,
        },
        20,
    )
    .expect("order B")
}

/// T162 bank order: legacy identity in the presented frame (campaign-core-order.md section 3).
pub fn t162_comparator() -> NodeComparator {
    NodeComparator::legacy(20, None).expect("legacy")
}

/// Dots + rank ONLY (planted control: never used by the library; R29 (b)).
pub struct DotsRankOnly {
    pub n_lines: usize,
}
impl Descent for DotsRankOnly {
    fn lower(&self, a: &[i32], b: &[i32]) -> bool {
        let dr = |k: &[i32]| -> i64 {
            k.iter()
                .enumerate()
                .map(|(i, &v)| {
                    if i < self.n_lines {
                        (v - 1).max(0) as i64
                    } else {
                        (-v).max(0) as i64
                    }
                })
                .sum()
        };
        dr(a) < dr(b)
    }
}

pub fn case_of(rc: &RecCase, arity: usize) -> CaseSpec {
    let fixed: BTreeMap<usize, i32> = rc
        .fixed
        .iter()
        .map(|(s, v)| (s.parse().unwrap(), *v))
        .collect();
    let eqs: Vec<AffineEq> = rc
        .eqs
        .iter()
        .map(|e| AffineEq::parse(e, arity).expect("eq"))
        .collect();
    CaseSpec::new(Face::new(rc.sd.clone(), rc.sn.clone()), fixed, eqs).expect("case")
}

/// Setting identity of a record (floor normalised exactly as the library does).
pub fn setting_of(r: &Rec, case: &CaseSpec, lines: &[bool]) -> SeedSetting {
    let offsets: Vec<BTreeMap<usize, i32>> = r
        .offsets
        .iter()
        .map(|m| m.iter().map(|(s, v)| (s.parse().unwrap(), *v)).collect())
        .collect();
    let face_line = case
        .free()
        .iter()
        .any(|&s| lines[s] && case.face.sd.contains(&s))
        || case.fixed_off_floor(lines).iter().any(|&s| lines[s]);
    SeedSetting {
        centre: CentreKind::Explicit,
        floor: r.floor && face_line && !offsets.is_empty(),
        offsets,
        depth: r.depth,
        tight: false,
    }
}

/// A scripted engine: answers (case, setting) from the recorded attempts; anything not recorded is "no relation".
pub struct Scripted {
    pub params: NodeParams,
    pub recs: Vec<Rec>,
    /// (case, setting) -> recorded attempts (all measures; a relation of any of them is a valid answer).
    pub index: BTreeMap<(String, String), Vec<usize>>,
    /// The reference worker read census "exception" verdicts as covered (item -> bool).
    pub exception_covered: BTreeMap<i64, bool>,
    pub calls: usize,
    pub unrecorded: BTreeSet<(String, String)>,
}

impl Scripted {
    pub fn new(params: NodeParams, recs: Vec<Rec>, exception_covered: BTreeMap<i64, bool>) -> Self {
        let mut index: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        for (i, r) in recs.iter().enumerate() {
            let c = case_of(&r.case, params.arity);
            let s = setting_of(r, &c, &params.lines);
            index
                .entry((c.canonical_text(), s.canonical_key()))
                .or_default()
                .push(i);
        }
        Scripted {
            params,
            recs,
            index,
            exception_covered,
            calls: 0,
            unrecorded: BTreeSet::new(),
        }
    }

    fn covered(&self, r: &Rec, v: &str) -> bool {
        v == "regular"
            || (v == "exception" && *self.exception_covered.get(&r.item).unwrap_or(&false))
    }

    /// The candidate a recorded relation stands for: the case's domain, with the reference's non-covered points as
    /// exception branches (a fit through them that holds at no covered point; else single points). The reference
    /// tools have no coefficients, so this is how the engine's exception set is stood in for.
    pub fn candidate(&self, ri: usize) -> CandidateRule {
        let r = &self.recs[ri];
        let p = &self.params;
        let case = case_of(&r.case, p.arity);
        let mut d = CaseDomain::free(p.arity, Some(p.lines.clone()));
        let free: BTreeSet<usize> = case.free().into_iter().collect();
        for i in 0..p.arity {
            if !free.contains(&i) {
                d.fixed[i] =
                    Some(
                        case.fixed
                            .get(&i)
                            .copied()
                            .unwrap_or(if p.lines[i] { 1 } else { 0 }),
                    );
            }
        }
        for e in &case.equations {
            d.equations
                .push(Poly::parse_d_free(&e.to_engine(), p.arity).unwrap());
        }
        let bad: Vec<Key> = r
            .points
            .iter()
            .zip(&r.verdicts)
            .filter(|(k, v)| !self.covered(r, v) && case.contains(k, &p.lines))
            .map(|(k, _)| k.clone())
            .collect();
        let good: Vec<Key> = r
            .points
            .iter()
            .zip(&r.verdicts)
            .filter(|(_, v)| self.covered(r, v))
            .map(|(k, _)| k.clone())
            .collect();
        let fv: Vec<usize> = free.iter().copied().collect();
        let mut done = vec![false; bad.len()];
        for (eq, held) in fit_loci(&bad, &fv) {
            if good.iter().any(|g| eq.holds(g)) {
                continue;
            }
            d.exceptions
                .push(vec![Poly::parse_d_free(&eq.to_engine(), p.arity).unwrap()]);
            for i in held {
                done[i] = true;
            }
        }
        for (i, k) in bad.iter().enumerate() {
            if !done[i] {
                d.exceptions.push(
                    fv.iter()
                        .map(|&s| {
                            Poly::parse_d_free(&AffineEq::fixed(s, k[s]).to_engine(), p.arity)
                                .unwrap()
                        })
                        .collect(),
                );
            }
        }
        CandidateRule {
            domain: d,
            digest: format!("rec{ri}"),
            rhs_terms: r.rhs_terms.unwrap_or(1000),
            monomials: None,
            max_degree: None,
            bytes: None,
            rows: r.rows,
            tie_break_only: r.tie_break,
            symmetry_rows: false,
        }
    }
}

impl CaseSolver for Scripted {
    fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        self.calls += 1;
        let k = (req.case.canonical_text(), req.setting.canonical_key());
        match self.index.get(&k).cloned() {
            Some(list) => {
                // every recorded relation of this (case, setting) is a candidate (the grid "reports every relation
                // found", 2.4.13); the C record first
                let mut with: Vec<usize> = list
                    .iter()
                    .copied()
                    .filter(|&i| self.recs[i].exists)
                    .collect();
                with.sort_by_key(|&i| (self.recs[i].meas != "C", i));
                let cpu: u64 = list
                    .iter()
                    .map(|&i| self.recs[i].cpu_ms.max(1))
                    .max()
                    .unwrap_or(1);
                if with.is_empty() {
                    SolveAnswer {
                        outcome: SolveOutcome::NoRelation,
                        cpu_ms: cpu,
                        rows: None,
                    }
                } else {
                    SolveAnswer {
                        outcome: SolveOutcome::Rules(
                            with.iter().map(|&i| self.candidate(i)).collect(),
                        ),
                        cpu_ms: cpu,
                        rows: self.recs[with[0]].rows,
                    }
                }
            }
            None => {
                self.unrecorded.insert(k);
                SolveAnswer {
                    outcome: SolveOutcome::NoRelation,
                    cpu_ms: 1000,
                    rows: None,
                }
            }
        }
    }
}

/// Scripted per-key facts: regular, exceptions complete, and ONE top RHS key: strictly lower in dots + rank for an
/// ordinary relation, or at EQUAL dots + rank but lower by the comparator's tie-breaks for a relation the reference
/// recorded as tie-break only (the comparator is the real node comparator).
pub struct ScriptedFacts<'a> {
    pub cmp: &'a NodeComparator,
    pub n_lines: usize,
    pub line_priority: Vec<usize>,
}

impl ScriptedFacts<'_> {
    pub fn lower_rhs(&self, k: &[i32]) -> Vec<Key> {
        let mut r = k.to_vec();
        if let Some(i) = (0..self.n_lines).find(|&i| r[i] >= 2) {
            r[i] -= 1;
            return vec![r];
        }
        if let Some(j) = (self.n_lines..r.len()).find(|&j| r[j] <= -1) {
            r[j] += 1;
            return vec![r];
        }
        vec![]
    }
    pub fn tie_rhs(&self, k: &[i32]) -> Option<Key> {
        let h = self.line_priority[0];
        if let Some(j) = (self.n_lines..k.len()).find(|&j| k[j] <= -1) {
            let mut r = k.to_vec();
            r[j] += 1;
            r[h] += 1;
            return self.cmp.lower(&r, k).then_some(r);
        }
        let l = self
            .line_priority
            .iter()
            .rev()
            .copied()
            .find(|&i| k[i] >= 2)?;
        let h = self.line_priority.iter().copied().find(|&i| i != l)?;
        let mut r = k.to_vec();
        r[l] -= 1;
        r[h] += 1;
        self.cmp.lower(&r, k).then_some(r)
    }
}

impl FactSource for ScriptedFacts<'_> {
    fn facts(&mut self, rule: &CandidateRule, key: &[i32]) -> PointFacts {
        let rhs = if rule.tie_break_only == Some(true) {
            self.tie_rhs(key)
                .map(|r| vec![r])
                .unwrap_or_else(|| self.lower_rhs(key))
        } else {
            self.lower_rhs(key)
        };
        PointFacts {
            regular: Tri::Pass,
            rhs_top: Some(rhs),
            exceptions_complete: Tri::Pass,
            note: None,
        }
    }
}

/// A prover that confirms every key at the wanted level (the reference relations replayed at two primes).
pub struct ReplayProver;
impl KeyProver for ReplayProver {
    fn prove(
        &mut self,
        _r: &CandidateRule,
        keys: &[Key],
        wanted: EvidenceLevel,
    ) -> Vec<KeyEvidence> {
        keys.iter()
            .map(|k| KeyEvidence {
                key: k.clone(),
                level: Some(wanted),
                failure: None,
            })
            .collect()
    }
}

pub fn root_item(params: &NodeParams, face: Face, census: Vec<Key>) -> Item {
    let case = CaseSpec::face_case(face);
    let dim = case.dim();
    let kind = if dim == 0 {
        ItemKind::Point
    } else {
        ItemKind::GenericFace
    };
    Item::new(
        &params.node,
        kind,
        case,
        census,
        None,
        0,
        dim,
        ChildOrigin::Root(RootSource::Partition),
        None,
    )
}

/// Outcome of one replayed reference item.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub cases: usize,
    pub point_rules: usize,
    pub terminals: Vec<Key>,
    pub attention: Vec<String>,
    /// (case canonical text, setting label) per accepted rule.
    pub rules: Vec<(String, String, usize)>,
    pub attempts: usize,
}

pub fn outcome(rep: &TreeReport) -> Outcome {
    let mut rules = Vec::new();
    let mut point_rules = 0;
    for (id, m) in rep.accepted_rules() {
        let it = rep.items.iter().find(|r| r.state.item.id == id).unwrap();
        if it.state.item.case.is_point() {
            point_rules += 1;
        }
        // record the setting as the recorded offsets (labels of explicit settings are rebuilt by the library)
        rules.push((
            it.state.item.case.canonical_text(),
            m.setting.label(),
            m.served.len(),
        ));
    }
    let attention = rep
        .items
        .iter()
        .filter_map(|r| match &r.disposition {
            Some(Disposition::Attention { why, .. }) => Some(why.clone()),
            _ => None,
        })
        .collect();
    Outcome {
        cases: rules.len(),
        point_rules,
        terminals: rep
            .terminal_candidates()
            .into_iter()
            .map(|(k, _)| k)
            .collect(),
        attention,
        rules,
        attempts: rep.attempts.len(),
    }
}

pub fn run_item(
    params: &NodeParams,
    solver: &mut Scripted,
    descent: &dyn Descent,
    facts: &mut dyn FactSource,
    face: Face,
    census: Vec<Key>,
) -> (TreeReport, Outcome) {
    let root = root_item(params, face, census);
    let rep = drive_tree(
        vec![root],
        params,
        descent,
        solver,
        facts,
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    let o = outcome(&rep);
    (rep, o)
}
