//! The spec parameters of the pole diagnostics and of the preference (pole-diagnostics.md section 10, addenda 4 and 7;
//! TRICKS item 12 for the inherent-vs-spurious composition). Every field has a documented default; a spec may omit any
//! of them. Unknown members are REFUSED (a typo must not silently fall back to a default).
//!
//! ```json
//! "pole_diagnostics": {"enabled": true, "dimensions": ["4"], "primes": 2, "generic_points": 2,
//!   "relations": {"enabled": true, "table_rules": "factor_and_locus", "per_key": true, "worst_max": 20},
//!   "basis": {"enabled": true, "routes": ["residue", "engine"], "seed_box": "auto", "seed_margin": {"dots": 1, "rank": 0},
//!             "symmetry_rows": true, "budget_cpu_s": 3600, "comparison_sets": []},
//!   "census": {"enabled": true, "mode": "auto", "keys": "box", "max_cpu_h": 4, "sample": {"n": 2000, "seed": 0},
//!              "truncation": 6, "exact_leading": true, "include_exits": false, "terminal_sets": ["found", "regular"]},
//!   "inherent": {"enabled": true, "max_cone_keys": 200000, "budget_s": 600},
//!   "max_cpu_h_per_node": 8},
//! "repair_policy": {"prefer_regular_at": ["4"],
//!   "prefer_regular": {"criteria_order": "economy_first", "max_size_ratio": "unbounded", "max_further_steps": 1,
//!     "probe": {"key_budget_s": 60, "primes": 2}, "solve": {"key_budget_s": 600}, "max_keys_per_node": 1000,
//!     "max_cpu_h_per_node": 2, "revert_if_open": true,
//!     "terminal_set": {"enabled": true, "max_candidates": 32, "max_rotations": "deficiency", "export": "regular",
//!                      "confirm_engine": false, "budget_cpu_h": 1}}}
//! ```
//!
//! | field | default | effect |
//! |---|---|---|
//! | `pole_diagnostics.enabled` | true | false = no diagnostic ARTEFACTS (the preference still computes what it needs) |
//! | `dimensions` | ["4"] | rational d0 values (`a` or `a/b`) sampled by D1-D3 |
//! | `primes` | 2 | primes per modular statement (probes, route E, census); >= 2 |
//! | `generic_points` | 2 | generic d values of the "generic" side of D2 |
//! | `relations.enabled` | true | D1 records per relation |
//! | `relations.table_rules` | factor_and_locus | `off` / `factor_only` / `factor_and_locus` (locus keys instantiated) |
//! | `relations.per_key` | true | per-key verdicts for parametric rules (TRICKS 9: poles can depend on the indices) |
//! | `relations.worst_max` | 20 | maximal-order terms listed in a record's `worst` |
//! | `basis.routes` | residue, engine | D2 routes (`residue`, `engine`, `exact`) |
//! | `basis.seed_box` / `seed_margin` | auto / +1 dot, +0 rank | node-local seeds: smallest box covering T and the census keys + margin |
//! | `basis.symmetry_rows` | true | node-local system with the node's exact symmetry rows |
//! | `basis.budget_cpu_s` | 3600 | CPU budget of one node-basis run |
//! | `basis.comparison_sets` | [] | extra terminal sets measured for comparison only (data; never steering) |
//! | `census.mode` | auto | `exact` / `sampled` / `auto` (exact under `max_cpu_h`) |
//! | `census.keys` | box | `box` (box definition) or `closure` |
//! | `census.max_cpu_h` | 4 | budget of an exact census before `auto` samples |
//! | `census.sample` | n 2000, seed 0 | key sample of `sampled` mode |
//! | `census.truncation` | 6 | Laurent terms kept above the pole; a full-zero series is `truncated` |
//! | `census.exact_leading` | true | reconstruct leading Laurent coefficients over Q |
//! | `census.include_exits` | false | per key also the max order of subsector coefficients |
//! | `census.terminal_sets` | found, regular | labelled sets that get a census |
//! | `inherent.enabled` | true | TRICKS 12(c): compose a singular relation down to a regular terminal set |
//! | `inherent.max_cone_keys` | 200000 | composition budget (keys of the cone); over it: `undecided: budget` |
//! | `inherent.budget_s` | 600 | composition wall budget per relation; over it: `undecided: budget` (time-decided) |
//! | `max_cpu_h_per_node` | 8 | cap of all diagnostics of one node |
//! | `prefer_regular_at` | ["4"] | dimensions of the preference; [] = OFF (parsed and recorded; no production consumer today) |
//! | `criteria_order` | economy_first | USER RULING 2026-11-19 (rule 21): `economy_first` (size first; regularity breaks exact ties only) is the default; `regular_first` (addendum 4/7, withdrawn) only when named |
//! | `max_size_ratio` | "unbounded" | bounds the regular class under the explicit `regular_first` only (the 2026-11-15 ruling it served is withdrawn) |
//! | `max_further_steps` | 1 | escalation steps after the serving step tried for a regular relation (parsed and recorded; no production consumer today) |
//! | `probe.key_budget_s` / `probe.primes` | 60 / 2 | probe wall cap per key / primes |
//! | `solve.key_budget_s` | 600 | wall cap of a d0-pivoted solve |
//! | `max_keys_per_node`, `max_cpu_h_per_node` | 1000, 2 | caps of the relation preference per node |
//! | `revert_if_open` | true | the revert rule of section 8 |
//! | `terminal_set.enabled` | true | phase F (section 9) |
//! | `terminal_set.max_candidates` / `max_rotations` | 32 / deficiency | candidate cap; rotations up to the deficiency (or a number) |
//! | `terminal_set.export` | regular | which set the export marks as the node's basis (`found` keeps the closure's) |
//! | `terminal_set.confirm_engine` | false | route E confirmation of T' at d0 |
//! | `terminal_set.budget_cpu_h` | 1 | phase F budget per node |
//!
//! `max_size_ratio` is also read where other policies carry it (`symbolic.max_size_ratio` of the repair policy, the
//! seed-minimisation choice of coord-seed-minimise.md 6): [`MaxSizeRatio::parse`] is the one parser.

