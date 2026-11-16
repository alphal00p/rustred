//! Acceptance of a MINIMISED object (seed-minimised recipe or rule) on a region of a face (directing session 2026-11-14,
//! binding): a minimised object is accepted only after it was TESTED at EVERY key of the region within the layers
//! claimed (never from the minimisation key or a few keys: a seed list minimised at one key overfits); the keys it no
//! longer serves become a GUARD sub-locus kept with the ORIGINAL object, so minimisation never makes a key disappear
//! from the served set. Pure: the solves are the caller's (the engine's re-solve with the minimised seed list).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::faces::Key;

/// The decision on a minimised object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinimiseVerdict {
    /// The minimised object is accepted (for `accepted_keys` only).
    pub accept: bool,
    pub accepted_keys: Vec<Key>,
    /// Keys the original serves and the minimised object does not: the guard sub-locus (served by the original).
    pub lost_keys: Vec<Key>,
    /// Region keys without a test result of the minimised object.
    pub untested: u64,
    /// `tested: n of N keys of the region`.
    pub tested: String,
    pub reason: String,
}

/// Judge a minimised object on `region` (every key of the region in the claimed layers), given the keys the original
/// object serves and the minimised object's per-key result (true = solved and proved / matched).
pub fn judge_minimised(
    region: &[Key],
    original_served: &BTreeSet<Key>,
    minimised: &BTreeMap<Key, bool>,
) -> MinimiseVerdict {
    let n = region.len() as u64;
    let tested = region.iter().filter(|k| minimised.contains_key(*k)).count() as u64;
    let accepted: Vec<Key> = region
        .iter()
        .filter(|k| minimised.get(*k) == Some(&true))
        .cloned()
        .collect();
    let lost: Vec<Key> = region
        .iter()
        .filter(|k| original_served.contains(*k) && minimised.get(*k) != Some(&true))
        .cloned()
        .collect();
    let untested = n - tested;
    let (accept, reason) = if untested > 0 {
        (
            false,
            format!("not tested at every key of the region ({tested} of {n})"),
        )
    } else if accepted.is_empty() {
        (
            false,
            "the minimised object serves no key of the region".to_string(),
        )
    } else if lost.is_empty() {
        (
            true,
            "tested at every key; serves every key the original served".to_string(),
        )
    } else {
        (
            true,
            format!(
                "tested at every key; {} key(s) lost: kept as a guard sub-locus served by the original",
                lost.len()
            ),
        )
    };
    MinimiseVerdict {
        accept,
        accepted_keys: accepted,
        lost_keys: lost,
        untested,
        tested: format!("{tested} of {n} keys of the region"),
        reason,
    }
}
