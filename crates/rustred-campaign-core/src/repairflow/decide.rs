//! The node-level decision of the repair flow (interfaces/repairflow.md sections 0, 3): which faces get a symbolic
//! attempt, the judge of every answer, the NODE BUDGET at a barrier (deterministic order), and the recipe escalation
//! of every face. Pure: same faces + same answers + same policy -> the same decisions (U-1), in any completion order.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::faces::{FaceGroup, Key, ShadowClass};
use super::judge::{Class, Judgement, Measures, judge, without_candidate};
use super::policy::RepairPolicy;

pub const DECISIONS_SCHEMA: &str = "rustred.campaign.repairflow-decisions.v1";

/// Which faces get a step-S job: symbolic on, not blocked by the shadowing test, within `symbolic.max_faces` (counted
/// over the searchable faces in face order). Returns one flag per face, and the class of the faces left out.
pub fn s_plan(faces: &[FaceGroup], shadow: &[ShadowClass], p: &RepairPolicy) -> Vec<Option<Class>> {
    let mut n = 0usize;
    faces
        .iter()
        .zip(shadow)
        .map(|(f, sh)| {
            if !p.enabled || !p.symbolic.enabled {
                return Some(Class::SymbolicOff);
            }
            match sh {
                ShadowClass::Shadowed { .. } => return Some(Class::Shadowed),
                ShadowClass::EmptyDomain { .. } => return Some(Class::EmptyDomain),
                _ => {}
            }
            let plain = f.exception.is_none();
            if plain
                && (p.faces.plain_faces == "none"
                    || (p.faces.plain_faces == "unattributed" && f.unattributed == 0))
            {
                return Some(Class::PlainFaceExplained);
            }
            if !plain && !p.faces.exception_faces {
                return Some(Class::SymbolicOff);
            }
            if p.symbolic.max_faces.is_some_and(|m| n >= m) {
                return Some(Class::FaceCap);
            }
            n += 1;
            None
        })
        .collect()
}

/// THE ordering among several candidates of one face (pluggable, policy data `keep.choice_order`): `keep` first,
/// then each criterion in order (`regular_at` is reserved and compares equal until the fixed-d regularity data exists);
/// the record digest always ends the order, so the choice is total and deterministic.
pub fn compare_candidates(
    a: (&Judgement, &CandidateAnswer),
    b: (&Judgement, &CandidateAnswer),
    order: &[String],
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (ja, ca) = a;
    let (jb, cb) = b;
    let ka = ja.class == Class::Keep;
    let kb = jb.class == Class::Keep;
    let mut o = kb.cmp(&ka);
    for c in order {
        if o != Ordering::Equal {
            return o;
        }
        o = match c.as_str() {
            "served_desc" => cb.served.len().cmp(&ca.served.len()),
            "bytes_asc" => ca.measures.bytes.cmp(&cb.measures.bytes),
            "rhs_terms_asc" => ca.measures.rhs_terms.cmp(&cb.measures.rhs_terms),
            "monomials_asc" => ca.measures.monomials.cmp(&cb.measures.monomials),
            "record_digest" => ca.record_sha256.cmp(&cb.record_sha256),
            // "regular_at": reserved (no regularity data yet)
            _ => Ordering::Equal,
        };
    }
    o.then(ca.record_sha256.cmp(&cb.record_sha256))
}

/// One candidate rule of a settled step-S job.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CandidateAnswer {
    /// sha256 (hex) of the record line (no newline) and the line itself (kept verbatim for the re-homed table).
    pub record_sha256: String,
    pub line: String,
    pub measures: Measures,
    /// The face's open keys where the candidate's domain applies.
    pub served: Vec<Key>,
}

/// What the step-S job of a face returned.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum FaceAnswer {
    Candidates {
        job: String,
        candidates: Vec<CandidateAnswer>,
    },
    NoCandidate {
        job: String,
        why: String,
    },
    FrameMismatch {
        job: String,
        why: String,
    },
    DomainError {
        job: String,
        why: String,
    },
}

impl FaceAnswer {
    pub fn job(&self) -> &str {
        match self {
            FaceAnswer::Candidates { job, .. }
            | FaceAnswer::NoCandidate { job, .. }
            | FaceAnswer::FrameMismatch { job, .. }
            | FaceAnswer::DomainError { job, .. } => job,
        }
    }
}

