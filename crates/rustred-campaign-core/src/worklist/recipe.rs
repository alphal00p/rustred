//! RECIPE RULES (R48; addenda 4 and 5 of the directing session; engine verb interfaces/coord-recipe-solve.md DRAFT v0):
//! a [`Recipe`] is the complete, explicit, serialisable and digested description of ONE solve (domain, node order, seed
//! set at the key: centres, ball radius, the slots that move down, the floored slots NAMED explicitly, layer cap,
//! symmetry rows with the group digest, elimination convention, determinism id and engine digest), with NO
//! coefficients. [`CaseSolver::solve_recipe_at_key`](crate::repair::CaseSolver::solve_recipe_at_key) is the trait face
//! of the engine's "solve this recipe at this key" verb. A [`RecipeRule`] is accepted PER KEY over every key it is
//! credited with (success, right-hand-side keys, relation digest, descent of every RHS key under the full comparator,
//! regularity, exact product check = per-key evidence by construction); a key of its domain outside the certified set
//! is NOT covered and dispatch FALLS THROUGH there (a recipe never blocks a key it does not certify). Recipes take over
//! only after the node's full attempt ended ([`FullAttemptEnded`], R35 order). Nothing node-specific (rule L8).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::affine::AffineEq;
use super::items::{CaseSpec, EvidenceLevel, FullAttemptEnded, Key, NodeParams, fnv1a128};
use super::ladder::Tri;
use super::seed::SeedSetting;
use crate::repair::{CaseSolver, Descent};

pub const RECIPE_SCHEMA: &str = "rustred.recipe.v1";
/// Elimination convention id of the coordinator's draft (coord-recipe-solve.md section 4).
pub const RECIPE_ELIMINATION: &str = "rustred.recipe-elim.v1";

/// The recipe's domain (presented slots): fixed values (`None` = free), equations that must vanish, exception
/// branches (OR of AND-lists) outside which the recipe applies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeDomain {
    pub fixed: Vec<Option<i32>>,
    pub equations: Vec<AffineEq>,
    #[serde(default)]
    pub exceptions: Vec<Vec<AffineEq>>,
}

/// The seed set at a key, EXPLICIT (R48 addendum 2: nothing depends on which slots were symbolic in a case).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeSeeds {
    /// Offsets FROM THE KEY, one per centre (union); empty = the key itself.
    pub centres: Vec<Vec<i32>>,
    /// L1 ball radius around each centre (= the setting's shell depth).
    pub radius: u8,
    /// Slots on which the ball may move DOWN (written out: the free slots of the case that found the recipe).
    pub down: Vec<usize>,
    /// FLOOR, explicit: drop a seed with `seed[s] - key[s] < v`, for the NAMED slots only.
    pub min_offset: BTreeMap<usize, i32>,
    pub layer_cap: Option<Vec<u32>>,
    pub sector_filter: bool,
}

/// One complete solve description.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    pub schema: String,
    pub id: String,
    pub arity: usize,
    /// `chart[i]` = slot i is a line.
    pub chart: Vec<bool>,
    pub domain: RecipeDomain,
    pub seeds: RecipeSeeds,
    pub symmetry_rows: bool,
    /// Digest of the node's symmetry group used for the symmetry rows (required when `symmetry_rows`).
    pub group_digest: Option<String>,
    /// The node order exactly as the engine takes it (name or NodeOrder JSON text).
    pub node_order: String,
    pub elimination: String,
    /// The determinism contract (seed order, pivot rule, primes, elimination convention) as the engine names it;
    /// `None` until the engine reports it.
    pub determinism_id: Option<String>,
    /// Digest of the engine binary that certified the recipe.
    pub engine_digest: Option<String>,
}

