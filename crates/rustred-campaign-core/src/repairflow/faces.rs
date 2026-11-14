//! Faces of the open keys, the shadowing test, and the step-S request of a face (interfaces/repairflow.md section 4).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::policy::RepairPolicy;
use crate::cover::RuleRef;
use crate::domain::CaseDomain;
use crate::key::NodeFrame;
use crate::loci::contain::{Containment, Emptiness, ProofBudget, is_empty_with, shadowed_with};
use crate::partition::DomainSource;
use crate::worklist::items::{CaseSpec, Face};

pub type Key = Vec<i32>;

/// The open keys of one face.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceGroup {
    pub label: String,
    pub sd: Vec<usize>,
    pub sn: Vec<usize>,
    /// Sorted ascending.
    pub open_keys: Vec<Key>,
    /// An EXCEPTION face (rule, branch) as a parametric domain; None = a plain face (dotted lines x numerators).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exception: Option<ExceptionRef>,
    /// Plain faces: open keys NOT attributed to a parametrisable exception face.
    #[serde(default)]
    pub unattributed: u64,
}

impl FaceGroup {
    pub fn face(&self) -> Face {
        Face::new(self.sd.clone(), self.sn.clone())
    }
}

/// Group open keys into faces (`Face::of_key`); keys outside the chart orthant are returned apart (they are never
/// searched: a hand-down / sub-sector key belongs to another node). Order: `face_order` of the policy, ties by label.
pub fn group_open_keys(
    keys: &[Key],
    lines: &[bool],
    face_order: &str,
) -> (Vec<FaceGroup>, Vec<Key>) {
    let mut by: BTreeMap<String, FaceGroup> = BTreeMap::new();
    let mut outside = vec![];
    for k in keys {
        match Face::of_key(k, lines) {
            Some(f) => {
                let label = f.label();
                by.entry(label.clone())
                    .or_insert_with(|| FaceGroup {
                        label,
                        sd: f.sd.clone(),
                        sn: f.sn.clone(),
                        open_keys: vec![],
                        exception: None,
                        unattributed: 0,
                    })
                    .open_keys
                    .push(k.clone());
            }
            None => outside.push(k.clone()),
        }
    }
    let mut v: Vec<FaceGroup> = by.into_values().collect();
    for g in &mut v {
        g.open_keys.sort();
        g.open_keys.dedup();
        g.unattributed = g.open_keys.len() as u64;
    }
    match face_order {
        "open_keys_asc" => v.sort_by(|a, b| {
            a.open_keys
                .len()
                .cmp(&b.open_keys.len())
                .then(a.label.cmp(&b.label))
        }),
        "label" => v.sort_by(|a, b| a.label.cmp(&b.label)),
        _ => v.sort_by(|a, b| {
            b.open_keys
                .len()
                .cmp(&a.open_keys.len())
                .then(a.label.cmp(&b.label))
        }),
    }
    outside.sort();
    (v, outside)
}

/// The PROPER face domain: face lines >= 2, face numerators <= -1 (bounds), every other slot at its floor.
pub fn face_domain(face: &Face, lines: &[bool]) -> Option<CaseDomain> {
    let n = lines.len();
    let mut d = CaseSpec::face_case(face.clone()).case_domain(lines)?;
    for &i in &face.sd {
        d.bounds
            .push(crate::poly::Poly::parse_d_free(&format!("n{i}-2"), n).ok()?);
    }
    for &j in &face.sn {
        d.bounds
            .push(crate::poly::Poly::parse_d_free(&format!("-n{j}-1"), n).ok()?);
    }
    Some(d)
}

/// Shadow verdict of a face (R31 evidence: test before search).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum ShadowClass {
    /// Some key of the face is not matched by an earlier record (search it).
    NotShadowed {
        witness: Option<Key>,
    },
    /// Every key of the proper face is MATCHED by an earlier record: a rule appended after them never fires.
    Shadowed {
        certificate: String,
    },
    EmptyDomain {
        certificate: String,
    },
    Unknown {
        why: String,
    },
    /// The test is off by policy.
    Skipped,
}

impl ShadowClass {
    /// The face must not be searched symbolically.
    pub fn blocks_search(&self) -> bool {
        matches!(
            self,
            ShadowClass::Shadowed { .. } | ShadowClass::EmptyDomain { .. }
        )
    }
}

/// The node's dispatch entries followed by one probe domain.
struct Probe<'a> {
    base: &'a dyn DomainSource,
    probe: CaseDomain,
}

