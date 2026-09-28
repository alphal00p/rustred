//! `rustred walk-rescue-plan`: offline frontier classification and rescue
//! amendment generation for a stopped (or finished) checkpointed walk.
//!
//! Reads one CP5 generation raw (no owner import, no restore validators),
//! classifies every frontier-bearing native, computes the frontier taint
//! (reverse reachability over recorded edges) and decides, per physics query
//! (a query id NOT containing the helper pattern), whether some input root
//! outside the taint still contains it. For every blocked physics query of a
//! KNOWN frontier class it proposes a bounded helper for the query's owner
//! and writes the next digest-chained amendment (`rescue.rs`).
//!
//! Frontier catalogue (legacy engine; see the W1 rescue note):
//! - `unbounded_rank_guard`: `local_dispatch_frontier` with an `Unresolved`
//!   guard predicate on a node whose numerator rank is unbounded. Known
//!   rescue: a rank-bounded helper (four-loop R-free helpers: 85-556
//!   frontiers; the same controls with R <= 12 helpers: 0).
//! - `unbounded_positive_power_guard`: the same on a rank-bounded node with
//!   unbounded positive power A. Known rescue: an A-bounded helper (five-loop
//!   interim unbounded-A helpers: 1,299 frontiers at ranks 4-7 in 6 guard
//!   owners; plan-v3 A-bounded guard helpers: 0 frontiers through gen 7).
//! - everything else (a guard obstruction on a fully bounded node, an exact
//!   gap, an invalid source condition, an RHS obligation, a routing or route
//!   source-validity frontier, an input frontier): no known rescue; the
//!   owner decides (verdict `unknown_frontier_class`).
//!
//! The rescue helper for owner X is, in order: the first helper of X in the
//! optional rescue-helper query document that contains the blocked query and
//! is bounded where the observed classes require it; else the hull of X's
//! physics queries (lower = minimum, uppers open, rank = maximum rank, A =
//! maximum A when an A-class frontier was seen, no D bounds); else a copy of
//! the blocked query itself. A helper domain re-added `max_repeats` times
//! (its cone keeps reaching new frontiers) ends the automatic rescue.
use super::super::{input, matching};
use super::{
    OwnerDomainWalkRequest, checkpoint, mask,
    queue::{Domain, Phase},
    rescue,
};
use crate::AppError;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

const FLAG_SEALED: u8 = 1;
const FLAG_INSPECTED: u8 = 2;
pub const OWNER_DOMAIN_WALK_RESCUE_PLAN_SCHEMA: &str = "rustred.walk-rescue-plan.json.v1";
const MAX_EXAMPLES: usize = 20;

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkRescuePlanOptions {
    pub checkpoint: PathBuf,
    /// Query ids containing this substring are helpers; every other query is
    /// a physics query whose certification the rescue must preserve.
    pub helper_pattern: String,
    /// Optional query document of preferred rescue helpers (e.g. plan-v3).
    pub rescue_helpers_json: Option<String>,
    /// A helper domain may be added by at most this many amendments.
    pub max_repeats: usize,
}

impl OwnerDomainWalkRescuePlanOptions {
    pub fn new(checkpoint: impl Into<PathBuf>) -> Self {
        Self {
            checkpoint: checkpoint.into(),
            helper_pattern: "anchor".into(),
            rescue_helpers_json: None,
            max_repeats: 3,
        }
    }
}

/// The plan document and, for verdict `rescue`, the exact bytes of the next
/// amendment (its digest is the blake3 of these bytes).
pub struct OwnerDomainWalkRescuePlan {
    pub plan: Value,
    pub amendment: Option<String>,
}

pub fn owner_domain_walk_rescue_plan(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkRescuePlanOptions,
) -> Result<OwnerDomainWalkRescuePlan, AppError> {
    let (_, arity, _) = input::Selection::parse(&request.matching.selection_json)?;
    macro_rules! dispatch { ($($n:literal),*) => { match arity {
        $($n => plan::<$n>(request, options),)*
        _ => Err(AppError::input("unsupported owner arity")),
    }} }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Class {
    UnboundedRank,
    UnboundedPositivePower,
    Unknown,
}

impl Class {
    fn name(self) -> &'static str {
        match self {
            Self::UnboundedRank => "unbounded_rank_guard",
            Self::UnboundedPositivePower => "unbounded_positive_power_guard",
            Self::Unknown => "unknown",
        }
    }
}

