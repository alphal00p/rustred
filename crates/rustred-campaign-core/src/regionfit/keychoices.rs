//! Adapter from the repair flow's per-key record of ALL acceptable choices (`repairflow::composite::KeyChoices`,
//! schema `rustred.campaign.repairflow-keychoices.v1`) to [`FaceInput`]: one face per face label, one choice label per
//! acceptable choice. Keys with no choice enter with an EMPTY set (they block every region whose condition they meet).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{FaceInput, FaceKey};
use crate::repairflow::composite::{Choice, KEY_CHOICES_SCHEMA, KeyChoices};

/// Which identity of a choice the regions are fitted on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceLabel {
    /// The composed-rule form (`e3`, `e3+<source>#<label>,..`): element AND image rules fixed per region (default).
    #[default]
    Form,
    /// The group element only (`e3`); choices without an element fall back to their id.
    Element,
    /// The producer's id within the key (`sym:e3`, `tworow:e3:<shift>`, `core:<step>`).
    Id,
}

/// Options of the adapter (spec data).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeyChoicesOptions {
    pub label: ChoiceLabel,
    /// Drop choices the producer (lane poles) marked singular at d = 4 (`regular_at_4 == Some(false)`); choices not
    /// evaluated (None) are kept.
    pub exclude_singular_at_4: bool,
    /// Keep only these choice kinds (`plain`, `composite`, `two_row`, `core`, ...); None = all.
    pub kinds: Option<BTreeSet<String>>,
}

/// The label of one choice under `label`.
pub fn choice_label(c: &Choice, label: ChoiceLabel) -> String {
    match label {
        ChoiceLabel::Form => c.form.clone(),
        ChoiceLabel::Element => match c.element {
            Some(e) => format!("e{e}"),
            None => c.id.clone(),
        },
        ChoiceLabel::Id => c.id.clone(),
    }
}

fn kept(c: &Choice, o: &KeyChoicesOptions) -> bool {
    !(o.exclude_singular_at_4 && c.regular_at_4 == Some(false))
        && o.kinds.as_ref().is_none_or(|k| k.contains(&c.kind))
}

/// One [`FaceInput`] per face label (sorted by label). Refuses a record of another schema or a key listed twice.
pub fn faces_from_key_choices(
    records: &[KeyChoices],
    o: &KeyChoicesOptions,
) -> Result<Vec<FaceInput>, String> {
    let mut by: BTreeMap<&str, Vec<FaceKey>> = BTreeMap::new();
    let mut seen: BTreeSet<&[i32]> = BTreeSet::new();
    for r in records {
        if r.schema != KEY_CHOICES_SCHEMA {
            return Err(format!(
                "record schema {:?}, expected {KEY_CHOICES_SCHEMA}",
                r.schema
            ));
        }
        if !seen.insert(&r.key) {
            return Err(format!("key {:?} listed twice", r.key));
        }
        let acceptable: BTreeSet<String> = r
            .choices
            .iter()
            .filter(|c| kept(c, o))
            .map(|c| choice_label(c, o.label))
            .collect();
        by.entry(&r.face).or_default().push(FaceKey {
            key: r.key.clone(),
            acceptable,
        });
    }
    Ok(by
        .into_iter()
        .map(|(f, keys)| FaceInput {
            signature: f.to_string(),
            keys,
        })
        .collect())
}

/// A default ranking from the producer's `preferred` choice: labels by the number of keys where they are preferred
/// (descending), then by label. (Lane poles' regular-first list replaces it when available.)
pub fn ranking_from_preferred(records: &[KeyChoices], o: &KeyChoicesOptions) -> Vec<String> {
    let mut n: BTreeMap<String, u64> = BTreeMap::new();
    for r in records {
        if let Some(c) = r.preferred.and_then(|p| r.choices.get(p))
            && kept(c, o)
        {
            *n.entry(choice_label(c, o.label)).or_default() += 1;
        }
    }
    let mut v: Vec<(String, u64)> = n.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.into_iter().map(|(l, _)| l).collect()
}
