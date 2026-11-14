//! RECIPE RULES (R48 and its addenda; engine verb interfaces/coord-recipe-solve.md v1, implemented): a [`Recipe`] IS
//! the engine's recipe file `rustred.recipe.v1` (section 3 there), field for field: the domain, the seed set at the key
//! stated EXPLICITLY (centres as offset vectors, the generator, the floored slots NAMED, the sector-filter slots NAMED,
//! layer cap; R48 addendum 2: nothing refers to which slots were symbolic in a case), symmetry rows and the elimination
//! convention, plus an optional `provenance` (node, node order, setting: echoed and part of the engine's digest). NO
//! coefficients. This is the ONE recipe definition of the crate (re-audit A M6): the ladder writes it, the acceptance
//! lane wraps it ([`Recipe::to_acceptance`]: its record's solve recipe is this file), and dispatch matches through the
//! acceptance lane's [`crate::accept::recipe::RecipeEntry`] (in the domain AND certified, else fall through).
//!
//! [`CaseSolver::solve_recipe_at_key`](crate::repair::CaseSolver::solve_recipe_at_key) is the trait face of the verb. A
//! [`RecipeRule`] (private fields, Serialize only) is accepted PER KEY over every key it is credited with; the
//! determinism id and the engine's recipe / binary digests of every certified answer must agree. Recipes take over
//! only after the node's full attempt ended ([`FullAttemptEnded`], R35 order) and only after the item's own cheap
//! symbolic attempt ran (R48 addendum 3 (a)). Nothing node-specific (rule L8).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::items::{
    CaseSpec, EvidenceLevel, Face, FullAttemptEnded, Item, Key, NodeParams, fnv1a128,
};
use super::ladder::{ItemState, Tri};
use super::seed::{CentreKind, SeedSetting};
use crate::domain::CaseDomain;
use crate::repair::{CaseSolver, Descent};

pub const RECIPE_SCHEMA: &str = "rustred.recipe.v1";
/// Elimination convention id (coord-recipe-solve.md section 4).
pub const RECIPE_ELIMINATION: &str = "rustred.recipe-elim.v1";
/// The seed unit of the engine's file: one step = +1 on a line, -1 on a numerator.
pub const RECIPE_UNIT: &str = "harder";

/// Optional per-slot box of the domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeBounds {
    pub min: Vec<Option<i32>>,
    pub max: Vec<Option<i32>>,
}

/// The recipe's domain (presented slots): fixed values (`None` = free; a fixed slot must equal the key), index
/// polynomials that must vanish, exception branches (OR of AND-lists: the key is OUTSIDE if one branch vanishes), an
/// optional per-slot box.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeDomain {
    pub fixed: Vec<Option<i32>>,
    #[serde(default)]
    pub equations: Vec<String>,
    #[serde(default)]
    pub exceptions: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<RecipeBounds>,
}

/// The L1 ball around each centre: up-moves on every slot, down-moves only on the NAMED slots.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeBall {
    pub radius: u8,
    pub down: Vec<usize>,
}

/// The seed generator around each centre: the checker's named shells in order, or an L1 ball.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeGenerator {
    Shells(Vec<String>),
    Ball(RecipeBall),
}

/// The seed set at a key, EXPLICIT (R48 addendum 2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeSeeds {
    pub unit: String,
    /// Offsets FROM THE KEY, one per centre (union); empty = the key itself.
    pub centres: Vec<Vec<i32>>,
    pub generator: RecipeGenerator,
    /// FLOOR, explicit: drop a seed with `seed[s] - key[s] < v`, for the NAMED slots only.
    #[serde(default)]
    pub min_offset: BTreeMap<usize, i32>,
    #[serde(default)]
    pub max_offset: BTreeMap<usize, i32>,
    #[serde(default)]
    pub layer_cap: Option<Vec<u32>>,
    #[serde(default = "yes")]
    pub sector_filter: bool,
    /// The slots the sector filter acts on (`None` = all), NAMED.
    #[serde(default)]
    pub sector_slots: Option<Vec<usize>>,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeElimination {
    pub convention: String,
}