use serde_json::{Map, Value};
use std::fmt;

/// A typed refusal of a spec section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyError(pub String);

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pole policy: {}", self.0)
    }
}

impl std::error::Error for PolicyError {}

fn err<T>(m: impl Into<String>) -> Result<T, PolicyError> {
    Err(PolicyError(m.into()))
}

/// The size guard of the preference (addendum 7): `Unbounded` (DEFAULT: a candidate regular at d0 always wins over a
/// singular one, whatever its size) or a rational `num/den >= 1` (a regular candidate wins only up to that multiple of
/// the smallest admissible candidate's size).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaxSizeRatio {
    #[default]
    Unbounded,
    Ratio {
        num: u64,
        den: u64,
    },
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

impl MaxSizeRatio {
    /// `"unbounded"`, a rational text `"8"` / `"5/2"`, or a JSON integer; must be >= 1.
    pub fn parse(v: &Value) -> Result<MaxSizeRatio, PolicyError> {
        let bad = || {
            PolicyError(format!(
                "max_size_ratio {v} is not \"unbounded\" or a rational >= 1"
            ))
        };
        let (num, den) = match v {
            Value::String(s) if s == "unbounded" => return Ok(MaxSizeRatio::Unbounded),
            Value::String(s) => {
                let (a, b) = s.split_once('/').unwrap_or((s.as_str(), "1"));
                let p = |t: &str| {
                    (!t.is_empty() && t.bytes().all(|c| c.is_ascii_digit()))
                        .then(|| t.parse::<u64>().ok())
                        .flatten()
                };
                (p(a).ok_or_else(bad)?, p(b).ok_or_else(bad)?)
            }
            Value::Number(n) => (n.as_u64().ok_or_else(bad)?, 1),
            _ => return Err(bad()),
        };
        if den == 0 || num < den {
            return Err(bad());
        }
        let g = gcd(num, den);
        Ok(MaxSizeRatio::Ratio {
            num: num / g,
            den: den / g,
        })
    }

    /// Does a candidate of `size` stay within the guard against the smallest admissible size `smallest`? Exact
    /// integer arithmetic (`size * den <= num * smallest`).
    pub fn admits(&self, size: u64, smallest: u64) -> bool {
        match self {
            MaxSizeRatio::Unbounded => true,
            MaxSizeRatio::Ratio { num, den } => {
                u128::from(size) * u128::from(*den) <= u128::from(*num) * u128::from(smallest)
            }
        }
    }