/// The decision of one face.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceDecision {
    pub face: String,
    pub sd: Vec<usize>,
    pub sn: Vec<usize>,
    pub open_keys: u64,
    pub shadow: ShadowClass,
    pub job: Option<String>,
    pub symbolic: Judgement,
    /// The kept candidate (after the node budget): its record digest.
    pub kept: Option<String>,
    /// Open keys served by the kept candidate.
    pub served_keys: Vec<Key>,
    /// The recipe escalation given to the face (empty: none; the default recipe still applies).
    pub recipe_steps: Vec<String>,
    pub recipe_note: String,
    /// The exception face (rule, branch, parametric case) when this is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exception: Option<super::faces::ExceptionRef>,
    /// Observed keys per layer (D, r) with the signature capacity.
    #[serde(default)]
    pub layers: Vec<Value>,
    /// The kept rule's domain as written (fixed, equations, exceptions): what it covers for every power.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_covered: Option<Value>,
}

/// The node's decisions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeDecisions {
    pub schema: String,
    pub node: String,
    pub policy_digest: String,
    pub faces: Vec<FaceDecision>,
    /// Totals: kept rules, their bytes, faces by class.
    pub kept_rules: u64,
    pub kept_bytes: u64,
    pub by_class: BTreeMap<String, u64>,
    /// Open keys outside the chart orthant (never searched here).
    pub outside_chart: Vec<Key>,
}

fn class_name(c: &Class) -> String {
    match serde_json::to_value(c) {
        Ok(v) => v["class"].as_str().unwrap_or("?").to_string(),
        Err(_) => "?".into(),
    }
}

