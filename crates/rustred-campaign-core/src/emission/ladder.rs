//! The replacement LADDER, pure part (emission contract section 5; TRICKS 68 "certify by replacement"): what a new
//! exception branch or an OPEN locus becomes (a fixed slot, an affine case equation, a point, or a curve that cannot be
//! branched), the CHILD case record of a branch, the exception bookkeeping of the pieces (every piece carries the
//! replaced record's branches + the branches of its ancestors and of the siblings before it), and the partition proof of
//! the pieces against the replaced record (`parametric::prune::record_liveness`, exact on the chart orthant). Generic:
//! slots, values and branch texts come from the records and the check outputs.
use crate::cover::{Dispatcher, Table};
use crate::domain::DomainOptions;
use crate::key::NodeFrame;
use crate::parametric::cone::{Cone, ConeOptions};
use crate::parametric::prune::{Liveness as PLive, record_liveness};
use crate::poly::{D_VAR, Poly};
use crate::record::{ReadOptions, RhsMode, read_table_file};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// What one branch (a list of polynomials that all vanish on it) of a piece becomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BranchPlan {
    /// the branch fixes these slots (one linear one-slot polynomial each, integer roots): child case = the slots fixed
    Fix(Vec<(usize, i64)>),
    /// a linear polynomial in several slots: child case = the parent case + this equation
    Equation(String),
    /// no integer point once the fixed slots are substituted (a one-slot polynomial with a non-integer root, or a
    /// nonzero constant): nothing to cover
    Empty,
    /// nonlinear (a curve): cannot be branched by this ladder
    Curve(Vec<String>),
}

/// Engine text of the polynomial n_s - v ("n14", "-1+n8", "1+n13").
pub fn slot_minus_text(slot: usize, v: i64) -> String {
    match v {
        0 => format!("n{slot}"),
        v if v > 0 => format!("-{v}+n{slot}"),
        v => format!("{}+n{slot}", -v),
    }
}

fn substitute_fixed(p: &Poly, fixed: &[Option<i64>]) -> Result<Poly, String> {
    // evaluate the fixed slots: rebuild term by term
    let mut out = Poly::zero();
    for (m, c) in p.terms() {
        let mut t = Poly::constant(*c);
        for (v, e) in m {
            let f = if *v == D_VAR {
                Poly::d()
            } else {
                match fixed.get(*v as usize).copied().flatten() {
                    Some(x) => Poly::constant(x as i128),
                    None => Poly::var(*v as usize),
                }
            };
            for _ in 0..*e {
                t = t.mul(&f).map_err(|e| format!("{e:?}"))?;
            }
        }
        out = out.add(&t).map_err(|e| format!("{e:?}"))?;
    }
    Ok(out)
}

/// Linear one-slot form `a n_s + b` of a polynomial (None when it is not of that form).
fn one_slot_linear(p: &Poly) -> Option<(usize, i128, i128)> {
    let mut slot = None;
    let (mut a, mut b) = (0i128, 0i128);
    for (m, c) in p.terms() {
        match m.as_slice() {
            [] => b = *c,
            [(v, 1)] if *v != D_VAR => {
                if slot.is_some_and(|s| s != *v as usize) {
                    return None;
                }
                slot = Some(*v as usize);
                a = *c;
            }
            _ => return None,
        }
    }
    slot.map(|s| (s, a, b))
}

fn is_linear(p: &Poly) -> bool {
    p.terms()
        .iter()
        .all(|(m, _)| m.iter().map(|(_, e)| *e).sum::<u32>() <= 1 && m.iter().all(|(v, _)| *v != D_VAR))
}

/// Plan one branch on a piece whose fixed slots are `fixed`.
pub fn plan_branch(branch: &[String], fixed: &[Option<i64>]) -> Result<BranchPlan, String> {
    let arity = fixed.len();
    let mut polys = vec![];
    for t in branch {
        let p = Poly::parse(t, arity, true).map_err(|e| format!("{t}: {e:?}"))?;
        let q = substitute_fixed(&p, fixed)?;
        if q.terms().is_empty() {
            continue; // vanishes identically on the piece: no constraint
        }
        polys.push((t.clone(), q));
    }
    if polys.is_empty() {
        return Ok(BranchPlan::Curve(branch.to_vec())); // every key of the piece is on it: not a branch
    }
    let mut fix = vec![];
    for (_, q) in &polys {
        if q.terms().iter().all(|(m, _)| m.is_empty()) {
            return Ok(BranchPlan::Empty); // nonzero constant
        }
        match one_slot_linear(q) {
            Some((s, a, b)) => {
                if a == 0 || b % a != 0 {
                    return Ok(BranchPlan::Empty);
                }
                fix.push((s, (-b / a) as i64));
            }
            None => {
                if polys.len() == 1 && is_linear(q) {
                    return Ok(BranchPlan::Equation(polys[0].0.clone()));
                }
                return Ok(BranchPlan::Curve(branch.to_vec()));
            }
        }
    }
    fix.sort_unstable();
    fix.dedup();
    let mut slots = BTreeSet::new();
    for (s, _) in &fix {
        if !slots.insert(*s) {
            return Ok(BranchPlan::Empty); // two different values for one slot
        }
    }
    Ok(BranchPlan::Fix(fix))
}

