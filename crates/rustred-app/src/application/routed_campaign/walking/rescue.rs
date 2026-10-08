//! Resume-time frontier rescue on the saved-owner engine. Recognized guard
//! obstructions may trigger bounded append-only rescue attempts after a
//! checkpoint pause. Unknown failures and exhausted attempts stop explicitly;
//! no rescue outcome discards an already certified node or weakens scope.
//!
//! # Amendments
//!
//! An amendment is an append-only, digest-chained input extension applied at
//! `--resume` (`--amend-queries FILE`, repeatable, in chain order). Each file
//! carries its `sequence` (1, 2, ...), the `parent` digest (the checkpoint's
//! request binding for the first amendment, the previous amendment's blake3
//! file digest afterwards) and additional queries in the query-document row
//! format. The checkpoint manifest records the applied chain (`AmendmentRef`);
//! a resume must re-supply every recorded amendment byte for byte (digest
//! equality) and may append new ones that chain from the head. Nothing is
//! ever removed or rewritten: the original request binding, every admitted
//! domain, record, edge and closed node survive unchanged.
//!
//! # Quarantine
//!
//! At every resume of an amended walk the frontier taint (every node that
//! reaches a native that finished with frontiers) is recomputed from the
//! dependency monitor and installed as a lookup quarantine: no later
//! admission (exact, orthant, candidate index, helper preparation, initial
//! orthant shortcut or initial-overlap anchor) resolves into a tainted node,
//! so the amended queries and their whole cones stay out of frontier-blocked
//! cones. The taint only grows (frontier nodes never seal, edges are never
//! removed), so recomputation is idempotent. The quarantine is a scheduling
//! choice, never a certification input: closure is re-derived from the
//! recorded edges and seals exactly as before.
//!
//! # Certification per query
//!
//! A query is certified iff some input root (of an original or an amended
//! query) whose domain contains the query's domain is descendant-closed.
//! Required and auxiliary roles are an immutable exact-ID partition in the
//! original request. Rescue amendments add auxiliary queries; explicit scope
//! extensions append required queries without rewriting the original request.
//! No original or appended required query can be superseded. The audit and
//! `walk-verify-closure` independently
//! re-derive per-query containment and closure, never classifying by name.
//! `family_closure_claim` stays false.
use super::matching::input::{self as query_input, power_bounds_json};
use super::queue::{Domain, Phase};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

pub const AMENDMENT_SCHEMA: &str = "rustred.owner-domain-walk-amendment.json.v1";
/// Append-only enlargement of required scope, using the same chained resume
/// transaction as rescue but never reclassifying or superseding old queries.
pub const SCOPE_EXTENSION_SCHEMA: &str = "rustred.owner-domain-scope-extension.json.v1";
/// Bounds of one amendment file and of the chain.
pub const MAX_AMENDMENT_BYTES: usize = 16 << 20;
pub const MAX_AMENDMENT_QUERIES: usize = 65_536;
pub const MAX_AMENDMENTS: usize = 1_024;

/// One amendment file as supplied on the command line, in chain order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerDomainWalkAmendment {
    pub path: PathBuf,
    pub text: String,
}

impl OwnerDomainWalkAmendment {
    /// blake3 of the exact file bytes: the chain link of the next amendment.
    pub fn digest(&self) -> String {
        blake3::hash(self.text.as_bytes()).to_hex().to_string()
    }
}

/// The manifest record of one applied amendment.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct AmendmentRef {
    pub sequence: u64,
    pub digest: String,
    pub parent: String,
    pub queries: u64,
    /// Index of the amendment's first entry in the walk's `inputs`.
    pub first_input: u64,
    /// Admitted domain count before the amendment (its new IDs start here).
    pub first_domain: u64,
    /// IDs of the frontier taint quarantined when it was applied.
    pub quarantined: u64,
    /// Checkpoint generation the amendment was applied to (the resumed one).
    pub resumed_generation: u64,
}

/// A parsed, header-validated amendment.
pub(super) struct Parsed {
    pub sequence: u64,
    pub parent: String,
    pub digest: String,
    pub path: PathBuf,
    pub queries: Vec<query_input::Query>,
    /// Input query ids whose root records stop being live roots (the class
    /// rescue retires every open helper of the frontier's unbounded class).
    pub supersede: Vec<String>,
}

