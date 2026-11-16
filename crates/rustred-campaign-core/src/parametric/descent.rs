//! B1: descent of every right-hand-side term of ONE rule record on the record's WHOLE case domain
//! (interfaces/parametric.md section 1).
//!
//! THE ARGUMENT. Let D be the keys where the record APPLIES ([`CaseDomain::applies`]) and pt the rule point of a key.
//! A term slot is a constant or `n_s + o` (the slot's own symbol, as the engine writes patterns). Split D by the term:
//! - A (in chart): every line slot of the term >= 1, every numerator slot <= 0. On A the comparison of the term with
//!   the target is the engine's symbolic pattern comparison ([`NodeComparator::cmp_pattern`], rustred-order
//!   `compare_mixed`): every comparison component (support, degree rows, coordinates) is linear in each symbolic slot
//!   on one chart side, so the common symbolic part cancels and the verdict is the SAME at every key of A (the
//!   comparison is not "sampled": it is index-free). `Less` = the term descends on all of A.
//! - H (hand-down): some line slot <= 0 and no numerator slot >= 1: the term has strictly fewer active slots than the
//!   target; under a support-first order (no pre-support degree rows; legacy orders count actives first) it is LOWER.
//!   It also leaves the node's sector (descent in the DAG). Listed as sub-loci, never a failure.
//! - P (positive numerator): some numerator slot >= 1: NOT lower; the term needs a ZERO coefficient there. P is a finite
//!   union of planes `pt_s = v` (v from 1 - o to the slot's upper bound) or a half-space / the whole domain.
//!
//! Every emptiness question is decided EXACTLY by [`crate::loci::contain::is_empty_with`] (sign splits, rational
//! Fourier-Motzkin, integer witnesses verified with `applies`); `Unknown` stays undecided. A zero proof of the same
//! (record sha256, term) on the plane or on the whole domain discharges the term ON THAT LOCUS ONLY.
//! Nothing is sampled. The brute-force differential (tests) only cross-checks.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use crate::accept::descent::{Range, point_ranges};
use crate::closure::bodies::rule_point;
use crate::closure::sha256::Sha256;
use crate::domain::CaseDomain;
use crate::import::zero_proofs::{DenStatus, Locus, VERDICT_PROVED, ZERO_PROOF_SCHEMA, parse_line};
use crate::key::NodeFrame;
use crate::loci::contain::{Emptiness, ProofBudget, is_empty_with};
use crate::order::{Engine, NodeComparator, PatternSlot, parse_pattern};
use crate::poly::Poly;

/// Planes enumerated at most per (term slot) before the slot's crossing set is treated as ONE half-space.
pub const MAX_PLANES_PER_SLOT: i64 = 64;

/// Proved zero claims (`rustred.campaign.zero-proof.v1`, verdict `zero_on_locus`) indexed by record sha256 (hex).
/// Lines of other schemas (headers, non-zero / per-key hand-down lines) are counted and skipped; a malformed zero-proof
/// line refuses the file (fail closed).
#[derive(Clone, Debug, Default)]
pub struct ZeroIndex {
    by_record: HashMap<String, Vec<(u32, Locus, DenStatus)>>,
    /// (path, sha256 hex) of every loaded file.
    pub files: Vec<(String, String)>,
    pub proved_lines: usize,
    pub other_lines: usize,
}

impl ZeroIndex {
    pub fn new() -> Self {
        ZeroIndex::default()
    }

    /// Add one parsed JSON line.
    pub fn insert_value(&mut self, v: &Value) -> Result<(), String> {
        if v.get("schema").and_then(Value::as_str) != Some(ZERO_PROOF_SCHEMA) {
            self.other_lines += 1;
            return Ok(());
        }
        let l = parse_line(v)?;
        if l.verdict != VERDICT_PROVED {
            self.other_lines += 1;
            return Ok(());
        }
        let den = l
            .denominator
            .ok_or("a proved line without a denominator class")?;
        self.by_record
            .entry(l.record_sha256.to_ascii_lowercase())
            .or_default()
            .push((l.term, l.locus, den));
        self.proved_lines += 1;
        Ok(())
    }

    /// Load proof files (JSONL), hashing each.
    pub fn load(paths: &[&Path]) -> Result<ZeroIndex, String> {
        let mut z = ZeroIndex::new();
        for p in paths {
            let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
            let mut h = Sha256::new();
            let mut r = std::io::BufReader::with_capacity(1 << 20, f);
            let mut line = Vec::new();
            let mut n = 0usize;
            loop {
                line.clear();
                let got = r
                    .read_until(b'\n', &mut line)
                    .map_err(|e| format!("{}: {e}", p.display()))?;
                if got == 0 {
                    break;
                }
                h.update(&line);
                n += 1;
                let t = line.trim_ascii();
                if t.is_empty() {
                    continue;
                }
                let v: Value = serde_json::from_slice(t)
                    .map_err(|e| format!("{} line {n}: {e}", p.display()))?;
                z.insert_value(&v)
                    .map_err(|e| format!("{} line {n}: {e}", p.display()))?;
            }
            z.files.push((p.display().to_string(), h.finish_hex()));
        }
        Ok(z)
    }