/// THE recipe: the engine's file `rustred.recipe.v1`, field for field (`serde_json::to_string` writes the file the
/// verb reads; unknown fields are refused on reading, as the verb does).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub schema: String,
    pub id: String,
    pub arity: usize,
    /// The sector: '1' = line (>= 1), '0' = numerator (<= 0), one character per slot.
    pub chart: String,
    pub domain: RecipeDomain,
    pub seeds: RecipeSeeds,
    pub symmetry_rows: bool,
    pub elimination: RecipeElimination,
    /// Free JSON echoed by the engine and part of its digest: the campaign writes the node, the node order and the
    /// setting label here (identity, re-audit A m4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<serde_json::Value>,
}

impl Recipe {
    /// The recipe of an attempt: the setting that found a relation on `case` (R48: "the ladder's attempt IS such a
    /// solve"), written out by the engine's own mapping (coord-recipe-solve.md section 3): centres = the setting's
    /// offsets as full vectors; `--max-depth K` = ball radius K with `down` = the case's free slots WRITTEN OUT; the
    /// face floor = `min_offset {L: -1}` on the case's face lines (free dotted lines and fixed lines at value >= 2;
    /// a line fixed at 1 is not floored); the sector filter on the case's FIXED slots, NAMED; provenance = node, node
    /// order, setting label.
    pub fn from_setting(
        id: &str,
        setting: &SeedSetting,
        case: &CaseSpec,
        params: &NodeParams,
        node_order: &str,
    ) -> Recipe {
        let n = params.arity;
        let centres: Vec<Vec<i32>> = setting
            .offsets
            .iter()
            .filter(|m| m.values().any(|&v| v != 0))
            .map(|m| (0..n).map(|i| m.get(&i).copied().unwrap_or(0)).collect())
            .collect();
        let free = case.free();
        let mut min_offset = BTreeMap::new();
        if setting.floor {
            for i in 0..n {
                let face_line = params.lines[i]
                    && (free.contains(&i) && case.face.sd.contains(&i)
                        || case.fixed.get(&i).is_some_and(|&v| v >= 2));
                if face_line {
                    min_offset.insert(i, -1);
                }
            }
        }
        let fixed: Vec<Option<i32>> = (0..n)
            .map(|i| {
                if free.contains(&i) {
                    None
                } else {
                    Some(
                        case.fixed
                            .get(&i)
                            .copied()
                            .unwrap_or(if params.lines[i] { 1 } else { 0 }),
                    )
                }
            })
            .collect();
        let sector_slots: Vec<usize> = (0..n).filter(|i| !free.contains(i)).collect();
        Recipe {
            schema: RECIPE_SCHEMA.into(),
            id: id.into(),
            arity: n,
            chart: params
                .lines
                .iter()
                .map(|&l| if l { '1' } else { '0' })
                .collect(),
            domain: RecipeDomain {
                fixed,
                equations: case.equations.iter().map(|e| e.to_engine()).collect(),
                exceptions: vec![],
                bounds: None,
            },
            seeds: RecipeSeeds {
                unit: RECIPE_UNIT.into(),
                centres,
                generator: RecipeGenerator::Ball(RecipeBall {
                    radius: setting.depth,
                    down: free,
                }),
                min_offset,
                max_offset: BTreeMap::new(),
                layer_cap: setting.layer_cap.clone(),
                sector_filter: true,
                sector_slots: Some(sector_slots),
            },
            symmetry_rows: setting.symmetry_rows,
            elimination: RecipeElimination {
                convention: RECIPE_ELIMINATION.into(),
            },
            provenance: Some(serde_json::json!({
                "node": params.node,
                "node_order": node_order,
                "setting": setting.label(),
            })),
        }
    }

    /// Read an engine recipe file (unknown fields refused; schema and convention checked).
    pub fn parse(text: &str) -> Result<Recipe, String> {
        let r: Recipe = serde_json::from_str(text).map_err(|e| e.to_string())?;
        r.validate()?;
        Ok(r)
    }

