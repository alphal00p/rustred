//! Verified symmetry for the import (interfaces/import.md section 13): the chart's group DERIVED from the family file
//! (presented in the tables' frame) and proved exactly by the identity-test lane's verifier
//! (`replay_base::symrows::{verify_derived, verify_cross_checked}`); supplied group data (a full group or generators)
//! is CHECKED against it, never trusted. Two uses:
//! - image credits: `CreditPolicy::from_verified` (the closure then counts them; an unverified group is `diagnostic`);
//! - SUPPLIED SYMMETRY RELATIONS ("symmetry relation at the key", extra-rules.md reserved candidate hook): a supplied
//!   multi-term relation `I(target) = sum c_j I(k_j)` is VERIFIED by re-deriving it: it must lie in the exact row span
//!   of the symmetry rows of the verified group at the seeds (default: the target itself; numerator images expanded in
//!   the basis {D_i, 1}, `rows_at(.., SymRowMode::Expanded, ..)`). Every rational operation goes through the
//!   [`CheckAlgebra`] trait (the core's certificate backend `Independent`, or the Symbolica backend of crate
//!   rustred-campaign-algebra): no algebra is written here, only the matrix assembly (columns = keys).
//!
//! Nothing here knows a node: arity, chart, presentation and group order come from the family file and the tables.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::{ImportError, io_err};
use super::lines::verify_file;
use super::spec::{FileRef, resolve};
use crate::replay_base::algebra::{CheckAlgebra, Q};
use crate::replay_base::family::Family;
use crate::replay_base::symrows::{
    DeriveOptions, SymGroup, SymRowMode, VerifiedGroup, rows_at_with, verify_cross_checked,
    verify_derived,
};

/// How the supplied group data is given (interfaces/import.md 13.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupFormat {
    /// No group data: the group is derived from the family alone.
    #[default]
    Derive,
    /// `rustred.campaign.symgroup.v1` (every non-identity element).
    Symgroup,
    /// The engine's `symmetry_group` table record (or its `group` object).
    EngineRecord,
    /// GENERATORS in the `symgroup.v1` element form (line images exact; other images optional): every generator must
    /// be an element of the derived group with equal stated images, and the generators must generate the WHOLE derived
    /// group (on the line action); a proper subgroup is refused.
    Generators,
}

/// The verified-group input of a source (`symmetry_credit`, or `supplied_relations.verify`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupSpec {
    /// The family file (propagators in the FAMILY order; presented into the tables' frame by the import). Absent:
    /// the frame table header's `family_source` text (already in the PRESENTED frame, as the engine writes it).
    #[serde(default)]
    pub family: Option<FileRef>,
    /// Supplied group data in the PRESENTED frame (checked against the derived group), if any.
    #[serde(default)]
    pub group: Option<FileRef>,
    #[serde(default)]
    pub group_format: GroupFormat,
    /// Bound of the derivation's enumeration (default: the verifier's).
    #[serde(default)]
    pub max_candidates: Option<u64>,
    /// Supplied relations: bound of one expanded symmetry row (default [`DEFAULT_MAX_TERMS`]).
    #[serde(default)]
    pub max_terms: Option<usize>,
}

/// Default bound of one expanded symmetry row when verifying a supplied relation.
pub const DEFAULT_MAX_TERMS: usize = 1 << 16;

/// What was verified (written into statements).
#[derive(Clone, Debug, Default, Serialize)]
pub struct GroupReport {
    pub family: String,
    pub family_sha256: String,
    pub presented: bool,
    pub group_file: Option<(String, String)>,
    pub format: String,
    pub elements: usize,
    pub provenance: String,
    pub digest: String,
    pub family_digest: String,
}

