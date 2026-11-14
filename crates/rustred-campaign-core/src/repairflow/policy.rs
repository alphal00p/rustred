//! The REPAIR POLICY (interfaces/repairflow.md section 2): every threshold, budget, escalation step, weight and
//! tie-break of the table-vs-recipe decision is a named field here, with a documented default; no constant in the code
//! decides. Effective policy = built-in defaults <- campaign `repair_policy` <- `nodes.<node>` (deep merge of JSON
//! objects; unknown fields refused). Nothing here knows a node, an arity or a count (L8).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Schema of the policy file (campaign input role `repairflow`).
pub const POLICY_SCHEMA: &str = "rustred.campaign.repairflow.v1";

/// The policy of one node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RepairPolicy {
    /// false = the flow is off for the node (the closing loop alone).
    pub enabled: bool,
    /// Shadowing test before any symbolic search (R31 evidence 04:37Z).
    pub shadow_test: bool,
    pub symbolic: SymbolicPolicy,
    pub keep: KeepPolicy,
    pub node_budget: NodeBudget,
    pub cost: CostModel,
    pub recipes: RecipePolicy,
    pub proposals: ProposalPolicy,
}

impl Default for RepairPolicy {
    fn default() -> Self {
        RepairPolicy {
            enabled: true,
            shadow_test: true,
            symbolic: SymbolicPolicy::default(),
            keep: KeepPolicy::default(),
            node_budget: NodeBudget::default(),
            cost: CostModel::default(),
            recipes: RecipePolicy::default(),
            proposals: ProposalPolicy::default(),
        }
    }
}

/// Step S: the cheap symbolic attempt per face (one generate job).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SymbolicPolicy {
    /// false = NEVER TABLES (every face goes to recipes).
    pub enabled: bool,
    /// `--max-depth` of the face search (shells around each centre).
    pub max_depth: u32,
    /// Centre of the face search: `last_line_floored` | `first_line_floored` | `target` | `numerator` |
    /// `union_floored`.
    pub centre: String,
    /// `--numeric-depth` (per-point search of point faces).
    pub numeric_depth: u32,
    /// Wall cap of one face attempt (s; the time-decided safety net: `budgets.wall.case_s`).
    pub wall_case_s: u64,
    /// The lift PRE-SCREEN (`--screen R,P,D,T --on-heavy refuse`): a relation over these is refused BEFORE its exact
    /// lift (deterministic modular replay of the trace).
    pub screen: Screen,
    /// Cap of faces tried symbolically (in `face_order`); null = all.
    pub max_faces: Option<usize>,
    /// `open_keys_desc` | `open_keys_asc` | `label`.
    pub face_order: String,
    pub threads: u32,
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    pub heartbeat_s: u32,
    pub stop_grace_s: u32,
    /// Extra generate arguments (each must pass the contract's allow-list).
    pub extra_args: Vec<String>,
}

impl Default for SymbolicPolicy {
    fn default() -> Self {
        SymbolicPolicy {
            enabled: true,
            max_depth: 2,
            centre: "last_line_floored".into(),
            numeric_depth: 2,
            wall_case_s: 120,
            screen: Screen::default(),
            max_faces: None,
            face_order: "open_keys_desc".into(),
            threads: 1,
            cap_bytes: 2 << 30,
            expected_peak_bytes: 1 << 30,
            heartbeat_s: 10,
            stop_grace_s: 30,
            extra_args: vec![],
        }
    }
}

/// The engine's lift pre-screen `MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Screen {
    pub min_rows: u64,
    pub max_points: u64,
    pub max_degree: u64,
    pub max_rhs_terms: u64,
}

impl Default for Screen {
    fn default() -> Self {
        Screen {
            min_rows: 0,
            max_points: 64,
            max_degree: 24,
            max_rhs_terms: 2500,
        }
    }
}

impl Screen {
    /// The `--screen` argument.
    pub fn arg(&self) -> String {
        format!(
            "{},{},{},{}",
            self.min_rows, self.max_points, self.max_degree, self.max_rhs_terms
        )
    }
}

