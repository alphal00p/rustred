//! Resume-time frontier rescue on the legacy engine (owner requirement of
//! 2026-09-28: a frontier for which a known rescue exists must never end the
//! campaign; a stop is a pause and the rescue keeps every certified node).
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
//! Helper roots and physics queries are classified by id pattern in the
//! audit and in `walk-verify-closure`, which re-derive this independently.
//! `family_closure_claim` stays false.
use super::matching::input::{self as query_input, power_bounds_json};
use super::queue::{Domain, Phase};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

pub const AMENDMENT_SCHEMA: &str = "rustred.owner-domain-walk-amendment.json.v1";
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
    if let Some(key) = object.keys().find(|k| {
        !matches!(
            k.as_str(),
            "schema" | "sequence" | "parent" | "queries" | "provenance"
        )
    }) {
        return Err(format!("amendment {name}: unknown field {key:?}"));
    }
    if document["schema"] != AMENDMENT_SCHEMA {
        return Err(format!(
            "amendment {name}: schema must be {AMENDMENT_SCHEMA}"
        ));
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
    let text = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":rows}).to_string();
    let queries = query_input::parse(&text, arity, MAX_AMENDMENT_QUERIES, MAX_AMENDMENT_BYTES)
        .map_err(|e| format!("amendment {name}: {e}"))?;
    Ok(Parsed {
        sequence,
        parent,
        digest: amendment.digest(),
        path: amendment.path.clone(),
        queries,
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
    original_ids: &[&str],
) -> Result<usize, String> {
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
    let mut ids: std::collections::BTreeSet<&str> = original_ids.iter().copied().collect();
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
        for query in &amendment.queries {
            if !ids.insert(query.id.as_str()) {
                return Err(format!(
                    "amendment {} repeats query id {:?}",
                    amendment.path.display(),
                    query.id
                ));
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
    let owner: [bool; N] = query
        .owner
        .as_slice()
        .try_into()
        .map_err(|_| "amended query arity".to_owned())?;
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
        lower: query.lower.clone(),
        upper: query.upper.clone(),
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
    ids: &[&str],
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
    for (index, (id, query)) in ids.iter().zip(query_domains).enumerate() {
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
        rows.push(json!({"id":id,"root":root,"root_closed":root_closed,
            "certified_via_root":via.map(|v| v.1),"certified_via_input":via.map(|v| v.0),
            "amended":inputs.get(index).is_some_and(|input| input.get("amendment").is_some())}));
    }
    json!({"method":"closed_containing_input_root",
        "scope":"a query is certified iff some input root (original or amended) whose domain contains it is descendant-closed; helper roots and physics queries are classified by id pattern in the audit and in walk-verify-closure",
        "queries_total":ids.len(),"queries_certified":certified,
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
    let tainted = state.closure.borrow().tainted().ok_or(
        "the frontier rescue needs the dependency monitor, which is unavailable in this checkpoint",
    )?;
    let quarantined = state.queue.install_quarantine(tainted)?;
    let resumed_generation = store.generation();
    observer(
        json!({"event":"rescue_quarantine","operation":"owner_domain_walk",
        "quarantined_domains":quarantined,"admitted_domains":state.queue.domains.len(),
        "amendments_recorded":store.amendments().len(),"amendments_new":amendments.len() - first_new,
        "scope":"lookup quarantine of every node reaching a frontier-bearing native; certification unchanged",
        "family_closure_claim":false}),
    );
    let source_conditions = reducer.domain_routing_requires_source_conditions();
    for amendment in &amendments[first_new..] {
        let first_input = inputs.len();
        let first_domain = state.queue.domains.len();
        let (mut fresh, mut reused) = (0usize, 0usize);
        for query in &amendment.queries {
            let installed = reducer
                .programs()
                .owner_sectors()
                .any(|owner| owner.as_slice() == query.owner.as_slice());
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

    #[test]
    fn chain_must_start_at_the_request_and_link_every_digest() {
        let request = "a".repeat(64);
        let first = parse(&amendment(1, &request, &["h1"]), 2).unwrap();
        let second = parse(&amendment(2, &first.digest, &["h2"]), 2).unwrap();
        let chain = [first, second];
        assert_eq!(check_chain(&[], &chain, &request, &["q"]), Ok(0));
        let recorded = [reference(&chain[0], 1)];
        assert_eq!(check_chain(&recorded, &chain, &request, &["q"]), Ok(1));
        // A foreign request, a broken link and a skipped sequence are refused.
        let other = "b".repeat(64);
        assert!(
            check_chain(&[], &chain, &other, &["q"])
                .unwrap_err()
                .contains("request binding")
        );
        let loose = parse(&amendment(2, &request, &["h2"]), 2).unwrap();
        let broken = [parse(&amendment(1, &request, &["h1"]), 2).unwrap(), loose];
        assert!(
            check_chain(&[], &broken, &request, &["q"])
                .unwrap_err()
                .contains("previous amendment")
        );
        let skipped = [parse(&amendment(2, &request, &["h1"]), 2).unwrap()];
        assert!(
            check_chain(&[], &skipped, &request, &["q"])
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
            check_chain(&recorded, &[], &request, &["q"])
                .unwrap_err()
                .contains("must be supplied again")
        );
        // A rewritten first amendment (same header, other queries) is refused.
        let rewritten = parse(&amendment(1, &request, &["h9"]), 2).unwrap();
        assert!(
            check_chain(&recorded, &[rewritten], &request, &["q"])
                .unwrap_err()
                .contains("append-only")
        );
        // Repeated query ids are refused (against the originals too).
        let repeat = parse(&amendment(1, &request, &["q"]), 2).unwrap();
        assert!(
            check_chain(&[], &[repeat], &request, &["q"])
                .unwrap_err()
                .contains("repeats")
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
    fn certification_uses_the_first_closed_containing_root() {
        let d = |rank: Option<u32>| Domain::<2> {
            phase: Phase::Apply,
            owner: [true, true],
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
            &["h", "p", "r"],
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
            &["p"],
            &queries[1..2],
            &inputs[1..2],
            |id| domains.get(id).cloned(),
            |_| None,
        );
        assert_eq!(none["queries_certified"], 0);
    }
}