fn is_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Parse one amendment file: schema, header and query rows (the row format
/// and validation of the query document, `matching::input::parse`).
pub(super) fn parse(amendment: &OwnerDomainWalkAmendment, arity: usize) -> Result<Parsed, String> {
    let name = amendment.path.display();
    if amendment.text.len() > MAX_AMENDMENT_BYTES {
        return Err(format!(
            "amendment {name} exceeds {MAX_AMENDMENT_BYTES} bytes"
        ));
    }
    let document: Value = serde_json::from_str(&amendment.text)
        .map_err(|e| format!("amendment {name}: invalid JSON: {e}"))?;
    let object = document
        .as_object()
        .ok_or_else(|| format!("amendment {name} is not a JSON object"))?;
    let scope_extension = match document["schema"].as_str() {
        Some(AMENDMENT_SCHEMA) => false,
        Some(SCOPE_EXTENSION_SCHEMA) => true,
        _ => {
            return Err(format!(
                "amendment {name}: schema must be {AMENDMENT_SCHEMA} or {SCOPE_EXTENSION_SCHEMA}"
            ));
        }
    };
    if let Some(key) = object.keys().find(|k| {
        !matches!(
            k.as_str(),
            "schema" | "sequence" | "parent" | "queries" | "provenance"
        ) && !(scope_extension && k.as_str() == "query_roles")
            && !(!scope_extension && k.as_str() == "supersede")
    }) {
        return Err(format!("amendment {name}: unknown field {key:?}"));
    }
    let sequence = document["sequence"]
        .as_u64()
        .filter(|&s| s >= 1)
        .ok_or_else(|| format!("amendment {name}: sequence must be a positive integer"))?;
    let parent = document["parent"]
        .as_str()
        .filter(|p| is_digest(p))
        .ok_or_else(|| format!("amendment {name}: parent must be a 64-digit lowercase hex digest"))?
        .to_owned();
    if !document["provenance"].is_null() && !document["provenance"].is_object() {
        return Err(format!("amendment {name}: provenance must be an object"));
    }
    let rows = document["queries"]
        .as_array()
        .ok_or_else(|| format!("amendment {name}: queries must be an array"))?;
    let supersede = match document.get("supersede") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(ids)) => ids
            .iter()
            .map(|id| id.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("amendment {name}: supersede must list query ids"))?,
        Some(_) => return Err(format!("amendment {name}: supersede must list query ids")),
    };
    // An amendment adds queries, supersedes roots, or both; never neither.
    let queries = if rows.is_empty() && !supersede.is_empty() {
        Vec::new()
    } else {
        let mut query_document =
            json!({"schema":"rustred.owner-domain-queries.json.v2","queries":rows});
        if scope_extension {
            if !document["query_roles"].is_object() {
                return Err(format!(
                    "amendment {name}: scope extension needs explicit complete query_roles"
                ));
            }
            query_document["query_roles"] = document["query_roles"].clone();
        }
        let text = query_document.to_string();
        let mut queries =
            query_input::parse(&text, arity, MAX_AMENDMENT_QUERIES, MAX_AMENDMENT_BYTES)
                .map_err(|e| format!("amendment {name}: {e}"))?;
        if scope_extension {
            if queries
                .iter()
                .any(|query| query.auxiliary || !query.role_declared)
            {
                return Err(format!(
                    "amendment {name}: scope extension may append only explicitly required queries"
                ));
            }
        } else {
            // Preserve the original rescue-v1 auxiliary-only semantics.
            for query in &mut queries {
                query.auxiliary = true;
                query.role_declared = true;
            }
        }
        queries
    };
    Ok(Parsed {
        sequence,
        parent,
        digest: amendment.digest(),
        path: amendment.path.clone(),
        queries,
        supersede,
    })
}

/// Validate the supplied chain against the recorded one (append-only, digest
/// equality for every recorded link) and the request binding (the first
/// parent). Query ids must be unique across the original queries and every
/// amendment. Returns the index of the first new amendment.
pub(super) fn check_chain(
    recorded: &[AmendmentRef],
    supplied: &[Parsed],
    request_digest: &str,
    originals: &[query_input::Query],
) -> Result<usize, String> {
    if !recorded.is_empty() || !supplied.is_empty() {
        query_input::require_explicit_roles(originals)?;
    }
    if supplied.len() > MAX_AMENDMENTS {
        return Err(format!("at most {MAX_AMENDMENTS} rescue amendments"));
    }
    if supplied.len() < recorded.len() {
        return Err(format!(
            "the checkpoint records {} rescue amendment(s) but {} were supplied; every recorded amendment must be supplied again, in order (--amend-queries)",
            recorded.len(),
            supplied.len()
        ));
    }
    let mut ids: std::collections::BTreeSet<&str> =
        originals.iter().map(|q| q.id.as_str()).collect();
    let mut required: std::collections::BTreeSet<&str> = originals
        .iter()
        .filter(|q| !q.auxiliary)
        .map(|q| q.id.as_str())
        .collect();
    for (index, amendment) in supplied.iter().enumerate() {
        let expected_parent = if index == 0 {
            request_digest
        } else {
            supplied[index - 1].digest.as_str()
        };
        if amendment.sequence != index as u64 + 1 {
            return Err(format!(
                "amendment {} has sequence {}, expected {} (chain order)",
                amendment.path.display(),
                amendment.sequence,
                index + 1
            ));
        }
        if amendment.parent != expected_parent {
            return Err(format!(
                "amendment {} does not chain from {} (parent {}, expected {expected_parent})",
                amendment.path.display(),
                if index == 0 {
                    "the checkpoint request binding"
                } else {
                    "the previous amendment"
                },
                amendment.parent
            ));
        }
        if let Some(saved) = recorded.get(index)
            && (saved.digest != amendment.digest
                || saved.sequence != amendment.sequence
                || saved.parent != amendment.parent
                || saved.queries != amendment.queries.len() as u64)
        {
            return Err(format!(
                "amendment {} differs from the recorded amendment {} (digest {} vs {}); amendments are append-only",
                amendment.path.display(),
                saved.sequence,
                amendment.digest,
                saved.digest
            ));
        }
        for id in &amendment.supersede {
            if required.contains(id.as_str()) {
                return Err(format!("amendment cannot supersede required query {id:?}"));
            }
            if !ids.contains(id.as_str()) {
                return Err(format!(
                    "amendment {} supersedes {id:?}, which is not an earlier query",
                    amendment.path.display()
                ));
            }
        }
        for query in &amendment.queries {
            if !ids.insert(query.id.as_str()) {
                return Err(format!(
                    "amendment {} repeats query id {:?}",
                    amendment.path.display(),
                    query.id
                ));
            }
            if !query.auxiliary {
                required.insert(query.id.as_str());
            }
        }
    }
    Ok(recorded.len())
}

