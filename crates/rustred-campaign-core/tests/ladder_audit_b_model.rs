//! AUDIT audit-ladder-B: model-based adversarial testing of the repair library through its PUBLIC API.
//! PORTED by lane ladderfix as a regression test (from implementer-scratch/audit-ladder-B, audit at 2c82576c): API of
//! repair-library v1.2 (exclusions, verdict records, step bounds), and every part ASSERTS that the invariants the
//! audit found violated (C1 livelock, C2 domain safety, M1 re-run attempt keys, m1 multi-key terminals) and the
//! soundness / accounting / counter invariants have ZERO violations. Default AUDIT_WORLDS = 300 (the audit: 2,000).
//!
//! Synthetic toy worlds (seeded): a node of arity 3..6 with a random line mask and the crate's legacy NodeComparator
//! (random tie permutation), gap faces with gap points, a hidden GROUND TRUTH of rules (domains with exception
//! planes / diagonals / general affine literals with non-unit coefficients, declared poles, hidden pole planes,
//! non-descending planes, tie-break-only descent, invalid identities), truly irreducible keys. Trait
//! implementations answer TRUTHFULLY from the ground truth (or adversarially, by mode). Invariants I1-I7 are
//! checked against the ground truth. Run: `cargo test --test audit_b_model -- --nocapture` (debug: overflow checks).
//! Scale with AUDIT_WORLDS (default 2000).
#![allow(clippy::too_many_arguments, clippy::type_complexity, dead_code)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use rustred_campaign_core::domain::{CaseDomain, PoleKind, PoleLocus, Verdict};
use rustred_campaign_core::explain::explain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::poly::Poly;
use rustred_campaign_core::repair::{
    CaseRequest, CaseSolver, Coverage, CoverageVerdict, Descent, FactSource, KeyProver,
    RepairBudget, RepairFailure, RepairInputs, TreeLimits, TreeReport, drive_tree, repair_key,
};
use rustred_campaign_core::terminals::{FallbackSet, TerminalError, check_terminals, preferred_report};
use rustred_campaign_core::worklist::affine::{AffineEq, AffineError, canonical_system, fit_loci};
use rustred_campaign_core::worklist::items::{
    CaseSpec, EvidenceLevel, Face, Item, ItemKind, Key, NodeParams, PreferredEntry,
    UserBox, WorklistError, box_restricted_item, items_from_closure_keys,
};
use rustred_campaign_core::worklist::ladder::{
    CandidateRule, Disposition, Interruption, ItemState, KeyEvidence, Next, PointFacts,
    PointVerdict, ProofFailure, SolveAnswer, SolveOutcome, Tri, judge_points_with_hits,
    next_steps, sub_counter,
};
use rustred_campaign_core::worklist::policy::CentreSpec;
use rustred_campaign_core::worklist::seed::{AttemptBudget, SeedSetting, seed_space};

// ------------------------------------------------------------------------------------------------ rng / hash

#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(s: u64) -> Self {
        Rng(s ^ 0x5DEECE66D)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    pub fn range(&mut self, a: i64, b: i64) -> i64 {
        a + self.below((b - a + 1) as u64) as i64
    }
    pub fn chance(&mut self, permille: u64) -> bool {
        self.below(1000) < permille
    }
    pub fn pick<T: Clone>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u64) as usize].clone()
    }
}

pub fn hsh<T: Hash>(seed: u64, x: &T) -> u64 {
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    x.hash(&mut h);
    let mut r = Rng(h.finish());
    r.next()
}

fn floor_of(is_line: bool) -> i32 {
    if is_line { 1 } else { 0 }
}

fn poly_of(e: &AffineEq, n: usize) -> Option<Poly> {
    Poly::parse_d_free(&e.to_engine(), n).ok()
}

/// The case's own domain (fixed slots outside the face at their floor, chart, equations).
fn domain_of_case(c: &CaseSpec, lines: &[bool]) -> Option<CaseDomain> {
    let n = lines.len();
    let mut d = CaseDomain::free(n, Some(lines.to_vec()));
    let face: BTreeSet<usize> = c.face.slots().into_iter().collect();
    for i in 0..n {
        if let Some(v) = c.fixed.get(&i) {
            d.fixed[i] = Some(*v);
        } else if !face.contains(&i) {
            d.fixed[i] = Some(floor_of(lines[i]));
        }
    }
    for e in &c.equations {
        d.equations.push(poly_of(e, n)?);
    }
    Some(d)
}

// ------------------------------------------------------------------------------------------------ world

#[derive(Clone, Debug)]
pub struct Req {
    pub centre: Option<CentreSpec>,
    pub floor: Option<bool>,
    pub min_depth: u8,
}
impl Req {
    fn matches(&self, s: &SeedSetting) -> bool {
        self.centre.is_none_or(|c| s.centre.spec() == c)
            && self.floor.is_none_or(|f| s.floor == f)
            && s.depth >= self.min_depth
    }
    fn random(r: &mut Rng) -> Req {
        let cs = [
            CentreSpec::Target,
            CentreSpec::Line,
            CentreSpec::Num,
            CentreSpec::Pair,
            CentreSpec::Combined,
            CentreSpec::UnionLines,
            CentreSpec::Fixed,
        ];
        Req {
            centre: if r.chance(350) { None } else { Some(r.pick(&cs)) },
            floor: if r.chance(600) { None } else { Some(r.chance(500)) },
            min_depth: if r.chance(700) { 2 } else { 3 },
        }
    }
}

#[derive(Clone, Debug)]
pub struct HiddenRule {
    pub digest: String,
    pub req: Req,
    pub domain: CaseDomain,
    pub rhs_terms: u64,
    pub p_invalid: u64,
    pub p_irregular: u64,
    pub p_nodesc: u64,
    pub p_tie: u64,
    pub hidden_plane: Option<AffineEq>,
    pub nodesc_plane: Option<AffineEq>,
    pub tie_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Desc {
    Strict,
    Tie,
    No,
}

#[derive(Clone, Copy, Debug)]
pub struct KeyTruth {
    pub valid: bool,
    pub regular: bool,
    pub desc: Desc,
}

pub struct World {
    pub seed: u64,
    pub n: usize,
    pub lines: Vec<bool>,
    pub cmp: NodeComparator,
    pub faces: Vec<Face>,
    pub gap: Vec<Key>,
    pub irreducible: BTreeSet<Key>,
    pub root_rules: BTreeMap<String, Vec<HiddenRule>>,
    pub p_child: u64,
    pub p_fail: u64,
    pub registry: RefCell<BTreeMap<String, HiddenRule>>,
}

fn random_exception(r: &mut Rng, free: &[usize], lines: &[bool], n: usize) -> Option<Vec<Poly>> {
    if free.is_empty() {
        return None;
    }
    let s = r.pick(free);
    let val = |r: &mut Rng, s: usize| -> i32 {
        if lines[s] { r.range(2, 4) as i32 } else { r.range(-3, -1) as i32 }
    };
    let eq = match r.below(4) {
        0 => AffineEq::fixed(s, val(r, s)),
        1 if free.len() >= 2 => {
            let t = r.pick(free);
            if t == s {
                AffineEq::fixed(s, val(r, s))
            } else {
                AffineEq::new(vec![(s, 1), (t, -1)], r.range(-2, 2))?
            }
        }
        2 if free.len() >= 2 => {
            // general affine with non-unit / negative coefficients
            let t = r.pick(free);
            let a = [1i64, 2, 3, -2, -3][r.below(5) as usize];
            let b = [1i64, 2, -1, -2, 3][r.below(5) as usize];
            if t == s {
                AffineEq::fixed(s, val(r, s))
            } else {
                AffineEq::new(vec![(s, a), (t, b)], r.range(-6, 6))?
            }
        }
        _ => {
            // a two-literal branch (conjunction of two planes)
            let t = r.pick(free);
            let e1 = AffineEq::fixed(s, val(r, s));
            let e2 = AffineEq::fixed(t, val(r, t));
            return Some(vec![poly_of(&e1, n)?, poly_of(&e2, n)?]);
        }
    };
    Some(vec![poly_of(&eq, n)?])
}

fn random_plane(r: &mut Rng, free: &[usize], lines: &[bool]) -> Option<AffineEq> {
    if free.is_empty() {
        return None;
    }
    let s = r.pick(free);
    if r.chance(600) {
        Some(AffineEq::fixed(
            s,
            if lines[s] { r.range(2, 4) as i32 } else { r.range(-3, -1) as i32 },
        ))
    } else {
        let t = r.pick(free);
        if t == s {
            return None;
        }
        AffineEq::new(vec![(s, 1), (t, 1)], -r.range(-2, 6))
    }
}

impl World {
    pub fn generate(seed: u64) -> World {
        let mut r = Rng::new(seed);
        let n = r.range(3, 6) as usize;
        let n_lines = r.range(1, n as i64) as usize;
        let mut lines: Vec<bool> = (0..n).map(|i| i < n_lines).collect();
        // shuffle the mask
        for i in (1..n).rev() {
            let j = r.below(i as u64 + 1) as usize;
            lines.swap(i, j);
        }
        let mut ties: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = r.below(i as u64 + 1) as usize;
            ties.swap(i, j);
        }
        let cmp = NodeComparator::legacy(n, Some(ties)).expect("legacy comparator");
        let line_slots: Vec<usize> = (0..n).filter(|&i| lines[i]).collect();
        let num_slots: Vec<usize> = (0..n).filter(|&i| !lines[i]).collect();
        let n_faces = r.range(1, 3) as usize;
        let mut faces = BTreeSet::new();
        for _ in 0..n_faces * 3 {
            if faces.len() >= n_faces {
                break;
            }
            let mut sd: Vec<usize> = line_slots.iter().copied().filter(|_| r.chance(450)).collect();
            sd.truncate(3);
            let mut sn: Vec<usize> = num_slots.iter().copied().filter(|_| r.chance(350)).collect();
            sn.truncate(2);
            if sd.is_empty() && sn.is_empty() {
                if !line_slots.is_empty() {
                    sd.push(r.pick(&line_slots));
                } else {
                    sn.push(r.pick(&num_slots));
                }
            }
            faces.insert(Face::new(sd, sn));
        }
        let faces: Vec<Face> = faces.into_iter().collect();
        let mut gap = BTreeSet::new();
        for f in &faces {
            let np = r.range(2, 12);
            for _ in 0..np * 2 {
                let mut k: Key = (0..n).map(|i| floor_of(lines[i])).collect();
                for &s in &f.sd {
                    k[s] = r.range(2, 4) as i32;
                }
                for &s in &f.sn {
                    k[s] = r.range(-3, -1) as i32;
                }
                gap.insert(k);
                if gap.len() as i64 >= np + 50 {
                    break;
                }
            }
        }
        let gap: Vec<Key> = gap.into_iter().collect();
        let irreducible: BTreeSet<Key> = gap.iter().filter(|_| r.chance(60)).cloned().collect();
        let mut root_rules = BTreeMap::new();
        for f in &faces {
            let case = CaseSpec::face_case(f.clone());
            let free = case.free();
            let mut v = Vec::new();
            for j in 0..r.range(0, 4) {
                let mut d = domain_of_case(&case, &lines).unwrap();
                for _ in 0..r.below(3) {
                    if let Some(b) = random_exception(&mut r, &free, &lines, n) {
                        d.exceptions.push(b);
                    }
                }
                if r.chance(200)
                    && let Some(p) = random_plane(&mut r, &free, &lines)
                    && let Some(pp) = poly_of(&p, n)
                {
                    d.poles.push(PoleLocus {
                        kind: PoleKind::Denominator,
                        coeffs: vec![pp],
                    });
                }
                let digest = format!("w{seed}f{}r{j}", f.label());
                let hr = HiddenRule {
                    digest: digest.clone(),
                    req: Req::random(&mut r),
                    domain: d,
                    rhs_terms: 1 + r.below(5000),
                    p_invalid: [0, 0, 30, 300][r.below(4) as usize],
                    p_irregular: [0, 0, 20, 100][r.below(4) as usize],
                    p_nodesc: [0, 0, 50, 300, 1000][r.below(5) as usize],
                    p_tie: [0, 200, 1000][r.below(3) as usize],
                    hidden_plane: if r.chance(150) { random_plane(&mut r, &free, &lines) } else { None },
                    nodesc_plane: if r.chance(200) { random_plane(&mut r, &free, &lines) } else { None },
                    tie_only: false,
                };
                v.push(hr);
            }
            root_rules.insert(f.label(), v);
        }
        let w = World {
            seed,
            n,
            lines,
            cmp,
            faces,
            gap,
            irreducible,
            root_rules,
            p_child: [200, 500, 800][r.below(3) as usize],
            p_fail: [0, 0, 50, 200][r.below(4) as usize],
            registry: RefCell::new(BTreeMap::new()),
        };
        for v in w.root_rules.values() {
            for hr in v {
                w.registry.borrow_mut().insert(hr.digest.clone(), hr.clone());
            }
        }
        w
    }

    pub fn truth(&self, digest: &str, k: &[i32]) -> KeyTruth {
        let reg = self.registry.borrow();
        let def = HiddenRule {
            digest: digest.into(),
            req: Req { centre: None, floor: None, min_depth: 2 },
            domain: CaseDomain::free(self.n, None),
            rhs_terms: 1,
            p_invalid: 300,
            p_irregular: 100,
            p_nodesc: 300,
            p_tie: 200,
            hidden_plane: None,
            nodesc_plane: None,
            tie_only: false,
        };
        let r = reg.get(digest).unwrap_or(&def);
        let x = hsh(self.seed, &(digest, k));
        let mut t = KeyTruth {
            valid: (x % 1000) >= r.p_invalid,
            regular: ((x >> 10) % 1000) >= r.p_irregular,
            desc: if ((x >> 20) % 1000) < r.p_nodesc {
                Desc::No
            } else if ((x >> 30) % 1000) < r.p_tie {
                Desc::Tie
            } else {
                Desc::Strict
            },
        };
        if r.hidden_plane.as_ref().is_some_and(|p| p.holds(k)) {
            t.regular = false;
        }
        if r.nodesc_plane.as_ref().is_some_and(|p| p.holds(k)) || self.irreducible.contains(k) {
            t.desc = Desc::No;
        }
        t
    }

    /// The rule truly reduces k (geometry + identity when evidence >= A2 is required) and its domain applies.
    pub fn acceptable(&self, rule: &CandidateRule, k: &[i32], min: EvidenceLevel) -> bool {
        if rule.domain.applies(k) != Verdict::Applies {
            return false;
        }
        let t = self.truth(&rule.digest, k);
        t.regular && t.desc != Desc::No && (min < EvidenceLevel::A2 || t.valid)
    }