fn query_domain<const N: usize>(query: &matching::input::Query) -> Option<Domain<N>> {
    Some(Domain {
        phase: Phase::Apply,
        owner: query.owner.as_slice().try_into().ok()?,
        lower: query.lower.clone(),
        upper: query.upper.clone(),
        rank: query.rank,
        powers: query.powers,
    })
}

fn helper_id(pattern: &str, sequence: u64, body: &str, taken: &mut BTreeSet<String>) -> String {
    let separator = if pattern.is_empty() || pattern.ends_with('-') {
        ""
    } else {
        "-"
    };
    let mut base = format!("{pattern}{separator}rescue{sequence}-{body}");
    if base.len() > 120 {
        let digest = blake3::hash(base.as_bytes()).to_hex();
        base = format!("{pattern}{separator}rescue{sequence}-{}", &digest[..32]);
    }
    let mut id = base.clone();
    let mut suffix = 2;
    while taken.contains(&id) {
        id = format!("{base}-{suffix}");
        suffix += 1;
    }
    taken.insert(id.clone());
    id
}

fn plan<const N: usize>(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkRescuePlanOptions,
) -> Result<OwnerDomainWalkRescuePlan, AppError> {
    let input_error = |e: String| AppError::input(e);
    let queries = matching::input::parse(
        &request.matching.queries_json,
        N,
        request.matching.max_queries,
        request.matching.max_query_bytes,
    )?;
    let amendments = request
        .amendments
        .iter()
        .map(|a| rescue::parse(a, N))
        .collect::<Result<Vec<_>, _>>()
        .map_err(input_error)?;
    let raw = checkpoint::read_raw::<N>(&options.checkpoint).map_err(input_error)?;
    if checkpoint::request_binding(request) != raw.request {
        return Err(AppError::input(
            "checkpoint request digest differs from the command's request/queries binding",
        ));
    }
    let original: Vec<&str> = queries.iter().map(|q| q.id.as_str()).collect();
    rescue::check_chain(&raw.amendments, &amendments, &raw.request, &original)
        .map_err(input_error)?;
    if amendments.len() != raw.amendments.len() {
        return Err(AppError::input(
            "the planner reads the checkpoint's recorded amendment chain exactly; supply every recorded amendment and no new one",
        ));
    }
    let helpers = match &options.rescue_helpers_json {
        Some(text) => matching::input::parse(text, N, 1 << 20, 64 << 20)?,
        None => Vec::new(),
    };
    let total = raw.domains.len();
    let domain = |id: usize| raw.domains[id].expand();
    // Frontier-bearing natives: inspected and unsealed.
    let frontier_nodes: Vec<usize> = (0..total)
        .filter(|&id| {
            let flag = raw.flags.get(id).copied().unwrap_or(0);
            flag & FLAG_INSPECTED != 0 && flag & FLAG_SEALED == 0
        })
        .collect();
    let wanted: BTreeSet<usize> = frontier_nodes.iter().copied().collect();
    // Their frontier records (kind and disposition of every entry).
    let mut entries: BTreeMap<usize, Vec<(String, String)>> = BTreeMap::new();
    for (path, _) in &raw.records {
        let file = std::fs::File::open(path)
            .map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
        for line in BufReader::with_capacity(1 << 20, file).lines() {
            let line = line.map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
            if !line.contains("\"frontiers\":[{") {
                continue;
            }
            let record: Value = serde_json::from_str(&line)
                .map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
            let Some(id) = record["id"].as_u64().map(|id| id as usize) else {
                continue;
            };
            if !wanted.contains(&id) {
                continue;
            }
            let list = record["frontiers"]
                .as_array()
                .map(|frontiers| {
                    frontiers
                        .iter()
                        .map(|f| {
                            (
                                f["kind"].as_str().unwrap_or("").to_owned(),
                                f["disposition"].as_str().unwrap_or("").to_owned(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            entries.insert(id, list);
        }
    }
    let mut classes = BTreeMap::<&str, usize>::new();
    let mut node_classes = BTreeMap::<usize, Class>::new();
    let mut examples = Vec::new();
    let mut frontier_records = 0usize;
    for &id in &frontier_nodes {
        let d = domain(id);
        let list = entries.get(&id).cloned().unwrap_or_default();
        frontier_records += list.len();
        let known = !list.is_empty()
            && list.iter().all(|(kind, disposition)| {
                kind == "local_dispatch_frontier" && disposition.starts_with("Unresolved")
            });
        let class = if !known {
            Class::Unknown
        } else if d.rank.is_none() {
            Class::UnboundedRank
        } else if d.powers.max_positive_power.is_none() {
            Class::UnboundedPositivePower
        } else {
            Class::Unknown
        };
        *classes.entry(class.name()).or_default() += 1;
        node_classes.insert(id, class);
        if class == Class::Unknown && examples.len() < MAX_EXAMPLES {
            examples.push(json!({"id":id,"phase":format!("{:?}", d.phase),"owner":mask(&d.owner),
                "rank":d.rank,"power_bounds":matching::input::power_bounds_json(d.powers),
                "frontier_kinds":list.iter().map(|(k, _)| k.as_str()).collect::<BTreeSet<_>>(),
                "dispositions":list.iter().take(3).map(|(_, d)| d.chars().take(160).collect::<String>()).collect::<Vec<_>>(),
                "reason":if !known {"frontier kind or disposition without a known rescue"}
                    else {"guard obstruction on a node bounded in rank and positive power"}}));
        }
    }
    for frontier in &raw.input_frontiers {
        *classes.entry("input_frontier").or_default() += 1;
        if examples.len() < MAX_EXAMPLES {
            examples.push(
                json!({"input_frontier":frontier,"reason":"input frontier (no known rescue)"}),
            );
        }
    }
    // Frontier taint: reverse reachability over recorded edges.
    let mut offsets = vec![0u32; total + 1];
    for &(_, target) in &raw.edges {
        if let Some(slot) = offsets.get_mut(target as usize + 1) {
            *slot += 1;
        }
    }
    for index in 1..offsets.len() {
        offsets[index] += offsets[index - 1];
    }
    let mut fill = offsets.clone();
    let mut sources = vec![0u32; raw.edges.len()];
    for &(source, target) in &raw.edges {
        if (target as usize) < total {
            let slot = &mut fill[target as usize];
            sources[*slot as usize] = source;
            *slot += 1;
        }
    }
    let mut tainted = vec![false; total];
    let mut stack: Vec<usize> = frontier_nodes.clone();
    for &id in &frontier_nodes {
        tainted[id] = true;
    }
    while let Some(id) = stack.pop() {
        for &source in &sources[offsets[id] as usize..offsets[id + 1] as usize] {
            let source = source as usize;
            if source < total && !tainted[source] {
                tainted[source] = true;
                stack.push(source);
            }
        }
    }
    let tainted_count = tainted.iter().filter(|&&t| t).count();
    // Queries (original, then amended) with their roots.
    let all: Vec<&matching::input::Query> = queries
        .iter()
        .chain(amendments.iter().flat_map(|a| a.queries.iter()))
        .collect();
    let roots: Vec<Option<usize>> = (0..all.len())
        .map(|index| {
            raw.inputs
                .get(index)
                .and_then(|input| input["domain"].as_u64())
                .map(|id| id as usize)
                .filter(|&id| id < total)
        })
        .collect();
    let is_helper = |id: &str| id.contains(options.helper_pattern.as_str());
    let root_domains: Vec<Option<Domain<N>>> = roots.iter().map(|root| root.map(&domain)).collect();
    let mut blocked = Vec::new();
    let mut physics_total = 0usize;
    for (index, query) in all.iter().enumerate() {
        if is_helper(&query.id) {
            continue;
        }
        physics_total += 1;
        let Some(probe) = query_domain::<N>(query) else {
            continue;
        };
        let covered = roots.iter().zip(&root_domains).any(|(root, root_domain)| {
            let (Some(root), Some(root_domain)) = (root, root_domain) else {
                return false;
            };
            let mut probe = probe.clone();
            probe.phase = root_domain.phase;
            !tainted[*root] && rescue::contains(root_domain, &probe)
        });
        if !covered {
            blocked.push(index);
        }
    }
    let helper_roots: BTreeSet<usize> = all
        .iter()
        .zip(&roots)
        .filter(|(q, _)| is_helper(&q.id))
        .filter_map(|(_, root)| *root)
        .collect();
    let helper_roots_tainted: Vec<usize> = helper_roots
        .iter()
        .copied()
        .filter(|&r| tainted[r])
        .collect();
    let sequence = raw.amendments.len() as u64 + 1;
    let parent = raw
        .amendments
        .last()
        .map_or(raw.request.clone(), |a| a.digest.clone());
    let unknown = classes.get("unknown").copied().unwrap_or(0) + raw.input_frontiers.len();
    let need_power = node_classes
        .values()
        .any(|&class| class == Class::UnboundedPositivePower);
    let mut plan = json!({"schema":OWNER_DOMAIN_WALK_RESCUE_PLAN_SCHEMA,
        "checkpoint":{"directory":options.checkpoint,"generation":raw.generation,
            "request_digest":raw.request,"amendments":raw.amendments.len(),
            "walk_semantics_version":raw.walk_semantics_version},
        "helper_pattern":options.helper_pattern,
        "frontier_nodes":frontier_nodes.len(),"frontier_records":frontier_records,
        "input_frontiers":raw.input_frontiers.len(),
        "classes":classes,"unknown_examples":examples,
        "tainted_nodes":tainted_count,"helper_roots":helper_roots.len(),
        "helper_roots_tainted":helper_roots_tainted.len(),
        "helper_roots_tainted_ids":helper_roots_tainted.iter().take(1000).collect::<Vec<_>>(),
        "physics_queries":physics_total,
        "physics_blocked":blocked.iter().map(|&i| all[i].id.as_str()).collect::<Vec<_>>(),
        "rescue_level":{"finite_rank":true,"finite_positive_power":need_power},
        "known_rescue_classes":{
            "unbounded_rank_guard":"local_dispatch_frontier Unresolved on a rank-unbounded node: rescue with a rank-bounded helper of the owner",
            "unbounded_positive_power_guard":"local_dispatch_frontier Unresolved on a rank-bounded, A-unbounded node: rescue with a rank- and A-bounded helper of the owner"},
        "family_closure_claim":false});
    if frontier_nodes.is_empty() && raw.input_frontiers.is_empty() {
        plan["verdict"] = json!("no_frontier");
        plan["reason"] =
            json!("the checkpoint holds no frontier-bearing native and no input frontier");
        return Ok(OwnerDomainWalkRescuePlan {
            plan,
            amendment: None,
        });
    }
    if unknown > 0 {
        plan["verdict"] = json!("unknown_frontier_class");
        plan["reason"] = json!(format!(
            "{unknown} frontier(s) of a class without a known rescue; the campaign waits for the owner (see unknown_examples)"
        ));
        return Ok(OwnerDomainWalkRescuePlan {
            plan,
            amendment: None,
        });
    }
    if blocked.is_empty() {
        plan["verdict"] = json!("no_amendment_needed");
        plan["reason"] = json!(
            "every physics query still has an input root outside the frontier taint; resume without a new amendment"
        );
        return Ok(OwnerDomainWalkRescuePlan {
            plan,
            amendment: None,
        });
    }
    // Rescue helpers per blocked physics query.
    let mut taken: BTreeSet<String> = all.iter().map(|q| q.id.clone()).collect();
    let previous: Vec<Domain<N>> = amendments
        .iter()
        .flat_map(|a| a.queries.iter())
        .filter_map(query_domain::<N>)
        .collect();
    let bounded =
        |d: &Domain<N>| d.rank.is_some() && (!need_power || d.powers.max_positive_power.is_some());
    let mut chosen: Vec<(Domain<N>, &'static str, String, Vec<String>)> = Vec::new();
    for &index in &blocked {
        let query = all[index];
        let Some(target) = query_domain::<N>(query) else {
            continue;
        };
        let from_file = helpers
            .iter()
            .filter_map(query_domain::<N>)
            .find(|h| h.owner == target.owner && bounded(h) && rescue::contains(h, &target));
        let hull = || {
            let physics: Vec<Domain<N>> = all
                .iter()
                .filter(|q| !is_helper(&q.id))
                .filter_map(|q| query_domain::<N>(q))
                .filter(|d| d.owner == target.owner)
                .collect();
            let rank = physics
                .iter()
                .map(|d| d.rank)
                .try_fold(0u32, |a, r| r.map(|r| a.max(r)))?;
            let power = if need_power {
                Some(
                    physics
                        .iter()
                        .map(|d| d.powers.max_positive_power)
                        .try_fold(0u64, |a, p| p.map(|p| a.max(p)))?,
                )
            } else {
                None
            };
            let lower = (0..N)
                .map(|axis| physics.iter().map(|d| d.lower[axis]).min().unwrap_or(0))
                .collect();
            let helper = Domain {
                phase: Phase::Apply,
                owner: target.owner,
                lower,
                upper: vec![None; N],
                rank: Some(rank),
                powers: rustred::solver::DomainPowerBounds {
                    max_positive_power: power,
                    min_power_difference: None,
                    max_power_difference: None,
                },
            };
            rescue::contains(&helper, &target).then_some(helper)
        };
        let (helper, source) = match from_file {
            Some(helper) => (helper, "rescue_helpers_file"),
            None => match hull() {
                Some(helper) => (helper, "physics_hull"),
                None => (target.clone(), "physics_query_copy"),
            },
        };
        if let Some(entry) = chosen.iter_mut().find(|entry| entry.0 == helper) {
            entry.3.push(query.id.clone());
            continue;
        }
        let repeats = previous.iter().filter(|d| **d == helper).count();
        if repeats >= options.max_repeats {
            plan["verdict"] = json!("rescue_exhausted");
            plan["reason"] = json!(format!(
                "the rescue helper for owner {} (covering {}) was already added {repeats} time(s) and its cone keeps reaching frontiers; the campaign waits for the owner",
                mask(&helper.owner),
                query.id
            ));
            return Ok(OwnerDomainWalkRescuePlan {
                plan,
                amendment: None,
            });
        }
        let body = match source {
            "physics_query_copy" => format!("copy-{}", query.id),
            _ => format!(
                "r{}-a{}-{}",
                helper.rank.map_or("none".into(), |r| r.to_string()),
                helper
                    .powers
                    .max_positive_power
                    .map_or("none".into(), |p| p.to_string()),
                mask(&helper.owner)
            ),
        };
        let id = helper_id(&options.helper_pattern, sequence, &body, &mut taken);
        chosen.push((helper, source, id, vec![query.id.clone()]));
    }
    let rows: Vec<Value> = chosen
        .iter()
        .map(|(helper, _, id, _)| rescue::query_row(id, helper))
        .collect();
    let covers: BTreeMap<&str, &Vec<String>> = chosen
        .iter()
        .map(|(_, _, id, covered)| (id.as_str(), covered))
        .collect();
    let sources: BTreeMap<&str, &str> = chosen
        .iter()
        .map(|(_, source, id, _)| (id.as_str(), *source))
        .collect();
    let amendment = json!({"schema":rescue::AMENDMENT_SCHEMA,"sequence":sequence,"parent":parent,
        "queries":rows,
        "provenance":{"generator":"rustred walk-rescue-plan","plan_schema":OWNER_DOMAIN_WALK_RESCUE_PLAN_SCHEMA,
            "checkpoint_generation":raw.generation,"classes":plan["classes"].clone(),
            "frontier_nodes":frontier_nodes.len(),"helper_pattern":options.helper_pattern,
            "finite_positive_power":need_power,"covers":covers,"helper_sources":sources,
            "family_closure_claim":false}});
    let mut text =
        serde_json::to_string_pretty(&amendment).map_err(|e| AppError::input(e.to_string()))?;
    text.push('\n');
    let digest = blake3::hash(text.as_bytes()).to_hex().to_string();
    plan["verdict"] = json!("rescue");
    plan["reason"] = json!(format!(
        "{} physics quer{} blocked by frontier taint of known class; {} rescue helper(s) proposed",
        blocked.len(),
        if blocked.len() == 1 { "y" } else { "ies" },
        chosen.len()
    ));
    plan["amendment"] = json!({"sequence":sequence,"parent":parent,"digest":digest,
        "queries":rows.len(),"helpers":chosen.iter().map(|(helper, source, id, covered)| json!({
            "id":id,"owner":mask(&helper.owner),"rank":helper.rank,
            "max_positive_power":helper.powers.max_positive_power,"source":source,"covers":covered})).collect::<Vec<_>>()});
    Ok(OwnerDomainWalkRescuePlan {
        plan,
        amendment: Some(text),
    })
}