/// The walk domain of an amended query: Apply for an installed owner,
/// Route for an uninstalled one under route overcover. A query needing
/// source-validity conditions cannot be amended (it would be an input
/// frontier); neither can one of an uninstalled owner without overcover.
pub(super) fn domain<const N: usize>(
    query: &query_input::Query,
    installed: bool,
    route_domain_overcover: bool,
    source_conditions: bool,
) -> Result<Domain<N>, String> {
    let owner = rustred::storage_array(&query.owner, false)
        .ok_or_else(|| "amended query arity".to_owned())?;
    let phase = if installed {
        Phase::Apply
    } else if route_domain_overcover && !source_conditions {
        Phase::Route
    } else {
        return Err(format!(
            "amended query {} names an owner without an installed program that the rescue cannot admit",
            query.id
        ));
    };
    Ok(Domain {
        phase,
        owner,
        lower: rustred::storage_array::<_, N>(&query.lower, 0)
            .expect("validated query arity")
            .to_vec(),
        upper: rustred::storage_array::<_, N>(&query.upper, Some(0))
            .expect("validated query arity")
            .to_vec(),
        rank: query.rank,
        powers: query.powers,
    })
}

/// Exact native inclusion `outer ⊇ inner` (same phase and owner).
pub(super) fn contains<const N: usize>(outer: &Domain<N>, inner: &Domain<N>) -> bool {
    if outer.phase != inner.phase || outer.owner != inner.owner {
        return false;
    }
    let summary = |d: &Domain<N>| {
        rustred::solver::DomainPowerSummary::try_new(d.owner, &d.lower, &d.upper, d.rank, d.powers)
    };
    match (summary(outer), summary(inner)) {
        (Ok(outer), Ok(inner)) => outer.contains(&inner),
        _ => false,
    }
}

/// Per-query certification of an amended walk (see the module docs): each
/// query's own root, whether it is closed, and the first input root (in
/// input order) that contains the query and is closed. `closed` is the
/// dependency monitor's view (None: unavailable, then nothing certifies).
pub(super) fn query_certification<const N: usize>(
    ids: &[(&str, bool)],
    query_domains: &[Domain<N>],
    inputs: &[Value],
    domain_of: impl Fn(usize) -> Option<Domain<N>>,
    closed: impl Fn(usize) -> Option<bool>,
) -> Value {
    let roots: Vec<Option<usize>> = inputs
        .iter()
        .map(|input| {
            input["domain"]
                .as_u64()
                .and_then(|id| usize::try_from(id).ok())
        })
        .collect();
    let root_domains: Vec<Option<Domain<N>>> =
        roots.iter().map(|root| root.and_then(&domain_of)).collect();
    let mut rows = Vec::new();
    let mut certified = 0usize;
    let mut uncertified = Vec::new();
    let (mut required_total, mut required_certified) = (0usize, 0usize);
    let mut required_uncovered = Vec::new();
    for (index, ((id, auxiliary), query)) in ids.iter().zip(query_domains).enumerate() {
        let root = roots.get(index).copied().flatten();
        let root_closed = root.and_then(&closed);
        let mut via = None;
        for (candidate, (root, domain)) in roots.iter().zip(&root_domains).enumerate() {
            let (Some(root), Some(domain)) = (root, domain) else {
                continue;
            };
            // The query's own root is compared in its own phase (a Route root
            // of an uninstalled owner holds an Apply-shaped query).
            let mut probe = query.clone();
            probe.phase = domain.phase;
            if closed(*root) == Some(true) && contains(domain, &probe) {
                via = Some((candidate, *root));
                break;
            }
        }
        if via.is_some() {
            certified += 1;
        } else if uncertified.len() < 1_000 {
            uncertified.push(json!(id));
        }
        if !auxiliary {
            required_total += 1;
            required_certified += usize::from(via.is_some());
            if via.is_none() && required_uncovered.len() < 1_000 {
                required_uncovered.push(*id);
            }
        }
        rows.push(json!({"id":id,"root":root,"root_closed":root_closed,
            "role":if *auxiliary {"auxiliary"} else {"required"},
            "certified_via_root":via.map(|v| v.1),"certified_via_input":via.map(|v| v.0),
            "amended":inputs.get(index).is_some_and(|input| input.get("amendment").is_some())}));
    }
    json!({"method":"closed_containing_input_root",
        "scope":"a query is certified iff some input root (original or amended) whose domain contains it is descendant-closed; required and auxiliary roles come from the immutable exact-ID declaration, never the query name",
        "queries_total":ids.len(),"queries_certified":certified,
        "required_queries_total":required_total,"required_queries_certified":required_certified,
        "required_queries_uncovered":required_uncovered,
        "uncertified_queries":uncertified,"rows":rows,"family_closure_claim":false})
}