/// Per-rule caps and the keys a kept rule must serve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct KeepPolicy {
    pub max_rhs_terms: u64,
    pub max_monomials: u64,
    pub max_bytes: u64,
    pub max_trace_rows: u64,
    /// A rule credited at fewer open keys of its face goes to recipes.
    pub min_keys_served: u64,
    /// ... or at a smaller share of the face's open keys.
    pub min_served_fraction: f64,
}

impl Default for KeepPolicy {
    fn default() -> Self {
        KeepPolicy {
            max_rhs_terms: 2500,
            max_monomials: 200_000,
            max_bytes: 2 << 20,
            max_trace_rows: 5000,
            min_keys_served: 2,
            min_served_fraction: 0.0,
        }
    }
}

/// Per-node budget of NEW symbolic rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct NodeBudget {
    pub max_bytes: u64,
    pub max_rules: u64,
    /// Which candidates get the budget first: `keys_per_byte` | `keys` | `bytes_asc`.
    pub order: String,
}

impl Default for NodeBudget {
    fn default() -> Self {
        NodeBudget {
            max_bytes: 64 << 20,
            max_rules: 1000,
            order: "keys_per_byte".into(),
        }
    }
}

/// The crossover cost model (section 3 of the interface).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CostModel {
    pub enabled: bool,
    pub load_ns_per_byte: f64,
    pub apply_ns_base: f64,
    pub apply_ns_per_monomial: f64,
    pub recipe_solve_s: f64,
    pub cached_ns_base: f64,
    pub cached_ns_per_rhs_term: f64,
    pub cache_hit_fraction: f64,
    pub uses_per_served_key: f64,
    pub table_weight: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        CostModel {
            enabled: true,
            load_ns_per_byte: 30.0,
            apply_ns_base: 20_000.0,
            apply_ns_per_monomial: 100.0,
            recipe_solve_s: 12.0,
            cached_ns_base: 20_000.0,
            cached_ns_per_rhs_term: 2_000.0,
            cache_hit_fraction: 0.9,
            uses_per_served_key: 1.0,
            table_weight: 1.0,
        }
    }
}

/// Step R: the recipe escalation per face.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RecipePolicy {
    /// false = NEVER RECIPES from this flow (the closing loop's default recipe still applies).
    pub enabled: bool,
    /// false = no per-face recipes, only the default recipe (R49).
    pub per_face: bool,
    /// Step names (import.md 14 grammar), tried in order per key.
    pub escalation: Vec<String>,
    /// Centre choice priorities; null = the node order's coordinate priority.
    pub line_priority: Option<Vec<usize>>,
    pub numerator_priority: Option<Vec<usize>>,
    /// Cap of faces given per-face recipes (in `symbolic.face_order`); null = all.
    pub max_faces: Option<usize>,
}

