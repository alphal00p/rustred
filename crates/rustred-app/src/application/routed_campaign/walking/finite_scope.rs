//! Read-only bridge from existing exact query/role/amendment parsers.
//! No checkpoint is opened, adopted, created or certified here.
use super::{OwnerDomainWalkRequest, finite_replay, matching, rescue};
use crate::AppError;
use rustred::solver::{RootRegionInput, RoutedCandidateReducer};
use serde_json::{Value, json};

pub(in crate::application::routed_campaign) struct Row {
    pub query: matching::input::Query,
    pub source: usize,
}

pub(in crate::application::routed_campaign) struct Scope {
    pub rows: Vec<Row>,
    pub sources: Vec<Value>,
    pub request_binding: String,
}

pub(in crate::application::routed_campaign) fn parse(
    request: &OwnerDomainWalkRequest,
    arity: usize,
    max_rows: usize,
    max_source_bytes: usize,
) -> Result<Scope, AppError> {
    parse_bound_sources(
        &request.matching.queries_json,
        &request.amendments,
        request.checkpoint_binding(),
        arity,
        request.matching.max_queries,
        request.matching.max_query_bytes,
        max_rows,
        max_source_bytes,
    )
}

/// Reuse the same strict row/role and amendment-chain parser for a portable
/// captured source. The supplied binding must separately match its generation.
#[allow(clippy::too_many_arguments)]
pub(in crate::application::routed_campaign) fn parse_bound_sources(
    original: &str,
    amendments: &[crate::OwnerDomainWalkAmendment],
    request_binding: String,
    arity: usize,
    max_queries: usize,
    max_query_bytes: usize,
    max_rows: usize,
    max_source_bytes: usize,
) -> Result<Scope, AppError> {
    let bytes = amendments
        .iter()
        .try_fold(original.len(), |sum, amendment| {
            sum.checked_add(amendment.text.len())
        });
    if bytes.is_none_or(|bytes| bytes > max_source_bytes) || amendments.len() > max_rows {
        return Err(AppError::limit("finite demand source allowance exceeded"));
    }
    let originals = matching::input::parse(original, arity, max_queries, max_query_bytes)?;
    let amendments = amendments
        .iter()
        .map(|amendment| {
            rescue::parse_with_limits(amendment, arity, max_rows, max_source_bytes)
                .map_err(AppError::input)
        })
        .collect::<Result<Vec<_>, _>>()?;
    rescue::check_chain_with_limit(&[], &amendments, &request_binding, &originals, max_rows)
        .map_err(AppError::input)?;
    let row_count = amendments
        .iter()
        .try_fold(originals.len(), |sum, amendment| {
            sum.checked_add(amendment.queries.len())
        });
    if row_count.is_none_or(|count| count > max_rows) {
        return Err(AppError::limit("finite demand row allowance exceeded"));
    }
    if amendments.iter().any(|a| !a.supersede.is_empty()) {
        return Err(AppError::input(
            "finite demand is append-only: supersede amendments are unsupported",
        ));
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(row_count.unwrap())
        .map_err(|_| AppError::limit("cannot reserve admitted finite demand rows"))?;
    let mut sources = vec![json!({"ordinal":0,"kind":"original_queries",
        "blake3":blake3::hash(original.as_bytes()).to_hex().to_string(),
        "rows":originals.len()})];
    rows.extend(originals.into_iter().map(|query| Row { query, source: 0 }));
    for amendment in amendments {
        let source = sources.len();
        sources.push(json!({"ordinal":source,"kind":"append_only_amendment",
            "path":amendment.path,"sequence":amendment.sequence,
            "parent":amendment.parent,"blake3":amendment.digest,
            "rows":amendment.queries.len()}));
        rows.extend(
            amendment
                .queries
                .into_iter()
                .map(|query| Row { query, source }),
        );
    }
    Ok(Scope {
        rows,
        sources,
        request_binding,
    })
}

#[cfg(test)]
mod source_limit_tests {
    use super::*;

    fn row(id: &str) -> Value {
        json!({"id":id,"owner":"1","lower":[0],"upper":[0],"max_numerator_rank":0,
            "power_bounds":{"max_positive_power":1,"min_power_difference":0,"max_power_difference":1}})
    }

    #[test]
    fn finite_amendment_chain_uses_requested_grants_and_still_authenticates_each_link() {
        let binding = "12".repeat(32);
        let original =
            json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[row("original")],
            "query_roles":{"required":["original"],"auxiliary":[]}})
            .to_string();
        let mut parent = binding.clone();
        let amendments: Vec<_> = (1..=rescue::MAX_AMENDMENTS + 1).map(|sequence| {
            let id = format!("append-{sequence}");
            let amendment = crate::OwnerDomainWalkAmendment {
                path: format!("amendment-{sequence}.json").into(),
                text: json!({"schema":rescue::SCOPE_EXTENSION_SCHEMA,"sequence":sequence,"parent":parent,
                    "queries":[row(&id)],"query_roles":{"required":[id],"auxiliary":[]}}).to_string(),
            };
            parent = amendment.digest();
            amendment
        }).collect();
        let count = amendments.len() + 1;
        let parse = |rows, bytes, amendments: &[_]| {
            parse_bound_sources(
                &original,
                amendments,
                binding.clone(),
                1,
                1,
                original.len(),
                rows,
                bytes,
            )
        };
        assert!(parse(count - 1, usize::MAX, &amendments).is_err());
        assert!(parse(count, original.len(), &amendments).is_err());
        assert_eq!(
            parse(count, usize::MAX, &amendments).unwrap().rows.len(),
            count
        );
        let mut corrupt = amendments.clone();
        corrupt[1].text = corrupt[1].text.replace(&amendments[0].digest(), &binding);
        assert!(parse(count, usize::MAX, &corrupt).is_err());
    }
}

/// Preserve the original versus appended source-validity decision exactly.
/// Finite demand never silently drops a row with an unresolved input frontier.
pub(in crate::application::routed_campaign) fn region<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    row: &Row,
) -> Result<RootRegionInput<N>, AppError> {
    let domain = if row.source == 0 {
        finite_replay::query_domain(reducer, request, &row.query)
            .map_err(AppError::input)?
            .ok_or_else(|| AppError::input("finite demand has unresolved source validity"))?
    } else {
        let support = rustred::storage_array::<_, N>(&row.query.owner, false)
            .ok_or_else(|| AppError::input("finite demand physical/storage arity mismatch"))?;
        let installed = reducer
            .programs()
            .owner_sectors()
            .any(|owner| owner == &support);
        rescue::domain(
            &row.query,
            installed,
            request.route_domain_overcover,
            reducer.domain_routing_requires_source_conditions(),
        )
        .map_err(AppError::input)?
    };
    Ok(RootRegionInput {
        support: domain.owner,
        lower: domain.lower,
        upper: domain.upper,
        rank: domain.rank,
        powers: domain.powers,
    })
}
