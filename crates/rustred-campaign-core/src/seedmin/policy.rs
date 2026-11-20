//! The seedmin POLICY (interfaces/seedmin.md): every heuristic of the minimiser is data with a default backed by a
//! measurement or a ruling (cited per field). Nothing here knows a node, an arity or a line count (L8).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// v2 = the defaults of the user ruling of 2026-11-19 (rule 21: terms first, the pole order last, `drop` on an undecided
/// regularity); v1 = the withdrawn rule's (still parsed: results recorded under v1 stay recognisable).
pub const POLICY_SCHEMA: &str = "rustred.campaign.seedmin.v2";
pub const POLICY_SCHEMA_V1: &str = "rustred.campaign.seedmin.v1";

/// Measures of the lexicographic objective (pole-diagnostics.md addendum 8; TRICKS 15).
pub const OBJECTIVE_MEASURES: [&str; 3] = ["pole_order", "seeds", "terms"];

/// The spread keys of a joint minimisation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpreadPolicy {
    /// Keys minimised one by one before the union (TRICKS 7: a list minimised at ONE key overfits).
    #[serde(default = "d_spread_count")]
    pub count: usize,
    /// `farthest` (first key in the caller's order, then farthest-point in L1, ties: earlier in the caller's order) |
    /// `first` (the first `count` keys in the caller's order) | `explicit` (`keys`).
    #[serde(default = "d_spread_method")]
    pub method: String,
    #[serde(default)]
    pub keys: Vec<Vec<i32>>,
    /// More keys minimised (first failing key in farthest order) when the union fails somewhere.
    #[serde(default = "d_spread_extra")]
    pub max_extra: usize,
}

impl Default for SpreadPolicy {
    fn default() -> Self {
        SpreadPolicy {
            count: d_spread_count(),
            method: d_spread_method(),
            keys: vec![],
            max_extra: d_spread_extra(),
        }
    }
}

/// Provisional (no measurement yet: set from the T162 run of this lane): three keys span a face's corners. Default not
/// yet backed by an experiment (planfix, audit r2-planners ITEM 6; also `max_extra` 4).
fn d_spread_count() -> usize {
    3
}
fn d_spread_method() -> String {
    "farthest".into()
}
fn d_spread_extra() -> usize {
    4
}

/// Joint minimisation over a domain (TRICKS 5 / 7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointPolicy {
    #[serde(default)]
    pub spread: SpreadPolicy,
    /// `repair_union` (join the START relation's support of every failing key: a superset of a key's support still
    /// contains its relation) | `split` (failing keys go to the next piece).
    #[serde(default = "d_on_failure")]
    pub on_candidate_failure: String,
    /// Keys of the cheap screen before the every-key confirmation (the confirmation is never skipped). Default 4.
    /// Default not yet backed by an experiment.
    #[serde(default = "d_screen")]
    pub screen_keys: usize,
    /// Work budget of the joint greedy of one piece (screen + confirm trials; a count, never a clock). Default 600.
    /// Default not yet backed by an experiment (the T162 / T178 pieces used 67-335 solves, seedmin notes; no sweep).
    #[serde(default = "d_joint_max_solves")]
    pub max_solves: u64,
    /// Pieces per domain (the rest keeps the start list). Default 4. Default not yet backed by an experiment.
    #[serde(default = "d_max_pieces")]
    pub max_pieces: usize,
    /// Every piece list also solved at the other pieces' keys (complete choice sets for lane regions).
    #[serde(default = "yes")]
    pub choice_sets_complete: bool,
    /// The JOINT greedy's drop strategy (the per-key one is `drop_chunks`): `single` | `halving` | `auto` (default:
    /// halving when the piece has more than `halving_above_keys` keys, else single: a kept single drop costs one
    /// every-key confirmation, i.e. work ~ domain size per seed).
    #[serde(default = "d_joint_chunks")]
    pub drop_chunks: String,
    /// MEASURED (T162): 31-44 keys with single drops: 67-129 joint solves, under 40 min at 4 CPUs; 86 keys: ~4 min per
    /// kept drop (86-key confirmation) from a 185-seed repaired candidate, > 8 h projected: the threshold lies between.
    #[serde(default = "d_halving_above")]
    pub halving_above_keys: usize,
}

impl Default for JointPolicy {
    fn default() -> Self {
        JointPolicy {
            spread: SpreadPolicy::default(),
            on_candidate_failure: d_on_failure(),
            screen_keys: d_screen(),
            max_solves: d_joint_max_solves(),
            max_pieces: d_max_pieces(),
            choice_sets_complete: true,
            drop_chunks: d_joint_chunks(),
            halving_above_keys: d_halving_above(),
        }
    }
}