    /// [`ZeroIndex::load`] of proof files whose STANDALONE VERIFICATION is pinned (the rule of audit r2-zeros F5,
    /// `import::zero_proofs::VerifiedZeroProofs`): the verifier's summary hashes to `summary_sha256`, has the verify
    /// schema, says `verified` with no mismatch, no claim without its record and no header mismatch, and lists the
    /// sha256 of every file loaded here (`proofs_sha256`). Refusals start with `zero_proofs_unverified:`.
    pub fn load_verified(
        paths: &[&Path],
        summary: &Path,
        summary_sha256: &str,
    ) -> Result<ZeroIndex, String> {
        let un = |m: String| format!("zero_proofs_unverified: {m}");
        let bytes =
            std::fs::read(summary).map_err(|e| un(format!("{}: {e}", summary.display())))?;
        let mut h = Sha256::new();
        h.update(&bytes);
        let got = h.finish_hex();
        let pin = summary_sha256
            .trim_start_matches("sha256:")
            .to_ascii_lowercase();
        if got != pin {
            return Err(un(format!(
                "the verify summary {} has sha256 {got}, pinned {pin}",
                summary.display()
            )));
        }
        let v: Value = String::from_utf8_lossy(&bytes)
            .lines()
            .find(|l| !l.trim().is_empty())
            .and_then(|l| serde_json::from_str(l).ok())
            .or_else(|| serde_json::from_slice(&bytes).ok())
            .ok_or_else(|| un(format!("{}: no JSON summary", summary.display())))?;
        if v["schema"] != crate::import::zero_proofs::VERIFY_SUMMARY_SCHEMA {
            return Err(un(format!("summary schema {}", v["schema"])));
        }
        let clean = v["verified"] == true
            && v["counts"]["mismatches"].as_u64() == Some(0)
            && v["counts"]["claims_without_record"].as_u64() == Some(0)
            && v["header_mismatch"].as_array().is_some_and(Vec::is_empty);
        if !clean {
            return Err(un("the summary does not state a clean verification".into()));
        }
        let listed: Vec<String> = v["proofs_sha256"]
            .as_array()
            .ok_or_else(|| {
                un("the summary binds no proof file by its sha256 (proofs_sha256 absent)".into())
            })?
            .iter()
            .filter_map(|x| {
                x.as_str()
                    .map(|s| s.trim_start_matches("sha256:").to_ascii_lowercase())
            })
            .collect();
        let z = ZeroIndex::load(paths)?;
        for (p, s) in &z.files {
            if !listed.contains(s) {
                return Err(un(format!(
                    "{p} (sha256 {s}) is not a file the pinned summary verified"
                )));
            }
        }
        Ok(z)
    }

    /// The denominator class of a proved claim for (record, term) on `locus` or on the whole domain.
    pub fn proof(&self, record_sha256: &str, term: u32, locus: Locus) -> Option<DenStatus> {
        let v = self.by_record.get(record_sha256)?;
        let mut best: Option<DenStatus> = None;
        for (t, l, d) in v {
            if *t == term && (*l == locus || *l == Locus::Domain) {
                best = Some(match (best, *d) {
                    (Some(DenStatus::NowhereZero), _) | (_, DenStatus::NowhereZero) => {
                        DenStatus::NowhereZero
                    }
                    _ => DenStatus::WhereRegular,
                });
            }
        }
        best
    }

    /// Add every claim and file of `o`.
    pub fn merge(&mut self, o: ZeroIndex) {
        for (k, v) in o.by_record {
            self.by_record.entry(k).or_default().extend(v);
        }
        self.files.extend(o.files);
        self.proved_lines += o.proved_lines;
        self.other_lines += o.other_lines;
    }

    pub fn has_record(&self, record_sha256: &str) -> bool {
        self.by_record.contains_key(record_sha256)
    }
}

/// The relation of a sub-locus on the rule point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub enum Rel {
    #[serde(rename = "=")]
    Eq,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">=")]
    Ge,
    /// The whole domain (a constant slot out of its chart).
    #[serde(rename = "domain")]
    Domain,
}

/// Emptiness of a sub-locus (D intersected with the condition).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyState {
    Empty,
    NonEmpty,
    Unknown,
}

/// One sub-locus of the domain: `pt[param] <rel> value` (or the whole domain), with its emptiness and zero-proof status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SubLocus {
    /// Key slot of the term.
    pub slot: usize,
    /// Rule-point parameter.
    pub param: usize,
    pub relation: Rel,
    pub value: i64,
    pub emptiness: EmptyState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub witness: Option<Vec<i32>>,
    /// `nowhere_zero` / `where_regular`: the term's coefficient is proved zero on the sub-locus.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zero_proof: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The verdict of one term on the record's whole domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum TermVerdict {
    /// Lower than the target on every key of the domain where it is in the chart; every non-empty positive-numerator
    /// sub-locus carries a zero proof (`discharged`); `hand_down` = sub-loci where it leaves the sector (lower too).
    Descends {
        hand_down: Vec<SubLocus>,
        discharged: Vec<SubLocus>,
    },
    /// As `Descends` except on `loci` (positive-numerator sub-loci without a zero proof, or of unknown emptiness):
    /// there the term is NOT lower unless its coefficient vanishes.
    DescendsExcept {
        loci: Vec<SubLocus>,
        hand_down: Vec<SubLocus>,
        discharged: Vec<SubLocus>,
    },
    /// In the chart at `witness` (a key where the record applies) and not lower there; by the argument of the module
    /// doc, not lower at ANY in-chart key of the domain.
    DoesNotDescend {
        witness: Vec<i32>,
        comparison: String,
    },
    Undecided {
        reason: String,
    },
}

impl TermVerdict {
    pub fn word(&self) -> &'static str {
        match self {
            TermVerdict::Descends { .. } => "descends",
            TermVerdict::DescendsExcept { .. } => "descends_except",
            TermVerdict::DoesNotDescend { .. } => "does_not_descend",
            TermVerdict::Undecided { .. } => "undecided",
        }
    }
    /// Severity: descends 0 < descends_except 1 < undecided 2 < does_not_descend 3.
    pub fn severity(&self) -> u8 {
        match self {
            TermVerdict::Descends { .. } => 0,
            TermVerdict::DescendsExcept { .. } => 1,
            TermVerdict::Undecided { .. } => 2,
            TermVerdict::DoesNotDescend { .. } => 3,
        }
    }
}

