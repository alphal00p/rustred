//! THE SELECTION among candidates for one key (or one domain) (pole-diagnostics.md section 8, addenda 4 and 7; TRICKS
//! items 9 and 12-13): wherever several ADMISSIBLE relations / rules / scan entries exist (termmin, repairflow's symbolic
//! choice, the recipe relation choice, the seed-minimisation centre scan, the relation cache), this one pure function
//! chooses.
//!
//! Order (criteria `regular_first`, the default):
//! 1. higher evidence level (the caller's rank; equal when not used);
//! 2. regular at d0 over singular, with the size guard `max_size_ratio` (addendum 7, DEFAULT `"unbounded"`: a regular
//!    candidate ALWAYS wins, whatever its size; with a finite ratio R a regular candidate wins only up to R x the
//!    smallest admissible size, beyond it the smaller singular one is kept and the choice says so);
//! 3. among regular candidates: fewer terms; among singular ones: lower pole order, then fewer terms;
//! 4. ties: the smaller `id` (a relation digest or any stable text): the result does not depend on the input order.
//!
//! Criteria `economy_first`: fewer terms first; regularity, then pole order, break exact ties only.
//!
//! A candidate whose regularity is UNKNOWN (no D1 record, an undecided verdict) is not regular and ranks after every
//! singular candidate of known order. A NON-admissible candidate is never returned; when such a candidate is regular and
//! names the cap that removed it (`excluded_by`), the choice reports "regular candidate exists, over <cap>".
//! Pure: no I/O, no clock; the preference never decides whether a key is served or whether a node closes.

use super::params::{CriteriaOrder, MaxSizeRatio};
use serde_json::{Value, json};
use std::cmp::Ordering;

/// One candidate with the facts the selection needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Stable identity (a relation digest, a rule record sha256, a scan-entry label): the last tie-break.
    pub id: String,
    /// Certified by the normal accept chain (in domain, descent, exact proof, required evidence).
    pub admissible: bool,
    /// The cap that removed a found candidate (`"max_solves"`, `"rule_size_budget"`, ...): reported, never chosen.
    pub excluded_by: Option<String>,
    /// Higher = stronger evidence (e.g. per-key = 2, imported = 1, none = 0); equal everywhere when unused.
    pub evidence_level: u32,
    /// Size measure: right-hand-side terms (the sizes compared by `max_size_ratio`).
    pub size: u64,
    /// Pole order at d0 from its D1 record: `Some(0)` regular, `Some(p)` singular, `None` unknown. Over a DOMAIN, the
    /// maximum over its keys ([`aggregate_orders`]; TRICKS 9: decided per key, never from the rule's text).
    pub pole_order: Option<u64>,
}

impl Candidate {
    pub fn is_regular(&self) -> bool {
        self.pole_order == Some(0)
    }
}

/// The pole order of a candidate over several keys: the maximum, `None` when any key is unknown (undecided).
pub fn aggregate_orders(per_key: &[Option<u64>]) -> Option<u64> {
    per_key.iter().try_fold(0u64, |m, o| o.map(|x| m.max(x)))
}

/// Why the chosen candidate won.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceReason {
    /// The chosen candidate is regular at d0 (and won by regularity or was the only / smallest one).
    Regular,
    /// No regular admissible candidate: the lowest pole order, then the fewest terms.
    LowestPole,
    /// A regular admissible candidate exists but exceeds `max_size_ratio`: the smaller singular one is kept.
    SizeGuard,
    /// `economy_first`: the smallest candidate.
    Economy,
    /// Only one admissible candidate.
    Only,
}

impl ChoiceReason {
    pub fn word(&self) -> &'static str {
        match self {
            ChoiceReason::Regular => "regular",
            ChoiceReason::LowestPole => "lowest_pole",
            ChoiceReason::SizeGuard => "size_guard",
            ChoiceReason::Economy => "economy",
            ChoiceReason::Only => "only",
        }
    }
}

/// What happened to the best regular alternative when the chosen candidate is not regular.
#[derive(Debug, Clone, PartialEq)]
pub enum RegularAlternative {
    /// The chosen candidate is regular.
    Chosen,
    /// A regular admissible candidate was rejected by the size guard: its size and the ratio to the smallest size.
    RejectedForSize {
        id: String,
        size: u64,
        ratio: String,
    },
    /// A regular candidate exists but another cap removed it (not admissible).
    OverCap { id: String, cap: String, size: u64 },
    /// No regular candidate among those given.
    None,
}

/// The selection's result and its report line.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// Index into the input slice.
    pub index: usize,
    pub id: String,
    pub reason: ChoiceReason,
    pub chosen_size: u64,
    /// The smallest size among the admissible candidates of the chosen evidence level.
    pub smallest_size: u64,
    pub chosen_pole_order: Option<u64>,
    pub max_size_ratio: String,
    pub regular_alternative: RegularAlternative,
    pub criteria_order: CriteriaOrder,
}

fn ratio_text(size: u64, smallest: u64) -> String {
    if smallest == 0 {
        return "unbounded".into();
    }
    let g = {
        let (mut a, mut b) = (size, smallest);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a.max(1)
    };
    let (n, d) = (size / g, smallest / g);
    if d == 1 {
        n.to_string()
    } else {
        format!("{n}/{d}")
    }
}

/// singular ranking: known pole order ascending, unknown last, then size, then id
fn singular_key(c: &Candidate) -> (u8, u64, u64, &str) {
    match c.pole_order {
        Some(p) => (0, p, c.size, c.id.as_str()),
        None => (1, 0, c.size, c.id.as_str()),
    }
}