fn d_joint_chunks() -> String {
    "auto".into()
}
fn d_halving_above() -> usize {
    50
}

fn d_on_failure() -> String {
    "repair_union".into()
}
fn d_screen() -> usize {
    4
}
fn d_joint_max_solves() -> u64 {
    600
}
fn d_max_pieces() -> usize {
    4
}
fn yes() -> bool {
    true
}

/// The whole policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedMinPolicy {
    #[serde(default = "d_schema")]
    pub schema: String,
    /// `emulated` (repeated solve-recipe jobs; DEVIATION: the engine minimiser does not exist) | `engine` (the recipe
    /// carries `elimination.minimise`; one trial returns the minimised relation).
    #[serde(default = "d_backend")]
    pub backend: String,
    /// `greedy` | `support` | `off` (contract line-7 names; `off` = start relation only).
    #[serde(default = "d_mode")]
    pub mode: String,
    /// `distance` (L1 norm of the offset, OUTERMOST first, ties: later in the list first) | `outermost_first` (reverse
    /// list order). TRICKS 5 / coord-seed-minimise.md 8: distance order 62 seeds vs 179 / 197 for the other orders.
    #[serde(default = "d_order")]
    pub order: String,
    /// `centre` (distance of the centre-relative offset) | `key` (of the seed from the key).
    #[serde(default = "d_distance_from")]
    pub distance_from: String,
    /// `comparator` | `m1`: the measure of the caller's relation rule, recorded in `minimise.measure` (TRICKS 5:
    /// comparator).
    #[serde(default = "d_measure")]
    pub measure: String,
    /// Work budget of one per-key minimisation (sequential re-solves; a count, never a clock).
    #[serde(default = "d_max_solves")]
    pub max_solves: u64,
    /// Speculative drops tested against the same list at once (deterministic: the first success in order wins; the
    /// result and `solves` equal the sequential run). Default 4 (= the host's threads per process; a cost knob, never a
    /// result: default not yet backed by an experiment).
    #[serde(default = "d_parallel")]
    pub parallel_trials: usize,
    /// `single` (default: one seed per trial, the hand procedure) | `halving` (adaptive chunks: drop the next `chunk`
    /// candidates together; a failure halves the chunk, a success doubles it; a failing single seed is kept:
    /// coord-seed-minimise.md 1b, O(m log n) solves for m essential seeds; no speculative batches).
    #[serde(default = "d_drop_chunks")]
    pub drop_chunks: String,
    /// First chunk of `halving` (0 = the candidates divided by `halving_factor`).
    #[serde(default)]
    pub chunk_start: usize,
    /// planfix (audit r2-planners ITEM 6): the factor of the adaptive chunks of `halving` (per key AND joint): a
    /// refused chunk is divided by it, a kept one multiplied by it, the first chunk is the candidates divided by it.
    /// Default 2 (coord-seed-minimise.md 1b: bisection, O(m log n) solves for m essential seeds). Default not yet backed
    /// by an experiment (no other factor was measured). Omitted from the canonical policy (digest) at its default.
    #[serde(
        default = "d_halving_factor",
        skip_serializing_if = "is_d_halving_factor"
    )]
    pub halving_factor: usize,
    /// After a kept drop, restrict to the new relation's support by one more trial.
    #[serde(default = "yes")]
    pub resupport_after_drop: bool,
    /// Lexicographic objective (pole-diagnostics.md addendum 8, TRICKS 15 iii): measures from [`OBJECTIVE_MEASURES`].
    #[serde(default = "d_objective")]
    pub objective: Vec<String>,
    /// A kept drop never increases the right-hand-side terms (coord-seed-minimise.md 1b iii).
    #[serde(default = "yes")]
    pub monotone_rhs: bool,
    /// `max` | `sum`: terms of a joint state.
    #[serde(default = "d_terms_aggregate")]
    pub terms_aggregate: String,
    /// `keep_seed` (an undecided regularity refuses the drop) | `drop` (implementer 2026-11-15).
    #[serde(default = "d_on_undecided")]
    pub on_undecided_regularity: String,
    /// `on` (default: the pole order of the DELIVERED relation is the first measure, a preference: refusing a drop only
    /// keeps a larger list; POLES_OBJECTIVE.md 1 / 2 A) | `off` (plain size minimisation: no pole measure, no system
    /// judge).
    #[serde(default = "d_on")]
    pub pole_objective: String,
    /// `off` (default) | `on`: regularity check (ii), the SYSTEM's best pole order p* (lane tadic) not worse after a
    /// change; informative only (the binding test is the delivered relation's order, check (i)); off by default for its
    /// cost (one exact p* of the start system per key plus one per change: seconds for hundreds of rows, far more for
    /// the thousands of rows of a face's list; notes, record of the measurement).
    #[serde(default = "d_off")]
    pub system_regularity: String,
    /// `smallest_first` (default, MEASURED: T162 {0,1,3,4} centre line 4 down, 44 keys: base six + {S2} gave 67 seeds /
    /// 654-833 terms / 44 of 44 regular against 103 / 1,258-1,528 / 43 regular from base + {S2, S2m1}; base + {S2m1} solved
    /// no key) | `full_first` | `off` (the full list only): how a caller whose start list is base + escalation shells
    /// orders the subset alternatives of [`super::subsets::minimise_shell_subsets`] (TRICKS 17: greedy drops from a
    /// superset need not reach the minimum of a sub-family).
    #[serde(default = "d_shell_subsets")]
    pub shell_subsets: String,
    /// The ROW ORDER of the lists (TRICKS 17 (2): head reduction depends on it): `given` (the start list's order = the
    /// recipe's generation order, kept by the engine minimiser design, coord-seed-minimise.md 1b) | `distance_asc` |
    /// `distance_desc` (stable, L1 per `distance_from`). Applied once to the start list; every later list keeps it.
    #[serde(default = "d_row_order")]
    pub row_order: String,
    /// The dimension of the pole objective (user: d = 4).
    #[serde(default = "d_d0")]
    pub d0: String,
    /// TRICKS 14 (c): a DIAGNOSTIC probe-mode pass at d0 (needs an engine with probe mode); its list is a candidate,
    /// solved and proved exactly in normal mode before use; its failure is no verdict (TRICKS 16).
    #[serde(default)]
    pub rows_at_d0: bool,
    #[serde(default)]
    pub joint: JointPolicy,
}