    pub fn lower_strict(&self, k: &[i32]) -> Vec<Key> {
        let mut r = k.to_vec();
        if let Some(i) = (0..self.n).find(|&i| self.lines[i] && r[i] >= 2) {
            r[i] -= 1;
            return vec![r];
        }
        if let Some(j) = (0..self.n).find(|&j| !self.lines[j] && r[j] <= -1) {
            r[j] += 1;
            return vec![r];
        }
        vec![]
    }

    pub fn rhs(&self, digest: &str, k: &[i32]) -> Vec<Key> {
        match self.truth(digest, k).desc {
            Desc::Strict => self.lower_strict(k),
            Desc::Tie => {
                // same total excess, lower by the comparator's tie-breaks
                for i in 0..self.n {
                    for j in 0..self.n {
                        if i == j || !self.lines[i] || !self.lines[j] || k[i] < 2 {
                            continue;
                        }
                        let mut r = k.to_vec();
                        r[i] -= 1;
                        r[j] += 1;
                        if self.cmp.lower(&r, k) {
                            return vec![r];
                        }
                    }
                }
                self.lower_strict(k)
            }
            Desc::No => {
                let mut r = k.to_vec();
                let l = (0..self.n).find(|&i| self.lines[i]).unwrap_or(0);
                if self.lines[l] {
                    r[l] += 1;
                } else {
                    r[l] -= 1;
                }
                let mut v = self.lower_strict(k);
                v.push(r);
                v
            }
        }
    }

    /// Lazily generated child rules of a non-root case (pure function of the world seed and the case).
    pub fn child_rules(&self, c: &CaseSpec) -> Vec<HiddenRule> {
        let text = c.canonical_text();
        let mut out = Vec::new();
        for j in 0..2u64 {
            let x = hsh(self.seed, &("child", &text, j));
            if x % 1000 >= self.p_child {
                continue;
            }
            let Some(mut d) = domain_of_case(c, &self.lines) else {
                continue;
            };
            let mut r = Rng::new(x);
            if r.chance(300)
                && let Some(b) = random_exception(&mut r, &c.free(), &self.lines, self.n)
            {
                d.exceptions.push(b);
            }
            let digest = format!("w{}c{:016x}", self.seed, x);
            let hr = HiddenRule {
                digest: digest.clone(),
                req: Req::random(&mut r),
                domain: d,
                rhs_terms: 1 + r.below(5000),
                p_invalid: [0, 0, 30, 300][r.below(4) as usize],
                p_irregular: [0, 0, 20, 100][r.below(4) as usize],
                p_nodesc: [0, 0, 50, 300][r.below(4) as usize],
                p_tie: [0, 200, 1000][r.below(3) as usize],
                hidden_plane: if r.chance(100) { random_plane(&mut r, &c.free(), &self.lines) } else { None },
                nodesc_plane: None,
                tie_only: false,
            };
            self.registry.borrow_mut().entry(digest).or_insert(hr.clone());
            out.push(hr);
        }
        out
    }

    pub fn rules_for_case(&self, c: &CaseSpec) -> Vec<HiddenRule> {
        if c.fixed.is_empty() && c.equations.is_empty() {
            self.root_rules.get(&c.face.label()).cloned().unwrap_or_default()
        } else {
            self.child_rules(c)
        }
    }

    pub fn candidate(&self, hr: &HiddenRule) -> CandidateRule {
        CandidateRule {
            domain: hr.domain.clone(),
            digest: hr.digest.clone(),
            rhs_terms: hr.rhs_terms,
            monomials: None,
            max_degree: None,
            bytes: None,
            rows: None,
            tie_break_only: Some(hr.p_tie == 1000),
            symmetry_rows: false, witness: rustred_campaign_core::cover::WitnessStatus::Complete,
        }
    }

    /// The TRUTHFUL engine answer for (case, setting) (pure).
    pub fn answer(&self, case: &CaseSpec, s: &SeedSetting) -> SolveAnswer {
        let x = hsh(self.seed, &("ans", case.canonical_text(), s.canonical_key()));
        let cpu = 1 + x % 5000;
        if (x >> 16) % 1000 < self.p_fail {
            let outcome = match (x >> 32) % 6 {
                0 => SolveOutcome::BudgetStop { time_decided: true },
                1 => SolveOutcome::BudgetStop { time_decided: false },
                2 => SolveOutcome::RefusedHeavy { reason: "heavy".into() },
                3 => SolveOutcome::ProfileRefused { reason: "profile".into() },
                4 => SolveOutcome::KilledMemory,
                _ => SolveOutcome::Error("engine error".into()),
            };
            return SolveAnswer { outcome, cpu_ms: cpu, rows: None };
        }
        let rules: Vec<CandidateRule> = self
            .rules_for_case(case)
            .iter()
            .filter(|h| h.req.matches(s))
            .map(|h| self.candidate(h))
            .collect();
        SolveAnswer {
            outcome: if rules.is_empty() { SolveOutcome::NoRelation } else { SolveOutcome::Rules(rules) },
            cpu_ms: cpu,
            rows: None,
        }
    }

    pub fn params(&self, r: &mut Rng) -> NodeParams {
        let mut p = NodeParams::with_lines("toy", self.lines.clone());
        p.min_evidence = [EvidenceLevel::A1, EvidenceLevel::A2, EvidenceLevel::A3][r.below(3) as usize];
        p.terminals_max = [0usize, 1, 3, 100][r.below(4) as usize];
        p
    }
}

// ------------------------------------------------------------------------------------------------ traits

pub struct Calls {
    pub last: String,
    pub repeat: u64,
    pub solve: u64,
    pub prove: u64,
    pub facts: u64,
    pub cap: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolverMode {
    Truthful,
    ForeignDomain,
    WrongArity,
    SameRuleForever,
    RegenExceptions,
    Alternating,
    AllFailures,
    Garbage,
}

pub struct Solver<'w> {
    pub w: &'w World,
    pub mode: SolverMode,
    pub calls: Rc<RefCell<Calls>>,
    pub per_request: BTreeMap<String, u64>,
    pub p_operator: u64,
    pub p_infra: u64,
    pub rng: Rng,
}

impl Solver<'_> {
    fn garbage(&self, case: &CaseSpec, salt: u64) -> CandidateRule {
        let n = self.w.n;
        let x = hsh(self.w.seed, &("garbage", case.canonical_text(), salt));
        let mut r = Rng::new(x);
        let mut d = CaseDomain::free(n, Some(self.w.lines.clone()));
        if r.chance(500) {
            // fixed to other values: a rule for a different target / case
            let s = r.below(n as u64) as usize;
            d.fixed[s] = Some(r.range(-3, 4) as i32);
        }
        if r.chance(300) {
            d.chart = None;
        }
        let digest = format!("g{x:016x}");
        self.w.registry.borrow_mut().entry(digest.clone()).or_insert(HiddenRule {
            digest: digest.clone(),
            req: Req { centre: None, floor: None, min_depth: 2 },
            domain: d.clone(),
            rhs_terms: 1,
            p_invalid: 300,
            p_irregular: 100,
            p_nodesc: 300,
            p_tie: 200,
            hidden_plane: None,
            nodesc_plane: None,
            tie_only: false,
        });
        CandidateRule {
            domain: d,
            digest,
            rhs_terms: 1 + x % 100,
            monomials: Some(x % 7),
            max_degree: Some(1),
            bytes: None,
            rows: None,
            tie_break_only: None,
            symmetry_rows: r.chance(100), witness: rustred_campaign_core::cover::WitnessStatus::Complete,
        }
    }
}