/// The verdict of one record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleDescent {
    /// Emptiness of the record's domain (an empty domain: every term descends vacuously).
    pub domain: EmptyState,
    pub terms: Vec<TermVerdict>,
    /// Per verdict word: number of terms.
    pub counts: BTreeMap<&'static str, usize>,
    /// The worst term verdict word ("descends" for a record without terms).
    pub worst: &'static str,
    /// Upper bound, valid on the whole domain, of (D + r of a hand-down term, dropped lines counted as numerators)
    /// minus (D + r of the target), over the terms that can hand down; `None` = no term can hand down.
    pub handdown_excess_bound: Option<i64>,
    /// Some hand-down term has no finite bound (a crossing slot without a lower bound).
    pub handdown_excess_unbounded: bool,
    /// Max over the terms that can be IN the chart (on the per-slot ranges; equations and exceptions not used) of
    /// (D + r of the term) - (D + r of the target): on the in-chart part this difference is index-free (a sum of
    /// offsets). `None` = no term can be in the chart. The arrival-cone lemma "top terms never raise D + r" is
    /// `top_excess_max <= 0`.
    pub top_excess_max: Option<i64>,
    /// Terms that can be in the chart (the terms `top_excess_max` is taken over).
    pub top_terms: usize,
}

/// What B1 needs about one record.
pub struct RuleInput<'a> {
    pub domain: &'a CaseDomain,
    /// The target pattern text (`I(n0,...)`).
    pub target: &'a str,
    /// The right-hand-side pattern texts in record order.
    pub rhs: &'a [String],
    /// sha256 (hex, lower case) of the raw record line: the zero-proof binding.
    pub record_sha256: Option<&'a str>,
}

/// The fixed context of a run.
pub struct DescentContext<'a> {
    pub frame: &'a NodeFrame,
    pub cmp: &'a NodeComparator,
    pub zeros: Option<&'a ZeroIndex>,
    pub budget: ProofBudget,
}

impl DescentContext<'_> {
    /// Support-first (the hand-down argument): no pre-support degree rows in a program order.
    pub fn support_first(&self) -> bool {
        match self.cmp.engine() {
            Engine::Legacy { .. } => true,
            Engine::Program(p) => p.descriptor().pre_support_degree_rows.is_empty(),
        }
    }
}

fn lower_of(r: Range) -> Option<i64> {
    match r {
        Range::Exactly(v) | Range::AtLeast(v) => Some(v),
        Range::AtMost(_) | Range::Any => None,
    }
}

fn upper_of(r: Range) -> Option<i64> {
    match r {
        Range::Exactly(v) | Range::AtMost(v) => Some(v),
        Range::AtLeast(_) | Range::Any => None,
    }
}

/// A candidate sub-locus before its emptiness is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Cand {
    slot: usize,
    param: usize,
    rel: Rel,
    value: i64,
}

/// Emptiness of a sub-locus with its witness and the reason of an `Unknown`.
pub type CellState = (EmptyState, Option<Vec<i32>>, Option<String>);

/// One record's evidence line and verdict (`None`: the record could not be read).
pub type JobResult = (Value, Option<RuleDescent>);

/// Per record: emptiness of D intersected with one condition, cached across terms.
struct RecordCache<'a, 'b> {
    ctx: &'a DescentContext<'b>,
    dom: &'a CaseDomain,
    cells: HashMap<(usize, Rel, i64), CellState>,
}

/// `n_p - v` / `-(n_p) + v` / `n_p - v` as polynomials on the rule point.
fn cond_poly(param: usize, rel: Rel, value: i64) -> Result<Poly, String> {
    let x = Poly::var(param);
    let c = Poly::constant(i128::from(value));
    match rel {
        Rel::Eq | Rel::Ge => x.sub(&c).map_err(|e| format!("{e:?}")), // n - v (= 0 / >= 0)
        Rel::Le => c.sub(&x).map_err(|e| format!("{e:?}")),           // v - n >= 0
        Rel::Domain => Err("no polynomial for the whole domain".into()),
    }
}

/// Exact emptiness of `dom` with extra equations / bounds (rule-point polynomials). If the exact test is `Unknown`, the
/// domain is relaxed by dropping exception branches with a nonlinear member (a SUPERSET: an empty superset proves
/// emptiness; a witness of the superset counts only if the record applies there).
pub fn emptiness(
    dom: &CaseDomain,
    frame: &NodeFrame,
    budget: &ProofBudget,
    eqs: &[Poly],
    bounds: &[Poly],
) -> CellState {
    let mut d = dom.clone();
    d.equations.extend_from_slice(eqs);
    d.bounds.extend_from_slice(bounds);
    match is_empty_with(&d, frame, budget) {
        Emptiness::ProvedEmpty { .. } => (EmptyState::Empty, None, None),
        Emptiness::NonEmpty { witness } => (EmptyState::NonEmpty, Some(witness), None),
        Emptiness::Unknown { reason } => {
            let nonlinear = |b: &Vec<Poly>| b.iter().any(|m| m.degree() > 1);
            if !d.exceptions.iter().any(nonlinear) {
                return (EmptyState::Unknown, None, Some(reason));
            }
            let mut r = d.clone();
            r.exceptions.retain(|b| !nonlinear(b));
            match is_empty_with(&r, frame, budget) {
                Emptiness::ProvedEmpty { .. } => (EmptyState::Empty, None, None),
                Emptiness::NonEmpty { witness } if d.applies(&witness).applies() => {
                    (EmptyState::NonEmpty, Some(witness), None)
                }
                Emptiness::NonEmpty { .. } => (
                    EmptyState::Unknown,
                    None,
                    Some(format!(
                        "{reason}; relaxed (nonlinear exceptions dropped): its witness lies on a dropped branch"
                    )),
                ),
                Emptiness::Unknown { reason: r2 } => (
                    EmptyState::Unknown,
                    None,
                    Some(format!("{reason}; relaxed: {r2}")),
                ),
            }
        }
    }
}

