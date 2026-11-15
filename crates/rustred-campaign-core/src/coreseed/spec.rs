//! The coreseed POLICY (interfaces/coreseed.md section 2): every heuristic of the IBP-core flow is data with a default
//! backed by a measurement (cited per field). Nothing here knows a node, an arity or a line count (L8).

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const POLICY_SCHEMA: &str = "rustred.campaign.coreseed.v1";

/// A centre of a seed rule: a generic token resolved per key from the frame mask, or an explicit offset vector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CentreSpec {
    Token(String),
    Offset(Vec<i32>),
}

/// Centre tokens understood by [`super::seeds::resolve_centres`].
pub const CENTRE_TOKENS: [&str; 6] = [
    "key",
    "dotted_line_down",
    "numerator_up",
    "numerator_pairs_up",
    "line_up_at_lowest",
    "floor_of_free",
];

impl CentreSpec {
    pub fn token(t: &str) -> CentreSpec {
        CentreSpec::Token(t.to_string())
    }
    pub fn label(&self) -> String {
        match self {
            CentreSpec::Token(t) => t.clone(),
            CentreSpec::Offset(v) => format!("{v:?}"),
        }
    }
}

/// The seed rule: centres (union) x shells around each centre (harder unit, all slots, no floor).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSpec {
    #[serde(default = "d_rule_centres")]
    pub centres: Vec<CentreSpec>,
    #[serde(default = "d_rule_shells")]
    pub shells: Vec<String>,
    /// `comparator` (relation_rule `first_admissible`) | `m1` (relation_rule `m1_first`).
    #[serde(default = "d_measure")]
    pub measure: String,
}

impl Default for RuleSpec {
    fn default() -> Self {
        RuleSpec {
            centres: d_rule_centres(),
            shells: d_rule_shells(),
            measure: d_measure(),
        }
    }
}

fn d_rule_centres() -> Vec<CentreSpec> {
    vec![CentreSpec::token("key")]
}
/// TRICKS 5 (helper N11, 2026-11-14T23:17Z): the generic shells that closed every T162 core tried.
fn d_rule_shells() -> Vec<String> {
    ["S0", "S1", "Sm1", "S1m1", "S1m2", "Sm2"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}
fn d_measure() -> String {
    "comparator".into()
}

/// Seed minimisation, named as the contract's line-7 `elimination.minimise` (mode, order, max_solves, measure).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinimiseSpec {
    /// `emulated` (repeated solve-recipe jobs; the engine minimiser does not exist yet: DEVIATION) | `engine` (the
    /// recipe carries `elimination.minimise`; one job returns the minimised relation).
    #[serde(default = "d_backend")]
    pub backend: String,
    /// `greedy` | `support` | `off`.
    #[serde(default = "d_mode")]
    pub mode: String,
    /// `distance` (L1 distance of the offset from the key, ties: generation order) | `outermost_first` | `given` (the
    /// generation order itself: an explicit list given as centres with shell S0 keeps its row order, TRICKS 17 b).
    #[serde(default = "d_order")]
    pub order: String,
    /// `comparator` | `m1`: the measure a kept drop must still satisfy.
    #[serde(default = "d_measure")]
    pub measure: String,
    /// Work budget: re-solves per minimisation (a count, never a clock).
    #[serde(default = "d_max_solves")]
    pub max_solves: u64,
    /// A kept drop never increases the right-hand-side terms (coord-seed-minimise 1b iii).
    #[serde(default = "yes")]
    pub monotone_rhs: bool,
    /// A relation regular at d0 stays regular after a drop (coord-seed-minimise 1b i; user ruling regular wins).
    #[serde(default = "yes")]
    pub keep_regular_at_d0: bool,
    /// Speculative drops tested at once (deterministic: the first success in order wins).
    #[serde(default = "d_parallel")]
    pub parallel_trials: usize,
    /// TRICKS 14 (c): minimise with the rows evaluated AT d0 (engine probe / discovery_d: diagnostic records, never
    /// evidence), then solve and prove the found list exactly. Needs an engine with line 7's probe mode: refused (typed)
    /// on an engine without it.
    #[serde(default)]
    pub rows_at_d0: bool,
}

impl Default for MinimiseSpec {
    fn default() -> Self {
        MinimiseSpec {
            backend: d_backend(),
            mode: d_mode(),
            order: d_order(),
            measure: d_measure(),
            max_solves: d_max_solves(),
            monotone_rhs: true,
            keep_regular_at_d0: true,
            parallel_trials: d_parallel(),
            rows_at_d0: false,
        }
    }
}

