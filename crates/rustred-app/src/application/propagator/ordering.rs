//! p artefact v2: the explicit rule ordering of a p library (owner: M5
//! m5-1l3l; spec notes/artifact-ordering.SPEC.md items 1-3, 5).
//!
//! A package MAY carry `ordering.json` (schema [`P_ORDERING_SCHEMA`], version
//! [`P_ORDERING_VERSION`]), listed as `files.ordering` in `library.json`. It
//! holds, per owner of the library (in class order):
//! - the dispatch policy: `first-valid` (a Custom owner: its rules are tried
//!   in the listed order and the first applicable one is applied),
//!   `first-valid+whole-piece-alternatives` (the payload also has rules that
//!   are dispatched only after a baseline rule selected a whole piece, as the
//!   generated schema-v3 dispatch table says), or `template` (a closed-form
//!   lane without a rule table);
//! - for a Custom owner, per chart, the payload's rule indices in application
//!   order (a permutation; two rules with the same case are two entries);
//! - per rule, metadata precomputed at generation: RHS terms, the largest
//!   numerator-rank change of a term, lines raised and lowered, exception
//!   branches, coefficient sizes, whether `d` occurs in a coefficient
//!   denominator (a possible eps pole), seed rows (provenance) and the
//!   heuristic's score;
//! - the ordering provenance: heuristic name and parameters, and an optional
//!   measured census (keys, edges, time, RSS on reference cells).
//!
//! The loader hands the orders to `PLibraryBuilder::set_ordering`; the engine
//! then applies exactly that order (`custom::build_custom_step_ordered`), and
//! the library identity, so also the step-cache identity, gains
//! `,P_ORDERING_VERSION=<version>:<heuristic>`: caches recorded under another
//! ordering are never reused. A package without `ordering.json` keeps its
//! identity and the payload order (the generation order), so every older
//! package stays loadable.
//!
//! [`write_ordering`] adds an ordering to a written package (in place).
use std::collections::BTreeMap;
use std::path::Path;

use rustred::propagator::custom::{CustomPower, CustomRule, PRuleOrder, custom_payload_rules};
use rustred::propagator::error::PError;
use rustred::propagator::{OwnerClassId, P_ORDERING_SCHEMA, P_ORDERING_VERSION};
use serde::{Deserialize, Serialize};

use super::format::{LIBRARY_FILE, LibraryManifest, malformed, owner_file};

