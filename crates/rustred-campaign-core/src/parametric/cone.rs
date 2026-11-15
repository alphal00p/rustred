//! B2: coverage of a stated CONE of keys at FIRST MATCH (interfaces/parametric.md section 2).
//!
//! THE ARGUMENT. The dispatch-mode partition of the dispatcher's rule tables ([`crate::partition::gap_partition`] with
//! `CoverageNotion::DispatchCoverage`, `ElidedPolicy::FailClosed`, exact literal tests, no rule subset, no cap: the
//! options of the minting function, partfix4, only the budget is the caller's) describes the dispatch-UNCOVERED set of
//! the WHOLE top-sector chart orthant exactly, as a finite list of GAP ELEMENTS: uncovered classes (cells: products of
//! per-slot intervals), resolved fragile pieces and unresolved parts (lattice regions `x = x0 + M t, A t + b >= 0`),
//! the same for every blocked sub-partition (keys where the first selected record is blocked), and unexplored classes.
//! A key of the cone is uncovered iff it lies in one element. Each element is intersected with the cone EXACTLY: the
//! cone's linear constraints are substituted into the element's region; rational Fourier-Motzkin infeasibility or an
//! exhaustive search of a bounded region proves the intersection EMPTY; an integer point of the intersection that the
//! dispatcher's own `first_match` reports OPEN is a verified WITNESS (an element may be a superset of its uncovered
//! keys, e.g. a cell minus covered sub-loci, so a point is only a witness after that check); anything else is
//! UNDECIDED. Covered = the partition run is complete (CompleteRun token, no caveat) and every element misses the cone.
//! Nothing is sampled: the probe keys of the certificate are a cross-check for the independent checker.

use serde::Serialize;
use serde_json::{Value, json};

use crate::cover::{Dispatch, Dispatcher};
use crate::key::NodeFrame;
use crate::loci::region::{Region, find_point};
use crate::partition::classes::Label;
use crate::partition::exact::Affine;
use crate::partition::{
    Budget, CoverageNotion, DispatcherSource, DomainSource, ElidedPolicy, LiteralTests,
    PartitionOptions, PartitionReport, WorkMeter, gap_partition,
};

/// The cone: the frame's top-sector chart orthant intersected with `sum_i a_i k_i <= b` for every constraint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Cone {
    /// Human label (e.g. "D + r <= 16").
    pub label: String,
    /// `(a, b)`: `sum_i a[i] * k[i] <= b`.
    pub constraints: Vec<(Vec<i64>, i64)>,
}

impl Cone {
    /// The whole top-sector chart orthant (no constraint).
    pub fn orthant() -> Cone {
        Cone {
            label: "the top-sector chart orthant".into(),
            constraints: vec![],
        }
    }

    /// `D + r <= s`: dots `sum_lines (k - 1)` plus rank `sum_numerators (-k)` at most `s`.
    pub fn excess_simplex(frame: &NodeFrame, s: i64) -> Cone {
        let n = frame.arity();
        let a: Vec<i64> = (0..n)
            .map(|i| if frame.is_line(i) { 1 } else { -1 })
            .collect();
        let lines = (0..n).filter(|&i| frame.is_line(i)).count() as i64;
        Cone {
            label: format!("D + r <= {s}"),
            constraints: vec![(a, s.saturating_add(lines))],
        }
    }

    /// Per-slot caps: every line `<= line_max`, every numerator `>= num_min`.
    pub fn box_caps(frame: &NodeFrame, line_max: i64, num_min: i64) -> Cone {
        let n = frame.arity();
        let mut constraints = Vec::new();
        for i in 0..n {
            let mut a = vec![0i64; n];
            if frame.is_line(i) {
                a[i] = 1;
                constraints.push((a, line_max));
            } else {
                a[i] = -1;
                constraints.push((a, num_min.saturating_neg()));
            }
        }
        Cone {
            label: format!("lines <= {line_max}, numerators >= {num_min}"),
            constraints,
        }
    }

    /// Intersection with another cone.
    pub fn and(mut self, o: &Cone) -> Cone {
        self.label = format!("{} and {}", self.label, o.label);
        self.constraints.extend(o.constraints.iter().cloned());
        self
    }