    /// Consistency: schema, convention, unit, arity of every vector, chart text.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != RECIPE_SCHEMA {
            return Err(format!("schema {} (want {RECIPE_SCHEMA})", self.schema));
        }
        if self.elimination.convention != RECIPE_ELIMINATION {
            return Err(format!("elimination {}", self.elimination.convention));
        }
        if self.seeds.unit != RECIPE_UNIT {
            return Err(format!("seed unit {}", self.seeds.unit));
        }
        let n = self.arity;
        if self.chart.len() != n || !self.chart.chars().all(|c| c == '0' || c == '1') {
            return Err(format!("chart {:?} for arity {n}", self.chart));
        }
        if self.domain.fixed.len() != n || self.seeds.centres.iter().any(|c| c.len() != n) {
            return Err("vector of the wrong arity".into());
        }
        if let Some(b) = &self.domain.bounds
            && (b.min.len() != n || b.max.len() != n)
        {
            return Err("bounds of the wrong arity".into());
        }
        let slot_ok = |s: &usize| *s < n;
        let gen_ok = match &self.seeds.generator {
            RecipeGenerator::Ball(b) => b.down.iter().all(slot_ok),
            RecipeGenerator::Shells(s) => !s.is_empty(),
        };
        if !gen_ok
            || !self.seeds.min_offset.keys().all(slot_ok)
            || !self.seeds.max_offset.keys().all(slot_ok)
            || !self
                .seeds
                .sector_slots
                .as_ref()
                .is_none_or(|v| v.iter().all(slot_ok))
        {
            return Err("slot index out of range".into());
        }
        self.case_domain().map(|_| ())
    }

    /// The chart as a line mask.
    pub fn lines(&self) -> Vec<bool> {
        self.chart.chars().map(|c| c == '1').collect()
    }

    /// The node named in the provenance (if any).
    pub fn node(&self) -> Option<&str> {
        self.provenance.as_ref()?.get("node")?.as_str()
    }

    /// The recipe's domain as a core [`CaseDomain`] (THE domain semantics of the dispatcher: chart orthant, fixed
    /// slots, equations, exception branches, bounds).
    pub fn case_domain(&self) -> Result<CaseDomain, String> {
        let n = self.arity;
        let parse =
            |t: &str| crate::poly::Poly::parse_d_free(t, n).map_err(|e| format!("{t}: {e:?}"));
        let mut d = CaseDomain::free(n, Some(self.lines()));
        d.fixed = self.domain.fixed.clone();
        for e in &self.domain.equations {
            d.equations.push(parse(e)?);
        }
        for br in &self.domain.exceptions {
            d.exceptions
                .push(br.iter().map(|e| parse(e)).collect::<Result<_, _>>()?);
        }
        if let Some(b) = &self.domain.bounds {
            for i in 0..n {
                if let Some(lo) = b.min[i] {
                    d.bounds.push(parse(&format!("n{i}-({lo})"))?);
                }
                if let Some(hi) = b.max[i] {
                    d.bounds.push(parse(&format!("({hi})-n{i}"))?);
                }
            }
        }
        d.validate().map_err(|e| e.to_string())?;
        Ok(d)
    }

    /// Content digest of the campaign side (FNV-1a 128 over the canonical JSON; a version constant, not a security
    /// digest). The ENGINE's digest (BLAKE3 of its parsed canonical JSON) is the one a certification binds: it is
    /// reported per answer ([`KeyRelation::recipe_digest`]) and recorded in the [`RecipeRule`].
    pub fn digest(&self) -> String {
        let text = serde_json::to_string(self).unwrap_or_default();
        format!("rcp1-{:032x}", fnv1a128(text.as_bytes()))
    }

    /// The key lies in the recipe's domain (the dispatcher's domain semantics, [`Self::case_domain`]).
    pub fn in_domain(&self, key: &[i32]) -> bool {
        key.len() == self.arity && self.case_domain().is_ok_and(|d| d.applies(key).applies())
    }

    /// The acceptance lane's view of THIS recipe (one definition: re-audit A M6, int4): it carries this file
    /// verbatim and derives its domain from it ([`crate::accept::recipe::Recipe::from_file`]); `order_digest` = the
    /// node comparator's identity, `contract` = the determinism contract id.
    pub fn to_acceptance(
        &self,
        order_digest: &str,
        contract: &str,
        frame: &crate::key::NodeFrame,
    ) -> Result<crate::accept::recipe::Recipe, String> {
        crate::accept::recipe::Recipe::from_file(self, order_digest, contract, frame)
    }
}