/// Choose among `cands`. `None` iff no candidate is admissible.
pub fn select(cands: &[Candidate], ratio: MaxSizeRatio, criteria: CriteriaOrder) -> Option<Choice> {
    let adm: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].admissible).collect();
    let top_level = adm.iter().map(|&i| cands[i].evidence_level).max()?;
    let pool: Vec<usize> = adm
        .into_iter()
        .filter(|&i| cands[i].evidence_level == top_level)
        .collect();
    let smallest = pool.iter().map(|&i| cands[i].size).min()?;
    let by = |a: &usize, b: &usize, f: &dyn Fn(&Candidate, &Candidate) -> Ordering| {
        f(&cands[*a], &cands[*b])
    };

    let best_regular = pool
        .iter()
        .copied()
        .filter(|&i| cands[i].is_regular())
        .min_by(|a, b| {
            by(a, b, &|x, y| {
                (x.size, x.id.as_str()).cmp(&(y.size, y.id.as_str()))
            })
        });
    let best_singular = pool
        .iter()
        .copied()
        .filter(|&i| !cands[i].is_regular())
        .min_by(|a, b| by(a, b, &|x, y| singular_key(x).cmp(&singular_key(y))));
    // a regular candidate removed by another cap (not admissible), for the report
    let over_cap = cands
        .iter()
        .filter(|c| !c.admissible && c.is_regular() && c.excluded_by.is_some())
        .min_by(|x, y| (x.size, x.id.as_str()).cmp(&(y.size, y.id.as_str())));

    let (index, reason) = match criteria {
        CriteriaOrder::EconomyFirst => {
            let i = pool
                .iter()
                .copied()
                .min_by(|a, b| {
                    by(a, b, &|x, y| {
                        (x.size, !x.is_regular(), singular_key(x)).cmp(&(
                            y.size,
                            !y.is_regular(),
                            singular_key(y),
                        ))
                    })
                })
                .expect("non-empty pool");
            (i, ChoiceReason::Economy)
        }
        CriteriaOrder::RegularFirst => match (best_regular, best_singular) {
            (Some(r), _) if ratio.admits(cands[r].size, smallest) => (r, ChoiceReason::Regular),
            (Some(_), Some(s)) => (s, ChoiceReason::SizeGuard),
            (Some(r), None) => (r, ChoiceReason::Regular),
            (None, Some(s)) => (s, ChoiceReason::LowestPole),
            (None, None) => unreachable!("non-empty pool"),
        },
    };
    let reason = if pool.len() == 1 && reason != ChoiceReason::SizeGuard {
        ChoiceReason::Only
    } else {
        reason
    };
    let chosen = &cands[index];
    let regular_alternative = if chosen.is_regular() {
        RegularAlternative::Chosen
    } else if let Some(r) = best_regular {
        RegularAlternative::RejectedForSize {
            id: cands[r].id.clone(),
            size: cands[r].size,
            ratio: ratio_text(cands[r].size, smallest),
        }
    } else if let Some(c) = over_cap {
        RegularAlternative::OverCap {
            id: c.id.clone(),
            cap: c.excluded_by.clone().unwrap_or_default(),
            size: c.size,
        }
    } else {
        RegularAlternative::None
    };
    Some(Choice {
        index,
        id: chosen.id.clone(),
        reason,
        chosen_size: chosen.size,
        smallest_size: smallest,
        chosen_pole_order: chosen.pole_order,
        max_size_ratio: ratio.text(),
        regular_alternative,
        criteria_order: criteria,
    })
}

impl Choice {
    /// The report line (addendum 7: the parameter value in force and both sizes, chosen / smallest admissible).
    pub fn report_line(&self) -> String {
        let alt = match &self.regular_alternative {
            RegularAlternative::Chosen => "chosen is regular".to_string(),
            RegularAlternative::RejectedForSize { size, ratio, .. } => {
                format!(
                    "regular alternative exists, rejected for size: {size} terms, ratio {ratio}"
                )
            }
            RegularAlternative::OverCap { cap, size, .. } => {
                format!("regular candidate exists, over {cap} ({size} terms)")
            }
            RegularAlternative::None => "no regular candidate".to_string(),
        };
        let pole = self
            .chosen_pole_order
            .map_or("unknown".to_string(), |p| p.to_string());
        format!(
            "choice {}: reason {}, pole order {pole}, max_size_ratio {}, chosen size {}, smallest admissible size {}; {alt}",
            self.id,
            self.reason.word(),
            self.max_size_ratio,
            self.chosen_size,
            self.smallest_size
        )
    }

    /// The same as a JSON record (decision files, `centre_scan.chosen`, relation-entry diagnostics).
    pub fn to_json(&self) -> Value {
        let alt = match &self.regular_alternative {
            RegularAlternative::Chosen => json!({"status": "chosen"}),
            RegularAlternative::RejectedForSize { id, size, ratio } => {
                json!({"status": "rejected_for_size", "id": id, "size": size, "ratio": ratio})
            }
            RegularAlternative::OverCap { id, cap, size } => {
                json!({"status": "over_cap", "id": id, "lost_to": cap, "size": size})
            }
            RegularAlternative::None => json!({"status": "none"}),
        };
        json!({"chosen": self.id, "reason": self.reason.word(), "pole_order": self.chosen_pole_order,
            "max_size_ratio": self.max_size_ratio, "chosen_size": self.chosen_size, "smallest_size": self.smallest_size,
            "criteria_order": match self.criteria_order { CriteriaOrder::RegularFirst => "regular_first", CriteriaOrder::EconomyFirst => "economy_first" },
            "regular_alternative": alt})
    }
}