impl DomainSource for Probe<'_> {
    fn arity(&self) -> usize {
        self.base.arity()
    }
    fn len(&self) -> usize {
        self.base.len() + 1
    }
    fn domain(&self, i: usize) -> &CaseDomain {
        if i == self.base.len() {
            &self.probe
        } else {
            self.base.domain(i)
        }
    }
    fn rule_ref(&self, i: usize) -> RuleRef {
        if i == self.base.len() {
            RuleRef::bare(usize::MAX, i, -1)
        } else {
            self.base.rule_ref(i)
        }
    }
    fn witness_block(&self, i: usize) -> Option<crate::domain::Verdict> {
        if i == self.base.len() {
            None
        } else {
            self.base.witness_block(i)
        }
    }
}

/// The shadowing test of a face against the dispatch entries in force (`base`, dispatch order).
pub fn shadow_face(
    face: &Face,
    base: &dyn DomainSource,
    frame: &NodeFrame,
    budget: &ProofBudget,
) -> ShadowClass {
    let Some(d) = face_domain(face, frame.mask()) else {
        return ShadowClass::Unknown {
            why: "face not expressible as a domain".into(),
        };
    };
    if let Emptiness::ProvedEmpty { certificate } = is_empty_with(&d, frame, budget) {
        return ShadowClass::EmptyDomain { certificate };
    }
    let p = Probe { base, probe: d };
    match shadowed_with(&p, base.len(), frame, budget) {
        Containment::Proved { certificate } => ShadowClass::Shadowed { certificate },
        Containment::NotContained { witness } => ShadowClass::NotShadowed {
            witness: Some(witness.to_vec()),
        },
        Containment::Unknown { reason } => ShadowClass::Unknown { why: reason },
    }
}

/// The PRESENTED case line of a face (cases input of the generate job, child-job.md 3.8.3: `frame` PRESENTED): face
/// slots free, the rest at the floor.
pub fn case_line(face: &Face, lines: &[bool]) -> Value {
    let slots = face.slots();
    let fixed: Vec<Value> = (0..lines.len())
        .map(|i| {
            if slots.contains(&i) {
                Value::Null
            } else {
                json!(if lines[i] { 1 } else { 0 })
            }
        })
        .collect();
    json!({"frame": "PRESENTED", "fixed": fixed, "equations": []})
}

/// The centre / floor arguments of the face search (`symbolic.centre`), with the priorities of the node order.
pub fn centre_args(
    face: &Face,
    centre: &str,
    line_priority: &[usize],
    num_priority: &[usize],
) -> Vec<String> {
    let lines_in: Vec<usize> = line_priority
        .iter()
        .copied()
        .filter(|i| face.sd.contains(i))
        .collect();
    let nums_in: Vec<usize> = num_priority
        .iter()
        .copied()
        .filter(|j| face.sn.contains(j))
        .collect();
    let line = |i: usize| format!("{i}:-1");
    let num = |j: usize| format!("{j}:1");
    let floor = |v: &mut Vec<String>| {
        v.push("--seed-face-floor".into());
        v.push("1".into());
    };
    let mut v = vec![];
    match centre {
        "target" => {}
        "union_floored" => {
            let mut slots: Vec<usize> = face.slots();
            slots.sort_unstable();
            if !slots.is_empty() {
                let c: Vec<String> = slots
                    .iter()
                    .map(|&s| {
                        if face.sd.contains(&s) {
                            line(s)
                        } else {
                            num(s)
                        }
                    })
                    .collect();
                v.push("--seed-centre-offset".into());
                v.push(c.join(";"));
                if !face.sd.is_empty() {
                    floor(&mut v);
                }
            }
        }
        "numerator" => {
            let c = nums_in
                .first()
                .map(|&j| num(j))
                .or(lines_in.last().map(|&i| line(i)));
            if let Some(c) = c {
                v.push("--seed-centre-offset".into());
                v.push(c);
            }
        }
        other => {
            let pick = if other == "first_line_floored" {
                lines_in.first()
            } else {
                lines_in.last()
            };
            match pick {
                Some(&i) => {
                    v.push("--seed-centre-offset".into());
                    v.push(line(i));
                    floor(&mut v);
                }
                None => {
                    if let Some(&j) = nums_in.first() {
                        v.push("--seed-centre-offset".into());
                        v.push(num(j));
                    }
                }
            }
        }
    }
    v
}