/// What the solve of a recipe at one key returned (keys only, no coefficients).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRelation {
    pub key: Key,
    /// Top-sector right-hand-side keys with a non-zero coefficient.
    pub rhs_keys: Vec<Key>,
    pub relation_digest: String,
    /// No pole of the relation's coefficients at the key (d-only poles excluded).
    pub regular: Tri,
    /// The exact product check of the weights passed (coord-recipe-solve.md section 6).
    pub proof_ok: bool,
    /// The engine's determinism contract id (REQUIRED for certification: every certified key's id must agree).
    pub determinism_id: Option<String>,
    pub cost_ms: u64,
    /// The engine's digest of the recipe it solved (BLAKE3 of its parsed canonical JSON).
    #[serde(default)]
    pub recipe_digest: Option<String>,
    /// The engine binary digest.
    #[serde(default)]
    pub binary_digest: Option<String>,
}

/// Typed failures of a recipe solve (R48 addendum 3 failure classes).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecipeFailure {
    NotInDomain,
    NotInCertifiedSet,
    NoRelation,
    NotDescending { rhs: Key },
    Pole,
    ProofFailed,
    DigestMismatch,
    Resource { why: String },
    Unsupported { why: String },
    Error(String),
}

/// One per-key certification record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyCertificate {
    pub key: Key,
    pub relation_digest: String,
    pub rhs_keys: Vec<Key>,
    pub cost_ms: u64,
}

/// An accepted recipe rule: the recipe, its CERTIFIED key set and the per-key records, bound to the engine's
/// determinism id, recipe digest and binary digest. FIELDS ARE PRIVATE and there is no `Deserialize` (re-audit A m4 /
/// B m5): only [`accept_recipe`] makes one; [`RecipeRuleRecord`] is its unverified, serialisable form (a claim, never
/// a dispatch entry). It MATCHES a key only if the key is in its domain AND certified, through the acceptance lane's
/// [`crate::accept::recipe::RecipeEntry`]; otherwise dispatch falls through. Dispatch POSITION (re-audit B m5): a
/// recipe never blocks and needs no exclusions; it is placed AFTER the node's symbolic rules of the same face (a
/// symbolic rule that applies wins; the recipe serves the certified keys they leave).
#[derive(Clone, Debug, Serialize)]
pub struct RecipeRule {
    recipe: Recipe,
    recipe_digest: String,
    engine_recipe_digest: Option<String>,
    determinism_id: Option<String>,
    binary_digest: Option<String>,
    node: String,
    certified: BTreeSet<Key>,
    certified_digest: String,
    records: Vec<KeyCertificate>,
    failed: Vec<(Key, RecipeFailure)>,
    #[serde(skip)]
    domain: Option<CaseDomain>,
}

impl PartialEq for RecipeRule {
    fn eq(&self, o: &Self) -> bool {
        self.to_record() == o.to_record()
    }
}
impl Eq for RecipeRule {}

/// UNVERIFIED serialisable form of a [`RecipeRule`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeRuleRecord {
    pub recipe: Recipe,
    pub recipe_digest: String,
    pub engine_recipe_digest: Option<String>,
    pub determinism_id: Option<String>,
    pub binary_digest: Option<String>,
    pub node: String,
    pub certified: Vec<Key>,
    pub certified_count: usize,
    pub certified_digest: String,
    pub records: Vec<KeyCertificate>,
    pub failed: Vec<(Key, RecipeFailure)>,
}

impl RecipeRule {
    pub fn recipe(&self) -> &Recipe {
        &self.recipe
    }
    pub fn recipe_digest(&self) -> &str {
        &self.recipe_digest
    }
    pub fn engine_recipe_digest(&self) -> Option<&str> {
        self.engine_recipe_digest.as_deref()
    }
    pub fn determinism_id(&self) -> Option<&str> {
        self.determinism_id.as_deref()
    }
    pub fn binary_digest(&self) -> Option<&str> {
        self.binary_digest.as_deref()
    }
    pub fn node(&self) -> &str {
        &self.node
    }
    /// The certified keys, sorted.
    pub fn certified(&self) -> Vec<Key> {
        self.certified.iter().cloned().collect()
    }
    pub fn certified_count(&self) -> usize {
        self.certified.len()
    }
    pub fn certified_digest(&self) -> &str {
        &self.certified_digest
    }
    pub fn records(&self) -> &[KeyCertificate] {
        &self.records
    }
    pub fn failed(&self) -> &[(Key, RecipeFailure)] {
        &self.failed
    }
    /// Dispatch: in the domain AND in the certified set (the acceptance lane's `RecipeEntry`), else fall through.
    pub fn matches(&self, key: &[i32]) -> bool {
        match &self.domain {
            Some(d) => crate::accept::recipe::RecipeEntry {
                domain: d,
                certified: &self.certified,
            }
            .matches(key),
            None => false,
        }
    }
    pub fn to_record(&self) -> RecipeRuleRecord {
        RecipeRuleRecord {
            recipe: self.recipe.clone(),
            recipe_digest: self.recipe_digest.clone(),
            engine_recipe_digest: self.engine_recipe_digest.clone(),
            determinism_id: self.determinism_id.clone(),
            binary_digest: self.binary_digest.clone(),
            node: self.node.clone(),
            certified: self.certified(),
            certified_count: self.certified.len(),
            certified_digest: self.certified_digest.clone(),
            records: self.records.clone(),
            failed: self.failed.clone(),
        }
    }
}

