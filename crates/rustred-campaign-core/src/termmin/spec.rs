//! The terminal-minimisation spec `rustred.campaign.termmin.v1` (interfaces/termmin.md section 2): the escalation
//! sequence per terminal, the resource caps of every solve, the per-terminal work budget and the objective, all DATA.
//! Nothing here knows a node, an arity or a count (L8): the built-in sequence is a list of step NAMES.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::import::recipe_steps::{Step, parse_step};
use crate::import::symmetry::GroupSpec;

pub const TERMMIN_SCHEMA: &str = "rustred.campaign.termmin.v1";
pub const REPORT_SCHEMA: &str = "rustred.campaign.termmin-report.v1";
/// The only objective implemented (user correction 2026-11-14T15:08:50Z): fewest terminal keys, then smaller closure.
pub const OBJECTIVE: &str = "fewest_terminals_then_smaller_closure";

/// The built-in escalation sequence (the brief's order; `dispatch` is a free first check).
pub fn default_sequence() -> Vec<String> {
    [
        "dispatch",
        "symmetry_relation",
        "d0_cnone_foff_ssym_mM1C",
        "d2_clast_fon_mM1C",
        "d2_cunion_fon_mM1C",
        // the MIXED SHELL (hundreds of seeds; before depth 3): helper K15 finding 2026-11-14T22:00Z
        "mixed_shell",
        "d3_cfirst_foff_mM1C",
        "d3_cunion_fon_mM1C",
        "d4_cfirst_foff_mM1C",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn yes() -> bool {
    true
}

/// The MIXED-SHELL step (`mixed_shell` in the sequence): explicit seeds around centres {the key} and, for each dotted
/// line of the key, that line one step down, in shell families of the engine's `harder` unit u[j] (+1 on a line, -1
/// on a numerator; the helper's `face.py unit()`, although its seeds_Kstar.json text says "+1 on every slot"):
/// `S<p>m<q>` = the centre + p raised slots (multiset) - q lowered slots (multiset), raised and lowered slots
/// disjoint; `S0` the centre (helper K15 `face.py shells`). Seeds outside the chart's top sector (a line
/// <= 0 or a numerator > 0) are dropped; no floor. Written as an explicit offset list (one centre per seed, shell S0).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct MixedShell {
    pub enabled: bool,
    pub shells: Vec<String>,
    /// The relation rule asked (`m1_first`: the engine's default; its fallback is the comparator relation).
    pub relation_rule: String,
}

impl Default for MixedShell {
    fn default() -> Self {
        MixedShell {
            enabled: true,
            shells: ["S0", "S1", "Sm1", "S1m1", "S1m2", "Sm2"]
                .iter()
                .map(|x| x.to_string())
                .collect(),
            relation_rule: "m1_first".into(),
        }
    }
}
fn objective() -> String {
    OBJECTIVE.to_string()
}

/// Resource caps of every solve of the pass (the same caps and typed refusals as other recipe solves).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Caps {
    /// A recipe step whose explicit seed set is larger is skipped (`over_seed_cap`).
    pub max_seeds: u64,
    /// The verb's deterministic work limit (`--max-trace-rows`, verdict BUDGET_WORK).
    pub max_trace_rows: Option<u64>,
    /// The verb's per-key wall budget (`--key-budget-s`, verdict BUDGET_WALL, time-decided).
    pub key_budget_s: Option<u64>,
    pub threads: u32,
    /// Memory cap of one solve child (a child over it is killed: typed refusal).
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    /// Bound of one expanded symmetry row (`symmetry_relation`).
    pub symmetry_max_terms: usize,
}

impl Default for Caps {
    fn default() -> Self {
        Caps {
            max_seeds: 12_000,
            max_trace_rows: None,
            // directing session 2026-11-14 (lane closeloop3): depth 3 decides in 13-19 min per key; 540 s was too short
            key_budget_s: Some(1800),
            threads: 1,
            cap_bytes: 2 << 30,
            expected_peak_bytes: 1 << 30,
            symmetry_max_terms: 1 << 16,
        }
    }
}