impl<'a, 'b> RecordCache<'a, 'b> {
    fn cell(&mut self, c: Cand) -> CellState {
        let k = (c.param, c.rel, c.value);
        if let Some(v) = self.cells.get(&k) {
            return v.clone();
        }
        let v = match cond_poly(c.param, c.rel, c.value) {
            Err(e) => (EmptyState::Unknown, None, Some(e)),
            Ok(p) => {
                let (eqs, bounds) = if c.rel == Rel::Eq {
                    (vec![p], vec![])
                } else {
                    (vec![], vec![p])
                };
                emptiness(self.dom, self.ctx.frame, &self.ctx.budget, &eqs, &bounds)
            }
        };
        self.cells.insert(k, v.clone());
        v
    }
}

fn den_word(d: DenStatus) -> &'static str {
    d.word()
}

/// The verdicts of every term of one record (the module doc's argument).
pub fn rule_descent(ctx: &DescentContext<'_>, input: &RuleInput<'_>) -> RuleDescent {
    let dom = input.domain;
    let n = dom.arity;
    let undecided_all = |reason: String| -> RuleDescent {
        let terms: Vec<TermVerdict> = input
            .rhs
            .iter()
            .map(|_| TermVerdict::Undecided {
                reason: reason.clone(),
            })
            .collect();
        finish(EmptyState::Unknown, terms, None, false, (None, 0))
    };
    if ctx.frame.arity() != n || ctx.cmp.arity() != n {
        return undecided_all(format!(
            "arity: domain {n}, frame {}, comparator {}",
            ctx.frame.arity(),
            ctx.cmp.arity()
        ));
    }
    let target = match parse_pattern(input.target) {
        Ok(t) if t.len() == n => t,
        Ok(t) => return undecided_all(format!("target of {} slots", t.len())),
        Err(e) => return undecided_all(format!("target pattern: {e}")),
    };
    // the top-term excess on the per-slot ranges (index-free on the in-chart part; computed for every record)
    let ranges = point_ranges(dom);
    let (mut top_max, mut top_terms): (Option<i64>, usize) = (None, 0);
    for text in input.rhs {
        if let Some(x) = top_excess(&target, text, &ranges, ctx.frame) {
            top_terms += 1;
            top_max = Some(top_max.map_or(x, |m| m.max(x)));
        }
    }
    // the record's domain: empty = never applies (every term vacuous)
    let (dstate, _, dreason) = emptiness(dom, ctx.frame, &ctx.budget, &[], &[]);
    if dstate == EmptyState::Empty {
        let terms = input
            .rhs
            .iter()
            .map(|_| TermVerdict::Descends {
                hand_down: vec![],
                discharged: vec![],
            })
            .collect();
        return finish(EmptyState::Empty, terms, None, false, (top_max, top_terms));
    }
    let chart: Vec<bool> = dom
        .chart
        .clone()
        .unwrap_or_else(|| ctx.frame.mask().to_vec());
    let support_first = ctx.support_first();
    let mut cache = RecordCache {
        ctx,
        dom,
        cells: HashMap::new(),
    };
    let mut terms = Vec::with_capacity(input.rhs.len());
    let mut hd_bound: Option<i64> = None;
    let mut hd_unbounded = false;
    for (j, text) in input.rhs.iter().enumerate() {
        let term = term_verdict(
            ctx,
            &mut cache,
            input,
            j,
            text,
            &target,
            &chart,
            &ranges,
            support_first,
            dstate,
            dreason.as_deref(),
        );
        if let Some(b) = term.1 {
            match b {
                Some(v) => hd_bound = Some(hd_bound.map_or(v, |h| h.max(v))),
                None => hd_unbounded = true,
            }
        }
        terms.push(term.0);
    }
    finish(dstate, terms, hd_bound, hd_unbounded, (top_max, top_terms))
}

fn finish(
    domain: EmptyState,
    terms: Vec<TermVerdict>,
    hd: Option<i64>,
    hd_unbounded: bool,
    top: (Option<i64>, usize),
) -> RuleDescent {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut worst: Option<&TermVerdict> = None;
    for t in &terms {
        *counts.entry(t.word()).or_default() += 1;
        if worst.is_none_or(|w| t.severity() > w.severity()) {
            worst = Some(t);
        }
    }
    let worst = worst.map_or("descends", TermVerdict::word);
    RuleDescent {
        domain,
        terms,
        counts,
        worst,
        handdown_excess_bound: hd,
        handdown_excess_unbounded: hd_unbounded,
        top_excess_max: top.0,
        top_terms: top.1,
    }
}

/// (D + r of the term) - (D + r of the target) on the term's in-chart part, when that part is non-empty on the per-slot
/// ranges (`None` otherwise or for an index-dependent pattern): a sum of offsets (lines `o - o_t`, numerators `o_t - o`,
/// constants their difference), the same at every in-chart key.
pub fn top_excess(
    target: &[PatternSlot],
    text: &str,
    ranges: &[Range],
    frame: &NodeFrame,
) -> Option<i64> {
    let term = parse_pattern(text).ok()?;
    if term.len() != target.len() {
        return None;
    }
    let mut d: i64 = 0;
    for (s, (&t, &x)) in target.iter().zip(&term).enumerate() {
        let line = frame.is_line(s);
        let r = ranges.get(s).copied().unwrap_or(Range::Any);
        let x = match (t, x) {
            (PatternSlot::Fixed(_), PatternSlot::Symbolic(o)) => match r {
                Range::Exactly(v) => {
                    PatternSlot::Fixed(i32::try_from(v.checked_add(i64::from(o))?).ok()?)
                }
                _ => return None,
            },
            (PatternSlot::Symbolic(_), PatternSlot::Fixed(_)) => return None,
            _ => x,
        };
        match (t, x) {
            (PatternSlot::Fixed(ct), PatternSlot::Fixed(c)) => {
                let (ct, c) = (i64::from(ct), i64::from(c));
                if (line && c <= 0) || (!line && c >= 1) {
                    return None;
                }
                d = d.checked_add(if line { c - ct } else { ct - c })?;
            }
            (PatternSlot::Symbolic(ot), PatternSlot::Symbolic(o)) => {
                let (ot, o) = (i64::from(ot), i64::from(o));
                if line {
                    // in chart: pt + o >= 1 must meet the range
                    if upper_of(r).is_some_and(|h| h + o < 1) {
                        return None;
                    }
                    d = d.checked_add(o - ot)?;
                } else {
                    if lower_of(r).is_some_and(|l| l + o > 0) {
                        return None;
                    }
                    d = d.checked_add(ot - o)?;
                }
            }
            _ => return None,
        }
    }
    Some(d)
}