/// The generate arguments of the step-S job of a face (`--order` first; budgets go into the job spec: work.cases = 1,
/// wall.case_s = `symbolic.wall_case_s`).
pub fn s_args(
    face: &Face,
    policy: &RepairPolicy,
    order_json: &str,
    line_priority: &[usize],
    num_priority: &[usize],
) -> Vec<String> {
    let s = &policy.symbolic;
    let mut a = vec![
        "--order".to_string(),
        order_json.to_string(),
        "--max-depth".into(),
        s.max_depth.to_string(),
        "--numeric-depth".into(),
        s.numeric_depth.to_string(),
    ];
    a.extend(centre_args(face, &s.centre, line_priority, num_priority));
    a.extend([
        "--descend".into(),
        "open".into(),
        "--screen".into(),
        s.screen.arg(),
        "--on-heavy".into(),
        "refuse".into(),
        "--open-on-refusal".into(),
    ]);
    a.extend(s.extra_args.iter().cloned());
    a
}

// ------------------------------------------------------------------------------------------------------------------
// EXCEPTION FACES (coordinator ruling 2026-11-14: the primary object): the gaps of a catalogue are exception faces of
// its parametric rules. A key is ATTRIBUTED to the first dispatch entry whose domain puts it on an exception branch
// (`Verdict::OnException`); the face is (that rule, that branch) as a PARAMETRIC domain: the rule's case (fixed slots,
// equations) intersected with the branch's literals (all must vanish), unbounded directions included. Observed keys
// are only its evidence and priority.

/// The parametric exception face of a rule entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceptionRef {
    /// Dispatch entry index of the rule (in the source's order) and its provenance (filled by the caller).
    pub entry: usize,
    pub source: String,
    pub label: i64,
    pub record_sha256: String,
    pub branch: usize,
    /// The face as a PRESENTED case line (`frame`, `fixed`, `equations`): the rule's case + the branch literals.
    pub case: Value,
    /// Free directions of the case (symbolic slots minus independent equations, as written).
    pub free_dims: u64,
}

/// Affine polynomial -> engine text (`2*n6+n11-1`); None if not affine in the indices (or depends on d).
pub fn affine_text(p: &crate::poly::Poly, arity: usize) -> Option<String> {
    if p.has_d() || p.degree() > 1 {
        return None;
    }
    let zero = vec![0i32; arity];
    let c0 = p.eval_checked_i128(&zero).ok()?;
    let mut parts: Vec<(i128, String)> = vec![];
    for i in 0..arity {
        let mut e = zero.clone();
        e[i] = 1;
        let c = p.eval_checked_i128(&e).ok()? - c0;
        if c != 0 {
            parts.push((c, format!("n{i}")));
        }
    }
    let mut s = String::new();
    for (k, (c, v)) in parts.iter().enumerate() {
        let sign = if *c < 0 {
            "-"
        } else if k > 0 {
            "+"
        } else {
            ""
        };
        let a = c.abs();
        if a == 1 {
            s.push_str(&format!("{sign}{v}"));
        } else {
            s.push_str(&format!("{sign}{a}*{v}"));
        }
    }
    if c0 != 0 || s.is_empty() {
        s.push_str(&if c0 < 0 || s.is_empty() {
            format!("{c0}")
        } else {
            format!("+{c0}")
        });
    }
    Some(s)
}

/// The parametric case of (rule domain, branch): fixed slots of the rule, its equations and the branch literals
/// (a unit single-slot literal becomes a fixed value). Err: not expressible (pattern target, nonlinear literal, empty).
pub fn exception_case(d: &CaseDomain, branch: usize) -> Result<(Value, u64), String> {
    if d.target != crate::domain::TargetMap::Canonical {
        return Err("rule with a non-canonical target pattern".into());
    }
    let n = d.arity;
    let br = d.exceptions.get(branch).ok_or("no such branch")?;
    let mut fixed: Vec<Option<i32>> = d.fixed.clone();
    let mut eqs: Vec<String> = vec![];
    for p in d.equations.iter().chain(br.iter()) {
        let t = affine_text(p, n).ok_or_else(|| format!("nonlinear literal {p}"))?;
        // unit single-slot literal: c*n_i + k = 0 with |c| = 1 -> n_i = -k/c
        let zero = vec![0i32; n];
        let c0 = p.eval_checked_i128(&zero).map_err(|e| format!("{e:?}"))?;
        let mut nz = vec![];
        for i in 0..n {
            let mut e = zero.clone();
            e[i] = 1;
            let c = p.eval_checked_i128(&e).map_err(|e| format!("{e:?}"))? - c0;
            if c != 0 {
                nz.push((i, c));
            }
        }
        match nz.as_slice() {
            [] if c0 == 0 => {}
            [] => return Err("an inconsistent literal (non-zero constant)".into()),
            [(i, c)] if c.abs() == 1 => {
                let v = (-c0 * c) as i32;
                if let Some(f) = fixed[*i]
                    && f != v
                {
                    return Err("inconsistent fixed value".into());
                }
                fixed[*i] = Some(v);
            }
            _ => eqs.push(t),
        }
    }
    let free = fixed.iter().filter(|x| x.is_none()).count() as u64;
    let fdims = free.saturating_sub(eqs.len() as u64);
    let fv: Vec<Value> = fixed
        .iter()
        .map(|x| x.map(|v| json!(v)).unwrap_or(Value::Null))
        .collect();
    Ok((
        json!({"frame": "PRESENTED", "fixed": fv, "equations": eqs}),
        fdims,
    ))
}