/// File name of the ordering inside a package.
pub const ORDERING_FILE: &str = "ordering.json";
/// File name of the saved route verifications inside a package.
pub const ROUTES_FILE: &str = "routes.rrbin";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderingFile {
    pub schema: String,
    pub version: u32,
    pub heuristic: String,
    pub parameters: BTreeMap<String, String>,
    pub owners: Vec<OwnerOrdering>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerOrdering {
    /// Class label `L<l>T<t>-<id hex>`.
    pub class: String,
    pub lane: String,
    pub dispatch: String,
    /// Per chart, the payload's rule indices in application order.
    pub charts: Vec<ChartOrder>,
    /// Per rule, payload order (chart by chart).
    pub rules: Vec<RuleMeta>,
    pub provenance: OrderingProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartOrder {
    pub chart: u64,
    pub order: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleMeta {
    pub chart: u64,
    /// Index of the rule in its chart in the payload (the generation order).
    pub index: u32,
    pub affine: bool,
    /// Fixed axes of the rule's case and their values.
    pub fixed: Vec<(usize, i64)>,
    pub rhs_terms: usize,
    /// Largest numerator-rank change of one RHS term against the target
    /// (positive = the term has more numerator powers).
    pub max_rank_change: i64,
    /// RHS terms that raise the numerator rank.
    pub rank_raising_terms: usize,
    /// Slots (bit masks) whose line power a term raises / lowers.
    pub raised_lines: u64,
    pub lowered_lines: u64,
    /// RHS terms with a raised line power.
    pub raising_terms: usize,
    pub exceptions: usize,
    pub coefficient_bytes_max: usize,
    pub coefficient_bytes_total: usize,
    /// `d` occurs in a coefficient denominator (a possible eps pole).
    pub d_in_denominator: bool,
    pub dispatched_after_baseline: bool,
    /// Provenance: seed rows (IBP row x seed) the rule was solved from.
    pub sources: usize,
    /// The heuristic's sort key (smaller first); empty for `generation`.
    pub score: Vec<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderingProvenance {
    pub heuristic: String,
    pub parameters: BTreeMap<String, String>,
    /// Measured census of this ordering on reference cells, when recorded
    /// (`keys`, `edges`, `seconds`, `peak_rss_kb`, per cell), else null.
    pub census: Option<serde_json::Value>,
}

/// Ordering heuristics this build can write.
pub const ORDERING_HEURISTICS: [&str; 2] = ["generation", "fewest-terms"];

fn power_offset(power: &CustomPower) -> (bool, i64) {
    match *power {
        CustomPower::Symbolic(offset) => (true, offset),
        CustomPower::Fixed(value) => (false, value),
    }
}

/// The metadata of one rule of a chart (line slots = the chart's bits).
pub fn rule_meta(rule: &CustomRule) -> RuleMeta {
    let mut meta = RuleMeta {
        chart: rule.chart,
        index: u32::try_from(rule.ordinal).unwrap_or(u32::MAX),
        affine: rule.affine,
        fixed: rule.fixed.clone(),
        rhs_terms: rule.rhs.len(),
        max_rank_change: i64::MIN,
        rank_raising_terms: 0,
        raised_lines: 0,
        lowered_lines: 0,
        raising_terms: 0,
        exceptions: rule.exclusions.len(),
        coefficient_bytes_max: 0,
        coefficient_bytes_total: 0,
        d_in_denominator: false,
        dispatched_after_baseline: rule.dispatched,
        sources: rule.sources,
        score: Vec::new(),
    };
    for (powers, coefficient) in &rule.rhs {
        let mut rank_change = 0_i64;
        let mut raises = false;
        for (slot, (target, term)) in rule.target.iter().zip(powers).enumerate() {
            let (_, t) = power_offset(target);
            let (_, u) = power_offset(term);
            let delta = u - t;
            if rule.chart >> slot & 1 == 1 {
                if delta > 0 {
                    meta.raised_lines |= 1 << slot;
                    raises = true;
                } else if delta < 0 {
                    meta.lowered_lines |= 1 << slot;
                }
            } else {
                // A numerator slot: a more negative power is a higher rank.
                rank_change -= delta;
            }
        }
        meta.max_rank_change = meta.max_rank_change.max(rank_change);
        meta.rank_raising_terms += usize::from(rank_change > 0);
        meta.raising_terms += usize::from(raises);
        meta.coefficient_bytes_max = meta.coefficient_bytes_max.max(coefficient.len());
        meta.coefficient_bytes_total += coefficient.len();
        if let Some((_, denominator)) = coefficient.split_once(")/(") {
            meta.d_in_denominator |= denominator
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .any(|word| word == "d");
        }
    }
    if rule.rhs.is_empty() {
        meta.max_rank_change = 0;
    }
    meta
}

/// The application order of one chart's rules under `heuristic`.
fn chart_order(heuristic: &str, rules: &mut [RuleMeta]) -> Result<Vec<u32>, PError> {
    match heuristic {
        "generation" => Ok(rules.iter().map(|rule| rule.index).collect()),
        // The Forcer author's criteria (user 2026-11-10T20:51Z): fewest RHS
        // terms, then fewest raising terms, then the least numerator raising;
        // ties keep the generation order. FirstValid over this order applies,
        // at every key, the valid rule with the fewest terms (A3's dispatch).
        "fewest-terms" => {
            for rule in rules.iter_mut() {
                rule.score = vec![
                    rule.rhs_terms as i64,
                    rule.raising_terms as i64,
                    rule.rank_raising_terms as i64,
                    rule.max_rank_change.max(0),
                ];
            }
            let mut sorted: Vec<&RuleMeta> = rules.iter().collect();
            sorted.sort_by(|a, b| a.score.cmp(&b.score).then(a.index.cmp(&b.index)));
            Ok(sorted.iter().map(|rule| rule.index).collect())
        }
        other => Err(malformed(format!(
            "unknown ordering heuristic {other:?} (known: {ORDERING_HEURISTICS:?})"
        ))),
    }
}

/// The ordering of a package under `heuristic`, computed from its payloads.
pub fn compute_ordering(directory: &Path, heuristic: &str) -> Result<OrderingFile, PError> {
    let read = |name: &str| {
        std::fs::read(directory.join(name))
            .map_err(|error| malformed(format!("{}: {error}", directory.join(name).display())))
    };
    let manifest: LibraryManifest = serde_json::from_slice(&read(LIBRARY_FILE)?)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    let mut owners = Vec::with_capacity(manifest.owners.len());
    let parameters = BTreeMap::new();
    for record in &manifest.owners {
        let lane = serde_json::to_value(record.lane)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("{:?}", record.lane));
        let mut owner = OwnerOrdering {
            class: record.class.clone(),
            lane,
            dispatch: "template".to_owned(),
            charts: Vec::new(),
            rules: Vec::new(),
            provenance: OrderingProvenance {
                heuristic: heuristic.to_owned(),
                parameters: parameters.clone(),
                census: None,
            },
        };
        if let Some(payload) = &record.payload {
            let expected = owner_file(&record.class)?;
            if payload.file != expected {
                return Err(malformed(format!(
                    "the payload of {} is {}, not {expected}",
                    record.class, payload.file
                )));
            }
            let rules = custom_payload_rules(&read(&payload.file)?)?;
            let mut by_chart: BTreeMap<u64, Vec<RuleMeta>> = BTreeMap::new();
            for rule in &rules {
                by_chart
                    .entry(rule.chart)
                    .or_default()
                    .push(rule_meta(rule));
            }
            let dispatched = rules.iter().any(|rule| rule.dispatched);
            owner.dispatch = if dispatched {
                "first-valid+whole-piece-alternatives"
            } else {
                "first-valid"
            }
            .to_owned();
            for (chart, mut metas) in by_chart {
                let order = chart_order(heuristic, &mut metas)?;
                owner.charts.push(ChartOrder { chart, order });
                owner.rules.extend(metas);
            }
        }
        owners.push(owner);
    }
    if !ORDERING_HEURISTICS.contains(&heuristic) {
        return Err(malformed(format!(
            "unknown ordering heuristic {heuristic:?}"
        )));
    }
    Ok(OrderingFile {
        schema: P_ORDERING_SCHEMA.to_owned(),
        version: P_ORDERING_VERSION,
        heuristic: heuristic.to_owned(),
        parameters,
        owners,
    })
}

/// The identity of a library with the ordering tag of `heuristic`
/// (`PLibrary::identity`): `base` must be an identity without one.
pub fn ordered_identity(base: &str, heuristic: &str) -> String {
    format!("{base},P_ORDERING_VERSION={P_ORDERING_VERSION}:{heuristic}")
}

/// Add an ordering under `heuristic` to the package in `directory`, in place:
/// writes `ordering.json`, and in `library.json` lists it as `files.ordering`
/// and sets the identity of the ordered library. A package that already has an
/// ordering is refused (start from the unordered package).
pub fn write_ordering(directory: &Path, heuristic: &str) -> Result<OrderingFile, PError> {
    let path = directory.join(LIBRARY_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| malformed(format!("{}: {error}", path.display())))?;
    let mut manifest: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    if manifest["files"]
        .get("ordering")
        .is_some_and(|v| !v.is_null())
    {
        return Err(malformed(format!(
            "{} already has an ordering",
            directory.display()
        )));
    }
    let ordering = compute_ordering(directory, heuristic)?;
    let bytes = serde_json::to_vec_pretty(&ordering)
        .map_err(|error| malformed(format!("{ORDERING_FILE}: {error}")))?;
    std::fs::write(directory.join(ORDERING_FILE), bytes)
        .map_err(|error| malformed(format!("{ORDERING_FILE}: {error}")))?;
    let base = manifest["identity"]
        .as_str()
        .ok_or_else(|| malformed(format!("{LIBRARY_FILE}: no identity")))?
        .to_owned();
    manifest["identity"] = serde_json::Value::String(ordered_identity(&base, heuristic));
    manifest["files"]["ordering"] = serde_json::Value::String(ORDERING_FILE.to_owned());
    let mut out = serde_json::to_string_pretty(&manifest)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    out.push('\n');
    std::fs::write(&path, out).map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    Ok(ordering)
}

/// Parse an ordering file and resolve its owners to class ids: version and
/// schema first ([`PError::VersionMismatch`]), then every owner of the
/// library exactly once, in class order, with its lane; Custom owners
/// (`first-valid*`) give their rule orders. The orders are checked against
/// the payloads when an owner's program is built.
pub fn read_ordering(
    bytes: &[u8],
    manifest: &LibraryManifest,
    classes: &BTreeMap<String, OwnerClassId>,
) -> Result<(String, BTreeMap<OwnerClassId, PRuleOrder>), PError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| malformed(format!("{ORDERING_FILE}: {error}")))?;
    if value["schema"] != P_ORDERING_SCHEMA || value["version"] != P_ORDERING_VERSION {
        return Err(PError::VersionMismatch {
            name: "P_ORDERING_VERSION".to_owned(),
            found: format!("{}:{}", value["schema"], value["version"]),
            expected: format!("\"{P_ORDERING_SCHEMA}\":{P_ORDERING_VERSION}"),
        });
    }
    let file: OrderingFile = serde_json::from_value(value)
        .map_err(|error| malformed(format!("{ORDERING_FILE}: {error}")))?;
    if file.owners.len() != manifest.owners.len() {
        return Err(malformed(format!(
            "{ORDERING_FILE}: {} owners, the library has {}",
            file.owners.len(),
            manifest.owners.len()
        )));
    }
    let mut orders = BTreeMap::new();
    for (owner, record) in file.owners.iter().zip(&manifest.owners) {
        if owner.class != record.class {
            return Err(malformed(format!(
                "{ORDERING_FILE}: owner {} where the library lists {}",
                owner.class, record.class
            )));
        }
        let custom = record.payload.is_some();
        match (owner.dispatch.as_str(), custom) {
            ("first-valid" | "first-valid+whole-piece-alternatives", true) => {
                let class = *classes.get(&owner.class).ok_or_else(|| {
                    malformed(format!("{ORDERING_FILE}: unknown class {}", owner.class))
                })?;
                let mut order = PRuleOrder::default();
                for chart in &owner.charts {
                    if order
                        .charts
                        .insert(chart.chart, chart.order.clone())
                        .is_some()
                    {
                        return Err(malformed(format!(
                            "{ORDERING_FILE}: {} lists chart {:#x} twice",
                            owner.class, chart.chart
                        )));
                    }
                }
                orders.insert(class, order);
            }
            ("template", false) if owner.charts.is_empty() => {}
            (dispatch, _) => {
                return Err(malformed(format!(
                    "{ORDERING_FILE}: owner {} has dispatch {dispatch:?} and {} charts",
                    owner.class,
                    owner.charts.len()
                )));
            }
        }
    }
    Ok((file.heuristic, orders))
}

/// Record the route verification of every routed sector of the package in
/// `directory` (`routes.rrbin`, `rustred::propagator::saved_routes`) and list
/// it in `library.json`: `(sectors saved, sectors left to run-time
/// verification)`. Every map is proved here by `symmetry::verify`; only its
/// record is trusted when a package loads.
pub fn write_routes(directory: &Path) -> Result<(usize, usize), PError> {
    use super::{PLibraryOptions, PLibrarySource, load_p_library};
    use rustred::propagator::saved_routes::export_saved_routes;
    let library = load_p_library(
        PLibrarySource::Directory(directory.to_owned()),
        &PLibraryOptions {
            verify_descent: false,
        },
    )?;
    let (bytes, saved, skipped) = export_saved_routes(&library)?;
    std::fs::write(directory.join(ROUTES_FILE), bytes)
        .map_err(|error| malformed(format!("{ROUTES_FILE}: {error}")))?;
    let path = directory.join(LIBRARY_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| malformed(format!("{}: {error}", path.display())))?;
    let mut manifest: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    manifest["files"]["routes"] = serde_json::Value::String(ROUTES_FILE.to_owned());
    let mut out = serde_json::to_string_pretty(&manifest)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    out.push('\n');
    std::fs::write(&path, out).map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    Ok((saved, skipped))
}