fn d_backend() -> String {
    "emulated".into()
}
fn d_mode() -> String {
    "greedy".into()
}
/// coord-seed-minimise.md 8 (helper, four-dot cores): distance order 62 seeds vs 179 / 197 for the other orders.
fn d_order() -> String {
    "distance".into()
}
fn d_max_solves() -> u64 {
    400
}
fn d_parallel() -> usize {
    4
}
fn yes() -> bool {
    true
}

/// Where a list is minimised and how many lists a rule may carry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainSpec {
    /// `highest_dr` | `lowest_dr` | `first` (lexicographic first key).
    #[serde(default = "d_pivot")]
    pub pivot: String,
    #[serde(default = "d_max_lists")]
    pub max_lists: usize,
}

impl Default for DomainSpec {
    fn default() -> Self {
        DomainSpec {
            pivot: d_pivot(),
            max_lists: d_max_lists(),
        }
    }
}

fn d_pivot() -> String {
    "highest_dr".into()
}
fn d_max_lists() -> usize {
    4
}

/// One escalation step: added shells (cumulative) and / or centres (one job per centre when `per_centre`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepSpec {
    pub id: String,
    #[serde(default)]
    pub add_shells: Vec<String>,
    /// Centres of this step (empty = the rule's centres).
    #[serde(default)]
    pub centres: Vec<CentreSpec>,
    /// Keep the rule's centres next to each new centre (the K* shape: key + line 0 down).
    #[serde(default = "yes")]
    pub keep_base_centre: bool,
    /// One rule (= one job) per resolved centre.
    #[serde(default = "yes")]
    pub per_centre: bool,
}

/// TRICKS 6: S2 + S2m1 first ({0,1,4}, {0}/{9,10}), then shifted centres ({0}/{9,10,12,16}: line 0 down, kept with the
/// key centre = t162-handdown round 1: 126 / 252 keys vs 0 / 252 with the key centre alone).
pub fn default_escalation() -> Vec<StepSpec> {
    vec![
        StepSpec {
            id: "shells_S2_S2m1".into(),
            add_shells: vec!["S2".into(), "S2m1".into()],
            centres: vec![],
            keep_base_centre: true,
            per_centre: false,
        },
        StepSpec {
            id: "shifted_centres".into(),
            add_shells: vec![],
            centres: vec![
                CentreSpec::token("dotted_line_down"),
                CentreSpec::token("numerator_up"),
            ],
            keep_base_centre: true,
            per_centre: true,
        },
    ]
}

/// TRICKS 13: the up-centre (a dotted line at its lowest power, one step up) cures the spurious d = 4 pole on that
/// locus ({0,1,3,7}: regular at 75 / 75, same size); the rule's shells; the key centre NOT kept (the union with the key
/// centre stayed singular there).
pub fn default_regular_escalation() -> Vec<StepSpec> {
    vec![StepSpec {
        id: "up_centres".into(),
        add_shells: vec![],
        centres: vec![CentreSpec::token("line_up_at_lowest")],
        keep_base_centre: false,
        per_centre: true,
    }]
}

/// TRICKS 14 (a): after the first step that yields a relation at a key, a CENTRE SCAN over cheap candidates (each
/// dotted line one step down; each dotted line at its lowest power one step up), with the shells of the step that
/// solved, BEFORE a large or singular relation is accepted as the key's choice; candidates are compared by `select`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CentreScanSpec {
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "d_scan_centres")]
    pub centres: Vec<CentreSpec>,
    /// Keep the step's own centres next to each scan centre (item 14: the shifted centre alone).
    #[serde(default)]
    pub keep_base_centre: bool,
    /// Trigger: the key's best relation has more right-hand-side terms than this.
    #[serde(default = "d_scan_min_terms")]
    pub min_terms: u64,
    /// Trigger: the key's best relation is not regular at d0 (mode regular_preferred).
    #[serde(default = "yes")]
    pub on_singular: bool,
    /// Budget: scan candidates per key (in order) and scan jobs per signature group.
    #[serde(default = "d_scan_per_key")]
    pub max_candidates_per_key: usize,
    #[serde(default = "d_scan_per_face")]
    pub max_jobs_per_face: usize,
}

impl Default for CentreScanSpec {
    fn default() -> Self {
        CentreScanSpec {
            enabled: true,
            centres: d_scan_centres(),
            keep_base_centre: false,
            min_terms: d_scan_min_terms(),
            on_singular: true,
            max_candidates_per_key: d_scan_per_key(),
            max_jobs_per_face: d_scan_per_face(),
        }
    }
}