fn d_schema() -> String {
    POLICY_SCHEMA.into()
}
fn d_backend() -> String {
    "emulated".into()
}
fn d_mode() -> String {
    "greedy".into()
}
fn d_order() -> String {
    "distance".into()
}
fn d_distance_from() -> String {
    "centre".into()
}
fn d_measure() -> String {
    "comparator".into()
}
/// Provisional: K* (TRICKS 5) 458 seeds -> 155 by support -> 62-73 by greedy: < 160 drop trials.
fn d_max_solves() -> u64 {
    400
}
fn d_parallel() -> usize {
    4
}
fn d_drop_chunks() -> String {
    "single".into()
}
fn d_halving_factor() -> usize {
    2
}
fn is_d_halving_factor(f: &usize) -> bool {
    *f == d_halving_factor()
}
/// USER RULING 2026-11-19 (rule 21): terms first, then seeds, the pole order LAST (a tie-break). It was
/// [pole_order, seeds, terms] under the withdrawn rule; a policy naming that order is the explicit, non-default option.
fn d_objective() -> Vec<String> {
    ["terms", "seeds", "pole_order"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}
fn d_terms_aggregate() -> String {
    "max".into()
}
/// USER RULING 2026-11-19 (rule 21): an undecided regularity does not stop a drop (`drop`); `keep_seed` only when named.
fn d_on_undecided() -> String {
    "drop".into()
}
fn d_on() -> String {
    "on".into()
}
fn d_shell_subsets() -> String {
    "smallest_first".into()
}
fn d_off() -> String {
    "off".into()
}
fn d_row_order() -> String {
    "given".into()
}
fn d_d0() -> String {
    "4".into()
}

impl Default for SeedMinPolicy {
    fn default() -> Self {
        serde_json::from_value(Value::Object(Default::default())).expect("defaults")
    }
}

impl SeedMinPolicy {
    pub fn parse(text: &str) -> Result<SeedMinPolicy, String> {
        let p: SeedMinPolicy =
            serde_json::from_str(text).map_err(|e| format!("seedmin policy: {e}"))?;
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<(), String> {
        let pick = |v: &str, list: &[&str], what: &str| -> Result<(), String> {
            if list.contains(&v) {
                Ok(())
            } else {
                Err(format!("{what} {v:?} not in {list:?}"))
            }
        };
        if self.schema != POLICY_SCHEMA && self.schema != POLICY_SCHEMA_V1 {
            return Err(format!(
                "schema {} (want {POLICY_SCHEMA} or {POLICY_SCHEMA_V1})",
                self.schema
            ));
        }
        pick(&self.backend, &["emulated", "engine"], "backend")?;
        pick(&self.mode, &["greedy", "support", "off"], "mode")?;
        pick(&self.order, &["distance", "outermost_first"], "order")?;
        pick(&self.distance_from, &["centre", "key"], "distance_from")?;
        pick(&self.measure, &["comparator", "m1"], "measure")?;
        pick(&self.terms_aggregate, &["max", "sum"], "terms_aggregate")?;
        pick(&self.pole_objective, &["on", "off"], "pole_objective")?;
        pick(&self.drop_chunks, &["single", "halving"], "drop_chunks")?;
        pick(
            &self.joint.drop_chunks,
            &["single", "halving", "auto"],
            "joint.drop_chunks",
        )?;
        pick(&self.system_regularity, &["off", "on"], "system_regularity")?;
        pick(
            &self.shell_subsets,
            &["smallest_first", "full_first", "off"],
            "shell_subsets",
        )?;
        pick(
            &self.row_order,
            &["given", "distance_asc", "distance_desc"],
            "row_order",
        )?;
        pick(
            &self.on_undecided_regularity,
            &["keep_seed", "drop"],
            "on_undecided_regularity",
        )?;
        pick(
            &self.joint.on_candidate_failure,
            &["repair_union", "split"],
            "joint.on_candidate_failure",
        )?;
        pick(
            &self.joint.spread.method,
            &["farthest", "first", "explicit"],
            "joint.spread.method",
        )?;
        if self.objective.is_empty() {
            return Err("objective is empty".into());
        }
        for m in &self.objective {
            pick(m, &OBJECTIVE_MEASURES, "objective measure")?;
        }
        let mut seen = self.objective.clone();
        seen.sort();
        seen.dedup();
        if seen.len() != self.objective.len() {
            return Err("objective lists a measure twice".into());
        }
        if self.parallel_trials == 0 {
            return Err("parallel_trials must be >= 1".into());
        }
        if self.halving_factor < 2 {
            return Err("halving_factor must be >= 2".into());
        }
        if self.joint.screen_keys == 0 || self.joint.max_pieces == 0 {
            return Err("joint.screen_keys and joint.max_pieces must be >= 1".into());
        }
        if self.joint.spread.method == "explicit" && self.joint.spread.keys.is_empty() {
            return Err("joint.spread.method explicit needs joint.spread.keys".into());
        }
        if self.d0.trim().is_empty() {
            return Err("d0 is empty".into());
        }
        Ok(())
    }

    /// sha256 of the canonical JSON of the policy (defaults filled in).
    pub fn digest(&self) -> String {
        let v = serde_json::to_value(self).unwrap_or(Value::Null);
        let t = crate::schema::canon::canonical_json(&v, false).unwrap_or_default();
        super::sha256_hex(t.as_bytes())
    }

    /// The line-7 `elimination.minimise` member of the engine backend.
    pub fn engine_member(&self) -> Value {
        serde_json::json!({"mode": self.mode, "order": self.order, "max_solves": self.max_solves,
            "measure": self.measure})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_round_trip() {
        let p = SeedMinPolicy::default();
        p.validate().unwrap();
        assert_eq!(p.order, "distance");
        // rule 21 (2026-11-19): size first, the pole order a tie-break; an undecided regularity never keeps a seed
        assert_eq!(p.objective, vec!["terms", "seeds", "pole_order"]);
        assert_eq!(p.on_undecided_regularity, "drop");
        assert_eq!(p.schema, POLICY_SCHEMA);
        // a v1 policy (the old rule's defaults written out) still parses and keeps its explicit order
        let old = SeedMinPolicy::parse(r#"{"schema":"rustred.campaign.seedmin.v1","objective":["pole_order","seeds","terms"],"on_undecided_regularity":"keep_seed"}"#).unwrap();
        assert_eq!(old.objective[0], "pole_order");
        let t = serde_json::to_string(&p).unwrap();
        assert_eq!(SeedMinPolicy::parse(&t).unwrap(), p);
        assert_eq!(SeedMinPolicy::parse("{}").unwrap().digest(), p.digest());
        assert!(SeedMinPolicy::parse(r#"{"order":"random"}"#).is_err());
        assert!(SeedMinPolicy::parse(r#"{"objective":["seeds","seeds"]}"#).is_err());
        assert!(SeedMinPolicy::parse(r#"{"objective":["beauty"]}"#).is_err());
        assert!(SeedMinPolicy::parse(r#"{"unknown":1}"#).is_err());
        assert!(SeedMinPolicy::parse(r#"{"joint":{"spread":{"method":"explicit"}}}"#).is_err());
    }
}
