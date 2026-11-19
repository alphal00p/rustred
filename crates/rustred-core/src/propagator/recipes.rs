//! vk5l WP2 (plan VK5L §3.3): RECIPES IN THE REDUCTION.
//!
//! A boxed node may be closed by its parametric rules plus predetermined seed
//! RECIPES (`rustred.recipe.v1`, `node::recipe`) that the engine SOLVES AT THE
//! KEY, guaranteed on a certified key set (not a live repair). This module
//! holds:
//!
//! - the RECIPE BUNDLE of a boxed payload ([`RecipeBundle`], provenance line
//!   [`RECIPES_MARKER`], version [`P_RECIPE_VERSION`]): the node's family text
//!   in its original presentation, its node order, the table -> presentation
//!   slot map, the dispatch order (table rules first by default) and the
//!   RECIPE RECORDS ([`RecipeRecord`]): the recipe object verbatim, its digest,
//!   its domain (an explicit certified key list, or the recipe's own predicate
//!   inside the node's certified set), optional certified relation digests;
//!   a record matches a key only inside its domain, else dispatch falls
//!   through;
//! - the IN-PROCESS SOLVE ([`RecipeDispatch::relation`]): an in-process memo,
//!   then the relation caches (read-only pre-filled overlays, then the user
//!   cache), then the engine's solve entry point (`node::recipe::session_group`
//!   in worker threads, one preparation per (chart, symmetry rows) shared by
//!   every recipe of the node), verified (verdict SOLVED, exact proof, every
//!   RHS key lower, certified digest when the record has one), stored, applied;
//! - the RELATION CACHE ([`super::relation_entries::RelationStore`]): relation
//!   entries `rustred.relation-entry.v1` (the engine's relation record plus
//!   recipe, binding, evidence and diagnostics), shipped read-only roots
//!   first, then the persistent run-time cache (per-process append-only
//!   segments), one directory per node identity (family + order + arity);
//! - the typed failure [`PRecipeFailure`] (`PError::RecipeFailed`): node,
//!   recipe id and digest, the key being solved, the failure kind, the verb's
//!   failure object verbatim, the original request ([`PRequestContext`],
//!   filled by `PLibrary::reduce`), one rule chain from the request to the key
//!   (when the input lies on the node), and a one-line reproduction command
//!   (the files it needs are written next to it). Never a silent fallback.
//!
//! Validity of cached relations is decided by version constants only (user
//! decision Q2): the bundle revision (a plain name), [`P_RECIPE_VERSION`] and
//! the recipe elimination convention name. The determinism id is recorded in
//! failures and in the prefill report, never used to refuse a cache.
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, mpsc};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};

use super::error::{PError, PRequestContext};

/// Version of the recipe bundle and of its dispatch semantics (record
/// matching, frame conversion, verification, cache layout). Joins the step
/// identity of a library whose scope reaches 5 loops and the binding of every
/// relation cache.
pub const P_RECIPE_VERSION: u32 = 1;
/// Schema of a [`RecipeBundle`].
pub const RECIPE_BUNDLE_SCHEMA: &str = "rustred.p-recipe-bundle.v1";
/// The bundle of a boxed payload is the provenance line after this marker
/// (base64 of the bundle's JSON), after the certified key set.
pub const RECIPES_MARKER: &str = "\nrecipes ";
/// Schema of the JSON form of a [`PRecipeFailure`].
pub const RECIPE_FAILED_SCHEMA: &str = "rustred.p-recipe-failed.v1";
/// User relation cache (read-write) root.
pub const RECIPE_CACHE_ENV: &str = "RUSTRED_P_RELATION_CACHE";
/// Older name of [`RECIPE_CACHE_ENV`], still read.
pub const RECIPE_CACHE_ENV_OLD: &str = "RUSTRED_P_RECIPE_CACHE";
/// Pre-filled relation caches (read-only overlays), `:`-separated roots.
pub const RECIPE_PREFILLED_ENV: &str = "RUSTRED_P_RELATION_SHIPPED";
/// Older name of [`RECIPE_PREFILLED_ENV`], still read.
pub const RECIPE_PREFILLED_ENV_OLD: &str = "RUSTRED_P_RECIPE_PREFILLED";
/// Engine sessions per (chart, symmetry rows) group (default 1).
pub const RECIPE_WORKERS_ENV: &str = "RUSTRED_P_RECIPE_WORKERS";
/// What a boxed node does with a key outside its certified set:
/// `solve` (default, user ruling 2026-11-14) or `refuse`.
pub const OUTSIDE_POLICY_ENV: &str = "RUSTRED_P_OUTSIDE_CERTIFIED";
/// Resource cap of one run-time solve outside the certified set: the trace
/// rows of the engine solve (deterministic work limit).
pub const OUTSIDE_MAX_TRACE_ROWS_ENV: &str = "RUSTRED_P_OUTSIDE_MAX_TRACE_ROWS";
/// At most this many recipe families are tried at one outside key.
pub const OUTSIDE_MAX_RECIPES_ENV: &str = "RUSTRED_P_OUTSIDE_MAX_RECIPES";
/// `on` / `1`: keep searching past a solved relation with a (d-4) pole for a
/// regular one. Default OFF (LANE_RULES rule 21, user 2026-11-19: d = 4
/// regularity is the last tie-break only; results stay exact in d): the first
/// solved relation is taken.
pub const PREFER_REGULAR_ENV: &str = "RUSTRED_P_PREFER_REGULAR_D4";
/// Defaults of the two caps.
pub const OUTSIDE_MAX_TRACE_ROWS: usize = 200_000;
pub const OUTSIDE_MAX_RECIPES: usize = 8;

/// The policy of a boxed node for keys outside its certified set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum POutsidePolicy {
    /// The node's rules first, then its recipe families solved at the key
    /// under the resource caps; the key is reported in the result
    /// ([`super::library::PReduction::outside_certified`]); no relation:
    /// [`PError::RecipeFailed`] of kind `OUTSIDE_NO_RELATION`.
    Solve,
    /// The hard [`PError::OutsideCertifiedSet`] (WP1 behaviour).
    Refuse,
}

static OUTSIDE_OVERRIDES: RwLock<BTreeMap<String, POutsidePolicy>> = RwLock::new(BTreeMap::new());

/// Set the outside policy of the boxed node `node` in this process (tests,
/// embedders); it wins over [`OUTSIDE_POLICY_ENV`].
pub fn set_outside_policy_for(node: &str, policy: POutsidePolicy) {
    if policy == POutsidePolicy::Refuse {
        REFUSE_OVERRIDDEN.store(true, Ordering::Relaxed);
    }
    OUTSIDE_OVERRIDES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(node.to_owned(), policy);
}

/// Whether any node may refuse outside keys in this process (an override
/// or the environment says `refuse`): enables the early check of step
/// children (a Solve-only process skips it).
pub fn refuse_policy_anywhere() -> bool {
    static FROM_ENV: OnceLock<bool> = OnceLock::new();
    REFUSE_OVERRIDDEN.load(Ordering::Relaxed)
        || *FROM_ENV.get_or_init(|| {
            matches!(
                std::env::var(OUTSIDE_POLICY_ENV).as_deref().map(str::trim),
                Ok("refuse" | "off" | "0")
            )
        })
}

/// Set once a node is given the Refuse policy ([`set_outside_policy_for`]).
static REFUSE_OVERRIDDEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The outside policy of `node`: its override, else [`OUTSIDE_POLICY_ENV`]
/// (`refuse` / `off` / `0` refuse; anything else solves).
pub fn outside_policy_of(node: &str) -> POutsidePolicy {
    if let Some(policy) = OUTSIDE_OVERRIDES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(node)
    {
        return *policy;
    }
    match std::env::var(OUTSIDE_POLICY_ENV).as_deref().map(str::trim) {
        Ok("refuse" | "off" | "0") => POutsidePolicy::Refuse,
        _ => POutsidePolicy::Solve,
    }
}

/// Optional cap of the size of a regular relation relative to a relation
/// with a (d-4) pole already found: `unbounded` (default) or a rational >= 1
/// (`3/2`, `2`).
pub const OUTSIDE_MAX_SIZE_RATIO_ENV: &str = "RUSTRED_P_OUTSIDE_MAX_SIZE_RATIO";

/// The settings of run-time solves at keys outside a certified set
/// (programmatic, for embedders such as vakint; the environment is the
/// fallback when none were set).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct POutsideSettings {
    /// Recipe families tried at one key. 0 = LIST-ONLY (batch 11, for a
    /// coverage pass): no recipe is solved; a key no rule and no shipped or
    /// cached entry takes gets the typed refusal at once, which
    /// `PUncoveredPolicy::Retain` keeps as a residual terminal, so one forward
    /// pass lists every arrival outside the certified sets.
    pub max_recipes: usize,
    /// Trace-row cap of one solve.
    pub max_trace_rows: usize,
    /// Prefer a relation regular at d = 4 (default OFF, rule 21): when on, a
    /// relation with a (d-4) pole is kept as fallback while the next families
    /// are tried.
    pub prefer_regular_at_4: bool,
    /// `None` (default, "unbounded"): a regular relation ALWAYS wins, whatever
    /// its size. `Some((p, q))`: a regular relation wins over a pole relation
    /// already found only when its number of terms is at most p/q times the
    /// pole relation's (p/q >= 1); a larger one is set aside and the search
    /// goes on; when no regular relation qualifies, the smallest pole relation
    /// is taken.
    pub max_size_ratio: Option<(u64, u64)>,
    /// Outside the certified set the table's first match comes first (its
    /// rules are parametric and strictly lowering; the step is counted as an
    /// outside rule step). `false`: certified-only use, the rules are not
    /// applied outside the set (entries, then recipes, then the refusal).
    pub rules_outside: bool,
    /// Wall bound of ONE recipe solve at an outside key (a lift cannot be
    /// interrupted: the wait stops, the solve is typed BUDGET and its engine
    /// session group is retired); `None`: no bound.
    pub max_seconds_per_solve: Option<f64>,
    /// Wall bound of all recipe solves at one outside key; `None`: no bound.
    pub max_seconds_per_key: Option<f64>,
}

/// Defaults of the two outside-solve wall bounds (seconds).
pub const OUTSIDE_MAX_SECONDS_PER_SOLVE: f64 = 120.0;
pub const OUTSIDE_MAX_SECONDS_PER_KEY: f64 = 600.0;

impl Default for POutsideSettings {
    fn default() -> Self {
        Self {
            max_recipes: OUTSIDE_MAX_RECIPES,
            max_trace_rows: OUTSIDE_MAX_TRACE_ROWS,
            prefer_regular_at_4: false,
            max_size_ratio: None,
            rules_outside: true,
            max_seconds_per_solve: Some(OUTSIDE_MAX_SECONDS_PER_SOLVE),
            max_seconds_per_key: Some(OUTSIDE_MAX_SECONDS_PER_KEY),
        }
    }
}

/// Parse a size ratio: `unbounded` -> `None`, `p/q` or `p` (>= 1) -> `Some`.
pub fn parse_size_ratio(text: &str) -> Result<Option<(u64, u64)>, String> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("unbounded") {
        return Ok(None);
    }
    let (p, q) = match text.split_once('/') {
        Some((p, q)) => (p.trim(), q.trim()),
        None => (text, "1"),
    };
    let (Ok(p), Ok(q)) = (p.parse::<u64>(), q.parse::<u64>()) else {
        return Err(format!(
            "max_size_ratio {text:?}: `unbounded` or a rational p/q >= 1"
        ));
    };
    if q == 0 || p < q {
        return Err(format!("max_size_ratio {text:?}: must be a rational >= 1"));
    }
    Ok(Some((p, q)))
}

static OUTSIDE_SETTINGS: RwLock<Option<POutsideSettings>> = RwLock::new(None);
static NODE_OUTSIDE_SETTINGS: RwLock<BTreeMap<String, POutsideSettings>> =
    RwLock::new(BTreeMap::new());

/// Set the outside-solve settings of this process.
pub fn set_outside_settings(settings: POutsideSettings) {
    *OUTSIDE_SETTINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(settings);
}

/// Set the outside-solve settings of node `node` (wins over the process's).
pub fn set_outside_settings_for(node: &str, settings: POutsideSettings) {
    NODE_OUTSIDE_SETTINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(node.to_owned(), settings);
}

/// The outside-solve settings in force for `node`: its own, the process's,
/// else the environment over the defaults.
pub fn outside_settings_of(node: &str) -> POutsideSettings {
    if let Some(settings) = NODE_OUTSIDE_SETTINGS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(node)
    {
        return *settings;
    }
    if let Some(settings) = *OUTSIDE_SETTINGS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        return settings;
    }
    POutsideSettings {
        // 0 is accepted here: list-only (see `POutsideSettings::max_recipes`)
        max_recipes: std::env::var(OUTSIDE_MAX_RECIPES_ENV)
            .ok()
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(OUTSIDE_MAX_RECIPES),
        max_trace_rows: env_cap(OUTSIDE_MAX_TRACE_ROWS_ENV, OUTSIDE_MAX_TRACE_ROWS),
        prefer_regular_at_4: matches!(
            std::env::var(PREFER_REGULAR_ENV).as_deref().map(str::trim),
            Ok("1" | "on" | "yes" | "true")
        ),
        max_size_ratio: std::env::var(OUTSIDE_MAX_SIZE_RATIO_ENV)
            .ok()
            .and_then(|text| match parse_size_ratio(&text) {
                Ok(ratio) => Some(ratio),
                Err(error) => {
                    eprintln!("rustred: {OUTSIDE_MAX_SIZE_RATIO_ENV} ignored: {error}");
                    None
                }
            })
            .flatten(),
        rules_outside: !matches!(
            std::env::var(OUTSIDE_RULES_ENV).as_deref().map(str::trim),
            Ok("0" | "off" | "no" | "false")
        ),
        max_seconds_per_solve: env_seconds(
            OUTSIDE_MAX_SECONDS_PER_SOLVE_ENV,
            OUTSIDE_MAX_SECONDS_PER_SOLVE,
        ),
        max_seconds_per_key: env_seconds(
            OUTSIDE_MAX_SECONDS_PER_KEY_ENV,
            OUTSIDE_MAX_SECONDS_PER_KEY,
        ),
    }
}

/// `off` / `0`: certified-only use, no table rules outside the set.
pub const OUTSIDE_RULES_ENV: &str = "RUSTRED_P_OUTSIDE_RULES";
/// Seconds (or `unbounded`) of one outside recipe solve / of all at one key.
pub const OUTSIDE_MAX_SECONDS_PER_SOLVE_ENV: &str = "RUSTRED_P_OUTSIDE_MAX_SECONDS_PER_SOLVE";
pub const OUTSIDE_MAX_SECONDS_PER_KEY_ENV: &str = "RUSTRED_P_OUTSIDE_MAX_SECONDS_PER_KEY";

fn env_seconds(name: &str, default: f64) -> Option<f64> {
    match std::env::var(name).as_deref().map(str::trim) {
        Ok(text) if text.eq_ignore_ascii_case("unbounded") => None,
        Ok(text) => text
            .parse::<f64>()
            .ok()
            .filter(|s| *s > 0.0)
            .or(Some(default)),
        Err(_) => Some(default),
    }
}

fn env_cap(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .filter(|&value: &usize| value > 0)
        .unwrap_or(default)
}

fn malformed(detail: impl fmt::Display) -> PError {
    PError::MalformedLibrary(format!("recipe bundle: {detail}"))
}

/// Where recipes stand in a boxed node's first-match dispatch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeDispatchOrder {
    /// The table's rules first; a key no rule takes goes to the first recipe
    /// record whose domain holds it (the plan's default).
    #[default]
    RulesFirst,
    /// A key of a record's domain is solved by the recipe before any rule.
    RecipesFirst,
}

/// The domain of a recipe record: where it may be dispatched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeDomain {
    /// An explicit certified key list (TABLE slots).
    Keys(Vec<Vec<i64>>),
    /// The recipe's own `domain` (fixed slots, bounds, equations, exception
    /// branches, explicit keys), inside the node's certified key set.
    Predicate,
}

/// batch 12a: where a recipe of an UNBOXED table stands in first match: just
/// before the table record `index` (the delivery's `position`), i.e. after the
/// `rules_before` ordinary rules of its chart that precede that record; its
/// domain is the recipe's own predicate AND `D + r <= bound_dr` (dots on the
/// chart's lines plus the numerator rank of the others, TABLE slots).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeTablePosition {
    pub index: usize,
    pub rules_before: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_dr: Option<i64>,
}

/// `D + r` of a TABLE key on `chart` (`1` = line): the dots `n - 1` of the
/// lines plus the rank `-n` of the other slots.
pub fn dots_plus_rank(chart: &str, key: &[i64]) -> i64 {
    chart
        .bytes()
        .zip(key)
        .map(|(line, &power)| if line == b'1' { power - 1 } else { -power })
        .sum()
}

/// One recipe record of a boxed node (or, with a table position, of an
/// unboxed table).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecipeRecord {
    /// The recipe's id (free text, echoed in every message).
    pub id: String,
    /// The `rustred.recipe.v1` object, verbatim.
    pub recipe: Value,
    /// `node::recipe::Recipe::digest` of it.
    pub recipe_digest: String,
    pub domain: RecipeDomain,
    /// Optional certified relation digests (`node::recipe` relation digest)
    /// by key text `n0,n1,..` (TABLE slots): a fresh solve must reproduce it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub relation_digests: BTreeMap<String, String>,
    /// Optional deterministic work limit of the solve (`--max-trace-rows`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_trace_rows: Option<usize>,
    /// batch 12a: a recipe of an unboxed table, dispatched at a table position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<RecipeTablePosition>,
}

impl RecipeRecord {
    /// Whether this positioned record's domain holds the TABLE key `table`.
    pub fn holds_at_position(&self, table: &[i64]) -> bool {
        let Some(position) = &self.position else {
            return false;
        };
        predicate_holds(&self.recipe, table)
            && position
                .bound_dr
                .is_none_or(|bound| dots_plus_rank(self.chart(), table) <= bound)
    }

    fn chart(&self) -> &str {
        self.recipe["chart"].as_str().unwrap_or("")
    }

    fn symmetry_rows(&self) -> bool {
        self.recipe["symmetry_rows"].as_bool().unwrap_or(false)
    }

    /// The recipe kind for messages: `ibp`, `ibp+symmetry-rows`.
    pub fn kind(&self) -> &'static str {
        if self.symmetry_rows() {
            "ibp+symmetry-rows"
        } else {
            "ibp"
        }
    }

    fn relation_rule(&self) -> &str {
        self.recipe["relation_rule"].as_str().unwrap_or("m1_first")
    }
}

/// vk5l WP2: what a table import is given ([`super::table_import::TableImportOptions::recipes`]):
/// the bundle's revision name, its dispatch order and the records; the
/// importer adds the family text, node order and slot map.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeImport {
    pub revision: String,
    pub dispatch: RecipeDispatchOrder,
    pub records: Vec<RecipeRecord>,
}

