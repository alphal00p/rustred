//! SHELL SUBSETS as a dimension of the start lists (implementer 2026-11-15, TRICKS 17: head reduction depends on which
//! rows are present, so greedy drops from a superset need not reach the minimum of a sub-family). For a centre that
//! solves a face with base shells + escalation shells E, the start lists of base + S for each non-empty subset S of E
//! (in the spec's order) are each solved at every key (the joint minimiser's start trial: a key the list does not solve
//! is `unserved` for that alternative), minimised jointly, and returned as ALTERNATIVES ranked by the spec's measures
//! (coverage, then the pole objective, then size). Generic: shells, centre, frame and every order are data (L8).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::joint::{JointMin, minimise_joint};
use super::policy::SeedMinPolicy;
use super::{Key, Offset, PoleOrder, SeedMinError, SystemPoles, TrialSolver, add, list_digest};
use crate::coreseed::seeds::{shell_offsets, valid_at};

/// The subset dimension (spec data).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellSubsetSpec {
    /// Shells always present (e.g. the generic six).
    pub base: Vec<String>,
    /// The escalation shells whose subsets are tried.
    pub escalation: Vec<String>,
    /// `smallest_first` (default: proper subsets before the full set, smaller first, then escalation order) |
    /// `full_first`.
    #[serde(default = "d_order")]
    pub order: String,
    /// Ranking measures (lexicographic, each smaller-is-better): `unserved`, `pieces`, `max_pole`, `singular_keys`,
    /// `seeds`, `max_terms`.
    #[serde(default = "d_rank")]
    pub rank: Vec<String>,
}

fn d_order() -> String {
    "smallest_first".into()
}
fn d_rank() -> Vec<String> {
    [
        "unserved",
        "pieces",
        "max_pole",
        "singular_keys",
        "seeds",
        "max_terms",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Rank measures understood by [`ShellSubsetSpec::rank`].
pub const RANK_MEASURES: [&str; 6] = [
    "unserved",
    "pieces",
    "max_pole",
    "singular_keys",
    "seeds",
    "max_terms",
];

impl ShellSubsetSpec {
    /// The spec of a caller's start list `base + escalation` under the policy's `shell_subsets` (None when `off`).
    pub fn from_policy(
        base: &[String],
        escalation: &[String],
        policy: &SeedMinPolicy,
    ) -> Option<ShellSubsetSpec> {
        (policy.shell_subsets != "off" && !escalation.is_empty()).then(|| ShellSubsetSpec {
            base: base.to_vec(),
            escalation: escalation.to_vec(),
            order: policy.shell_subsets.clone(),
            rank: d_rank(),
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.escalation.is_empty() {
            return Err("shell subsets: no escalation shells".into());
        }
        if !["smallest_first", "full_first"].contains(&self.order.as_str()) {
            return Err(format!("shell subsets: order {:?}", self.order));
        }
        for m in &self.rank {
            if !RANK_MEASURES.contains(&m.as_str()) {
                return Err(format!(
                    "shell subsets: rank measure {m:?} not in {RANK_MEASURES:?}"
                ));
            }
        }
        Ok(())
    }

    /// The non-empty subsets of the escalation shells in the spec's order (each in escalation order).
    pub fn subsets(&self) -> Vec<Vec<String>> {
        let n = self.escalation.len();
        let mut v: Vec<Vec<String>> = (1u32..(1 << n))
            .map(|m| {
                (0..n)
                    .filter(|i| m & (1 << i) != 0)
                    .map(|i| self.escalation[i].clone())
                    .collect()
            })
            .collect();
        // smaller first; ties: the earlier escalation shells first (bit order)
        v.sort_by_key(|s| s.len());
        if self.order == "full_first" {
            v.reverse();
        }
        v
    }
}

/// One alternative: the shells, its start list size, its joint result and its rank measures.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubsetAlternative {
    pub shells: Vec<String>,
    pub start_seeds: usize,
    pub start_list_sha256: String,
    pub joint: JointMin,
    pub measures: Value,
}

/// The start list of `shells` around the centre (centre-relative), kept when the seed lies in the top sector at SOME
/// key; generation order of the shells (= its row order).
pub fn subset_list(
    keys: &[Key],
    centre: &[i32],
    lines: &[bool],
    shells: &[String],
) -> Result<Vec<Offset>, String> {
    let zero = vec![0; centre.len()];
    Ok(shell_offsets(&zero, lines, shells)?
        .into_iter()
        .filter(|o| keys.iter().any(|k| valid_at(k, &add(centre, o), lines)))
        .collect())
}

fn measures(j: &JointMin, keys: usize) -> Value {
    let mut max_pole: u64 = 0;
    let mut singular = 0u64;
    let mut undecided = 0u64;
    let mut max_terms = 0u64;
    let mut seeds = 0usize;
    for p in &j.pieces {
        seeds = seeds.max(p.list.len());
        for f in &p.per_key {
            max_terms = max_terms.max(f.rhs_count);
            match f.pole {
                PoleOrder::Order { order } => {
                    max_pole = max_pole.max(order as u64);
                    singular += (order > 0) as u64;
                }
                PoleOrder::Singular => {
                    singular += 1;
                    max_pole = max_pole.max(1);
                }
                PoleOrder::Undecided => undecided += 1,
            }
        }
    }
    let served: usize = j.pieces.iter().map(|p| p.keys.len()).sum();
    json!({"unserved": keys - served, "pieces": j.pieces.len(), "max_pole": max_pole,
        "singular_keys": singular, "undecided_poles": undecided, "seeds": seeds, "max_terms": max_terms})
}

/// Try every subset of the escalation shells (spec order), minimise each jointly, and return the alternatives ranked
/// (best first). A subset whose list serves no key is kept in the result with its `unserved` count (never dropped).
#[allow(clippy::too_many_arguments)]
pub fn minimise_shell_subsets(
    keys: &[Key],
    centre: &[i32],
    lines: &[bool],
    spec: &ShellSubsetSpec,
    policy: &SeedMinPolicy,
    solver: &mut dyn TrialSolver,
    system: &mut dyn SystemPoles,
) -> Result<Vec<SubsetAlternative>, SeedMinError> {
    spec.validate()
        .map_err(|message| SeedMinError::Input { message })?;
    let mut out = vec![];
    for sub in spec.subsets() {
        let mut shells = spec.base.clone();
        shells.extend(sub.iter().cloned());
        let list = subset_list(keys, centre, lines, &shells)
            .map_err(|message| SeedMinError::Input { message })?;
        let joint = minimise_joint(keys, centre, &list, policy, solver, system)?;
        let m = measures(&joint, keys.len());
        out.push(SubsetAlternative {
            shells,
            start_seeds: list.len(),
            start_list_sha256: list_digest(&list),
            joint,
            measures: m,
        });
    }
    let key_of = |a: &SubsetAlternative| -> Vec<u64> {
        spec.rank
            .iter()
            .map(|m| a.measures[m.as_str()].as_u64().unwrap_or(u64::MAX))
            .collect()
    };
    // stable: equal measures keep the spec's subset order
    out.sort_by_key(|a| key_of(a));
    Ok(out)
}