/// Resume of an amended walk: install the frontier-taint quarantine, then
/// admit every new amendment's queries (in chain and row order) and record
/// the amendment in the store. Called after restore and owner preparation,
/// before the forced first save, which persists the admitted domains, the
/// extended inputs and the chain in one generation. An error leaves the
/// saved checkpoint untouched (nothing was saved yet).
#[allow(clippy::too_many_arguments)]
pub(super) fn apply<const N: usize>(
    state: &mut super::execution::State<N>,
    inputs: &mut Vec<Value>,
    store: &mut super::checkpoint::Store,
    reducer: &rustred::solver::RoutedCandidateReducer<N>,
    route_domain_overcover: bool,
    amendments: &[Parsed],
    first_new: usize,
    observer: &impl Fn(Value),
) -> Result<(), String> {
    // Durable G2' loans already fix the producer's replay, even at zero
    // callbacks. Include their edges before computing the dead frontier cone.
    state.g2_restore_pin_dependencies()?;
    // Frontier seeds: finished natives that kept frontiers (the tracker's
    // inspected-unsealed nodes) and inspections whose accepted, uncommitted
    // prefix already holds frontiers (an A10 stop inside a chunked stream).
    let prefix_frontiers = frontier_seeds(state);
    let tainted = state
        .closure
        .borrow()
        .tainted_with(&prefix_frontiers)
        .ok_or(
            "the frontier rescue needs the dependency monitor, which is unavailable in this checkpoint",
        )?;
    // First the frontier taint and every superseded root record, so the
    // amended queries never resolve into them (a superseded unbounded helper
    // contains its owner's rescue helper).
    let quarantined = tainted
        .iter()
        .map(|w| w.count_ones() as usize)
        .sum::<usize>();
    let mut first = tainted;
    for record in superseded_records(inputs, amendments) {
        if (record as usize) < state.queue.domains.len() {
            first[record as usize / 64] |= 1 << (record % 64);
        }
    }
    state.queue.install_quarantine(first)?;
    let resumed_generation = store.generation();
    let recorded = store.amendments().len();
    let source_conditions = reducer.domain_routing_requires_source_conditions();
    for amendment in &amendments[first_new..] {
        let first_input = inputs.len();
        let first_domain = state.queue.domains.len();
        let (mut fresh, mut reused) = (0usize, 0usize);
        for query in &amendment.queries {
            let installed = reducer.programs().owner_sectors().any(|owner| {
                owner.as_slice()
                    == rustred::storage_array::<_, N>(&query.owner, false)
                        .as_ref()
                        .map(|owner| owner.as_slice())
                        .unwrap_or(&[])
            });
            let domain = domain::<N>(query, installed, route_domain_overcover, source_conditions)?;
            let (id, new) = state
                .queue
                .admit(domain)
                .map_err(|e| format!("amended query {}: {e}", query.id))?;
            fresh += usize::from(new);
            reused += usize::from(!new);
            inputs
                .try_reserve(1)
                .map_err(|_| "amended input allocation")?;
            inputs.push(input_json(query, id, amendment.sequence));
        }
        state
            .closure
            .borrow_mut()
            .discovered(state.queue.domains.len());
        store.record_amendment(AmendmentRef {
            sequence: amendment.sequence,
            digest: amendment.digest.clone(),
            parent: amendment.parent.clone(),
            queries: amendment.queries.len() as u64,
            first_input: first_input as u64,
            first_domain: first_domain as u64,
            quarantined: quarantined as u64,
            resumed_generation,
        });
        observer(
            json!({"event":"rescue_amendment_applied","operation":"owner_domain_walk",
            "sequence":amendment.sequence,"digest":amendment.digest,"parent":amendment.parent,
            "path":amendment.path,"queries":amendment.queries.len(),"admitted_new_domains":fresh,
            "resolved_to_existing_domains":reused,"quarantined_domains":quarantined,
            "first_input":first_input,"first_domain":first_domain,"family_closure_claim":false}),
        );
    }
    abandon_dead_cones(
        state,
        inputs,
        amendments,
        quarantined,
        recorded,
        amendments.len() - first_new,
        observer,
    )
}