/// An OPEN locus of a check (its d-coefficient generators, stored frame, fixed slots substituted) as a branch: the
/// first generator that fixes a slot, or that is a linear equation; a curve otherwise.
pub fn open_locus_branch(generators: &[String], fixed: &[Option<i64>]) -> Result<BranchPlan, String> {
    let mut best: Option<(BranchPlan, String)> = None;
    for g in generators {
        match plan_branch(std::slice::from_ref(g), fixed)? {
            BranchPlan::Fix(f) => return Ok(BranchPlan::Fix(f)),
            BranchPlan::Empty => return Ok(BranchPlan::Empty),
            BranchPlan::Equation(e) if best.is_none() => {
                best = Some((BranchPlan::Equation(e.clone()), e));
            }
            _ => {}
        }
    }
    Ok(best
        .map(|b| b.0)
        .unwrap_or_else(|| BranchPlan::Curve(generators.to_vec())))
}

/// The branch text list of a plan (what goes into `exceptions`).
pub fn branch_texts(plan: &BranchPlan) -> Option<Vec<String>> {
    match plan {
        BranchPlan::Fix(f) => Some(f.iter().map(|(s, v)| slot_minus_text(*s, *v)).collect()),
        BranchPlan::Equation(e) => Some(vec![e.clone()]),
        _ => None,
    }
}

/// Canonical text of a branch (polynomials sorted and joined by `&`) for set membership.
pub fn branch_key(b: &[String]) -> String {
    let mut v: Vec<String> = b.iter().map(|x| x.replace(' ', "")).collect();
    v.sort();
    v.join("&")
}

/// Branches of a record as text lists.
pub fn branches_of(rec: &Value) -> Vec<Vec<String>> {
    rec.get("exceptions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|b| match b {
            Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
            Value::String(t) => vec![t.clone()],
            o => vec![o.to_string()],
        })
        .collect()
}

/// Union of branch lists, first occurrence kept, original texts.
pub fn union_branches(lists: &[&[Vec<String>]]) -> Vec<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut out = vec![];
    for l in lists {
        for b in l.iter() {
            if seen.insert(branch_key(b)) {
                out.push(b.clone());
            }
        }
    }
    out
}

fn target_slots(t: &str) -> Result<Vec<String>, String> {
    let inner = t
        .trim()
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .ok_or_else(|| format!("target {t} is not I(..)"))?;
    Ok(inner.split(',').map(|x| x.trim().to_string()).collect())
}

/// Value of a target slot text "n<s>" / "n<s>+k" / "n<s>-k" at n<s> = v (a number stays).
fn slot_at(t: &str, v: i64) -> Result<String, String> {
    match t.strip_prefix('n') {
        None => Ok(t.to_string()),
        Some(r) => {
            let p = r.find(['+', '-']).unwrap_or(r.len());
            let off: i64 = if p < r.len() {
                r[p..]
                    .trim_start_matches('+')
                    .parse()
                    .map_err(|_| format!("target slot {t}"))?
            } else {
                0
            };
            Ok((v + off).to_string())
        }
    }
}

/// Rust-Debug coordinate case text of a fixed vector (the engine's `case` form for coordinate cases).
pub fn coordinate_case_text(fixed: &[Option<i64>]) -> String {
    let f: Vec<String> = fixed
        .iter()
        .map(|x| match x {
            Some(v) => format!("Some({v})"),
            None => "None".into(),
        })
        .collect();
    format!(
        "Coordinate(CoordinateCase {{ fixed: [{}] }})",
        f.join(", ")
    )
}