impl CaseSolver for Solver<'_> {
    fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        {
            let mut c = self.calls.borrow_mut();
            c.solve += 1;
            c.repeat = 0;
            c.last.clear();
            if c.solve > c.cap.max(30_000) {
                panic!("LIVELOCK: solver calls above {}", c.cap.max(30_000));
            }
        }
        if self.p_operator > 0 && self.rng.chance(self.p_operator) {
            return SolveAnswer {
                outcome: SolveOutcome::Interrupted { by: Interruption::Operator },
                cpu_ms: 3,
                rows: None,
            };
        }
        if self.p_infra > 0 && self.rng.chance(self.p_infra) {
            return SolveAnswer {
                outcome: SolveOutcome::Interrupted { by: Interruption::Infra },
                cpu_ms: 3,
                rows: None,
            };
        }
        let key = format!("{}|{}", req.case.canonical_text(), req.setting.canonical_key());
        let cnt = {
            let e = self.per_request.entry(key).or_insert(0);
            *e += 1;
            *e
        };
        let w = self.w;
        let truthful = w.answer(&req.case, &req.setting);
        let x = hsh(w.seed, &("mode", req.case.canonical_text(), req.setting.canonical_key(), cnt));
        match self.mode {
            SolverMode::Truthful => truthful,
            SolverMode::ForeignDomain => {
                let mut v = match truthful.outcome {
                    SolveOutcome::Rules(v) => v,
                    _ => vec![],
                };
                v.push(self.garbage(&req.case, x % 3));
                SolveAnswer { outcome: SolveOutcome::Rules(v), cpu_ms: truthful.cpu_ms, rows: None }
            }
            SolverMode::WrongArity => {
                let mut g = self.garbage(&req.case, 7);
                g.domain = CaseDomain::free(w.n + 1 + (x % 3) as usize, None);
                g.digest = format!("{}-arity", g.digest);
                SolveAnswer { outcome: SolveOutcome::Rules(vec![g]), cpu_ms: 10, rows: None }
            }
            SolverMode::SameRuleForever => {
                let g = self.garbage(&CaseSpec::face_case(Face::new(vec![], vec![])), 1);
                SolveAnswer { outcome: SolveOutcome::Rules(vec![g]), cpu_ms: 10, rows: None }
            }
            SolverMode::RegenExceptions => {
                // exceptions that regenerate the parent case: literals over slots OUTSIDE the face (at their floor:
                // vanish identically on the case), the empty AND-list, the zero polynomial, the case's own fixed value
                let mut g = self.garbage(&req.case, 11);
                g.digest = format!("{}-regen{}", g.digest, x % 4);
                let face: BTreeSet<usize> = req.case.face.slots().into_iter().collect();
                let outside: Vec<usize> = (0..w.n).filter(|s| !face.contains(s)).collect();
                g.domain = domain_of_case(&req.case, &w.lines).unwrap_or(g.domain);
                match x % 4 {
                    0 if outside.len() >= 2 => {
                        let (a, b) = (outside[0], outside[1]);
                        let c = floor_of(w.lines[a]) as i64 + floor_of(w.lines[b]) as i64;
                        if let Some(e) = AffineEq::new(vec![(a, 1), (b, 1)], -c)
                            && let Some(p) = poly_of(&e, w.n)
                        {
                            g.domain.exceptions.push(vec![p]);
                        }
                    }
                    1 => g.domain.exceptions.push(vec![]),
                    2 => g.domain.exceptions.push(vec![Poly::zero()]),
                    _ => {
                        if let Some((&s, &v)) = req.case.fixed.iter().next()
                            && let Some(p) = poly_of(&AffineEq::fixed(s, v), w.n)
                        {
                            g.domain.exceptions.push(vec![p]);
                        } else if let Some(e) = req.case.equations.first()
                            && let Some(p) = poly_of(e, w.n)
                        {
                            g.domain.exceptions.push(vec![p]);
                        }
                    }
                }
                w.registry.borrow_mut().entry(g.digest.clone()).or_insert(HiddenRule {
                    digest: g.digest.clone(),
                    req: Req { centre: None, floor: None, min_depth: 2 },
                    domain: g.domain.clone(),
                    rhs_terms: 1,
                    p_invalid: 0,
                    p_irregular: 0,
                    p_nodesc: 0,
                    p_tie: 0,
                    hidden_plane: None,
                    nodesc_plane: None,
                    tie_only: false,
                });
                SolveAnswer { outcome: SolveOutcome::Rules(vec![g]), cpu_ms: 10, rows: None }
            }
            SolverMode::Alternating => {
                if cnt % 2 == 1 {
                    let g = self.garbage(&req.case, cnt);
                    SolveAnswer { outcome: SolveOutcome::Rules(vec![g]), cpu_ms: 5, rows: None }
                } else {
                    truthful
                }
            }
            SolverMode::AllFailures => {
                let total = self.calls.borrow().solve;
                let outcome = match total % 9 {
                    0 => SolveOutcome::BudgetStop { time_decided: true },
                    1 => SolveOutcome::BudgetStop { time_decided: false },
                    2 => SolveOutcome::RefusedHeavy { reason: "h".into() },
                    3 => SolveOutcome::ProfileRefused { reason: "p".into() },
                    4 => SolveOutcome::KilledMemory,
                    5 => SolveOutcome::Error("e".into()),
                    6 => SolveOutcome::NoRelation,
                    7 => SolveOutcome::Rules(vec![]),
                    _ => SolveOutcome::Interrupted { by: Interruption::Operator },
                };
                SolveAnswer { outcome, cpu_ms: 7, rows: None }
            }
            SolverMode::Garbage => match x % 6 {
                0 => truthful,
                1 => SolveAnswer { outcome: SolveOutcome::Rules(vec![self.garbage(&req.case, x)]), cpu_ms: 1, rows: None },
                2 => SolveAnswer { outcome: SolveOutcome::Rules(vec![self.garbage(&req.case, 1), self.garbage(&req.case, 1)]), cpu_ms: 0, rows: None },
                3 => SolveAnswer { outcome: SolveOutcome::Error(String::new()), cpu_ms: u64::MAX / 4, rows: Some(u64::MAX) },
                4 => SolveAnswer { outcome: SolveOutcome::Interrupted { by: Interruption::Infra }, cpu_ms: 0, rows: None },
                _ => SolveAnswer { outcome: SolveOutcome::Rules(vec![]), cpu_ms: 0, rows: None },
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactMode {
    Truthful,
    AllUnknown,
    Random,
}

pub struct Facts<'w> {
    pub w: &'w World,
    pub mode: FactMode,
    pub calls: Rc<RefCell<Calls>>,
}

impl FactSource for Facts<'_> {
    fn facts(&mut self, rule: &CandidateRule, key: &[i32]) -> PointFacts {
        {
            let mut c = self.calls.borrow_mut();
            c.facts += 1;
            c.repeat = 0;
            c.last.clear();
            if c.facts > c.cap.max(30_000) * 50 {
                panic!("LIVELOCK: facts calls above {}", c.cap * 50);
            }
        }
        match self.mode {
            FactMode::Truthful => {
                let t = self.w.truth(&rule.digest, key);
                PointFacts {
                    regular: if t.regular { Tri::Pass } else { Tri::Fail },
                    rhs_top: Some(self.w.rhs(&rule.digest, key)),
                    exceptions_complete: Tri::Pass,
                    note: None,
                }
            }
            FactMode::AllUnknown => PointFacts {
                regular: Tri::Unknown,
                rhs_top: None,
                exceptions_complete: Tri::Unknown,
                note: None,
            },
            FactMode::Random => {
                let x = hsh(self.w.seed, &("facts", &rule.digest, key));
                let tri = |v: u64| match v % 3 {
                    0 => Tri::Pass,
                    1 => Tri::Fail,
                    _ => Tri::Unknown,
                };
                PointFacts {
                    regular: tri(x),
                    rhs_top: if (x >> 8) % 4 == 0 { None } else { Some(self.w.rhs(&rule.digest, key)) },
                    exceptions_complete: tri(x >> 16),
                    note: None,
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProverMode {
    Truthful,
    UnknownAll,
    UnsupportedAll,
    Empty,
    NoneNone,
    WrongKey,
    LowerLevel,
    HigherWithFailure,
    Random,
}

pub struct Prover<'w> {
    pub w: &'w World,
    pub mode: ProverMode,
    pub calls: Rc<RefCell<Calls>>,
    /// (digest, key) -> best level GRANTED without a failure.
    pub granted: BTreeMap<(String, Key), EvidenceLevel>,
}

impl KeyProver for Prover<'_> {
    fn prove(&mut self, rule: &CandidateRule, keys: &[Key], wanted: EvidenceLevel) -> Vec<KeyEvidence> {
        {
            let mut c = self.calls.borrow_mut();
            c.prove += 1;
            // a GENUINE livelock: the same request (rule, keys) asked again and again with nothing in between
            let sig = format!("{}|{keys:?}", rule.digest);
            if c.last == sig {
                c.repeat += 1;
            } else {
                c.last = sig;
                c.repeat = 0;
            }
            if c.repeat >= 1000 {
                panic!("LIVELOCK: identical evidence request repeated {} times with no other call in between (rule {})", c.repeat, rule.digest);
            }
            if c.prove > c.cap.max(300_000) {
                panic!("LIVELOCK: prover calls above {}", c.cap.max(300_000));
            }
        }
        let truthful = |k: &Key| -> KeyEvidence {
            let ok = rule.domain.applies(k) == Verdict::Applies && self.w.truth(&rule.digest, k).valid;
            if ok {
                KeyEvidence { key: k.clone(), level: Some(wanted), failure: None }
            } else {
                KeyEvidence { key: k.clone(), level: None, failure: Some(ProofFailure::NonMembership) }
            }
        };
        let out: Vec<KeyEvidence> = match self.mode {
            ProverMode::Truthful => keys.iter().map(truthful).collect(),
            ProverMode::UnknownAll => keys
                .iter()
                .map(|k| KeyEvidence { key: k.clone(), level: None, failure: Some(ProofFailure::Unknown("?".into())) })
                .collect(),
            ProverMode::UnsupportedAll => keys
                .iter()
                .map(|k| KeyEvidence {
                    key: k.clone(),
                    level: None,
                    failure: Some(ProofFailure::Unsupported("symmetry rows".into())),
                })
                .collect(),
            ProverMode::Empty => vec![],
            ProverMode::NoneNone => keys.iter().map(|k| KeyEvidence { key: k.clone(), level: None, failure: None }).collect(),
            ProverMode::WrongKey => keys
                .iter()
                .map(|k| {
                    let mut k2 = k.clone();
                    k2[0] += 1;
                    KeyEvidence { key: k2, level: Some(EvidenceLevel::A4), failure: None }
                })
                .collect(),
            ProverMode::LowerLevel => keys
                .iter()
                .map(|k| KeyEvidence { key: k.clone(), level: Some(EvidenceLevel::A1), failure: None })
                .collect(),
            ProverMode::HigherWithFailure => keys
                .iter()
                .map(|k| KeyEvidence { key: k.clone(), level: Some(EvidenceLevel::A4), failure: Some(ProofFailure::Fail) })
                .collect(),
            ProverMode::Random => keys
                .iter()
                .map(|k| {
                    let x = hsh(self.w.seed, &("prove", &rule.digest, k));
                    match x % 5 {
                        0 => truthful(k),
                        1 => KeyEvidence { key: k.clone(), level: None, failure: Some(ProofFailure::Undecided("u".into())) },
                        2 => KeyEvidence { key: k.clone(), level: Some(EvidenceLevel::A0), failure: None },
                        3 => KeyEvidence { key: k.clone(), level: None, failure: Some(ProofFailure::NotInSpanModular) },
                        _ => KeyEvidence { key: k.clone(), level: Some(wanted), failure: Some(ProofFailure::Hidden) },
                    }
                })
                .collect(),
        };
        for e in &out {
            if e.failure.is_none()
                && let Some(l) = e.level
            {
                let ent = self.granted.entry((rule.digest.clone(), e.key.clone())).or_insert(l);
                if l > *ent {
                    *ent = l;
                }
            }
        }
        out
    }
}

// ------------------------------------------------------------------------------------------------ batch driver

/// A WAVE-PARALLEL tree driver mirroring `drive_tree`'s tree semantics (children, alternatives, reserve, served
/// set) but answering every attempt of a `Run` list before the next `next_steps`, in a shuffled completion order.
pub fn batch_tree(
    roots: Vec<Item>,
    params: &NodeParams,
    descent: &dyn Descent,
    solver: &mut dyn CaseSolver,
    facts: &mut dyn FactSource,
    prover: &mut dyn KeyProver,
    shuffle: Option<u64>,
    step_cap: u64,
) -> Result<Vec<(String, String)>, String> {
    // ported to the v1.2 driver contract: reuse the solver answers of an item id seen before (no attempt key run
    // twice, audit B M1), children inherit the reserve (no driver-side reserve), seal_tree at the end
    struct B<'a> {
        params: &'a NodeParams,
        descent: &'a dyn Descent,
        served: BTreeSet<Key>,
        out: Vec<rustred_campaign_core::repair::ItemReport>,
        seen: BTreeMap<String, usize>,
        rng: Option<Rng>,
        steps: u64,
        cap: u64,
    }
    fn process(
        b: &mut B,
        mut item: Item,
        solver: &mut dyn CaseSolver,
        facts: &mut dyn FactSource,
        prover: &mut dyn KeyProver,
    ) -> Result<(), String> {
        item.gap_points.retain(|k| !b.served.contains(k));
        item.weight = item.gap_points.len() as u64;
        if item.gap_points.is_empty() && item.kind != ItemKind::Optimise {
            return Ok(());
        }
        let mut st = ItemState::new(item);
        if let Some(&j) = b.seen.get(&st.item.id.0) {
            for (k, r) in &b.out[j].state.records {
                if r.is_verdict() || !r.non_verdict.is_empty() {
                    st.records.entry(k.clone()).or_insert_with(|| r.clone());
                }
            }
        }
        let idx = b.out.len();
        b.seen.insert(st.item.id.0.clone(), idx);
        b.out.push(rustred_campaign_core::repair::ItemReport { state: st.clone(), disposition: None });
        loop {
            b.steps += 1;
            if b.steps > b.cap {
                return Err(format!("batch driver: more than {} steps (livelock)", b.cap));
            }
            match next_steps(&st, b.params) {
                Next::Run(list) => {
                    let mut order: Vec<usize> = (0..list.len()).collect();
                    if let Some(r) = b.rng.as_mut() {
                        for i in (1..order.len()).rev() {
                            let j = r.below(i as u64 + 1) as usize;
                            order.swap(i, j);
                        }
                    }
                    for i in order {
                        let a = &list[i];
                        let ans = solver.solve(&CaseRequest::from_attempt(&b.params.node, a), &a.budget);
                        st.record_answer(a, ans);
                    }
                }
                Next::NeedVerdicts(reqs) => {
                    for r in reqs {
                        let cand = st.records[&r.attempt_key].candidates()[r.index].clone();
                        let (v, hits) = judge_points_with_hits(&cand, &r.keys, facts, b.descent, &b.params.preferred, &b.params.lines);
                        st.record_verdicts(&r.attempt_key, r.index, v);
                        st.record_preferred_hits(&r.attempt_key, r.index, hits);
                    }
                }
                Next::NeedEvidence(reqs) => {
                    for r in reqs {
                        let cand = st.records[&r.attempt_key].candidates()[r.index].clone();
                        let mut ev = prover.prove(&cand, &r.keys, r.level);
                        ev.retain(|e| r.keys.contains(&e.key));
                        for k in &r.keys {
                            if !ev.iter().any(|e| &e.key == k) {
                                ev.push(KeyEvidence { key: k.clone(), level: None, failure: Some(ProofFailure::Unknown("no answer".into())) });
                            }
                        }
                        st.record_evidence(&r.attempt_key, r.index, ev);
                    }
                }
                Next::NeedOffers => st.record_offers("none", vec![]),
                Next::NeedContainment(reqs) => {
                    for r in reqs {
                        st.record_containment(&r.attempt_key, r.index, Tri::Unknown);
                    }
                }
                Next::Done(d) => {
                    if let Disposition::Covered { cover } | Disposition::Split { cover, .. } = &d {
                        for m in &cover.members {
                            b.served.extend(m.served.iter().cloned());
                        }
                    }
                    let follow = match &d {
                        Disposition::Split { children, alternatives, .. } => Some((children.clone(), alternatives.clone())),
                        _ => None,
                    };
                    b.out[idx].state = st;
                    b.out[idx].disposition = Some(d);
                    if let Some((children, alternatives)) = follow {
                        for c in children {
                            process(b, c, solver, facts, prover)?;
                        }
                        for c in alternatives {
                            process(b, c, solver, facts, prover)?;
                        }
                    }
                    return Ok(());
                }
            }
        }
    }
    let mut b = B { params, descent, served: BTreeSet::new(), out: vec![], seen: BTreeMap::new(), rng: shuffle.map(Rng::new), steps: 0, cap: step_cap };
    for r in roots {
        process(&mut b, r, solver, facts, prover)?;
    }
    let served = b.served.clone();
    let mut items = b.out;
    for r in &mut items {
        if let Some(Disposition::TerminalCandidates { keys, .. }) = &mut r.disposition {
            keys.retain(|k| !served.contains(k));
        }
    }
    rustred_campaign_core::repair::seal_tree(&mut items, params);
    Ok(items.into_iter().map(|r| (r.state.item.id.0.clone(), format!("{:?}", r.disposition))).collect())
}

pub fn report_signature(rep: &TreeReport) -> Vec<(String, String)> {
    rep.items
        .iter()
        .map(|r| (r.state.item.id.0.clone(), format!("{:?}", r.disposition)))
        .collect()
}

// ------------------------------------------------------------------------------------------------ invariants

#[derive(Default, Debug, Clone)]
pub struct Stat {
    pub runs: u64,
    pub violations: u64,
    pub examples: Vec<String>,
}
impl Stat {
    fn fail(&mut self, ex: String) {
        self.violations += 1;
        if self.examples.len() < 6 {
            self.examples.push(ex);
        }
    }
}

#[derive(Default, Debug)]
pub struct Tally {
    pub s: BTreeMap<String, Stat>,
    pub counts: BTreeMap<String, u64>,
    pub maxes: BTreeMap<String, f64>,
}
impl Tally {
    pub fn run(&mut self, k: &str) {
        self.s.entry(k.into()).or_default().runs += 1;
    }
    pub fn fail(&mut self, k: &str, ex: String) {
        self.s.entry(k.into()).or_default().fail(ex);
    }
    pub fn count(&mut self, k: &str, n: u64) {
        *self.counts.entry(k.into()).or_default() += n;
    }
    pub fn max(&mut self, k: &str, v: f64) {
        let e = self.maxes.entry(k.into()).or_insert(f64::MIN);
        if v > *e {
            *e = v;
        }
    }
    /// Regression gate: every invariant whose name starts with one of `prefixes` has zero violations.
    pub fn assert_zero(&self, title: &str, prefixes: &[&str]) {
        let bad: Vec<String> = self
            .s
            .iter()
            .filter(|(k, s)| s.violations > 0 && prefixes.iter().any(|p| k.starts_with(p)))
            .map(|(k, s)| format!("{k}: {} violations, e.g. {:?}", s.violations, s.examples.first()))
            .collect();
        assert!(bad.is_empty(), "{title}: {bad:#?}");
    }
    pub fn print(&self, title: &str) {
        eprintln!("==== {title}");
        for (k, s) in &self.s {
            eprintln!("  {k:<48} runs {:>8} violations {:>6}", s.runs, s.violations);
            eprintln!("STAT|{title}|{k}|{}|{}", s.runs, s.violations);
            for e in &s.examples {
                eprintln!("      e.g. {e}");
            }
        }
        for (k, v) in &self.counts {
            eprintln!("  count {k:<42} {v}");
            eprintln!("COUNT|{title}|{k}|{v}");
        }
        for (k, v) in &self.maxes {
            eprintln!("  max   {k:<42} {v}");
            eprintln!("MAX|{title}|{k}|{v}");
        }
    }
}

pub fn candidate_of<'a>(st: &'a ItemState, key: &str, idx: usize) -> Option<&'a CandidateRule> {
    st.records.get(key)?.candidates().get(idx)
}