impl Recipe {
    /// The recipe of an attempt: the setting that found a relation on `case` (R48: "the ladder's attempt IS such a
    /// solve"). Centres become full offset vectors; `down` = the case's free slots; the FLOOR is written as
    /// `min_offset {L: -1}` on the case's face lines (free dotted lines, and fixed lines at value >= 2).
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
                    Some(case.fixed.get(&i).copied().unwrap_or(if params.lines[i] {
                        1
                    } else {
                        0
                    }))
                }
            })
            .collect();
        Recipe {
            schema: RECIPE_SCHEMA.into(),
            id: id.into(),
            arity: n,
            chart: params.lines.clone(),
            domain: RecipeDomain {
                fixed,
                equations: case.equations.clone(),
                exceptions: vec![],
            },
            seeds: RecipeSeeds {
                centres,
                radius: setting.depth,
                down: free,
                min_offset,
                layer_cap: setting.layer_cap.clone(),
                sector_filter: true,
            },
            symmetry_rows: setting.symmetry_rows,
            group_digest: if setting.symmetry_rows {
                params
                    .symmetry
                    .as_ref()
                    .map(|g| format!("{:032x}", fnv1a128(format!("{:?}", g.elements).as_bytes())))
            } else {
                None
            },
            node_order: node_order.into(),
            elimination: RECIPE_ELIMINATION.into(),
            determinism_id: None,
            engine_digest: None,
        }
    }

    /// Content digest (FNV-1a 128 over the canonical JSON; a version constant, not a security digest).
    pub fn digest(&self) -> String {
        let text = serde_json::to_string(self).unwrap_or_default();
        format!("rcp1-{:032x}", fnv1a128(text.as_bytes()))
    }

    /// The key lies in the recipe's domain (fixed slots equal, free slots in the chart, equations hold, no exception
    /// branch vanishes entirely).
    pub fn in_domain(&self, key: &[i32]) -> bool {
        if key.len() != self.arity {
            return false;
        }
        for (i, &v) in key.iter().enumerate() {
            match self.domain.fixed[i] {
                Some(f) if f != v => return false,
                None if (self.chart[i] && v < 1) || (!self.chart[i] && v > 0) => return false,
                _ => {}
            }
        }
        self.domain.equations.iter().all(|e| e.holds(key))
            && !self
                .domain
                .exceptions
                .iter()
                .any(|b| !b.is_empty() && b.iter().all(|e| e.holds(key)))
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
    pub determinism_id: Option<String>,
    pub cost_ms: u64,
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

/// An accepted recipe rule: the recipe, its CERTIFIED key set (sorted keys, count and digest) and the per-key records.
/// It matches a key only if the key is in its domain AND in the certified set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeRule {
    pub recipe: Recipe,
    pub recipe_digest: String,
    pub certified: Vec<Key>,
    pub certified_count: usize,
    pub certified_digest: String,
    pub records: Vec<KeyCertificate>,
    /// Keys tried and not certified, with the failure (not covered by the recipe; open or for other forms).
    pub failed: Vec<(Key, RecipeFailure)>,
}

impl RecipeRule {
    pub fn matches(&self, key: &[i32]) -> bool {
        self.recipe.in_domain(key) && self.certified.binary_search(&key.to_vec()).is_ok()
    }
}

/// ACCEPTANCE of a recipe at a key set (R48): the recipe is solved at EVERY key through the solver trait; a key is
/// certified only if it is in the domain, the solve succeeded, every RHS key is strictly lower under the full node
/// comparator, the relation is regular at the key and the exact product check passed (per-key evidence by
/// construction: level A3), and that level reaches the node's `min_evidence`. Allowed only after the node's full
/// attempt ended (R35 order: the token is the precondition).
pub fn accept_recipe(
    recipe: &Recipe,
    keys: &[Key],
    solver: &mut dyn CaseSolver,
    descent: &dyn Descent,
    params: &NodeParams,
    _ended: &FullAttemptEnded,
) -> RecipeRule {
    let mut cert: BTreeMap<Key, KeyCertificate> = BTreeMap::new();
    let mut failed = Vec::new();
    let per_key_level = EvidenceLevel::A3;
    let mut uniq: Vec<Key> = keys.to_vec();
    uniq.sort();
    uniq.dedup();
    for k in uniq {
        if !recipe.in_domain(&k) {
            failed.push((k, RecipeFailure::NotInDomain));
            continue;
        }
        if per_key_level < params.min_evidence {
            failed.push((
                k,
                RecipeFailure::Unsupported {
                    why: format!("required evidence {:?} above per-key", params.min_evidence),
                },
            ));
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
                } else if recipe.determinism_id.is_some() && rel.determinism_id != recipe.determinism_id {
                    failed.push((k, RecipeFailure::DigestMismatch));
                } else {
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
    let certified: Vec<Key> = cert.keys().cloned().collect();
    let digest_text: String = cert
        .values()
        .map(|c| format!("{:?}:{}", c.key, c.relation_digest))
        .collect::<Vec<_>>()
        .join(";");
    RecipeRule {
        recipe_digest: recipe.digest(),
        recipe: recipe.clone(),
        certified_count: certified.len(),
        certified_digest: format!("{:032x}", fnv1a128(digest_text.as_bytes())),
        certified,
        records: cert.into_values().collect(),
        failed,
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

/// The item's RECIPE outcome (R48 (b)): accept `recipe` at the item's gap points (after the full attempt ended) and
/// return `Disposition::RecipeRule` if at least one key is certified; the uncertified keys are listed (they stay for
/// other forms, never covered by the recipe).
pub fn recipe_disposition(
    item: &super::items::Item,
    recipe: &Recipe,
    solver: &mut dyn CaseSolver,
    descent: &dyn Descent,
    params: &NodeParams,
    ended: &FullAttemptEnded,
) -> Option<super::ladder::Disposition> {
    let rule = accept_recipe(recipe, &item.gap_points, solver, descent, params, ended);
    if rule.certified.is_empty() {
        return None;
    }
    let uncertified: Vec<Key> = item
        .gap_points
        .iter()
        .filter(|k| !rule.certified.contains(k))
        .cloned()
        .collect();
    Some(super::ladder::Disposition::RecipeRule {
        rule: Box::new(rule),
        uncertified,
    })
}