/// The fixed vector of a record.
pub fn fixed_of(rec: &Value) -> Vec<Option<i64>> {
    rec.get("fixed")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(Value::as_i64).collect())
        .unwrap_or_default()
}

/// The CHILD case record of a branch plan: the parent with the slots fixed (target and case text rewritten) or the
/// equation appended; `exceptions` set to `inherited`; rhs emptied (the child is lifted, not copied).
pub fn child_record(parent: &Value, plan: &BranchPlan, inherited: &[Vec<String>]) -> Result<Value, String> {
    let mut r = parent.clone();
    let mut fixed = fixed_of(parent);
    match plan {
        BranchPlan::Fix(f) => {
            let mut t = target_slots(parent["target"].as_str().ok_or("target")?)?;
            for (s, v) in f {
                if *s >= fixed.len() {
                    return Err(format!("slot {s} beyond the arity"));
                }
                fixed[*s] = Some(*v);
                t[*s] = slot_at(&t[*s], *v)?;
            }
            r["fixed"] = json!(fixed);
            r["target"] = json!(format!("I({})", t.join(",")));
            let eqs = r["equations"].as_array().map_or(0, Vec::len);
            if eqs == 0 {
                r["case"] = json!(coordinate_case_text(&fixed));
            }
        }
        BranchPlan::Equation(e) => {
            let mut eqs: Vec<Value> = r["equations"].as_array().cloned().unwrap_or_default();
            eqs.push(json!(e));
            r["equations"] = Value::Array(eqs);
        }
        _ => return Err("only fixed-slot and equation branches have a child case".into()),
    }
    r["exceptions"] = json!(inherited);
    r["rhs"] = json!([]);
    Ok(r)
}

// ------------------------------------------------------------------------------------------------ partition proof

/// One liveness decision of the partition proof.
#[derive(Clone, Debug)]
pub struct PartCheck {
    pub check: String,
    pub witness: bool,
    pub verdict: String,
    pub why: String,
}

