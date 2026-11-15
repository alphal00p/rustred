//! The planner's input from the repair flow's per-key record of all acceptable choices (lane repairflow2,
//! `repairflow::composite::KeyChoices`, schema `rustred.campaign.repairflow-keychoices.v1`, one JSON line per open key):
//!
//! `{"schema", "key", "face", "face_slots", "choices": [{"id", "kind": "plain"|"composite"|"two_row"|"core"|..,
//!   "element", "form", "rhs": [[..],..], "records", "key_solved", "regular_at_4"}], "refused", "preferred"}`
//!
//! Read here with a serde mirror (unknown fields ignored) so this module does not depend on the producer's module.
//! The mapping of kind words to [`ServiceKind`] is DATA ([`KeyChoicesOptions::kinds`]); an unknown word is an error.
//! A choice of a symmetry kind whose right-hand side is exactly ONE key of the open set is a HAND-DOWN (TRICKS 4 / 14b:
//! a one-term image strictly lower onto an open key, of another face or of the same face's core). Right-hand sides are
//! the terms of exact relations (zero terms already dropped by the producer): every term is an edge.

use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::plan::{Candidate, PlanInput, ServiceKind, UnservedKey};
use super::{NonZero, Term};

pub const KEYCHOICES_SCHEMA: &str = "rustred.campaign.repairflow-keychoices.v1";

/// One choice of a key (mirror of the producer's record).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceRecord {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub element: Option<i64>,
    #[serde(default)]
    pub form: String,
    #[serde(default)]
    pub rhs: Vec<Vec<i32>>,
    #[serde(default)]
    pub records: Vec<String>,
    #[serde(default)]
    pub key_solved: bool,
    #[serde(default)]
    pub regular_at_4: Option<bool>,
}

/// The record of one open key (mirror of the producer's `KeyChoices`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyChoicesRecord {
    #[serde(default)]
    pub schema: String,
    pub key: Vec<i32>,
    pub face: String,
    #[serde(default)]
    pub face_slots: Vec<usize>,
    #[serde(default)]
    pub choices: Vec<ChoiceRecord>,
    #[serde(default)]
    pub refused: Vec<serde_json::Value>,
    #[serde(default)]
    pub preferred: Option<usize>,
    #[serde(default)]
    pub status: Option<String>,
}

/// How choices become candidates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyChoicesOptions {
    /// Kind word -> service kind.
    pub kinds: BTreeMap<String, ServiceKind>,
    /// Kind words of symmetry services: one of them with exactly one rhs key in the open set is a hand-down.
    pub symmetry_kinds: Vec<String>,
}

impl Default for KeyChoicesOptions {
    fn default() -> Self {
        let kinds = [
            ("plain", ServiceKind::Composite),
            ("composite", ServiceKind::Composite),
            ("two_row", ServiceKind::TwoRow),
            ("hand_down", ServiceKind::HandDown),
            ("core", ServiceKind::CoreRecipe),
            ("table_rule", ServiceKind::TableRule),
        ]
        .into_iter()
        .map(|(w, k)| (w.to_string(), k))
        .collect();
        KeyChoicesOptions {
            kinds,
            symmetry_kinds: vec!["plain".into(), "composite".into(), "two_row".into()],
        }
    }
}

/// Reads a KeyChoices JSONL file (a line with another `schema` is an error; empty lines are skipped).
pub fn read_key_choices(path: &Path) -> Result<Vec<KeyChoicesRecord>, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = vec![];
    for (i, l) in std::io::BufReader::with_capacity(1 << 20, f)
        .lines()
        .enumerate()
    {
        let l = l.map_err(|e| format!("{}: {e}", path.display()))?;
        if l.trim().is_empty() {
            continue;
        }
        let r: KeyChoicesRecord = serde_json::from_str(&l)
            .map_err(|e| format!("{} line {}: {e}", path.display(), i + 1))?;
        if !r.schema.is_empty() && r.schema != KEYCHOICES_SCHEMA {
            return Err(format!(
                "{} line {}: schema {} (expected {KEYCHOICES_SCHEMA})",
                path.display(),
                i + 1,
                r.schema
            ));
        }
        out.push(r);
    }
    Ok(out)
}

/// The planner input: every key of `open` (the unserved set of the run, all faces together) and every recorded key;
/// a key of `open` without a record gets the face `face_of(key)` and no candidate (it goes to the core).
pub fn plan_input_from_key_choices(
    arity: usize,
    records: &[KeyChoicesRecord],
    open: &BTreeSet<Vec<i32>>,
    face_of: &dyn Fn(&[i32]) -> String,
    opts: &KeyChoicesOptions,
) -> Result<PlanInput, String> {
    let mut keys: BTreeMap<Vec<i32>, UnservedKey> = BTreeMap::new();
    for r in records {
        let mut cands = Vec::with_capacity(r.choices.len());
        for c in &r.choices {
            let mut kind = *opts.kinds.get(&c.kind).ok_or_else(|| {
                format!(
                    "key {:?}: choice {} has the unknown kind {:?}",
                    r.key, c.id, c.kind
                )
            })?;
            if opts.symmetry_kinds.contains(&c.kind) && c.rhs.len() == 1 && open.contains(&c.rhs[0])
            {
                kind = ServiceKind::HandDown;
            }
            let prov = if c.form.is_empty() || c.form == c.id {
                c.id.clone()
            } else {
                format!("{} [{}]", c.id, c.form)
            };
            cands.push(Candidate {
                kind,
                terms: c
                    .rhs
                    .iter()
                    .map(|k| Term::new(k.clone(), NonZero::Yes))
                    .collect(),
                provenance: prov,
                regular_at_4: c.regular_at_4,
            });
        }
        let e = keys.entry(r.key.clone()).or_insert_with(|| UnservedKey {
            key: r.key.clone(),
            face: r.face.clone(),
            candidates: vec![],
        });
        if e.face != r.face {
            return Err(format!(
                "key {:?} recorded on faces {} and {}",
                r.key, e.face, r.face
            ));
        }
        e.candidates.extend(cands);
    }
    for k in open {
        keys.entry(k.clone()).or_insert_with(|| UnservedKey {
            key: k.clone(),
            face: face_of(k),
            candidates: vec![],
        });
    }
    Ok(PlanInput::new(arity, keys.into_values().collect()))
}
