//! SMALL EXACT COMBINATIONS OF SYMMETRY ROWS BEFORE IBP (TRICKS 8; interfaces/tworow.md; definition in
//! notes/implementer.tworow.md).
//!
//! `row(g, S)` is the verified group's EXPANDED symmetry row of element `g` at the integer seed `S`
//! (`replay_base::symrows::rows_at_with(.., SymRowMode::Expanded, ..)`: `I(S) - s E_g(S) = 0`, a non-pure element's
//! numerator images expanded exactly; d-free rationals). A COMBINATION at the key `K` is a small set of such rows, at `K`
//! and at shifted seeds `K' = sigma(K)`, with exact weights `w_i` such that `C = sum w_i row_i` has coefficient EXACTLY 1
//! at `I(K)` and every other nonzero term strictly LOWER than `K` under the caller's full node comparator. Then
//! `I(K) = -sum_{T != K} C_T I(T)`. The hand case (two rows of one non-pure element at `K` and at `K` shifted by one
//! unit on two slots, weights `(1, -1)`) is the default case of this one generic step; the number of rows, the number of
//! elements per combination and the shift neighbourhood are spec data ([`TwoRowPolicy`]).
//!
//! The search ([`search`]) is COMPLETE for minimal combinations of at most `max_rows` rows inside the neighbourhood:
//! pool = rows reachable from the rows containing `K` through shared non-lower terms, minimal subsets enumerated under
//! the cancellation pre-filter, each decided by an exact row echelon form (`CheckAlgebra::q_rref`, columns in comparator
//! order, highest first: a valid combination exists iff `K`'s column is a pivot). Budgets are spec data; a budget hit
//! is never reported as "no relation". Region forms: [`region`]. The composite with a catalogue rule: [`composite`]
//! (typed stub). Nothing here knows a node, an arity, an order or a group (L8): all are arguments.
//!
//! Symmetry rows are d-free: every combination is regular at d = 4 by construction (recorded in its JSON).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::replay_base::algebra::Q;

pub mod cert;
pub mod composite;
pub mod region;
pub mod rows;
pub mod search;

pub use cert::{CertificateCheck, KernelCertificateRef, check_certificate};
pub use composite::{CompositeStep, with_catalogue_rule};
pub use region::{RegionReport, Shape, ShapeVerdict, WeightForm, region_forms};
pub use search::{Reject, apply_combination, import_check, search_key, two_row_relations};

/// One operation of a shift (the engine's symface `--pair-shift` text: `SLOT=VALUE`, `SLOT+DELTA`, `SLOT-DELTA`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShiftOp {
    Add { slot: usize, delta: i32 },
    Set { slot: usize, value: i32 },
}

/// A shift `K -> K'`: the ops applied in order. Text form = the engine's PairShift (`11=-1,12+1`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PairShift {
    pub ops: Vec<ShiftOp>,
}