/// I1-I5 + counters on a finished tree report. `gap` = root gap keys after stripping preferred keys.
pub fn check_tree(
    t: &mut Tally,
    w: &World,
    params: &NodeParams,
    rep: &TreeReport,
    granted: &BTreeMap<(String, Key), EvidenceLevel>,
    gap: &[Key],
    tag: &str,
    strong_truth: bool,
) {
    let min = params.min_evidence;
    let wid = format!("{tag} seed={} n={} lines={:?}", w.seed, w.n, w.lines);
    // v1.2 (audit A M3): only HONOURED preferred keys (at most the bound, in key order) are allowed terminals
    let hon = params.preferred.honoured(&w.gap, &params.lines, params.terminals_max);
    // served map over the whole tree (final covers incl. reserve)
    let mut served_by: BTreeMap<Key, Vec<(usize, usize)>> = BTreeMap::new();
    for (ii, r) in rep.items.iter().enumerate() {
        if let Some(Disposition::Covered { cover } | Disposition::Split { cover, .. }) = &r.disposition {
            for (mi, m) in cover.members.iter().enumerate() {
                for k in &m.served {
                    served_by.entry(k.clone()).or_default().push((ii, mi));
                }
            }
        }
    }
    let terminals: Vec<(Key, _)> = rep.terminal_candidates();
    let term_set: BTreeSet<Key> = terminals.iter().map(|(k, _)| k.clone()).collect();
    // ---- I1 soundness of every served key; (b) nothing accepted that the prover did not justify
    t.run(&format!("I1 soundness [{tag}]"));
    for (ii, r) in rep.items.iter().enumerate() {
        let Some(Disposition::Covered { cover } | Disposition::Split { cover, .. }) = &r.disposition else {
            continue;
        };
        for m in &cover.members {
            let Some(c) = candidate_of(&r.state, &m.attempt_key, m.index) else {
                t.fail(&format!("I1 soundness [{tag}]"), format!("{wid}: member {} not in records", m.attempt_key));
                continue;
            };
            for k in m.served.iter().chain(m.credited.iter().filter(|k| !m.served.contains(k))) {
                t.count(&format!("served keys [{tag}]"), 1);
                if !r.state.item.gap_points.contains(k) {
                    t.fail(&format!("I1 soundness [{tag}]"), format!("{wid}: item {ii} serves {k:?} not among its gap points"));
                }
                if c.domain.applies(k) != Verdict::Applies {
                    t.fail(&format!("I1 soundness [{tag}]"), format!("{wid}: {} serves {k:?} outside its domain", c.digest));
                    continue;
                }
                if strong_truth {
                    let tr = w.truth(&c.digest, k);
                    if !tr.regular || tr.desc == Desc::No {
                        t.fail(&format!("I1 soundness [{tag}]"), format!("{wid}: {} serves {k:?} but truth {tr:?}", c.digest));
                    }
                    if min >= EvidenceLevel::A2 && !tr.valid {
                        t.fail(&format!("I1 soundness [{tag}]"), format!("{wid}: {} serves {k:?}, identity invalid", c.digest));
                    }
                }
                if min >= EvidenceLevel::A2 {
                    match granted.get(&(c.digest.clone(), k.clone())) {
                        Some(l) if *l >= min => {}
                        g => t.fail(
                            &format!("I1b evidence justified by prover [{tag}]"),
                            format!("{wid}: {} serves {k:?} at min {min:?}, prover granted {g:?}", c.digest),
                        ),
                    }
                }
            }
        }
    }
    t.run(&format!("I1b evidence justified by prover [{tag}]"));
    // ---- I3 accounting
    t.run(&format!("I3 accounting [{tag}]"));
    for k in gap {
        let ns = served_by.get(k).map_or(0, |v| v.len());
        let term = term_set.contains(k);
        if ns > 1 {
            t.fail(&format!("I3 accounting [{tag}]"), format!("{wid}: {k:?} served {ns} times"));
        } else if ns == 1 && term {
            t.fail(&format!("I3 accounting [{tag}]"), format!("{wid}: {k:?} served and terminal"));
        } else if ns == 0 && !term {
            let open = rep.stopped.is_some()
                || rep.items.iter().any(|r| {
                    r.state.item.gap_points.contains(k)
                        && matches!(r.disposition, None | Some(Disposition::Attention { .. }))
                });
            if open {
                t.count(&format!("open keys (attention/stopped) [{tag}]"), 1);
            } else {
                t.fail(&format!("I3 accounting [{tag}]"), format!("{wid}: {k:?} LOST (neither served, terminal nor open)"));
            }
        }
        if ns == 1 {
            t.count(&format!("disposition served [{tag}]"), 1);
        } else if term {
            t.count(&format!("disposition terminal [{tag}]"), 1);
        }
    }
    // no preferred key is ever served
    for k in served_by.keys() {
        if hon.contains(k) {
            t.fail(&format!("P4 preferred never reduced [{tag}]"), format!("{wid}: preferred {k:?} served"));
        }
    }
    // ---- I2 whole written domain over the root key range
    let i2_before = t.counts.get(&format!("I2 same-cover shadow, bad first [{tag}]")).copied().unwrap_or(0);
    if strong_truth {
        t.run(&format!("I2 domain safety [{tag}]"));
        for (ii, r) in rep.items.iter().enumerate() {
            let Some(d) = &r.disposition else { continue };
            let (cover, kids): (_, Vec<&Item>) = match d {
                Disposition::Covered { cover } => (cover, vec![]),
                Disposition::Split { cover, children, alternatives, .. } => {
                    (cover, children.iter().chain(alternatives).collect())
                }
                _ => continue,
            };
            for (mi, m) in cover.members.iter().enumerate() {
                if m.served.is_empty() {
                    continue;
                }
                let Some(c) = candidate_of(&r.state, &m.attempt_key, m.index) else { continue };
                for k in gap {
                    // v1.2: a member is dispatched only on its EFFECTIVE domain (written domain minus exclusions);
                    // where it is credited, I1 checks it
                    if c.domain.applies(k) != Verdict::Applies
                        || m.credited.contains(k)
                        || m.exclusions.iter().any(|e| e.target.holds(k))
                    {
                        continue;
                    }
                    let tr = w.truth(&c.digest, k);
                    let bad = !tr.regular || tr.desc == Desc::No || (min >= EvidenceLevel::A2 && !tr.valid);
                    if !bad {
                        continue;
                    }
                    let what = if !tr.regular { "pole" } else if tr.desc == Desc::No { "non-descending" } else { "invalid" };
                    let servers = served_by.get(k).cloned().unwrap_or_default();
                    if let Some(&(si, smi)) = servers.first() {
                        if si == ii {
                            let class = if smi > mi { "SAME COVER, bad member FIRST" } else { "same cover, bad member later" };
                            t.fail(
                                &format!("I2 domain safety [{tag}]"),
                                format!("{wid}: item {ii} member {mi} ({}) {what} at {k:?}, served by member {smi}: {class}; no exclusion emitted", c.digest),
                            );
                            t.count(&format!("I2 same-cover shadow, bad first [{tag}]"), u64::from(smi > mi));
                            t.count(&format!("I2 same-cover shadow, bad later [{tag}]"), u64::from(smi < mi));
                        } else if si < ii {
                            t.count(&format!("I2 key served by an EARLIER item (safe under D6 order) [{tag}]"), 1);
                        } else if kids.iter().any(|ch| ch.gap_points.contains(k)) {
                            t.count(&format!("I2 key in emitted child, served later (needs D6 exclusion) [{tag}]"), 1);
                        } else {
                            t.count(&format!("I2 key served by a later non-child item [{tag}]"), 1);
                        }
                    } else if kids.iter().any(|ch| ch.gap_points.contains(k)) {
                        t.count(&format!("I2 key in emitted child, unserved [{tag}]"), 1);
                    } else if r.state.item.gap_points.contains(k) {
                        t.fail(
                            &format!("I2 domain safety [{tag}]"),
                            format!("{wid}: item {ii} member {} {what} at its gap point {k:?}: no child, no exclusion, not served", c.digest),
                        );
                    } else {
                        t.count(&format!("I2 key outside item gap points, unserved (export must exclude) [{tag}]"), 1);
                    }
                }
            }
        }
    }
    if t.counts.get(&format!("I2 same-cover shadow, bad first [{tag}]")).copied().unwrap_or(0) > i2_before {
        t.count(&format!("worlds with a same-cover shadow, bad member first [{tag}]"), 1);
    }
    // ---- I4 terminals
    t.run(&format!("I4 terminals [{tag}]"));
    for (k, tok) in &terminals {
        if hon.contains(k) {
            t.fail(&format!("I4 terminals [{tag}]"), format!("{wid}: preferred {k:?} proposed after ladder effort"));
        }
        if tok.key() != k.as_slice() && !rep.items.iter().any(|r| matches!(&r.disposition, Some(Disposition::TerminalCandidates{keys, exhausted}) if keys.contains(k) && exhausted == tok)) {
            t.fail(&format!("I4 terminals [{tag}]"), format!("{wid}: token key {:?} vs {k:?}", tok.key()));
        }
        if !tok.depth3_unfloored_tried() {
            t.count(&format!("I4 token without depth-3 unfloored (add_by_rule refuses) [{tag}]"), 1);
        }
        if strong_truth {
            // an acceptable candidate at k inside a visited item that did not reject it for a hidden pole
            for r in &rep.items {
                if !r.state.item.gap_points.contains(k) {
                    continue;
                }
                for (ak, rec) in &r.state.records {
                    for (ci, c) in rec.candidates().iter().enumerate() {
                        if !w.acceptable(c, k, min) {
                            continue;
                        }
                        // v1.2 (addendum 3): a rule found WITH symmetry rows is acceptable only if the caller's
                        // provers can establish the required level for such rules ("awaiting a certificate route")
                        if (c.symmetry_rows || rec.setting.symmetry_rows) && !params.capabilities.can(true, min) {
                            t.count(&format!("I4 terminal: candidate awaiting a certificate route (symmetry rows) [{tag}]"), 1);
                            continue;
                        }
                        let hidden = r
                            .state
                            .verdicts
                            .get(&(ak.clone(), ci))
                            .is_some_and(|v| v.iter().any(|x| matches!(x, PointVerdict::HiddenPole)));
                        if hidden {
                            t.count(&format!("I5 terminal: acceptable rule rejected for hidden pole elsewhere [{tag}]"), 1);
                        } else {
                            t.fail(
                                &format!("I4 terminals [{tag}]"),
                                format!("{wid}: terminal {k:?} but candidate {} of item {} acceptable there", c.digest, r.state.item.id),
                            );
                        }
                    }
                }
            }
            if w.irreducible.contains(k) {
                t.count(&format!("I5 terminal truly irreducible [{tag}]"), 1);
            } else {
                t.count(&format!("I5 terminal NOT irreducible (missed closure) [{tag}]"), 1);
                // classify: budget (a failure answer where the pure oracle had an acceptable rule)
                let mut why = "no acceptable rule in visited spaces";
                for r in &rep.items {
                    if !r.state.item.gap_points.contains(k) {
                        continue;
                    }
                    for (_ak, rec) in &r.state.records {
                        let failed = !rec.non_verdict.is_empty();
                        if failed
                            && w.rules_for_case(&r.state.item.case)
                                .iter()
                                .any(|h| h.req.matches(&rec.setting) && w.acceptable(&w.candidate(h), k, min))
                        {
                            why = "budget/failure answer hid an acceptable rule";
                        }
                    }
                }
                t.count(&format!("I5 missed closure: {why} [{tag}]"), 1);
            }
        }
    }
    // bound: never silently truncated
    let mut fb = FallbackSet::new(params.terminals_max);
    let mut over = 0usize;
    for (k, tok) in &terminals {
        match fb.add_by_rule(k, tok) {
            Ok(_) => {}
            Err(TerminalError::OverBound { .. }) => over += 1,
            Err(TerminalError::NotDeepTried) => {}
            Err(e) => t.fail(&format!("I4 terminals [{tag}]"), format!("{wid}: add_by_rule {e:?}")),
        }
    }
    let flags = check_terminals(
        &terminals.iter().map(|(k, x)| (k.clone(), Some(x.clone()))).collect::<Vec<_>>(),
        params.terminals_max,
        &rustred_campaign_core::repair::NoTables,
        &params.preferred,
        &params.lines,
    );
    let term_keys: Vec<Key> = terminals.iter().map(|(k, _)| k.clone()).collect();
    let hon_t = params.preferred.honoured(&term_keys, &params.lines, params.terminals_max);
    let counted = terminals.iter().filter(|(k, _)| !hon_t.contains(k)).count();
    let flagged = flags.iter().any(|f| matches!(f, rustred_campaign_core::terminals::TerminalFlag::OverBound { .. }));
    if (counted > params.terminals_max) != flagged || (over > 0) != (fb.len() >= params.terminals_max && terminals.len() > params.terminals_max) {
        t.fail(&format!("I4 terminals [{tag}]"), format!("{wid}: bound {} terminals {} over {over} flagged {flagged}", params.terminals_max, terminals.len()));
    }
    if over > 0 {
        t.count(&format!("I4 over bound: refused by FallbackSet (reported, not truncated) [{tag}]"), 1);
    }
    // ---- counters
    t.run(&format!("counters [{tag}]"));
    let mut seen_keys: BTreeMap<String, u32> = BTreeMap::new();
    for r in &rep.items {
        let st = &r.state;
        let sc = sub_counter(st, params) as u64;
        let answered = st.records.iter().filter(|(k, r)| r.is_verdict() && !k.contains("|offer|")).count() as u64;
        let op: u64 = st.records.values().map(|r| r.operator_interrupts as u64).sum();
        let inf: u64 = st.records.values().map(|r| r.infra_interrupts as u64).sum();
        let max_inf = st.records.values().map(|r| r.infra_interrupts).max().unwrap_or(0);
        if answered > sc {
            t.fail(&format!("counters [{tag}]"), format!("{wid}: item {} answered {answered} > sub-counter {sc}", st.item.id));
        }
        if max_inf > params.budgets.infra_retries + 1 {
            t.fail(&format!("counters [{tag}]"), format!("{wid}: infra interrupts {max_inf} on one key"));
        }
        t.max(&format!("attempts/item - sub_counter [{tag}]"), answered as f64 - sc as f64);
        t.max(&format!("solver calls per item (incl. interrupts) [{tag}]"), (answered + op + inf) as f64);
        t.max(&format!("sub_counter per item [{tag}]"), sc as f64);
        if st.item.depth > st.item.root_dim {
            t.fail(&format!("counters [{tag}]"), format!("{wid}: depth {} > root dim {}", st.item.depth, st.item.root_dim));
        }
        if st.item.kind == ItemKind::BoxRestricted || st.item.box_id.is_some() {
            t.fail(&format!("R35 no box item without user box [{tag}]"), format!("{wid}: item {} kind {:?}", st.item.id, st.item.kind));
        }
        for (k, rec) in &st.records {
            if rec.is_verdict() && !k.contains("|offer|") {
                *seen_keys.entry(k.clone()).or_default() += 1;
            }
        }
        // explain must not panic
        let e = explain(st, params);
        let _ = e.to_text();
        // dimension of a "point" item: free slots left although dim()==0 (dimension counted wrongly)
        if st.item.case.is_point() && !st.item.case.free().is_empty() {
            t.count(&format!("POINT item with free slots left (dim miscounted) [{tag}]"), 1);
        }
        if let Some(p) = &st.item.parent
            && let Some(pr) = rep.items.iter().find(|x| &x.state.item.id == p)
            && st.item.case.dim() >= pr.state.item.case.dim()
        {
            t.fail(&format!("counters [{tag}]"), format!("{wid}: child dim {} >= parent dim {}", st.item.case.dim(), pr.state.item.case.dim()));
        }
        for g in &st.item.gap_points {
            if !st.item.case.contains(g, &params.lines) {
                t.count(&format!("item gap point outside its own case [{tag}]"), 1);
            }
        }
    }
    let dup = seen_keys.values().filter(|&&c| c > 1).count() as u64;
    t.count(&format!("attempt keys answered twice in one tree (same item id, new state) [{tag}]"), dup);
    let total: u64 = seen_keys.values().map(|&c| c as u64).sum();
    let wasted: u64 = seen_keys.values().map(|&c| c as u64 - 1).sum();
    t.count(&format!("solver answers total [{tag}]"), total);
    t.count(&format!("solver answers spent re-running an already answered attempt key [{tag}]"), wasted);
    let mut ids: BTreeMap<&str, u32> = BTreeMap::new();
    for r in &rep.items {
        *ids.entry(r.state.item.id.0.as_str()).or_default() += 1;
    }
    let dup_items = ids.values().filter(|&&c| c > 1).count() as u64;
    t.count(&format!("item ids processed more than once in one tree [{tag}]"), dup_items);
    if wasted > 0 {
        t.count(&format!("trees with re-run attempt keys [{tag}]"), 1);
    }
    t.max(&format!("items per tree [{tag}]"), rep.items.len() as f64);
}