/// The recipes of one boxed node (provenance of its payload).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecipeBundle {
    pub schema: String,
    pub version: u32,
    /// The boxed node (`T178`).
    pub node: String,
    /// A plain content name (no digest), part of the relation cache's
    /// binding: a changed bundle gets a new name.
    pub revision: String,
    /// The node's family in its ORIGINAL presentation (`rr_node generate`).
    pub family_text: String,
    /// The node order (`NodeOrder` JSON), as the recipe verb takes it.
    pub order: Value,
    /// Table slot -> presentation slot (the import's slot map); recipes, their
    /// keys and relations are in TABLE (presented) slots.
    pub table_to_presentation: Vec<usize>,
    #[serde(default)]
    pub dispatch: RecipeDispatchOrder,
    pub records: Vec<RecipeRecord>,
}

impl RecipeBundle {
    /// A bundle with the version constants filled in; checked.
    pub fn new(
        node: impl Into<String>,
        revision: impl Into<String>,
        family_text: impl Into<String>,
        order: Value,
        table_to_presentation: Vec<usize>,
        dispatch: RecipeDispatchOrder,
        records: Vec<RecipeRecord>,
    ) -> Result<Self, PError> {
        let bundle = Self {
            schema: RECIPE_BUNDLE_SCHEMA.to_owned(),
            version: P_RECIPE_VERSION,
            node: node.into(),
            revision: revision.into(),
            family_text: family_text.into(),
            order,
            table_to_presentation,
            dispatch,
            records,
        };
        bundle.check()?;
        Ok(bundle)
    }

    /// The record of `recipe` (a `rustred.recipe.v1` JSON text): parsed by
    /// the engine (schema, convention, fields), its digest computed; the
    /// domain is the recipe's `domain.keys` when it has one, else its
    /// predicate.
    pub fn record_of(recipe_text: &str) -> Result<RecipeRecord, PError> {
        let parsed = crate::node::recipe::Recipe::parse(recipe_text)
            .map_err(|error| malformed(format!("recipe: {error}")))?;
        let recipe: Value = serde_json::from_str(recipe_text).map_err(malformed)?;
        let domain = match &parsed.domain.keys {
            Some(keys) => RecipeDomain::Keys(keys.clone()),
            None => RecipeDomain::Predicate,
        };
        Ok(RecipeRecord {
            id: parsed.id.clone(),
            recipe,
            recipe_digest: parsed.digest(),
            domain,
            relation_digests: BTreeMap::new(),
            max_trace_rows: None,
            position: None,
        })
    }

    /// Schema, version, slot map, every recipe parsed by the engine with the
    /// recorded digest, ids unique, keys of the arity.
    pub fn check(&self) -> Result<(), PError> {
        if self.schema != RECIPE_BUNDLE_SCHEMA || self.version != P_RECIPE_VERSION {
            return Err(PError::VersionMismatch {
                name: "P_RECIPE_VERSION".to_owned(),
                found: format!("{} v{}", self.schema, self.version),
                expected: format!("{RECIPE_BUNDLE_SCHEMA} v{P_RECIPE_VERSION}"),
            });
        }
        let arity = self.table_to_presentation.len();
        let mut seen = vec![false; arity];
        for &image in &self.table_to_presentation {
            match seen.get_mut(image) {
                Some(slot) if !*slot => *slot = true,
                _ => return Err(malformed("the slot map is no permutation")),
            }
        }
        if self.revision.is_empty()
            || !self
                .revision
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(malformed(format!(
                "revision {:?} (a plain name [A-Za-z0-9._-]+)",
                self.revision
            )));
        }
        crate::node::order::NodeOrder::from_json(&self.order.to_string())
            .map_err(|error| malformed(format!("order: {error}")))?;
        let mut ids = std::collections::BTreeSet::new();
        for record in &self.records {
            if !ids.insert(record.id.as_str()) {
                return Err(malformed(format!("two records with id {:?}", record.id)));
            }
            let parsed = crate::node::recipe::Recipe::parse(&record.recipe.to_string())
                .map_err(|error| malformed(format!("record {}: {error}", record.id)))?;
            if parsed.id != record.id || parsed.digest() != record.recipe_digest {
                return Err(malformed(format!(
                    "record {}: id or digest does not describe its recipe",
                    record.id
                )));
            }
            if parsed.arity != arity {
                return Err(malformed(format!(
                    "record {}: recipe arity {} on a node of {arity} slots",
                    record.id, parsed.arity
                )));
            }
            if let RecipeDomain::Keys(keys) = &record.domain
                && keys.iter().any(|key| key.len() != arity)
            {
                return Err(malformed(format!(
                    "record {}: a key of another arity",
                    record.id
                )));
            }
        }
        Ok(())
    }

    /// The provenance line of the bundle (marker included).
    pub fn provenance_line(&self) -> Result<String, PError> {
        let text = serde_json::to_string(self).map_err(malformed)?;
        Ok(format!(
            "{RECIPES_MARKER}{}",
            super::certified_keys::base64_encode(text.as_bytes())
        ))
    }

    /// The key of `table` (table slots) in presentation slots.
    pub fn to_presentation(&self, table: &[i64]) -> Vec<i64> {
        let mut out = vec![0; table.len()];
        for (slot, &power) in table.iter().enumerate() {
            out[self.table_to_presentation[slot]] = power;
        }
        out
    }

    /// The key of `presentation` (presentation slots) in table slots.
    pub fn to_table(&self, presentation: &[i64]) -> Vec<i64> {
        self.table_to_presentation
            .iter()
            .map(|&image| presentation[image])
            .collect()
    }

    /// Records and keys in one line (messages).
    pub fn describe(&self) -> String {
        let keys: usize = self
            .records
            .iter()
            .map(|record| match &record.domain {
                RecipeDomain::Keys(keys) => keys.len(),
                RecipeDomain::Predicate => 0,
            })
            .sum();
        format!(
            "{} recipe records ({keys} explicit keys) of node {}, revision {}, P_RECIPE_VERSION={}",
            self.records.len(),
            self.node,
            self.revision,
            self.version
        )
    }
}

/// The recipe bundle of a payload's provenance text, if any.
pub fn recipes_of_source(family_source: &str) -> Result<Option<RecipeBundle>, PError> {
    let Some(at) = family_source.find(RECIPES_MARKER) else {
        return Ok(None);
    };
    let text = &family_source[at + RECIPES_MARKER.len()..];
    let text = text.split('\n').next().unwrap_or("");
    let bytes = super::certified_keys::base64_decode(text)?;
    let bundle: RecipeBundle = serde_json::from_slice(&bytes).map_err(malformed)?;
    bundle.check()?;
    Ok(Some(bundle))
}

// ----- the domain predicate (table slots) -----

/// Value of an index polynomial (`n3-n5`, `2*n0+n1^2-3`) at `key`; `None`
/// for a text outside that grammar.
fn eval_index_poly(text: &str, key: &[i64]) -> Option<i128> {
    struct P<'a> {
        s: &'a [u8],
        i: usize,
        key: &'a [i64],
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self.i < self.s.len() && self.s[self.i] == b' ' {
                self.i += 1;
            }
        }
        fn peek(&mut self) -> Option<u8> {
            self.ws();
            self.s.get(self.i).copied()
        }
        fn expr(&mut self) -> Option<i128> {
            let mut value = match self.peek()? {
                b'-' => {
                    self.i += 1;
                    self.term()?.checked_neg()?
                }
                b'+' => {
                    self.i += 1;
                    self.term()?
                }
                _ => self.term()?,
            };
            while let Some(c) = self.peek() {
                match c {
                    b'+' => {
                        self.i += 1;
                        value = value.checked_add(self.term()?)?;
                    }
                    b'-' => {
                        self.i += 1;
                        value = value.checked_sub(self.term()?)?;
                    }
                    _ => break,
                }
            }
            Some(value)
        }
        fn term(&mut self) -> Option<i128> {
            let mut value = self.power()?;
            while self.peek() == Some(b'*') {
                self.i += 1;
                value = value.checked_mul(self.power()?)?;
            }
            Some(value)
        }
        fn power(&mut self) -> Option<i128> {
            let base = self.atom()?;
            if self.peek() == Some(b'^') {
                self.i += 1;
                let exponent = u32::try_from(self.number()?).ok()?;
                return base.checked_pow(exponent);
            }
            Some(base)
        }
        fn number(&mut self) -> Option<i128> {
            self.ws();
            let start = self.i;
            while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                self.i += 1;
            }
            std::str::from_utf8(&self.s[start..self.i])
                .ok()?
                .parse()
                .ok()
        }
        fn atom(&mut self) -> Option<i128> {
            match self.peek()? {
                b'(' => {
                    self.i += 1;
                    let value = self.expr()?;
                    (self.peek()? == b')').then_some(())?;
                    self.i += 1;
                    Some(value)
                }
                b'n' => {
                    self.i += 1;
                    let slot = usize::try_from(self.number()?).ok()?;
                    self.key.get(slot).map(|&power| i128::from(power))
                }
                b'-' => {
                    self.i += 1;
                    self.atom()?.checked_neg()
                }
                _ => self.number(),
            }
        }
    }
    let mut parser = P {
        s: text.as_bytes(),
        i: 0,
        key,
    };
    let value = parser.expr()?;
    (parser.peek().is_none()).then_some(value)
}

/// The recipe's own domain holds `key` (table slots): the chart's top
/// sector, fixed slots, bounds, explicit keys, equations vanish, no
/// exception branch vanishes entirely. A text the evaluator cannot read is
/// `false` (the record does not match; dispatch falls through).
fn predicate_holds(recipe: &Value, key: &[i64]) -> bool {
    let chart = recipe["chart"].as_str().unwrap_or("");
    if chart.len() != key.len() {
        return false;
    }
    for (line, &power) in chart.bytes().map(|c| c == b'1').zip(key) {
        if line && power < 1 || !line && power > 0 {
            return false;
        }
    }
    let domain = &recipe["domain"];
    if let Some(fixed) = domain["fixed"].as_array() {
        for (slot, value) in fixed.iter().enumerate() {
            if let Some(value) = value.as_i64()
                && key.get(slot) != Some(&value)
            {
                return false;
            }
        }
    }
    for (side, below) in [("min", true), ("max", false)] {
        if let Some(bounds) = domain["bounds"][side].as_array() {
            for (slot, bound) in bounds.iter().enumerate() {
                if let (Some(bound), Some(&power)) = (bound.as_i64(), key.get(slot))
                    && (below && power < bound || !below && power > bound)
                {
                    return false;
                }
            }
        }
    }
    if let Some(keys) = domain["keys"].as_array()
        && !keys.iter().any(|candidate| {
            candidate
                .as_array()
                .is_some_and(|c| c.iter().map(Value::as_i64).eq(key.iter().map(|&k| Some(k))))
        })
    {
        return false;
    }
    let vanishes = |text: &Value| {
        text.as_str()
            .and_then(|text| eval_index_poly(text, key))
            .map(|value| value == 0)
    };
    if let Some(equations) = domain["equations"].as_array() {
        for equation in equations {
            if vanishes(equation) != Some(true) {
                return false;
            }
        }
    }
    if let Some(branches) = domain["exceptions"].as_array() {
        for branch in branches {
            let Some(polys) = branch.as_array() else {
                return false;
            };
            let mut all = true;
            for poly in polys {
                match vanishes(poly) {
                    Some(true) => {}
                    Some(false) => all = false,
                    None => return false,
                }
            }
            if all {
                return false;
            }
        }
    }
    true
}

// ----- the failure -----

/// The kind of a recipe failure (the verb's failure classes, plus the
/// dispatcher's own checks).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PRecipeFailureKind {
    /// The seeds' rows never reach the key as a pivot.
    NoRelationInSeeds,
    /// The key is outside the recipe's own domain (fixed slot, equation,
    /// exception branch, bound, sector, key set).
    NotInDomain,
    /// A work or wall budget of the solve.
    Budget,
    /// The exact proof failed, is missing, or the relation is not lower.
    ProofMismatch,
    /// The solved relation differs from the certified one (relation digest).
    DigestMismatch,
    /// The lift reported a pole or structural failure at the key.
    Irregular,
    /// Any other engine error (a caught panic, a setup error), or a relation
    /// the dispatcher cannot read.
    Error,
    /// A key outside the certified set: no rule took it and no recipe family
    /// of the node gave a verified relation at it under the caps.
    OutsideNoRelation,
}

impl PRecipeFailureKind {
    fn from_class(class: &str) -> Self {
        match class {
            "no_relation_in_seeds" => Self::NoRelationInSeeds,
            "not_in_domain" => Self::NotInDomain,
            "budget_work" | "budget_wall" => Self::Budget,
            "proof_failed" | "not_lower" => Self::ProofMismatch,
            "digest_mismatch" => Self::DigestMismatch,
            "irregular" => Self::Irregular,
            _ => Self::Error,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::NoRelationInSeeds => "NO_RELATION_IN_SEEDS",
            Self::NotInDomain => "NOT_IN_DOMAIN",
            Self::Budget => "BUDGET",
            Self::ProofMismatch => "PROOF_MISMATCH",
            Self::DigestMismatch => "DIGEST_MISMATCH",
            Self::Irregular => "IRREGULAR",
            Self::Error => "ERROR",
            Self::OutsideNoRelation => "OUTSIDE_NO_RELATION",
        }
    }
}

/// One link of a rule chain: `key` (presentation slots) reached by `via`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PChainLink {
    pub key: Vec<i64>,
    /// `request input`, `table rule <i> (chart 0x..)` or `recipe <id>`.
    pub via: String,
}

/// A recipe that failed at run time (`PError::RecipeFailed`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PRecipeFailure {
    pub node: String,
    pub recipe_id: String,
    pub recipe_digest: String,
    pub recipe_kind: &'static str,
    pub relation_rule: String,
    /// The key being solved, presentation slots and table (recipe) slots.
    pub key: Vec<i64>,
    pub key_table: Vec<i64>,
    pub kind: PRecipeFailureKind,
    /// The verb's class (`no_relation_in_seeds`, ...) or the dispatcher's.
    pub class: String,
    pub message: String,
    /// The verb's failure object (`rustred.recipe-failure.v1`) verbatim, or
    /// the dispatcher's detail.
    pub failure: Value,
    /// One shell line that reruns exactly this solve stand-alone.
    pub reproduce: String,
    /// Where its files were written.
    pub reproduce_dir: Option<String>,
    pub determinism_id: String,
    /// The original request (filled by `PLibrary::reduce`).
    pub request: Option<Arc<PRequestContext>>,
    /// One rule chain from a request input to `key`, when one was found.
    pub rule_chain: Option<Vec<PChainLink>>,
    /// Why no chain is given.
    pub rule_chain_note: Option<String>,
}

fn powers_text(key: &[i64]) -> String {
    let parts: Vec<String> = key.iter().map(i64::to_string).collect();
    format!("I({})", parts.join(","))
}

impl PRecipeFailure {
    /// The JSON form ([`RECIPE_FAILED_SCHEMA`]): the verb's failure object
    /// with `context` {`request_key`, `rule_chain`} filled.
    pub fn to_json(&self) -> Value {
        let mut failure = self.failure.clone();
        let request: Value = match &self.request {
            Some(request) => json!({
                "library": request.library,
                "inputs": request.inputs.iter().map(|input| json!({
                    "presentation": input.presentation.0, "family": input.family, "powers": input.powers,
                })).collect::<Vec<_>>(),
                "reproduce": request.reproduce,
            }),
            None => Value::Null,
        };
        let chain: Value = match &self.rule_chain {
            Some(chain) => serde_json::to_value(chain).unwrap_or(Value::Null),
            None => json!([]),
        };
        if failure.is_object() {
            failure["context"] = json!({
                "request_key": request.get("inputs").cloned().unwrap_or(Value::Null),
                "rule_chain": chain,
            });
        }
        json!({
            "schema": RECIPE_FAILED_SCHEMA,
            "node": self.node,
            "recipe": {"id": self.recipe_id, "digest": self.recipe_digest, "kind": self.recipe_kind,
                       "relation_rule": self.relation_rule},
            "key": self.key, "key_table_frame": self.key_table,
            "kind": self.kind.name(), "class": self.class, "message": self.message,
            "request": request, "rule_chain": chain, "rule_chain_note": self.rule_chain_note,
            "reproduce": self.reproduce, "reproduce_dir": self.reproduce_dir,
            "determinism_id": self.determinism_id,
            "failure": failure,
        })
    }
}

impl fmt::Display for PRecipeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "recipe {kind} failed for boxed node {node}: recipe {id:?} ({rkind}, relation rule {rule}, digest {digest}) \
             could not solve the key {key} (recipe frame {ktable}): {class}: {message}",
            kind = self.kind.name(),
            node = self.node,
            id = self.recipe_id,
            rkind = self.recipe_kind,
            rule = self.relation_rule,
            digest = self.recipe_digest.get(..16).unwrap_or(&self.recipe_digest),
            key = powers_text(&self.key),
            ktable = powers_text(&self.key_table),
            class = self.class,
            message = self.message,
        )?;
        match &self.request {
            Some(request) => write!(formatter, "; original request: {}", request.inputs_text())?,
            None => write!(formatter, "; original request: not recorded")?,
        }
        match (&self.rule_chain, &self.rule_chain_note) {
            (Some(chain), _) => {
                let links: Vec<String> = chain
                    .iter()
                    .map(|link| format!("{} [{}]", powers_text(&link.key), link.via))
                    .collect();
                write!(formatter, "; rule chain: {}", links.join(" -> "))?;
            }
            (None, Some(note)) => write!(formatter, "; rule chain: {note}")?,
            (None, None) => {}
        }
        write!(
            formatter,
            "; no fallback was tried (a recipe failure is never retried by another rule); reproduce the solve: {}",
            self.reproduce
        )?;
        if let Some(request) = &self.request {
            write!(formatter, "; reproduce the request: {}", request.reproduce)?;
        }
        Ok(())
    }
}

// ----- the solver -----

/// The engine's solve at a key: the recipe verb's record (`results.jsonl`
/// form) for `record` of `bundle` at `key_table` (TABLE slots).
pub trait PRecipeSolver: Send + Sync {
    fn solve(
        &self,
        bundle: &RecipeBundle,
        record: &RecipeRecord,
        key_table: &[i64],
    ) -> Result<Value, String>;

    /// [`Self::solve`] within a wall bound: `Err` starting with
    /// [`BUDGET_WALL_PREFIX`] when it passed (default: no bound).
    fn solve_within(
        &self,
        bundle: &RecipeBundle,
        record: &RecipeRecord,
        key_table: &[i64],
        _budget: Option<std::time::Duration>,
    ) -> Result<Value, String> {
        self.solve(bundle, record, key_table)
    }
}

/// The error text of a solve stopped by its wall bound.
pub const BUDGET_WALL_PREFIX: &str = "budget_wall:";

type Job = (
    String,
    Vec<i64>,
    mpsc::Sender<Result<Value, String>>,
    Arc<std::sync::atomic::AtomicU8>,
);