impl PairShift {
    /// Parse the engine's text: a comma list of `SLOT=VALUE` / `SLOT+DELTA` / `SLOT-DELTA` (0-based slots).
    pub fn parse(text: &str) -> Result<PairShift, String> {
        let mut ops = vec![];
        for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let op = if let Some((a, b)) = part.split_once('=') {
                ShiftOp::Set {
                    slot: a
                        .trim()
                        .parse()
                        .map_err(|_| format!("shift {part}: slot"))?,
                    value: b
                        .trim()
                        .parse()
                        .map_err(|_| format!("shift {part}: value"))?,
                }
            } else if let Some(i) = part
                .char_indices()
                .skip(1)
                .find(|(_, c)| *c == '+' || *c == '-')
                .map(|(i, _)| i)
            {
                let (a, b) = part.split_at(i);
                ShiftOp::Add {
                    slot: a
                        .trim()
                        .parse()
                        .map_err(|_| format!("shift {part}: slot"))?,
                    delta: b
                        .trim()
                        .trim_start_matches('+')
                        .parse()
                        .map_err(|_| format!("shift {part}: delta"))?,
                }
            } else {
                return Err(format!(
                    "shift {part}: SLOT=VALUE, SLOT+DELTA or SLOT-DELTA"
                ));
            };
            ops.push(op);
        }
        if ops.is_empty() {
            return Err(format!("empty shift {text:?}"));
        }
        Ok(PairShift { ops })
    }

    /// The engine's text form.
    pub fn text(&self) -> String {
        self.ops
            .iter()
            .map(|op| match *op {
                ShiftOp::Add { slot, delta } if delta >= 0 => format!("{slot}+{delta}"),
                ShiftOp::Add { slot, delta } => format!("{slot}{delta}"),
                ShiftOp::Set { slot, value } => format!("{slot}={value}"),
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// `sigma(key)`; `None` when a slot is out of range or a value overflows.
    pub fn apply(&self, key: &[i32]) -> Option<Vec<i32>> {
        let mut k = key.to_vec();
        for op in &self.ops {
            match *op {
                ShiftOp::Add { slot, delta } => {
                    let v = k.get_mut(slot)?;
                    *v = v.checked_add(delta)?;
                }
                ShiftOp::Set { slot, value } => *k.get_mut(slot)? = value,
            }
        }
        Some(k)
    }
}

/// The shift neighbourhood (spec object `tworow.shifts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShiftPolicy {
    /// Unit shifts of up to this many slots at once (0 = none): every choice of that many slots, each by +-`step`.
    /// Default 2: chosen to contain the hand's F8 shift (two slots, `11-1,12+1`; tworow notes 2026-11-15T04:30Z); not
    /// tuned by an experiment (default not yet backed by an experiment).
    pub unit_slots: usize,
    /// The magnitude of a unit shift.
    pub step: i32,
    /// Restrict the unit shifts to these slots (default: every slot).
    pub slots: Option<Vec<usize>>,
    /// Further shifts in the engine's text form (appended after the unit shifts).
    pub explicit: Vec<String>,
    /// Budget: at most this many shifts (in generation order); `None` = all.
    pub max_shifts: Option<usize>,
}

impl Default for ShiftPolicy {
    fn default() -> Self {
        ShiftPolicy {
            unit_slots: 2,
            step: 1,
            slots: None,
            explicit: vec![],
            max_shifts: None,
        }
    }
}

/// The policy of the step (spec object `tworow`; every field optional). Defaults: the hand's case (two rows of one
/// element) and the import verifier's term cap; see interfaces/tworow.md 2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TwoRowPolicy {
    /// Rows per combination (1 = single rows only). Default 2: MEASURED on the hand case (T162: a third row served
    /// nothing; interfaces/tworow.md 2, audit r2-planners ITEM 6).
    pub max_rows: usize,
    /// Distinct group elements one combination may use.
    pub elements_per_combination: usize,
    /// Use only the first this many group elements (`None` = all).
    pub max_elements: Option<usize>,
    /// Drop shifted seeds with a line below 1 or a numerator above 0 (the engine's rule for K').
    pub keep_in_chart: bool,
    pub shifts: ShiftPolicy,
    /// One row above this many terms = that seed is over the cap (typed, never "no relation"). Default 65536 (the
    /// import verifier's term cap). Default not yet backed by an experiment (observed: hit at 2 of 4 F8 probe keys,
    /// tworow notes 2026-11-15T04:30Z).
    pub max_terms_per_row: usize,
    /// Budget of the pool (rows that can take part); truncated deterministically, flagged incomplete.
    pub max_pool_rows: usize,
    /// Budget of the subsets examined per key; flagged incomplete when hit.
    pub max_subsets: usize,
    /// How many valid combinations are kept per key (ranked). Default 8. Default not yet backed by an experiment.
    pub max_relations: usize,
    /// Report one-row relations too (a row at K alone: route 0b's object; one row at a shifted seed alone).
    pub single_rows: bool,
    /// When no valid combination exists: keep at most this many NEAR MISSES (every non-lower term but one cancels),
    /// the input of the composite with one catalogue rule (0 = off). Default 8. Default not yet backed by an experiment.
    pub max_near_misses: usize,
    /// Expand a row at a shifted seed only when its structural shape may contain a frontier term (exact; `false` =
    /// expand every row, for comparisons).
    pub structural_filter: bool,
}