/// One term: its verdict and, when it can hand down, `Some(bound)` (`Some(None)` = unbounded).
#[allow(clippy::too_many_arguments)]
fn term_verdict(
    ctx: &DescentContext<'_>,
    cache: &mut RecordCache<'_, '_>,
    input: &RuleInput<'_>,
    j: usize,
    text: &str,
    target: &[PatternSlot],
    chart: &[bool],
    ranges: &[Range],
    support_first: bool,
    dstate: EmptyState,
    dreason: Option<&str>,
) -> (TermVerdict, Option<Option<i64>>) {
    let n = target.len();
    let und = |reason: String| (TermVerdict::Undecided { reason }, None);
    let raw = match parse_pattern(text) {
        Ok(p) if p.len() == n => p,
        Ok(p) => return und(format!("term {j}: {} slots", p.len())),
        Err(e) if e.contains(" at position ") => {
            // a slot-permuted term (symmetry alias): the exact comparison on the whole domain (parametric::permuted)
            use crate::parametric::permuted::{PermutedVerdict, permuted_descent};
            return match permuted_descent(
                ctx.cmp,
                ctx.frame,
                input.domain,
                input.target,
                text,
                &ctx.budget,
            ) {
                PermutedVerdict::Lower { .. } => (
                    TermVerdict::Descends {
                        hand_down: vec![],
                        discharged: vec![],
                    },
                    None,
                ),
                PermutedVerdict::NotLower {
                    witness,
                    component,
                    lower_elsewhere: false,
                } => (
                    TermVerdict::DoesNotDescend {
                        witness,
                        comparison: format!(
                            "slot-permuted term not lower anywhere (first deciding component {component})"
                        ),
                    },
                    None,
                ),
                PermutedVerdict::NotLower {
                    witness, component, ..
                } => und(format!(
                    "term {j}: slot-permuted term lower on part of the domain only; not lower at {witness:?} ({component})"
                )),
                PermutedVerdict::Undecided { reason } => {
                    und(format!("term {j}: slot-permuted term: {reason}"))
                }
            };
        }
        Err(e) => return und(format!("term {j}: pattern {e}")),
    };
    // normalise: a symbolic term slot where the target is a constant takes the slot's fixed value
    let mut slots = raw.clone();
    for s in 0..n {
        match (target[s], raw[s]) {
            (PatternSlot::Fixed(_), PatternSlot::Symbolic(o)) => match ranges.get(s) {
                Some(Range::Exactly(v)) => {
                    let Some(x) = v
                        .checked_add(i64::from(o))
                        .and_then(|x| i32::try_from(x).ok())
                    else {
                        return und(format!("term {j}: slot {s} value overflow"));
                    };
                    slots[s] = PatternSlot::Fixed(x);
                }
                _ => {
                    return und(format!(
                        "term {j}: symbolic slot {s} where the target is a constant and the slot is not fixed"
                    ));
                }
            },
            (PatternSlot::Symbolic(_), PatternSlot::Fixed(_)) => {
                return und(format!(
                    "term {j}: constant slot {s} where the target is symbolic (index-dependent comparison)"
                ));
            }
            _ => {}
        }
    }
    let zero = |locus: Locus| -> Option<DenStatus> {
        let z = ctx.zeros?;
        let sha = input.record_sha256?;
        z.proof(sha, u32::try_from(j).ok()?, locus)
    };
    // crossing candidates
    let mut p_cands: Vec<Cand> = Vec::new();
    let mut h_cands: Vec<Cand> = Vec::new();
    let mut a_bounds: Vec<Poly> = Vec::new();
    let mut a_empty = false;
    let mut can_hand_down = false;
    // hand-down excess bound (module doc of RuleDescent)
    let mut delta: i64 = 0;
    let mut delta_unbounded = false;
    for s in 0..n {
        let line = ctx.frame.is_line(s);
        match (target[s], slots[s]) {
            (PatternSlot::Fixed(ct), PatternSlot::Fixed(c)) => {
                let (ct, c) = (i64::from(ct), i64::from(c));
                if line {
                    if c <= 0 {
                        can_hand_down = true;
                        a_empty = true;
                        h_cands.push(Cand {
                            slot: s,
                            param: s,
                            rel: Rel::Domain,
                            value: c,
                        });
                        delta = delta.saturating_add(1 - c - ct);
                    } else {
                        delta = delta.saturating_add(c - ct);
                    }
                } else {
                    if c >= 1 {
                        a_empty = true;
                        p_cands.push(Cand {
                            slot: s,
                            param: s,
                            rel: Rel::Domain,
                            value: c,
                        });
                    }
                    delta = delta.saturating_add(ct - c);
                }
            }
            (PatternSlot::Symbolic(ot), PatternSlot::Symbolic(o)) => {
                let (ot, o) = (i64::from(ot), i64::from(o));
                let r = ranges.get(s).copied().unwrap_or(Range::Any);
                if line {
                    let in_chart = o - ot;
                    // crossing where pt_s <= -o
                    match lower_of(r) {
                        Some(lo) if lo > -o => delta = delta.saturating_add(in_chart),
                        lo => {
                            can_hand_down = true;
                            if let Ok(p) = Poly::var(s).add(&Poly::constant(i128::from(o - 1))) {
                                a_bounds.push(p);
                            } else {
                                return und(format!("term {j}: slot {s} bound"));
                            }
                            match lo {
                                Some(lo) if -o - lo < MAX_PLANES_PER_SLOT => {
                                    for v in lo..=-o {
                                        h_cands.push(Cand {
                                            slot: s,
                                            param: s,
                                            rel: Rel::Eq,
                                            value: v,
                                        });
                                    }
                                    let dropped = 1 - 2 * lo - o - ot;
                                    delta = delta.saturating_add(in_chart.max(dropped));
                                }
                                Some(lo) => {
                                    h_cands.push(Cand {
                                        slot: s,
                                        param: s,
                                        rel: Rel::Le,
                                        value: -o,
                                    });
                                    let dropped = 1 - 2 * lo - o - ot;
                                    delta = delta.saturating_add(in_chart.max(dropped));
                                }
                                None => {
                                    h_cands.push(Cand {
                                        slot: s,
                                        param: s,
                                        rel: Rel::Le,
                                        value: -o,
                                    });
                                    delta_unbounded = true;
                                }
                            }
                        }
                    }
                } else {
                    delta = delta.saturating_add(ot - o);
                    // positive where pt_s >= 1 - o
                    match upper_of(r) {
                        Some(hi) if hi < 1 - o => {}
                        hi => {
                            if let Ok(p) = Poly::constant(i128::from(-o)).sub(&Poly::var(s)) {
                                a_bounds.push(p);
                            } else {
                                return und(format!("term {j}: slot {s} bound"));
                            }
                            match hi {
                                Some(hi) if hi - (1 - o) < MAX_PLANES_PER_SLOT => {
                                    for v in (1 - o)..=hi {
                                        p_cands.push(Cand {
                                            slot: s,
                                            param: s,
                                            rel: Rel::Eq,
                                            value: v,
                                        });
                                    }
                                }
                                _ => p_cands.push(Cand {
                                    slot: s,
                                    param: s,
                                    rel: Rel::Ge,
                                    value: 1 - o,
                                }),
                            }
                        }
                    }
                }
            }
            _ => return und(format!("term {j}: slot {s} kinds")),
        }
    }
    let mut loci = Vec::new();
    let mut discharged = Vec::new();
    let mut hand_down = Vec::new();
    let mut undecided: Option<String> = None;
    let sub = |c: Cand,
               e: EmptyState,
               w: Option<Vec<i32>>,
               z: Option<&'static str>,
               reason: Option<String>| SubLocus {
        slot: c.slot,
        param: c.param,
        relation: c.rel,
        value: c.value,
        emptiness: e,
        witness: w,
        zero_proof: z,
        reason,
    };
    // P: positive numerators
    for c in p_cands {
        let (e, w, reason) = if c.rel == Rel::Domain {
            (dstate, None, dreason.map(str::to_string))
        } else {
            cache.cell(c)
        };
        if e == EmptyState::Empty {
            continue;
        }
        let locus = match c.rel {
            Rel::Eq => u32::try_from(c.param).ok().map(|param| Locus::Plane {
                param,
                value: c.value,
            }),
            Rel::Domain => Some(Locus::Domain),
            Rel::Le | Rel::Ge => None,
        };
        let z = match locus {
            Some(l) => zero(l),
            None => zero(Locus::Domain),
        };
        match z {
            Some(d) => discharged.push(sub(c, e, w, Some(den_word(d)), None)),
            None => loci.push(sub(c, e, w, None, reason)),
        }
    }
    // H: hand-down sub-loci
    for c in h_cands {
        let (e, w, reason) = if c.rel == Rel::Domain {
            (dstate, None, dreason.map(str::to_string))
        } else {
            cache.cell(c)
        };
        if e == EmptyState::Empty {
            continue;
        }
        if !support_first {
            undecided = Some(format!(
                "term {j}: the order has pre-support degree rows: a hand-down term (slot {}) need not be lower",
                c.slot
            ));
        }
        hand_down.push(sub(c, e, w, None, reason));
    }
    // A: in chart
    match ctx.cmp.cmp_pattern(chart, &slots, target) {
        Ok(Ordering::Less) => {}
        Ok(o) => {
            if !a_empty {
                let (e, w, reason) = emptiness(cache.dom, ctx.frame, &ctx.budget, &[], &a_bounds);
                match e {
                    EmptyState::Empty => {}
                    EmptyState::NonEmpty => {
                        if let Some(d) = zero(Locus::Domain) {
                            discharged.push(SubLocus {
                                slot: usize::MAX,
                                param: usize::MAX,
                                relation: Rel::Domain,
                                value: 0,
                                emptiness: e,
                                witness: w,
                                zero_proof: Some(den_word(d)),
                                reason: Some(format!("in chart: comparison {o:?}")),
                            });
                        } else {
                            let w = w.unwrap_or_default();
                            // the concrete comparator must agree at the witness
                            let mut pt = Vec::new();
                            if !rule_point(cache.dom, &w, &mut pt) {
                                return und(format!(
                                    "term {j}: no rule point at the witness {w:?}"
                                ));
                            }
                            let mut tk = Vec::with_capacity(n);
                            for (s, slot) in slots.iter().enumerate() {
                                let v = match *slot {
                                    PatternSlot::Fixed(c) => Some(c),
                                    PatternSlot::Symbolic(off) => pt[s].checked_add(off),
                                };
                                match v {
                                    Some(v) => tk.push(v),
                                    None => {
                                        return und(format!("term {j}: overflow at the witness"));
                                    }
                                }
                            }
                            if ctx.cmp.cmp(&tk, &w) == Ordering::Less {
                                return und(format!(
                                    "term {j}: the pattern comparison ({o:?}) disagrees with the concrete comparator at {w:?}"
                                ));
                            }
                            return (
                                TermVerdict::DoesNotDescend {
                                    witness: w,
                                    comparison: format!("{o:?}"),
                                },
                                can_hand_down.then_some((!delta_unbounded).then_some(delta)),
                            );
                        }
                    }
                    EmptyState::Unknown => {
                        undecided = Some(format!(
                            "term {j}: not lower in the chart ({o:?}) and the in-chart part's emptiness is unknown: {}",
                            reason.unwrap_or_default()
                        ));
                    }
                }
            }
        }
        Err(e) => undecided = Some(format!("term {j}: pattern comparison: {e:?}")),
    }
    let hd = can_hand_down.then_some((!delta_unbounded).then_some(delta));
    if let Some(reason) = undecided {
        return (TermVerdict::Undecided { reason }, hd);
    }
    if loci.is_empty() {
        (
            TermVerdict::Descends {
                hand_down,
                discharged,
            },
            hd,
        )
    } else {
        (
            TermVerdict::DescendsExcept {
                loci,
                hand_down,
                discharged,
            },
            hd,
        )
    }
}