/// Job states: the waiter and the worker agree on who owns a solve that
/// passed its wall bound (exactly one of them wins the transition).
const JOB_PENDING: u8 = 0;
const JOB_DONE: u8 = 1;
const JOB_ABANDONED: u8 = 2;

/// Lifts abandoned at their wall bound and still running (a lift cannot be
/// interrupted: its thread ends when the lift does).
static ABANDONED_RUNNING: AtomicU64 = AtomicU64::new(0);

/// At most this many abandoned lifts may run at once (default: the solver's
/// worker count, at least 1): while the cap is reached a bounded solve is
/// refused at once (typed BUDGET), so abandoned lifts never pile up on the
/// CPUs and memory of a request. (The former default 1 refused every later
/// bounded solve of the process once one solve had passed its time bound.)
pub const OUTSIDE_MAX_ABANDONED_ENV: &str = "RUSTRED_P_OUTSIDE_MAX_ABANDONED";

/// Abandoned lifts still running in this process.
pub fn abandoned_solves_running() -> u64 {
    ABANDONED_RUNNING.load(Ordering::Relaxed)
}

/// One (chart, symmetry rows, work limit) group: engine sessions in worker
/// threads, each prepared once, fed by one queue.
struct Group {
    queue: Mutex<mpsc::Sender<Job>>,
}

/// The real solver: `node::recipe::session_group` in worker threads (one key
/// on one thread, so the result does not depend on the worker count).
/// (chart, symmetry rows, trace-row cap) of a prepared session group.
type GroupKey = (String, bool, Option<usize>);

pub struct EngineRecipeSolver {
    groups: Mutex<HashMap<GroupKey, Arc<Group>>>,
    workers: usize,
}

impl Default for EngineRecipeSolver {
    fn default() -> Self {
        Self::new(
            std::env::var(RECIPE_WORKERS_ENV)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1),
        )
    }
}

impl EngineRecipeSolver {
    pub fn new(workers: usize) -> Self {
        Self {
            groups: Mutex::new(HashMap::new()),
            workers: workers.max(1),
        }
    }

    fn group(&self, bundle: &RecipeBundle, record: &RecipeRecord) -> Result<Arc<Group>, String> {
        let key = (
            record.chart().to_owned(),
            record.symmetry_rows(),
            record.max_trace_rows,
        );
        let mut groups = self
            .groups
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(group) = groups.get(&key) {
            return Ok(Arc::clone(group));
        }
        let order = crate::node::order::NodeOrder::from_json(&bundle.order.to_string())?;
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        let stack = std::env::var("RUST_MIN_STACK")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(32 << 20)
            .max(32 << 20);
        for worker in 0..self.workers {
            let receiver = Arc::clone(&receiver);
            let family = bundle.family_text.clone();
            let order = order.clone();
            let (chart, symmetry, rows) = key.clone();
            let mut opts = crate::node::recipe::BatchOptions::new(binary_identity().clone());
            opts.max_trace_rows = rows;
            opts.order_text = order.to_json();
            std::thread::Builder::new()
                .name(format!("recipe-{}-{worker}", bundle.node))
                .stack_size(stack)
                .spawn(move || {
                    let next = || {
                        receiver
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .recv()
                            .ok()
                    };
                    let served = crate::node::recipe::session_group(
                        &family,
                        &order,
                        &chart,
                        symmetry,
                        &opts,
                        |solve| {
                            while let Some((recipe, key, reply, state)) = next() {
                                let result = solve(&recipe, &key);
                                let mine = state
                                    .compare_exchange(
                                        JOB_PENDING,
                                        JOB_DONE,
                                        Ordering::AcqRel,
                                        Ordering::Acquire,
                                    )
                                    .is_ok();
                                if mine {
                                    let _ = reply.send(Ok(result));
                                } else {
                                    // abandoned at its wall bound: this lift is over
                                    ABANDONED_RUNNING.fetch_sub(1, Ordering::Relaxed);
                                }
                            }
                        },
                    );
                    // A preparation failure answers every job with it.
                    if let Err(error) = served {
                        while let Some((_, _, reply, _)) = next() {
                            let _ = reply.send(Err(format!("engine preparation: {error}")));
                        }
                    }
                })
                .map_err(|error| format!("recipe worker thread: {error}"))?;
        }
        let group = Arc::new(Group {
            queue: Mutex::new(sender),
        });
        groups.insert(key, Arc::clone(&group));
        Ok(group)
    }
}

impl EngineRecipeSolver {
    /// The cap on abandoned lifts running at once ([`OUTSIDE_MAX_ABANDONED_ENV`]; default the worker count).
    fn abandoned_cap(&self) -> u64 {
        env_cap(OUTSIDE_MAX_ABANDONED_ENV, self.workers.max(1)) as u64
    }
}

impl PRecipeSolver for EngineRecipeSolver {
    fn solve(
        &self,
        bundle: &RecipeBundle,
        record: &RecipeRecord,
        key_table: &[i64],
    ) -> Result<Value, String> {
        self.solve_within(bundle, record, key_table, None)
    }

    /// A lift cannot be interrupted: past the bound the wait stops, the
    /// group is retired (its busy worker ends after its lift, the next solve
    /// of that group prepares a fresh one) and the solve is typed BUDGET.
    fn solve_within(
        &self,
        bundle: &RecipeBundle,
        record: &RecipeRecord,
        key_table: &[i64],
        budget: Option<std::time::Duration>,
    ) -> Result<Value, String> {
        if budget.is_some() {
            let cap = self.abandoned_cap();
            let running = ABANDONED_RUNNING.load(Ordering::Relaxed);
            if running >= cap {
                return Err(format!(
                    "{BUDGET_WALL_PREFIX} {running} earlier solve(s) over their wall bound still running (cap {cap}); \
                     no new solve started"
                ));
            }
        }
        let group = self.group(bundle, record)?;
        let (reply, answer) = mpsc::channel();
        let state = Arc::new(std::sync::atomic::AtomicU8::new(JOB_PENDING));
        group
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send((
                record.recipe.to_string(),
                key_table.to_vec(),
                reply,
                Arc::clone(&state),
            ))
            .map_err(|_| "the recipe worker stopped".to_owned())?;
        let Some(budget) = budget else {
            return answer
                .recv()
                .map_err(|_| "the recipe worker stopped without an answer".to_owned())?;
        };
        match answer.recv_timeout(budget) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if state
                    .compare_exchange(
                        JOB_PENDING,
                        JOB_ABANDONED,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_err()
                {
                    // the lift ended just now: its answer is on its way
                    return answer
                        .recv()
                        .map_err(|_| "the recipe worker stopped without an answer".to_owned())?;
                }
                ABANDONED_RUNNING.fetch_add(1, Ordering::Relaxed);
                let key = (
                    record.chart().to_owned(),
                    record.symmetry_rows(),
                    record.max_trace_rows,
                );
                self.groups
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&key);
                Err(format!(
                    "{BUDGET_WALL_PREFIX} the solve exceeded its wall bound of {:.0} s",
                    budget.as_secs_f64()
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("the recipe worker stopped without an answer".to_owned())
            }
        }
    }
}

/// The identity of the running binary (path, BLAKE3, git commit), once.
fn binary_identity() -> &'static Value {
    static IDENTITY: OnceLock<Value> = OnceLock::new();
    IDENTITY.get_or_init(crate::node::recipe::binary_identity)
}

/// Solvers installed for a node name (tests, embedders): every dispatcher of
/// that node built after the call uses it instead of the engine solver.
static SOLVER_OVERRIDES: RwLock<BTreeMap<String, Arc<dyn PRecipeSolver>>> =
    RwLock::new(BTreeMap::new());

/// Install `solver` for the boxed node `node` (dispatchers built later).
pub fn override_recipe_solver(node: &str, solver: Arc<dyn PRecipeSolver>) {
    SOLVER_OVERRIDES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(node.to_owned(), solver);
}

fn solver_for(node: &str) -> Arc<dyn PRecipeSolver> {
    SOLVER_OVERRIDES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(node)
        .cloned()
        .unwrap_or_else(|| Arc::new(EngineRecipeSolver::default()))
}

// ----- the relation cache -----

/// Cache roots of this process: the user cache (read-write) and the
/// pre-filled overlays (read-only), set by [`set_recipe_cache_roots`], else
/// read once from [`RECIPE_CACHE_ENV`] and [`RECIPE_PREFILLED_ENV`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecipeCacheRoots {
    pub user: Option<PathBuf>,
    pub prefilled: Vec<PathBuf>,
    /// Size cap of the user root's relation entries (bytes); `None`: the
    /// environment ([`super::relation_entries::MAX_BYTES_ENV`]), else none.
    pub max_bytes: Option<u64>,
    /// Whether relation entries of evidence level `none` are used; `None`:
    /// the environment ([`super::relation_entries::ALLOW_FIELD_ENV`]), else yes.
    pub allow_field: Option<bool>,
}

static CACHE_ROOTS: RwLock<Option<RecipeCacheRoots>> = RwLock::new(None);

/// Set the cache roots of the dispatchers built after this call.
pub fn set_recipe_cache_roots(roots: RecipeCacheRoots) {
    *CACHE_ROOTS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(roots);
}

/// Cache roots installed for one node name (tests, embedders that keep the
/// caches of several nodes apart); they win over the process roots.
static NODE_CACHE_ROOTS: RwLock<BTreeMap<String, RecipeCacheRoots>> = RwLock::new(BTreeMap::new());

/// Set the cache roots of the dispatchers of node `node` built after this call.
pub fn set_recipe_cache_roots_for(node: &str, roots: RecipeCacheRoots) {
    NODE_CACHE_ROOTS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(node.to_owned(), roots);
}

/// The cache roots in force for node `node`.
pub fn recipe_cache_roots_of(node: &str) -> RecipeCacheRoots {
    if let Some(roots) = NODE_CACHE_ROOTS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(node)
        .cloned()
    {
        return roots;
    }
    recipe_cache_roots()
}

/// The cache roots in force.
pub fn recipe_cache_roots() -> RecipeCacheRoots {
    if let Some(roots) = CACHE_ROOTS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
    {
        return roots;
    }
    RecipeCacheRoots {
        user: std::env::var_os(RECIPE_CACHE_ENV)
            .or_else(|| std::env::var_os(RECIPE_CACHE_ENV_OLD))
            .filter(|value| !value.is_empty())
            .map(PathBuf::from),
        prefilled: std::env::var(RECIPE_PREFILLED_ENV)
            .or_else(|_| std::env::var(RECIPE_PREFILLED_ENV_OLD))
            .map(|value| {
                value
                    .split(':')
                    .filter(|part| !part.is_empty())
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default(),
        max_bytes: None,
        allow_field: None,
    }
}

/// The bytes of relation entries on disk under the user (writable) root in
/// force for this process (all nodes; 0 without a root).
pub fn relation_cache_bytes_on_disk() -> u64 {
    recipe_cache_roots()
        .user
        .as_deref()
        .map_or(0, super::relation_entries::bytes_on_disk)
}

/// vk5l WP2 producer: the CHECKER recipe family at `key_table` (the family
/// that solved 969 of the 1,000 heaviest T266 rules at their keys,
/// artefacts/size/recipe_vs_table_T266.json): one centre one step DOWN the
/// first dotted line (else one step UP the first negative numerator, else the
/// key), `min_offset` -1 on every dotted line, sector filter, symmetry rows,
/// relation rule `m1_first`, shells S0..S2 (depth 2) or S0..S3 (depth >= 3);
/// `symmetry_rows: false` gives the `-nosym` variant.
pub fn checker_family(node: &str, key_table: &[i64], depth: u8, symmetry_rows: bool) -> Value {
    let mut centre = vec![0_i64; key_table.len()];
    let dotted: Vec<usize> = (0..key_table.len())
        .filter(|&slot| key_table[slot] > 1)
        .collect();
    if let Some(&slot) = dotted.first() {
        centre[slot] = -1;
    } else if let Some(slot) = key_table.iter().position(|&power| power < 0) {
        centre[slot] = 1;
    }
    let chart: String = key_table
        .iter()
        .map(|&power| if power > 0 { '1' } else { '0' })
        .collect();
    let shells: &[&str] = if depth >= 3 {
        &["S0", "S1", "Sm1", "S1m1", "S2", "S3"]
    } else {
        &["S0", "S1", "Sm1", "S1m1", "S2"]
    };
    let min_offset: serde_json::Map<String, Value> = dotted
        .iter()
        .map(|slot| (slot.to_string(), json!(-1)))
        .collect();
    let variant = if symmetry_rows { "" } else { "-nosym" };
    json!({"schema": "rustred.recipe.v1", "id": format!("{node}/checker-d{depth}{variant}"),
        "arity": key_table.len(), "chart": chart, "domain": {"keys": [key_table]},
        "seeds": {"centres": [centre], "generator": {"shells": shells}, "min_offset": min_offset,
                  "sector_filter": true},
        "symmetry_rows": symmetry_rows, "relation_rule": "m1_first"})
}

/// The node identity of a bundle (relation entries; family text + node
/// order + arity, [`super::relation_entries::node_identity`]).
pub fn bundle_node_identity(bundle: &RecipeBundle) -> String {
    super::relation_entries::node_identity(
        &bundle.family_text,
        &bundle.order,
        bundle.table_to_presentation.len(),
    )
}

/// The directory of `bundle`'s node under a relation cache root.
pub fn recipe_cache_directory(root: &Path, bundle: &RecipeBundle) -> PathBuf {
    super::relation_entries::node_directory(root, &bundle.node, &bundle_node_identity(bundle))
}

/// Where a relation came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationSource {
    Solved,
    Memo,
    /// A shipped / pre-filled (read-only) relation entry.
    Prefilled,
    /// The persistent run-time cache.
    UserCache,
}

/// The multiplicity of the root `at` of `coefficient`'s denominator (a
/// univariate polynomial in d): the first derivative not vanishing there.
/// `None` for a coefficient in more than one variable.
pub fn pole_order_at(coefficient: &Coefficient, at: i64) -> Option<u32> {
    let denominator = &coefficient.denominator;
    if denominator.is_constant() {
        return Some(0);
    }
    if denominator.nvars() != 1 {
        return None;
    }
    let point = symbolica::domains::integer::Integer::from(at);
    let mut polynomial = denominator.clone();
    for order in 0..=u32::from(polynomial.degree(0)) {
        if !polynomial.replace(0, &point).is_zero() {
            return Some(order);
        }
        polynomial = polynomial.derivative(0);
    }
    None
}

// ----- the dispatcher -----

/// An outside key's relation and the id of the family that solved it.
type OutsideSolved = (Arc<Relation>, String);
/// Solved relations by (record index, key).
type RelationMemo = HashMap<(usize, Vec<i64>), Arc<Relation>>;

/// A solved relation of one recipe key: children (presentation slots) with
/// their exact coefficients in the presentation's convention.
#[derive(Clone, Debug, PartialEq)]
pub struct Relation {
    pub children: Vec<(Vec<i64>, Coefficient)>,
    /// The engine's relation digest.
    pub relation_digest: Option<String>,
    /// The largest multiplicity of the factor (d-4) in a coefficient's
    /// denominator (0: regular at d = 4; `None`: not decidable).
    pub pole_order_at_4: Option<u32>,
}

/// Process-wide counters of the recipe dispatch (reports, tests).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecipeStatistics {
    pub solves: u64,
    pub solve_seconds: f64,
    pub memo_hits: u64,
    pub prefilled_hits: u64,
    pub user_cache_hits: u64,
    pub failures: u64,
    /// Run-time solves at keys outside a certified set (each attempt) and
    /// the outside keys no recipe family solved.
    pub outside_solves: u64,
    pub outside_failures: u64,
    /// Outside keys taken by a table rule / served by a relation entry, and
    /// outside solves stopped by a wall bound.
    pub outside_rule_steps: u64,
    pub outside_entry_hits: u64,
    pub outside_budget_hits: u64,
    /// Keys outside a certified set left unsolved in list-only mode
    /// (`max_recipes` 0): no rule or entry took them.
    pub outside_listed: u64,
}

static SOLVES: AtomicU64 = AtomicU64::new(0);
static SOLVE_MICROS: AtomicU64 = AtomicU64::new(0);
static MEMO_HITS: AtomicU64 = AtomicU64::new(0);
static PREFILLED_HITS: AtomicU64 = AtomicU64::new(0);
static USER_HITS: AtomicU64 = AtomicU64::new(0);
static FAILURES: AtomicU64 = AtomicU64::new(0);
static OUTSIDE_SOLVES: AtomicU64 = AtomicU64::new(0);
static OUTSIDE_FAILURES: AtomicU64 = AtomicU64::new(0);
static OUTSIDE_RULE_STEPS: AtomicU64 = AtomicU64::new(0);
static OUTSIDE_ENTRY_HITS: AtomicU64 = AtomicU64::new(0);
static OUTSIDE_LISTED: AtomicU64 = AtomicU64::new(0);
static OUTSIDE_BUDGET_HITS: AtomicU64 = AtomicU64::new(0);

/// vk5l WP2: an outside key taken by a table rule (counted by the step).
pub fn count_outside_rule_step() {
    OUTSIDE_RULE_STEPS.fetch_add(1, Ordering::Relaxed);
}

/// vk5l WP2: an outside key served by a shipped entry before the table.
pub fn count_outside_entry_hit() {
    OUTSIDE_ENTRY_HITS.fetch_add(1, Ordering::Relaxed);
}

/// The counters since the start of the process.
pub fn recipe_statistics() -> RecipeStatistics {
    RecipeStatistics {
        solves: SOLVES.load(Ordering::Relaxed),
        solve_seconds: SOLVE_MICROS.load(Ordering::Relaxed) as f64 / 1e6,
        memo_hits: MEMO_HITS.load(Ordering::Relaxed),
        prefilled_hits: PREFILLED_HITS.load(Ordering::Relaxed),
        user_cache_hits: USER_HITS.load(Ordering::Relaxed),
        failures: FAILURES.load(Ordering::Relaxed),
        outside_solves: OUTSIDE_SOLVES.load(Ordering::Relaxed),
        outside_failures: OUTSIDE_FAILURES.load(Ordering::Relaxed),
        outside_rule_steps: OUTSIDE_RULE_STEPS.load(Ordering::Relaxed),
        outside_entry_hits: OUTSIDE_ENTRY_HITS.load(Ordering::Relaxed),
        outside_budget_hits: OUTSIDE_BUDGET_HITS.load(Ordering::Relaxed),
        outside_listed: OUTSIDE_LISTED.load(Ordering::Relaxed),
    }
}

/// The recipe dispatch of one boxed Custom owner.
pub struct RecipeDispatch {
    bundle: RecipeBundle,
    /// Explicit keys (presentation slots) -> record index (first record wins).
    keys: HashMap<Vec<i64>, usize>,
    /// Predicate records, in record order.
    predicates: Vec<usize>,
    /// batch 12a: records at a table position (unboxed tables), record order.
    positioned: Vec<usize>,
    solver: Arc<dyn PRecipeSolver>,
    cache: super::relation_entries::RelationStore,
    node_identity: String,
    memo: Mutex<RelationMemo>,
    /// Outside keys already solved in this process, with the family's id.
    outside: Mutex<HashMap<Vec<i64>, OutsideSolved>>,
    context: CoefficientContext,
}