impl Default for RecipePolicy {
    fn default() -> Self {
        RecipePolicy {
            enabled: true,
            per_face: true,
            escalation: vec![
                "d0_cnone_foff_ssym_mM1C".into(),
                "d2_clast_fon_mM1C".into(),
                "d2_cunion_fon_mM1C".into(),
                "d3_clast_fon_mM1C".into(),
            ],
            line_priority: None,
            numerator_priority: None,
            max_faces: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ProposalPolicy {
    pub enabled: bool,
}

impl Default for ProposalPolicy {
    fn default() -> Self {
        ProposalPolicy { enabled: true }
    }
}

/// The policy FILE: campaign section + per-node overrides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyFile {
    pub schema: String,
    #[serde(default)]
    pub repair_policy: Value,
    #[serde(default)]
    pub nodes: BTreeMap<String, Value>,
}

/// Deep merge of JSON objects (`over` wins; arrays and scalars are replaced).
pub fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(bv) if bv.is_object() && v.is_object() => merge(bv, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

impl PolicyFile {
    pub fn parse(text: &str) -> Result<PolicyFile, String> {
        let f: PolicyFile = serde_json::from_str(text).map_err(|e| format!("repair policy: {e}"))?;
        if f.schema != POLICY_SCHEMA {
            return Err(format!(
                "repair policy: schema {:?}, expected {POLICY_SCHEMA}",
                f.schema
            ));
        }
        // every section must deserialize on its own (unknown fields refused early, per node too)
        f.effective(None)?;
        for n in f.nodes.keys() {
            f.effective(Some(n))?;
        }
        Ok(f)
    }

    /// The effective policy of `node` (None = the campaign section only).
    pub fn effective(&self, node: Option<&str>) -> Result<RepairPolicy, String> {
        let mut v = serde_json::to_value(RepairPolicy::default()).expect("policy json");
        if !self.repair_policy.is_null() {
            merge(&mut v, &self.repair_policy);
        }
        if let Some(n) = node
            && let Some(o) = self.nodes.get(n)
        {
            merge(&mut v, o);
        }
        let p: RepairPolicy = serde_json::from_value(v).map_err(|e| {
            format!(
                "repair policy{}: {e}",
                node.map(|n| format!(" of node {n}")).unwrap_or_default()
            )
        })?;
        p.validate()?;
        Ok(p)
    }
}

impl RepairPolicy {
    /// Sanity of the values (a usage error, never a silent clamp).
    pub fn validate(&self) -> Result<(), String> {
        let frac = |x: f64, what: &str| {
            if (0.0..=1.0).contains(&x) {
                Ok(())
            } else {
                Err(format!("repair policy: {what} = {x} not in [0, 1]"))
            }
        };
        frac(self.keep.min_served_fraction, "keep.min_served_fraction")?;
        frac(self.cost.cache_hit_fraction, "cost.cache_hit_fraction")?;
        for (x, what) in [
            (self.cost.load_ns_per_byte, "cost.load_ns_per_byte"),
            (self.cost.apply_ns_base, "cost.apply_ns_base"),
            (self.cost.apply_ns_per_monomial, "cost.apply_ns_per_monomial"),
            (self.cost.recipe_solve_s, "cost.recipe_solve_s"),
            (self.cost.cached_ns_base, "cost.cached_ns_base"),
            (self.cost.cached_ns_per_rhs_term, "cost.cached_ns_per_rhs_term"),
            (self.cost.uses_per_served_key, "cost.uses_per_served_key"),
            (self.cost.table_weight, "cost.table_weight"),
        ] {
            if !x.is_finite() || x < 0.0 {
                return Err(format!("repair policy: {what} = {x} must be finite and >= 0"));
            }
        }
        if !matches!(
            self.symbolic.centre.as_str(),
            "last_line_floored" | "first_line_floored" | "target" | "numerator" | "union_floored"
        ) {
            return Err(format!(
                "repair policy: symbolic.centre {:?} unknown",
                self.symbolic.centre
            ));
        }
        if !matches!(
            self.symbolic.face_order.as_str(),
            "open_keys_desc" | "open_keys_asc" | "label"
        ) {
            return Err(format!(
                "repair policy: symbolic.face_order {:?} unknown",
                self.symbolic.face_order
            ));
        }
        if !matches!(
            self.node_budget.order.as_str(),
            "keys_per_byte" | "keys" | "bytes_asc"
        ) {
            return Err(format!(
                "repair policy: node_budget.order {:?} unknown",
                self.node_budget.order
            ));
        }
        if self.symbolic.wall_case_s == 0 {
            return Err("repair policy: symbolic.wall_case_s must be > 0".into());
        }
        for s in &self.recipes.escalation {
            crate::import::recipe_steps::parse_step(s)
                .map_err(|e| format!("repair policy: recipes.escalation: {e}"))?;
        }
        Ok(())
    }

    /// Canonical JSON of the effective policy (recorded in every decision record).
    pub fn canonical(&self) -> String {
        let v = serde_json::to_value(self).expect("policy json");
        crate::schema::canon::canonical_json(&v, true).unwrap_or_else(|_| v.to_string())
    }

    /// `fnv1a128` digest of [`Self::canonical`] (hex): the policy identity in records and the statement header.
    pub fn digest(&self) -> String {
        format!(
            "{:032x}",
            crate::worklist::items::fnv1a128(self.canonical().as_bytes())
        )
    }
}