/// The family text with its `prop(..)` lines reordered into the presented frame: presented slot `j` carries the
/// family's slot `new_to_old[j]` (its momentum and eps shift) and is RELABELLED `D{j+1}`, as the engine's header
/// `family_source` is. Refused unless the text has exactly one `prop(` line per slot, each with a label.
pub fn presented_family_text(src: &str, new_to_old: &[usize]) -> Result<String, String> {
    let lines: Vec<&str> = src.lines().collect();
    let props: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with("prop("))
        .map(|(i, _)| i)
        .collect();
    if props.len() != new_to_old.len() {
        return Err(format!(
            "the family has {} prop(..) lines, the frame {} slots",
            props.len(),
            new_to_old.len()
        ));
    }
    let mut seen = vec![false; new_to_old.len()];
    for &o in new_to_old {
        if o >= seen.len() || std::mem::replace(&mut seen[o], true) {
            return Err("the presentation is not a permutation".into());
        }
    }
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    for (j, &line_no) in props.iter().enumerate() {
        let src_line = lines[props[new_to_old[j]]].trim();
        // keep the indentation of the slot's line; a trailing comma is kept per POSITION (the last prop may differ)
        let indent: String = lines[line_no]
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        let had_comma = lines[line_no].trim_end().ends_with(',');
        let body = src_line.trim_end_matches(',');
        // relabel: presented slot j is `D{j+1}` (Family::parse wants D1..Dn in order; audit-credits m2), the rest of
        // the line (momentum, eps shift) travels with the slot
        let rest = body
            .strip_prefix("prop(")
            .and_then(|r| r.split_once(','))
            .map(|(_, r)| r)
            .ok_or_else(|| format!("prop line without a label: {src_line:?}"))?;
        out[line_no] = format!(
            "{indent}prop(D{},{rest}{}",
            j + 1,
            if had_comma { "," } else { "" }
        );
    }
    Ok(out.join("\n"))
}

/// Derive and verify the chart's group (presented frame), checking any supplied group data against it.
pub fn load_verified_group(
    spec: &GroupSpec,
    base: &Path,
    chart: &[bool],
    new_to_old: &[usize],
    header_family: Option<&str>,
) -> Result<(VerifiedGroup, GroupReport), ImportError> {
    load_verified_group_family(spec, base, chart, new_to_old, header_family)
        .map(|(vg, r, _)| (vg, r))
}

/// [`load_verified_group`] that also returns the PRESENTED family model the group was derived from (lane import5:
/// the momentum proof of a `sym` record's own matrix needs it).
pub fn load_verified_group_family(
    spec: &GroupSpec,
    base: &Path,
    chart: &[bool],
    new_to_old: &[usize],
    header_family: Option<&str>,
) -> Result<(VerifiedGroup, GroupReport, Family), ImportError> {
    let (fpath, fsha, src, presented_already) = match &spec.family {
        Some(f) => {
            let p = resolve(base, &f.path);
            let d = verify_file(&p, f.sha256.as_deref())?;
            let t = std::fs::read_to_string(&p).map_err(|e| io_err(&p, e))?;
            (p, d, t, false)
        }
        None => {
            let t = header_family.ok_or_else(|| {
                ImportError::Spec(
                    "no family file and the frame table header carries no family_source".into(),
                )
            })?;
            let mut h = crate::closure::sha256::Sha256::new();
            h.update(t.as_bytes());
            (
                std::path::PathBuf::from("<frame header family_source>"),
                h.finish_hex(),
                t.to_string(),
                true,
            )
        }
    };
    let identity = presented_already || new_to_old.iter().enumerate().all(|(i, &o)| i == o);
    let text = if identity {
        src
    } else {
        presented_family_text(&src, new_to_old).map_err(|m| ImportError::Source {
            source: fpath.display().to_string(),
            msg: format!("family presentation: {m}"),
        })?
    };
    let family = Family::parse(&text).map_err(|e| ImportError::Source {
        source: fpath.display().to_string(),
        msg: format!("family: {e:?}"),
    })?;
    if family.n() != chart.len() {
        return Err(ImportError::Source {
            source: fpath.display().to_string(),
            msg: format!(
                "family of {} propagators for a frame of arity {}",
                family.n(),
                chart.len()
            ),
        });
    }
    let opts = DeriveOptions {
        max_candidates: spec
            .max_candidates
            .unwrap_or(DeriveOptions::default().max_candidates),
    };
    let refused = |m: String| ImportError::Source {
        source: fpath.display().to_string(),
        msg: format!("symmetry group refused: {m}"),
    };
    let mut report = GroupReport {
        family: fpath.display().to_string(),
        family_sha256: fsha,
        presented: !identity || presented_already,
        format: format!("{:?}", spec.group_format),
        ..Default::default()
    };
    let vg = match (&spec.group, spec.group_format) {
        (None, GroupFormat::Derive) => {
            verify_derived(&family, chart, &opts).map_err(|e| refused(format!("{e:?}")))?
        }
        (None, f) => {
            return Err(ImportError::Spec(format!(
                "group_format {f:?} needs a group file"
            )));
        }
        (Some(_), GroupFormat::Derive) => {
            return Err(ImportError::Spec(
                "a group file needs group_format symgroup | engine_record | generators".into(),
            ));
        }
        (Some(g), fmt) => {
            let gpath = resolve(base, &g.path);
            let gsha = verify_file(&gpath, g.sha256.as_deref())?;
            report.group_file = Some((gpath.display().to_string(), format!("sha256:{gsha}")));
            let txt = std::fs::read_to_string(&gpath).map_err(|e| io_err(&gpath, e))?;
            let v: Value = serde_json::from_str(&txt)
                .map_err(|e| refused(format!("{}: {e}", gpath.display())))?;
            let supplied = match fmt {
                GroupFormat::EngineRecord => SymGroup::from_engine_record(&v),
                _ => SymGroup::from_json(&v),
            }
            .map_err(|m| refused(format!("{}: {m}", gpath.display())))?;
            if supplied.chart != chart {
                return Err(refused(format!(
                    "the supplied group's chart {} is not the frame's",
                    supplied.chart_text()
                )));
            }
            let source = gpath.display().to_string();
            if fmt == GroupFormat::Generators {
                verify_generators(&family, chart, &supplied, &source, &opts).map_err(refused)?
            } else {
                verify_cross_checked(&family, &supplied, &source, &opts)
                    .map_err(|e| refused(format!("{e:?}")))?
            }
        }
    };
    report.elements = vg.group().elements.len();
    report.provenance = format!("{:?}", vg.provenance());
    report.digest = vg.digest().to_string();
    report.family_digest = vg.family_digest().to_string();
    Ok((vg, report, family))
}

