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
    PartitionOptions, PartitionReport, Progress, WorkMeter, gap_partition_cells,
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

    /// The box of a node as a cone: dots `sum_lines (k - 1) <= d_max`, rank `sum_numerators (-k) <= r_max`, every line
    /// `<= line_max`.
    pub fn dots_rank_box(frame: &NodeFrame, d_max: i64, r_max: i64, line_max: i64) -> Cone {
        let n = frame.arity();
        let lines = (0..n).filter(|&i| frame.is_line(i)).count() as i64;
        let dots: Vec<i64> = (0..n).map(|i| i64::from(frame.is_line(i))).collect();
        let rank: Vec<i64> = (0..n).map(|i| -i64::from(!frame.is_line(i))).collect();
        let mut c = Cone {
            label: format!("D <= {d_max}, r <= {r_max}, lines <= {line_max}"),
            constraints: vec![(dots, d_max.saturating_add(lines)), (rank, r_max)],
        };
        for i in 0..n {
            if frame.is_line(i) {
                let mut a = vec![0i64; n];
                a[i] = 1;
                c.constraints.push((a, line_max));
            }
        }
        c
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
    /// The partition's EXACT product split of exception branches (`PartitionOptions::factor_exceptions`: AND(f g, h) =
    /// AND(f, h) OR AND(g, h), linear factors first); default off (the minting options). For dispatchers whose
    /// exception branches are products of linear factors (e.g. pole loci moved by [`fall_through_poles`]).
    pub factor_exceptions: bool,
    /// Run the partition in Describe mode (with the face list) instead of Cells mode (diagnostics).
    pub describe: bool,
    /// A blocked sub-report with more classes than this is NOT listed cell by cell: it becomes ONE `unexplored`
    /// element (the whole chart orthant, blocked by its source), so every cone meeting it is undecided there (never
    /// covered); flagged. `None` = list everything. (T162: a superset stop piece of its generic first record gives a
    /// 66.6 M-class sub-report.)
    pub max_blocked_classes: Option<usize>,
    /// Engine model: the engine's zero test is exact (rustred slot-alias 4895ee68): no modular-zero-risk stop pieces.
    pub engine_exact_zero: bool,
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
            factor_exceptions: false,
            describe: false,
            max_blocked_classes: None,
            engine_exact_zero: false,
        }
    }
}

/// Number of keys of a CELL (per-slot intervals, every slot bounded inside the cone's reach) inside the cone, by
/// enumeration over the cell when it has at most `cap` points; `None` = unbounded or larger than `cap`. A U cell's
/// uncovered keys are the cell minus covered sub-loci: an UPPER bound of its uncovered keys.
pub fn cell_keys_in_cone(region: &Region, cone: &Cone, frame: &NodeFrame, cap: u64) -> Option<u64> {
    let mut n = 0u64;
    cell_keys_each(region, cone, frame, cap, &mut |_| n += 1)?;
    Some(n)
}

/// Every key of a cell inside the cone (see [`cell_keys_in_cone`]); `None` = unbounded or more than `cap` cell points.
pub fn cell_keys_each(
    region: &Region,
    cone: &Cone,
    frame: &NodeFrame,
    cap: u64,
    f: &mut dyn FnMut(&[i32]),
) -> Option<()> {
    let n = frame.arity();
    // the cell's intervals from its constraints (identity parameter map), clipped by the cone's one-slot reach
    let mut lo = vec![i64::MIN; n];
    let mut hi = vec![i64::MAX; n];
    for c in &region.cons {
        let nz: Vec<usize> = (0..c.coef.len()).filter(|&j| c.coef[j] != 0).collect();
        if nz.len() != 1 {
            return None;
        }
        let j = nz[0];
        let v = i64::try_from(c.c).ok()?;
        match c.coef[j] {
            1 => lo[j] = lo[j].max(v.checked_neg()?),
            -1 => hi[j] = hi[j].min(v),
            _ => return None,
        }
    }
    let reg = with_cone(region, cone)?;
    for j in 0..n {
        let (l, h) = reg.range_of(j).ok()?;
        if let Some(l) = l {
            lo[j] = lo[j].max(i64::try_from(l).ok()?);
        }
        if let Some(h) = h {
            hi[j] = hi[j].min(i64::try_from(h).ok()?);
        }
        if lo[j] == i64::MIN || hi[j] == i64::MAX {
            return None;
        }
        if lo[j] > hi[j] {
            return Some(());
        }
    }
    let mut size: u64 = 1;
    for j in 0..n {
        size = size.checked_mul(u64::try_from(hi[j] - lo[j] + 1).ok()?)?;
        if size > cap {
            return None;
        }
    }
    let mut k: Vec<i64> = lo.clone();
    loop {
        let key: Option<Vec<i32>> = k.iter().map(|v| i32::try_from(*v).ok()).collect();
        if let Some(key) = key
            && cone.contains(&key)
            && region
                .contains_t(&k.iter().map(|v| i128::from(*v)).collect::<Vec<_>>())
                .unwrap_or(false)
        {
            f(&key);
        }
        let mut s = 0;
        loop {
            if s == n {
                return Some(());
            }
            if k[s] < hi[s] {
                k[s] += 1;
                break;
            }
            k[s] = lo[s];
            s += 1;
        }
    }
}