/// Read a record line (exact bytes at `offset`, `bytes` long; `bytes == 0`: up to the newline) and return (target,
/// rhs patterns, sha256 hex of the line without its newline).
pub fn read_record(
    path: &Path,
    offset: u64,
    bytes: u64,
) -> Result<(String, Vec<String>, String), String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = Vec::new();
    if bytes > 0 {
        let len = usize::try_from(bytes).map_err(|_| "record too long".to_string())?;
        buf.resize(len, 0);
        f.read_exact(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    } else {
        let mut r = std::io::BufReader::new(f);
        r.read_until(b'\n', &mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    while buf.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        buf.pop();
    }
    let mut h = Sha256::new();
    h.update(&buf);
    let sha = h.finish_hex();
    let v: Value = serde_json::from_slice(&buf).map_err(|e| format!("record JSON: {e}"))?;
    let target = v
        .get("target")
        .and_then(Value::as_str)
        .ok_or("record without a target")?
        .to_string();
    let rhs_v = match v.get("rhs") {
        Some(Value::Array(a)) => a.clone(),
        Some(Value::String(s)) => match serde_json::from_str::<Value>(s) {
            Ok(Value::Array(a)) => a,
            _ => return Err("rhs string is not an array".into()),
        },
        _ => return Err("record without an rhs".into()),
    };
    let rhs = rhs_v
        .iter()
        .map(|t| {
            t.get(0)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| "rhs term is not [pattern, coefficient]".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((target, rhs, sha))
}

/// The JSON line of one record's verdict (the evidence format of interfaces/parametric.md section 1). Terms that
/// plainly descend with no sub-locus are only counted; every other term is listed in full.
pub fn record_line(meta: &Value, d: &RuleDescent) -> Value {
    let mut listed = Vec::new();
    let (mut hd_terms, mut discharged_terms) = (0usize, 0usize);
    for (j, t) in d.terms.iter().enumerate() {
        match t {
            TermVerdict::Descends {
                hand_down,
                discharged,
            } => {
                hd_terms += usize::from(!hand_down.is_empty());
                discharged_terms += usize::from(!discharged.is_empty());
            }
            other => listed.push(json!({"term": j, "v": other})),
        }
    }
    json!({
        "record": meta,
        "domain": d.domain,
        "terms": d.terms.len(),
        "counts": d.counts,
        "worst": d.worst,
        "descends_with_hand_down": hd_terms,
        "descends_with_zero_proofs": discharged_terms,
        "handdown_excess_bound": d.handdown_excess_bound,
        "handdown_excess_unbounded": d.handdown_excess_unbounded,
        "top_excess_max": d.top_excess_max,
        "top_terms": d.top_terms,
        "listed": listed,
    })
}

/// One record of a table set, prepared for a run (the dispatcher's domain and the record's provenance).
#[derive(Clone, Debug)]
pub struct RecordJob {
    pub table: usize,
    pub record: usize,
    pub domain: CaseDomain,
    pub path: String,
    pub offset: u64,
    pub bytes: u64,
    /// sha256 hex (lower case) the import recorded for the raw line (checked against the bytes read).
    pub record_sha256: String,
    /// Provenance shown in the evidence (`source`, `class`, `label`, `hand_name`, ...).
    pub meta: Value,
}

/// The jobs of every DISPATCHED record of an imported set, in dispatch order.
pub fn jobs_of(set: &crate::import::ImportedSet) -> Vec<RecordJob> {
    let mut out = Vec::new();
    for &t in set.disp.priority() {
        for (r, dom) in set.disp.tables()[t].domains.iter().enumerate() {
            let info = set.rule(t, r);
            let sha = info
                .record_sha256
                .trim_start_matches("sha256:")
                .to_ascii_lowercase();
            out.push(RecordJob {
                table: t,
                record: r,
                domain: dom.clone(),
                path: info.path.clone(),
                offset: info.offset,
                bytes: info.bytes,
                record_sha256: sha,
                meta: json!({
                    "table": t, "record": r, "source": info.source, "class": info.class,
                    "table_name": info.table_name, "label": info.label, "index": info.index,
                    "hand_name": info.hand_name, "path": info.path, "offset": info.offset,
                    "record_sha256": info.record_sha256,
                }),
            });
        }
    }
    out
}

/// Run B1 on `jobs` with `threads` workers; returns one evidence line per job (in job order). A record whose bytes do
/// not hash to its recorded sha256, or cannot be read, gives an `error` line (every term undecided).
pub fn run_jobs(
    jobs: &[RecordJob],
    frame: &NodeFrame,
    cmp: &NodeComparator,
    zeros: Option<&ZeroIndex>,
    budget: &ProofBudget,
    threads: usize,
) -> Vec<JobResult> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering as AO};
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<JobResult>>> = Mutex::new(vec![None; jobs.len()]);
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| {
                let ctx = DescentContext {
                    frame,
                    cmp,
                    zeros,
                    budget: budget.clone(),
                };
                loop {
                    let i = next.fetch_add(1, AO::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    let res = match read_record(Path::new(&job.path), job.offset, job.bytes) {
                        Err(e) => (json!({"record": job.meta, "error": e}), None),
                        Ok((_, _, sha)) if sha != job.record_sha256 => (
                            json!({"record": job.meta, "error": format!("record bytes hash to {sha}, the import recorded {}", job.record_sha256)}),
                            None,
                        ),
                        Ok((target, rhs, sha)) => {
                            let d = rule_descent(
                                &ctx,
                                &RuleInput {
                                    domain: &job.domain,
                                    target: &target,
                                    rhs: &rhs,
                                    record_sha256: Some(&sha),
                                },
                            );
                            let mut line = record_line(&job.meta, &d);
                            if let Some(z) = zeros {
                                line["zero_proofs_bound"] = json!(z.has_record(&sha));
                            }
                            (line, Some(d))
                        }
                    };
                    if let Ok(mut o) = out.lock() {
                        o[i] = Some(res);
                    }
                }
            });
        }
    });
    out.into_inner()
        .unwrap_or_default()
        .into_iter()
        .map(|x| x.unwrap_or((json!({"error": "not run"}), None)))
        .collect()
}