/// Decide the node: `answers` by face label (faces with an S job); faces without an answer but planned are
/// `no_candidate: missing` (the caller only decides at the barrier, when every S item is final).
pub fn decide(
    node: &str,
    faces: &[FaceGroup],
    shadow: &[ShadowClass],
    answers: &BTreeMap<String, FaceAnswer>,
    outside_chart: &[Key],
    p: &RepairPolicy,
) -> NodeDecisions {
    let plan = s_plan(faces, shadow, p);
    let mut out: Vec<FaceDecision> = Vec::with_capacity(faces.len());
    // (face index, candidate index) of the judge's keep candidates, for the budget barrier
    let mut keep: Vec<(usize, CandidateAnswer)> = vec![];
    for (i, f) in faces.iter().enumerate() {
        let open = f.open_keys.len() as u64;
        let (job, j) = match &plan[i] {
            Some(c) => (None, without_candidate(c.clone(), open)),
            None => match answers.get(&f.label) {
                None => (
                    None,
                    without_candidate(
                        Class::NoCandidate {
                            why: "missing answer".into(),
                        },
                        open,
                    ),
                ),
                Some(a) => {
                    let job = Some(a.job().to_string());
                    let j = match a {
                        FaceAnswer::NoCandidate { why, .. } => {
                            without_candidate(Class::NoCandidate { why: why.clone() }, open)
                        }
                        FaceAnswer::FrameMismatch { why, .. } => {
                            without_candidate(Class::FrameMismatch { why: why.clone() }, open)
                        }
                        FaceAnswer::DomainError { why, .. } => {
                            without_candidate(Class::DomainError { why: why.clone() }, open)
                        }
                        FaceAnswer::Candidates { candidates, .. } if candidates.is_empty() => {
                            without_candidate(
                                Class::NoCandidate {
                                    why: "no rule record".into(),
                                },
                                open,
                            )
                        }
                        FaceAnswer::Candidates { candidates, .. } => {
                            // judge every candidate; the best KEEP by (served desc, bytes asc, digest); else the first
                            let mut js: Vec<(Judgement, &CandidateAnswer)> = candidates
                                .iter()
                                .map(|c| (judge(&c.measures, c.served.len() as u64, open, p), c))
                                .collect();
                            js.sort_by(|(ja, ca), (jb, cb)| {
                                compare_candidates((ja, ca), (jb, cb), &p.keep.choice_order)
                            });
                            let (j, c) = js.swap_remove(0);
                            if j.class == Class::Keep {
                                keep.push((i, c.clone()));
                            }
                            j
                        }
                    };
                    (job, j)
                }
            },
        };
        out.push(FaceDecision {
            face: f.label.clone(),
            sd: f.sd.clone(),
            sn: f.sn.clone(),
            open_keys: open,
            shadow: shadow[i].clone(),
            job,
            symbolic: j,
            kept: None,
            served_keys: vec![],
            recipe_steps: vec![],
            recipe_note: String::new(),
            exception: f.exception.clone(),
            layers: vec![],
            domain_covered: None,
        });
    }
    // NODE BUDGET (barrier): deterministic order over the keep candidates
    let b = &p.node_budget;
    keep.sort_by(|(ia, a), (ib, b2)| {
        let la = &faces[*ia].label;
        let lb = &faces[*ib].label;
        let (sa, sb) = (a.served.len() as u128, b2.served.len() as u128);
        let (ba, bb) = (
            a.measures.bytes.max(1) as u128,
            b2.measures.bytes.max(1) as u128,
        );
        let ord = match b.order.as_str() {
            "keys" => sb.cmp(&sa),
            "bytes_asc" => ba.cmp(&bb),
            // keys_per_byte desc: sa/ba > sb/bb  <=>  sa*bb > sb*ba
            _ => (sb * ba).cmp(&(sa * bb)),
        };
        ord.then(ba.cmp(&bb)).then(la.cmp(lb))
    });
    let (mut run_b, mut run_r) = (0u64, 0u64);
    for (i, c) in keep {
        let fits = run_b.saturating_add(c.measures.bytes) <= b.max_bytes && run_r < b.max_rules;
        if fits {
            run_b += c.measures.bytes;
            run_r += 1;
            out[i].kept = Some(c.record_sha256.clone());
            out[i].served_keys = c.served.clone();
            out[i].domain_covered = serde_json::from_str::<Value>(&c.line).ok().map(|v| {
                json!({"fixed": v["fixed"], "equations": v["equations"], "exceptions": v["exceptions"],
                       "target": v["target"], "free_dims": c.measures.free_dims})
            });
        } else {
            out[i].symbolic.class = Class::OverNodeBudget {
                running_bytes: run_b,
                running_rules: run_r,
                max_bytes: b.max_bytes,
                max_rules: b.max_rules,
            };
        }
    }
    // RECIPES: every face within recipes.max_faces gets the escalation (also faces with a kept rule: its exception
    // keys and keys first met later on the face fall through to the face recipes)
    let mut given = 0usize;
    for d in &mut out {
        if !p.enabled || !p.recipes.enabled {
            d.recipe_note = "recipes off by policy".into();
            continue;
        }
        if !p.recipes.per_face {
            d.recipe_note = "per-face recipes off: default recipe only".into();
            continue;
        }
        if d.exception.is_some() {
            d.recipe_note =
                "exception face: its keys fall to their plain faces' recipes (pointwise fallback)"
                    .into();
            continue;
        }
        if p.recipes.max_faces.is_some_and(|m| given >= m) {
            d.recipe_note = "beyond recipes.max_faces: default recipe only".into();
            continue;
        }
        given += 1;
        d.recipe_steps = p.recipes.escalation.clone();
        d.recipe_note = if d.kept.is_some() {
            "after the kept rule (its exception keys / later keys of the face)".into()
        } else {
            "symbolic not kept".into()
        };
    }
    let mut by_class: BTreeMap<String, u64> = BTreeMap::new();
    for d in &out {
        *by_class.entry(class_name(&d.symbolic.class)).or_default() += 1;
    }
    NodeDecisions {
        schema: DECISIONS_SCHEMA.into(),
        node: node.into(),
        policy_digest: p.digest(),
        kept_rules: run_r,
        kept_bytes: run_b,
        faces: out,
        by_class,
        outside_chart: outside_chart.to_vec(),
    }
}

/// The import sources of the extension (inserted before the closing loop's default recipe): the kept table first
/// (records in face order), then every face's recipe steps in order. `recipe_files` maps (face label, step) -> path.
pub fn extension_sources(
    d: &NodeDecisions,
    table_path: Option<&str>,
    recipe_files: &BTreeMap<(String, String), String>,
) -> Vec<Value> {
    let mut v = vec![];
    if let Some(t) = table_path
        && d.kept_rules > 0
    {
        v.push(
            json!({"id": "rf:symbolic", "kind": "engine_table", "class": "repair-symbolic",
                      "path": t, "hand_name": {"template": "rf:symbolic#{ordinal}"}}),
        );
    }
    for f in &d.faces {
        for s in &f.recipe_steps {
            if let Some(p) = recipe_files.get(&(f.face.clone(), s.clone())) {
                let tag = super::recipes::face_tag(&crate::worklist::items::Face::new(
                    f.sd.clone(),
                    f.sn.clone(),
                ));
                v.push(json!({"id": format!("rf:{tag}:{s}"), "kind": "recipe",
                              "class": "recipe-face", "path": p}));
            }
        }
    }
    v
}
