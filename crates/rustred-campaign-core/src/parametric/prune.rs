//! DEAD-RECORD PRUNING relative to a stated region (lane parametric 2026-11-17, implementer item "size step"; builder tool
//! `prune --region <cone> --keep-undecided`).
//!
//! A record is DEAD in a region when no key of the region has it as FIRST MATCH ([`record_liveness`]: the record's face,
//! capped, partitioned by the EARLIER records only; exact cell emptiness of the record's domain on every element the
//! earlier records leave uncovered, or bounded lattice enumeration; a budget overrun is UNDECIDED, never dead). Dropping a
//! dead record leaves the first match of every key of the region unchanged (an earlier winner stays the winner; where
//! the record is not first match it does not apply, so the next record still wins), hence coverage on the region is
//! never reduced. [`prune`] re-proves every drop (unless told to trust the verdicts), builds the pruned dispatcher,
//! re-runs the region certificate on both and FAILS when the pruned run has an uncovered or undecided element the
//! original has not, or when a probe key of the region changes its first-match record (table name + label).
//! The whole-sector statement may shrink: a dead record can serve keys outside the region (the dropped records are
//! carried separately by the caller, at their dispatch positions).

use serde_json::{Value, json};

use super::cone::{
    Cone, ConeOptions, capped_dispatcher, cone_coverage, coverage_from_partition, partition_for_cones_with,
    region_keys_each,
};
use crate::cover::{Dispatch, Dispatcher};
use crate::key::NodeFrame;
use crate::loci::contain::ProofBudget;
use crate::poly::Poly;

/// First-match liveness of one record in a region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Liveness {
    /// A key of the region whose first match is the record.
    Live(Vec<i32>),
    /// No key of the region has the record as first match (exact).
    Dead,
    /// Not decided (budget, overflow, nonlinear part): never pruned.
    Undecided(String),
}

/// The records of `disp` in first-match order: (table, record).
pub fn order(disp: &Dispatcher) -> Vec<(usize, usize)> {
    disp.priority()
        .iter()
        .flat_map(|&ti| (0..disp.tables()[ti].domains.len()).map(move |r| (ti, r)))
        .collect()
}