/// ACCEPTANCE of a recipe at a key set (R48). Refused BEFORE ANY SOLVE (every key failed, typed) when the
/// full-attempt token names another node, the recipe's provenance names another node, the recipe is malformed, or
/// the provers cannot bring the recipe's class (with / without symmetry rows) to the node's required level
/// (re-audit A M4; per-key level A3 by construction). Then each key is solved through the solver trait and certified
/// only if: in the domain; solved for that key; the exact product check passed; regular at the key; every RHS key
/// strictly lower under the full node comparator; and the answer carries a determinism id. All certified answers must
/// agree on the determinism id, the engine's recipe digest and the binary digest (the first certified key in key
/// order fixes them; a disagreeing key is `DigestMismatch`).
pub fn accept_recipe(
    recipe: &Recipe,
    keys: &[Key],
    solver: &mut dyn CaseSolver,
    descent: &dyn Descent,
    params: &NodeParams,
    ended: &FullAttemptEnded,
) -> RecipeRule {
    let mut uniq: Vec<Key> = keys.to_vec();
    uniq.sort();
    uniq.dedup();
    let domain = recipe.case_domain().ok();
    let refusal: Option<String> = if ended.node() != params.node {
        Some(format!(
            "full-attempt token of node {} (this node {})",
            ended.node(),
            params.node
        ))
    } else if recipe.node().is_some_and(|n| n != params.node) {
        Some(format!("recipe of node {:?}", recipe.node()))
    } else if let Err(e) = recipe.validate() {
        Some(format!("malformed recipe: {e}"))
    } else if recipe.arity != params.arity || recipe.lines() != params.lines {
        Some("recipe chart is not the node's".into())
    } else if !params
        .capabilities
        .can(recipe.symmetry_rows, params.min_evidence)
    {
        Some(format!(
            "the provers cannot bring a recipe {} symmetry rows to {:?}",
            if recipe.symmetry_rows {
                "with"
            } else {
                "without"
            },
            params.min_evidence
        ))
    } else if EvidenceLevel::A3 < params.min_evidence {
        Some(format!(
            "required evidence {:?} above per-key",
            params.min_evidence
        ))
    } else {
        None
    };
    let mut cert: BTreeMap<Key, KeyCertificate> = BTreeMap::new();
    let mut failed = Vec::new();
    let (mut det, mut eng, mut bin): (Option<String>, Option<String>, Option<String>) =
        (None, None, None);
    let mut fixed_ids = false;
    for k in uniq {
        if let Some(why) = &refusal {
            failed.push((k, RecipeFailure::Unsupported { why: why.clone() }));
            continue;
        }
        if !domain.as_ref().is_some_and(|d| d.applies(&k).applies()) {
            failed.push((k, RecipeFailure::NotInDomain));
            continue;
        }
        match solver.solve_recipe_at_key(recipe, &k) {
            Err(f) => failed.push((k, f)),
            Ok(rel) => {
                if rel.key != k {
                    failed.push((k, RecipeFailure::Error("relation for another key".into())));
                } else if !rel.proof_ok {
                    failed.push((k, RecipeFailure::ProofFailed));
                } else if rel.regular != Tri::Pass {
                    failed.push((k, RecipeFailure::Pole));
                } else if let Some(r) = rel.rhs_keys.iter().find(|r| !descent.lower(r, &k)) {
                    failed.push((k, RecipeFailure::NotDescending { rhs: r.clone() }));
                } else if rel.determinism_id.is_none() {
                    failed.push((
                        k,
                        RecipeFailure::Unsupported {
                            why: "the answer carries no determinism id".into(),
                        },
                    ));
                } else if fixed_ids
                    && (rel.determinism_id != det
                        || rel.recipe_digest != eng
                        || rel.binary_digest != bin)
                {
                    failed.push((k, RecipeFailure::DigestMismatch));
                } else {
                    if !fixed_ids {
                        det = rel.determinism_id.clone();
                        eng = rel.recipe_digest.clone();
                        bin = rel.binary_digest.clone();
                        fixed_ids = true;
                    }
                    cert.insert(
                        k.clone(),
                        KeyCertificate {
                            key: k,
                            relation_digest: rel.relation_digest,
                            rhs_keys: rel.rhs_keys,
                            cost_ms: rel.cost_ms,
                        },
                    );
                }
            }
        }
    }
    let digest_text: String = cert
        .values()
        .map(|c| format!("{:?}:{}", c.key, c.relation_digest))
        .collect::<Vec<_>>()
        .join(";");
    RecipeRule {
        recipe_digest: recipe.digest(),
        recipe: recipe.clone(),
        engine_recipe_digest: eng,
        determinism_id: det,
        binary_digest: bin,
        node: params.node.clone(),
        certified_digest: format!("{:032x}", fnv1a128(digest_text.as_bytes())),
        certified: cert.keys().cloned().collect(),
        records: cert.into_values().collect(),
        failed,
        domain,
    }
}