/// TRICKS 14 (a) (dotted line down: {0,1,3,4} 39 seeds / 499-585 terms vs 156 / 2,091-2,297), 13 (up-centres: the
/// spurious pole on a lowest-power locus) and 15 (numerators one step up, alone and in pairs: on {0}/{9,12} the only
/// regular relation comes from n9 and n12 both one step up).
fn d_scan_centres() -> Vec<CentreSpec> {
    vec![
        CentreSpec::token("dotted_line_down"),
        CentreSpec::token("line_up_at_lowest"),
        CentreSpec::token("numerator_up"),
        CentreSpec::token("numerator_pairs_up"),
    ]
}
/// Provisional (item 14: 2,091-2,297 terms from the bigger shell vs 499-585 from a shifted centre); to be set from
/// the T162 run of this lane.
fn d_scan_min_terms() -> u64 {
    1_000
}
fn d_scan_per_key() -> usize {
    8
}
fn d_scan_per_face() -> usize {
    16
}

/// Joint minimisation over all core keys of a signature (TRICKS 7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointSpec {
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "d_joint_centres")]
    pub centres: Vec<CentreSpec>,
    #[serde(default = "yes")]
    pub keep_base_centre: bool,
    /// `rule` | `rule+escalation` (the rule's shells plus every escalation step's added shells).
    #[serde(default = "d_joint_shells")]
    pub shells: String,
    /// Stop after the first centre whose list is certified at every core key.
    #[serde(default = "yes")]
    pub stop_at_first_full: bool,
    #[serde(default = "d_joint_max")]
    pub max_centres: usize,
    /// Keys that failed the latest trials are solved first (cost only: the decision is "certified at every key").
    #[serde(default = "d_witness")]
    pub witness_keys: usize,
}

impl Default for JointSpec {
    fn default() -> Self {
        JointSpec {
            enabled: true,
            centres: d_joint_centres(),
            keep_base_centre: true,
            shells: d_joint_shells(),
            stop_at_first_full: true,
            max_centres: d_joint_max(),
            witness_keys: d_witness(),
        }
    }
}

/// TRICKS 7 ({0}/{9,12}: shifted centre: dotted line one step down, numerator one step up; 156 -> 48 seeds).
fn d_joint_centres() -> Vec<CentreSpec> {
    vec![
        CentreSpec::token("dotted_line_down"),
        CentreSpec::token("numerator_up"),
    ]
}
fn d_joint_shells() -> String {
    "rule".into()
}
fn d_joint_max() -> usize {
    3
}
fn d_witness() -> usize {
    2
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapsSpec {
    /// A rule whose explicit list at a key is larger is not solved (typed `over_seed_cap`).
    #[serde(default = "d_max_seeds")]
    pub max_seeds: usize,
    /// A candidate rule (C) with more right-hand-side terms is reported `within_caps: false`.
    #[serde(default = "d_rule_terms")]
    pub rule_max_terms: u64,
}

impl Default for CapsSpec {
    fn default() -> Self {
        CapsSpec {
            max_seeds: d_max_seeds(),
            rule_max_terms: d_rule_terms(),
        }
    }
}

fn d_max_seeds() -> usize {
    20_000
}
fn d_rule_terms() -> u64 {
    2_000
}

/// The whole policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreSeedPolicy {
    #[serde(default = "d_schema")]
    pub schema: String,
    /// `regular_preferred` | `any_relation`.
    #[serde(default = "d_mode_rel")]
    pub mode: String,
    #[serde(default = "d_d0")]
    pub d0: String,
    #[serde(default)]
    pub rule: RuleSpec,
    #[serde(default)]
    pub minimise: MinimiseSpec,
    #[serde(default)]
    pub domain: DomainSpec,
    #[serde(default = "default_escalation")]
    pub escalation: Vec<StepSpec>,
    #[serde(default = "default_regular_escalation")]
    pub regular_escalation: Vec<StepSpec>,
    #[serde(default)]
    pub centre_scan: CentreScanSpec,
    #[serde(default)]
    pub joint: JointSpec,
    #[serde(default)]
    pub caps: CapsSpec,
    /// `smallest` (lane poles' selection replaces it).
    #[serde(default = "d_select")]
    pub select: String,
}

fn d_schema() -> String {
    POLICY_SCHEMA.into()
}
fn d_mode_rel() -> String {
    "regular_preferred".into()
}
fn d_d0() -> String {
    "4".into()
}
fn d_select() -> String {
    "smallest".into()
}

impl Default for CoreSeedPolicy {
    fn default() -> Self {
        serde_json::from_value(Value::Object(Default::default())).expect("defaults")
    }
}