/// Is there a key of `region` whose FIRST MATCH in `disp` is the record at first-match position `pos`? Exact
/// (see the module header); `opts.budget` bounds the partition AND the element pass (its wall), `enum_cap` the lattice
/// enumeration of an element without a box cell.
pub fn record_liveness(
    disp: &Dispatcher,
    frame: &NodeFrame,
    region: &Cone,
    pos: usize,
    opts: &ConeOptions,
    enum_cap: u64,
) -> Liveness {
    let t0 = std::time::Instant::now();
    let ord = order(disp);
    let Some(&(ti, ri)) = ord.get(pos) else {
        return Liveness::Undecided(format!("position {pos} beyond {} records", ord.len()));
    };
    let n = frame.arity();
    let d = &disp.tables()[ti].domains[ri];
    // caps: the record's fixed slots (two one-slot caps each) and its one-slot linear bounds
    let mut caps = Cone {
        label: format!("face of record {pos}"),
        constraints: vec![],
    };
    for (s, fx) in d.fixed.iter().enumerate() {
        if let Some(v) = fx {
            let mut up = vec![0i64; n];
            up[s] = 1;
            let mut dn = vec![0i64; n];
            dn[s] = -1;
            caps.constraints.push((up, i64::from(*v)));
            caps.constraints.push((dn, -i64::from(*v)));
        }
    }
    for p in &d.bounds {
        let sup = p.support();
        if sup.len() != 1 || p.degree() != 1 || p.has_d() {
            continue;
        }
        let s = sup[0];
        let Some(terms) = p.small_terms() else { continue };
        let (mut c, mut e) = (0i128, 0i128);
        for (mo, k) in terms {
            if mo.is_empty() {
                e = *k;
            } else {
                c = *k;
            }
        }
        let Ok(e64) = i64::try_from(e) else { continue };
        if c == 1 {
            let mut dn = vec![0i64; n];
            dn[s] = -1;
            caps.constraints.push((dn, e64));
        } else if c == -1 {
            let mut up = vec![0i64; n];
            up[s] = 1;
            caps.constraints.push((up, e64));
        }
    }
    let capped = match capped_dispatcher(disp, frame, &caps) {
        Ok(c) => c,
        Err(e) => return Liveness::Undecided(format!("caps: {e}")),
    };
    let ncaps = capped.tables()[0].domains.len();
    let mut o = opts.clone();
    o.rule_subset = Some((0..ncaps).chain((0..pos).map(|p| ncaps + p)).collect());
    let cp = match partition_for_cones_with(&capped, frame, &o, None) {
        Ok(c) => c,
        Err(e) => return Liveness::Undecided(format!("partition: {e}")),
    };
    if let Some(b) = cp.report.budget_exhausted() {
        return Liveness::Undecided(format!("partition budget: {b}"));
    }
    let cone = region.clone().and(&caps);
    let cov = match coverage_from_partition(&cp, &capped, frame, &cone, &o) {
        Ok(c) => c,
        Err(e) => return Liveness::Undecided(format!("coverage: {e}")),
    };
    let deadline = o.budget.wall.map(|w| t0 + w);
    // the region as bounds b - a.k >= 0
    let mut rbounds: Vec<Poly> = Vec::new();
    for (a, b) in &cone.constraints {
        let mut q = Poly::constant(i128::from(*b));
        for (s, c) in a.iter().enumerate() {
            if *c != 0 {
                q = match Poly::var(s).mul(&Poly::constant(i128::from(*c))).and_then(|x| q.sub(&x)) {
                    Ok(x) => x,
                    Err(e) => return Liveness::Undecided(format!("region bound: {e:?}")),
                };
            }
        }
        rbounds.push(q);
    }
    let mut exhausted = true;
    for l in &cov.listed {
        if deadline.is_some_and(|dl| std::time::Instant::now() > dl) {
            return Liveness::Undecided("wall budget reached in the element pass".into());
        }
        let Some(i) = l["element"].as_u64() else { continue };
        // every listed gap element of the EARLIER records is examined (an "enumerated_covered" verdict is decided with the FULL
        // dispatcher, the record itself included: never a reason to skip); a witness counts only if the full dispatcher's first
        // match at it IS the record (2026-11-17 fix: a key of a nonlinear part where an earlier record applies gave a false live)
        let el = &cp.elements[i as usize];
        if let Some(cell) = &el.cell {
            use super::descent::{EmptyState, emptiness};
            let mut bounds = rbounds.clone();
            for (sl, (lo, hi)) in cell.iter().enumerate() {
                if let Some(lo) = lo {
                    if let Ok(x) = Poly::var(sl).sub(&Poly::constant(i128::from(*lo))) {
                        bounds.push(x);
                    }
                }
                if let Some(hi) = hi {
                    if let Ok(x) = Poly::constant(i128::from(*hi)).sub(&Poly::var(sl)) {
                        bounds.push(x);
                    }
                }
            }
            match emptiness(d, frame, &ProofBudget::default(), &[], &bounds) {
                (EmptyState::Empty, _, _) => continue,
                (EmptyState::NonEmpty, Some(w), _) if is_first_match(disp, &w, ti, ri) => return Liveness::Live(w),
                (EmptyState::NonEmpty, _, _) => {
                    // a key of the element where the record applies but is not first match: enumerate the element instead
                }
                _ => {
                    exhausted = false;
                    continue;
                }
            }
        }
        let mut found = None;
        let mut push = |k: &[i32]| {
            if found.is_none() && d.applies(k).applies() && is_first_match(disp, k, ti, ri) {
                found = Some(k.to_vec());
            }
        };
        let sized = region_keys_each(&el.region(), &cone, enum_cap, &mut push);
        if let Some(k) = found {
            return Liveness::Live(k);
        }
        if sized.is_none() {
            exhausted = false;
        }
    }
    if exhausted {
        Liveness::Dead
    } else {
        Liveness::Undecided("an element was not decided (emptiness budget or enumeration cap)".into())
    }
}

/// Is the record (table `ti`, record `ri`) the FIRST MATCH of `disp` at `k` (the witness check of a live verdict)?
fn is_first_match(disp: &Dispatcher, k: &[i32], ti: usize, ri: usize) -> bool {
    matches!(disp.first_match(k), Ok(Dispatch::Covered(r)) if r.table == ti && r.record == ri)
}

/// The pruning outcome.
#[derive(Clone, Debug)]
pub struct PruneReport {
    /// First-match positions dropped (all DEAD, re-proved unless `trust_verdicts`).
    pub dropped: Vec<usize>,
    pub kept: usize,
    pub verdict_before: String,
    pub verdict_after: String,
    pub problems: Vec<String>,
    pub probes_checked: usize,
}

impl PruneReport {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
    pub fn to_json(&self) -> Value {
        json!({"dropped": self.dropped.len(), "kept": self.kept, "verdict_before": self.verdict_before,
            "verdict_after": self.verdict_after, "probes_checked": self.probes_checked, "problems": self.problems,
            "ok": self.ok()})
    }
}