/// A dispatcher with leading CAP records (B2 scaling, sound): one record per one-slot cap `a k_s <= b` of `caps`, whose
/// domain is the cap's VIOLATION (`a k_s - b - 1 >= 0`), placed BEFORE every table of `disp`. A key outside the caps is
/// served by a cap record (no exception, no pole: it applies wherever it is selected); a key inside every cap selects no
/// cap record and is dispatched exactly as by `disp`. So the dispatch-uncovered set of the result is `disp`'s
/// uncovered set intersected with the caps, and the partition only has to describe a bounded region. The cone used
/// with it must lie inside the caps: [`cone_inside_caps`].
pub fn capped_dispatcher(
    disp: &Dispatcher,
    frame: &NodeFrame,
    caps: &Cone,
) -> Result<Dispatcher, String> {
    use crate::cover::Table;
    use crate::domain::CaseDomain;
    use crate::poly::Poly;
    let n = frame.arity();
    let mut t = Table::new("cone-caps", n);
    for (i, (a, b)) in caps.constraints.iter().enumerate() {
        let nz: Vec<usize> = (0..a.len()).filter(|&j| a[j] != 0).collect();
        if a.len() != n || nz.len() != 1 {
            return Err(format!("cap {i}: only one-slot caps are allowed"));
        }
        let s = nz[0];
        // a k_s - b - 1 >= 0
        let p = Poly::var(s)
            .mul(&Poly::constant(i128::from(a[s])))
            .and_then(|x| x.sub(&Poly::constant(i128::from(*b) + 1)))
            .map_err(|e| format!("{e:?}"))?;
        let mut d = CaseDomain::free(n, Some(frame.mask().to_vec()));
        d.bounds.push(p);
        t.push(i as i64, d).map_err(|e| e.to_string())?;
    }
    let mut tables = vec![t];
    for &ti in disp.priority() {
        tables.push(disp.tables()[ti].clone());
    }
    let m = tables.len();
    Dispatcher::new(tables, (0..m).collect())
        .and_then(|d| d.with_chart(frame.mask().to_vec()))
        .map_err(|e| e.to_string())
}

/// The `2^k` FACE PIECES of the chart orthant split on `slots` (coordinator decision "face by face": one partition per
/// sector piece, each run on [`capped_dispatcher`] with the piece as its caps). Per split slot the piece takes the
/// boundary value (line slot `= 1`, numerator slot `= 0`) or the rest of the chart (line `>= 2`, numerator `<= -1`);
/// all constraints are one-slot. The pieces are pairwise disjoint and their union is the chart orthant, so a cone is
/// covered iff `cone and piece` is covered for every piece, and the uncovered sets add up. Order: binary counting, the
/// first slot of `slots` most significant; bit 0 = boundary. Repeated slots are an error (`None`).
pub fn face_pieces(frame: &NodeFrame, slots: &[usize]) -> Option<Vec<Cone>> {
    let k = slots.len();
    let mut out = Vec::with_capacity(1usize.checked_shl(u32::try_from(k).ok()?)?);
    for code in 0..(1usize << k) {
        let choice: Vec<(usize, bool)> = slots
            .iter()
            .enumerate()
            .map(|(j, &s)| (s, (code >> (k - 1 - j)) & 1 == 1))
            .collect();
        out.push(face_piece(frame, &choice)?);
    }
    Some(out)
}

/// One face piece: per `(slot, beyond)` the boundary value (`beyond = false`: line `= 1`, numerator `= 0`) or the rest
/// of the chart (line `>= 2`, numerator `<= -1`); see [`face_pieces`]. `None` for a slot outside the arity or repeated.
pub fn face_piece(frame: &NodeFrame, choice: &[(usize, bool)]) -> Option<Cone> {
    let n = frame.arity();
    let mut seen = vec![false; n];
    let mut c = Cone {
        label: String::new(),
        constraints: vec![],
    };
    let mut parts = Vec::new();
    for &(s, beyond) in choice {
        if s >= n || seen[s] {
            return None;
        }
        seen[s] = true;
        let mut up = vec![0i64; n];
        up[s] = 1;
        let mut down = vec![0i64; n];
        down[s] = -1;
        let (cons, text) = match (frame.is_line(s), beyond) {
            // n_s = 1: n_s <= 1 (>= 1 by the chart)
            (true, false) => ((up, 1), format!("n{s}=1")),
            // n_s >= 2
            (true, true) => ((down, -2), format!("n{s}>=2")),
            // n_s = 0: -n_s <= 0 (<= 0 by the chart)
            (false, false) => ((down, 0), format!("n{s}=0")),
            // n_s <= -1
            (false, true) => ((up, -1), format!("n{s}<=-1")),
        };
        c.constraints.push(cons);
        parts.push(text);
    }
    c.label = if parts.is_empty() {
        "the whole chart orthant (no split)".into()
    } else {
        format!("piece {}", parts.join(", "))
    };
    Some(c)
}

/// Is every key of `cone` (in the chart orthant) inside every one-slot cap of `caps`? Proved by the per-slot ranges of
/// the cone's region (rational Fourier-Motzkin projection with rounding: a SUPERSET of the true range, so `true` is a
/// proof); `false` = not proved.
pub fn cone_inside_caps(cone: &Cone, caps: &Cone, frame: &NodeFrame) -> bool {
    let n = frame.arity();
    let lo: Vec<Option<i64>> = (0..n).map(|i| frame.is_line(i).then_some(1)).collect();
    let hi: Vec<Option<i64>> = (0..n).map(|i| (!frame.is_line(i)).then_some(0)).collect();
    let Some(reg) = with_cone(&identity_region(n, &lo, &hi), cone) else {
        return false;
    };
    caps.constraints.iter().all(|(a, b)| {
        let nz: Vec<usize> = (0..a.len()).filter(|&j| a[j] != 0).collect();
        if nz.len() != 1 {
            return false;
        }
        let s = nz[0];
        let Ok((l, h)) = reg.range_of(s) else {
            return false;
        };
        let b = i128::from(*b);
        match a[s].signum() {
            1 => h.is_some_and(|h| i128::from(a[s]) * h <= b),
            _ => l.is_some_and(|l| i128::from(a[s]) * l <= b),
        }
    })
}

/// One gap element of the partition.
#[derive(Clone, Debug)]
pub struct GapElement {
    /// `generic` (uncovered class), `locus` (resolved fragile piece), `unresolved`, `unexplored`.
    pub kind: &'static str,
    /// The blocking record (dispatch position) of a blocked sub-partition, if the element is one.
    pub blocked_by: Option<usize>,
    /// A CELL (uncovered / unexplored class): its per-slot intervals `(lo, hi)` (chart bounds included); the region
    /// (identity parameter map) is built on demand by [`GapElement::region`]. Compact: a real partition lists
    /// hundreds of thousands of cells (T162: 770,709), whose full regions do not fit in memory.
    pub cell: Option<Vec<(Option<i64>, Option<i64>)>>,
    /// The region of a non-cell element (resolved locus piece, unresolved part): a lattice region.
    pub lattice: Option<Region>,
    /// Uncovered keys the partition found in it (oracle witnesses, never a decision).
    pub seeds: Vec<Vec<i32>>,
    /// Text for the report (cut trail, reason; empty for a cell: its intervals are the text).
    pub note: String,
    /// For an element of a blocked sub-partition: which stop piece of its blocker it lies in (`selected` = selected
    /// minus applies: loci, recenter / range bounds, elision, witness status; `overflow:n<s>` = the rule point leaves
    /// the i32 range; `modular:<why>` = the engine's modular zero test may read a nonzero value as zero).
    pub stop: Option<String>,
}