    /// Exact membership (i128 arithmetic; the chart orthant is the caller's).
    pub fn contains(&self, k: &[i32]) -> bool {
        self.constraints.iter().all(|(a, b)| {
            let s: Option<i128> = a.iter().zip(k).try_fold(0i128, |acc, (&x, &y)| {
                acc.checked_add(i128::from(x).checked_mul(i128::from(y))?)
            });
            s.is_some_and(|s| s <= i128::from(*b))
        })
    }
}

/// Work limits of a cone-coverage run.
#[derive(Clone, Debug)]
pub struct ConeOptions {
    /// The partition's budget (work units, wall clock, memo).
    pub budget: Budget,
    /// Points tried per element before it is declared undecided.
    pub point_tries: usize,
    /// Values of the exhaustive search of a bounded intersection.
    pub enumeration: u64,
}

impl Default for ConeOptions {
    fn default() -> Self {
        ConeOptions {
            budget: Budget {
                max_nodes: Some(20_000_000),
                wall: None,
                max_memo: Some(100_000),
                max_faces: None,
            },
            point_tries: 48,
            enumeration: 200_000,
        }
    }
}

/// One gap element of the partition.
#[derive(Clone, Debug)]
pub struct GapElement {
    /// `generic` (uncovered class), `locus` (resolved fragile piece), `unresolved`, `unexplored`.
    pub kind: &'static str,
    /// The blocking record (dispatch position) of a blocked sub-partition, if the element is one.
    pub blocked_by: Option<usize>,
    /// The element's region (cells: one parameter per slot, interval constraints).
    pub region: Region,
    /// Uncovered keys the partition found in it (oracle witnesses, never a decision).
    pub seeds: Vec<Vec<i32>>,
    /// Text for the report (state intervals, cut trail, reason).
    pub note: String,
}

/// The verdict of one element within the cone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum ElementVerdict {
    /// The element and the cone are disjoint (proved).
    OutsideCone,
    /// A key of the cone in the element, OPEN for the dispatcher's first match.
    Uncovered {
        witness: Vec<i32>,
    },
    Undecided {
        reason: String,
    },
}

/// The result of a cone-coverage run.
#[derive(Clone, Debug, Serialize)]
pub struct ConeCoverage {
    /// `covered` / `uncovered` / `undecided`.
    pub verdict: &'static str,
    pub cone: Cone,
    pub elements: usize,
    pub outside: usize,
    /// (element description, verdict) of every element meeting the cone or undecided.
    pub listed: Vec<Value>,
    pub uncovered: usize,
    pub undecided: usize,
    /// Why the partition run is not exact (empty = exact).
    pub partition_flags: Vec<String>,
    pub partition: Value,
    /// Probe keys inside the cone with the dispatcher's answer (`[table, record]` or null = open) and, for an open
    /// key, the listed element that contains it (null = not explained).
    pub probes: Vec<Value>,
    pub probes_open_unexplained: usize,
}

fn identity_region(n: usize, lo: &[Option<i64>], hi: &[Option<i64>]) -> Region {
    let mut x = Vec::with_capacity(n);
    let mut cons = Vec::new();
    for s in 0..n {
        let mut coef = vec![0i128; n];
        coef[s] = 1;
        x.push(Affine {
            coef: coef.clone(),
            c: 0,
        });
        if let Some(l) = lo[s] {
            cons.push(Affine {
                coef: coef.clone(),
                c: -i128::from(l),
            });
        }
        if let Some(h) = hi[s] {
            let neg: Vec<i128> = coef.iter().map(|v| -v).collect();
            cons.push(Affine {
                coef: neg,
                c: i128::from(h),
            });
        }
    }
    Region {
        k: n,
        x,
        cons,
        names: (0..n).map(|s| format!("n{s}")).collect(),
    }
}