/// The deterministic work budget, PER TERMINAL (independent of thread count and completion order).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Budget {
    pub max_steps_per_terminal: usize,
    /// Sum of the explicit seed counts of the recipe steps issued for one terminal.
    pub max_seeds_per_terminal: u64,
    pub max_closing_tries_per_terminal: u32,
    /// A relation that leaves more unserved keys than this is discarded without a closing try.
    pub max_closing_keys: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Budget {
            max_steps_per_terminal: 16,
            max_seeds_per_terminal: 40_000,
            max_closing_tries_per_terminal: 1,
            max_closing_keys: 64,
        }
    }
}

/// The spec (every field optional; unknown fields refused).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TermminSpec {
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "default_sequence")]
    pub sequence: Vec<String>,
    /// Keys never attempted (added to the import spec's preferred terminals).
    #[serde(default)]
    pub preferred: Vec<Vec<i32>>,
    #[serde(default)]
    pub caps: Caps,
    #[serde(default)]
    pub budget: Budget,
    #[serde(default = "objective")]
    pub objective: String,
    /// Line priority for the centre rules `first` / `last` (null: the node order's coordinate priority, else slot order).
    #[serde(default)]
    pub line_priority: Option<Vec<usize>>,
    #[serde(default)]
    pub numerator_priority: Option<Vec<usize>>,
    /// The symmetry group of the `symmetry_relation` step (null: derived from the frame table header's family).
    #[serde(default)]
    pub group: Option<GroupSpec>,
    /// Per-node overrides (any field above, objects `caps` / `budget` merged field by field).
    #[serde(default)]
    pub nodes: BTreeMap<String, Value>,
    /// The mixed-shell step's data (on by default).
    #[serde(default)]
    pub mixed_shell: MixedShell,
    /// TESTS ONLY: contract `RecipeQuirk` names applied to the FAKE child of phase E's solves (used only when the node's
    /// solve configuration runs the fake child; a real engine never sees them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fake_quirks: Vec<String>,
    /// The objective of the `choice` annotation of an elimination ([`crate::poles::PoleObjective::setting`]: absent =
    /// default, `"none"` = no annotation, an object = its measures / guard; audit r2-poles F4). Omitted from the
    /// canonical spec when absent (digests unchanged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pole_objective: Option<Value>,
    /// lane wireflow (h, TRICKS 9 / 17): the engine's per-key pole order at d = 4 of the DELIVERED relation
    /// (`pole_order_d4` of the solved line, frozen engine line7d on) is the elimination choice's pole profile; false
    /// (default, omitted from the canonical spec) = "no profile" (ranked by size, as before).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pole_profiles: bool,
}

impl Default for TermminSpec {
    fn default() -> Self {
        TermminSpec {
            schema: Some(TERMMIN_SCHEMA.into()),
            enabled: true,
            sequence: default_sequence(),
            preferred: vec![],
            caps: Caps::default(),
            budget: Budget::default(),
            objective: OBJECTIVE.into(),
            line_priority: None,
            numerator_priority: None,
            group: None,
            nodes: BTreeMap::new(),
            mixed_shell: MixedShell::default(),
            fake_quirks: vec![],
            pole_objective: None,
            pole_profiles: false,
        }
    }
}

/// One step of the escalation sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum StepKind {
    /// The in-force dispatch at the terminal with the terminal NOT declared terminal.
    Dispatch,
    /// The exact relation from the verified group's symmetry rows at the terminal alone.
    SymmetryRelation,
    /// A point recipe (import.md 14 step grammar) solved at the terminal.
    Recipe(Step),
    /// The mixed-shell point recipe ([`MixedShell`]).
    MixedShell,
}

impl StepKind {
    pub fn word(&self) -> &'static str {
        match self {
            StepKind::Dispatch => "dispatch",
            StepKind::SymmetryRelation => "symmetry_relation",
            StepKind::Recipe(_) => "recipe",
            StepKind::MixedShell => "mixed_shell",
        }
    }
}

impl TermminSpec {
    /// Parse and validate a spec file's text.
    pub fn parse(text: &str) -> Result<TermminSpec, String> {
        let s: TermminSpec =
            serde_json::from_str(text).map_err(|e| format!("termmin spec: {e}"))?;
        s.validate()?;
        Ok(s)
    }

