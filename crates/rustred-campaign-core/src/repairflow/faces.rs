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

/// The PRESENTED case line of a face (cases input of the generate job): face slots free, the rest at the floor.
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
    json!({"fixed": fixed, "equations": []})
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