impl GapElement {
    /// The element's region (a cell's is built from its intervals).
    pub fn region(&self) -> std::borrow::Cow<'_, Region> {
        match (&self.lattice, &self.cell) {
            (Some(r), _) => std::borrow::Cow::Borrowed(r),
            (None, Some(c)) => {
                let lo: Vec<Option<i64>> = c.iter().map(|x| x.0).collect();
                let hi: Vec<Option<i64>> = c.iter().map(|x| x.1).collect();
                std::borrow::Cow::Owned(identity_region(c.len(), &lo, &hi))
            }
            (None, None) => unreachable!("a gap element is a cell or a lattice region"),
        }
    }
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
    /// The element meets the cone in a BOUNDED set of `points` integer keys (parameter box `t_box`), every one of which
    /// the dispatcher's first match serves: no uncovered key of the element lies in the cone (exhaustive, exact).
    EnumeratedCovered {
        points: u64,
        t_box: Vec<(i64, i64)>,
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
fn class_box(
    rep: &PartitionReport,
    frame: &NodeFrame,
    states: &[Option<u16>],
) -> Vec<(Option<i64>, Option<i64>)> {
    let n = frame.arity();
    let mut out = Vec::with_capacity(n);
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
        let lo = match (l, cl) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        let hi = match (h, ch) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        out.push((lo, hi));
    }
    out
}

/// The stop piece of blocker `src` that the blocked sub-report `sub` of `rep` describes, as a label
/// ([`GapElement::stop`]).
fn stop_label(rep: &PartitionReport, src: usize, sub: &PartitionReport) -> String {
    use crate::partition::nf::{StopRef, StopWhy};
    match sub.blocked_stop() {
        StopRef::Selected => "selected".to_string(),
        StopRef::Extra(j) => match rep.nf().extra_stops.get(&src).and_then(|v| v.get(j)) {
            Some(p) => match &p.why {
                StopWhy::RulePointOverflow { slot } => format!("overflow:n{slot}"),
                StopWhy::ModularZeroRisk(w) => format!("modular:{w}"),
            },
            None => format!("extra:{j}"),
        },
    }
}

/// [`gap_elements`] with a bound on the classes of a blocked sub-report (`ConeOptions::max_blocked_classes`): a larger
/// one becomes one `unexplored` element covering the chart orthant (sound: its cones are undecided there), with a flag.
pub fn gap_elements_bounded(
    rep: &PartitionReport,
    frame: &NodeFrame,
    blocked_by: Option<usize>,
    max_blocked_classes: Option<usize>,
    out: &mut Vec<GapElement>,
    flags: &mut Vec<String>,
) {
    gap_elements_rec(
        rep,
        frame,
        blocked_by,
        None,
        max_blocked_classes,
        out,
        flags,
    );
}

fn gap_elements_rec(
    rep: &PartitionReport,
    frame: &NodeFrame,
    blocked_by: Option<usize>,
    stop: Option<String>,
    max_blocked_classes: Option<usize>,
    out: &mut Vec<GapElement>,
    flags: &mut Vec<String>,
) {
    gap_elements_own(rep, frame, blocked_by, stop, out);
    for (src, sub) in rep.blocked() {
        let label = stop_label(rep, *src, sub);
        if let Some(max) = max_blocked_classes
            && sub.classes().len() > max
        {
            let n = frame.arity();
            let cell: Vec<(Option<i64>, Option<i64>)> = (0..n)
                .map(|s| {
                    if frame.is_line(s) {
                        (Some(1), None)
                    } else {
                        (None, Some(0))
                    }
                })
                .collect();
            flags.push(format!(
                "blocked sub-report of source {src} ({} classes > max_blocked_classes {max}) not listed: one unexplored element (the chart orthant); every cone meeting it is undecided there",
                sub.classes().len()
            ));
            out.push(GapElement {
                kind: "unexplored",
                blocked_by: Some(*src),
                cell: Some(cell),
                lattice: None,
                seeds: vec![],
                note: format!(
                    "blocked sub-report of source {src}: {} classes not listed",
                    sub.classes().len()
                ),
                stop: Some(label),
            });
        } else {
            gap_elements_rec(
                sub,
                frame,
                Some(*src),
                Some(label),
                max_blocked_classes,
                out,
                flags,
            );
        }
    }
}

/// Every gap element of `rep` and of its blocked sub-partitions.
pub fn gap_elements(
    rep: &PartitionReport,
    frame: &NodeFrame,
    blocked_by: Option<usize>,
    out: &mut Vec<GapElement>,
) {
    let mut flags = Vec::new();
    gap_elements_rec(rep, frame, blocked_by, None, None, out, &mut flags);
}

/// The gap elements of `rep` itself (classes, locus pieces, unresolved parts), not of its blocked sub-reports.
fn gap_elements_own(
    rep: &PartitionReport,
    frame: &NodeFrame,
    blocked_by: Option<usize>,
    stop: Option<String>,
    out: &mut Vec<GapElement>,
) {
    for c in rep.classes() {
        let kind = match c.label {
            Label::Uncovered => "generic",
            Label::Unexplored => "unexplored",
            Label::Fragile => continue,
        };
        out.push(GapElement {
            kind,
            blocked_by,
            cell: Some(class_box(rep, frame, &c.states)),
            lattice: None,
            seeds: vec![],
            note: String::new(),
            stop: stop.clone(),
        });
    }
    if let Some(res) = rep.resolution() {
        for l in &res.loci {
            for p in &l.pieces {
                out.push(GapElement {
                    kind: "locus",
                    blocked_by,
                    cell: None,
                    lattice: Some(p.region.clone()),
                    seeds: p.witness.clone(),
                    stop: stop.clone(),
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
                    cell: None,
                    lattice: Some(u.region.clone()),
                    seeds: u.points.clone(),
                    stop: stop.clone(),
                    note: format!("locus {}: {} ({})", l.id, u.reason, u.equation),
                });
            }
        }
    }
}