/// The cell of a class: per slot its state interval intersected with the chart side.
fn class_cell(
    rep: &PartitionReport,
    frame: &NodeFrame,
    states: &[Option<u16>],
) -> (Region, String) {
    let n = frame.arity();
    let mut lo = vec![None; n];
    let mut hi = vec![None; n];
    let mut text = Vec::with_capacity(n);
    for s in 0..n {
        let (cl, ch) = if frame.is_line(s) {
            (Some(1i64), None)
        } else {
            (None, Some(0i64))
        };
        let (l, h) = match states.get(s).copied().flatten() {
            Some(i) => {
                let r = rep.states()[s][usize::from(i)];
                (r.lo, r.hi)
            }
            None => (None, None),
        };
        lo[s] = match (l, cl) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        hi[s] = match (h, ch) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        text.push(format!(
            "[{},{}]",
            lo[s].map_or("-inf".into(), |v| v.to_string()),
            hi[s].map_or("inf".into(), |v| v.to_string())
        ));
    }
    (identity_region(n, &lo, &hi), text.join(" "))
}

/// Every gap element of `rep` and of its blocked sub-partitions.
pub fn gap_elements(
    rep: &PartitionReport,
    frame: &NodeFrame,
    blocked_by: Option<usize>,
    out: &mut Vec<GapElement>,
) {
    for c in rep.classes() {
        let kind = match c.label {
            Label::Uncovered => "generic",
            Label::Unexplored => "unexplored",
            Label::Fragile => continue,
        };
        let (region, note) = class_cell(rep, frame, &c.states);
        out.push(GapElement {
            kind,
            blocked_by,
            region,
            seeds: vec![],
            note,
        });
    }
    if let Some(res) = rep.resolution() {
        for l in &res.loci {
            for p in &l.pieces {
                out.push(GapElement {
                    kind: "locus",
                    blocked_by,
                    region: p.region.clone(),
                    seeds: p.witness.clone(),
                    note: format!(
                        "locus {} face {:?}: trail {:?} minus {:?}{}",
                        l.id,
                        l.face,
                        p.trail,
                        p.minus,
                        if p.covered_part_unresolved {
                            " (superset: covered part unresolved)"
                        } else {
                            ""
                        }
                    ),
                });
            }
            for u in &l.unresolved_parts {
                out.push(GapElement {
                    kind: "unresolved",
                    blocked_by,
                    region: u.region.clone(),
                    seeds: u.points.clone(),
                    note: format!("locus {}: {} ({})", l.id, u.reason, u.equation),
                });
            }
        }
    }
    for (src, sub) in rep.blocked() {
        gap_elements(sub, frame, Some(*src), out);
    }
}

/// The structured region `{k, x: [[coef], c].., cons: [[coef], c]..}` (null when a number exceeds i64).
pub fn region_json(r: &Region) -> Value {
    let aff = |a: &Affine| -> Option<Value> {
        let coef = a
            .coef
            .iter()
            .map(|v| i64::try_from(*v).ok())
            .collect::<Option<Vec<i64>>>()?;
        Some(json!([coef, i64::try_from(a.c).ok()?]))
    };
    let x = r.x.iter().map(aff).collect::<Option<Vec<_>>>();
    let cons = r.cons.iter().map(aff).collect::<Option<Vec<_>>>();
    match (x, cons) {
        (Some(x), Some(cons)) => json!({"k": r.k, "x": x, "cons": cons}),
        _ => Value::Null,
    }
}

/// The element's region with the cone's constraints added (in the region's parameters).
pub fn with_cone(r: &Region, cone: &Cone) -> Option<Region> {
    let mut out = r.clone();
    for (a, b) in &cone.constraints {
        // b - sum_i a_i x_i(t) >= 0
        let mut coef = vec![0i128; r.k];
        let mut c = i128::from(*b);
        for (i, &ai) in a.iter().enumerate() {
            if ai == 0 {
                continue;
            }
            let xi = r.x.get(i)?;
            let ai = i128::from(ai);
            for (j, v) in xi.coef.iter().enumerate() {
                coef[j] = coef[j].checked_sub(ai.checked_mul(*v)?)?;
            }
            c = c.checked_sub(ai.checked_mul(xi.c)?)?;
        }
        out.cons.push(Affine { coef, c });
    }
    Some(out)
}