/// The summary of a run: per worst verdict (records), per verdict (terms), per class, the exceptional sub-loci and
/// every does-not-descend / undecided term (capped lists with totals), and the hand-down excess bounds.
pub fn summarize(results: &[JobResult], max_listed: usize) -> Value {
    let mut by_worst: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_class: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut terms: BTreeMap<String, usize> = BTreeMap::new();
    let (mut errors, mut loci_n, mut discharged_n, mut hd_terms) = (0usize, 0usize, 0usize, 0usize);
    let mut by_den: BTreeMap<String, usize> = BTreeMap::new();
    let mut dnd = Vec::new();
    let mut und: BTreeMap<String, usize> = BTreeMap::new();
    let mut und_examples = Vec::new();
    let mut except_examples = Vec::new();
    let mut hd_max: Option<i64> = None;
    let mut hd_unbounded = 0usize;
    let mut hd_hist: BTreeMap<i64, usize> = BTreeMap::new();
    let mut empty_domains = 0usize;
    let mut top_hist: BTreeMap<i64, usize> = BTreeMap::new();
    let mut top_terms_total = 0usize;
    for (line, d) in results {
        let class = line["record"]["class"].as_str().unwrap_or("?").to_string();
        let Some(d) = d else {
            errors += 1;
            *by_class
                .entry(class)
                .or_default()
                .entry("error".into())
                .or_default() += 1;
            continue;
        };
        if d.domain == EmptyState::Empty {
            empty_domains += 1;
        }
        *by_worst.entry(d.worst.into()).or_default() += 1;
        *by_class
            .entry(class)
            .or_default()
            .entry(d.worst.into())
            .or_default() += 1;
        if let Some(b) = d.handdown_excess_bound {
            hd_max = Some(hd_max.map_or(b, |m| m.max(b)));
            *hd_hist.entry(b).or_default() += 1;
        }
        hd_unbounded += usize::from(d.handdown_excess_unbounded);
        if let Some(t) = d.top_excess_max {
            *top_hist.entry(t).or_default() += 1;
        }
        top_terms_total += d.top_terms;
        for (j, t) in d.terms.iter().enumerate() {
            *terms.entry(t.word().into()).or_default() += 1;
            match t {
                TermVerdict::Descends {
                    hand_down,
                    discharged,
                } => {
                    discharged_n += discharged.len();
                    for x in discharged {
                        *by_den
                            .entry(x.zero_proof.unwrap_or("?").into())
                            .or_default() += 1;
                    }
                    hd_terms += usize::from(!hand_down.is_empty());
                }
                TermVerdict::DescendsExcept {
                    loci, discharged, ..
                } => {
                    loci_n += loci.len();
                    discharged_n += discharged.len();
                    for x in discharged {
                        *by_den
                            .entry(x.zero_proof.unwrap_or("?").into())
                            .or_default() += 1;
                    }
                    if except_examples.len() < max_listed {
                        except_examples
                            .push(json!({"record": line["record"], "term": j, "loci": loci}));
                    }
                }
                TermVerdict::DoesNotDescend { .. } => {
                    if dnd.len() < max_listed {
                        dnd.push(json!({"record": line["record"], "term": j, "v": t}));
                    }
                }
                TermVerdict::Undecided { reason } => {
                    // the reason's category: the text after "term <j>: " up to the first ':' or '('
                    let cat: String = reason
                        .split_once(": ")
                        .map_or(reason.as_str(), |(_, r)| r)
                        .chars()
                        .take_while(|c| *c != ':' && *c != '(')
                        .collect();
                    *und.entry(cat.trim().to_string()).or_default() += 1;
                    if und_examples.len() < max_listed {
                        und_examples
                            .push(json!({"record": line["record"], "term": j, "reason": reason}));
                    }
                }
            }
        }
    }
    json!({
        "records": results.len(),
        "records_by_worst": by_worst,
        "records_by_class": by_class,
        "record_errors": errors,
        "records_with_empty_domain": empty_domains,
        "terms_by_verdict": terms,
        "descending_terms_with_hand_down": hd_terms,
        "subloci_discharged_by_zero_proofs": discharged_n,
        "discharged_by_denominator_class": by_den,
        "exceptional_subloci": loci_n,
        "undecided_by_reason": und,
        "does_not_descend_listed": dnd,
        "undecided_listed": und_examples,
        "descends_except_listed": except_examples,
        "handdown_excess_bound_max": hd_max,
        "handdown_excess_bound_histogram": hd_hist,
        "records_with_unbounded_handdown_excess": hd_unbounded,
        "top_excess_max_histogram": top_hist,
        "top_terms": top_terms_total,
    })
}