/// I6 (economy): a ONE-CASE cover of a root face exists in its seed space (per the pure oracle) => the root item ends
/// Covered with exactly one member.
pub fn check_economy(t: &mut Tally, w: &World, params: &NodeParams, rep: &TreeReport, roots: &[Item], tag: &str) {
    for root in roots {
        if root.gap_points.is_empty() {
            continue;
        }
        t.run(&format!("I6 economy [{tag}]"));
        let mut exists: Option<(String, String)> = None;
        for wave in seed_space(&root.case, params) {
            for s in wave {
                if let SolveOutcome::Rules(v) = w.answer(&root.case, &s).outcome {
                    for c in v {
                        if root.gap_points.iter().all(|k| w.acceptable(&c, k, params.min_evidence)) {
                            exists.get_or_insert((c.digest.clone(), s.label()));
                        }
                    }
                }
            }
        }
        let Some(r) = rep.items.iter().find(|r| r.state.item.id == root.id) else { continue };
        let one = matches!(&r.disposition, Some(Disposition::Covered { cover }) if cover.members.iter().filter(|m| !m.served.is_empty()).count() == 1);
        if let Some((d, s)) = exists {
            t.count(&format!("I6 faces with a one-case cover [{tag}]"), 1);
            if !one {
                t.fail(
                    &format!("I6 economy [{tag}]"),
                    format!("seed={} face {}: one-case {d} @ {s} exists, got {:?}", w.seed, root.case.face.label(), r.disposition.as_ref().map(disp_short)),
                );
            }
        }
    }
}

pub fn disp_short(d: &Disposition) -> String {
    match d {
        Disposition::Covered { cover } => format!("Covered({} members)", cover.members.len()),
        Disposition::Split { cover, children, alternatives, reserve } => format!(
            "Split({} members, {} children, {} alt, {} reserve)",
            cover.members.len(),
            children.len(),
            alternatives.len(),
            reserve.len()
        ),
        Disposition::TerminalCandidates { keys, .. } => format!("Terminal({})", keys.len()),
        Disposition::Attention { why, .. } => format!("Attention({why})"),
        Disposition::OptimiseKept { .. } => "OptimiseKept".into(),
        Disposition::RecipeRule { .. } => "RecipeRule".into(),
    }
}

pub fn calls(cap: u64) -> Rc<RefCell<Calls>> {
    Rc::new(RefCell::new(Calls { last: String::new(), repeat: 0, solve: 0, prove: 0, facts: 0, cap }))
}

pub fn roots_of(w: &World, params: &NodeParams) -> Vec<Item> {
    items_from_closure_keys(&w.gap, params).expect("closure items")
}

pub fn n_worlds() -> u64 {
    std::env::var("AUDIT_WORLDS").ok().and_then(|s| s.parse().ok()).unwrap_or(300)
}

fn run_drive(
    w: &World,
    params: &NodeParams,
    roots: Vec<Item>,
    sm: SolverMode,
    fm: FactMode,
    pm: ProverMode,
    p_op: u64,
    p_inf: u64,
    limits: &TreeLimits,
    cap: u64,
) -> Result<(TreeReport, BTreeMap<(String, Key), EvidenceLevel>, Rc<RefCell<Calls>>), String> {
    let c = calls(cap);
    let res = catch_unwind(AssertUnwindSafe(|| {
        let mut solver = Solver { w, mode: sm, calls: c.clone(), per_request: BTreeMap::new(), p_operator: p_op, p_infra: p_inf, rng: Rng::new(w.seed ^ 77) };
        let mut facts = Facts { w, mode: fm, calls: c.clone() };
        let mut prover = Prover { w, mode: pm, calls: c.clone(), granted: BTreeMap::new() };
        let rep = drive_tree(roots, params, &w.cmp, &mut solver, &mut facts, &mut prover, None, None, limits);
        (rep, prover.granted)
    }));
    match res {
        Ok((rep, g)) => Ok((rep, g, c)),
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            Err(msg)
        }
    }
}

// ------------------------------------------------------------------------------------------------ PART 1

#[test]
fn part1_s0() { part1_truthful_worlds_invariants(0, 4) }
#[test]
fn part1_s1() { part1_truthful_worlds_invariants(1, 4) }
#[test]
fn part1_s2() { part1_truthful_worlds_invariants(2, 4) }
#[test]
fn part1_s3() { part1_truthful_worlds_invariants(3, 4) }

fn part1_truthful_worlds_invariants(shard: u64, shards: u64) {
    let nw = n_worlds();
    let mut t = Tally::default();
    for seed in (0..nw).filter(|s| s % shards == shard) {
        let w = World::generate(seed);
        let mut r = Rng::new(seed ^ 0xABCD);
        let params = w.params(&mut r);
        let roots = roots_of(&w, &params);
        let gap: Vec<Key> = roots.iter().flat_map(|i| i.gap_points.clone()).collect();
        let lim = TreeLimits::default();
        match run_drive(&w, &params, roots.clone(), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &lim, 2_000_000) {
            Ok((rep, granted, c)) => {
                check_tree(&mut t, &w, &params, &rep, &granted, &gap, "truthful", true);
                // audit B M1 (v1.2): records of an item id seen before are REUSED, so the solver is called at most
                // once per answer counted in the records (max over the items sharing an attempt key)
                let mut per_key: BTreeMap<String, u64> = BTreeMap::new();
                for it in &rep.items {
                    for (k, rec) in &it.state.records {
                        if k.contains("|offer|") {
                            continue;
                        }
                        let n = u64::from(rec.is_verdict()) + rec.non_verdict.len() as u64 + rec.infra_interrupts as u64 + rec.operator_interrupts as u64;
                        let e = per_key.entry(k.clone()).or_insert(0);
                        *e = (*e).max(n);
                    }
                }
                let expected: u64 = per_key.values().sum();
                let made = c.borrow().solve;
                t.count("M1 solver calls beyond the distinct attempt answers", made.saturating_sub(expected));
                if made > expected {
                    let mut dup: BTreeMap<String, u32> = BTreeMap::new();
                    for it in &rep.items {
                        for k in it.state.records.keys() {
                            *dup.entry(k.clone()).or_default() += 1;
                        }
                    }
                    eprintln!("M1 seed={seed}: {made} calls, {expected} answers; items {}; stopped {:?}", rep.items.len(), rep.stopped);
                }
                check_economy(&mut t, &w, &params, &rep, &roots, "truthful");
                t.max("solver calls per world [truthful]", c.borrow().solve as f64);
                // I7 determinism: same world, same seed -> identical report
                t.run("I7 determinism (rerun identical)");
                let (rep2, _, _) = run_drive(&w, &params, roots.clone(), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &lim, 2_000_000).unwrap();
                if report_signature(&rep) != report_signature(&rep2) || format!("{:?}", rep.attempts) != format!("{:?}", rep2.attempts) {
                    t.fail("I7 determinism (rerun identical)", format!("seed={seed}"));
                }
                // I7 interrupts: random operator interrupts do not change dispositions, cannot livelock
                t.run("I7/P6 operator interrupts: same dispositions");
                match run_drive(&w, &params, roots.clone(), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 300, 0, &lim, 2_000_000) {
                    Ok((rep3, _, c3)) => {
                        if report_signature(&rep) != report_signature(&rep3) {
                            t.fail("I7/P6 operator interrupts: same dispositions", format!("seed={seed}"));
                        }
                        t.max("solver calls with 30% operator interrupts / without", c3.borrow().solve as f64 / c.borrow().solve.max(1) as f64);
                    }
                    Err(e) => t.fail("I7/P6 operator interrupts: same dispositions", format!("seed={seed}: {e}")),
                }
                // infra interrupts: either same dispositions or an explicit Attention (never silent)
                t.run("P6 infra interrupts: same or Attention");
                if let Ok((rep4, _, _)) = run_drive(&w, &params, roots.clone(), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 400, &lim, 2_000_000) {
                    let att = rep4.items.iter().any(|r| matches!(r.disposition, Some(Disposition::Attention { .. })));
                    if att {
                        t.count("infra interrupts -> Attention (cap reached)", 1);
                    } else if report_signature(&rep) != report_signature(&rep4) {
                        t.fail("P6 infra interrupts: same or Attention", format!("seed={seed}"));
                    }
                }
                // I7 batch (wave-parallel, shuffled) vs sequential with an unbounded economy extra budget
                let mut p_inf = params.clone();
                p_inf.budgets.economy_extra_factor = 1e30;
                let (rep_seq, _, _) = run_drive(&w, &p_inf, roots.clone(), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &lim, 2_000_000).unwrap();
                let c5 = calls(2_000_000);
                let mk = |sh: Option<u64>| {
                    let mut solver = Solver { w: &w, mode: SolverMode::Truthful, calls: c5.clone(), per_request: BTreeMap::new(), p_operator: 0, p_infra: 0, rng: Rng::new(1) };
                    let mut facts = Facts { w: &w, mode: FactMode::Truthful, calls: c5.clone() };
                    let mut prover = Prover { w: &w, mode: ProverMode::Truthful, calls: c5.clone(), granted: BTreeMap::new() };
                    batch_tree(roots.clone(), &p_inf, &w.cmp, &mut solver, &mut facts, &mut prover, sh, 1_000_000)
                };
                t.run("I7 batch(shuffled) == batch(in order)");
                t.run("I7 batch == sequential (unbounded extra budget)");
                match (mk(None), mk(Some(seed * 31 + 7))) {
                    (Ok(a), Ok(b)) => {
                        if a != b {
                            t.fail("I7 batch(shuffled) == batch(in order)", format!("seed={seed}"));
                        }
                        if a != report_signature(&rep_seq) {
                            t.fail("I7 batch == sequential (unbounded extra budget)", format!("seed={seed}"));
                        }
                        t.run("I7 batch == sequential (DEFAULT extra budget 2.0)");
                        if a != report_signature(&rep) {
                            t.count("batch != sequential under default economy budget (documented)", 1);
                        }
                    }
                    (a, b) => t.fail("I7 batch(shuffled) == batch(in order)", format!("seed={seed}: {:?} {:?}", a.err(), b.err())),
                }
            }
            Err(e) => t.fail("PANIC / LIVELOCK [truthful]", format!("seed={seed}: {e}")),
        }
    }
    t.print(&format!("PART 1 (shard {shard}/{shards} of {nw} truthful worlds)"));
    let crit: u64 = t
        .s
        .iter()
        .filter(|(k, _)| k.starts_with("I1") || k.starts_with("I3") || k.starts_with("PANIC") || k.starts_with("counters"))
        .map(|(_, s)| s.violations)
        .sum();
    eprintln!("PART1 critical violations: {crit}");
    t.assert_zero(
        "PART 1",
        &["I1", "I2", "I3", "I4", "PANIC", "counters", "R35", "P4", "I7 determinism", "I7 batch(shuffled)", "I7 batch == sequential (unbounded", "I7/P6", "P6"],
    );
    assert_eq!(t.counts.get("M1 solver calls beyond the distinct attempt answers").copied().unwrap_or(0), 0, "audit B M1");
}

// ------------------------------------------------------------------------------------------------ PART 2

#[test]
fn part2_s0() { part2_adversarial_traits(0, 4) }
#[test]
fn part2_s1() { part2_adversarial_traits(1, 4) }
#[test]
fn part2_s2() { part2_adversarial_traits(2, 4) }
#[test]
fn part2_s3() { part2_adversarial_traits(3, 4) }

fn part2_adversarial_traits(shard: u64, shards: u64) {
    let nw = n_worlds();
    let mut t = Tally::default();
    let solver_modes = [
        SolverMode::ForeignDomain,
        SolverMode::WrongArity,
        SolverMode::SameRuleForever,
        SolverMode::RegenExceptions,
        SolverMode::Alternating,
        SolverMode::AllFailures,
        SolverMode::Garbage,
    ];
    let prover_modes = [
        ProverMode::Truthful,
        ProverMode::UnknownAll,
        ProverMode::UnsupportedAll,
        ProverMode::Empty,
        ProverMode::NoneNone,
        ProverMode::WrongKey,
        ProverMode::LowerLevel,
        ProverMode::HigherWithFailure,
        ProverMode::Random,
    ];
    let fact_modes = [FactMode::Truthful, FactMode::AllUnknown, FactMode::Random];
    let mut runs = 0u64;
    let off: u64 = std::env::var("AUDIT_SEED0").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    for seed in (off..off + nw * 5).filter(|s| s % shards == shard) {
        let w = World::generate(seed);
        let mut r = Rng::new(seed ^ 0x1234);
        let mut params = w.params(&mut r);
        if params.min_evidence < EvidenceLevel::A2 && r.chance(700) {
            params.min_evidence = EvidenceLevel::A2;
        }
        let roots = roots_of(&w, &params);
        let gap: Vec<Key> = roots.iter().flat_map(|i| i.gap_points.clone()).collect();
        let sm = if r.chance(250) { SolverMode::Truthful } else { r.pick(&solver_modes) };
        let pm = r.pick(&prover_modes);
        let fm = if r.chance(600) { FactMode::Truthful } else { r.pick(&fact_modes) };
        let tag = format!("{sm:?}/{pm:?}");
        let lim = TreeLimits {
            max_attempts: 20_000, max_cpu_ms: u64::MAX, max_items: 100_000, max_steps: 400_000 };
        runs += 1;
        t.run("P2 terminates without panic");
        match run_drive(&w, &params, roots.clone(), sm, fm, pm, if r.chance(200) { 200 } else { 0 }, 0, &lim, 5_000) {
            Ok((rep, granted, c)) => {
                // (b): nothing accepted that the prover did not justify (min >= A2); soundness wrt truth only for
                // truthful facts (facts are trusted input)
                // truth-based checks only where facts AND prover are truthful (they are trusted inputs); the
                // prover-justification check (I1b) and accounting run in every mode
                let tg = if std::env::var("AUDIT_TAG_MODE").is_ok() { format!("adv/{sm:?}") } else { "adversarial".to_string() };
                check_tree(&mut t, &w, &params, &rep, &granted, &gap, &tg, fm == FactMode::Truthful && pm == ProverMode::Truthful);
                t.max(&format!("solver calls per world [{sm:?}]"), c.borrow().solve as f64);
                for it in &rep.items {
                    if let Some(Disposition::TerminalCandidates { keys, .. }) = &it.disposition
                        && keys.len() > 1
                    {
                        t.count(&format!("TerminalCandidates with >1 key (one token) [{sm:?}]"), 1);
                    }
                    if it.state.item.case.is_point() && !it.state.item.case.free().is_empty() {
                        t.count(&format!("POINT item with free slots [{sm:?}]"), 1);
                    }
                }
                let _ = tag;
            }
            Err(e) => {
                t.fail("P2 terminates without panic", format!("seed={seed} solver={sm:?} prover={pm:?} facts={fm:?}: {e}"));
                t.count(&format!("P2 failure with prover {pm:?}"), 1);
                t.count(&format!("P2 failure with solver {sm:?}"), 1);
            }
        }
    }
    t.print(&format!("PART 2 (shard {shard}/{shards}): {runs} adversarial runs"));
    t.assert_zero("PART 2", &["P2", "I1", "I2", "I3", "I4", "counters", "R35", "P4"]);
    for (k, v) in &t.counts {
        if k.starts_with("TerminalCandidates with >1 key") {
            assert_eq!(*v, 0, "audit B m1: {k}");
        }
    }
}