/// INDEPENDENT VERIFICATION of one `sym` record (interfaces/coord-sym-records.md section 5, the seven steps;
/// interfaces/import.md 18.4) from the family and the record alone, plus membership in OUR derived group:
/// 1. the family's momenta (the presented `family`);
/// 2. the record's `matrix`: q -> +-q and |det| = 1 of the loop block (exact integers);
/// 3. every slot that can be non-zero on the case (fixed non-zero, or free) has an action `(t, flip)`, no flip,
///    `chart[i] == chart[t]`, targets distinct, and `c_i T = c_t` exactly (the element is proved by
///    `replay_base::symrows::verify_group`, which recomputes every stated image from the momenta);
/// 4. every slot without an action is FIXED at 0 on the case;
/// 5. hence the integrand of I(a) maps factor by factor onto that of I(g a) (unit Jacobian): I(a) = I(g a), sign `+`;
/// 6. the `image` pattern is the target moved along the action (power of i goes to t; other slots 0), and the
///    `target` pattern is the case's (fixed values, `n<i>` on free slots);
/// plus: the action on the slots non-zero on the case equals that of an element of `group` (our derived group).
/// Step 7 (descent) is checked at every key the record serves (the closure's descent check; import.md 18.4).
/// `Ok(element index in group)` or the first failed step.
pub fn verify_sym_record(
    rec: &crate::record::SymRecord,
    family: &Family,
    group: &VerifiedGroup,
    chart: &[bool],
) -> Result<usize, String> {
    use crate::replay_base::symrows::{SlotImage, SymElement};
    let n = chart.len();
    if rec.fixed.len() != n || rec.action.len() != n {
        return Err(format!("step 1: record of {} slots for arity {n}", rec.fixed.len()));
    }
    let chart_text: String = chart.iter().map(|&b| if b { '1' } else { '0' }).collect();
    if rec.chart != chart_text {
        return Err(format!("chart {} is not the frame's {chart_text}", rec.chart));
    }
    if rec.sign != "+" {
        return Err(format!(
            "step 5: sign {:?} (squared propagators never flip: the sign is +)",
            rec.sign
        ));
    }
    // step 4 / 3: the slots that can be non-zero on the case
    let mut hit = vec![false; n];
    for i in 0..n {
        let nonzero = rec.fixed[i] != Some(0);
        match rec.action[i] {
            None if nonzero => {
                return Err(format!(
                    "step 4: slot {i} has no action but is not fixed at 0 on the case"
                ));
            }
            None => {}
            Some((t, flip)) => {
                if t >= n {
                    return Err(format!("step 3: slot {i} -> {t} beyond arity {n}"));
                }
                if flip {
                    return Err(format!(
                        "step 3: slot {i} flips (a squared propagator cannot change sign)"
                    ));
                }
                if chart[i] != chart[t] {
                    return Err(format!("step 3: slot {i} -> {t} mixes a line and a numerator"));
                }
                if std::mem::replace(&mut hit[t], true) {
                    return Err(format!("step 3: target slot {t} hit twice"));
                }
            }
        }
    }
    // step 2 / 3 / 5: the record's own momentum map, proved exactly from the family (every stated image recomputed)
    let w = family.loops() + 1;
    if rec.matrix.len() != w * w {
        return Err(format!(
            "step 2: matrix of {} entries, (L+1)^2 = {}",
            rec.matrix.len(),
            w * w
        ));
    }
    let action: Vec<SlotImage> = rec
        .action
        .iter()
        .enumerate()
        .map(|(i, a)| match a {
            // only the slots non-zero on the case are stated: the identity needs no more (step 4)
            Some((t, _)) if rec.fixed[i] != Some(0) => SlotImage::Exact {
                target: *t,
                flip: false,
            },
            _ => SlotImage::Unknown,
        })
        .collect();
    let element = SymElement {
        matrix: Some(rec.matrix.clone()),
        action: action.clone(),
    };
    let one = crate::replay_base::symrows::SymGroup {
        arity: n,
        loops: family.loops(),
        chart: chart.to_vec(),
        elements: vec![element],
    };
    crate::replay_base::symrows::verify_group(family, one, "sym record")
        .map_err(|e| format!("steps 2-3: {}", e.reason))?;
    // step 6: target and image patterns
    let pat = |t: &str| -> Result<Vec<String>, String> {
        let inner = t
            .trim()
            .strip_prefix("I(")
            .and_then(|x| x.strip_suffix(')'))
            .ok_or_else(|| format!("step 6: pattern {t:?}"))?;
        let v: Vec<String> = inner
            .split(',')
            .map(|x| x.chars().filter(|c| !c.is_whitespace()).collect())
            .collect();
        if v.len() != n {
            return Err(format!("step 6: pattern {t:?} of {} slots", v.len()));
        }
        Ok(v)
    };
    let target = pat(&rec.target)?;
    let image = pat(&rec.image)?;
    for i in 0..n {
        let want = match rec.fixed[i] {
            Some(v) => v.to_string(),
            None => format!("n{i}"),
        };
        if target[i] != want {
            return Err(format!(
                "step 6: target slot {i} is {:?}, the case says {want:?}",
                target[i]
            ));
        }
    }
    let mut want_image = vec!["0".to_string(); n];
    for i in 0..n {
        if let Some((t, _)) = rec.action[i] {
            want_image[t] = target[i].clone();
        }
    }
    if image != want_image {
        return Err(format!(
            "step 6: image {} is not the target moved along the action (I({}))",
            rec.image,
            want_image.join(",")
        ));
    }
    // OUR derived group: an element with the same action on the slots that can be non-zero on the case
    let same = |e: &SymElement| -> bool {
        (0..n).all(|i| match &action[i] {
            SlotImage::Unknown => true,
            SlotImage::Exact { target, .. } => match &e.action[i] {
                SlotImage::Exact { target: t2, flip } => t2 == target && !flip,
                SlotImage::Linear { terms, constant } => {
                    constant.n == 0
                        && terms.len() == 1
                        && terms[0].0 == *target
                        && terms[0].1.n == terms[0].1.d
                }
                SlotImage::Unknown => false,
            },
            SlotImage::Linear { .. } => false,
        })
    };
    let identity = (0..n).all(|i| match &action[i] {
        SlotImage::Exact { target, .. } => *target == i,
        _ => true,
    });
    if identity {
        return Err("the identity element (I(a) = I(a): no descent)".into());
    }
    group
        .group()
        .elements
        .iter()
        .position(same)
        .ok_or_else(|| "the action is not that of an element of the derived group".to_string())
}