impl fmt::Debug for RecipeDispatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecipeDispatch")
            .field("bundle", &self.bundle.describe())
            .field("cache", &self.cache)
            .finish_non_exhaustive()
    }
}

impl RecipeDispatch {
    /// The dispatch of `bundle` on an owner of `arity` slots and `support`
    /// lines, its caches opened under the roots in force.
    pub fn new(
        bundle: RecipeBundle,
        arity: usize,
        family_fingerprint: &str,
        context: &CoefficientContext,
        support: u16,
    ) -> Result<Self, PError> {
        if bundle.table_to_presentation.len() != arity {
            return Err(malformed(format!(
                "a slot map of {} slots on an owner of {arity}",
                bundle.table_to_presentation.len()
            )));
        }
        let mut keys = HashMap::new();
        let mut predicates = Vec::new();
        let mut positioned = Vec::new();
        for (index, record) in bundle.records.iter().enumerate() {
            if record.position.is_some() {
                // batch 12a: dispatched by the owner in table order, never by `find`
                positioned.push(index);
                continue;
            }
            match &record.domain {
                RecipeDomain::Keys(list) => {
                    for key in list {
                        keys.entry(bundle.to_presentation(key)).or_insert(index);
                    }
                }
                RecipeDomain::Predicate => predicates.push(index),
            }
        }
        let _ = (family_fingerprint, support);
        let roots = recipe_cache_roots_of(&bundle.node);
        let node_identity = bundle_node_identity(&bundle);
        let cache = super::relation_entries::RelationStore::with_options(
            &bundle.node,
            &node_identity,
            &roots.prefilled,
            roots.user.as_deref(),
            super::relation_entries::StoreOptions {
                max_bytes: roots.max_bytes,
                allow_field: roots.allow_field,
            },
        );
        Ok(Self {
            node_identity,
            solver: solver_for(&bundle.node),
            bundle,
            keys,
            predicates,
            positioned,
            cache,
            memo: Mutex::new(HashMap::new()),
            outside: Mutex::new(HashMap::new()),
            context: context.clone(),
        })
    }

    pub fn bundle(&self) -> &RecipeBundle {
        &self.bundle
    }

    pub fn order(&self) -> RecipeDispatchOrder {
        self.bundle.dispatch
    }

    pub fn cache(&self) -> &super::relation_entries::RelationStore {
        &self.cache
    }

    /// Every explicit key (presentation slots) with its record index.
    pub fn explicit_keys(&self) -> Vec<(Vec<i64>, usize)> {
        let mut keys: Vec<_> = self.keys.iter().map(|(k, &i)| (k.clone(), i)).collect();
        keys.sort();
        keys
    }

    /// batch 12a: the first positioned record whose domain holds `key`
    /// (presentation slots), with the number of its chart's ordinary rules
    /// that precede it in first match.
    pub fn positioned(&self, key: &[i64]) -> Option<(usize, usize)> {
        if self.positioned.is_empty() {
            return None;
        }
        let table = self.bundle.to_table(key);
        self.positioned.iter().copied().find_map(|index| {
            let record = &self.bundle.records[index];
            record.holds_at_position(&table).then(|| {
                (
                    index,
                    record.position.as_ref().expect("positioned").rules_before,
                )
            })
        })
    }

    /// Every record of the bundle stands at a table position (an unboxed
    /// table's recipes).
    pub fn all_positioned(&self) -> bool {
        !self.bundle.records.is_empty() && self.positioned.len() == self.bundle.records.len()
    }

    /// The record whose domain holds `key` (presentation slots), if any; the
    /// caller has checked the node's certified set.
    pub fn find(&self, key: &[i64]) -> Option<usize> {
        if let Some(&index) = self.keys.get(key) {
            return Some(index);
        }
        if self.predicates.is_empty() {
            return None;
        }
        let table = self.bundle.to_table(key);
        self.predicates
            .iter()
            .copied()
            .find(|&index| predicate_holds(&self.bundle.records[index].recipe, &table))
    }