// ------------------------------------------------------------------------------------------------ PART 3

struct RandCover {
    seed: u64,
    p_cov: u64,
    p_und: u64,
}
impl Coverage for RandCover {
    fn verdict(&self, key: &[i32]) -> CoverageVerdict {
        let x = hsh(self.seed, &("cov", key)) % 1000;
        if x < self.p_cov {
            CoverageVerdict::Covered { rule: "tbl#1".into() }
        } else if x < self.p_cov + self.p_und {
            CoverageVerdict::Undecided { why: "blocked".into() }
        } else {
            CoverageVerdict::Uncovered
        }
    }
}

#[test]
fn part3_repair_key() {
    let nw = n_worlds();
    let mut t = Tally::default();
    for seed in 0..nw * 2 {
        let w = World::generate(seed);
        let mut r = Rng::new(seed ^ 0x77);
        let mut params = w.params(&mut r);
        let mut key = r.pick(&w.gap);
        match r.below(10) {
            0 => key.push(1),                                    // wrong arity
            1 => params.preferred.entries.push(PreferredEntry::Key(key.clone())),
            2 => key[0] = if w.lines[0] { 0 } else { 1 },        // not a top chart key
            _ => {}
        }
        let extra: Vec<Key> = w.gap.iter().filter(|_| r.chance(300)).cloned().collect();
        let tables = RandCover { seed, p_cov: [0, 100, 300][r.below(3) as usize], p_und: [0, 50][r.below(2) as usize] };
        let budget = RepairBudget {
            max_attempts: r.range(0, 80) as u32,
            max_cpu_ms: [u64::MAX, 20_000, 2_000][r.below(3) as usize],
            max_items: r.range(1, 40) as u32,
            max_steps: 20_000,
        };
        let pm = if r.chance(800) { ProverMode::Truthful } else { ProverMode::Random };
        let sm = if r.chance(800) { SolverMode::Truthful } else { SolverMode::Garbage };
        let c = calls(1_000_000);
        let res = catch_unwind(AssertUnwindSafe(|| {
            let mut solver = Solver { w: &w, mode: sm, calls: c.clone(), per_request: BTreeMap::new(), p_operator: 0, p_infra: 0, rng: Rng::new(seed) };
            let mut facts = Facts { w: &w, mode: FactMode::Truthful, calls: c.clone() };
            let mut prover = Prover { w: &w, mode: pm, calls: c.clone(), granted: BTreeMap::new() };
            let inputs = RepairInputs { params: &params, descent: &w.cmp, key: key.clone(), extra_targets: extra.clone(), tables: &tables };
            let out = repair_key(&inputs, &budget, &mut solver, &mut facts, &mut prover, None);
            (out, prover.granted)
        }));
        t.run("P3 repair_key");
        let (out, granted) = match res {
            Ok(x) => x,
            Err(_) => {
                t.fail("P3 repair_key", format!("seed={seed}: PANIC"));
                continue;
            }
        };
        let calls_made = c.borrow().solve;
        t.run("P3 budget: solver calls <= max_attempts");
        if calls_made > budget.max_attempts as u64 {
            t.fail("P3 budget: solver calls <= max_attempts", format!("seed={seed}: {calls_made} > {}", budget.max_attempts));
        }
        let is_pref = key.len() == w.n && params.preferred.matches(&key, &params.lines);
        match &out {
            Ok(o) if o.already_covered => {
                t.count("P3 Ok(already covered)", 1);
                if !matches!(tables.verdict(&key), CoverageVerdict::Covered { .. }) {
                    t.fail("P3 repair_key", format!("seed={seed}: already_covered but table says otherwise"));
                }
            }
            Ok(o) => {
                t.count("P3 Ok(rules)", 1);
                if is_pref {
                    t.fail("P3 repair_key", format!("seed={seed}: preferred key repaired"));
                }
                let servers: Vec<_> = o.rules.iter().filter(|x| x.serves_key).collect();
                if servers.len() != 1 {
                    t.fail("P3 repair_key", format!("seed={seed}: {} rules serve the key", servers.len()));
                }
                for s in servers {
                    if !s.served.contains(&key) {
                        t.fail("P3 repair_key", format!("seed={seed}: serves_key but key not in served"));
                    }
                    let reg = w.registry.borrow();
                    let Some(hr) = reg.get(&s.digest) else {
                        t.fail("P3 repair_key", format!("seed={seed}: unknown digest {}", s.digest));
                        continue;
                    };
                    let cand = w.candidate(hr);
                    drop(reg);
                    if sm == SolverMode::Truthful && !w.acceptable(&cand, &key, params.min_evidence) {
                        t.fail("P3 WRONG RULE returned by repair_key", format!("seed={seed}: {} at {key:?}", s.digest));
                    }
                    if params.min_evidence >= EvidenceLevel::A2
                        && granted.get(&(s.digest.clone(), key.clone())).is_none_or(|l| *l < params.min_evidence)
                    {
                        t.fail("P3 WRONG RULE returned by repair_key", format!("seed={seed}: {} evidence not granted at {key:?}", s.digest));
                    }
                }
            }
            Err(RepairFailure::PreferredTerminal { .. }) => {
                t.count("P3 Err(PreferredTerminal)", 1);
                if !is_pref {
                    t.fail("P3 repair_key", format!("seed={seed}: PreferredTerminal for a non-preferred key"));
                }
                if calls_made > 0 {
                    t.fail("P3 repair_key", format!("seed={seed}: ladder effort on a preferred key"));
                }
            }
            Err(e) => {
                let tag = match e {
                    RepairFailure::InvalidKey { .. } => "InvalidKey",
                    RepairFailure::BudgetExhausted { .. } => "BudgetExhausted",
                    RepairFailure::NoRelation { .. } => "NoRelation",
                    RepairFailure::NotAccepted { .. } => "NotAccepted",
                    RepairFailure::Terminal { .. } => "Terminal",
                    _ => "other",
                };
                t.count(&format!("P3 Err({tag})"), 1);
                if is_pref {
                    t.fail("P3 repair_key", format!("seed={seed}: preferred key gave {tag}"));
                }
            }
        }
    }
    t.print("PART 3: repair_key");
    t.assert_zero("PART 3", &[""]);
}

// ------------------------------------------------------------------------------------------------ PART 4

#[test]
fn part4_preferred_terminals() {
    let nw = n_worlds();
    let mut t = Tally::default();
    for seed in 0..nw {
        let w = World::generate(seed);
        let mut r = Rng::new(seed ^ 0x99);
        let params0 = w.params(&mut r);
        let mut params = params0.clone();
        let kind = r.below(6);
        params.preferred.entries = match kind {
            0 => vec![],
            1 => w.gap.iter().map(|k| PreferredEntry::Key(k.clone())).collect(), // the whole gap set
            2 => vec![PreferredEntry::Key(vec![9; w.n + 1]), PreferredEntry::Key(vec![7; w.n]), PreferredEntry::Sector { lines: vec![w.n + 3] }],
            3 => w.gap.iter().filter(|_| r.chance(300)).map(|k| PreferredEntry::Key(k.clone())).collect(),
            4 => {
                // RHS keys of the root rules (inside accepted rules' domains / landing points)
                let mut v = vec![];
                for k in &w.gap {
                    for l in w.lower_strict(k) {
                        if r.chance(300) {
                            v.push(PreferredEntry::Key(l));
                        }
                    }
                }
                v
            }
            _ => vec![PreferredEntry::Sector { lines: (0..w.n).filter(|&i| w.lines[i]).collect() }],
        };
        params.preferred.count_against_bound = r.chance(300);
        let tag = format!("pref{kind}");
        let roots0 = roots_of(&w, &params0);
        let roots = roots_of(&w, &params);
        let gap: Vec<Key> = roots.iter().flat_map(|i| i.gap_points.clone()).collect();
        let lim = TreeLimits::default();
        t.run("P4 never a failure / panic");
        let (Ok((rep0, _g0, _)), Ok((rep, g, _))) = (
            run_drive(&w, &params0, roots0.clone(), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &lim, 2_000_000),
            run_drive(&w, &params, roots.clone(), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &lim, 2_000_000),
        ) else {
            t.fail("P4 never a failure / panic", format!("seed={seed}"));
            continue;
        };
        check_tree(&mut t, &w, &params, &rep, &g, &gap, "preferred", true);
        // preferred keys are never gap points of any item, never served, never terminal proposals
        t.run("P4 preferred never reduced");
        let hon = params.preferred.honoured(&w.gap, &params.lines, params.terminals_max);
        for it in &rep.items {
            for k in &it.state.item.gap_points {
                if hon.contains(k) {
                    t.fail("P4 preferred never reduced", format!("seed={seed}: preferred {k:?} gap point of {}", it.state.item.id));
                }
            }
        }
        // economy class per root face: one-case without preferences => one-case with preferences, size class not worse
        t.run("P4 economy class not worsened");
        for r0 in &roots0 {
            let face = r0.case.face.clone();
            let get = |rep: &TreeReport| {
                rep.items.iter().find(|x| x.state.item.case == CaseSpec::face_case(face.clone()) && x.state.item.parent.is_none()).and_then(|x| x.disposition.clone())
            };
            let (d0, d1) = (get(&rep0), get(&rep));
            if let Some(Disposition::Covered { cover: c0 }) = &d0
                && c0.members.len() == 1
            {
                match &d1 {
                    None => {} // every gap point preferred: item never created
                    Some(Disposition::Covered { cover: c1 }) if c1.members.len() <= 1 => {
                        if let (Some(a), Some(b)) = (c0.members.first(), c1.members.first())
                            && 64 - b.rhs_terms.leading_zeros() > 64 - a.rhs_terms.leading_zeros()
                        {
                            t.fail("P4 economy class not worsened", format!("seed={seed} face {}: size class {} -> {}", face.label(), a.rhs_terms, b.rhs_terms));
                        }
                    }
                    other => t.fail(
                        "P4 economy class not worsened",
                        format!("seed={seed} face {}: one-case without preferences, {:?} with", face.label(), other.as_ref().map(disp_short)),
                    ),
                }
            }
        }
        // report partition over the preferred KEY entries of this node
        t.run("P4 report categories partition");
        let terms: Vec<(Key, Option<_>)> = rep.terminal_candidates().into_iter().map(|(k, x)| (k, Some(x))).collect();
        let mut terms_all = terms.clone();
        let reached: BTreeSet<Key> = w.gap.iter().cloned().collect();
        for e in &params.preferred.entries {
            if let PreferredEntry::Key(k) = e
                && reached.contains(k)
            {
                terms_all.push((k.clone(), None));
            }
        }
        let rp = preferred_report(&params.preferred, &params.lines, &terms_all, &reached, &[], &[]);
        let pref_keys: BTreeSet<Key> = params.preferred.entries.iter().filter_map(|e| if let PreferredEntry::Key(k) = e { Some(k.clone()) } else { None }).collect();
        for k in &pref_keys {
            let n = usize::from(rp.used.contains(k)) + usize::from(rp.not_needed.contains(k)) + usize::from(rp.not_kept.iter().any(|(x, _)| x == k));
            if n != 1 {
                t.fail("P4 report categories partition", format!("seed={seed}: preferred {k:?} in {n} categories"));
            }
        }
        for (k, _) in &rp.other_terminals {
            if params.preferred.matches(k, &params.lines) {
                t.fail("P4 report categories partition", format!("seed={seed}: preferred {k:?} in other_terminals"));
            }
        }
        if kind == 2 && rp.warnings.is_empty() {
            t.fail("P4 report categories partition", format!("seed={seed}: malformed entries without warnings"));
        }
        let _ = tag;
    }
    t.print("PART 4: preferred terminals");
    t.assert_zero("PART 4", &["I1", "I3", "I4", "P4 preferred never", "P4 report", "P4 malformed"]);
}

// ------------------------------------------------------------------------------------------------ PART 5

fn brute_in_case(face: &Face, fixed: &BTreeMap<usize, i32>, eqs: &[AffineEq], lines: &[bool], k: &[i32]) -> bool {
    let fs: BTreeSet<usize> = face.slots().into_iter().collect();
    for i in 0..lines.len() {
        if let Some(&v) = fixed.get(&i) {
            // v1.2 semantics (ladderfix): a case contains chart keys only, and non-face slots stay at the floor
            if k[i] != v
                || (lines[i] && k[i] < 1)
                || (!lines[i] && k[i] > 0)
                || (!fs.contains(&i) && k[i] != floor_of(lines[i]))
            {
                return false;
            }
        } else if fs.contains(&i) {
            if (lines[i] && k[i] < 1) || (!lines[i] && k[i] > 0) {
                return false;
            }
        } else if k[i] != floor_of(lines[i]) {
            return false;
        }
    }
    eqs.iter().all(|e| e.holds(k))
}