/// GENERATORS: derive + verify the whole group from the family, then require every generator to be an element of it
/// (same line permutation, every slot image it states equal) and the generated group (closure of the generators' line
/// permutations under composition) to equal the derived group's line permutations.
fn verify_generators(
    family: &Family,
    chart: &[bool],
    gens: &SymGroup,
    source: &str,
    opts: &DeriveOptions,
) -> Result<VerifiedGroup, String> {
    let vg = verify_derived(family, chart, opts).map_err(|e| format!("{e:?}"))?;
    let derived = vg.group();
    let lines: Vec<usize> = (0..chart.len()).filter(|&i| chart[i]).collect();
    let perm_of = |e: &crate::replay_base::symrows::SymElement| -> Result<Vec<usize>, String> {
        e.line_perm(chart)
            .ok_or_else(|| format!("{source}: an element without exact line images"))
    };
    let mut by_perm: BTreeMap<Vec<usize>, Vec<&crate::replay_base::symrows::SymElement>> =
        BTreeMap::new();
    for e in &derived.elements {
        by_perm.entry(perm_of(e)?).or_default().push(e);
    }
    let mut gen_perms = vec![];
    for (k, g) in gens.elements.iter().enumerate() {
        let p = perm_of(g)?;
        let cands = by_perm.get(&p).ok_or_else(|| {
            format!("{source}: generator {k} (line images {p:?}) is not in the derived group")
        })?;
        // every image the generator states must equal the derived element's (Exact onto t == Linear 1*D_t)
        let ok = cands.iter().any(|d| {
            g.action.iter().zip(&d.action).all(|(a, b)| {
                use crate::replay_base::symrows::SlotImage as S;
                match (a, b) {
                    (S::Unknown, _) => true,
                    (x, y) if x == y => true,
                    (
                        S::Exact {
                            target,
                            flip: false,
                        },
                        S::Linear { terms, constant },
                    )
                    | (
                        S::Linear { terms, constant },
                        S::Exact {
                            target,
                            flip: false,
                        },
                    ) => {
                        constant.n == 0
                            && terms.len() == 1
                            && terms[0].0 == *target
                            && terms[0].1.n == terms[0].1.d
                    }
                    _ => false,
                }
            })
        });
        if !ok {
            return Err(format!(
                "{source}: generator {k} states a slot image that differs from the derived element's"
            ));
        }
        gen_perms.push(p);
    }
    // closure of the generators' line permutations (as maps line slot -> line slot)
    let pos: BTreeMap<usize, usize> = lines.iter().enumerate().map(|(i, &s)| (s, i)).collect();
    let compose = |a: &[usize], b: &[usize]| -> Vec<usize> {
        // (a o b)[i] = a[pos[b[i]]]
        b.iter().map(|t| a[pos[t]]).collect()
    };
    let id: Vec<usize> = lines.clone();
    let mut group: BTreeSet<Vec<usize>> = BTreeSet::from([id]);
    let mut frontier: Vec<Vec<usize>> = group.iter().cloned().collect();
    while let Some(x) = frontier.pop() {
        for g in &gen_perms {
            let y = compose(g, &x);
            if group.insert(y.clone()) {
                frontier.push(y);
            }
        }
    }
    let mut derived_perms: BTreeSet<Vec<usize>> = by_perm.keys().cloned().collect();
    derived_perms.insert(lines.clone());
    if group != derived_perms {
        return Err(format!(
            "{source}: the generators generate {} line permutations, the derived group has {}",
            group.len(),
            derived_perms.len()
        ));
    }
    Ok(vg)
}