/// Output form of a face (R48 ECONOMY AND CHOICE).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleForm {
    Symbolic,
    Recipe,
    Point,
}

/// Spec thresholds of the form choice (DATA; defaults are proposals, seed-policy.md section 6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormThresholds {
    /// A symbolic rule above this many RHS terms is output as a recipe instead.
    pub symbolic_max_rhs_terms: u64,
    /// ... or above this many bytes.
    pub symbolic_max_bytes: u64,
    /// A certified key whose per-key solve took longer than this is materialised as a stored point rule.
    pub point_min_solve_ms: u64,
}

impl Default for FormThresholds {
    fn default() -> Self {
        FormThresholds {
            symbolic_max_rhs_terms: 20_000,
            symbolic_max_bytes: 4 << 20,
            point_min_solve_ms: 60_000,
        }
    }
}

/// The form of a face: a SMALL symbolic rule is preferred (valid for all powers); a large one (thresholds) or none
/// (heavy-refused / lift budget) gives a recipe when a recipe was certified; else whatever exists.
pub fn choose_form(
    symbolic: Option<(u64, Option<u64>)>,
    recipe_certified: bool,
    t: &FormThresholds,
) -> Option<RuleForm> {
    match symbolic {
        Some((terms, bytes))
            if terms <= t.symbolic_max_rhs_terms
                && bytes.is_none_or(|b| b <= t.symbolic_max_bytes) =>
        {
            Some(RuleForm::Symbolic)
        }
        _ if recipe_certified => Some(RuleForm::Recipe),
        Some(_) => Some(RuleForm::Symbolic),
        None => None,
    }
}

/// Keys of a recipe rule to materialise as stored point rules (their per-key solve is expensive).
pub fn keys_to_materialise(rule: &RecipeRule, t: &FormThresholds) -> BTreeSet<Key> {
    rule.records
        .iter()
        .filter(|c| c.cost_ms > t.point_min_solve_ms)
        .map(|c| c.key.clone())
        .collect()
}

/// Per-node report of rules by form (R48 (d)).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormReport {
    pub symbolic: usize,
    pub recipe: usize,
    pub point: usize,
    pub certified_keys: usize,
}

/// Why [`recipe_disposition`] gives no recipe outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecipeRefusal {
    /// R48 addendum 3 (a): the item's own cheap symbolic attempt has not run yet (its first gating wave is not
    /// settled).
    SymbolicNotTried,
    /// No key of the item was certified (the failures per key).
    NothingCertified { failed: Vec<(Key, RecipeFailure)> },
}