/// The conjunction record of two records (their domains' intersection), or None when two fixed slots disagree.
pub fn conjunction(x: &Value, y: &Value) -> Result<Option<Value>, String> {
    let fx = fixed_of(x);
    let fy = fixed_of(y);
    if fx.len() != fy.len() || x["chart"] != y["chart"] {
        return Err("conjunction of records of different frames".into());
    }
    let n = fx.len();
    let mut fixed = vec![None; n];
    for s in 0..n {
        fixed[s] = match (fx[s], fy[s]) {
            (Some(a), Some(b)) if a != b => return Ok(None),
            (Some(a), _) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
    }
    let slots = target_slots(x["target"].as_str().ok_or("target")?)?;
    let mut target = vec![];
    for s in 0..n {
        target.push(match (fixed[s], fx[s]) {
            (Some(v), None) => slot_at(&slots[s], v)?,
            _ => slots[s].clone(),
        });
    }
    let cat = |k: &str| -> Value {
        let mut v: Vec<Value> = x[k].as_array().cloned().unwrap_or_default();
        for e in y[k].as_array().cloned().unwrap_or_default() {
            if !v.contains(&e) {
                v.push(e);
            }
        }
        Value::Array(v)
    };
    let side = |r: &Value, k: &str, s: usize| r["domain_bounds"][k].get(s).and_then(Value::as_i64);
    let mut z = x.clone();
    z["fixed"] = json!(fixed);
    z["target"] = json!(format!("I({})", target.join(",")));
    z["equations"] = cat("equations");
    z["exceptions"] = cat("exceptions");
    z["witness_loci"] = cat("witness_loci");
    if !x["domain_bounds"].is_null() || !y["domain_bounds"].is_null() {
        let pick = |k: &str, max: bool| -> Vec<Value> {
            (0..n)
                .map(|s| match (side(x, k, s), side(y, k, s)) {
                    (Some(a), Some(b)) => json!(if max { a.max(b) } else { a.min(b) }),
                    (Some(a), None) | (None, Some(a)) => json!(a),
                    _ => Value::Null,
                })
                .collect()
        };
        z["domain_bounds"] = json!({"min": pick("min", true), "max": pick("max", false)});
    }
    z["case"] = json!("conjunction");
    Ok(Some(z))
}

fn write_mini(path: &std::path::Path, header: &str, charts: &[String], recs: &[Value]) -> Result<(), String> {
    let mut s = String::new();
    s.push_str(header);
    s.push('\n');
    for c in charts {
        s.push_str(c);
        s.push('\n');
    }
    for (i, r) in recs.iter().enumerate() {
        let mut r = r.clone();
        r["type"] = json!("rule");
        r["index"] = json!(i);
        r["rhs"] = json!([]);
        if let Some(o) = r.as_object_mut() {
            o.remove("derivation");
        }
        s.push_str(&r.to_string());
        s.push('\n');
    }
    s.push_str(&json!({"type": "end", "charts": charts.len(), "rules": recs.len(), "residuals": 0}).to_string());
    s.push('\n');
    std::fs::write(path, s).map_err(|e| format!("{}: {e}", path.display()))
}

fn liveness(path: &std::path::Path, pos: usize, witness: bool) -> Result<(String, String), String> {
    let tf = read_table_file(path, ReadOptions { rhs: RhsMode::Skip }).map_err(|e| format!("{e:?}"))?;
    let frame = NodeFrame::from_header(tf.header.as_ref().ok_or("no header")?).map_err(|e| format!("{e:?}"))?;
    let opts = DomainOptions {
        witness,
        ..DomainOptions::default()
    };
    let t = Table::from_table_file("t", path.display().to_string(), &tf, opts).map_err(|e| format!("{e:?}"))?;
    let d = Dispatcher::new(vec![t], vec![0]).map_err(|e| format!("{e:?}"))?;
    Ok(
        match record_liveness(&d, &frame, &Cone::orthant(), pos, &ConeOptions::default(), 1_000_000) {
            PLive::Dead => ("dead".into(), String::new()),
            PLive::Live(k) => ("live".into(), format!("witness key {k:?}")),
            PLive::Undecided(y) => ("undecided".into(), y),
        },
    )
}

/// PARTITION PROOF (exact on the chart orthant, witness check of the domains off and on): (1) the original is dead
/// after the pieces (cover); (2) every piece is dead after the original (containment); (3) every pairwise conjunction
/// record is dead (disjoint); controls (the original alone, every piece alone) live so the dead verdicts are not
/// vacuous. `witness_modes`: the domain witness options to run (`[false, true]` for real tables, as the importer's
/// default; records without `witness_loci` decide only with `false`). Temporary tables go to `tmp`. Returns (partition
/// holds, the checks).
pub fn prove_partition(
    header: &str,
    charts: &[String],
    original: &Value,
    pieces: &[Value],
    tmp: &std::path::Path,
    witness_modes: &[bool],
) -> Result<(bool, Vec<PartCheck>), String> {
    let mut out = vec![];
    let mut dead_ok = true;
    let mut controls = true;
    let k = pieces.len();
    let p = |name: &str| tmp.join(format!("partition-{name}.table.jsonl"));
    for &witness in witness_modes {
        let mut push = |check: String, v: (String, String), want: &str, flag: &mut bool| {
            *flag &= v.0 == want;
            out.push(PartCheck {
                check,
                witness,
                verdict: v.0,
                why: v.1,
            });
        };
        for (i, r) in std::iter::once(original).chain(pieces.iter()).enumerate() {
            write_mini(&p("control"), header, charts, std::slice::from_ref(r))?;
            let v = liveness(&p("control"), 0, witness)?;
            push(format!("control: record {i} alone (live)"), v, "live", &mut controls);
        }
        let mut recs: Vec<Value> = pieces.to_vec();
        recs.push(original.clone());
        write_mini(&p("cover"), header, charts, &recs)?;
        let v = liveness(&p("cover"), k, witness)?;
        push("cover: original after the pieces".into(), v, "dead", &mut dead_ok);
        let mut recs = vec![original.clone()];
        recs.extend(pieces.iter().cloned());
        write_mini(&p("contained"), header, charts, &recs)?;
        for i in 0..k {
            let v = liveness(&p("contained"), 1 + i, witness)?;
            push(format!("contained: piece {i} after the original"), v, "dead", &mut dead_ok);
        }
        for i in 0..k {
            for j in i + 1..k {
                match conjunction(&pieces[i], &pieces[j])? {
                    None => push(
                        format!("disjoint: pieces {i} and {j}"),
                        ("dead".into(), "fixed slots disagree".into()),
                        "dead",
                        &mut dead_ok,
                    ),
                    Some(z) => {
                        write_mini(&p("meet"), header, charts, &[z])?;
                        let v = liveness(&p("meet"), 0, witness)?;
                        push(format!("disjoint: pieces {i} and {j}"), v, "dead", &mut dead_ok);
                    }
                }
            }
        }
    }
    Ok((dead_ok && controls, out))
}