/// Prune the records whose verdict is DEAD (`keep_undecided`: undecided records are kept; `false` drops them too, which
/// the run then has to justify by the certificate checks alone). `verdicts[pos]` per first-match position (None = not
/// computed: kept). `trust_verdicts = false` re-proves every drop with [`record_liveness`]. Returns the pruned
/// dispatcher and the report; `Err` only for construction failures; a failed check is in `report.problems`.
pub fn prune(
    disp: &Dispatcher,
    frame: &NodeFrame,
    region: &Cone,
    verdicts: &[Option<Liveness>],
    keep_undecided: bool,
    trust_verdicts: bool,
    opts: &ConeOptions,
    enum_cap: u64,
) -> Result<(Dispatcher, PruneReport), String> {
    let ord = order(disp);
    let mut problems = Vec::new();
    let mut drop = vec![false; ord.len()];
    for (pos, v) in verdicts.iter().enumerate().take(ord.len()) {
        match v {
            Some(Liveness::Dead) => drop[pos] = true,
            Some(Liveness::Undecided(_)) if !keep_undecided => drop[pos] = true,
            _ => {}
        }
    }
    if !trust_verdicts {
        for (pos, &dr) in drop.iter().enumerate() {
            if !dr {
                continue;
            }
            match record_liveness(disp, frame, region, pos, opts, enum_cap) {
                Liveness::Dead => {}
                Liveness::Live(w) => problems.push(format!("position {pos}: IS first match in the region at {w:?} (not dead)")),
                Liveness::Undecided(why) if keep_undecided => {
                    problems.push(format!("position {pos}: dead not re-proved ({why})"))
                }
                Liveness::Undecided(_) => {}
            }
        }
    }
    // the pruned dispatcher: every table with the dropped records removed (labels / meta in step)
    let mut tables: Vec<crate::cover::Table> = disp.tables().to_vec();
    let kept = drop.iter().filter(|&&x| !x).count();
    for (ti, t) in tables.iter_mut().enumerate() {
        let keep: Vec<bool> = (0..t.domains.len())
            .map(|ri| {
                let pos = ord.iter().position(|&x| x == (ti, ri));
                !pos.is_some_and(|p| drop[p])
            })
            .collect();
        let mut it = keep.iter();
        t.domains.retain(|_| *it.next().unwrap());
        let mut it = keep.iter();
        t.labels.retain(|_| *it.next().unwrap());
        let mut it = keep.iter();
        t.meta.retain(|_| *it.next().unwrap());
    }
    let mut pruned = Dispatcher::new(tables, disp.priority().to_vec()).map_err(|e| e.to_string())?;
    if let Some(ch) = disp.chart() {
        pruned = pruned.with_chart(ch.to_vec()).map_err(|e| e.to_string())?;
    }
    let a = cone_coverage(disp, frame, region, opts)?;
    let b = cone_coverage(&pruned, frame, region, opts)?;
    if b.uncovered > a.uncovered {
        problems.push(format!("coverage REDUCED on the region: {} uncovered elements, {} before", b.uncovered, a.uncovered));
    }
    if b.undecided > a.undecided {
        problems.push(format!("{} undecided elements on the region, {} before", b.undecided, a.undecided));
    }
    if a.verdict != b.verdict {
        problems.push(format!("region verdict {} -> {}", a.verdict, b.verdict));
    }
    // first-match identity on the original run's probe keys (table name + label)
    let name_of = |dd: &Dispatcher, k: &[i32]| -> String {
        match dd.first_match(k) {
            Ok(Dispatch::Covered(r)) => format!("{}#{}", dd.tables()[r.table].name, dd.tables()[r.table].labels[r.record]),
            Ok(x) => format!("{x:?}"),
            Err(e) => format!("error {e:?}"),
        }
    };
    let mut probes_checked = 0usize;
    for p in &a.probes {
        let Some(k) = p["key"].as_array() else { continue };
        let k: Vec<i32> = k.iter().filter_map(|x| x.as_i64().and_then(|v| i32::try_from(v).ok())).collect();
        if !region.contains(&k) {
            continue;
        }
        probes_checked += 1;
        let (x, y) = (name_of(disp, &k), name_of(&pruned, &k));
        if x != y {
            problems.push(format!("probe {k:?}: first match {x} -> {y}"));
        }
    }
    let report = PruneReport {
        dropped: (0..ord.len()).filter(|&p| drop[p]).collect(),
        kept,
        verdict_before: a.verdict.to_string(),
        verdict_after: b.verdict.to_string(),
        problems,
        probes_checked,
    };
    Ok((pruned, report))
}