    /// Schema, objective and every step name (and every per-node override) checked.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(s) = &self.schema
            && s != TERMMIN_SCHEMA
        {
            return Err(format!(
                "termmin spec: schema {s:?}, expected {TERMMIN_SCHEMA}"
            ));
        }
        if self.objective != OBJECTIVE {
            return Err(format!(
                "termmin spec: objective {:?} (only {OBJECTIVE:?} is implemented)",
                self.objective
            ));
        }
        self.steps()?;
        self.objective_setting()?;
        for n in self.nodes.keys() {
            self.for_node(n)?;
        }
        Ok(())
    }

    /// The pole objective of the elimination's `choice` annotation (`None` = off: no annotation).
    pub fn objective_setting(&self) -> Result<Option<crate::poles::PoleObjective>, String> {
        crate::poles::PoleObjective::setting(self.pole_objective.as_ref())
            .map_err(|e| format!("termmin spec: {e}"))
    }

    /// The parsed escalation sequence.
    pub fn steps(&self) -> Result<Vec<StepKind>, String> {
        if self.sequence.is_empty() {
            return Err("termmin spec: empty sequence".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut out = vec![];
        for s in &self.sequence {
            if !seen.insert(s.clone()) {
                return Err(format!("termmin spec: step {s:?} listed twice"));
            }
            out.push(match s.as_str() {
                "dispatch" => StepKind::Dispatch,
                "symmetry_relation" => StepKind::SymmetryRelation,
                "mixed_shell" => {
                    for sh in &self.mixed_shell.shells {
                        super::steps::parse_shell(sh).map_err(|e| format!("termmin spec: {e}"))?;
                    }
                    StepKind::MixedShell
                }
                x => StepKind::Recipe(parse_step(x).map_err(|e| format!("termmin spec: {e}"))?),
            });
        }
        Ok(out)
    }

    /// The spec of one node: the global fields overlaid with `nodes.<node>` (objects `caps` / `budget` field by field).
    pub fn for_node(&self, node: &str) -> Result<TermminSpec, String> {
        let Some(o) = self.nodes.get(node) else {
            let mut s = self.clone();
            s.nodes.clear();
            return Ok(s);
        };
        let mut base = serde_json::to_value(self).map_err(|e| e.to_string())?;
        let obj = base.as_object_mut().ok_or("spec is not an object")?;
        obj.remove("nodes");
        let ov = o
            .as_object()
            .ok_or_else(|| format!("termmin spec: nodes.{node} is not an object"))?;
        for (k, v) in ov {
            if k == "nodes" {
                return Err(format!("termmin spec: nodes.{node}.nodes"));
            }
            match (obj.get_mut(k), v) {
                (Some(Value::Object(dst)), Value::Object(src)) if k == "caps" || k == "budget" => {
                    for (kk, vv) in src {
                        dst.insert(kk.clone(), vv.clone());
                    }
                }
                _ => {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        let s: TermminSpec =
            serde_json::from_value(base).map_err(|e| format!("termmin spec (node {node}): {e}"))?;
        if s.objective != OBJECTIVE {
            return Err(format!(
                "termmin spec (node {node}): objective {:?}",
                s.objective
            ));
        }
        s.steps()?;
        s.objective_setting()?;
        Ok(s)
    }

    /// `sha256:` of the canonical JSON (recorded in every report).
    pub fn digest(&self) -> String {
        let v = serde_json::to_value(self).unwrap_or(Value::Null);
        let c = crate::schema::canon::canonical_json(&v, false).unwrap_or_default();
        let mut h = crate::closure::sha256::Sha256::new();
        h.update(c.as_bytes());
        format!("sha256:{}", h.finish_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_spec_parses_and_overrides_merge() {
        let s = TermminSpec::parse("{}").unwrap();
        assert!(s.enabled);
        assert_eq!(s.steps().unwrap().len(), 9);
        let t = TermminSpec::parse(
            r#"{"nodes":{"N":{"enabled":false,"caps":{"max_seeds":5}}},"caps":{"threads":2}}"#,
        )
        .unwrap();
        let n = t.for_node("N").unwrap();
        assert!(!n.enabled);
        assert_eq!((n.caps.max_seeds, n.caps.threads), (5, 2));
        assert!(t.for_node("M").unwrap().enabled);
        assert!(TermminSpec::parse(r#"{"what":1}"#).is_err());
        assert!(TermminSpec::parse(r#"{"sequence":["d2_cxx"]}"#).is_err());
        assert!(TermminSpec::parse(r#"{"objective":"fewest_recipes"}"#).is_err());
        assert!(TermminSpec::parse(r#"{"sequence":["dispatch","dispatch"]}"#).is_err());
    }
}