impl Default for TwoRowPolicy {
    fn default() -> Self {
        TwoRowPolicy {
            max_rows: 2,
            elements_per_combination: 1,
            max_elements: None,
            keep_in_chart: true,
            shifts: ShiftPolicy::default(),
            max_terms_per_row: 65_536,
            max_pool_rows: 20_000,
            max_subsets: 200_000,
            max_relations: 8,
            single_rows: true,
            max_near_misses: 8,
            structural_filter: true,
        }
    }
}

impl TwoRowPolicy {
    /// Read the policy from a spec: the object under `"tworow"` if present, else the value itself; `null` = defaults.
    pub fn from_spec(v: &Value) -> Result<TwoRowPolicy, String> {
        let obj = v.get("tworow").unwrap_or(v);
        if obj.is_null() {
            return Ok(TwoRowPolicy::default());
        }
        let p: TwoRowPolicy =
            serde_json::from_value(obj.clone()).map_err(|e| format!("tworow policy: {e}"))?;
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.max_rows == 0 {
            return Err("tworow.max_rows must be >= 1".into());
        }
        if self.elements_per_combination == 0 {
            return Err("tworow.elements_per_combination must be >= 1".into());
        }
        if self.shifts.unit_slots > 0 && self.shifts.step == 0 {
            return Err("tworow.shifts.step must be nonzero".into());
        }
        if self.max_relations == 0 {
            return Err("tworow.max_relations must be >= 1".into());
        }
        for t in &self.shifts.explicit {
            PairShift::parse(t)?;
        }
        Ok(())
    }

    /// The shifts in generation order (deterministic): unit shifts of 1, then 2, ... slots (slots ascending, `-step`
    /// before `+step`), then the explicit ones; duplicates (same ops) dropped; cut at `max_shifts`.
    pub fn shifts(&self, arity: usize) -> Result<Vec<PairShift>, String> {
        let slots: Vec<usize> = match &self.shifts.slots {
            Some(s) => {
                let mut s = s.clone();
                s.sort_unstable();
                s.dedup();
                if let Some(&x) = s.iter().find(|&&x| x >= arity) {
                    return Err(format!("tworow.shifts.slots: slot {x} >= arity {arity}"));
                }
                s
            }
            None => (0..arity).collect(),
        };
        let step = self.shifts.step;
        let mut out: Vec<PairShift> = vec![];
        let mut seen: BTreeSet<PairShift> = BTreeSet::new();
        let cap = self.shifts.max_shifts.unwrap_or(usize::MAX);
        let mut push = |s: PairShift, out: &mut Vec<PairShift>| {
            if out.len() < cap && seen.insert(s.clone()) {
                out.push(s);
            }
        };
        for width in 1..=self.shifts.unit_slots.min(slots.len()) {
            // every `width`-subset of the slots (lexicographic), every sign pattern (- before +)
            let mut idx: Vec<usize> = (0..width).collect();
            loop {
                for signs in 0..(1usize << width) {
                    let ops = idx
                        .iter()
                        .enumerate()
                        .map(|(j, &i)| ShiftOp::Add {
                            slot: slots[i],
                            delta: if signs >> (width - 1 - j) & 1 == 1 {
                                step
                            } else {
                                -step
                            },
                        })
                        .collect();
                    push(PairShift { ops }, &mut out);
                }
                // next combination
                let mut j = width;
                while j > 0 && idx[j - 1] == slots.len() - width + j - 1 {
                    j -= 1;
                }
                if j == 0 {
                    break;
                }
                idx[j - 1] += 1;
                for t in j..width {
                    idx[t] = idx[t - 1] + 1;
                }
            }
        }
        for t in &self.shifts.explicit {
            push(PairShift::parse(t)?, &mut out);
        }
        Ok(out)
    }
}

/// One row of a combination: the element and the shift of its seed (`None` = the key itself).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowSpec {
    pub element: usize,
    pub shift: Option<PairShift>,
}

impl RowSpec {
    pub fn text(&self) -> String {
        match &self.shift {
            None => format!("g{}@K", self.element),
            Some(s) => format!("g{}@K[{}]", self.element, s.text()),
        }
    }
}