/// The item's first gating wave is settled (the cheap symbolic attempt ran; R48 addendum 3 (a)).
fn symbolic_tried(state: &ItemState, params: &NodeParams) -> bool {
    let point = state.item.kind == super::items::ItemKind::Point || state.item.case.is_point();
    let waves = super::seed::seed_space(&state.item.case, params);
    let Some(first) = waves
        .iter()
        .find(|w| w.iter().any(|s| !super::seed::is_information(s, params)))
    else {
        return true; // no symbolic search space at all
    };
    first.iter().all(|s| {
        state
            .records
            .get(&super::ladder::attempt_key(&state.item.id, s))
            .is_some_and(|r| {
                let rows =
                    super::seed::attempt_budget(s, &state.item.case, params, point, true).max_rows;
                r.state(&params.budgets, rows) != super::ladder::SettingState::Open
            })
    })
}

/// The item's RECIPE outcome (R48 (b)): accept `recipe` at the item's gap points (after the full attempt ended and
/// after the item's own cheap symbolic attempt ran: R35 order, R48 addendum 3 (a)) and return
/// `Disposition::RecipeRule` if at least one key is certified; the uncertified keys are listed AND returned as the
/// gap points of `rest` (a copy of the item restricted to them) so each gap point gets exactly one disposition
/// (re-audit A m5).
pub fn recipe_disposition(
    state: &ItemState,
    recipe: &Recipe,
    solver: &mut dyn CaseSolver,
    descent: &dyn Descent,
    params: &NodeParams,
    ended: &FullAttemptEnded,
) -> Result<super::ladder::Disposition, RecipeRefusal> {
    if !symbolic_tried(state, params) {
        return Err(RecipeRefusal::SymbolicNotTried);
    }
    let item = &state.item;
    let rule = accept_recipe(recipe, &item.gap_points, solver, descent, params, ended);
    if rule.certified.is_empty() {
        return Err(RecipeRefusal::NothingCertified {
            failed: rule.failed.clone(),
        });
    }
    let uncertified: Vec<Key> = item
        .gap_points
        .iter()
        .filter(|k| !rule.certified.contains(*k))
        .cloned()
        .collect();
    let rest = if uncertified.is_empty() {
        None
    } else {
        let mut it: Item = item.clone();
        it.gap_points = uncertified.clone();
        it.weight = it.gap_points.len() as u64;
        Some(Box::new(it))
    };
    Ok(super::ladder::Disposition::RecipeRule {
        rule: Box::new(rule),
        uncertified,
        rest,
    })
}

// ------------------------------------------------------------------------------------------------------------------
// R49 / R48 addenda 3 (a) and 4: the DEFAULT recipe, the recipe economy, the cheap-first mode (types)

/// The DEFAULT RECIPE SEQUENCE for a face first met while closing (R49 (b), R48 addendum 4), cheapest first:
/// (1) symmetry rows at the key alone (only for a node with a nontrivial group whose provers can certify rules with
/// symmetry rows at the node's level); (2) depth 2 centred on the key's LAST dotted line, floored on the face lines;
/// (3) depth 2 with the union of one centre per off-floor slot (a line one step down, a numerator one step up),
/// floored. The domain of each is the key's whole FACE. Generic: nothing depends on the node beyond its line mask and
/// group. Empty for a key outside the chart.
pub fn default_recipes(key: &[i32], params: &NodeParams, node_order: &str) -> Vec<Recipe> {
    let Some(face) = Face::of_key(key, &params.lines) else {
        return vec![];
    };
    let case = CaseSpec::face_case(face.clone());
    let mut out = Vec::new();
    let tag = |i: usize| format!("default/{}/{i}", face.label());
    if params.symmetry.as_ref().is_some_and(|g| g.nontrivial())
        && params.capabilities.can(true, params.min_evidence)
    {
        out.push(Recipe::from_setting(
            &tag(1),
            &SeedSetting::symmetry_target(),
            &case,
            params,
            node_order,
        ));
    }
    if let Some(&last) = face.sd.last() {
        let s = SeedSetting {
            centre: CentreKind::LineDown(last),
            offsets: vec![BTreeMap::from([(last, -1)])],
            floor: true,
            ..SeedSetting::target(2)
        };
        out.push(Recipe::from_setting(&tag(2), &s, &case, params, node_order));
    }
    let off: Vec<usize> = face.slots();
    if off.len() >= 2 {
        let s = SeedSetting {
            centre: CentreKind::Explicit,
            offsets: off
                .iter()
                .map(|&x| BTreeMap::from([(x, if params.lines[x] { -1 } else { 1 })]))
                .collect(),
            floor: !face.sd.is_empty(),
            ..SeedSetting::target(2)
        };
        out.push(Recipe::from_setting(&tag(3), &s, &case, params, node_order));
    }
    // lane termmin2: the MIXED SHELL (before any depth-3 setting; hundreds of seeds), explicit offsets as centres with
    // the single shell S0 (the format's named shells are `harder`-unit and have no S1m2), no floor, sector filter on
    if params.policy.mixed_shell
        && let Ok(offs) = crate::termmin::steps::mixed_shell_offsets(
            key,
            &params.lines,
            &crate::termmin::spec::MixedShell::default().shells,
        )
    {
        let mut r = Recipe::from_setting(&tag(4), &SeedSetting::target(1), &case, params, node_order);
        r.seeds.centres = offs;
        r.seeds.generator = RecipeGenerator::Shells(vec!["S0".into()]);
        r.seeds.min_offset = BTreeMap::new();
        r.seeds.sector_filter = true;
        r.seeds.sector_slots = None;
        out.push(r);
    }
    out
}