/// The first dispatch entry (in order) that puts `key` on one of its exception branches: (entry, branch), within the
/// evaluation budget (`budget` counts domain evaluations and is decremented; None when exhausted or no entry).
pub fn attribute_exception(
    src: &dyn DomainSource,
    key: &[i32],
    budget: &mut u64,
) -> Option<(usize, usize)> {
    for i in 0..src.len() {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        if let crate::domain::Verdict::OnException { branch } = src.domain(i).applies(key) {
            return Some((i, branch));
        }
    }
    None
}

/// Attribution counters of [`exception_faces`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionStats {
    pub keys: u64,
    pub attributed: u64,
    /// Attributed to a face that is not expressible as a case (reason -> keys).
    pub not_parametric: BTreeMap<String, u64>,
    pub evaluations: u64,
    /// The evaluation budget ended before every key was examined.
    pub truncated: bool,
}

/// The EXCEPTION FACES of `keys` against the dispatch entries `src` (dispatch order): grouped by (entry, branch),
/// parametric case from the domain code; `prov(entry)` = (source, label, record_sha256). Returns the faces (largest
/// first, then label), the keys attributed to a PARAMETRIC face, and the counters. `lines` = the chart mask.
pub fn exception_faces(
    keys: &[Key],
    src: &dyn DomainSource,
    lines: &[bool],
    budget: u64,
    prov: &dyn Fn(usize) -> (String, i64, String),
) -> (
    Vec<FaceGroup>,
    std::collections::BTreeSet<Key>,
    AttributionStats,
) {
    let mut st = AttributionStats {
        keys: keys.len() as u64,
        ..Default::default()
    };
    let mut left = budget;
    let mut by: BTreeMap<(usize, usize), Vec<Key>> = BTreeMap::new();
    for k in keys {
        if left == 0 {
            st.truncated = true;
            break;
        }
        let before = left;
        let a = attribute_exception(src, k, &mut left);
        st.evaluations += before - left;
        if let Some(e) = a {
            by.entry(e).or_default().push(k.clone());
        } else if left == 0 {
            st.truncated = true;
        }
    }
    let mut faces = vec![];
    let mut attributed = std::collections::BTreeSet::new();
    for ((entry, branch), ks) in by {
        st.attributed += ks.len() as u64;
        match exception_case(src.domain(entry), branch) {
            Ok((case, free_dims)) => {
                let (source, label, sha) = prov(entry);
                let f0 = Face::of_key(&ks[0], lines).unwrap_or_else(|| Face::new(vec![], vec![]));
                for k in &ks {
                    attributed.insert(k.clone());
                }
                let mut ks = ks;
                ks.sort();
                faces.push(FaceGroup {
                    label: format!("x:{source}#{label}@b{branch}"),
                    sd: f0.sd.clone(),
                    sn: f0.sn.clone(),
                    open_keys: ks,
                    exception: Some(ExceptionRef {
                        entry,
                        source,
                        label,
                        record_sha256: sha,
                        branch,
                        case,
                        free_dims,
                    }),
                    unattributed: 0,
                });
            }
            Err(why) => *st.not_parametric.entry(why).or_default() += ks.len() as u64,
        }
    }
    faces.sort_by(|a, b| {
        b.open_keys
            .len()
            .cmp(&a.open_keys.len())
            .then(a.label.cmp(&b.label))
    });
    (faces, attributed, st)
}