/// A row as used at one key: its spec, the seed, its number of terms (after the zero-sector drop, if any).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowRef {
    pub spec: RowSpec,
    pub seed: Vec<i32>,
    pub terms: usize,
}

/// A valid combination at a key: `sum weights[i] * row_i` has coefficient exactly 1 at `I(key)` and reads
/// `I(key) - sum rhs = 0`, every RHS key strictly lower than the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combination {
    pub key: Vec<i32>,
    pub rows: Vec<RowRef>,
    pub weights: Vec<Q>,
    /// The coefficient of `I(key)` in each row (a key reappearing in a non-pure image, or in a shifted row).
    pub key_coefficients: Vec<Q>,
    /// `I(key) = sum c I(k)`, highest key first.
    pub rhs: Vec<(Vec<i32>, Q)>,
}

impl Combination {
    fn empty(key: &[i32]) -> Combination {
        Combination {
            key: key.to_vec(),
            rows: vec![],
            weights: vec![],
            key_coefficients: vec![],
            rhs: vec![],
        }
    }
    /// `single_row` (one row at the key), `shifted_row` (one row at a shifted seed alone), `pair` (two rows of one
    /// element), `two_elements` (two rows of two elements), `rows<m>` otherwise.
    pub fn kind(&self) -> String {
        let el: BTreeSet<usize> = self.rows.iter().map(|r| r.spec.element).collect();
        match (self.rows.len(), el.len()) {
            (1, _) if self.rows[0].spec.shift.is_none() => "single_row".into(),
            (1, _) => "shifted_row".into(),
            (2, 1) => "pair".into(),
            (2, _) => "two_elements".into(),
            (m, _) => format!("rows{m}"),
        }
    }
    pub fn elements(&self) -> Vec<usize> {
        let el: BTreeSet<usize> = self.rows.iter().map(|r| r.spec.element).collect();
        el.into_iter().collect()
    }
    pub fn shape(&self) -> Vec<RowSpec> {
        self.rows.iter().map(|r| r.spec.clone()).collect()
    }
    pub fn seeds(&self) -> Vec<Vec<i32>> {
        self.rows.iter().map(|r| r.seed.clone()).collect()
    }
    /// The RHS as (key text, rational text): the supplied-relation form of the import.
    pub fn rhs_text(&self) -> Vec<(Vec<i32>, String)> {
        self.rhs
            .iter()
            .map(|(k, c)| (k.clone(), crate::termmin::symrel::q_text(c)))
            .collect()
    }
    /// The record of the combination (no node name: the caller adds its binding).
    pub fn to_json(&self) -> Value {
        let q = crate::termmin::symrel::q_text;
        json!({
            "kind": self.kind(),
            "key": self.key,
            "rows": self.rows.iter().map(|r| json!({
                "element": r.spec.element,
                "shift": r.spec.shift.as_ref().map(PairShift::text),
                "seed": r.seed,
                "terms": r.terms,
            })).collect::<Vec<_>>(),
            "weights": self.weights.iter().map(q).collect::<Vec<_>>(),
            "key_coefficients": self.key_coefficients.iter().map(q).collect::<Vec<_>>(),
            "rhs_terms": self.rhs.len(),
            "rhs": self.rhs.iter().map(|(k, c)| json!([k, q(c)])).collect::<Vec<_>>(),
            "d_free": true,
            "regular_at_d4": "by construction (symmetry rows are d-free)",
            // planfix: a kernel certificate (rr_proof --symrows, contract kind proof-symrows) is attached by
            // `Combination::to_json_with` and checked by `tworow::cert::check_certificate`; none here
            "engine_certificate": "none (no kernel certificate attached; see tworow::cert)",
        })
    }
}