#[test]
fn part5_affine_fuzz() {
    let nw = n_worlds() * 10;
    let mut t = Tally::default();
    for seed in 0..nw {
        let mut r = Rng::new(seed ^ 0xAFF1);
        let n = r.range(3, 6) as usize;
        let lines: Vec<bool> = (0..n).map(|_| r.chance(600)).collect();
        let slots: Vec<usize> = (0..n).collect();
        let face_slots: Vec<usize> = slots.iter().copied().filter(|_| r.chance(700)).collect();
        let sd: Vec<usize> = face_slots.iter().copied().filter(|&s| lines[s]).collect();
        let sn: Vec<usize> = face_slots.iter().copied().filter(|&s| !lines[s]).collect();
        let face = Face::new(sd, sn);
        let fsl = face.slots();
        let mut fixed = BTreeMap::new();
        if !fsl.is_empty() && r.chance(300) {
            let s = r.pick(&fsl);
            fixed.insert(s, r.range(-3, 3) as i32);
        }
        let big = r.chance(50);
        let mut eqs = Vec::new();
        for _ in 0..r.range(1, 4) {
            let m = r.range(1, 3);
            let mut co = Vec::new();
            for _ in 0..m {
                let s = if !fsl.is_empty() && r.chance(850) { r.pick(&fsl) } else { r.pick(&slots) };
                let c = if big { [i64::MAX / 3, i64::MIN / 3, 1 << 40, -(1 << 50), i64::MAX, i64::MIN + 1][r.below(6) as usize] } else { [1i64, -1, 2, -2, 3, -3, 5, 4][r.below(8) as usize] };
                co.push((s, c));
            }
            let constant = if big { [i64::MAX, i64::MIN, 0, 7][r.below(4) as usize] } else { r.range(-8, 8) };
            if let Some(e) = AffineEq::new(co, constant) {
                eqs.push(e);
            }
        }
        if eqs.len() >= 2 && r.chance(300) {
            // redundant row: sum of the first two
            let mut co = eqs[0].coeffs.clone();
            co.extend(eqs[1].coeffs.iter().copied());
            if let Some(e) = AffineEq::new(co, eqs[0].constant.saturating_add(eqs[1].constant)) {
                eqs.push(e);
            }
        }
        if !eqs.is_empty() && r.chance(150) {
            // inconsistent row: same lhs, other constant
            let e = &eqs[0];
            if let Some(e2) = AffineEq::new(e.coeffs.clone(), e.constant.saturating_add(1)) {
                eqs.push(e2);
            }
        }
        let tag = if big { "big" } else { "small" };
        t.run(&format!("P5 canonical case == brute force [{tag}]"));
        let res = catch_unwind(|| CaseSpec::new(face.clone(), fixed.clone(), eqs.clone()));
        let res = match res {
            Ok(x) => x,
            Err(_) => {
                t.fail(&format!("P5 canonical case == brute force [{tag}]"), format!("seed={seed}: PANIC eqs={eqs:?}"));
                continue;
            }
        };
        // window: face slots in [-4, 4], others at floor or floor +- 1
        let mut win: Vec<Key> = vec![vec![]];
        for i in 0..n {
            let vals: Vec<i32> = if fsl.contains(&i) { (-4..=4).collect() } else { vec![floor_of(lines[i]), floor_of(lines[i]) + 1] };
            let mut nw2 = Vec::new();
            for k in &win {
                for &v in &vals {
                    let mut k2 = k.clone();
                    k2.push(v);
                    nw2.push(k2);
                }
            }
            win = nw2;
        }
        match res {
            Ok(c) => {
                let mut sol = 0;
                for k in &win {
                    let b = brute_in_case(&face, &fixed, &eqs, &lines, k);
                    let m = c.contains(k, &lines);
                    if b {
                        sol += 1;
                    }
                    if b != m {
                        let class = if m { "contains() TRUE, brute FALSE" } else { "contains() FALSE, brute TRUE" };
                        // classify: a slot fixed by the canonical form outside the chart orthant
                        let off_chart = c.fixed.iter().any(|(&s, &v)| !fixed.contains_key(&s) && ((lines[s] && v < 1) || (!lines[s] && v > 0)));
                        t.fail(
                            &format!("P5 canonical case == brute force [{tag}]"),
                            format!("seed={seed}: {class} at {k:?}; face {} fixed {fixed:?} eqs {:?} -> {} (fixed from eqs off-chart: {off_chart})", face.label(), eqs.iter().map(|e| e.to_engine()).collect::<Vec<_>>(), c.canonical_text()),
                        );
                        t.count(&format!("P5 mismatch class: {class}, off-chart fixed {off_chart}"), 1);
                        break;
                    }
                }
                if c.is_point() && sol > 1 {
                    t.fail(&format!("P5 canonical case == brute force [{tag}]"), format!("seed={seed}: dim 0 but {sol} solutions in window"));
                }
                if c.is_point() && !c.free().is_empty() {
                    t.count("P5 dim()==0 with free slots left (equations on non-face slots count)", 1);
                }
            }
            Err(e) => {
                t.count(&format!("P5 error {e:?} [{tag}]"), 1);
                if matches!(e, AffineError::Inconsistent | AffineError::NoIntegerPoint) {
                    // the original system must have no integer solution in the window (necessary condition) when every
                    // equation slot is a face slot (other slots are at the floor anyway)
                    for k in &win {
                        if brute_in_case(&face, &fixed, &eqs, &lines, k) {
                            t.fail(&format!("P5 canonical case == brute force [{tag}]"), format!("seed={seed}: {e:?} but {k:?} solves {eqs:?}"));
                            break;
                        }
                    }
                }
            }
        }
        // AffineEq::new must preserve the solution set
        t.run("P5 AffineEq::new preserves solutions");
        let co: Vec<(usize, i64)> = (0..r.range(1, 3)).map(|_| (r.pick(&slots), [1i64, -1, -2, 3, i64::MIN + 1, -i64::MAX][r.below(6) as usize])).collect();
        let constant = [i64::MIN, i64::MAX, -1, 0, 5][r.below(5) as usize];
        if let Ok(Some(e)) = catch_unwind(|| AffineEq::new(co.clone(), constant)) {
            for k in win.iter().take(400) {
                let orig: i128 = co.iter().map(|&(s, c)| c as i128 * k[s] as i128).sum::<i128>() + constant as i128;
                if (orig == 0) != e.holds(k) {
                    t.fail("P5 AffineEq::new preserves solutions", format!("seed={seed}: {co:?} + {constant} -> {:?} differs at {k:?}", e));
                    break;
                }
            }
            // as_fixed must not panic
            if catch_unwind(|| e.as_fixed()).is_err() {
                t.fail("P5 AffineEq::new preserves solutions", format!("seed={seed}: as_fixed PANIC on {e:?}"));
            }
        } else if catch_unwind(|| AffineEq::new(co.clone(), constant)).is_err() {
            t.fail("P5 AffineEq::new preserves solutions", format!("seed={seed}: PANIC {co:?} {constant}"));
        }
        // fit_loci: every fit holds at its points, groups disjoint, >= 2 points
        t.run("P5 fit_loci");
        let pts: Vec<Vec<i32>> = (0..r.range(0, 8)).map(|_| (0..n).map(|_| if big { r.range(-2_000_000_000, 2_000_000_000) as i32 } else { r.range(-4, 4) as i32 }).collect()).collect();
        match catch_unwind(|| fit_loci(&pts, &slots)) {
            Ok(fits) => {
                let mut used = BTreeSet::new();
                for (eq, held) in &fits {
                    if held.len() < 2 || held.iter().any(|&i| !eq.holds(&pts[i]) || !used.insert(i)) {
                        t.fail("P5 fit_loci", format!("seed={seed}: {eq:?} {held:?}"));
                    }
                }
            }
            Err(_) => t.fail("P5 fit_loci", format!("seed={seed}: PANIC on {pts:?}")),
        }
        // canonical_system with huge fixed values (i32 extremes)
        t.run("P5 canonical_system extremes");
        let mut fx = BTreeMap::new();
        fx.insert(0usize, i32::MIN);
        let e = AffineEq::new(vec![(0, i64::MAX / 2), (1, 3)], 1).unwrap();
        if catch_unwind(|| canonical_system(&[e.clone()], &fx)).is_err() {
            t.fail("P5 canonical_system extremes", format!("seed={seed}: PANIC"));
        }
    }
    // targeted: single-slot row with constant i64::MIN (negation overflow)
    t.run("P5 targeted i64::MIN");
    let e = AffineEq { coeffs: vec![(0, 1)], constant: i64::MIN };
    if catch_unwind(|| canonical_system(&[e.clone()], &BTreeMap::new())).is_err() {
        t.fail("P5 targeted i64::MIN", "canonical_system PANIC on n0 + i64::MIN = 0 (struct literal)".into());
    }
    if catch_unwind(|| e.as_fixed()).is_err() {
        t.fail("P5 targeted i64::MIN", "as_fixed PANIC on n0 + i64::MIN".into());
    }
    if let Some(e2) = AffineEq::new(vec![(0, 1)], i64::MIN)
        && catch_unwind(|| canonical_system(&[e2.clone()], &BTreeMap::new())).is_err()
    {
        t.fail("P5 targeted i64::MIN", "canonical_system PANIC on AffineEq::new(n0, i64::MIN)".into());
    }
    if let Some(e3) = AffineEq::new(vec![(0, -1)], i64::MIN) {
        // -n0 + MIN = 0  <=>  n0 = MIN ; normalised it must still say n0 = MIN (not n0 = -MIN)
        if e3.holds(&[i32::MAX, 0, 0]) || e3.constant != i64::MIN && e3.coeffs[0].1 == 1 && e3.constant == i64::MIN {
            t.fail("P5 targeted i64::MIN", format!("AffineEq::new(-n0, MIN) -> {e3:?}"));
        }
        eprintln!("AffineEq::new(vec![(0,-1)], i64::MIN) = {e3:?} (exact: n0 - (-2^63) ... = n0 = i64::MIN)");
    }
    t.print("PART 5: affine fuzz");
    t.assert_zero("PART 5", &["P5"]);
}

// ------------------------------------------------------------------------------------------------ PART 6

#[test]
fn part6_full_coverage_first() {
    let mut t = Tally::default();
    let p = NodeParams::new("toy", 5, 3);
    let case = CaseSpec::face_case(Face::new(vec![0], vec![]));
    let ended = rustred_campaign_core::worklist::items::FullAttemptEnded::from_evidence(
        &p,
        rustred_campaign_core::worklist::items::FullAttemptEvidence {
            node: "toy".into(),
            cpu_ms: 0,
            fixer_rounds: p.full_attempt.fixer_rounds,
            open: vec!["x".into()],
        },
    )
    .unwrap();
    t.run("R35 box item refused without user box");
    if box_restricted_item(case.clone(), vec![vec![2, 1, 1, 0, 0]], &p, &ended) != Err(WorklistError::NoUserBox) {
        t.fail("R35 box item refused without user box", "constructed".into());
    }
    let mut p2 = p.clone();
    p2.user_box = Some(UserBox { id: "b".into(), given_by: "tool".into() });
    t.run("R35 box not given by user refused");
    if !matches!(box_restricted_item(case.clone(), vec![], &p2, &ended), Err(WorklistError::BoxNotUserGiven(_))) {
        t.fail("R35 box not given by user refused", "constructed".into());
    }
    p2.user_box = Some(UserBox { id: "b".into(), given_by: "user".into() });
    t.run("R35 box item with user box");
    if box_restricted_item(case, vec![], &p2, &ended).is_err() {
        t.fail("R35 box item with user box", "refused".into());
    }
    // pure next_steps under an always-interrupting operator: documented as uncapped; check it keeps asking the
    // SAME attempt (no progress, no Attention) => a driver without its own cap would livelock
    let w = World::generate(5);
    let params = NodeParams::with_lines("toy", w.lines.clone());
    let roots = roots_of(&w, &params);
    let mut st = ItemState::new(roots[0].clone());
    st.record_offers("none", vec![]);
    let mut same = 0;
    for _ in 0..1000 {
        match next_steps(&st, &params) {
            Next::Run(list) => {
                st.record_answer(&list[0], SolveAnswer { outcome: SolveOutcome::Interrupted { by: Interruption::Operator }, cpu_ms: 1, rows: None });
                same += 1;
            }
            _ => break,
        }
    }
    eprintln!("P6: 1000 operator interrupts in a row -> {same} Run steps");
    assert!(
        same <= params.budgets.max_operator_interrupts as usize + 1,
        "operator interrupts bounded by the sanity bound (v1.2)"
    );
    t.print("PART 6");
    t.assert_zero("PART 6", &["R35"]);
}

// ------------------------------------------------------------------------------------------------ minimal repros

/// A prover that answers every key with `level: None, failure: None` ("unavailable"): drive_tree must terminate.
#[test]
fn repro_prover_none_none_livelock() {
    for seed in 0..200 {
        let w = World::generate(seed);
        let mut params = NodeParams::with_lines("toy", w.lines.clone());
        params.min_evidence = EvidenceLevel::A2;
        let roots = roots_of(&w, &params);
        match run_drive(&w, &params, roots, SolverMode::Truthful, FactMode::Truthful, ProverMode::NoneNone, 0, 0, &TreeLimits::default(), 20_000) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("REPRO prover NoneNone: seed={seed}: {e}");
                return;
            }
        }
    }
    eprintln!("REPRO prover NoneNone: no livelock in 200 worlds");
}