/// The root records of every query a chain amendment supersedes.
fn superseded_records(inputs: &[Value], amendments: &[Parsed]) -> std::collections::BTreeSet<u64> {
    let ids: std::collections::BTreeSet<&str> = amendments
        .iter()
        .flat_map(|a| a.supersede.iter().map(String::as_str))
        .collect();
    inputs
        .iter()
        .filter(|input| input["id"].as_str().is_some_and(|id| ids.contains(id)))
        .filter_map(|input| input["domain"].as_u64())
        .collect()
}

/// Bitset of the inspections whose accepted, uncommitted prefix holds frontiers.
fn frontier_seeds<const N: usize>(state: &super::execution::State<N>) -> Vec<u64> {
    let mut seeds = vec![0u64; state.queue.domains.len().div_ceil(64)];
    for id in state.frontier_prefix_holders() {
        if id < state.queue.domains.len() {
            seeds[id / 64] |= 1 << (id % 64);
        }
    }
    seeds
}

/// Liveness after the amendments: every node reachable over recorded edges
/// from an untainted input root (original or amended) is live. An
/// unpublished obligation no live root reaches is dead: no query can certify
/// through it. Dead obligations and every node reaching one join the
/// quarantine; dead, non-delegated obligations without a restored partial
/// prefix are abandoned (published without inspection, one bookkeeping
/// frontier each); a dead obligation with a partial prefix is inspected and
/// every domain it newly admits dies with it (`Queue::mark_dead`). Live
/// nodes are never quarantined (a node reaching a dead or frontier node
/// would make its root reach it).
fn abandon_dead_cones<const N: usize>(
    state: &mut super::execution::State<N>,
    inputs: &[Value],
    amendments: &[Parsed],
    tainted: usize,
    recorded: usize,
    new: usize,
    observer: &impl Fn(Value),
) -> Result<(), String> {
    let unavailable = "the frontier rescue could not reserve its liveness scratch";
    // Superseded root records (the class rescue) are not live roots, even
    // when another query (a physics query absorbed by the helper) names them.
    let superseded_ids: std::collections::BTreeSet<&str> = amendments
        .iter()
        .flat_map(|a| a.supersede.iter().map(String::as_str))
        .collect();
    let superseded: std::collections::BTreeSet<u64> = inputs
        .iter()
        .filter(|input| {
            input["id"]
                .as_str()
                .is_some_and(|id| superseded_ids.contains(id))
        })
        .filter_map(|input| input["domain"].as_u64())
        .collect();
    let roots: Vec<usize> = inputs
        .iter()
        .filter_map(|input| input["domain"].as_u64())
        .filter(|id| !superseded.contains(id))
        .map(|id| id as usize)
        .filter(|&id| id < state.queue.domains.len() && !state.queue.is_quarantined(id))
        .collect();
    let total = state.queue.domains.len();
    let words = total.div_ceil(64);
    let bit = |bits: &[u64], id: usize| bits.get(id / 64).is_some_and(|w| w >> (id % 64) & 1 != 0);
    let set = |bits: &mut [u64], id: usize| bits[id / 64] |= 1 << (id % 64);
    let closure = state.closure.borrow();
    let live = closure
        .reachable_from(roots.iter().copied())
        .ok_or(unavailable)?;
    let mut sources = vec![0u64; words];
    let _ = closure.try_for_each_edge(|source, _| {
        if source < total {
            sources[source / 64] |= 1 << (source % 64);
        }
        std::ops::ControlFlow::<()>::Continue(())
    });
    for id in state.accepted_prefix_holders() {
        if id < total {
            sources[id / 64] |= 1 << (id % 64);
        }
    }
    let ledger = state.queue.delegation.as_ref();
    let mut dead = vec![0u64; words];
    let mut abandon = vec![0u64; words];
    let (mut dead_count, mut abandoned, mut partial, mut delegated) =
        (0usize, 0usize, 0usize, 0usize);
    for id in 0..total {
        let published = ledger.map_or(id < state.queue.next, |l| l.is_published(id));
        if published || bit(&live, id) {
            continue;
        }
        if let Some(mut to) = ledger.and_then(|l| l.delegated_to(id)) {
            // An unpublished alias of a live representative is live: it will
            // seal through it.
            while let Some(next) = ledger.and_then(|l| l.delegated_to(to)) {
                to = next;
            }
            if bit(&live, to) {
                continue;
            }
            set(&mut dead, id);
            dead_count += 1;
            delegated += 1;
            continue;
        }
        set(&mut dead, id);
        dead_count += 1;
        if bit(&sources, id) {
            partial += 1;
        } else {
            set(&mut abandon, id);
            abandoned += 1;
        }
    }
    let mut seeds = frontier_seeds(state);
    seeds.resize(words.max(seeds.len()), 0);
    for (word, bits) in dead.iter().enumerate() {
        seeds[word] |= bits;
    }
    let quarantine = closure.tainted_with(&seeds).ok_or(unavailable)?;
    drop(closure);
    let quarantined = state.queue.install_quarantine(quarantine)?;
    state.queue.set_abandoned(abandon)?;
    // Worker-view epoch: the quarantined initial domains the shortcut views
    // exclude from now on. A restored accepted prefix keeps the epoch it
    // started under (epoch 0 = unfiltered, before any rescue), so its replay
    // sees the same native stream.
    let excluded: Vec<usize> = (0..state.initial_domain_count)
        .filter(|&id| state.queue.is_quarantined(id))
        .collect();
    if state.rescue_worker_epochs.is_empty() {
        state.rescue_worker_epochs.push(Vec::new());
    }
    if state.rescue_worker_epochs.last() != Some(&excluded) {
        state.rescue_worker_epochs.push(excluded);
    }
    let restored = std::mem::take(&mut state.rescue_holder_epochs);
    state.rescue_holder_epochs = state
        .accepted_prefix_holders()
        .into_iter()
        .map(|id| (id, restored.get(&id).copied().unwrap_or(0)))
        .collect();
    let epoch = state.rescue_worker_epochs.len() - 1;
    observer(
        json!({"event":"rescue_quarantine","operation":"owner_domain_walk",
        "frontier_tainted_domains":tainted,"live_input_roots":roots.len(),
        "superseded_root_records":superseded.len(),
        "dead_pending_domains":dead_count,"abandoned_domains":abandoned,
        "dead_delegated_domains":delegated,"dead_partial_prefix_domains":partial,
        "quarantined_domains":quarantined,"admitted_domains":total,
        "worker_view_epoch":epoch,"restored_prefix_epochs":state.rescue_holder_epochs.iter().map(|(id, e)| [*id, *e]).collect::<Vec<_>>(),
        "amendments_recorded":recorded,"amendments_new":new,
        "scope":"lookup quarantine of every node reaching a frontier-bearing native or a dead obligation; dead obligations are published without inspection; certification unchanged",
        "family_closure_claim":false}),
    );
    Ok(())
}