    /// The canonical text (`"unbounded"`, `"8"`, `"5/2"`).
    pub fn text(&self) -> String {
        match self {
            MaxSizeRatio::Unbounded => "unbounded".into(),
            MaxSizeRatio::Ratio { num, den: 1 } => num.to_string(),
            MaxSizeRatio::Ratio { num, den } => format!("{num}/{den}"),
        }
    }
}

/// `criteria_order` of the preference. USER RULING 2026-11-19 (rule 21): `economy_first` is the DEFAULT; `regular_first`
/// is an explicit, non-default option (its choices say so in their record).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CriteriaOrder {
    /// Addendum 4 / 7 (the default until 2026-11-19): regular first (with the size guard), then lower pole order, then
    /// fewer terms. Explicit only.
    RegularFirst,
    /// THE DEFAULT: size first; regularity, then pole order, break exact ties only.
    #[default]
    EconomyFirst,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelationsParams {
    pub enabled: bool,
    /// `off` / `factor_only` / `factor_and_locus`.
    pub table_rules: String,
    pub per_key: bool,
    /// How many maximal-order terms a record's `worst` lists.
    pub worst_max: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BasisParams {
    pub enabled: bool,
    pub routes: Vec<String>,
    pub seed_box: String,
    pub seed_margin_dots: u64,
    pub seed_margin_rank: u64,
    pub symmetry_rows: bool,
    pub budget_cpu_s: u64,
    pub comparison_sets: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CensusParams {
    pub enabled: bool,
    pub mode: String,
    pub keys: String,
    pub max_cpu_h: f64,
    pub sample_n: u64,
    pub sample_seed: u64,
    pub truncation: u64,
    pub exact_leading: bool,
    pub include_exits: bool,
    pub terminal_sets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InherentParams {
    pub enabled: bool,
    pub max_cone_keys: u64,
    pub budget_s: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerminalSetParams {
    pub enabled: bool,
    pub max_candidates: u64,
    /// `None` = "deficiency" (rotations up to the measured deficiency).
    pub max_rotations: Option<u64>,
    pub export: String,
    pub confirm_engine: bool,
    pub budget_cpu_h: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreferParams {
    pub criteria_order: CriteriaOrder,
    pub max_size_ratio: MaxSizeRatio,
    pub max_further_steps: u64,
    pub probe_key_budget_s: u64,
    pub probe_primes: u64,
    pub solve_key_budget_s: u64,
    pub max_keys_per_node: u64,
    pub max_cpu_h_per_node: f64,
    pub revert_if_open: bool,
    pub terminal_set: TerminalSetParams,
}

/// Every parameter of the pole diagnostics and the preference, defaulted ([`PoleParams::default`]).
#[derive(Debug, Clone, PartialEq)]
pub struct PoleParams {
    pub enabled: bool,
    /// Canonical rational texts (`a` or `a/b`, lowest terms, b > 0).
    pub dimensions: Vec<String>,
    pub primes: u64,
    pub generic_points: u64,
    pub relations: RelationsParams,
    pub basis: BasisParams,
    pub census: CensusParams,
    pub inherent: InherentParams,
    pub max_cpu_h_per_node: f64,
    /// `[]` = the preference is OFF.
    pub prefer_regular_at: Vec<String>,
    pub prefer: PreferParams,
}

impl Default for PoleParams {
    fn default() -> Self {
        PoleParams {
            enabled: true,
            dimensions: vec!["4".into()],
            primes: 2,
            generic_points: 2,
            relations: RelationsParams {
                enabled: true,
                table_rules: "factor_and_locus".into(),
                per_key: true,
                worst_max: 20,
            },
            basis: BasisParams {
                enabled: true,
                routes: vec!["residue".into(), "engine".into()],
                seed_box: "auto".into(),
                seed_margin_dots: 1,
                seed_margin_rank: 0,
                symmetry_rows: true,
                budget_cpu_s: 3600,
                comparison_sets: vec![],
            },
            census: CensusParams {
                enabled: true,
                mode: "auto".into(),
                keys: "box".into(),
                max_cpu_h: 4.0,
                sample_n: 2000,
                sample_seed: 0,
                truncation: 6,
                exact_leading: true,
                include_exits: false,
                terminal_sets: vec!["found".into(), "regular".into()],
            },
            inherent: InherentParams {
                enabled: true,
                max_cone_keys: 200_000,
                budget_s: 600,
            },
            max_cpu_h_per_node: 8.0,
            prefer_regular_at: vec!["4".into()],
            prefer: PreferParams {
                criteria_order: CriteriaOrder::EconomyFirst,
                max_size_ratio: MaxSizeRatio::Unbounded,
                max_further_steps: 1,
                probe_key_budget_s: 60,
                probe_primes: 2,
                solve_key_budget_s: 600,
                max_keys_per_node: 1000,
                max_cpu_h_per_node: 2.0,
                revert_if_open: true,
                terminal_set: TerminalSetParams {
                    enabled: true,
                    max_candidates: 32,
                    max_rotations: None,
                    export: "regular".into(),
                    confirm_engine: false,
                    budget_cpu_h: 1.0,
                },
            },
        }
    }
}

/// Canonical rational text of a dimension (`a` / `a/b`, lowest terms, b > 0; i64 range). The algebra crate's `D0`
/// accepts any size; the spec's list is of small rationals.
pub fn canonical_dimension(t: &str) -> Result<String, PolicyError> {
    let bad = || PolicyError(format!("dimension {t:?} is not a rational a or a/b"));
    let (a, b) = t.split_once('/').unwrap_or((t, "1"));
    let int = |s: &str| -> Option<i64> {
        let body = s.strip_prefix('-').unwrap_or(s);
        (!body.is_empty() && body.bytes().all(|c| c.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let (a, b) = (int(a).ok_or_else(bad)?, int(b).ok_or_else(bad)?);
    if b == 0 {
        return Err(bad());
    }
    let g = gcd(a.unsigned_abs(), b.unsigned_abs()) as i64;
    let (mut a, mut b) = (a / g, b / g);
    if b < 0 {
        a = -a;
        b = -b;
    }
    Ok(if b == 1 {
        a.to_string()
    } else {
        format!("{a}/{b}")
    })
}

struct Obj<'a> {
    at: &'a str,
    m: &'a Map<String, Value>,
}

impl<'a> Obj<'a> {
    fn new(at: &'a str, v: &'a Value, allowed: &[&str]) -> Result<Obj<'a>, PolicyError> {
        let m = v
            .as_object()
            .ok_or_else(|| PolicyError(format!("{at} is not an object")))?;
        if let Some(k) = m.keys().find(|k| !allowed.contains(&k.as_str())) {
            return err(format!("{at}: unknown member {k:?} (allowed: {allowed:?})"));
        }
        Ok(Obj { at, m })
    }
    fn get(&self, k: &str) -> Option<&'a Value> {
        self.m.get(k)
    }
    fn bool(&self, k: &str, d: &mut bool) -> Result<(), PolicyError> {
        if let Some(v) = self.get(k) {
            *d = v
                .as_bool()
                .ok_or_else(|| PolicyError(format!("{}.{k} is not a boolean", self.at)))?;
        }
        Ok(())
    }
    fn uint(&self, k: &str, d: &mut u64) -> Result<(), PolicyError> {
        if let Some(v) = self.get(k) {
            *d = v.as_u64().ok_or_else(|| {
                PolicyError(format!("{}.{k} is not an unsigned integer", self.at))
            })?;
        }
        Ok(())
    }
    fn num(&self, k: &str, d: &mut f64) -> Result<(), PolicyError> {
        if let Some(v) = self.get(k) {
            let x = v.as_f64().filter(|x| *x >= 0.0);
            *d = x.ok_or_else(|| {
                PolicyError(format!("{}.{k} is not a non-negative number", self.at))
            })?;
        }
        Ok(())
    }
    fn choice(&self, k: &str, allowed: &[&str], d: &mut String) -> Result<(), PolicyError> {
        if let Some(v) = self.get(k) {
            let s = v.as_str().filter(|s| allowed.contains(s));
            *d = s
                .ok_or_else(|| {
                    PolicyError(format!("{}.{k} = {v} is not one of {allowed:?}", self.at))
                })?
                .to_string();
        }
        Ok(())
    }
    fn strings(
        &self,
        k: &str,
        allowed: Option<&[&str]>,
        d: &mut Vec<String>,
    ) -> Result<(), PolicyError> {
        if let Some(v) = self.get(k) {
            let a = v
                .as_array()
                .ok_or_else(|| PolicyError(format!("{}.{k} is not a list", self.at)))?;
            let mut out = Vec::with_capacity(a.len());
            for x in a {
                let s = x
                    .as_str()
                    .ok_or_else(|| PolicyError(format!("{}.{k}: {x} is not a string", self.at)))?;
                if allowed.is_some_and(|al| !al.contains(&s)) {
                    return err(format!("{}.{k}: {s:?} is not one of {allowed:?}", self.at));
                }
                out.push(s.to_string());
            }
            *d = out;
        }
        Ok(())
    }
}

fn dimensions(v: &[String], at: &str) -> Result<Vec<String>, PolicyError> {
    let mut out: Vec<String> = Vec::with_capacity(v.len());
    for t in v {
        let c = canonical_dimension(t).map_err(|e| PolicyError(format!("{at}: {}", e.0)))?;
        if out.contains(&c) {
            return err(format!("{at}: dimension {c} listed twice"));
        }
        out.push(c);
    }
    Ok(out)
}

impl PoleParams {
    /// Parse the two spec sections (each optional: `None` = all defaults). `repair_policy` may hold other members of
    /// the repair policy: only `prefer_regular_at` and `prefer_regular` are read from it.
    pub fn from_spec(
        pole_diagnostics: Option<&Value>,
        repair_policy: Option<&Value>,
    ) -> Result<PoleParams, PolicyError> {
        let mut p = PoleParams::default();
        if let Some(v) = pole_diagnostics {
            let o = Obj::new(
                "pole_diagnostics",
                v,
                &[
                    "enabled",
                    "dimensions",
                    "primes",
                    "generic_points",
                    "relations",
                    "basis",
                    "census",
                    "inherent",
                    "max_cpu_h_per_node",
                    "prefer_regular",
                ],
            )?;
            o.bool("enabled", &mut p.enabled)?;
            o.strings("dimensions", None, &mut p.dimensions)?;
            p.dimensions = dimensions(&p.dimensions, "pole_diagnostics.dimensions")?;
            o.uint("primes", &mut p.primes)?;
            if p.primes < 2 {
                return err("pole_diagnostics.primes must be >= 2");
            }
            o.uint("generic_points", &mut p.generic_points)?;
            if p.generic_points < 1 {
                return err("pole_diagnostics.generic_points must be >= 1");
            }
            o.num("max_cpu_h_per_node", &mut p.max_cpu_h_per_node)?;
            if let Some(r) = o.get("relations") {
                let r = Obj::new(
                    "pole_diagnostics.relations",
                    r,
                    &["enabled", "table_rules", "per_key", "worst_max"],
                )?;
                r.bool("enabled", &mut p.relations.enabled)?;
                r.choice(
                    "table_rules",
                    &["off", "factor_only", "factor_and_locus"],
                    &mut p.relations.table_rules,
                )?;
                r.bool("per_key", &mut p.relations.per_key)?;
                r.uint("worst_max", &mut p.relations.worst_max)?;
            }
            if let Some(b) = o.get("basis") {
                let b = Obj::new(
                    "pole_diagnostics.basis",
                    b,
                    &[
                        "enabled",
                        "routes",
                        "seed_box",
                        "seed_margin",
                        "symmetry_rows",
                        "budget_cpu_s",
                        "comparison_sets",
                    ],
                )?;
                b.bool("enabled", &mut p.basis.enabled)?;
                b.strings(
                    "routes",
                    Some(&["residue", "engine", "exact"]),
                    &mut p.basis.routes,
                )?;
                b.choice("seed_box", &["auto"], &mut p.basis.seed_box)?;
                if let Some(m) = b.get("seed_margin") {
                    let m = Obj::new("pole_diagnostics.basis.seed_margin", m, &["dots", "rank"])?;
                    m.uint("dots", &mut p.basis.seed_margin_dots)?;
                    m.uint("rank", &mut p.basis.seed_margin_rank)?;
                }
                b.bool("symmetry_rows", &mut p.basis.symmetry_rows)?;
                b.uint("budget_cpu_s", &mut p.basis.budget_cpu_s)?;
                if let Some(c) = b.get("comparison_sets") {
                    p.basis.comparison_sets = c
                        .as_array()
                        .ok_or_else(|| {
                            PolicyError(
                                "pole_diagnostics.basis.comparison_sets is not a list".into(),
                            )
                        })?
                        .clone();
                }
            }
            if let Some(c) = o.get("census") {
                let c = Obj::new(
                    "pole_diagnostics.census",
                    c,
                    &[
                        "enabled",
                        "mode",
                        "keys",
                        "max_cpu_h",
                        "sample",
                        "truncation",
                        "exact_leading",
                        "include_exits",
                        "terminal_sets",
                    ],
                )?;
                c.bool("enabled", &mut p.census.enabled)?;
                c.choice("mode", &["auto", "exact", "sampled"], &mut p.census.mode)?;
                c.choice("keys", &["box", "closure"], &mut p.census.keys)?;
                c.num("max_cpu_h", &mut p.census.max_cpu_h)?;
                if let Some(s) = c.get("sample") {
                    let s = Obj::new("pole_diagnostics.census.sample", s, &["n", "seed"])?;
                    s.uint("n", &mut p.census.sample_n)?;
                    s.uint("seed", &mut p.census.sample_seed)?;
                }
                c.uint("truncation", &mut p.census.truncation)?;
                if p.census.truncation == 0 {
                    return err("pole_diagnostics.census.truncation must be >= 1");
                }
                c.bool("exact_leading", &mut p.census.exact_leading)?;
                c.bool("include_exits", &mut p.census.include_exits)?;
                c.strings("terminal_sets", None, &mut p.census.terminal_sets)?;
            }
            if let Some(i) = o.get("inherent") {
                let i = Obj::new(
                    "pole_diagnostics.inherent",
                    i,
                    &["enabled", "max_cone_keys", "budget_s"],
                )?;
                i.bool("enabled", &mut p.inherent.enabled)?;
                i.uint("max_cone_keys", &mut p.inherent.max_cone_keys)?;
                i.uint("budget_s", &mut p.inherent.budget_s)?;
            }
            // addendum 7: `pole_diagnostics.prefer_regular.max_size_ratio` is one of the places that carry the guard
            if let Some(pr) = o.get("prefer_regular") {
                let pr = Obj::new("pole_diagnostics.prefer_regular", pr, &["max_size_ratio"])?;
                if let Some(m) = pr.get("max_size_ratio") {
                    p.prefer.max_size_ratio = MaxSizeRatio::parse(m)?;
                }
            }
        }
        if let Some(rp) = repair_policy {
            let m = rp
                .as_object()
                .ok_or_else(|| PolicyError("repair_policy is not an object".into()))?;
            if let Some(v) = m.get("prefer_regular_at") {
                let o = Obj {
                    at: "repair_policy",
                    m,
                };
                o.strings("prefer_regular_at", None, &mut p.prefer_regular_at)?;
                let _ = v;
                p.prefer_regular_at =
                    dimensions(&p.prefer_regular_at, "repair_policy.prefer_regular_at")?;
            }
            if let Some(v) = m.get("prefer_regular") {
                let o = Obj::new(
                    "repair_policy.prefer_regular",
                    v,
                    &[
                        "criteria_order",
                        "max_size_ratio",
                        "max_further_steps",
                        "probe",
                        "solve",
                        "max_keys_per_node",
                        "max_cpu_h_per_node",
                        "revert_if_open",
                        "terminal_set",
                    ],
                )?;
                let mut co = String::new();
                o.choice(
                    "criteria_order",
                    &["regular_first", "economy_first"],
                    &mut co,
                )?;
                match co.as_str() {
                    "economy_first" => p.prefer.criteria_order = CriteriaOrder::EconomyFirst,
                    "regular_first" => p.prefer.criteria_order = CriteriaOrder::RegularFirst,
                    _ => {}
                }
                if let Some(m) = o.get("max_size_ratio") {
                    let r = MaxSizeRatio::parse(m)?;
                    if pole_diagnostics
                        .and_then(|pd| pd.get("prefer_regular"))
                        .and_then(|pr| pr.get("max_size_ratio"))
                        .is_some_and(|other| MaxSizeRatio::parse(other).ok() != Some(r))
                    {
                        return err(
                            "max_size_ratio differs between pole_diagnostics.prefer_regular and repair_policy.prefer_regular",
                        );
                    }
                    p.prefer.max_size_ratio = r;
                }
                o.uint("max_further_steps", &mut p.prefer.max_further_steps)?;
                if let Some(pr) = o.get("probe") {
                    let pr = Obj::new(
                        "repair_policy.prefer_regular.probe",
                        pr,
                        &["key_budget_s", "primes"],
                    )?;
                    pr.uint("key_budget_s", &mut p.prefer.probe_key_budget_s)?;
                    pr.uint("primes", &mut p.prefer.probe_primes)?;
                    if p.prefer.probe_primes < 2 {
                        return err("repair_policy.prefer_regular.probe.primes must be >= 2");
                    }
                }
                if let Some(s) = o.get("solve") {
                    let s = Obj::new("repair_policy.prefer_regular.solve", s, &["key_budget_s"])?;
                    s.uint("key_budget_s", &mut p.prefer.solve_key_budget_s)?;
                }
                o.uint("max_keys_per_node", &mut p.prefer.max_keys_per_node)?;
                o.num("max_cpu_h_per_node", &mut p.prefer.max_cpu_h_per_node)?;
                o.bool("revert_if_open", &mut p.prefer.revert_if_open)?;
                if let Some(t) = o.get("terminal_set") {
                    let t = Obj::new(
                        "repair_policy.prefer_regular.terminal_set",
                        t,
                        &[
                            "enabled",
                            "max_candidates",
                            "max_rotations",
                            "export",
                            "confirm_engine",
                            "budget_cpu_h",
                        ],
                    )?;
                    let ts = &mut p.prefer.terminal_set;
                    t.bool("enabled", &mut ts.enabled)?;
                    t.uint("max_candidates", &mut ts.max_candidates)?;
                    match t.get("max_rotations") {
                        None => {}
                        Some(Value::String(s)) if s == "deficiency" => ts.max_rotations = None,
                        Some(v) => {
                            ts.max_rotations = Some(v.as_u64().ok_or_else(|| {
                                PolicyError(format!(
                                    "repair_policy.prefer_regular.terminal_set.max_rotations = {v} is not \"deficiency\" or an unsigned integer"
                                ))
                            })?)
                        }
                    }
                    t.choice("export", &["regular", "found"], &mut ts.export)?;
                    t.bool("confirm_engine", &mut ts.confirm_engine)?;
                    t.num("budget_cpu_h", &mut ts.budget_cpu_h)?;
                }
            }
        }
        Ok(p)
    }

    /// The preference is on (some d0 listed in `prefer_regular_at`).
    pub fn prefer_on(&self) -> bool {
        !self.prefer_regular_at.is_empty()
    }

    /// The policy as canonical JSON (every value in force, defaults filled in): what a report names and its digest binds.
    pub fn policy_json(&self) -> Value {
        let p = &self.prefer;
        serde_json::json!({
            "prefer_regular_at": self.prefer_regular_at,
            "criteria_order": match p.criteria_order { CriteriaOrder::RegularFirst => "regular_first", CriteriaOrder::EconomyFirst => "economy_first" },
            "d4_rule_version": 2,
            "max_size_ratio": p.max_size_ratio.text(),
            "max_further_steps": p.max_further_steps,
            "probe": {"key_budget_s": p.probe_key_budget_s, "primes": p.probe_primes},
            "solve": {"key_budget_s": p.solve_key_budget_s},
            "max_keys_per_node": p.max_keys_per_node,
            "max_cpu_h_per_node": format!("{}", p.max_cpu_h_per_node),
            "revert_if_open": p.revert_if_open,
            "terminal_set": {"enabled": p.terminal_set.enabled, "max_candidates": p.terminal_set.max_candidates,
                "max_rotations": p.terminal_set.max_rotations.map_or(Value::from("deficiency"), Value::from),
                "export": p.terminal_set.export, "confirm_engine": p.terminal_set.confirm_engine,
                "budget_cpu_h": format!("{}", p.terminal_set.budget_cpu_h)},
            "dimensions": self.dimensions,
        })
    }

    /// sha256 of the canonical policy JSON (the `policy_digest` of every choice record).
    pub fn policy_digest(&self) -> String {
        let t = crate::schema::canon::canonical_json(&self.policy_json(), false)
            .expect("policy JSON holds no floats");
        crate::replay_base::sha256::sha256_hex(t.as_bytes())
    }
}