/// The per-slot intervals `[[lo | null, hi | null], ..]` of a CELL region (identity parameter map, every constraint on
/// one slot with coefficient +-1); `None` for any other region.
pub fn cell_box(r: &Region) -> Option<Value> {
    let n = r.x.len();
    if r.k != n
        || !r.x.iter().enumerate().all(|(s, a)| {
            a.c == 0
                && a.coef
                    .iter()
                    .enumerate()
                    .all(|(j, v)| *v == i128::from(j == s))
        })
    {
        return None;
    }
    let mut lo: Vec<Option<i64>> = vec![None; n];
    let mut hi: Vec<Option<i64>> = vec![None; n];
    for c in &r.cons {
        let nz: Vec<usize> = (0..c.coef.len()).filter(|&j| c.coef[j] != 0).collect();
        if nz.len() != 1 {
            return None;
        }
        let j = nz[0];
        let v = i64::try_from(c.c).ok()?;
        match c.coef[j] {
            1 => {
                lo[j] =
                    Some(lo[j].map_or(v.checked_neg()?, |l| l.max(v.checked_neg().unwrap_or(l))))
            }
            -1 => hi[j] = Some(hi[j].map_or(v, |h| h.min(v))),
            _ => return None,
        }
    }
    Some(json!(
        lo.iter()
            .zip(&hi)
            .map(|(l, h)| json!([l, h]))
            .collect::<Vec<_>>()
    ))
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
    let Some(reg) = with_cone(&el.region(), cone) else {
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
    // a BOUNDED intersection with at most `opts.enumeration` parameter points: decided by enumerating every point with the
    // dispatcher (exact on a finite set)
    if let Some(v) = enumerate_element(disp, frame, &reg, cone, opts.enumeration) {
        return v;
    }
    ElementVerdict::Undecided {
        reason: format!(
            "the element meets the cone but the {tried} point(s) tried there are covered (the element is a superset of its uncovered keys)"
        ),
    }
}

/// Enumerate a bounded region (its parameters' integer ranges from Fourier-Motzkin projection, a superset box) with at most
/// `cap` box points: the first OPEN key in the chart orthant and the cone is a witness; none = `EnumeratedCovered`.
/// `None` when a range is unbounded, the box is larger than `cap`, or arithmetic overflows.
pub fn enumerate_element(
    disp: &Dispatcher,
    frame: &NodeFrame,
    reg: &Region,
    cone: &Cone,
    cap: u64,
) -> Option<ElementVerdict> {
    let mut lo = Vec::with_capacity(reg.k);
    let mut hi = Vec::with_capacity(reg.k);
    let mut size: u64 = 1;
    for j in 0..reg.k {
        let (l, h) = reg.range_of(j).ok()?;
        let (l, h) = (i64::try_from(l?).ok()?, i64::try_from(h?).ok()?);
        if l > h {
            return Some(ElementVerdict::OutsideCone);
        }
        size = size.checked_mul(u64::try_from(h.checked_sub(l)?.checked_add(1)?).ok()?)?;
        if size > cap {
            return None;
        }
        lo.push(l);
        hi.push(h);
    }
    let in_orthant = |k: &[i32]| {
        k.iter()
            .enumerate()
            .all(|(i, &v)| if frame.is_line(i) { v >= 1 } else { v <= 0 })
    };
    let mut t: Vec<i64> = lo.clone();
    let mut points = 0u64;
    loop {
        let tt: Vec<i128> = t.iter().map(|v| i128::from(*v)).collect();
        if reg.contains_t(&tt).ok()? {
            let k = key_of(reg, &tt)?;
            if in_orthant(&k) && cone.contains(&k) {
                points += 1;
                if is_open(disp, &k) {
                    return Some(ElementVerdict::Uncovered { witness: k });
                }
            }
        }
        let mut j = 0;
        loop {
            if j == reg.k {
                return Some(ElementVerdict::EnumeratedCovered {
                    points,
                    t_box: lo.iter().copied().zip(hi.iter().copied()).collect(),
                });
            }
            if t[j] < hi[j] {
                t[j] += 1;
                break;
            }
            t[j] = lo[j];
            j += 1;
        }
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
        engine_exact_zero: false,
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
    partition_for_cones_with(disp, frame, opts, None)
}

/// [`partition_for_cones`] with the partition's progress callback (its main recursion calls it every 2^18 nodes).
pub fn partition_for_cones_with(
    disp: &Dispatcher,
    frame: &NodeFrame,
    opts: &ConeOptions,
    progress: Option<&dyn Fn(&Progress)>,
) -> Result<ConePartition, String> {
    partition_for_cones_traced(disp, frame, opts, progress, &|_| {})
}

/// [`partition_for_cones_with`] with a trace callback (one line after the partition, one after the gap elements).
pub fn partition_for_cones_traced(
    disp: &Dispatcher,
    frame: &NodeFrame,
    opts: &ConeOptions,
    progress: Option<&dyn Fn(&Progress)>,
    trace: &dyn Fn(&str),
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
    let mut po = partition_options(&opts.budget);
    po.factor_exceptions = opts.factor_exceptions;
    po.engine_exact_zero = opts.engine_exact_zero;
    let report = if opts.describe {
        crate::partition::gap_partition(&src, frame, &po, progress)
    } else {
        gap_partition_cells(&src, frame, &po, progress)
    }
    .map_err(|e| format!("partition: {e:?}"))?;
    // sizes: (classes, locus pieces, unresolved parts) of the report and of every blocked sub-report
    fn sizes(r: &PartitionReport) -> (usize, usize, usize) {
        let (p, u) = r.resolution().map_or((0, 0), |res| {
            (
                res.loci.iter().map(|l| l.pieces.len()).sum(),
                res.loci.iter().map(|l| l.unresolved_parts.len()).sum(),
            )
        });
        (r.classes().len(), p, u)
    }
    let blocked: Vec<(usize, (usize, usize, usize))> = report
        .blocked()
        .iter()
        .map(|(e, b)| (*e, sizes(b)))
        .collect();
    trace(&format!(
        "partition report in {:.1} s: (classes, locus pieces, unresolved) main {:?}, blocked [source, sizes] {:?}",
        t0.elapsed().as_secs_f64(),
        sizes(&report),
        blocked
    ));
    {
        let mut w: Vec<(usize, u64, usize, bool)> = report.blocked_work().to_vec();
        let total: u64 = w.iter().map(|x| x.1).sum();
        w.sort_by_key(|x| std::cmp::Reverse(x.1));
        w.truncate(25);
        trace(&format!(
            "blocked sub-partitions: {} computed, {} units; skipped {:?}; largest (source, units, classes, complete) {:?}",
            report.blocked_work().len(),
            total,
            report.blocked_skipped(),
            w
        ));
    }
    let mut flags = Vec::new();
    if let Err(e) = report.complete_run() {
        flags.push(format!("not a complete run: {e}"));
    }
    if !report.caveats().is_empty() {
        flags.push(format!("caveats: {}", report.caveats().join("; ")));
    }
    let mut elements = Vec::new();
    gap_elements_bounded(
        &report,
        frame,
        None,
        opts.max_blocked_classes,
        &mut elements,
        &mut flags,
    );
    trace(&format!(
        "gap elements in {:.1} s: {}",
        t0.elapsed().as_secs_f64(),
        elements.len()
    ));
    Ok(ConePartition {
        report,
        elements,
        flags,
        millis: u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// Membership with a fast path for cells (identity parameter map: the constraints are evaluated at the key).
fn element_contains(el: &GapElement, key: &[i32]) -> Option<bool> {
    if let Some(c) = &el.cell {
        return Some(
            c.len() == key.len()
                && c.iter().zip(key).all(|((lo, hi), &v)| {
                    lo.is_none_or(|l| i64::from(v) >= l) && hi.is_none_or(|h| i64::from(v) <= h)
                }),
        );
    }
    let r = el.region();
    let r = r.as_ref();
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
            ElementVerdict::OutsideCone | ElementVerdict::EnumeratedCovered { .. } => outside += 1,
            ElementVerdict::Uncovered { .. } => unc += 1,
            ElementVerdict::Undecided { .. } => und += 1,
        }
        if v != ElementVerdict::OutsideCone {
            // a CELL (identity parameter map, one-slot constraints) is listed compactly as its per-slot intervals
            let entry = match &el.cell {
                Some(c) => json!({
                    "element": i, "kind": el.kind, "blocked_by": el.blocked_by,
                    "box": c.iter().map(|(l, h)| json!([l, h])).collect::<Vec<_>>(), "v": v,
                }),
                None => {
                    let r = el.region();
                    json!({
                        "element": i, "kind": el.kind, "blocked_by": el.blocked_by, "note": el.note,
                        "x": r.render_x(), "constraints": r.render_cons(), "region": region_json(&r), "v": v,
                    })
                }
            };
            listed.push(entry);
        }
        verdicts.push(v);
    }
    if els.iter().zip(&verdicts).any(|(e, v)| {
        e.kind == "unexplored"
            && matches!(
                v,
                ElementVerdict::Uncovered { .. } | ElementVerdict::Undecided { .. }
            )
    }) {
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
                    if matches!(
                        v,
                        ElementVerdict::OutsideCone | ElementVerdict::EnumeratedCovered { .. }
                    ) {
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

/// The dispatcher under the coordinator's BLOCK semantics for witness loci (decision G5, COORDINATOR-ANSWERS-20261116-1.md:
/// "the record does not match on its witness locus; the key falls through"), as the engine lane will implement it (a locus
/// becomes an exclusion branch, like `domain_bounds`): every POLE LOCUS of every domain (witness loci, written
/// denominators, source conditions; each the AND of its d-coefficients vanishing) is moved into the exception branches,
/// so a key on it is NOT SELECTED by that record and reaches the next one; the core's own dispatcher would stop there
/// (Blocked, the key open). Records whose witness loci are ELIDED (unknown locus) keep blocking (`witness_elided`, record
/// metadata). A locus whose polynomial is identically zero becomes a branch that always holds (the record never matches).
pub fn fall_through_poles(disp: &Dispatcher) -> Result<Dispatcher, String> {
    use crate::poly::Poly;
    let mut tables = Vec::with_capacity(disp.tables().len());
    let mut moved = 0usize;
    for t in disp.tables() {
        let mut t2 = t.clone();
        for d in &mut t2.domains {
            for p in std::mem::take(&mut d.poles) {
                moved += 1;
                d.exceptions.push(if p.coeffs.is_empty() {
                    vec![Poly::constant(0)]
                } else {
                    p.coeffs
                });
            }
        }
        tables.push(t2);
    }
    let _ = moved;
    let d = Dispatcher::new(tables, disp.priority().to_vec()).map_err(|e| e.to_string())?;
    match disp.chart() {
        Some(c) => d.with_chart(c.to_vec()).map_err(|e| e.to_string()),
        None => Ok(d),
    }
}

/// A moved undecided locus of [`fall_through_poles_reduced`]: (table, record label, the locus's d-coefficients, the
/// record's exception branches).
pub type UndecidedLocus = (String, i64, Vec<String>, Vec<Vec<String>>);

/// Counts of [`fall_through_poles_reduced`].
#[derive(Clone, Debug, Default, Serialize)]
pub struct FallThroughStats {
    /// Pole loci seen (every domain of every table).
    pub poles: usize,
    /// Dropped: the record's selected set (poles removed) meets the locus nowhere (exact emptiness proof), so the locus
    /// never decides a key (the engine wrote its vanishing set into the exceptions already).
    pub redundant: usize,
    /// Of `redundant`: proved by an exception branch of the record whose every member is a scalar multiple of one of
    /// the locus's d-coefficients (`Poly::primitive_normal` equal), so the locus lies inside that branch.
    pub redundant_by_branch: usize,
    /// Moved into the exceptions: a verified key of the selected set lies on the locus.
    pub moved_nonempty: usize,
    /// Moved into the exceptions: emptiness not decided within the budget.
    pub moved_unknown: usize,
    /// Moved: identically zero polynomial (the record never matches).
    pub moved_everywhere: usize,
    /// Up to 20 examples of moved non-empty loci: (table, record label, witness key).
    pub examples: Vec<(String, i64, Vec<i32>)>,
    /// Up to 100 moved undecided loci.
    pub undecided: Vec<UndecidedLocus>,
}

/// [`fall_through_poles`] with the redundant loci DROPPED instead of moved (same dispatch, cheaper partition). For
/// every pole locus `L` (AND of its d-coefficients) of a domain with selected set `S` (the domain with its poles
/// removed: fixed slots, equations, bounds, exception branches), `S n L` is decided EXACTLY
/// ([`crate::parametric::descent::emptiness`]: sign splits, Fourier-Motzkin, verified witnesses; a superset without
/// nonlinear exception branches when undecided). Empty = no key of `S` lies on `L`, so moving `L` into the exceptions
/// would not change the dispatch: it is dropped. Non-empty or undecided = moved, as [`fall_through_poles`] does. The
/// engine writes the vanishing sets of its witness poles into `exceptions` (`DomainOptions::witness` doc), so most
/// loci are redundant; a moved non-empty locus is a record whose exceptions miss part of its witness locus.
pub fn fall_through_poles_reduced(
    disp: &Dispatcher,
    frame: &NodeFrame,
    budget: &crate::loci::contain::ProofBudget,
) -> Result<(Dispatcher, FallThroughStats), String> {
    use crate::parametric::descent::{EmptyState, emptiness};
    use crate::poly::Poly;
    let mut st = FallThroughStats::default();
    let mut tables = Vec::with_capacity(disp.tables().len());
    for t in disp.tables() {
        let mut t2 = t.clone();
        for (r, d) in t2.domains.iter_mut().enumerate() {
            if d.poles.is_empty() {
                continue;
            }
            let poles = std::mem::take(&mut d.poles);
            let base = d.clone();
            for p in poles {
                st.poles += 1;
                if p.coeffs.is_empty() {
                    st.moved_everywhere += 1;
                    d.exceptions.push(vec![Poly::constant(0)]);
                    continue;
                }
                // the engine writes the locus as an exception branch (AND of the same coefficients up to scalars)
                let normal: Vec<Poly> = p.coeffs.iter().map(Poly::primitive_normal).collect();
                let inside_branch = base.exceptions.iter().any(|b| {
                    !b.is_empty() && b.iter().all(|m| normal.contains(&m.primitive_normal()))
                });
                if inside_branch {
                    st.redundant += 1;
                    st.redundant_by_branch += 1;
                    continue;
                }
                match emptiness(&base, frame, budget, &p.coeffs, &[]) {
                    (EmptyState::Empty, _, _) => st.redundant += 1,
                    (EmptyState::NonEmpty, w, _) => {
                        st.moved_nonempty += 1;
                        if st.examples.len() < 20 {
                            st.examples.push((
                                t.name.clone(),
                                t.labels.get(r).copied().unwrap_or(-1),
                                w.unwrap_or_default(),
                            ));
                        }
                        d.exceptions.push(p.coeffs);
                    }
                    (EmptyState::Unknown, _, _) => {
                        st.moved_unknown += 1;
                        if st.undecided.len() < 100 {
                            st.undecided.push((
                                t.name.clone(),
                                t.labels.get(r).copied().unwrap_or(-1),
                                p.coeffs.iter().map(|c| c.to_string()).collect(),
                                base.exceptions
                                    .iter()
                                    .map(|b| b.iter().map(|c| c.to_string()).collect())
                                    .collect(),
                            ));
                        }
                        d.exceptions.push(p.coeffs);
                    }
                }
            }
        }
        tables.push(t2);
    }
    let d = Dispatcher::new(tables, disp.priority().to_vec()).map_err(|e| e.to_string())?;
    let d = match disp.chart() {
        Some(c) => d.with_chart(c.to_vec()).map_err(|e| e.to_string())?,
        None => d,
    };
    Ok((d, st))
}

/// Every key of a LATTICE region (`x = x0 + M t`, constraints on `t`) inside `cone`, when the parameter box of
/// `region and cone` (per-parameter ranges, rational projection with rounding: a superset) has at most `cap` points;
/// `None` = unbounded or too large. Each parameter point satisfying the constraints gives its key (`x(t)`); keys outside
/// `i32` are skipped. The companion of [`cell_keys_each`] for locus pieces / unresolved parts.
pub fn region_keys_each(
    region: &Region,
    cone: &Cone,
    cap: u64,
    f: &mut dyn FnMut(&[i32]),
) -> Option<()> {
    let reg = with_cone(region, cone)?;
    let k = reg.k;
    let mut lo = vec![0i128; k];
    let mut hi = vec![0i128; k];
    for j in 0..k {
        let (l, h) = reg.range_of(j).ok()?;
        lo[j] = l?;
        hi[j] = h?;
        if lo[j] > hi[j] {
            return Some(());
        }
    }
    let mut size: u64 = 1;
    for j in 0..k {
        size = size.checked_mul(u64::try_from(hi[j] - lo[j] + 1).ok()?)?;
        if size > cap {
            return None;
        }
    }
    let mut t = lo.clone();
    loop {
        if reg.contains_t(&t).unwrap_or(false) {
            let key: Option<Vec<i32>> = reg
                .x
                .iter()
                .map(|a| {
                    let v = a
                        .coef
                        .iter()
                        .zip(&t)
                        .try_fold(a.c, |acc, (c, x)| acc.checked_add(c.checked_mul(*x)?))?;
                    i32::try_from(v).ok()
                })
                .collect();
            if let Some(key) = key
                && cone.contains(&key)
            {
                f(&key);
            }
        }
        let mut j = 0;
        loop {
            if j == k {
                return Some(());
            }
            if t[j] < hi[j] {
                t[j] += 1;
                break;
            }
            t[j] = lo[j];
            j += 1;
        }
    }
}

/// Per-slot box `(lo, hi)` of a region (`None` = unbounded): exact for a cell; for a lattice region the interval image of
/// its parameter ranges under `x = x0 + M t` (a SUPERSET of the slot range). `None` on overflow / unreadable ranges.
pub fn slot_box(el: &GapElement) -> Option<Vec<(Option<i64>, Option<i64>)>> {
    if let Some(c) = &el.cell {
        return Some(c.clone());
    }
    let r = el.lattice.as_ref()?;
    let mut pr = Vec::with_capacity(r.k);
    for j in 0..r.k {
        pr.push(r.range_of(j).ok()?);
    }
    let mut out = Vec::with_capacity(r.x.len());
    for a in &r.x {
        let (mut lo, mut hi): (Option<i128>, Option<i128>) = (Some(a.c), Some(a.c));
        for (j, &c) in a.coef.iter().enumerate() {
            if c == 0 {
                continue;
            }
            let (pl, ph) = pr[j];
            let (cl, ch) = if c > 0 {
                (
                    pl.and_then(|v| v.checked_mul(c)),
                    ph.and_then(|v| v.checked_mul(c)),
                )
            } else {
                (
                    ph.and_then(|v| v.checked_mul(c)),
                    pl.and_then(|v| v.checked_mul(c)),
                )
            };
            lo = match (lo, cl) {
                (Some(x), Some(y)) => x.checked_add(y),
                _ => None,
            };
            hi = match (hi, ch) {
                (Some(x), Some(y)) => x.checked_add(y),
                _ => None,
            };
        }
        out.push((
            lo.and_then(|v| i64::try_from(v).ok()),
            hi.and_then(|v| i64::try_from(v).ok()),
        ));
    }
    Some(out)
}

/// Interval range of a polynomial over a box (`None` end = unbounded / not determined): exact for linear polynomials,
/// `(None, None)` for nonlinear ones (no claim).
fn poly_range(
    p: &crate::poly::Poly,
    bx: &[(Option<i64>, Option<i64>)],
) -> (Option<i128>, Option<i128>) {
    if p.degree() > 1 || p.is_big() {
        return (None, None);
    }
    let (mut lo, mut hi): (Option<i128>, Option<i128>) = (Some(0), Some(0));
    for (m, c) in p.terms() {
        if m.is_empty() {
            lo = lo.and_then(|x| x.checked_add(*c));
            hi = hi.and_then(|x| x.checked_add(*c));
            continue;
        }
        let s = m[0].0 as usize;
        let (bl, bh) = bx.get(s).copied().unwrap_or((None, None));
        let (bl, bh) = (bl.map(i128::from), bh.map(i128::from));
        let (tl, th) = if *c > 0 {
            (
                bl.and_then(|v| v.checked_mul(*c)),
                bh.and_then(|v| v.checked_mul(*c)),
            )
        } else {
            (
                bh.and_then(|v| v.checked_mul(*c)),
                bl.and_then(|v| v.checked_mul(*c)),
            )
        };
        lo = match (lo, tl) {
            (Some(x), Some(y)) => x.checked_add(y),
            _ => None,
        };
        hi = match (hi, th) {
            (Some(x), Some(y)) => x.checked_add(y),
            _ => None,
        };
    }
    (lo, hi)
}

/// Does the record domain `d` APPLY at EVERY key of the box `bx` (per-slot intervals, inside the chart)? An exact,
/// sufficient test: `Some(true)` = proved (fixed slots pinned by the box, every equation identically zero there, every
/// bound / recenter bound non-negative on the box, every exception branch misses the box, no pole, no elided loci);
/// `Some(false)` = refuted cheaply (a fixed slot the box does not pin to its value, a linear bound negative on the
/// whole box ...); `None` = not decided (nonlinear bound / branch with an undecided emptiness). Decisions by interval
/// arithmetic first, then by [`crate::parametric::descent::emptiness`] (exact sign splits / Fourier-Motzkin).
pub fn box_inside_domain(
    bx: &[(Option<i64>, Option<i64>)],
    d: &crate::domain::CaseDomain,
    frame: &NodeFrame,
    budget: &crate::loci::contain::ProofBudget,
) -> Option<bool> {
    use crate::domain::CaseDomain;
    use crate::parametric::descent::{EmptyState, emptiness};
    use crate::poly::Poly;
    if d.witness_elided || !d.poles.is_empty() {
        return Some(false);
    }
    for (s, f) in d.fixed.iter().enumerate() {
        if let Some(v) = f {
            let v = i64::from(*v);
            if bx[s] != (Some(v), Some(v)) {
                return Some(false);
            }
        }
    }
    // the box as a domain (for the exact tests)
    let n = frame.arity();
    let box_domain = || -> Option<CaseDomain> {
        let mut c = CaseDomain::free(n, Some(frame.mask().to_vec()));
        for (s, (l, h)) in bx.iter().enumerate() {
            if let Some(l) = l {
                c.bounds
                    .push(Poly::var(s).sub(&Poly::constant(i128::from(*l))).ok()?);
            }
            if let Some(h) = h {
                c.bounds
                    .push(Poly::constant(i128::from(*h)).sub(&Poly::var(s)).ok()?);
            }
        }
        Some(c)
    };
    let mut undecided = false;
    for e in &d.equations {
        match poly_range(e, bx) {
            (Some(0), Some(0)) => {}
            (Some(l), _) if l > 0 => return Some(false),
            (_, Some(h)) if h < 0 => return Some(false),
            _ => undecided = true,
        }
    }
    for b in d.bounds.iter().chain(&d.recenter_bounds) {
        match poly_range(b, bx) {
            (Some(l), _) if l >= 0 => {}
            (_, Some(h)) if h < 0 => return Some(false),
            _ => {
                // exact: the box with b <= -1 is empty
                let bd = box_domain()?;
                let neg = b.neg().ok()?.sub(&Poly::constant(1)).ok()?;
                match emptiness(&bd, frame, budget, &[], &[neg]) {
                    (EmptyState::Empty, _, _) => {}
                    (EmptyState::NonEmpty, _, _) => return Some(false),
                    _ => undecided = true,
                }
            }
        }
    }
    for br in &d.exceptions {
        // the branch misses the box if one member is nonzero on the whole box (interval), else exact emptiness
        let misses = br.iter().any(|m| match poly_range(m, bx) {
            (Some(l), _) if l > 0 => true,
            (_, Some(h)) if h < 0 => true,
            _ => false,
        });
        if misses {
            continue;
        }
        let bd = box_domain()?;
        match emptiness(&bd, frame, budget, br, &[]) {
            (EmptyState::Empty, _, _) => {}
            (EmptyState::NonEmpty, _, _) => return Some(false),
            _ => undecided = true,
        }
    }
    if undecided { None } else { Some(true) }
}

/// Up to one key of the element with `t <= D + r <= t + w` (the region with that slab of the frame's excess), for each
/// `t` of `ts`; and, for each unbounded slot of the element, one key with that slot at least `push` beyond its chart
/// corner. Keys from the exact point search of the region ([`Region::decide_point`]); `None` entries are dropped.
pub fn sample_keys_beyond(
    el: &GapElement,
    frame: &NodeFrame,
    ts: &[i64],
    w: i64,
    push: i64,
) -> Vec<(String, Vec<i32>)> {
    let n = frame.arity();
    let lines = (0..n).filter(|&i| frame.is_line(i)).count() as i64;
    let excess: Vec<i64> = (0..n)
        .map(|i| if frame.is_line(i) { 1 } else { -1 })
        .collect();
    let neg: Vec<i64> = excess.iter().map(|v| -v).collect();
    let reg = el.region();
    let mut out = Vec::new();
    let try_cone = |label: String, c: Cone, out: &mut Vec<(String, Vec<i32>)>| {
        let Some(r) = with_cone(&reg, &c) else { return };
        if let Ok(Some(Some(t))) = r.decide_point(24, 20_000)
            && let Some(k) = key_of(&r, &t)
        {
            out.push((label, k));
        }
    };
    for &t in ts {
        let c = Cone {
            label: format!("{t} <= D + r <= {}", t + w),
            constraints: vec![(excess.clone(), t + w + lines), (neg.clone(), -(t + lines))],
        };
        try_cone(format!("D+r={t}"), c, &mut out);
    }
    if let Some(bx) = slot_box(el) {
        for (s, (l, h)) in bx.iter().enumerate() {
            let unbounded = if frame.is_line(s) {
                h.is_none()
            } else {
                l.is_none()
            };
            if !unbounded {
                continue;
            }
            let mut a = vec![0i64; n];
            // line: k_s >= 1 + push  <=>  -k_s <= -(1 + push); numerator: k_s <= -push
            let cons = if frame.is_line(s) {
                a[s] = -1;
                (a, -(1 + push))
            } else {
                a[s] = 1;
                (a, -push)
            };
            let _ = l;
            try_cone(
                format!("n{s} pushed {push}"),
                Cone {
                    label: format!("n{s} pushed"),
                    constraints: vec![cons],
                },
                &mut out,
            );
        }
    }
    out
}

/// Can the ENGINE's two-prime zero test misread a literal of `d` (equations, exception members) on the box `bx`?
/// `Some(true)` = NO literal can be misread there: for each literal `f = g q` (content `g`) the bound
/// `sum |c_q| prod max|x_v|^e` over the box (unbounded ends clipped to the representable keys `|n| <= 2^31 + margin`)
/// stays below `pm`, the product of the engine primes not dividing `g`; a nonzero `q(pt)` then never vanishes modulo
/// `pm`, so the engine's test equals the exact one on the box (the core's `modular_zero_risk` argument, restricted to
/// the box). `Some(false)` = some literal's bound reaches `pm` (a misread is not excluded by the bound); `None` =
/// unreadable literal (beyond i128). The margin covers target-pattern offsets (`pt = n - o`).
pub fn literals_exact_on_box(
    d: &crate::domain::CaseDomain,
    bx: &[(Option<i64>, Option<i64>)],
    margin: u64,
) -> Option<bool> {
    use crate::poly::ENGINE_PRIMES;
    let cap: u128 = (1u128 << 31) + u128::from(margin);
    let maxabs: Vec<u128> = bx
        .iter()
        .map(|(l, h)| {
            let l = l.map_or(cap, |v| u128::from(v.unsigned_abs()) + u128::from(margin));
            let h = h.map_or(cap, |v| u128::from(v.unsigned_abs()) + u128::from(margin));
            l.max(h).min(cap)
        })
        .collect();
    let lits = d.equations.iter().chain(d.exceptions.iter().flatten());
    for f in lits {
        let terms = f.small_terms()?;
        let mut g: u128 = 0;
        for (_, c) in terms {
            let (mut a, mut b) = (c.unsigned_abs(), g);
            while b != 0 {
                let r = a % b;
                a = b;
                b = r;
            }
            g = a;
        }
        if g == 0 || terms.len() <= 1 {
            continue; // zero polynomial or a monomial: exact (a monomial vanishes modulo a huge prime only where 0)
        }
        let mut pm: u128 = 1;
        for p in ENGINE_PRIMES {
            if !g.is_multiple_of(u128::from(p)) {
                pm = pm.saturating_mul(u128::from(p));
            }
        }
        let mut bound: u128 = 0;
        for (mo, c) in terms {
            let mut t = c.unsigned_abs() / g;
            for &(v, e) in mo.iter() {
                let mv = *maxabs.get(usize::from(v)).unwrap_or(&cap);
                t = mv
                    .checked_pow(e)
                    .and_then(|x| t.checked_mul(x))
                    .unwrap_or(u128::MAX);
            }
            bound = bound.saturating_add(t);
        }
        if bound >= pm {
            return Some(false);
        }
    }
    Some(true)
}

/// Every record of `disp` (tables in priority order) whose domain APPLIES at `key` (selected and no exception branch holding),
/// as (table, record) pairs. TERMINAL RULE (TRICKS 51): a declared terminal key may never be served by a record, so a delivered
/// table is refused when this list is non-empty for any terminal.
pub fn records_containing(disp: &Dispatcher, key: &[i32]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for &ti in disp.priority() {
        let t = &disp.tables()[ti];
        for (r, d) in t.domains.iter().enumerate() {
            if d.applies(key).applies() {
                out.push((ti, r));
            }
        }
    }
    out
}