/// The report block of an amended walk's chain.
pub(super) fn chain_json(recorded: &[AmendmentRef]) -> Value {
    Value::Array(
        recorded
            .iter()
            .map(|a| {
                json!({"sequence":a.sequence,"digest":a.digest,"parent":a.parent,"queries":a.queries,
                    "first_input":a.first_input,"first_domain":a.first_domain,
                    "quarantined":a.quarantined,"resumed_generation":a.resumed_generation})
            })
            .collect(),
    )
}

/// An input row of an amended query.
pub(super) fn input_json(query: &query_input::Query, id: usize, sequence: u64) -> Value {
    json!({"id":query.id,"domain":id,"amendment":sequence})
}

/// The query row of a domain (the query-document format), for the planner.
pub(super) fn query_row<const N: usize>(id: &str, domain: &Domain<N>) -> Value {
    json!({"id":id,"owner":super::mask(&domain.owner),"lower":domain.lower,"upper":domain.upper,
        "max_numerator_rank":domain.rank,"power_bounds":power_bounds_json(domain.powers)})
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred::solver::DomainPowerBounds;

    fn originals(auxiliary: bool) -> Vec<query_input::Query> {
        let mut rows = parse(&amendment(1, &"a".repeat(64), &["q"]), 2)
            .unwrap()
            .queries;
        rows[0].auxiliary = auxiliary;
        rows
    }

    fn amendment(sequence: u64, parent: &str, ids: &[&str]) -> OwnerDomainWalkAmendment {
        let queries: Vec<Value> = ids
            .iter()
            .map(|id| {
                json!({"id":id,"owner":"11","lower":[0,0],"upper":[null,null],
                "max_numerator_rank":3,"power_bounds":{"max_positive_power":null,
                "min_power_difference":null,"max_power_difference":null}})
            })
            .collect();
        OwnerDomainWalkAmendment {
            path: PathBuf::from(format!("a{sequence}.json")),
            text: json!({"schema":AMENDMENT_SCHEMA,"sequence":sequence,"parent":parent,
                "queries":queries,"provenance":{"generator":"test"}})
            .to_string(),
        }
    }

    fn reference(parsed: &Parsed, first_input: u64) -> AmendmentRef {
        AmendmentRef {
            sequence: parsed.sequence,
            digest: parsed.digest.clone(),
            parent: parsed.parent.clone(),
            queries: parsed.queries.len() as u64,
            first_input,
            first_domain: 0,
            quarantined: 0,
            resumed_generation: 1,
        }
    }

    fn extension(sequence: u64, parent: &str, ids: &[&str]) -> OwnerDomainWalkAmendment {
        let mut result = amendment(sequence, parent, ids);
        let mut document: Value = serde_json::from_str(&result.text).unwrap();
        document["schema"] = SCOPE_EXTENSION_SCHEMA.into();
        document["query_roles"] = json!({"required":ids,"auxiliary":[]});
        result.text = document.to_string();
        result
    }

    #[test]
    fn scope_extension_requires_complete_required_roles_and_protects_the_chain() {
        let request = "a".repeat(64);
        let first_file = extension(1, &request, &["rank-1"]);
        let first = parse(&first_file, 2).unwrap();
        assert!(!first.queries[0].auxiliary);
        assert!(first.queries[0].role_declared);
        let second = parse(&extension(2, &first.digest, &["rank-2"]), 2).unwrap();
        assert_eq!(
            check_chain(&[], &[first, second], &request, &originals(false)),
            Ok(0)
        );
        for bad_roles in [
            Value::Null,
            json!({"required":[],"auxiliary":["rank-1"]}),
            json!({"required":[],"auxiliary":[]}),
            json!({"required":["rank-1"],"auxiliary":["rank-1"]}),
        ] {
            let mut file = first_file.clone();
            let mut doc: Value = serde_json::from_str(&file.text).unwrap();
            doc["query_roles"] = bad_roles;
            file.text = doc.to_string();
            assert!(parse(&file, 2).is_err());
        }
        let mut illegal = first_file.clone();
        let mut doc: Value = serde_json::from_str(&illegal.text).unwrap();
        doc["supersede"] = json!([]);
        illegal.text = doc.to_string();
        assert!(parse(&illegal, 2).err().unwrap().contains("supersede"));

        // A later legacy rescue cannot silently weaken the enlarged scope.
        let first = parse(&first_file, 2).unwrap();
        let mut rescue_file = amendment(2, &first.digest, &["helper"]);
        let mut doc: Value = serde_json::from_str(&rescue_file.text).unwrap();
        doc["supersede"] = json!(["rank-1"]);
        rescue_file.text = doc.to_string();
        let rescue = parse(&rescue_file, 2).unwrap();
        assert!(rescue.queries[0].auxiliary, "rescue-v1 keeps old semantics");
        assert!(
            check_chain(&[], &[first, rescue], &request, &originals(false))
                .unwrap_err()
                .contains("required query")
        );

        let first = parse(&first_file, 2).unwrap();
        let recorded = [reference(&first, 1)];
        let mut rewritten = first_file;
        rewritten.text.push(' ');
        assert!(
            check_chain(
                &recorded,
                &[parse(&rewritten, 2).unwrap()],
                &request,
                &originals(false)
            )
            .unwrap_err()
            .contains("append-only")
        );
    }

    #[test]
    fn chain_must_start_at_the_request_and_link_every_digest() {
        let request = "a".repeat(64);
        let first = parse(&amendment(1, &request, &["h1"]), 2).unwrap();
        let second = parse(&amendment(2, &first.digest, &["h2"]), 2).unwrap();
        let chain = [first, second];
        assert_eq!(check_chain(&[], &chain, &request, &originals(false)), Ok(0));
        let recorded = [reference(&chain[0], 1)];
        assert_eq!(
            check_chain(&recorded, &chain, &request, &originals(false)),
            Ok(1)
        );
        // A foreign request, a broken link and a skipped sequence are refused.
        let other = "b".repeat(64);
        assert!(
            check_chain(&[], &chain, &other, &originals(false))
                .unwrap_err()
                .contains("request binding")
        );
        let loose = parse(&amendment(2, &request, &["h2"]), 2).unwrap();
        let broken = [parse(&amendment(1, &request, &["h1"]), 2).unwrap(), loose];
        assert!(
            check_chain(&[], &broken, &request, &originals(false))
                .unwrap_err()
                .contains("previous amendment")
        );
        let skipped = [parse(&amendment(2, &request, &["h1"]), 2).unwrap()];
        assert!(
            check_chain(&[], &skipped, &request, &originals(false))
                .unwrap_err()
                .contains("sequence")
        );
    }

    #[test]
    fn recorded_amendments_are_append_only() {
        let request = "c".repeat(64);
        let first = parse(&amendment(1, &request, &["h1"]), 2).unwrap();
        let recorded = [reference(&first, 1)];
        // Omitting a recorded amendment is refused.
        assert!(
            check_chain(&recorded, &[], &request, &originals(false))
                .unwrap_err()
                .contains("must be supplied again")
        );
        // A rewritten first amendment (same header, other queries) is refused.
        let rewritten = parse(&amendment(1, &request, &["h9"]), 2).unwrap();
        assert!(
            check_chain(&recorded, &[rewritten], &request, &originals(false))
                .unwrap_err()
                .contains("append-only")
        );
        // Repeated query ids are refused (against the originals too).
        let repeat = parse(&amendment(1, &request, &["q"]), 2).unwrap();
        assert!(
            check_chain(&[], &[repeat], &request, &originals(false))
                .unwrap_err()
                .contains("repeats")
        );
    }

    #[test]
    fn rescue_rejects_undeclared_original_scope() {
        let request = "a".repeat(64);
        let mut original = originals(false);
        original[0].role_declared = false;
        let amendment = parse(&amendment(1, &request, &["new-helper"]), 2).unwrap();
        assert!(
            check_chain(&[], &[amendment], &request, &original)
                .unwrap_err()
                .contains("explicit complete query_roles")
        );
    }

    #[test]
    fn parse_refuses_unknown_fields_bad_schema_and_bad_parent() {
        let request = "d".repeat(64);
        let mut value: Value = serde_json::from_str(&amendment(1, &request, &["h"]).text).unwrap();
        value["extra"] = json!(1);
        let bad = OwnerDomainWalkAmendment {
            path: "x".into(),
            text: value.to_string(),
        };
        assert!(parse(&bad, 2).err().unwrap().contains("unknown field"));
        let mut value: Value = serde_json::from_str(&amendment(1, &request, &["h"]).text).unwrap();
        value["schema"] = json!("other");
        let bad = OwnerDomainWalkAmendment {
            path: "x".into(),
            text: value.to_string(),
        };
        assert!(parse(&bad, 2).err().unwrap().contains("schema"));
        let bad = amendment(1, "ABC", &["h"]);
        assert!(parse(&bad, 2).err().unwrap().contains("parent"));
        // Wrong arity is a query-row error.
        assert!(parse(&amendment(1, &request, &["h"]), 3).is_err());
    }

    #[test]
    fn supersede_lists_only_earlier_queries_and_may_stand_alone() {
        let request = "e".repeat(64);
        let text = |supersede: Value, queries: Value| OwnerDomainWalkAmendment {
            path: "s.json".into(),
            text: json!({"schema":AMENDMENT_SCHEMA,"sequence":1,"parent":request,
                "queries":queries,"supersede":supersede})
            .to_string(),
        };
        let alone = parse(&text(json!(["q"]), json!([])), 2).unwrap();
        assert!(alone.queries.is_empty());
        assert_eq!(alone.supersede, vec!["q".to_owned()]);
        assert!(
            check_chain(&[], &[alone], &request, &originals(false))
                .unwrap_err()
                .contains("required query")
        );
        let alone = parse(&text(json!(["q"]), json!([])), 2).unwrap();
        assert_eq!(
            check_chain(&[], &[alone], &request, &originals(true)),
            Ok(0)
        );
        let unknown = parse(&text(json!(["nope"]), json!([])), 2).unwrap();
        assert!(
            check_chain(&[], &[unknown], &request, &originals(false))
                .unwrap_err()
                .contains("not an earlier query")
        );
        assert!(
            parse(&text(json!([]), json!([])), 2).is_err(),
            "an empty amendment is refused"
        );
        assert!(parse(&text(json!([1]), json!([])), 2).is_err());
    }

    #[test]
    fn certification_uses_the_first_closed_containing_root() {
        // One numerator axis, so the rank bound is geometry.
        let d = |rank: Option<u32>| Domain::<2> {
            phase: Phase::Apply,
            owner: [true, false],
            lower: vec![0, 0],
            upper: vec![None, None],
            rank,
            powers: DomainPowerBounds::default(),
        };
        // Query 0 is a helper (rank None, root 0, open); query 1 a physics
        // query absorbed by it; query 2 the amended bounded helper (root 1).
        let queries = [d(None), d(Some(2)), d(Some(4))];
        let inputs = [
            json!({"id":"h","domain":0}),
            json!({"id":"p","domain":0}),
            json!({"id":"r","domain":1,"amendment":1}),
        ];
        let domains = [d(None), d(Some(4))];
        let report = query_certification(
            &[("h", true), ("p", false), ("r", true)],
            &queries,
            &inputs,
            |id| domains.get(id).cloned(),
            |id| Some(id == 1),
        );
        assert_eq!(report["queries_certified"], 2);
        assert_eq!(report["uncertified_queries"], json!(["h"]));
        assert_eq!(report["rows"][1]["certified_via_root"], 1);
        assert_eq!(report["rows"][1]["root_closed"], false);
        assert_eq!(report["rows"][2]["amended"], true);
        // Unavailable closure: nothing certifies.
        let none = query_certification(
            &[("p", false)],
            &queries[1..2],
            &inputs[1..2],
            |id| domains.get(id).cloned(),
            |_| None,
        );
        assert_eq!(none["queries_certified"], 0);
    }

    #[test]
    fn partial_replacement_cannot_drop_a_required_query_absorbed_by_the_same_root() {
        let d = |rank| Domain::<2> {
            phase: Phase::Apply,
            owner: [true, false],
            lower: vec![0, 0],
            upper: vec![None, None],
            rank,
            powers: DomainPowerBounds::default(),
        };
        // Both required queries were absorbed into auxiliary root 0. It was
        // quarantined/superseded; replacement root 1 covers only the first.
        let queries = [d(None), d(Some(2)), d(Some(5)), d(Some(4))];
        let inputs = [
            json!({"domain":0}),
            json!({"domain":0}),
            json!({"domain":0}),
            json!({"domain":1,"amendment":1}),
        ];
        let domains = [d(None), d(Some(4))];
        let report = query_certification(
            &[
                ("ordinary-auxiliary", true),
                ("required-anchor-a", false),
                ("required-anchor-b", false),
                ("replacement", true),
            ],
            &queries,
            &inputs,
            |id| domains.get(id).cloned(),
            |id| Some(id == 1),
        );
        assert_eq!(report["required_queries_total"], 2);
        assert_eq!(report["required_queries_certified"], 1);
        assert_eq!(
            report["required_queries_uncovered"],
            json!(["required-anchor-b"])
        );
        assert_eq!(report["rows"][2]["certified_via_root"], Value::Null);
    }
}