    /// The relation of `record` at `key` already known (memo or cache), no
    /// solve.
    pub fn known(&self, record: usize, key: &[i64]) -> Option<Arc<Relation>> {
        self.memo
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(record, key.to_vec()))
            .cloned()
    }

    /// The relation of record `index` at `key` (presentation slots): memo,
    /// caches, else a verified engine solve (stored).
    pub fn relation(
        &self,
        index: usize,
        key: &[i64],
    ) -> Result<(Arc<Relation>, RelationSource), PError> {
        if let Some(found) = self.known(index, key) {
            MEMO_HITS.fetch_add(1, Ordering::Relaxed);
            return Ok((found, RelationSource::Memo));
        }
        let record = &self.bundle.records[index];
        let key_table = self.bundle.to_table(key);
        if let Some((relation, source)) = self.stored(record, key, &key_table)? {
            let relation = Arc::new(relation);
            self.remember(index, key, &relation);
            return Ok((relation, source));
        }
        let started = Instant::now();
        let solved = self.solver.solve(&self.bundle, record, &key_table);
        SOLVES.fetch_add(1, Ordering::Relaxed);
        SOLVE_MICROS.fetch_add(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        let verified = match &solved {
            Ok(value) => self.verified(record, key, &key_table, value),
            Err(error) => Err(self.failure(
                record,
                key,
                &key_table,
                PRecipeFailureKind::Error,
                "error",
                &format!("the engine solve did not run: {error}"),
                json!({"message": error}),
            )),
        };
        let relation = match verified {
            Ok(relation) => Arc::new(relation),
            Err(error) => {
                FAILURES.fetch_add(1, Ordering::Relaxed);
                return Err(error);
            }
        };
        if let Ok(value) = &solved {
            self.insert(
                record,
                record,
                &key_table,
                value,
                &relation,
                false,
                "not_tried",
            );
        }
        self.remember(index, key, &relation);
        Ok((relation, RelationSource::Solved))
    }

    /// The relation entry of `key_table` from the shipped roots or the
    /// persistent cache, checked like a fresh solve (an entry that fails the
    /// check, e.g. another relation than the certified digest, is ignored
    /// with one stderr line and the key is solved).
    fn stored(
        &self,
        record: &RecipeRecord,
        key: &[i64],
        key_table: &[i64],
    ) -> Result<Option<(Relation, RelationSource)>, PError> {
        let Some((entry, source)) = self.cache.get(key_table) else {
            return Ok(None);
        };
        Ok(self.checked_entry(record, key, key_table, &entry, source))
    }

    /// An entry read from the store, checked as a fresh solve ([`Self::verified`]).
    fn checked_entry(
        &self,
        record: &RecipeRecord,
        key: &[i64],
        key_table: &[i64],
        entry: &Value,
        source: super::relation_entries::StoreSource,
    ) -> Option<(Relation, RelationSource)> {
        match self.verified(record, key, key_table, &entry["body"]) {
            Ok(relation) => {
                let source = match source {
                    super::relation_entries::StoreSource::Shipped => {
                        PREFILLED_HITS.fetch_add(1, Ordering::Relaxed);
                        RelationSource::Prefilled
                    }
                    super::relation_entries::StoreSource::Cache => {
                        USER_HITS.fetch_add(1, Ordering::Relaxed);
                        RelationSource::UserCache
                    }
                };
                Some((relation, source))
            }
            Err(error) => {
                eprintln!(
                    "rustred: relation entry {} of {} at {key_table:?} not used: {error}",
                    entry["entry_digest"], self.bundle.node
                );
                None
            }
        }
    }

    /// Write the entry of a fresh solve into the persistent cache (a write
    /// failure is reported on stderr, never fails the reduction).
    #[allow(clippy::too_many_arguments)]
    fn insert(
        &self,
        record: &RecipeRecord,
        solved_with: &RecipeRecord,
        key_table: &[i64],
        value: &Value,
        relation: &Relation,
        outside: bool,
        regular_alternative: &'static str,
    ) {
        let meta = super::relation_entries::EntryMeta {
            node: self.bundle.node.clone(),
            node_identity: self.node_identity.clone(),
            package_revision: Some(self.bundle.revision.clone()),
            recipe_id: record.id.clone(),
            recipe: solved_with.recipe.clone(),
            recipe_digest: solved_with.recipe_digest.clone(),
            relation_rule: solved_with.relation_rule().to_owned(),
            evidence_level: "per-key",
            pole_order_at_4: relation.pole_order_at_4,
            regular_alternative,
            outside_certified_set: outside,
            written_by: format!("rustred {} run-time", env!("CARGO_PKG_VERSION")),
        };
        let body = super::relation_entries::entry_body(value, key_table);
        let entry = super::relation_entries::build_entry(&meta, key_table, body);
        if let Err(error) = self.cache.put(&entry) {
            eprintln!(
                "rustred: relation cache of {}: entry at {key_table:?} not written: {error}",
                self.bundle.node
            );
        }
    }

    /// vk5l WP2 (user rulings 2026-11-14): a key OUTSIDE the certified set
    /// that no rule took, solved at run time. Order: the in-process memo,
    /// the shipped relation entries, the persistent cache, then the recipe
    /// FAMILIES at the key: the numerator-centred depth-2 family (centres one
    /// step up on each negative numerator slot, shells S0..S2, sector filter,
    /// relation rule m1_first) when the key has a negative numerator, then
    /// every record of the key's chart, nearest certified key first (one per
    /// distinct seed set), each re-centred on THIS key; at most
    /// [`OUTSIDE_MAX_RECIPES_ENV`] families under
    /// [`OUTSIDE_MAX_TRACE_ROWS_ENV`] trace rows each. A relation counts only
    /// when SOLVED with an exact proof and every key lower. With the d = 4
    /// preference ([`PREFER_REGULAR_ENV`], on by default) a relation with a
    /// (d-4) factor in a denominator is kept as a fallback while the other
    /// families are tried for a regular one. The winner goes to the memo and
    /// the persistent cache. No family solves:
    /// [`PRecipeFailureKind::OutsideNoRelation`] with every attempt.
    /// Returns the relation and the id of the family that solved it.
    pub fn solve_outside(&self, key: &[i64]) -> Result<Option<(Arc<Relation>, String)>, PError> {
        // shipped entries and the persistent cache first (with or without
        // recipe families)
        if let Some(found) = self.served_entry(key)? {
            OUTSIDE_ENTRY_HITS.fetch_add(1, Ordering::Relaxed);
            return Ok(Some(found));
        }
        if outside_settings_of(&self.bundle.node).max_recipes == 0 {
            // list-only: the caller's typed refusal (retained under Retain)
            OUTSIDE_LISTED.fetch_add(1, Ordering::Relaxed);
            return Ok(None);
        }
        self.solve_at_key(key, Vec::new(), true, None)
    }

    /// vk5l WP2 producer (plan/ARTIFACT_MANIFEST.md): an EXPLICIT relation at
    /// `key` (presentation slots; any key of the node, inside or outside the
    /// certified set, whatever rule serves it): an entry already in the
    /// stores is returned (resumable), else the checker families of
    /// `depths` ([`checker_family`]) re-centred on the key, then the outside
    /// families, under the node's [`POutsideSettings`] (bounds, d = 4
    /// preference); the winner is written to the writable cache root with
    /// `outside_certified_set = outside`. `Ok(None)`: no family at all.
    /// With `closure` (the certified set, TABLE frame) a relation counts only
    /// when every top-sector key of its right-hand side is in the set (else
    /// the attempt is `RHS_OUTSIDE_CERTIFIED` and the next family is tried;
    /// each checker depth then also has a variant without symmetry rows,
    /// whose relations do not trade a key for its mirror image).
    pub fn produce(
        &self,
        key: &[i64],
        depths: &[u8],
        outside: bool,
        closure: Option<&super::certified_keys::CertifiedKeySet>,
    ) -> Result<Option<(Arc<Relation>, String)>, PError> {
        if let Some(found) = self.served_entry(key)? {
            return Ok(Some(found));
        }
        let key_table = self.bundle.to_table(key);
        let settings = outside_settings_of(&self.bundle.node);
        let mut first = Vec::new();
        for &depth in depths {
            let variants: &[bool] = if closure.is_some() {
                &[true, false]
            } else {
                &[true]
            };
            for &symmetry_rows in variants {
                let recipe = checker_family(&self.bundle.node, &key_table, depth, symmetry_rows);
                let id = recipe["id"].as_str().unwrap_or("").to_owned();
                let mut record = RecipeBundle::record_of(&recipe.to_string())?;
                record.max_trace_rows = Some(settings.max_trace_rows.max(1));
                first.push((id, record));
            }
        }
        self.solve_at_key(key, first, outside, closure)
    }

    /// The top-sector keys of `relation` (TABLE frame) that `closure` does not
    /// hold (`key` in presentation slots).
    fn rhs_outside(
        &self,
        key: &[i64],
        relation: &Relation,
        closure: &super::certified_keys::CertifiedKeySet,
    ) -> Vec<Vec<i64>> {
        let sector = |powers: &[i64]| powers.iter().map(|&p| p > 0).collect::<Vec<bool>>();
        let top = sector(key);
        relation
            .children
            .iter()
            .filter(|(child, _)| sector(child) == top)
            .map(|(child, _)| self.bundle.to_table(child))
            .filter(|child| !closure.contains(child))
            .collect()
    }

    /// The bounded family loop of [`Self::solve_outside`] /
    /// [`Self::produce`]: `first` families, then the outside families.
    fn solve_at_key(
        &self,
        key: &[i64],
        first: Vec<(String, RecipeRecord)>,
        outside: bool,
        closure: Option<&super::certified_keys::CertifiedKeySet>,
    ) -> Result<Option<(Arc<Relation>, String)>, PError> {
        let key_table = self.bundle.to_table(key);
        let chart: String = key_table
            .iter()
            .map(|&p| if p > 0 { '1' } else { '0' })
            .collect();
        let settings = outside_settings_of(&self.bundle.node);
        let max_recipes = settings.max_recipes.max(1);
        let max_rows = settings.max_trace_rows.max(1);
        let mut families = first;
        families.extend(self.outside_families(&key_table, &chart, max_rows)?);
        if families.is_empty() {
            // no entry and no recipe family: the caller's typed refusal
            return Ok(None);
        }
        let prefer_regular = settings.prefer_regular_at_4;
        let tried = families.len().min(max_recipes);
        let mut attempts = Vec::new();
        let mut first_failure: Option<PRecipeFailure> = None;
        let mut fallback: Option<(Arc<Relation>, String, RecipeRecord, Value)> = None;
        let mut too_large = false;
        let key_started = Instant::now();
        for (id, at_key) in families.iter().take(max_recipes) {
            let left = settings
                .max_seconds_per_key
                .map(|limit| limit - key_started.elapsed().as_secs_f64());
            if left.is_some_and(|left| left <= 0.0) {
                attempts.push(
                    json!({"recipe": id, "kind": "BUDGET", "class": "budget_key",
                    "message": "the wall bound of this key was used up before this family"}),
                );
                OUTSIDE_BUDGET_HITS.fetch_add(1, Ordering::Relaxed);
                break;
            }
            let bound = match (settings.max_seconds_per_solve, left) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
            .map(|seconds| std::time::Duration::from_secs_f64(seconds.max(0.001)));
            let started = Instant::now();
            let solved = self
                .solver
                .solve_within(&self.bundle, at_key, &key_table, bound);
            OUTSIDE_SOLVES.fetch_add(1, Ordering::Relaxed);
            SOLVE_MICROS.fetch_add(started.elapsed().as_micros() as u64, Ordering::Relaxed);
            let verified = match &solved {
                Ok(value) => self.verified(at_key, key, &key_table, value),
                Err(error) if error.starts_with(BUDGET_WALL_PREFIX) => {
                    OUTSIDE_BUDGET_HITS.fetch_add(1, Ordering::Relaxed);
                    Err(self.failure(
                        at_key,
                        key,
                        &key_table,
                        PRecipeFailureKind::Budget,
                        "budget_wall",
                        error,
                        json!({"message": error, "bound_s": bound.map(|b| b.as_secs_f64())}),
                    ))
                }
                Err(error) => Err(self.failure(
                    at_key,
                    key,
                    &key_table,
                    PRecipeFailureKind::Error,
                    "error",
                    &format!("the engine solve did not run: {error}"),
                    json!({"message": error}),
                )),
            };
            match verified {
                Ok(relation) => {
                    if let Some(closure) = closure {
                        let leaving = self.rhs_outside(key, &relation, closure);
                        if !leaving.is_empty() {
                            attempts.push(json!({"recipe": id, "kind": "RHS_OUTSIDE_CERTIFIED",
                                "keys": leaving.len(), "first": leaving[0]}));
                            continue;
                        }
                    }
                    let relation = Arc::new(relation);
                    let value = solved.unwrap_or(Value::Null);
                    let regular = relation.pole_order_at_4 == Some(0);
                    attempts.push(json!({"recipe": id, "kind": "SOLVED",
                        "pole_order_at_4": relation.pole_order_at_4}));
                    let terms = relation.children.len() as u128;
                    // a regular relation wins unless a size ratio is set and it
                    // is more than p/q times the smallest pole relation found
                    let within_ratio = match (&fallback, settings.max_size_ratio) {
                        (Some((pole, ..)), Some((p, q))) => {
                            terms * u128::from(q) <= u128::from(p) * pole.children.len() as u128
                        }
                        _ => true,
                    };
                    if regular && !within_ratio {
                        too_large = true;
                        continue;
                    }
                    if regular || !prefer_regular {
                        let alternative = if regular { "yes" } else { "not_tried" };
                        return Ok(Some(self.keep_outside(
                            key,
                            &key_table,
                            id,
                            at_key,
                            &value,
                            relation,
                            alternative,
                            outside,
                        )));
                    }
                    let smaller = fallback
                        .as_ref()
                        .is_none_or(|(pole, ..)| relation.children.len() < pole.children.len());
                    if smaller {
                        fallback = Some((relation, id.clone(), at_key.clone(), value));
                    }
                }
                Err(PError::RecipeFailed(failed)) => {
                    attempts.push(json!({"recipe": id, "kind": failed.kind.name(),
                        "class": failed.class, "message": failed.message}));
                    first_failure.get_or_insert(*failed);
                }
                Err(other) => return Err(other),
            }
        }
        if let Some((relation, id, at_key, value)) = fallback {
            let alternative = if too_large { "too_large" } else { "no" };
            return Ok(Some(self.keep_outside(
                key,
                &key_table,
                &id,
                &at_key,
                &value,
                relation,
                alternative,
                outside,
            )));
        }
        OUTSIDE_FAILURES.fetch_add(1, Ordering::Relaxed);
        FAILURES.fetch_add(1, Ordering::Relaxed);
        let message = if outside {
            format!(
                "the key is outside the certified set of {}; no rule takes it and {} of {} recipe families of chart {chart} \
                 (at most {max_recipes}, {max_rows} trace rows each) gave no verified relation at it",
                self.bundle.node,
                tried,
                families.len()
            )
        } else {
            format!(
                "producer: {} of {} recipe families of chart {chart} (at most {max_recipes}, {max_rows} trace rows each) \
                 gave no verified relation at this key of {}",
                tried,
                families.len(),
                self.bundle.node
            )
        };
        let detail = json!({"outside_certified_set": outside, "chart": chart, "families": families.len(),
            "tried": tried, "max_recipes": max_recipes, "max_trace_rows": max_rows, "attempts": attempts});
        let error = match first_failure {
            Some(mut failed) => {
                // the reproduction of the first family stays
                failed.kind = PRecipeFailureKind::OutsideNoRelation;
                failed.class = "outside_no_relation".to_owned();
                failed.message = message;
                if failed.failure.is_object() {
                    failed.failure["outside"] = detail;
                }
                PError::RecipeFailed(Box::new(failed))
            }
            None => PError::RecipeFailed(Box::new(PRecipeFailure {
                node: self.bundle.node.clone(),
                recipe_id: String::new(),
                recipe_digest: String::new(),
                recipe_kind: "none",
                relation_rule: String::new(),
                key: key.to_vec(),
                key_table: key_table.clone(),
                kind: PRecipeFailureKind::OutsideNoRelation,
                class: "outside_no_relation".to_owned(),
                message,
                failure: json!({"schema": crate::node::recipe::FAILURE_SCHEMA,
                    "class": "outside_no_relation", "outside": detail}),
                reproduce: String::new(),
                reproduce_dir: None,
                determinism_id: crate::node::recipe::DETERMINISM_ID.to_owned(),
                request: None,
                rule_chain: None,
                rule_chain_note: Some("no recipe family of this chart".to_owned()),
            })),
        };
        Err(error)
    }

    /// vk5l WP2: the relation of `key` (presentation slots) from the
    /// in-process memo of served keys, the shipped relation entries or the
    /// persistent cache, with no solve and no recipe record needed (a boxed
    /// node without recipes is served by its entries alone). The entry is
    /// checked as a fresh solve is ([`Self::verified`]; an entry that fails is
    /// ignored with one stderr line). Returns the relation and the id of the
    /// recipe (or relation source) the entry names.
    /// vk5l WP2 (manifest): the relation of `key` from a shipped entry whose
    /// node manifest says `over_table` (used BEFORE the table's first match);
    /// `None` for every other key, with nothing read. Checked as
    /// [`Self::served_entry`].
    pub fn entry_over_table(&self, key: &[i64]) -> Result<Option<(Arc<Relation>, String)>, PError> {
        if !self.cache.over_table() {
            return Ok(None);
        }
        if !self.cache.is_over_table(&self.bundle.to_table(key)) {
            return Ok(None);
        }
        let found = self.served_entry(key)?;
        if found.is_some() {
            super::relation_entries::count_over_table_hit();
        }
        Ok(found)
    }

    pub fn served_entry(&self, key: &[i64]) -> Result<Option<(Arc<Relation>, String)>, PError> {
        if let Some((relation, id)) = self
            .outside
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
            .cloned()
        {
            MEMO_HITS.fetch_add(1, Ordering::Relaxed);
            return Ok(Some((relation, id)));
        }
        let key_table = self.bundle.to_table(key);
        let Some((entry, source)) = self.cache.get(&key_table) else {
            return Ok(None);
        };
        let id = entry["recipe"]["id"]
            .as_str()
            .map_or_else(|| format!("{}/entry", self.bundle.node), str::to_owned);
        let named = RecipeRecord {
            id: id.clone(),
            recipe: Value::Null,
            recipe_digest: entry["recipe"]["engine_digest"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
            domain: RecipeDomain::Keys(Vec::new()),
            relation_digests: BTreeMap::new(),
            max_trace_rows: None,
            position: None,
        };
        match self.checked_entry(&named, key, &key_table, &entry, source) {
            Some((relation, _)) => {
                let relation = Arc::new(relation);
                self.remember_outside(key, &relation, &id);
                Ok(Some((relation, id)))
            }
            None => Ok(None),
        }
    }

    /// The winner of an outside solve: memo + persistent cache.
    #[allow(clippy::too_many_arguments)]
    fn keep_outside(
        &self,
        key: &[i64],
        key_table: &[i64],
        id: &str,
        at_key: &RecipeRecord,
        value: &Value,
        relation: Arc<Relation>,
        alternative: &'static str,
        outside: bool,
    ) -> (Arc<Relation>, String) {
        let mut named = at_key.clone();
        named.id = id.to_owned();
        self.insert(
            &named,
            at_key,
            key_table,
            value,
            &relation,
            outside,
            alternative,
        );
        self.remember_outside(key, &relation, id);
        (relation, id.to_owned())
    }

    /// The recipe families tried at an outside key, in order, each as
    /// (family id, recipe re-centred on the key and capped).
    fn outside_families(
        &self,
        key_table: &[i64],
        chart: &str,
        max_rows: usize,
    ) -> Result<Vec<(String, RecipeRecord)>, PError> {
        let mut families = Vec::new();
        let numerators: Vec<Vec<i64>> = key_table
            .iter()
            .enumerate()
            .filter(|&(_, &power)| power < 0)
            .map(|(slot, _)| {
                let mut centre = vec![0_i64; key_table.len()];
                centre[slot] = 1;
                centre
            })
            .collect();
        if !numerators.is_empty() {
            let id = format!("{}/numerator-centred-d2", self.bundle.node);
            let recipe = json!({"schema": "rustred.recipe.v1", "id": id, "arity": key_table.len(),
                "chart": chart, "domain": {"keys": [key_table]},
                "seeds": {"centres": numerators,
                          "generator": {"shells": ["S0", "S1", "Sm1", "S1m1", "S2"]}, "sector_filter": true},
                "symmetry_rows": false, "relation_rule": "m1_first"});
            match RecipeBundle::record_of(&recipe.to_string()) {
                Ok(mut record) => {
                    record.max_trace_rows = Some(max_rows);
                    families.push((id, record));
                }
                Err(error) => eprintln!("rustred: numerator-centred family not built: {error}"),
            }
        }
        let mut candidates: Vec<(i64, usize)> = self
            .bundle
            .records
            .iter()
            .enumerate()
            .filter(|(_, record)| record.chart() == chart)
            .map(|(index, record)| {
                let distance = match &record.domain {
                    RecipeDomain::Keys(keys) => keys
                        .iter()
                        .map(|k| {
                            k.iter()
                                .zip(key_table)
                                .map(|(a, b)| (a - b).abs())
                                .sum::<i64>()
                        })
                        .min()
                        .unwrap_or(i64::MAX),
                    RecipeDomain::Predicate => i64::MAX,
                };
                (distance, index)
            })
            .collect();
        candidates.sort();
        let mut seen = std::collections::HashSet::new();
        for (_, index) in candidates {
            let record = &self.bundle.records[index];
            if seen.insert((record.recipe["seeds"].to_string(), record.symmetry_rows())) {
                families.push((record.id.clone(), self.at_key(record, key_table, max_rows)?));
            }
        }
        Ok(families)
    }

    /// `record`'s recipe re-centred on `key_table` (its seeds kept as
    /// centres relative to the key, domain = that one key), capped.
    fn at_key(
        &self,
        record: &RecipeRecord,
        key_table: &[i64],
        max_rows: usize,
    ) -> Result<RecipeRecord, PError> {
        let mut recipe = record.recipe.clone();
        recipe["id"] = json!(format!("{}@outside", record.id));
        recipe["domain"] = json!({"keys": [key_table]});
        let mut at_key = RecipeBundle::record_of(&recipe.to_string())?;
        at_key.max_trace_rows = Some(
            record
                .max_trace_rows
                .map_or(max_rows, |rows| rows.min(max_rows)),
        );
        Ok(at_key)
    }

    fn remember_outside(&self, key: &[i64], relation: &Arc<Relation>, id: &str) {
        self.outside
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key.to_vec(), (Arc::clone(relation), id.to_owned()));
    }

    fn remember(&self, index: usize, key: &[i64], relation: &Arc<Relation>) {
        self.memo
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert((index, key.to_vec()), Arc::clone(relation));
    }

    /// The verb's record checked and converted to presentation slots.
    fn verified(
        &self,
        record: &RecipeRecord,
        key: &[i64],
        key_table: &[i64],
        value: &Value,
    ) -> Result<Relation, PError> {
        let verdict = value["verdict"].as_str().unwrap_or("");
        if verdict != "SOLVED" {
            let failure = value["failure"].clone();
            let class = failure["class"].as_str().unwrap_or("error").to_owned();
            let message = failure["message"]
                .as_str()
                .map_or_else(|| format!("verdict {verdict}"), str::to_owned);
            return Err(self.failure_with(
                record,
                key,
                key_table,
                PRecipeFailureKind::from_class(&class),
                &class,
                &message,
                failure,
            ));
        }
        let fail = |kind, class: &str, message: String, detail: Value| {
            Err(self.failure(record, key, key_table, kind, class, &message, detail))
        };
        if value["proof"]["ok"] != Value::Bool(true) {
            return fail(
                PRecipeFailureKind::ProofMismatch,
                "proof_missing",
                "the relation carries no successful exact proof".to_owned(),
                json!({"proof": value["proof"]}),
            );
        }
        if value["all_lower"] != Value::Bool(true) {
            return fail(
                PRecipeFailureKind::ProofMismatch,
                "not_lower",
                "a right-hand-side key is not lower than the target".to_owned(),
                json!({"top_rhs": value["top_rhs"]}),
            );
        }
        let digest = value["relation_digest"].as_str().map(str::to_owned);
        let key_text: Vec<String> = key_table.iter().map(i64::to_string).collect();
        if let Some(certified) = record.relation_digests.get(&key_text.join(","))
            && digest.as_deref() != Some(certified.as_str())
        {
            return fail(
                PRecipeFailureKind::DigestMismatch,
                "digest_mismatch",
                "the relation solved at this key differs from the certified one (relation digest)"
                    .to_owned(),
                json!({"expected": certified, "got": digest}),
            );
        }
        let target_weight: i64 = key_table.iter().sum();
        let mut pole_order_at_4 = Some(0_u32);
        let rhs = value["rhs"].as_array().cloned().unwrap_or_default();
        let mut children = Vec::with_capacity(rhs.len());
        for term in &rhs {
            let (Some(text), Some(coefficient)) = (term[0].as_str(), term[1].as_str()) else {
                return fail(
                    PRecipeFailureKind::Error,
                    "unreadable_relation",
                    "a right-hand-side term is not [key, coefficient, mark]".to_owned(),
                    json!({"term": term}),
                );
            };
            let child: Option<Vec<i64>> = text
                .strip_prefix("I(")
                .and_then(|rest| rest.strip_suffix(')'))
                .and_then(|inner| {
                    inner
                        .split(',')
                        .map(|part| part.trim().parse().ok())
                        .collect()
                });
            let Some(child) = child.filter(|child| child.len() == key_table.len()) else {
                return fail(
                    PRecipeFailureKind::Error,
                    "unreadable_relation",
                    format!("the right-hand-side key {text:?} is not an integer key of the node"),
                    json!({"term": term}),
                );
            };
            let mut value = match super::basis::parse_coefficient(
                &self.context,
                coefficient,
                ExactAlgebraLimits::default(),
            ) {
                Ok(value) => value,
                Err(error) => {
                    return fail(
                        PRecipeFailureKind::Error,
                        "unreadable_relation",
                        format!("the coefficient of {text}: {error}"),
                        json!({"term": term[0]}),
                    );
                }
            };
            pole_order_at_4 = match (pole_order_at_4, pole_order_at(&value, 4)) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            };
            let weight: i64 = child.iter().sum();
            // The import's sign rule (q.q = +1 in the table, -1 here).
            if (target_weight - weight).rem_euclid(2) == 1 {
                value = -value;
            }
            children.push((self.bundle.to_presentation(&child), value));
        }
        Ok(Relation {
            children,
            relation_digest: digest,
            pole_order_at_4,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn failure(
        &self,
        record: &RecipeRecord,
        key: &[i64],
        key_table: &[i64],
        kind: PRecipeFailureKind,
        class: &str,
        message: &str,
        detail: Value,
    ) -> PError {
        let failure = json!({
            "schema": crate::node::recipe::FAILURE_SCHEMA, "class": class, "message": message,
            "detail": detail, "key": key_table,
            "recipe": {"id": record.id, "digest": record.recipe_digest},
            "determinism_id": crate::node::recipe::DETERMINISM_ID,
        });
        self.failure_with(record, key, key_table, kind, class, message, failure)
    }

    #[allow(clippy::too_many_arguments)]
    fn failure_with(
        &self,
        record: &RecipeRecord,
        key: &[i64],
        key_table: &[i64],
        kind: PRecipeFailureKind,
        class: &str,
        message: &str,
        mut failure: Value,
    ) -> PError {
        let (reproduce, reproduce_dir) = self.reproduction(record, key_table);
        // The verb's own line names its stand-alone binary with the in-process
        // (empty) family path: point it at the files written for this failure.
        if failure.is_object() {
            failure["reproduce"] = json!(reproduce);
            if let Some(directory) = &reproduce_dir {
                failure["recipe"]["path"] = json!(format!("{directory}/recipe.json"));
                failure["cwd"] = json!(directory);
            }
        }
        PError::RecipeFailed(Box::new(PRecipeFailure {
            node: self.bundle.node.clone(),
            recipe_id: record.id.clone(),
            recipe_digest: record.recipe_digest.clone(),
            recipe_kind: record.kind(),
            relation_rule: record.relation_rule().to_owned(),
            key: key.to_vec(),
            key_table: key_table.to_vec(),
            kind,
            class: class.to_owned(),
            message: message.to_owned(),
            failure,
            reproduce,
            reproduce_dir,
            determinism_id: crate::node::recipe::DETERMINISM_ID.to_owned(),
            request: None,
            rule_chain: None,
            rule_chain_note: None,
        }))
    }

    /// The one-line reproduction of a solve and the directory of its files
    /// (family text, recipe), written now; the line names the same binary.
    fn reproduction(&self, record: &RecipeRecord, key_table: &[i64]) -> (String, Option<String>) {
        let q = |text: &str| format!("'{}'", text.replace('\'', "'\\''"));
        let digest = record.recipe_digest.get(..12).unwrap_or("recipe");
        let directory = std::env::temp_dir()
            .join("rustred-recipe-repro")
            .join(format!("{}-{digest}", self.bundle.node));
        let written = std::fs::create_dir_all(&directory)
            .and_then(|()| std::fs::write(directory.join("family.sym"), &self.bundle.family_text))
            .and_then(|()| {
                std::fs::write(
                    directory.join("recipe.json"),
                    serde_json::to_string_pretty(&record.recipe).unwrap_or_default(),
                )
            })
            .is_ok();
        let exe = std::env::var("RUSTRED_P_RECIPE_REPRO_EXE").unwrap_or_else(|_| {
            std::env::current_exe()
                .ok()
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("rustred"))
                })
                .map_or_else(|| "rustred".to_owned(), |path| path.display().to_string())
        });
        let key: Vec<String> = key_table.iter().map(i64::to_string).collect();
        let key = key.join(",");
        let out = directory.join(format!("out-{}", key.replace([',', '-'], "_")));
        let mut line = format!(
            "{} p-recipes solve --family {} --order {} --recipe {} --key {key} --out {}",
            q(&exe),
            q(&directory.join("family.sym").display().to_string()),
            q(&self.bundle.order.to_string()),
            q(&directory.join("recipe.json").display().to_string()),
            q(&out.display().to_string()),
        );
        if let Some(rows) = record.max_trace_rows {
            line.push_str(&format!(" --max-trace-rows {rows}"));
        }
        let dir = written.then(|| directory.display().to_string());
        (line, dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_polynomials_and_predicates_evaluate_at_a_key() {
        let key = [2, 1, 3, -1, 0];
        assert_eq!(eval_index_poly("n0-n1", &key), Some(1));
        assert_eq!(eval_index_poly("2*n2^2 - n3 + 1", &key), Some(20));
        assert_eq!(eval_index_poly("-(n0+n3)", &key), Some(-1));
        assert_eq!(eval_index_poly("n9", &key), None);
        assert_eq!(eval_index_poly("n0 +", &key), None);
        let recipe = json!({"chart": "11100", "domain": {"fixed": [null, 1, null, null, null],
            "equations": ["n0-2"], "exceptions": [["n2-4"]], "bounds": {"min": [null, null, 2, null, null]}}});
        assert!(predicate_holds(&recipe, &[2, 1, 3, -1, 0]));
        assert!(!predicate_holds(&recipe, &[2, 2, 3, -1, 0]), "fixed slot");
        assert!(!predicate_holds(&recipe, &[3, 1, 3, -1, 0]), "equation");
        assert!(
            !predicate_holds(&recipe, &[2, 1, 4, -1, 0]),
            "exception branch"
        );
        assert!(!predicate_holds(&recipe, &[2, 1, 1, -1, 0]), "bound");
        assert!(!predicate_holds(&recipe, &[2, 1, 3, 1, 0]), "off the chart");
    }

    #[test]
    fn failure_kinds_follow_the_verb_classes() {
        for (class, kind) in [
            (
                "no_relation_in_seeds",
                PRecipeFailureKind::NoRelationInSeeds,
            ),
            ("not_in_domain", PRecipeFailureKind::NotInDomain),
            ("budget_work", PRecipeFailureKind::Budget),
            ("budget_wall", PRecipeFailureKind::Budget),
            ("proof_failed", PRecipeFailureKind::ProofMismatch),
            ("digest_mismatch", PRecipeFailureKind::DigestMismatch),
            ("irregular", PRecipeFailureKind::Irregular),
            ("error", PRecipeFailureKind::Error),
        ] {
            assert_eq!(PRecipeFailureKind::from_class(class), kind);
        }
    }
}

/// vk5l WP2 tests on the toy boxed kite (`testing::boxed_kite_table_d2`): a
/// counting fake solver for the dispatch, cache and failure paths, and the
/// real engine solve at the owner level.
#[cfg(test)]
mod kite_tests {
    use super::*;
    use crate::family::IntegralKey;
    use crate::propagator::PresentationId;
    use crate::propagator::custom::{replay_custom_payload, verify_custom_payload};
    use crate::propagator::library::{PLibrary, PReduction, PReductionRequest, PUncoveredPolicy};
    use crate::propagator::table_import::{
        ImportedOwner, TableImportOptions, canonical_node_graph, import_table,
    };
    use crate::propagator::testing::{
        BOXED_KITE_GRAPH, BOXED_KITE_OWNER, boxed_kite_library_with_recipes, boxed_kite_table_d2,
    };
    use crate::reduction::{
        OrderedSchedulerOptions, OrderedStepCache, OrderedWeightArithmetic, ReductionLimits,
    };

    /// Answers from a table (TABLE slots, table convention), counting calls.
    #[derive(Default)]
    struct Fake {
        calls: AtomicU64,
        answers: Mutex<HashMap<Vec<i64>, Value>>,
        /// Answers for one recipe id (checked first).
        by_recipe: Mutex<HashMap<(String, Vec<i64>), Value>>,
        /// Declared cost of every solve (seconds): a bounded solve with a
        /// smaller bound is stopped, as the engine solver's wait is.
        delay_s: Mutex<f64>,
    }

    impl PRecipeSolver for Fake {
        fn solve(
            &self,
            _: &RecipeBundle,
            record: &RecipeRecord,
            key: &[i64],
        ) -> Result<Value, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(answer) = self
                .by_recipe
                .lock()
                .unwrap()
                .get(&(record.id.clone(), key.to_vec()))
            {
                return Ok(answer.clone());
            }
            self.answers
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .ok_or_else(|| format!("no answer for {key:?}"))
        }

        fn solve_within(
            &self,
            bundle: &RecipeBundle,
            record: &RecipeRecord,
            key: &[i64],
            budget: Option<std::time::Duration>,
        ) -> Result<Value, String> {
            self.bounded(bundle, record, key, budget)
        }
    }

    impl Fake {
        fn bounded(
            &self,
            bundle: &RecipeBundle,
            record: &RecipeRecord,
            key: &[i64],
            budget: Option<std::time::Duration>,
        ) -> Result<Value, String> {
            let delay = *self.delay_s.lock().unwrap();
            if budget.is_some_and(|budget| budget.as_secs_f64() < delay) {
                self.calls.fetch_add(1, Ordering::SeqCst);
                return Err(format!(
                    "{BUDGET_WALL_PREFIX} the solve exceeded its wall bound"
                ));
            }
            self.solve(bundle, record, key)
        }

        fn calls(&self) -> u64 {
            self.calls.load(Ordering::SeqCst)
        }
    }

    fn dot(i: usize) -> [i64; 5] {
        let mut key = [1; 5];
        key[i] = 2;
        key
    }

    fn dots(a: usize, b: usize) -> [i64; 5] {
        let mut key = dot(a);
        key[b] = 2;
        key
    }

    /// The toy relation of a one-dot key in the verb's record form.
    fn toy(key: [i64; 5]) -> (Vec<i64>, Value) {
        (
            key.to_vec(),
            json!({"verdict": "SOLVED", "proof": {"ok": true, "kind": "exact-weights"}, "all_lower": true,
                   "relation_digest": "fake-digest", "rhs": [["I(1,1,1,1,1)", "(d-4)/(d-3)", "top"]]}),
        )
    }

    fn install(node: &str, answers: Vec<(Vec<i64>, Value)>) -> Arc<Fake> {
        let fake = Arc::new(Fake::default());
        fake.answers.lock().unwrap().extend(answers);
        override_recipe_solver(node, fake.clone());
        fake
    }

    fn record(id: &str, keys: &[[i64; 5]], shells: &[&str]) -> RecipeRecord {
        RecipeBundle::record_of(
            &json!({"schema": "rustred.recipe.v1", "id": id, "arity": 5, "chart": "11111",
                    "domain": {"keys": keys}, "seeds": {"centres": [], "generator": {"shells": shells}}})
            .to_string(),
        )
        .unwrap()
    }

    const SHELLS: [&str; 5] = ["S0", "S1", "Sm1", "S1m1", "S2"];

    fn import(
        node: &str,
        omit: &[[i64; 5]],
        records: Option<Vec<RecipeRecord>>,
    ) -> Result<ImportedOwner, PError> {
        let (_, graph) = canonical_node_graph(BOXED_KITE_GRAPH, 2, 3)?;
        import_table(
            &boxed_kite_table_d2(node, omit),
            &graph,
            &TableImportOptions {
                id: BOXED_KITE_OWNER,
                node: None,
                lp_count: 1,
                census_family: None,
                order_map: None,
                partial_box: false,
                mixed_box: false,
                certified_keys: None,
                recipes: records.map(|records| RecipeImport {
                    revision: "t1".to_owned(),
                    dispatch: RecipeDispatchOrder::RulesFirst,
                    records,
                }),
            },
        )
    }

    fn library(node: &str, omit: &[[i64; 5]], records: Option<Vec<RecipeRecord>>) -> Arc<PLibrary> {
        boxed_kite_library_with_recipes(
            &boxed_kite_table_d2(node, omit),
            None,
            records.map(|records| RecipeImport {
                revision: "t1".to_owned(),
                dispatch: RecipeDispatchOrder::RulesFirst,
                records,
            }),
        )
        .unwrap()
    }

    /// batch 12a: an UNBOXED table (the kite without its box record) takes a recipe at a table position: it
    /// serves its domain in first match before the table record `position` (the solve is the verified run-time
    /// solve), a rule earlier in the table wins, and the `D + r` bound limits its domain.
    #[test]
    fn a_positioned_recipe_of_an_unboxed_table_is_dispatched_in_table_order() {
        let node = "KITE-positioned";
        let target = dots(0, 1); // the table's rule 5
        let fake = install(node, vec![toy(target)]);
        let unboxed = |table: String| -> String {
            let mut lines: Vec<String> = table.lines().map(str::to_owned).collect();
            let mut header: Value = serde_json::from_str(&lines[0]).unwrap();
            header.as_object_mut().unwrap().remove("boxed_node");
            // an unboxed bundle's node is the table's family
            header["family"] = json!(node);
            lines[0] = header.to_string();
            lines.join("\n") + "\n"
        };
        let recipe = |position: usize, bound_dr: Option<i64>| {
            let mut record = RecipeBundle::record_of(
                &json!({"schema": "rustred.recipe.v1", "id": "kite/face01", "arity": 5, "chart": "11111",
                        "domain": {"fixed": [2, 2, 1, 1, 1]},
                        "seeds": {"centres": [], "generator": {"shells": SHELLS}}})
                .to_string(),
            )
            .unwrap();
            record.position = Some(RecipeTablePosition {
                index: position,
                rules_before: 0,
                bound_dr,
            });
            record
        };
        let replay = |position: usize, bound_dr: Option<i64>| {
            let (_, graph) = canonical_node_graph(BOXED_KITE_GRAPH, 2, 3).unwrap();
            let imported = import_table(
                &unboxed(boxed_kite_table_d2(node, &[])),
                &graph,
                &TableImportOptions {
                    id: BOXED_KITE_OWNER,
                    node: None,
                    lp_count: 1,
                    census_family: None,
                    order_map: None,
                    partial_box: false,
                    mixed_box: false,
                    certified_keys: None,
                    recipes: Some(RecipeImport {
                        revision: "t1".to_owned(),
                        dispatch: RecipeDispatchOrder::RulesFirst,
                        records: vec![recipe(position, bound_dr)],
                    }),
                },
            )
            .unwrap();
            let replays = crate::propagator::custom::replay_custom_payload(
                &imported.presentation,
                &imported.payload,
                &imported.meta,
                &[presented(target)],
            )
            .unwrap();
            let residuals: Vec<String> = replays[0]
                .residuals
                .iter()
                .map(|(key, value)| format!("{key:?}={value}"))
                .collect();
            residuals
        };
        // before rule 5: the recipe's relation (I = (d-4)/(d-3) I(1^5)), one solve
        let calls = fake.calls();
        let by_recipe = replay(5, Some(39));
        assert_eq!(fake.calls(), calls + 1);
        // after rule 5 (position 6) or outside its D + r bound (D + r = 2 > 1): the table's rule 5, no solve
        let by_rule = replay(6, None);
        assert_eq!(replay(5, Some(1)), by_rule);
        assert_eq!(fake.calls(), calls + 1);
        assert_ne!(by_recipe, by_rule);
        assert_eq!(by_recipe.len(), 1, "{by_recipe:?}");
    }

    /// The presentation key of a table key of the kite.
    fn presented(key: [i64; 5]) -> Vec<i64> {
        let slot_map = import("KITE-map", &[], None).unwrap().slot_map;
        let mut out = vec![0; 5];
        for (slot, &power) in key.iter().enumerate() {
            out[slot_map[slot]] = power;
        }
        out
    }

    fn reduce(
        library: &PLibrary,
        key: [i64; 5],
        cache: Option<Arc<OrderedStepCache>>,
        uncovered: PUncoveredPolicy,
    ) -> Result<PReduction, PError> {
        library.reduce(PReductionRequest {
            inputs: vec![(
                BOXED_KITE_OWNER,
                IntegralKey::try_new(presented(key)).unwrap(),
                library.coefficient_context().one(),
            )],
            arithmetic: OrderedWeightArithmetic::Exact,
            scheduler: OrderedSchedulerOptions {
                threads: 1,
                ..Default::default()
            },
            step_cache: cache,
            uncovered,
            onto_masters: false,
            limits: ReductionLimits::default(),
            inspector: None,
        })
    }

    /// (a, b): recipe records replace two point rules; every reduction equals
    /// the point-rule table's, each recipe key is solved once (then the
    /// in-process memo), and the library's existing step cache records the
    /// recipe step (a new library replays it with no solve).
    #[test]
    fn recipe_records_reduce_like_the_point_rules_they_replace() {
        let node = "KITE-same";
        let fake = install(node, vec![toy(dot(0)), toy(dot(1))]);
        let plain = library(node, &[], None);
        let records = vec![
            record("kite/d0", &[dot(0)], &SHELLS),
            record("kite/d1", &[dot(1)], &SHELLS),
        ];
        let with = library(node, &[dot(0), dot(1)], Some(records.clone()));
        for key in [dot(0), dot(1), dots(0, 1), dots(1, 2), dots(0, 4), dot(3)] {
            let a = reduce(&plain, key, None, PUncoveredPolicy::Reject).unwrap();
            let b = reduce(&with, key, None, PUncoveredPolicy::Reject).unwrap();
            assert_eq!(a.terms, b.terms, "{key:?}");
            assert_eq!(a.terms.len(), 1, "{key:?}: {:?}", a.terms);
        }
        assert_eq!(fake.calls(), 2, "one solve per recipe key, then the memo");
        // The existing Ordered step cache records the recipe step.
        let cache =
            OrderedStepCache::in_memory(with.step_cache_binding().unwrap(), Default::default());
        let fresh = library(node, &[dot(0), dot(1)], Some(records.clone()));
        reduce(
            &fresh,
            dots(0, 1),
            Some(cache.clone()),
            PUncoveredPolicy::Reject,
        )
        .unwrap();
        assert_eq!(fake.calls(), 3);
        let again = library(node, &[dot(0), dot(1)], Some(records));
        let replayed = reduce(&again, dots(0, 1), Some(cache), PUncoveredPolicy::Reject).unwrap();
        assert_eq!(fake.calls(), 3, "the step cache replays the recipe step");
        assert_eq!(
            replayed.terms,
            reduce(&plain, dots(0, 1), None, PUncoveredPolicy::Reject)
                .unwrap()
                .terms
        );
    }

    /// (b, d): the relation cache (relation entries, append-only segments):
    /// cold = one solve and one segment line; warm (a new library) = no solve;
    /// a pre-filled cache as a read-only overlay = no solve and nothing
    /// written to the user cache.
    #[test]
    fn relation_cache_cold_warm_and_prefilled_overlay() {
        let node = "KITE-cache";
        let fake = install(node, vec![toy(dot(0))]);
        let base = std::env::temp_dir().join(format!("rr-recipe-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let user = base.join("user");
        set_recipe_cache_roots_for(
            node,
            RecipeCacheRoots {
                user: Some(user.clone()),
                prefilled: vec![],
                ..Default::default()
            },
        );
        let records = vec![record("kite/d0", &[dot(0)], &SHELLS)];
        let cold = library(node, &[dot(0)], Some(records.clone()));
        let expected = reduce(&cold, dots(0, 2), None, PUncoveredPolicy::Reject).unwrap();
        assert_eq!(fake.calls(), 1);
        // relation entries: the root's stamp and one segment of this process
        assert!(user.join("RELATION_CACHE.json").exists());
        let segments: Vec<_> = std::fs::read_dir(&user)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .flat_map(|directory| {
                std::fs::read_dir(directory.path())
                    .unwrap()
                    .filter_map(Result::ok)
            })
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("seg-"))
            .collect();
        assert_eq!(segments.len(), 1, "{segments:?}");
        let warm = library(node, &[dot(0)], Some(records.clone()));
        assert_eq!(
            reduce(&warm, dots(0, 2), None, PUncoveredPolicy::Reject)
                .unwrap()
                .terms,
            expected.terms
        );
        assert_eq!(fake.calls(), 1, "warm: the user cache answers");
        let user2 = base.join("user2");
        set_recipe_cache_roots_for(
            node,
            RecipeCacheRoots {
                user: Some(user2.clone()),
                prefilled: vec![user.clone()],
                ..Default::default()
            },
        );
        let overlay = library(node, &[dot(0)], Some(records));
        assert_eq!(
            reduce(&overlay, dots(0, 2), None, PUncoveredPolicy::Reject)
                .unwrap()
                .terms,
            expected.terms
        );
        assert_eq!(fake.calls(), 1, "the pre-filled overlay answers");
        assert!(
            !user2.exists() || std::fs::read_dir(&user2).unwrap().next().is_none(),
            "nothing copied"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// (c): a recipe that fails is a typed error naming node, recipe, the
    /// key being solved, its kind, the ORIGINAL request and one rule chain,
    /// with a reproduction line whose files exist; Retain does not hide it.
    #[test]
    fn a_failing_recipe_names_node_recipe_key_request_chain_and_reproduction() {
        let node = "KITE-fail";
        let failure = json!({"verdict": "NO_RELATION_IN_SEEDS", "rhs": null, "relation_digest": null,
            "failure": {"schema": "rustred.recipe-failure.v1", "class": "no_relation_in_seeds",
                        "message": "the 1 seeds (5 rows) of recipe kite/d0 never reach I(2,1,1,1,1) as a pivot",
                        "detail": {"seeds": 1, "rows": 5}, "context": {"request_key": null, "rule_chain": []}}});
        install(node, vec![(dot(0).to_vec(), failure)]);
        let lib = library(
            node,
            &[dot(0)],
            Some(vec![record("kite/d0", &[dot(0)], &["S0"])]),
        );
        lib.set_source("/packages/kite");
        for policy in [PUncoveredPolicy::Reject, PUncoveredPolicy::Retain] {
            let error = reduce(&lib, dots(0, 3), None, policy).unwrap_err();
            let PError::RecipeFailed(failed) = &error else {
                panic!("expected RecipeFailed, got {error:?}");
            };
            assert_eq!(failed.kind, PRecipeFailureKind::NoRelationInSeeds);
            assert_eq!(
                (failed.node.as_str(), failed.recipe_id.as_str()),
                (node, "kite/d0")
            );
            assert_eq!(failed.key, presented(dot(0)));
            assert_eq!(failed.key_table, dot(0));
            let request = failed.request.as_ref().expect("the request");
            assert_eq!(request.inputs[0].powers, presented(dots(0, 3)));
            let chain = failed.rule_chain.as_ref().expect("a rule chain");
            assert_eq!(chain.len(), 2, "{chain:?}");
            assert_eq!(
                (chain[0].key.clone(), chain[0].via.as_str()),
                (presented(dots(0, 3)), "request input")
            );
            assert_eq!(chain[1].key, presented(dot(0)));
            assert!(chain[1].via.starts_with("table rule"), "{chain:?}");
            let text = error.to_string();
            for part in [
                "recipe NO_RELATION_IN_SEEDS failed for boxed node KITE-fail",
                "recipe \"kite/d0\"",
                "could not solve the key",
                "never reach I(2,1,1,1,1) as a pivot",
                "original request: KITE[p2] I(",
                "rule chain: I(",
                "no fallback was tried",
                "p-recipes solve --family",
                "--key 2,1,1,1,1",
                "reproduce the request: cargo run",
            ] {
                assert!(text.contains(part), "{part:?} not in {text}");
            }
            let dir = PathBuf::from(failed.reproduce_dir.as_ref().expect("files"));
            assert!(dir.join("family.sym").exists() && dir.join("recipe.json").exists());
            let json = failed.to_json();
            assert_eq!(json["failure"]["class"], "no_relation_in_seeds");
            assert_eq!(
                json["failure"]["context"]["rule_chain"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            assert_eq!(json["kind"], "NO_RELATION_IN_SEEDS");
        }
    }

    /// (c): a relation whose digest differs from the certified one, and one
    /// without its proof, are refused with their kinds; a key no rule and no
    /// record takes stays the typed refusal it was.
    #[test]
    fn certified_digests_and_proofs_are_checked_and_unrecorded_keys_take_their_rules() {
        let node = "KITE-digest";
        let mut unproved = toy(dot(1));
        unproved.1["proof"]["ok"] = json!(false);
        let fake = install(node, vec![toy(dot(0)), unproved]);
        let mut certified = record("kite/d0", &[dot(0)], &SHELLS);
        certified
            .relation_digests
            .insert("2,1,1,1,1".to_owned(), "certified-digest".to_owned());
        let lib = library(
            node,
            &[dot(0), dot(1)],
            Some(vec![certified, record("kite/d1", &[dot(1)], &SHELLS)]),
        );
        match reduce(&lib, dot(0), None, PUncoveredPolicy::Reject) {
            Err(PError::RecipeFailed(failed)) => {
                assert_eq!(failed.kind, PRecipeFailureKind::DigestMismatch);
                assert_eq!(failed.failure["detail"]["expected"], "certified-digest");
                assert_eq!(failed.failure["detail"]["got"], "fake-digest");
            }
            other => panic!("expected a digest mismatch, got {other:?}"),
        }
        match reduce(&lib, dot(1), None, PUncoveredPolicy::Reject) {
            Err(PError::RecipeFailed(failed)) => {
                assert_eq!(failed.kind, PRecipeFailureKind::ProofMismatch)
            }
            other => panic!("expected a proof refusal, got {other:?}"),
        }
        // A key of no record takes its rule (no solve); a box key with neither
        // rule nor record is refused by the import (box not closed).
        assert_eq!(
            reduce(&lib, dot(2), None, PUncoveredPolicy::Reject)
                .unwrap()
                .terms
                .len(),
            1
        );
        assert_eq!(fake.calls(), 2);
        let error = import(
            "KITE-digest",
            &[dot(0), dot(2)],
            Some(vec![record("kite/d0", &[dot(0)], &SHELLS)]),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("the box is not closed"),
            "{error}"
        );
    }

    /// (a): the importer refuses a recipe key that a point rule or a terminal
    /// already has, and a recipe bundle round-trips through the payload.
    #[test]
    fn the_import_checks_recipe_keys_and_the_payload_carries_the_bundle() {
        let error = import(
            "KITE-imp",
            &[],
            Some(vec![record("kite/d0", &[dot(0)], &SHELLS)]),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("also a terminal or a point-rule target"),
            "{error}"
        );
        let error = import(
            "KITE-imp",
            &[],
            Some(vec![record("kite/c", &[[1; 5]], &SHELLS)]),
        )
        .unwrap_err();
        assert!(error.to_string().contains("also a terminal"), "{error}");
        let error = import(
            "KITE-imp",
            &[dot(0)],
            Some(vec![record("kite/far", &[[3, 1, 1, 1, 1]], &SHELLS)]),
        )
        .unwrap_err();
        assert!(error.to_string().contains("outside the box"), "{error}");
        let imported = import(
            "KITE-imp",
            &[dot(0)],
            Some(vec![record("kite/d0", &[dot(0)], &SHELLS)]),
        )
        .unwrap();
        let bundle = recipes_of_source(
            &crate::propagator::custom::custom_payload_family_source(&imported.payload).unwrap(),
        )
        .unwrap()
        .expect("a bundle");
        assert_eq!(Some(&bundle), imported.recipes.as_ref());
        assert_eq!(
            (bundle.node.as_str(), bundle.records.len(), bundle.version),
            ("KITE-imp", 1, P_RECIPE_VERSION)
        );
        assert_eq!(bundle.table_to_presentation, imported.slot_map);
        assert_eq!(bundle.to_table(&bundle.to_presentation(&dot(0))), dot(0));
        // Without the dot rule and without a record the box is not closed.
        assert!(import("KITE-imp", &[dot(0)], None).is_err());
    }

    /// The certified set of the outside tests: the corner, the one-dot and
    /// the two-dot keys (every rule target, TABLE frame).
    fn certified(node: &str) -> super::super::certified_keys::CertifiedKeySet {
        let mut keys = vec![vec![1_i64; 5]];
        keys.extend((0..5).map(|i| dot(i).to_vec()));
        keys.extend((0..5).flat_map(|a| (a + 1..5).map(move |b| dots(a, b).to_vec())));
        super::super::certified_keys::CertifiedKeySet::new(node, 5, "table", keys).unwrap()
    }

    fn certified_library(node: &str, records: Vec<RecipeRecord>) -> Arc<PLibrary> {
        boxed_kite_library_with_recipes(
            &boxed_kite_table_d2(node, &[dot(0)]),
            Some(certified(node)),
            Some(RecipeImport {
                revision: "t1".to_owned(),
                dispatch: RecipeDispatchOrder::RulesFirst,
                records,
            }),
        )
        .unwrap()
    }

    fn outside_of(reduction: &PReduction) -> Vec<Vec<i64>> {
        reduction
            .outside_certified
            .iter()
            .map(|key| key.powers.clone())
            .collect()
    }

    /// orbitsw (T266-STUDY 7.1): with orbit canonicalisation on, a boxed
    /// owner's key moves only onto an image in its certified set. For every
    /// two-dot key K and every OTHER two-dot key X removed from the certified
    /// set, K reduces (no outside-certified report, no refusal) to the values
    /// of the run without canonicalisation: K is never moved onto X. Without
    /// the filter, a K whose least image is X would be moved outside the set.
    #[test]
    fn a_boxed_key_moves_only_onto_a_certified_image() {
        let node = "KITE-orbit-cert";
        let two: Vec<[i64; 5]> = (0..5)
            .flat_map(|a| (a + 1..5).map(move |b| dots(a, b)))
            .collect();
        let certified_without = |x: &[i64; 5]| {
            let mut keys = vec![vec![1_i64; 5]];
            keys.extend((0..5).map(|i| dot(i).to_vec()));
            keys.extend(two.iter().filter(|k| *k != x).map(|k| k.to_vec()));
            super::super::certified_keys::CertifiedKeySet::new(node, 5, "table", keys).unwrap()
        };
        let full = boxed_kite_library_with_recipes(
            &boxed_kite_table_d2(node, &[]),
            Some(certified(node)),
            None,
        )
        .unwrap();
        full.set_orbit_canonicalization(false);
        let reference: Vec<_> = two
            .iter()
            .map(|k| {
                reduce(&full, *k, None, PUncoveredPolicy::Reject)
                    .unwrap()
                    .terms
            })
            .collect();
        let mut checked = 0;
        for x in &two {
            // X leaves the certified set and the table (an import refuses a
            // rule whose target is not certified)
            let library = boxed_kite_library_with_recipes(
                &boxed_kite_table_d2(node, &[*x]),
                Some(certified_without(x)),
                None,
            )
            .unwrap();
            library.set_orbit_canonicalization(true);
            for (k, expected) in two.iter().zip(&reference) {
                if k == x {
                    continue;
                }
                let reduced = reduce(&library, *k, None, PUncoveredPolicy::Reject)
                    .unwrap_or_else(|e| panic!("key {k:?} with {x:?} uncertified: {e}"));
                assert!(
                    reduced.outside_certified.is_empty(),
                    "key {k:?} moved outside the certified set ({x:?} removed)"
                );
                assert_eq!(&reduced.terms, expected, "key {k:?}, {x:?} removed");
                checked += 1;
            }
        }
        assert_eq!(checked, 10 * 9);
    }

    /// User ruling 2026-11-14: a key OUTSIDE the certified set is solved at
    /// run time with the node's recipe families at the key (a three-dot key
    /// no rule takes; certified keys report nothing), and the result names
    /// every outside key; the step cache never records an outside step (a
    /// warm request steps and reports it again); no family solving is
    /// the typed OUTSIDE_NO_RELATION with its attempts; the Refuse policy
    /// keeps the WP1 hard error.
    #[test]
    fn keys_outside_the_certified_set_are_solved_at_run_time_and_reported() {
        let node = "KITE-out";
        let three = [2, 2, 2, 1, 1];
        let (_, solved) = toy(three);
        let fake = install(node, vec![toy(dot(0)), (three.to_vec(), solved)]);
        let records = vec![record("kite/d0", &[dot(0)], &SHELLS)];
        let library = certified_library(node, records.clone());
        // inside: no outside key
        let inside = reduce(&library, dot(0), None, PUncoveredPolicy::Reject).unwrap();
        assert!(inside.outside_certified.is_empty());
        let c0 = fake.calls();
        // a two-dot key: certified, a table rule
        let two = reduce(&library, dots(1, 2), None, PUncoveredPolicy::Reject).unwrap();
        assert!(two.outside_certified.is_empty());
        assert_eq!(fake.calls(), c0, "a rule, no solve");
        // a three-dot key: no rule; the family kite/d0 solved at the key
        let before = recipe_statistics().outside_solves;
        let three_out = reduce(&library, three, None, PUncoveredPolicy::Reject).unwrap();
        assert_eq!(outside_of(&three_out), vec![presented(three)]);
        assert_eq!(three_out.outside_certified[0].node, node);
        assert_eq!(three_out.terms.len(), 1, "{:?}", three_out.terms);
        assert_eq!(fake.calls(), c0 + 1);
        assert!(recipe_statistics().outside_solves > before);
        reduce(&library, three, None, PUncoveredPolicy::Reject).unwrap();
        assert_eq!(
            fake.calls(),
            c0 + 1,
            "the memo serves the outside key again"
        );
        // the step cache: an outside step is never recorded, so a warm request
        // (a new library, empty memo) steps the key again and reports it
        let cache =
            OrderedStepCache::in_memory(library.step_cache_binding().unwrap(), Default::default());
        for round in 0..2_u64 {
            let fresh = certified_library(node, records.clone());
            let calls = fake.calls();
            let warm =
                reduce(&fresh, three, Some(cache.clone()), PUncoveredPolicy::Reject).unwrap();
            assert_eq!(outside_of(&warm), vec![presented(three)], "round {round}");
            assert_eq!(warm.terms, three_out.terms);
            assert_eq!(
                fake.calls(),
                calls + 1,
                "round {round}: stepped, not replayed"
            );
        }
        // no family gives a relation: typed, with every attempt
        let four = [3, 2, 1, 1, 1];
        fake.answers.lock().unwrap().insert(
            four.to_vec(),
            json!({"verdict": "NO_RELATION_IN_SEEDS", "failure": {"schema": "rustred.recipe-failure.v1",
                   "class": "no_relation_in_seeds", "message": "the seeds never reach the key"}}),
        );
        let error = reduce(&library, four, None, PUncoveredPolicy::Reject).unwrap_err();
        let PError::RecipeFailed(failed) = &error else {
            panic!("expected RecipeFailed, got {error:?}");
        };
        assert_eq!(failed.kind, PRecipeFailureKind::OutsideNoRelation);
        assert_eq!(failed.class, "outside_no_relation");
        assert_eq!(failed.key, presented(four));
        assert_eq!(
            failed.failure["outside"]["outside_certified_set"],
            json!(true)
        );
        let attempts = failed.failure["outside"]["attempts"].as_array().unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0]["kind"], "NO_RELATION_IN_SEEDS");
        assert!(
            failed.reproduce.contains("p-recipes solve"),
            "{}",
            failed.reproduce
        );
        assert!(
            failed.reproduce.contains("--key 3,2,1,1,1"),
            "{}",
            failed.reproduce
        );
        assert!(
            failed.reproduce.contains("--max-trace-rows"),
            "{}",
            failed.reproduce
        );
        let text = error.to_string();
        assert!(
            text.contains("OUTSIDE_NO_RELATION") && text.contains("outside the certified set"),
            "{text}"
        );
        assert!(text.contains("original request: KITE"), "{text}");
        // the Refuse policy: the WP1 hard error
        let refused = "KITE-out-refuse";
        install(refused, vec![toy(dot(0))]);
        set_outside_policy_for(refused, POutsidePolicy::Refuse);
        let library = certified_library(refused, vec![record("kite/d0", &[dot(0)], &SHELLS)]);
        // (a direct input: the early box check of decision Q7 answers first)
        match reduce(&library, three, None, PUncoveredPolicy::Reject) {
            Err(PError::OutsideBox { node, .. } | PError::OutsideCertifiedSet { node, .. }) => {
                assert_eq!(node, refused)
            }
            other => panic!("expected the WP1 refusal, got {other:?}"),
        }
    }

    /// User requirement (6): an outside key solved once is written to the
    /// persistent relation cache (entry `rustred.relation-entry.v1`, table
    /// frame, outside flag) and a restart (a new library over the same root)
    /// takes it from there without a solve, still reporting the key.
    #[test]
    fn an_outside_key_solved_once_is_reused_after_a_restart() {
        let node = "KITE-out-persist";
        let three = [2, 2, 2, 1, 1];
        let (_, solved) = toy(three);
        let fake = install(node, vec![toy(dot(0)), (three.to_vec(), solved)]);
        let root = std::env::temp_dir().join(format!("rr-outside-persist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        set_recipe_cache_roots_for(
            node,
            RecipeCacheRoots {
                user: Some(root.clone()),
                prefilled: vec![],
                ..Default::default()
            },
        );
        let records = vec![record("kite/d0", &[dot(0)], &SHELLS)];
        let first = certified_library(node, records.clone());
        let before = super::super::relation_entries::relation_cache_statistics();
        let cold = reduce(&first, three, None, PUncoveredPolicy::Reject).unwrap();
        let calls = fake.calls();
        assert_eq!(outside_of(&cold), vec![presented(three)]);
        // the entry on disk
        let line = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .flat_map(|directory| {
                std::fs::read_dir(directory.path())
                    .unwrap()
                    .filter_map(Result::ok)
            })
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("seg-"))
            .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
            .collect::<String>();
        let entries: Vec<Value> = line
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let entry = entries
            .iter()
            .find(|entry| entry["key"] == json!(three))
            .expect("the outside entry");
        assert_eq!(entry["schema"], "rustred.relation-entry.v1");
        assert_eq!(entry["key_frame"], "table");
        assert_eq!(entry["diagnostics"]["outside_certified_set"], json!(true));
        assert_eq!(entry["diagnostics"]["pole_order_at"]["4"], json!(0));
        assert_eq!(entry["diagnostics"]["regular_alternative"], "yes");
        assert_eq!(entry["recipe"]["id"], "kite/d0");
        assert_eq!(entry["evidence"]["level"], "per-key");
        // restart: a new library, empty memo, same root: no solve
        let second = certified_library(node, records);
        let warm = reduce(&second, three, None, PUncoveredPolicy::Reject).unwrap();
        assert_eq!(
            fake.calls(),
            calls,
            "the persistent cache answers after the restart"
        );
        assert_eq!(warm.terms, cold.terms);
        assert_eq!(outside_of(&warm), vec![presented(three)]);
        let stats = super::super::relation_entries::relation_cache_statistics().since(&before);
        assert!(stats.inserts >= 1 && stats.cache_hits >= 1, "{stats:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The d = 4 preference (opt-in since rule 21): when asked for and the
    /// first family's relation has a (d-4) pole while a later family's is
    /// regular, the regular one is taken; without it the first solved
    /// relation is taken. The numerator-centred depth-2 family is built (and
    /// tried first) for a key with a negative numerator slot.
    #[test]
    fn outside_solves_prefer_a_relation_regular_at_d_4_and_try_the_numerator_family() {
        let node = "KITE-out-d4";
        let three = [2, 2, 2, 1, 1];
        let fake = install(node, vec![toy(dot(0)), toy(dot(1))]);
        let relation = |coefficient: &str| {
            json!({"verdict": "SOLVED", "proof": {"ok": true, "kind": "exact-weights"}, "all_lower": true,
                   "relation_digest": coefficient, "rhs": [["I(1,1,1,1,1)", coefficient, "top"]]})
        };
        fake.by_recipe.lock().unwrap().extend([
            (
                ("kite/d0@outside".to_owned(), three.to_vec()),
                relation("1/(d-4)^2"),
            ),
            (
                ("kite/d1@outside".to_owned(), three.to_vec()),
                relation("1/(d-3)"),
            ),
        ]);
        let records = vec![
            record("kite/d0", &[dot(0)], &SHELLS),
            record("kite/d1", &[dot(1)], &["S0", "S1"]),
        ];
        let imported = import(node, &[dot(0), dot(1)], Some(records)).unwrap();
        let solve = || {
            crate::propagator::custom::custom_payload_solve_outside(
                &imported.presentation,
                &imported.payload,
                &imported.meta,
                &presented(three),
            )
            .unwrap()
            .expect("a relation")
        };
        // default (rule 21): the first solved relation, with its (d-4) pole
        let (first, id) = solve();
        assert_eq!(id, "kite/d0", "no d = 4 preference by default");
        assert_eq!(first.pole_order_at_4, Some(2));
        set_outside_settings_for(
            node,
            POutsideSettings {
                prefer_regular_at_4: true,
                ..POutsideSettings::default()
            },
        );
        let (solved, id) = solve();
        assert_eq!(id, "kite/d1", "asked for: the regular relation wins");
        assert_eq!(solved.pole_order_at_4, Some(0));
        // the numerator family: a key with a negative slot (another chart: no record family)
        let numerator = [2, 1, 1, 1, -1];
        let family = format!("{node}/numerator-centred-d2");
        fake.by_recipe
            .lock()
            .unwrap()
            .insert((family.clone(), numerator.to_vec()), relation("1"));
        let (_, id) = crate::propagator::custom::custom_payload_solve_outside(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
            &presented(numerator),
        )
        .unwrap()
        .expect("a relation");
        assert_eq!(id, family);
    }

    /// User ruling 2026-11-15: by default a regular relation ALWAYS wins,
    /// whatever its size (max_size_ratio unbounded); with max_size_ratio 2 a
    /// regular relation of 3 terms loses against a 1-term pole relation (the
    /// pole one is taken, regular_alternative "too_large").
    #[test]
    fn a_regular_relation_wins_whatever_its_size_unless_a_size_ratio_is_set() {
        let three = [2, 2, 2, 1, 1];
        let relation = |rhs: Value| {
            json!({"verdict": "SOLVED", "proof": {"ok": true, "kind": "exact-weights"}, "all_lower": true,
                   "relation_digest": rhs.to_string(), "rhs": rhs})
        };
        let pole = relation(json!([["I(1,1,1,1,1)", "1/(d-4)", "top"]]));
        let big = relation(json!([
            ["I(1,1,1,1,1)", "1/(d-3)", "top"],
            ["I(2,1,1,1,1)", "1", "top"],
            ["I(1,2,1,1,1)", "1", "top"]
        ]));
        for (node, ratio, winner) in [
            ("KITE-ratio-unbounded", None, "kite/d1"),
            ("KITE-ratio-2", Some((2, 1)), "kite/d0"),
            ("KITE-ratio-3", Some((3, 1)), "kite/d1"),
        ] {
            let fake = install(node, vec![toy(dot(0)), toy(dot(1))]);
            fake.by_recipe.lock().unwrap().extend([
                (("kite/d0@outside".to_owned(), three.to_vec()), pole.clone()),
                (("kite/d1@outside".to_owned(), three.to_vec()), big.clone()),
            ]);
            set_outside_settings_for(
                node,
                POutsideSettings {
                    max_size_ratio: ratio,
                    // the size ratio applies to the opt-in d = 4 preference
                    // (rule 21: not the default)
                    prefer_regular_at_4: true,
                    ..POutsideSettings::default()
                },
            );
            let records = vec![
                record("kite/d0", &[dot(0)], &SHELLS),
                record("kite/d1", &[dot(1)], &["S0", "S1"]),
            ];
            let imported = import(node, &[dot(0), dot(1)], Some(records)).unwrap();
            let (solved, id) = crate::propagator::custom::custom_payload_solve_outside(
                &imported.presentation,
                &imported.payload,
                &imported.meta,
                &presented(three),
            )
            .unwrap()
            .expect("a relation");
            assert_eq!(id, winner, "{node}");
            assert_eq!(
                solved.pole_order_at_4,
                Some(u32::from(winner == "kite/d0")),
                "{node}"
            );
        }
        assert_eq!(parse_size_ratio("unbounded"), Ok(None));
        assert_eq!(parse_size_ratio("3/2"), Ok(Some((3, 2))));
        assert_eq!(parse_size_ratio("2"), Ok(Some((2, 1))));
        assert!(parse_size_ratio("1/2").is_err() && parse_size_ratio("x").is_err());
    }

    /// Coordinator 2026-11-15 (T266): a boxed node WITHOUT recipe records is
    /// served by its relation entries: the import embeds the relation binding
    /// (an empty bundle), a key the rules do not take comes from the shipped
    /// root (an imported symmetry relation, determinism line
    /// `rustred.relation-import.v1/`), is reported outside the certified set
    /// and needs no solver; a key with neither entry nor recipe is the typed
    /// OutsideCertifiedSet.
    #[test]
    fn a_boxed_node_without_recipes_is_served_by_its_relation_entries() {
        let node = "KITE-entries";
        let fake = install(node, vec![]);
        let three = [2, 2, 2, 1, 1];
        let imported = import(node, &[], None).unwrap();
        let bundle = recipes_of_source(
            &crate::propagator::custom::custom_payload_family_source(&imported.payload).unwrap(),
        )
        .unwrap()
        .expect("the relation binding of a boxed import");
        assert!(bundle.records.is_empty());
        assert_eq!(
            bundle.revision,
            crate::propagator::table_import::RELATION_BINDING_REVISION
        );
        let root = std::env::temp_dir().join(format!("rr-entries-only-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let identity = bundle_node_identity(&bundle);
        let body = json!({"type": "relation", "key": three, "verdict": "SOLVED", "all_lower": true,
            "proof": {"ok": true, "kind": "symverify"}, "relation_digest": null,
            "determinism_id": "rustred.relation-import.v1/kind=symmetry_at_key/statement=test",
            "rhs": [["I(1,1,1,1,1)", "(d-4)/(d-3)", "top"]]});
        let meta = super::super::relation_entries::EntryMeta {
            node: node.to_owned(),
            node_identity: identity.clone(),
            package_revision: None,
            recipe_id: format!("{node}/symmetry_at_key"),
            recipe: json!({}),
            recipe_digest: String::new(),
            relation_rule: "symmetry".to_owned(),
            evidence_level: "per-key",
            pole_order_at_4: Some(0),
            regular_alternative: "not_tried",
            outside_certified_set: true,
            written_by: "test".to_owned(),
        };
        let entry = super::super::relation_entries::build_entry(&meta, &three, body);
        super::super::relation_entries::RelationStore::new(node, &identity, &[], Some(&root))
            .put(&entry)
            .unwrap();
        set_recipe_cache_roots_for(
            node,
            RecipeCacheRoots {
                user: None,
                prefilled: vec![root.clone()],
                ..Default::default()
            },
        );
        let served = certified_library_without_recipes(node);
        let before = super::super::relation_entries::relation_cache_statistics();
        let reduced = reduce(&served, three, None, PUncoveredPolicy::Reject).unwrap();
        let stats = super::super::relation_entries::relation_cache_statistics().since(&before);
        assert_eq!(outside_of(&reduced), vec![presented(three)]);
        assert_eq!(reduced.terms.len(), 1, "{:?}", reduced.terms);
        assert!(stats.shipped_hits >= 1, "{stats:?}");
        assert_eq!(fake.calls(), 0, "entries, no solve");
        // neither entry nor recipe: the typed refusal
        match reduce(&served, [3, 2, 1, 1, 1], None, PUncoveredPolicy::Reject) {
            Err(PError::OutsideCertifiedSet { node: named, .. }) => assert_eq!(named, node),
            other => panic!("expected OutsideCertifiedSet, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// vk5l WP2 (manifest, plan/ARTIFACT_MANIFEST.md): a shipped node
    /// directory written by `write_node_service` with precedence `over_table`
    /// serves its keys BEFORE the table's first match (counted
    /// `over_table_hits`); the same entries with `after_table` leave a key the
    /// rules take to the rules.
    #[test]
    fn manifest_precedence_over_table_puts_entries_before_the_rules() {
        use super::super::relation_entries::{
            EntryMeta, EntryPrecedence, RelationStore, build_entry, write_node_service,
        };
        let node = "KITE-manifest";
        let _fake = install(node, vec![]);
        let imported = import(node, &[], None).unwrap();
        let bundle = recipes_of_source(
            &crate::propagator::custom::custom_payload_family_source(&imported.payload).unwrap(),
        )
        .unwrap()
        .unwrap();
        let identity = bundle_node_identity(&bundle);
        let base =
            std::env::temp_dir().join(format!("rr-manifest-dispatch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let key = dot(0); // a certified key the table's rules take
        let body = json!({"type": "relation", "key": key, "verdict": "SOLVED", "all_lower": true,
            "proof": {"ok": true, "kind": "symverify"}, "relation_digest": null,
            "determinism_id": "rustred.relation-import.v1/kind=test/statement=manifest",
            "rhs": [["I(1,1,1,1,1)", "7", "top"]]});
        let meta = EntryMeta {
            node: node.to_owned(),
            node_identity: identity.clone(),
            package_revision: None,
            recipe_id: format!("{node}/manifest-test"),
            recipe: json!({}),
            recipe_digest: String::new(),
            relation_rule: "test".to_owned(),
            evidence_level: "per-key",
            pole_order_at_4: Some(0),
            regular_alternative: "not_tried",
            outside_certified_set: false,
            written_by: "test".to_owned(),
        };
        let cache = base.join("cache");
        RelationStore::new(node, &identity, &[], Some(&cache))
            .put(&build_entry(&meta, &key, body))
            .unwrap();
        let reduce_with = |precedence: EntryPrecedence| {
            let ship = base.join(precedence.name());
            write_node_service(
                node,
                &identity,
                std::slice::from_ref(&cache),
                &ship,
                "t",
                precedence,
                &Value::Null,
            )
            .unwrap();
            set_recipe_cache_roots_for(
                node,
                RecipeCacheRoots {
                    user: None,
                    prefilled: vec![ship],
                    ..Default::default()
                },
            );
            let served = certified_library_without_recipes(node);
            let before = super::super::relation_entries::relation_cache_statistics();
            let reduced = reduce(&served, key, None, PUncoveredPolicy::Reject).unwrap();
            let stats = super::super::relation_entries::relation_cache_statistics().since(&before);
            (format!("{:?}", reduced.terms), stats.over_table_hits)
        };
        let (over, over_hits) = reduce_with(EntryPrecedence::OverTable);
        let (after, after_hits) = reduce_with(EntryPrecedence::AfterTable);
        assert!(
            over_hits >= 1 && after_hits == 0,
            "{over_hits} {after_hits}"
        );
        assert_ne!(over, after, "the entry, not the rule, served the key");
        assert!(over.contains('7'), "{over}");
        let _ = std::fs::remove_dir_all(&base);
    }

    fn certified_library_without_recipes(node: &str) -> Arc<PLibrary> {
        boxed_kite_library_with_recipes(
            &boxed_kite_table_d2(node, &[]),
            Some(certified(node)),
            None,
        )
        .unwrap()
    }

    /// Coordinator 2026-11-15 (T178 hand-down stall): an outside recipe solve
    /// is bounded in wall time and fails typed (BUDGET) instead of running for
    /// hours; the key then is OUTSIDE_NO_RELATION with that attempt.
    #[test]
    fn the_abandoned_solve_cap_defaults_to_the_worker_count() {
        // the former default 1 refused every later bounded solve of a process after one solve passed its bound
        if std::env::var(OUTSIDE_MAX_ABANDONED_ENV).is_err() {
            assert_eq!(EngineRecipeSolver::new(3).abandoned_cap(), 3);
            assert_eq!(EngineRecipeSolver::new(1).abandoned_cap(), 1);
            assert_eq!(EngineRecipeSolver::new(0).abandoned_cap(), 1);
        }
    }

    #[test]
    fn an_outside_solve_past_its_wall_bound_fails_typed() {
        let node = "KITE-out-budget";
        let three = [2, 2, 2, 1, 1];
        let (_, solved) = toy(three);
        let fake = install(node, vec![toy(dot(0)), (three.to_vec(), solved)]);
        *fake.delay_s.lock().unwrap() = 10.0;
        set_outside_settings_for(
            node,
            POutsideSettings {
                max_seconds_per_solve: Some(0.5),
                ..POutsideSettings::default()
            },
        );
        let records = vec![record("kite/d0", &[dot(0)], &SHELLS)];
        let imported = import(node, &[dot(0)], Some(records)).unwrap();
        let before = recipe_statistics().outside_budget_hits;
        let error = crate::propagator::custom::custom_payload_solve_outside(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
            &presented(three),
        )
        .unwrap_err();
        let PError::RecipeFailed(failed) = &error else {
            panic!("expected RecipeFailed, got {error:?}");
        };
        assert_eq!(failed.kind, PRecipeFailureKind::OutsideNoRelation);
        assert_eq!(failed.failure["outside"]["attempts"][0]["kind"], "BUDGET");
        assert!(recipe_statistics().outside_budget_hits > before);
        // without the bound the same solve succeeds
        set_outside_settings_for(
            node,
            POutsideSettings {
                max_seconds_per_solve: None,
                max_seconds_per_key: None,
                ..POutsideSettings::default()
            },
        );
        let again = import(
            node,
            &[dot(0)],
            Some(vec![record("kite/d0", &[dot(0)], &SHELLS)]),
        )
        .unwrap();
        assert!(
            crate::propagator::custom::custom_payload_solve_outside(
                &again.presentation,
                &again.payload,
                &again.meta,
                &presented(three),
            )
            .unwrap()
            .is_some()
        );
    }

    /// batch 11 (vk5l-physrun 630 coverage): max_recipes 0 is list-only: an
    /// outside key no rule or entry takes is refused at once, no recipe solved.
    #[test]
    fn list_only_outside_settings_solve_nothing() {
        let node = "KITE-out-list";
        let three = [2, 2, 2, 1, 1];
        let (_, solved) = toy(three);
        let fake = install(node, vec![toy(dot(0)), (three.to_vec(), solved)]);
        set_outside_settings_for(
            node,
            POutsideSettings {
                max_recipes: 0,
                ..POutsideSettings::default()
            },
        );
        let imported = import(
            node,
            &[dot(0)],
            Some(vec![record("kite/d0", &[dot(0)], &SHELLS)]),
        )
        .unwrap();
        let (calls, listed) = (fake.calls(), recipe_statistics().outside_listed);
        let found = crate::propagator::custom::custom_payload_solve_outside(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
            &presented(three),
        )
        .unwrap();
        assert!(found.is_none());
        assert_eq!(fake.calls(), calls, "no recipe solved");
        assert!(recipe_statistics().outside_listed > listed);
    }

    #[test]
    fn the_pole_order_at_d_4_counts_the_multiplicity_in_the_denominator() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let parse = |text: &str| {
            super::super::basis::parse_coefficient(&context, text, ExactAlgebraLimits::default())
                .unwrap()
        };
        assert_eq!(pole_order_at(&parse("1"), 4), Some(0));
        assert_eq!(pole_order_at(&parse("(d-4)/(d-3)"), 4), Some(0));
        assert_eq!(pole_order_at(&parse("1/(d-4)"), 4), Some(1));
        assert_eq!(pole_order_at(&parse("1/((d-4)^2*(3*d-16))"), 4), Some(2));
    }

    /// The real engine at a key outside the certified set: the family of
    /// kite/d0 (shells S0..S2) solves a three-dot key in process; an S0-only
    /// family fails typed (OUTSIDE_NO_RELATION) with a reproduction line
    /// that reruns the same class stand-alone.
    #[test]
    fn the_engine_solves_an_outside_key_with_the_recipe_families_or_fails_typed() {
        if !crate::compiled_runtime_arities().contains(&5) {
            eprintln!("skipped: arity 5 is not compiled");
            return;
        }
        use crate::propagator::custom::custom_payload_solve_outside;
        // (the toy library's census stops at the kite's own sectors, so the
        // real relation's subsector children are checked here at the payload)
        let three = [2, 2, 2, 1, 1];
        let imported = import(
            "KITE-engine-out",
            &[dot(0)],
            Some(vec![record("kite/d0", &[dot(0)], &SHELLS)]),
        )
        .unwrap();
        let (relation, id) = custom_payload_solve_outside(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
            &presented(three),
        )
        .unwrap()
        .expect("a relation");
        assert_eq!(id, "kite/d0");
        assert!(!relation.children.is_empty());
        assert!(relation.relation_digest.is_some(), "a fresh engine solve");
        let failing = import(
            "KITE-engine-out-x",
            &[dot(0)],
            Some(vec![record("kite/d0-S0", &[dot(0)], &["S0"])]),
        )
        .unwrap();
        match custom_payload_solve_outside(
            &failing.presentation,
            &failing.payload,
            &failing.meta,
            &presented(three),
        ) {
            Err(PError::RecipeFailed(failed)) => {
                assert_eq!(
                    failed.kind,
                    PRecipeFailureKind::OutsideNoRelation,
                    "{failed:?}"
                );
                assert_eq!(failed.failure["outside"]["tried"], json!(1));
                assert_eq!(
                    failed.failure["outside"]["attempts"][0]["kind"],
                    json!("NO_RELATION_IN_SEEDS")
                );
                assert!(
                    failed.reproduce.contains("p-recipes solve"),
                    "{}",
                    failed.reproduce
                );
                assert!(
                    failed.reproduce.contains("--key 2,2,2,1,1"),
                    "{}",
                    failed.reproduce
                );
                assert!(
                    failed.reproduce.contains("--max-trace-rows"),
                    "{}",
                    failed.reproduce
                );
                let dir = failed.reproduce_dir.as_ref().expect("files");
                assert!(PathBuf::from(dir).join("recipe.json").exists());
            }
            other => panic!("expected RecipeFailed, got {other:?}"),
        }
    }

    /// The explicit-expression producer with the REAL engine: a boxed node
    /// with no recipe records (relation binding only) gets an explicit
    /// relation at a key from the checker families, written to the cache
    /// root; a rerun is served by the stored entry (resumable, no solve); the
    /// cache turns into the shipped manifest form.
    #[test]
    fn the_producer_writes_explicit_relations_and_resumes() {
        if !crate::compiled_runtime_arities().contains(&5) {
            eprintln!("skipped: arity 5 is not compiled");
            return;
        }
        use crate::propagator::custom::custom_payload_relation_produce;
        let node = "KITE-produce";
        let root = std::env::temp_dir().join(format!("rr-produce-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        set_recipe_cache_roots_for(
            node,
            RecipeCacheRoots {
                user: Some(root.clone()),
                prefilled: vec![],
                ..Default::default()
            },
        );
        let imported = import(node, &[], None).unwrap();
        let three = vec![2_i64, 2, 2, 1, 1];
        let run = || {
            custom_payload_relation_produce(
                &imported.presentation,
                &imported.payload,
                std::slice::from_ref(&three),
                &[false],
                &[2, 3],
                None,
                1,
                &|_| {},
            )
            .unwrap()
        };
        let first = run();
        assert_eq!(first.len(), 1);
        let id = match &first[0].source {
            Ok(Some(id)) => id.clone(),
            other => panic!("expected a produced relation, got {other:?}"),
        };
        assert!(id.starts_with("KITE-produce/checker-d"), "{id}");
        assert!(first[0].children > 0);
        let bundle = recipes_of_source(
            &crate::propagator::custom::custom_payload_family_source(&imported.payload).unwrap(),
        )
        .unwrap()
        .unwrap();
        let identity = bundle_node_identity(&bundle);
        let stored =
            super::super::relation_entries::RelationStore::new(node, &identity, &[], Some(&root));
        let (entry, _) = stored.get(&three).expect("the entry is in the cache root");
        assert_eq!(entry["recipe"]["id"], json!(id));
        // a rerun (new payload program, same cache root): the stored entry
        let directory = super::super::relation_entries::node_directory(&root, node, &identity);
        let segments = || std::fs::read_dir(&directory).unwrap().count();
        let before = segments();
        let second = run();
        assert!(matches!(&second[0].source, Ok(Some(again)) if *again == id));
        assert_eq!(
            segments(),
            before,
            "resumed from the cache: nothing written"
        );
        let summary = super::super::relation_entries::write_node_service(
            node,
            &identity,
            std::slice::from_ref(&root),
            &root.join("ship"),
            "t",
            super::super::relation_entries::EntryPrecedence::OverTable,
            &Value::Null,
        )
        .unwrap();
        assert_eq!(summary.entries, 1);
        // a closure holding only the key itself: every relation's top-sector
        // right-hand side leaves it, so every family is refused, typed
        let node_c = "KITE-produce-closure";
        set_recipe_cache_roots_for(
            node_c,
            RecipeCacheRoots {
                user: Some(root.join("closure")),
                prefilled: vec![],
                ..Default::default()
            },
        );
        let imported_c = import(node_c, &[], None).unwrap();
        let closure = super::super::certified_keys::CertifiedKeySet::new(
            node_c,
            5,
            "table",
            vec![three.clone()],
        )
        .unwrap();
        let refused = custom_payload_relation_produce(
            &imported_c.presentation,
            &imported_c.payload,
            std::slice::from_ref(&three),
            &[false],
            &[2],
            Some(&closure),
            1,
            &|_| {},
        )
        .unwrap();
        match &refused[0].source {
            Err(PError::RecipeFailed(failed)) => {
                let attempts = &failed.failure["outside"]["attempts"];
                assert_eq!(
                    attempts[0]["kind"],
                    json!("RHS_OUTSIDE_CERTIFIED"),
                    "{attempts}"
                );
                assert_eq!(
                    attempts[1]["recipe"],
                    json!("KITE-produce-closure/checker-d2-nosym")
                );
            }
            other => panic!("expected a typed refusal, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// (b, c) with the REAL engine solve at the key (owner level, no fake):
    /// the load check steps the recipe key in process (solved, verified,
    /// within the box), the replay ends on the corner and subsector keys; a
    /// recipe whose seeds cannot reach its key fails with the verb's class.
    #[test]
    fn the_engine_solves_kite_recipes_in_process_and_reports_its_failures() {
        if !crate::compiled_runtime_arities().contains(&5) {
            eprintln!("skipped: arity 5 is not compiled");
            return;
        }
        let imported = import(
            "KITE-engine",
            &[dot(0)],
            Some(vec![record("kite/d0", &[dot(0)], &SHELLS)]),
        )
        .unwrap();
        let check =
            verify_custom_payload(&imported.presentation, &imported.payload, &imported.meta)
                .unwrap();
        assert_eq!(check.recipe_keys, 1);
        let replays = replay_custom_payload(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
            &[presented(dot(0)), presented(dots(0, 1))],
        )
        .unwrap();
        for replay in &replays {
            assert_eq!(replay.residuals.len(), 1, "{:?}", replay.residuals);
            assert_eq!(replay.residuals[0].0, vec![1; 5]);
            assert!(
                !replay.boundary.is_empty(),
                "a real IBP relation has subsector terms"
            );
        }
        let failing = import(
            "KITE-engine-x",
            &[dot(0)],
            Some(vec![record("kite/d0-S0", &[dot(0)], &["S0"])]),
        )
        .unwrap();
        match verify_custom_payload(&failing.presentation, &failing.payload, &failing.meta) {
            Err(PError::RecipeFailed(failed)) => {
                assert_eq!(failed.kind, PRecipeFailureKind::NoRelationInSeeds);
                assert_eq!(failed.class, "no_relation_in_seeds");
                assert_eq!(failed.failure["schema"], "rustred.recipe-failure.v1");
                assert!(
                    failed.reproduce.contains("p-recipes solve"),
                    "{}",
                    failed.reproduce
                );
                // the verb's object carries the stand-alone line, not its in-process one
                assert_eq!(failed.failure["reproduce"], json!(failed.reproduce));
                let dir = failed.reproduce_dir.as_ref().expect("files");
                assert_eq!(
                    failed.failure["recipe"]["path"],
                    json!(format!("{dir}/recipe.json"))
                );
                assert_ne!(
                    failed.failure["binary"]["git_commit"],
                    json!("unknown"),
                    "build commit stamped"
                );
                assert!(
                    failed
                        .to_string()
                        .contains("original request: not recorded")
                );
            }
            other => panic!("expected RecipeFailed, got {other:?}"),
        }
        let _ = PresentationId(0);
    }
}