/// Debug helper: replicate a part-2 run (AUDIT_DEBUG_SEED) with a manual driver and print the first repeated
/// evidence request.
#[test]
fn debug_part2_seed() {
    let Some(seed) = std::env::var("AUDIT_DEBUG_SEED").ok().and_then(|s| s.parse::<u64>().ok()) else {
        return;
    };
    let w = World::generate(seed);
    let mut r = Rng::new(seed ^ 0x1234);
    let mut params = w.params(&mut r);
    if params.min_evidence < EvidenceLevel::A2 && r.chance(700) {
        params.min_evidence = EvidenceLevel::A2;
    }
    let roots = roots_of(&w, &params);
    let solver_modes = [
        SolverMode::ForeignDomain,
        SolverMode::WrongArity,
        SolverMode::SameRuleForever,
        SolverMode::RegenExceptions,
        SolverMode::Alternating,
        SolverMode::AllFailures,
        SolverMode::Garbage,
    ];
    let prover_modes = [
        ProverMode::Truthful,
        ProverMode::UnknownAll,
        ProverMode::UnsupportedAll,
        ProverMode::Empty,
        ProverMode::NoneNone,
        ProverMode::WrongKey,
        ProverMode::LowerLevel,
        ProverMode::HigherWithFailure,
        ProverMode::Random,
    ];
    let fact_modes = [FactMode::Truthful, FactMode::AllUnknown, FactMode::Random];
    let sm = if r.chance(250) { SolverMode::Truthful } else { r.pick(&solver_modes) };
    let pm = r.pick(&prover_modes);
    let fm = if r.chance(600) { FactMode::Truthful } else { r.pick(&fact_modes) };
    let p_op = if r.chance(200) { 200 } else { 0 };
    eprintln!("seed {seed}: {sm:?} {pm:?} {fm:?} min {:?} op {p_op}", params.min_evidence);
    let c = calls(1_000_000);
    let mut solver = Solver { w: &w, mode: sm, calls: c.clone(), per_request: BTreeMap::new(), p_operator: p_op, p_infra: 0, rng: Rng::new(w.seed ^ 77) };
    let mut facts = Facts { w: &w, mode: fm, calls: c.clone() };
    let mut prover = Prover { w: &w, mode: pm, calls: c.clone(), granted: BTreeMap::new() };
    // manual sequential driver (DFS like drive_tree) with loop detection on evidence requests
    let mut served: BTreeSet<Key> = BTreeSet::new();
    let mut stack: Vec<Item> = roots.into_iter().rev().collect();
    while let Some(mut item) = stack.pop() {
        item.gap_points.retain(|k| !served.contains(k));
        if item.gap_points.is_empty() {
            continue;
        }
        let mut st = ItemState::new(item);
        let mut last: Option<String> = None;
        loop {
            match next_steps(&st, &params) {
                Next::Run(list) => {
                    let a = &list[0];
                    let ans = solver.solve(&CaseRequest::from_attempt(&params.node, a), &a.budget);
                    st.record_answer(a, ans);
                }
                Next::NeedVerdicts(reqs) => {
                    for q in reqs {
                        let cand = st.records[&q.attempt_key].candidates()[q.index].clone();
                        let (v, h) = judge_points_with_hits(&cand, &q.keys, &mut facts, &w.cmp, &params.preferred, &params.lines);
                        st.record_verdicts(&q.attempt_key, q.index, v);
                        st.record_preferred_hits(&q.attempt_key, q.index, h);
                    }
                }
                Next::NeedEvidence(reqs) => {
                    let sig = format!("{reqs:?}");
                    if last.as_deref() == Some(sig.as_str()) {
                        eprintln!("item {} case {}: SAME evidence request repeated:\n  {sig}", st.item.id, st.item.case.canonical_text());
                        let q = &reqs[0];
                        eprintln!("  gap points {:?}", st.item.gap_points);
                        eprintln!("  recorded evidence: {:?}", st.evidence.get(&(q.attempt_key.clone(), q.index)));
                        return;
                    }
                    last = Some(sig);
                    if c.borrow().prove > 990 && c.borrow().prove < 1000 {
                        eprintln!("item {} case {} gp {:?}\n   EVREQ {:?}", st.item.id, st.item.case.canonical_text(), st.item.gap_points, reqs);
                        for q in &reqs {
                            eprintln!("   recorded: {:?}", st.evidence.get(&(q.attempt_key.clone(), q.index)));
                            eprintln!("   verdicts: {:?}", st.verdicts.get(&(q.attempt_key.clone(), q.index)));
                        }
                    }
                    for q in reqs {
                        let cand = st.records[&q.attempt_key].candidates()[q.index].clone();
                        let mut ev = prover.prove(&cand, &q.keys, q.level);
                        for k in &q.keys {
                            if !ev.iter().any(|e| &e.key == k) {
                                ev.push(KeyEvidence { key: k.clone(), level: None, failure: Some(ProofFailure::Unknown("no answer".into())) });
                            }
                        }
                        st.record_evidence(&q.attempt_key, q.index, ev);
                    }
                }
                Next::NeedOffers => st.record_offers("none", vec![]),
                Next::NeedContainment(_) => unreachable!(),
                Next::Done(d) => {
                    if let Disposition::Covered { cover } | Disposition::Split { cover, .. } = &d {
                        for m in &cover.members {
                            served.extend(m.served.iter().cloned());
                        }
                    }
                    if let Disposition::Split { children, alternatives, .. } = d {
                        for c in alternatives.into_iter().rev() {
                            stack.push(c);
                        }
                        for c in children.into_iter().rev() {
                            stack.push(c);
                        }
                    }
                    break;
                }
            }
        }
    }
    eprintln!("no evidence loop found");
}

#[test]
fn debug_infra_seed() {
    let Some(seed) = std::env::var("AUDIT_INFRA_SEED").ok().and_then(|s| s.parse::<u64>().ok()) else {
        return;
    };
    let w = World::generate(seed);
    let mut r = Rng::new(seed ^ 0xABCD);
    let params = w.params(&mut r);
    let roots = roots_of(&w, &params);
    let lim = TreeLimits::default();
    let (a, _, _) = run_drive(&w, &params, roots.clone(), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &lim, 2_000_000).unwrap();
    let (b, _, _) = run_drive(&w, &params, roots.clone(), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 400, &lim, 2_000_000).unwrap();
    let (sa, sb) = (report_signature(&a), report_signature(&b));
    eprintln!("items {} vs {}; economy factor {}", sa.len(), sb.len(), params.budgets.economy_extra_factor);
    for (i, (x, y)) in sa.iter().zip(&sb).enumerate() {
        if x != y {
            eprintln!("first difference at item {i} ({})", x.0);
            eprintln!(" plain: {}", disp_short(a.items[i].disposition.as_ref().unwrap()));
            eprintln!(" infra: {}", disp_short(b.items[i].disposition.as_ref().unwrap()));
            for (k, rec) in &b.items[i].state.records {
                if rec.infra_interrupts > 0 || rec.operator_interrupts > 0 {
                    eprintln!("   {k}: infra {} cpu_total {} answer {:?}", rec.infra_interrupts, rec.cpu_ms_total, rec.answer.as_ref().map(|x| (x.outcome.tag(), x.cpu_ms)));
                }
            }
            let ma: Vec<_> = match a.items[i].disposition.as_ref().unwrap() { Disposition::Covered { cover } | Disposition::Split { cover, .. } => cover.members.iter().map(|m| (m.setting.label(), m.digest.clone(), m.rhs_terms)).collect(), _ => vec![] };
            let mb: Vec<_> = match b.items[i].disposition.as_ref().unwrap() { Disposition::Covered { cover } | Disposition::Split { cover, .. } => cover.members.iter().map(|m| (m.setting.label(), m.digest.clone(), m.rhs_terms)).collect(), _ => vec![] };
            eprintln!(" plain members {ma:?}\n infra members {mb:?}");
            eprintln!(" answered plain {} infra {}", a.items[i].state.attempts_made(), b.items[i].state.attempts_made());
            break;
        }
    }
}

#[test]
fn debug_pref_seed() {
    let Some(seed) = std::env::var("AUDIT_PREF_SEED").ok().and_then(|s| s.parse::<u64>().ok()) else {
        return;
    };
    let w = World::generate(seed);
    let mut r = Rng::new(seed ^ 0x99);
    let params0 = w.params(&mut r);
    let mut params = params0.clone();
    let kind = r.below(6);
    params.preferred.entries = match kind {
        0 => vec![],
        1 => w.gap.iter().map(|k| PreferredEntry::Key(k.clone())).collect(),
        2 => vec![PreferredEntry::Key(vec![9; w.n + 1])],
        3 => w.gap.iter().filter(|_| r.chance(300)).map(|k| PreferredEntry::Key(k.clone())).collect(),
        4 => {
            let mut v = vec![];
            for k in &w.gap {
                for l in w.lower_strict(k) {
                    if r.chance(300) {
                        v.push(PreferredEntry::Key(l));
                    }
                }
            }
            v
        }
        _ => vec![PreferredEntry::Sector { lines: (0..w.n).filter(|&i| w.lines[i]).collect() }],
    };
    eprintln!("kind {kind} entries {:?}", params.preferred.entries);
    for factor in [2.0, 1e30] {
        let mut p0 = params0.clone();
        let mut p1 = params.clone();
        p0.budgets.economy_extra_factor = factor;
        p1.budgets.economy_extra_factor = factor;
        let lim = TreeLimits::default();
        let (a, _, _) = run_drive(&w, &p0, roots_of(&w, &p0), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &lim, 2_000_000).unwrap();
        let (b, _, _) = run_drive(&w, &p1, roots_of(&w, &p1), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &lim, 2_000_000).unwrap();
        for rep in [&a, &b] {
            for it in rep.items.iter().filter(|x| x.state.item.parent.is_none()) {
                let mem: Vec<_> = match it.disposition.as_ref().unwrap() { Disposition::Covered { cover } | Disposition::Split { cover, .. } => cover.members.iter().map(|m| (m.setting.label(), m.rhs_terms, m.served.len())).collect(), _ => vec![] };
                eprintln!("factor {factor}: root {} gp {} answered {} -> {} {mem:?}", it.state.item.case.face.label(), it.state.item.gap_points.len(), it.state.attempts_made(), disp_short(it.disposition.as_ref().unwrap()));
            }
        }
    }
}

#[test]
fn debug_duplicate_items() {
    if std::env::var("AUDIT_DUP").is_err() {
        return;
    }
    let mut best: Option<(usize, u64)> = None;
    for seed in 0..400 {
        let w = World::generate(seed);
        let mut r = Rng::new(seed ^ 0xABCD);
        let params = w.params(&mut r);
        let (rep, _, _) = run_drive(&w, &params, roots_of(&w, &params), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &TreeLimits::default(), 2_000_000).unwrap();
        let mut ids: BTreeMap<&str, u32> = BTreeMap::new();
        for it in &rep.items {
            *ids.entry(it.state.item.id.0.as_str()).or_default() += 1;
        }
        if ids.values().any(|&c| c > 1) && best.is_none_or(|(n, _)| rep.items.len() < n) {
            best = Some((rep.items.len(), seed));
        }
    }
    let (_, seed) = best.unwrap();
    let w = World::generate(seed);
    let mut r = Rng::new(seed ^ 0xABCD);
    let params = w.params(&mut r);
    let (rep, _, _) = run_drive(&w, &params, roots_of(&w, &params), SolverMode::Truthful, FactMode::Truthful, ProverMode::Truthful, 0, 0, &TreeLimits::default(), 2_000_000).unwrap();
    eprintln!("smallest tree with a duplicated item: seed {seed}, n {} lines {:?}, {} items", w.n, w.lines, rep.items.len());
    for (i, it) in rep.items.iter().enumerate() {
        eprintln!(
            "  #{i} {} {:?} case {} origin {:?} gp {:?} answered {} -> {}",
            &it.state.item.id.0[..12],
            it.state.item.kind,
            it.state.item.case.canonical_text(),
            it.state.item.origin,
            it.state.item.gap_points,
            it.state.attempts_made(),
            it.disposition.as_ref().map(disp_short).unwrap_or_default()
        );
    }
}

/// Debug helper (ladderfix): replay one part-2 seed (AUDIT_I4_SEED) and print, for every terminal, the items holding
/// the key and the candidates acceptable there with their verdict / evidence.
#[test]
fn debug_i4_seed() {
    let Some(seed) = std::env::var("AUDIT_I4_SEED").ok().and_then(|s| s.parse::<u64>().ok()) else { return };
    let solver_modes = [SolverMode::ForeignDomain, SolverMode::WrongArity, SolverMode::SameRuleForever, SolverMode::RegenExceptions, SolverMode::Alternating, SolverMode::AllFailures, SolverMode::Garbage];
    let prover_modes = [ProverMode::Truthful, ProverMode::UnknownAll, ProverMode::UnsupportedAll, ProverMode::Empty, ProverMode::NoneNone, ProverMode::WrongKey, ProverMode::LowerLevel, ProverMode::HigherWithFailure, ProverMode::Random];
    let fact_modes = [FactMode::Truthful, FactMode::AllUnknown, FactMode::Random];
    let w = World::generate(seed);
    let mut r = Rng::new(seed ^ 0x1234);
    let mut params = w.params(&mut r);
    if params.min_evidence < EvidenceLevel::A2 && r.chance(700) {
        params.min_evidence = EvidenceLevel::A2;
    }
    let roots = roots_of(&w, &params);
    let sm = if r.chance(250) { SolverMode::Truthful } else { r.pick(&solver_modes) };
    let pm = r.pick(&prover_modes);
    let fm = if r.chance(600) { FactMode::Truthful } else { r.pick(&fact_modes) };
    let lim = TreeLimits { max_attempts: 20_000, max_cpu_ms: u64::MAX, max_items: 100_000, max_steps: 400_000 };
    let p_op = if r.chance(200) { 200 } else { 0 };
    let (rep, _, _) = run_drive(&w, &params, roots, sm, fm, pm, p_op, 0, &lim, 5_000).unwrap();
    eprintln!("modes {sm:?} {pm:?} {fm:?} stopped {:?} items {}", rep.stopped, rep.items.len());
    for (k, tok) in rep.terminal_candidates() {
        eprintln!("TERMINAL {k:?} from {}", tok.item());
        for (ii, it) in rep.items.iter().enumerate() {
            let Some(pos) = it.state.item.gap_points.iter().position(|g| g == &k) else { continue };
            eprintln!("  item {ii} {} {} disp {:?}", it.state.item.id, it.state.item.case.canonical_text(), it.disposition.as_ref().map(disp_short));
            eprintln!("  next_steps now: {:?} steps {} records {}", match next_steps(&it.state, &params) { Next::Done(d) => disp_short(&d), Next::NeedEvidence(r) => format!("NeedEvidence {}", r.len()), o => format!("{o:?}").chars().take(80).collect() }, it.state.steps, it.state.records.len());
            for (ak, rec) in &it.state.records {
                for (ci, c) in rec.candidates().iter().enumerate() {
                    if w.acceptable(c, &k, params.min_evidence) {
                        eprintln!("    gp {:?} min {:?} vlen {:?} vec {:?}", it.state.item.gap_points, params.min_evidence, it.state.verdicts.get(&(ak.clone(), ci)).map(|v| v.len()), it.state.verdicts.get(&(ak.clone(), ci)));
                        eprintln!("    acceptable {} at {ak}#{ci}: verdict {:?} evidence {:?}", c.digest, it.state.verdicts.get(&(ak.clone(), ci)).map(|v| v.get(pos).cloned()), it.state.evidence.get(&(ak.clone(), ci)).map(|e| e.iter().filter(|x| x.key == k).cloned().collect::<Vec<_>>()));
                    }
                }
            }
        }
    }
}