fn key_of(r: &Region, t: &[i128]) -> Option<Vec<i32>> {
    r.at(t)
        .ok()?
        .into_iter()
        .map(|v| i32::try_from(v).ok())
        .collect()
}

fn is_open(disp: &Dispatcher, key: &[i32]) -> bool {
    !matches!(disp.first_match(key), Ok(Dispatch::Covered(_)))
}

/// Decide one element within the cone (module doc).
pub fn decide_element(
    disp: &Dispatcher,
    frame: &NodeFrame,
    el: &GapElement,
    cone: &Cone,
    opts: &ConeOptions,
) -> ElementVerdict {
    let in_orthant = |k: &[i32]| {
        k.iter()
            .enumerate()
            .all(|(i, &v)| if frame.is_line(i) { v >= 1 } else { v <= 0 })
    };
    for s in &el.seeds {
        if cone.contains(s) && in_orthant(s) && is_open(disp, s) {
            return ElementVerdict::Uncovered { witness: s.clone() };
        }
    }
    let Some(reg) = with_cone(&el.region, cone) else {
        return ElementVerdict::Undecided {
            reason: "overflow while adding the cone constraints".into(),
        };
    };
    let first = match reg.decide_point(opts.point_tries, opts.enumeration) {
        Ok(Some(None)) => return ElementVerdict::OutsideCone,
        Ok(Some(Some(t))) => t,
        Ok(None) => {
            return ElementVerdict::Undecided {
                reason: "the element's intersection with the cone is neither proved empty nor shown non-empty".into(),
            };
        }
        Err(e) => {
            return ElementVerdict::Undecided {
                reason: format!("exact arithmetic: {e:?}"),
            };
        }
    };
    // a point of the intersection: open = witness; covered (the element is a superset there): try other points
    let mut anchors: Vec<Vec<i128>> = vec![first.clone()];
    for j in 0..reg.k {
        for d in [1i128, -1, 2, -2, 5, -5] {
            let mut a = first.clone();
            a[j] = a[j].saturating_add(d);
            anchors.push(a);
        }
    }
    let mut tried = 0usize;
    for a in anchors {
        if tried >= opts.point_tries.max(1) * 4 {
            break;
        }
        tried += 1;
        let t = if a == first {
            Some(first.clone())
        } else {
            find_point(&reg.cons, reg.k, 4, &a).ok().flatten()
        };
        let Some(t) = t else { continue };
        if !reg.contains_t(&t).unwrap_or(false) {
            continue;
        }
        if let Some(k) = key_of(&reg, &t)
            && in_orthant(&k)
            && cone.contains(&k)
            && is_open(disp, &k)
        {
            return ElementVerdict::Uncovered { witness: k };
        }
    }
    ElementVerdict::Undecided {
        reason: format!(
            "the element meets the cone but the {tried} point(s) tried there are covered (the element is a superset of its uncovered keys)"
        ),
    }
}

/// Exact membership of a key in a region (main side: hyperplane cuts with the exact lattice machinery).
pub fn region_contains_key(r: &Region, key: &[i32]) -> Option<bool> {
    let mut cur = r.clone();
    for s in 0..r.x.len() {
        // x_s(t) - key_s = 0, re-expressed in the current parameters: substitute through cur.x
        let target = i128::from(*key.get(s)?);
        let a = cur.x.get(s)?.clone();
        if a.coef.iter().all(|v| *v == 0) {
            if a.c != target {
                return Some(false);
            }
            continue;
        }
        match cur.cut(&a.coef, a.c.checked_sub(target)?) {
            Ok(Some(next)) => cur = next,
            Ok(None) => return Some(false),
            Err(_) => return None,
        }
    }
    match cur.decide_point(8, 10_000) {
        Ok(Some(Some(_))) => Some(true),
        Ok(Some(None)) => Some(false),
        _ => None,
    }
}

/// The dispatch-mode partition options of the minting function, with the caller's budget only.
pub fn partition_options(budget: &Budget) -> PartitionOptions {
    PartitionOptions {
        elided: ElidedPolicy::FailClosed,
        literal_tests: LiteralTests::Exact,
        memo: true,
        max_off_floor: None,
        rule_subset: None,
        dedup: true,
        factor_exceptions: false,
        resolve_loci: true,
        loci: crate::loci::LociOptions {
            deadline: budget
                .wall
                .and_then(|w| std::time::Instant::now().checked_add(w)),
            ..Default::default()
        },
        budget: budget.clone(),
        extra_critical: None,
        coverage: CoverageNotion::DispatchCoverage,
        max_any_slots: usize::MAX,
    }
}