/// The economy of a RECIPE SET (R48 addendum 4 ruling): the CLOSURE SIZE it produces and the number of USED rules weigh
/// ABOVE the number of recipes; then the summed per-key solve cost. Smaller is better, lexicographic in field order.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct RecipeEconomy {
    /// Keys of the node's forward closure with this recipe set (new keys reached while closing included).
    pub closure_keys: u64,
    /// Rules (table, symbolic, recipe) used on that closure: every one must be certified.
    pub used_rules: u64,
    /// Number of recipes.
    pub recipes: u64,
    /// Summed per-key solve cost (ms, as reported).
    pub solve_cost_ms: u64,
}

/// One recipe CHOICE of a face, with the closure size it produced (R48 addendum 4: recorded per choice).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeChoice {
    pub face: String,
    pub recipe_id: String,
    pub recipe_digest: String,
    /// The choice is the default recipe (face first met while closing, or live repair).
    pub default: bool,
    /// The closure size produced with this choice in place.
    pub closure_keys: u64,
    pub used_rules: u64,
    pub certified_keys: u64,
}

/// The per-node record of recipe choices and the resulting economy.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeChoiceReport {
    pub choices: Vec<RecipeChoice>,
    pub total: RecipeEconomy,
}

impl RecipeChoiceReport {
    /// Record one choice (the caller measured the closure with it in place).
    pub fn record(&mut self, c: RecipeChoice) {
        self.choices.push(c);
    }
}

/// The better of two recipe sets under the R48 addendum 4 measure (`Less` = `a` is better; ties keep `a`).
pub fn compare_recipe_sets(a: &RecipeEconomy, b: &RecipeEconomy) -> std::cmp::Ordering {
    a.cmp(b)
}

/// CHEAP-FIRST mode of the ladder (R48 addendum 3 (a): the symbolic rule is tried first within a SMALL deterministic
/// budget and kept when cheap; no heavy lift, no long chain of children; what remains goes to recipes). TYPES ONLY in
/// v1.3: the control flow in `next_steps` is an OPEN item (repair-library.md v1.3 section 9d); a batch driver can
/// already realise it with these limits ([`within_cheap_budget`]) and [`recipe_disposition`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheapFirst {
    /// Work counter of the symbolic attempt (rows); above it the symbolic route is not pursued.
    pub symbolic_max_rows: u64,
    /// Child depth below which the ladder may split; deeper points go to recipes.
    pub max_child_depth: u32,
}

impl Default for CheapFirst {
    fn default() -> Self {
        CheapFirst {
            symbolic_max_rows: 2_000_000,
            max_child_depth: 1,
        }
    }
}

/// Is an item within the cheap-first limits (child depth; every gating setting of its first wave within the
/// work counter)?
pub fn within_cheap_budget(item: &Item, params: &NodeParams, cheap: &CheapFirst) -> bool {
    if item.depth > cheap.max_child_depth {
        return false;
    }
    super::seed::seed_space(&item.case, params)
        .iter()
        .find(|w| w.iter().any(|s| !super::seed::is_information(s, params)))
        .is_none_or(|w| {
            w.iter()
                .all(|s| super::seed::setting_work(s, params) <= cheap.symbolic_max_rows)
        })
}
