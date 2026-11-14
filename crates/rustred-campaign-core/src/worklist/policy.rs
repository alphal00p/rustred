//! The seed-policy file `rustred.campaign.seed-policy.v1` (R37; interface `interfaces/seed-policy.md`): per node class
//! and face type the ordered settings of the seed-search rung, plus the evidence table. The rung takes the policy as
//! INPUT DATA. By default a policy REORDERS the built-in search space and never removes a setting (completeness rule);
//! a class with `replace_builtin: true` REPLACES it entirely (centres, floors, depths, order, budgets): nothing beyond
//! the built-in default DATA is hard-coded (audit A M7, rule L8).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::items::{CaseSpec, Face};

pub const SEED_POLICY_SCHEMA: &str = "rustred.campaign.seed-policy.v1";

/// Centre kinds of the policy file (expanded to instances on a case by [`super::seed::seed_space`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CentreSpec {
    Target,
    Line,
    Fixed,
    Num,
    Pair,
    Combined,
    /// Every off-floor NUMERATOR (free or fixed) one step towards its floor ("both numerators up", k1 w2 item 76).
    CombinedNum,
    UnionLines,
    UnionPairs,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingSpec {
    pub centre: CentreSpec,
    /// Default: true (normalised to false where the case has no face line, and for the target).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tight: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wave {
    pub depth: u8,
    pub settings: Vec<SettingSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassMatch {
    pub node_class: String,
    pub face_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBudgets {
    pub seconds_d2: u64,
    pub seconds_d3: u64,
    pub cap_seconds: u64,
    pub mem_mb_d2: u64,
    pub mem_mb_d3: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyClass {
    #[serde(rename = "match")]
    pub matcher: ClassMatch,
    pub waves: Vec<Wave>,
    #[serde(default)]
    pub tight: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budgets: Option<PolicyBudgets>,
    /// The class's waves ARE the search space (no built-in settings or depths appended). Default false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replace_builtin: bool,
}

/// Setting part of an evidence record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSetting {
    pub centre: CentreSpec,
    pub slots: Vec<usize>,
    pub floor: bool,
    pub depth: u8,
    #[serde(default)]
    pub tight: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub source: String,
    /// "reference" | "campaign"
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

/// One evidence record (output of every closed item; input as priors).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedEvidence {
    pub node: String,
    pub face: Face,
    pub case: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    pub setting: EvidenceSetting,
    pub rhs_terms: Option<u64>,
    pub rows: Option<u64>,
    pub monomials: Option<u64>,
    pub cpu_ms: Option<u64>,
    pub served: u64,
    pub gap_points: u64,
    pub landing_spread: Option<u64>,
    pub closure_size: Option<u64>,
    /// "closed_one_case" | "member_of_cover" | "no_relation" | "rejected"
    pub outcome: String,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedPolicy {
    pub schema: String,
    pub name: String,
    pub source: String,
    pub classes: Vec<PolicyClass>,
    #[serde(default)]
    pub evidence: Vec<SeedEvidence>,
}

fn s(centre: CentreSpec, floor: Option<bool>) -> SettingSpec {
    SettingSpec {
        centre,
        floor,
        tight: None,
    }
}

/// The built-in ordered list of one wave (today's evidence: R15 line floored first; R21 numerator / combined;
/// R26 unfloored; R29 (a) union of line centres; H s.2 fixed-slot centres of children).
pub fn builtin_wave(depth: u8) -> Wave {
    use CentreSpec::*;
    Wave {
        depth,
        settings: vec![
            s(Target, None),
            s(Line, Some(true)),
            s(Fixed, Some(true)),
            s(Num, Some(true)),
            s(Pair, Some(true)),
            s(Combined, Some(true)),
            s(CombinedNum, Some(true)),
            s(Line, Some(false)),
            s(Fixed, Some(false)),
            s(Num, Some(false)),
            s(Pair, Some(false)),
            s(Combined, Some(false)),
            s(CombinedNum, Some(false)),
            s(UnionLines, Some(true)),
            s(UnionLines, Some(false)),
            s(UnionPairs, Some(true)),
            s(UnionPairs, Some(false)),
        ],
    }
}

impl SeedPolicy {
    /// The built-in default policy (seed-policy.md section 4); evidence entries quote the reference runs.
    pub fn builtin() -> Self {
        SeedPolicy {
            schema: SEED_POLICY_SCHEMA.into(),
            name: "builtin-20261113".into(),
            source: "I-LADDER built-in default data (seed-policy.md section 4; tight seeds off)".into(),
            classes: vec![PolicyClass {
                matcher: ClassMatch {
                    node_class: "*".into(),
                    face_type: "*".into(),
                },
                waves: vec![builtin_wave(2), builtin_wave(3)],
                tight: false,
                budgets: None,
                replace_builtin: false,
            }],
            evidence: vec![],
        }
    }

    /// Read and validate a policy file's JSON text (unknown keys are errors; schema must match). A policy without a
    /// final `*`/`*` class gets the built-in one appended (reported in the second value).
    pub fn from_json(text: &str) -> Result<(Self, Vec<String>), String> {
        let mut p: SeedPolicy = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if p.schema != SEED_POLICY_SCHEMA {
            return Err(format!(
                "schema {} (expected {SEED_POLICY_SCHEMA})",
                p.schema
            ));
        }
        let mut notes = Vec::new();
        for c in &p.classes {
            for w in &c.waves {
                if w.depth == 0 {
                    return Err("wave depth 0 (shell depth must be >= 1)".to_string());
                }
            }
        }
        let has_star = p
            .classes
            .last()
            .is_some_and(|c| c.matcher.node_class == "*" && c.matcher.face_type == "*");
        if !has_star {
            p.classes.push(SeedPolicy::builtin().classes.remove(0));
            notes.push("no final */* class: built-in class appended".into());
        }
        Ok((p, notes))
    }

    /// The first class matching (node class, face type).
    pub fn class_for(&self, node_class: &str, face_type: &str) -> &PolicyClass {
        self.classes
            .iter()
            .find(|c| {
                (c.matcher.node_class == "*" || c.matcher.node_class == node_class)
                    && (c.matcher.face_type == "*" || c.matcher.face_type == face_type)
            })
            .unwrap_or_else(|| self.classes.last().expect("policy has a class"))
    }

    /// Mean RHS terms per (centre kind, floor, depth) of closing evidence for this face type (priors).
    pub fn priors(&self, face_type: &str) -> BTreeMap<(CentreSpec, bool, u8), f64> {
        let mut acc: BTreeMap<(CentreSpec, bool, u8), (f64, u32)> = BTreeMap::new();
        for e in &self.evidence {
            if face_type_of_face(&e.face) != face_type {
                continue;
            }
            if e.outcome != "closed_one_case" && e.outcome != "member_of_cover" {
                continue;
            }
            let Some(r) = e.rhs_terms else { continue };
            let k = (e.setting.centre, e.setting.floor, e.setting.depth);
            let a = acc.entry(k).or_insert((0.0, 0));
            a.0 += r as f64;
            a.1 += 1;
        }
        acc.into_iter()
            .map(|(k, (s, n))| (k, s / n as f64))
            .collect()
    }
}

/// Face type of a face (no fixed slots): nf1 / nf2 / nf3+ / num / line+num.
pub fn face_type_of_face(f: &Face) -> String {
    match (f.sd.len(), f.sn.len()) {
        (_, 0) if f.sd.len() <= 1 => "nf1".into(),
        (2, 0) => "nf2".into(),
        (_, 0) => "nf3+".into(),
        (0, _) => "num".into(),
        _ => "line+num".into(),
    }
}

/// Face type of a case: point (dim 0), child (fixed values or equations), else the face type.
pub fn face_type_of_case(c: &CaseSpec) -> String {
    if c.is_point() {
        "point".into()
    } else if !c.fixed.is_empty() || !c.equations.is_empty() {
        "child".into()
    } else {
        face_type_of_face(&c.face)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_round_trip_and_unknown_keys() {
        let p = SeedPolicy::builtin();
        let t = serde_json::to_string(&p).unwrap();
        let (q, notes) = SeedPolicy::from_json(&t).unwrap();
        assert_eq!(p, q);
        assert!(notes.is_empty());
        let bad = t.replace("\"source\"", "\"sauce\"");
        assert!(SeedPolicy::from_json(&bad).is_err());
    }
}