/// Counters of one key's search.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SearchStats {
    pub elements: usize,
    pub shifts: usize,
    /// Distinct seeds examined (the key + the shifted seeds kept).
    pub seeds: usize,
    /// Seeds with at least one row over `max_terms_per_row`.
    pub seeds_over_cap: usize,
    /// Rows over `max_terms_per_row` (unknown rows; the other rows of their seed are kept).
    pub rows_over_cap: usize,
    /// Rows computed in the scan.
    pub rows: usize,
    pub pool_rows: usize,
    pub pool_truncated: bool,
    pub subsets_tested: usize,
    pub subsets_budget_hit: bool,
    /// Exact-arithmetic failures (i128 overflow): those seeds / subsets are undecided.
    pub undecided: usize,
    /// Valid combinations found (before the `max_relations` cut).
    pub found: usize,
    /// Pivot rows refused by the explicit re-check (a term not strictly lower under the comparator).
    pub not_lower: usize,
    /// Near misses kept (keys without a valid combination only).
    pub near_misses: usize,
    /// Row checks skipped by the structural filter (summed over the rounds).
    pub rows_skipped_rounds: usize,
    /// Rows of the neighbourhood never expanded (they cannot touch any frontier term).
    pub rows_never_expanded: usize,
}

impl SearchStats {
    /// Every budget held and every arithmetic step was decided: a "none" outcome is a proof of absence inside the
    /// neighbourhood.
    pub fn complete(&self) -> bool {
        self.seeds_over_cap == 0
            && self.rows_over_cap == 0
            && !self.pool_truncated
            && !self.subsets_budget_hit
            && self.undecided == 0
    }
}

/// The typed outcome at one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Valid combinations, best first (at most `max_relations`); `SearchStats::complete` says whether the search
    /// covered the whole neighbourhood.
    Served { relations: Vec<Combination> },
    /// The complete search found none inside the neighbourhood.
    NoneInNeighbourhood,
    /// None found AND a budget or cap was hit: no claim of absence.
    OverCap { what: String },
    /// None found AND an exact-arithmetic step failed: no claim of absence.
    Undecided { reason: String },
    /// The input does not fit (arity, policy).
    Refused { reason: String },
}

impl KeyOutcome {
    pub fn word(&self) -> &'static str {
        match self {
            KeyOutcome::Served { .. } => "served",
            KeyOutcome::NoneInNeighbourhood => "none_in_neighbourhood",
            KeyOutcome::OverCap { .. } => "over_cap",
            KeyOutcome::Undecided { .. } => "undecided",
            KeyOutcome::Refused { .. } => "refused",
        }
    }
    pub fn best(&self) -> Option<&Combination> {
        match self {
            KeyOutcome::Served { relations } => relations.first(),
            _ => None,
        }
    }
}

/// A combination that cancels every non-lower term but ONE: `comb.rhs` reads `I(key) = c_T I(T) + lower` with the
/// remaining non-lower key(s) in `non_lower`. NOT a relation for the key by itself: the input of
/// [`composite::with_catalogue_rule`] (one catalogue rule at `T`, then descent against the key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearMiss {
    pub comb: Combination,
    pub non_lower: Vec<Vec<i32>>,
}

/// The result at one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyResult {
    pub key: Vec<i32>,
    pub outcome: KeyOutcome,
    pub stats: SearchStats,
    /// Only for keys without a valid combination (best first: fewest non-lower terms, then size).
    pub near_misses: Vec<NearMiss>,
}

impl KeyResult {
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "key": self.key,
            "outcome": self.outcome.word(),
            "complete": self.stats.complete(),
            "stats": serde_json::to_value(&self.stats).unwrap_or(Value::Null),
        });
        match &self.outcome {
            KeyOutcome::Served { relations } => {
                v["relations"] = relations.iter().map(Combination::to_json).collect();
            }
            KeyOutcome::OverCap { what } => v["what"] = json!(what),
            KeyOutcome::Undecided { reason } | KeyOutcome::Refused { reason } => {
                v["reason"] = json!(reason)
            }
            KeyOutcome::NoneInNeighbourhood => {}
        }
        if !self.near_misses.is_empty() {
            v["near_misses"] = self
                .near_misses
                .iter()
                .map(|n| {
                    let mut j = n.comb.to_json();
                    j["non_lower"] = json!(n.non_lower);
                    j
                })
                .collect();
        }
        v
    }
}