fn check_centres(c: &[CentreSpec], what: &str) -> Result<(), String> {
    for x in c {
        if let CentreSpec::Token(t) = x
            && !CENTRE_TOKENS.contains(&t.as_str())
        {
            return Err(format!(
                "{what}: centre token {t:?} not in {CENTRE_TOKENS:?}"
            ));
        }
    }
    Ok(())
}

fn check_shells(s: &[String], what: &str) -> Result<(), String> {
    for x in s {
        crate::termmin::steps::parse_shell(x).map_err(|e| format!("{what}: {e}"))?;
    }
    Ok(())
}

impl CoreSeedPolicy {
    pub fn parse(text: &str) -> Result<CoreSeedPolicy, String> {
        let p: CoreSeedPolicy =
            serde_json::from_str(text).map_err(|e| format!("coreseed policy: {e}"))?;
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
        if self.schema != POLICY_SCHEMA {
            return Err(format!("schema {} (want {POLICY_SCHEMA})", self.schema));
        }
        pick(&self.mode, &["regular_preferred", "any_relation"], "mode")?;
        pick(&self.rule.measure, &["comparator", "m1"], "rule.measure")?;
        pick(
            &self.minimise.backend,
            &["emulated", "engine"],
            "minimise.backend",
        )?;
        pick(
            &self.minimise.mode,
            &["greedy", "support", "off"],
            "minimise.mode",
        )?;
        pick(
            &self.minimise.order,
            &["distance", "outermost_first", "given"],
            "minimise.order",
        )?;
        pick(
            &self.minimise.measure,
            &["comparator", "m1"],
            "minimise.measure",
        )?;
        pick(
            &self.domain.pivot,
            &["highest_dr", "lowest_dr", "first"],
            "domain.pivot",
        )?;
        pick(
            &self.joint.shells,
            &["rule", "rule+escalation"],
            "joint.shells",
        )?;
        pick(&self.select, &["smallest"], "select")?;
        if self.rule.centres.is_empty() || self.rule.shells.is_empty() {
            return Err("rule: centres and shells must not be empty".into());
        }
        check_centres(&self.rule.centres, "rule")?;
        check_shells(&self.rule.shells, "rule")?;
        for s in self.escalation.iter().chain(&self.regular_escalation) {
            check_centres(&s.centres, &s.id)?;
            check_shells(&s.add_shells, &s.id)?;
            if s.add_shells.is_empty() && s.centres.is_empty() {
                return Err(format!("step {}: neither shells nor centres", s.id));
            }
        }
        check_centres(&self.joint.centres, "joint")?;
        check_centres(&self.centre_scan.centres, "centre_scan")?;
        if self.minimise.parallel_trials == 0 || self.domain.max_lists == 0 {
            return Err("parallel_trials and max_lists must be >= 1".into());
        }
        if self.d0.parse::<i64>().is_err() {
            return Err(format!("d0 {:?} is not an integer", self.d0));
        }
        Ok(())
    }

    /// The engine relation rule of the rule's measure.
    pub fn relation_rule(&self) -> &'static str {
        if self.rule.measure == "m1" {
            "m1_first"
        } else {
            "first_admissible"
        }
    }

    /// sha256 of the canonical JSON of the policy (defaults filled in).
    pub fn digest(&self) -> String {
        let v = serde_json::to_value(self).unwrap_or(Value::Null);
        let t = crate::schema::canon::canonical_json(&v, false).unwrap_or_default();
        let mut h = crate::closure::sha256::Sha256::new();
        h.update(t.as_bytes());
        h.finish_hex()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_round_trip() {
        let p = CoreSeedPolicy::default();
        p.validate().unwrap();
        assert_eq!(p.rule.shells.len(), 6);
        assert_eq!(p.minimise.order, "distance");
        assert_eq!(p.relation_rule(), "first_admissible");
        let t = serde_json::to_string(&p).unwrap();
        assert_eq!(CoreSeedPolicy::parse(&t).unwrap(), p);
        assert_eq!(CoreSeedPolicy::parse("{}").unwrap().digest(), p.digest());
        assert!(CoreSeedPolicy::parse(r#"{"mode":"x"}"#).is_err());
        assert!(CoreSeedPolicy::parse(r#"{"rule":{"centres":["nowhere"]}}"#).is_err());
        assert!(CoreSeedPolicy::parse(r#"{"rule":{"shells":["S9"]}}"#).is_err());
        assert!(CoreSeedPolicy::parse(r#"{"unknown":1}"#).is_err());
        let q = CoreSeedPolicy::parse(r#"{"rule":{"centres":["key",[0,-1,0]]}}"#).unwrap();
        assert_eq!(q.rule.centres[1], CentreSpec::Offset(vec![0, -1, 0]));
    }
}