/// Verdict of one supplied relation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum RelationCheck {
    /// The relation is an exact rational combination of the symmetry rows at the seeds.
    Verified {
        rows: usize,
        rank: usize,
        columns: usize,
    },
    /// Not in the row span of these rows (a statement about the rows at these seeds).
    NotInSpan {
        rows: usize,
        rank: usize,
        rank_with_relation: usize,
    },
    /// A coefficient text the algebra cannot read, a row expansion over its bound, an overflow.
    Unknown(String),
}

impl RelationCheck {
    pub fn verified(&self) -> bool {
        matches!(self, RelationCheck::Verified { .. })
    }
}

/// Verify `I(target) = sum c_j I(rhs_j)` against the symmetry rows of `group` at `seeds` (default: the target),
/// exactly, with the rational arithmetic of `alg`: the relation vector `e_target - sum c_j e_{rhs_j}` must lie in the
/// row space (rank of the rows equal to the rank of the rows plus the relation, the relation vector nonzero).
pub fn verify_relation_with<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    target: &[i32],
    rhs: &[(Vec<i32>, String)],
    seeds: &[Vec<i32>],
    max_terms: usize,
) -> RelationCheck {
    let unknown = |m: String| RelationCheck::Unknown(m);
    let mut rel: BTreeMap<Vec<i64>, Q> = BTreeMap::new();
    let tkey: Vec<i64> = target.iter().map(|&x| x as i64).collect();
    let add = |m: &mut BTreeMap<Vec<i64>, Q>, k: Vec<i64>, c: Q| -> Result<(), String> {
        let cur = m.remove(&k);
        let v = match cur {
            None => c,
            Some(old) => alg.q_add(old, c).map_err(|e| format!("{e:?}"))?,
        };
        if v.n != 0 {
            m.insert(k, v);
        }
        Ok(())
    };
    let one = match alg.q_new(1, 1) {
        Ok(q) => q,
        Err(e) => return unknown(format!("{e:?}")),
    };
    if let Err(e) = add(&mut rel, tkey.clone(), one) {
        return unknown(e);
    }
    for (k, c) in rhs {
        let q = match alg.q_parse(c.trim()) {
            Ok(q) => q,
            Err(e) => return unknown(format!("coefficient {c:?}: {e:?}")),
        };
        let nq = match alg.q_neg(q) {
            Ok(q) => q,
            Err(e) => return unknown(format!("{e:?}")),
        };
        if let Err(e) = add(&mut rel, k.iter().map(|&x| x as i64).collect(), nq) {
            return unknown(e);
        }
    }
    if rel.is_empty() {
        return unknown("the relation vector is zero".into());
    }
    let seeds: Vec<Vec<i32>> = if seeds.is_empty() {
        vec![target.to_vec()]
    } else {
        seeds.to_vec()
    };
    let mut rows: Vec<BTreeMap<Vec<i64>, Q>> = vec![];
    for s in &seeds {
        let pk = (0u64, s.iter().map(|&x| x as i64).collect::<Vec<i64>>());
        match rows_at_with(alg, group, &pk, SymRowMode::Expanded, max_terms) {
            Ok(rs) => {
                for r in rs {
                    let mut m = BTreeMap::new();
                    for ((mask, k), c) in r.terms {
                        if mask != 0 {
                            return unknown("a symbolic row term at an integer seed".into());
                        }
                        if let Err(e) = add(&mut m, k, c) {
                            return unknown(e);
                        }
                    }
                    if !m.is_empty() {
                        rows.push(m);
                    }
                }
            }
            Err(e) => return unknown(format!("rows at {s:?}: {e}")),
        }
    }
    // columns: every key of the rows and of the relation
    let mut cols: BTreeSet<Vec<i64>> = rel.keys().cloned().collect();
    for r in &rows {
        cols.extend(r.keys().cloned());
    }
    let index: BTreeMap<&Vec<i64>, usize> = cols.iter().enumerate().map(|(i, k)| (k, i)).collect();
    let zero = match alg.q_new(0, 1) {
        Ok(q) => q,
        Err(e) => return unknown(format!("{e:?}")),
    };
    let dense = |m: &BTreeMap<Vec<i64>, Q>| -> Vec<Q> {
        let mut v = vec![zero; cols.len()];
        for (k, c) in m {
            v[index[k]] = *c;
        }
        v
    };
    let mat: Vec<Vec<Q>> = rows.iter().map(dense).collect();
    let rank = match alg.q_rank(&mat) {
        Ok(r) => r,
        Err(e) => return unknown(format!("rank: {e:?}")),
    };
    let mut with = mat.clone();
    with.push(dense(&rel));
    let rank_with = match alg.q_rank(&with) {
        Ok(r) => r,
        Err(e) => return unknown(format!("rank: {e:?}")),
    };
    if rank_with == rank {
        RelationCheck::Verified {
            rows: mat.len(),
            rank,
            columns: cols.len(),
        }
    } else {
        RelationCheck::NotInSpan {
            rows: mat.len(),
            rank,
            rank_with_relation: rank_with,
        }
    }
}