/// The partition a cone-coverage run rests on (cone-independent: one partition serves several cones).
pub struct ConePartition {
    pub report: PartitionReport,
    pub elements: Vec<GapElement>,
    /// Why the partition run is not exact (empty = exact).
    pub flags: Vec<String>,
    pub millis: u64,
}

/// Run the dispatch-mode partition of `disp`'s rule tables (the minting function's options, the caller's budget).
pub fn partition_for_cones(
    disp: &Dispatcher,
    frame: &NodeFrame,
    opts: &ConeOptions,
) -> Result<ConePartition, String> {
    let src = DispatcherSource::new(disp);
    if src.arity() != frame.arity() {
        return Err(format!(
            "dispatcher arity {}, frame {}",
            src.arity(),
            frame.arity()
        ));
    }
    if frame.has_eps_axes() {
        return Err("eps frames are not modelled by the partition".into());
    }
    let t0 = std::time::Instant::now();
    let report = gap_partition(&src, frame, &partition_options(&opts.budget), None)
        .map_err(|e| format!("partition: {e:?}"))?;
    let mut flags = Vec::new();
    if let Err(e) = report.complete_run() {
        flags.push(format!("not a complete run: {e}"));
    }
    if !report.caveats().is_empty() {
        flags.push(format!("caveats: {}", report.caveats().join("; ")));
    }
    let mut elements = Vec::new();
    gap_elements(&report, frame, None, &mut elements);
    Ok(ConePartition {
        report,
        elements,
        flags,
        millis: u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// Membership with a fast path for cells (identity parameter map: the constraints are evaluated at the key).
fn element_contains(el: &GapElement, key: &[i32]) -> Option<bool> {
    let r = &el.region;
    let identity = r.k == key.len()
        && r.x.iter().enumerate().all(|(s, a)| {
            a.c == 0
                && a.coef
                    .iter()
                    .enumerate()
                    .all(|(j, v)| *v == i128::from(j == s))
        });
    if identity {
        let t: Vec<i128> = key.iter().map(|v| i128::from(*v)).collect();
        return r.contains_t(&t).ok();
    }
    region_contains_key(r, key)
}

/// Open probe keys whose element is searched at most (beyond: not listed in the certificate, counted).
pub const MAX_OPEN_PROBES_EXPLAINED: usize = 2_000;

/// The coverage of one cone from a partition (module doc).
pub fn coverage_from_partition(
    cp: &ConePartition,
    disp: &Dispatcher,
    frame: &NodeFrame,
    cone: &Cone,
    opts: &ConeOptions,
) -> Result<ConeCoverage, String> {
    for (a, _) in &cone.constraints {
        if a.len() != frame.arity() {
            return Err(format!("cone constraint of length {}", a.len()));
        }
    }
    let t0 = std::time::Instant::now();
    let els = &cp.elements;
    let mut flags = cp.flags.clone();
    let mut listed = Vec::new();
    let (mut outside, mut unc, mut und) = (0usize, 0usize, 0usize);
    let mut verdicts = Vec::with_capacity(els.len());
    for (i, el) in els.iter().enumerate() {
        let v = decide_element(disp, frame, el, cone, opts);
        match &v {
            ElementVerdict::OutsideCone => outside += 1,
            ElementVerdict::Uncovered { .. } => unc += 1,
            ElementVerdict::Undecided { .. } => und += 1,
        }
        if v != ElementVerdict::OutsideCone {
            listed.push(json!({
                "element": i, "kind": el.kind, "blocked_by": el.blocked_by, "note": el.note,
                "x": el.region.render_x(), "constraints": el.region.render_cons(), "region": region_json(&el.region), "v": v,
            }));
        }
        verdicts.push(v);
    }
    if els
        .iter()
        .zip(&verdicts)
        .any(|(e, v)| e.kind == "unexplored" && *v != ElementVerdict::OutsideCone)
    {
        flags.push("unexplored classes meet the cone".into());
    }
    // probes: the partition's own probe keys and the witnesses
    let meter = WorkMeter::unlimited();
    let mut keys = cp.report.dispatch_probes(&meter);
    for v in &verdicts {
        if let ElementVerdict::Uncovered { witness } = v {
            keys.push(witness.clone());
        }
    }
    keys.sort();
    keys.dedup();
    let in_orthant = |k: &[i32]| {
        k.iter()
            .enumerate()
            .all(|(i, &v)| if frame.is_line(i) { v >= 1 } else { v <= 0 })
    };
    let mut probes = Vec::new();
    let (mut unexplained, mut open_explained, mut open_not_listed) = (0usize, 0usize, 0usize);
    for k in keys
        .into_iter()
        .filter(|k| in_orthant(k) && cone.contains(k))
    {
        match disp.first_match(&k) {
            Ok(Dispatch::Covered(r)) => {
                probes.push(json!({"key": k, "answer": [r.table, r.record]}));
            }
            other => {
                if open_explained + unexplained >= MAX_OPEN_PROBES_EXPLAINED {
                    open_not_listed += 1;
                    continue;
                }
                let mut by = Value::Null;
                for (i, (el, v)) in els.iter().zip(&verdicts).enumerate() {
                    if *v == ElementVerdict::OutsideCone {
                        continue;
                    }
                    if element_contains(el, &k) == Some(true) {
                        by = json!(i);
                        break;
                    }
                }
                if by.is_null() {
                    unexplained += 1;
                } else {
                    open_explained += 1;
                }
                probes.push(json!({"key": k, "answer": Value::Null, "dispatch": format!("{other:?}"), "element": by}));
            }
        }
    }
    if unexplained > 0 {
        flags.push(format!(
            "{unexplained} open probe key(s) in the cone not inside any listed element"
        ));
    }
    let verdict = if unc > 0 {
        "uncovered"
    } else if und > 0 || !flags.is_empty() {
        "undecided"
    } else {
        "covered"
    };
    let rep = &cp.report;
    let partition = json!({
        "outcome": rep.outcome().tag(), "outcome_reasons": rep.outcome().reasons(),
        "nodes": rep.stats().nodes, "classes": rep.classes().len(), "blocked": rep.blocked().len(),
        "caveats": rep.caveats(), "millis": cp.millis, "elements": els.len(),
        "cone_millis": u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX),
        "open_probes_explained": open_explained, "open_probes_not_listed": open_not_listed,
    });
    Ok(ConeCoverage {
        verdict,
        cone: cone.clone(),
        elements: els.len(),
        outside,
        listed,
        uncovered: unc,
        undecided: und,
        partition_flags: flags,
        partition,
        probes,
        probes_open_unexplained: unexplained,
    })
}

/// B2: coverage of `cone` at first match by `disp`'s rule tables (module doc): [`partition_for_cones`] then
/// [`coverage_from_partition`].
pub fn cone_coverage(
    disp: &Dispatcher,
    frame: &NodeFrame,
    cone: &Cone,
    opts: &ConeOptions,
) -> Result<ConeCoverage, String> {
    let cp = partition_for_cones(disp, frame, opts)?;
    coverage_from_partition(&cp, disp, frame, cone, opts)
}

/// The certificate of a run (`rustred.campaign.cone-coverage.v1`), bound to the tables digest.
pub fn certificate(cov: &ConeCoverage, disp: &Dispatcher, frame: &NodeFrame) -> Value {
    let src = DispatcherSource::new(disp);
    json!({
        "schema": super::CONE_COVERAGE_SCHEMA,
        "version": super::CONE_COVERAGE_VERSION,
        "tables_digest": crate::economy::table_set_digest(&src, frame),
        "frame": frame.mask_string(),
        "cone": cov.cone,
        "verdict": cov.verdict,
        "elements": cov.elements,
        "listed": cov.listed,
        "probes": cov.probes,
        "partition": cov.partition,
        "partition_flags": cov.partition_flags,
    })
}
