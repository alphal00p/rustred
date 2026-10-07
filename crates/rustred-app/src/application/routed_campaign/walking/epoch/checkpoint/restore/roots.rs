//! Query-root validation uses the prepared reducer/request, never the saved
//! record phase. Decode one request-sized row at a time into final input rows.
#[cfg(test)]
use super::super::super::admission::phase;
use super::super::super::admission::{query_phase, source_frontier};
use super::super::super::verify::{Container, QueryImage, VerifyCounters, verify};
use super::super::invalid;
use super::super::metadata::Identity;
use super::super::publication::FileRef;
use super::super::read::Budget;
use super::{CheckedRead, Store};
use crate::application::routed_campaign::matching::input::Query;
use crate::application::routed_campaign::walking::queue::{CompactDomain, Domain, Phase};
#[cfg(test)]
use crate::application::routed_campaign::walking::{mask, power_bounds_json};
use rustred::solver::RoutedCandidateReducer;
use serde::Deserialize;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::cell::Cell;
use std::io;
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Role {
    Required,
    Auxiliary,
}
struct RootDomain(Option<u32>);
impl<'de> Deserialize<'de> for RootDomain {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct DomainVisitor;
        impl<'de> serde::de::Visitor<'de> for DomainVisitor {
            type Value = RootDomain;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an explicit null or u32 root ID")
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(RootDomain(None))
            }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                u32::try_from(value)
                    .map(|id| RootDomain(Some(id)))
                    .map_err(|_| E::custom("root ID range"))
            }
        }
        // deserialize_any deliberately rejects a missing field; transparent
        // Option wrappers would let serde supply None even when domain is absent.
        decoder.deserialize_any(DomainVisitor)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    domain: RootDomain,
    role: Role,
    role_declared: bool,
    source_validity_unresolved: Option<bool>,
    amendment: Option<u64>,
}

pub(super) struct Roots {
    pub rows: Vec<Value>,
    pub frontiers: Vec<Value>,
}

fn phase_for<const N: usize>(
    identity: &Identity<'_>,
    reducer: &RoutedCandidateReducer<N>,
    query: &Query,
    amended: Option<u64>,
) -> Option<Phase> {
    if amended.is_some() {
        crate::application::routed_campaign::walking::rescue::domain::<N>(
            query,
            reducer.programs().owner_sectors().any(|owner| {
                owner.as_slice()
                    == rustred::storage_array::<_, N>(&query.owner, false)
                        .as_ref()
                        .map(|owner| owner.as_slice())
                        .unwrap_or(&[])
            }),
            identity.route_domain_overcover(),
            reducer.domain_routing_requires_source_conditions(),
        )
        .ok()
        .map(|domain| domain.phase)
    } else {
        query_phase(reducer, identity.route_domain_overcover(), query)
    }
}

pub(super) fn read<const N: usize>(
    directory: &Path,
    inputs: &FileRef,
    frontiers: &FileRef,
    identity: &Identity<'_>,
    reducer: &RoutedCandidateReducer<N>,
    store: &Store<N>,
    p0: u32,
) -> io::Result<Roots> {
    read_query_rows(
        directory,
        inputs,
        frontiers,
        &identity.query_rows(identity.amendments().len()),
        store,
        p0,
        |query, amended| phase_for(identity, reducer, query, amended),
    )
}

fn read_with_phase<const N: usize>(
    directory: &Path,
    inputs: &FileRef,
    frontiers: &FileRef,
    queries: &[Query],
    store: &Store<N>,
    p0: u32,
    phase_of: impl Fn(&Query) -> Option<Phase>,
) -> io::Result<Roots> {
    read_query_rows(
        directory,
        inputs,
        frontiers,
        &queries.iter().map(|q| (q, None)).collect::<Vec<_>>(),
        store,
        p0,
        |query, _| phase_of(query),
    )
}

fn read_query_rows<const N: usize>(
    directory: &Path,
    inputs: &FileRef,
    frontiers: &FileRef,
    queries: &[(&Query, Option<u64>)],
    store: &Store<N>,
    p0: u32,
    phase_of: impl Fn(&Query, Option<u64>) -> Option<Phase>,
) -> io::Result<Roots> {
    if N > 32
        || p0 as usize > store.len()
        || inputs.count > queries.len() as u64
        || frontiers.count > inputs.count
        || inputs.count.checked_mul(3).is_none_or(|n| n > inputs.bytes)
        || frontiers
            .count
            .checked_mul(3)
            .is_none_or(|n| n > frontiers.bytes)
    {
        return Err(invalid("epoch query-root inventory shape"));
    }
    let mut input_reader = CheckedRead::open(directory, &inputs.file, inputs.bytes, inputs.blake3)?;
    let mut frontier_reader = CheckedRead::open(
        directory,
        &frontiers.file,
        frontiers.bytes,
        frontiers.blake3,
    )?;
    let input_budget = Cell::new(0);
    let frontier_budget = Cell::new(0);
    let mut input_decoder = serde_json::Deserializer::from_reader(Budget {
        input: &mut input_reader,
        remaining: &input_budget,
        reason: "epoch query row exceeds request-sized bound",
    });
    let mut frontier_decoder = serde_json::Deserializer::from_reader(Budget {
        input: &mut frontier_reader,
        remaining: &frontier_budget,
        reason: "epoch input frontier exceeds request-sized bound",
    });
    let mut result = Roots {
        rows: Vec::new(),
        frontiers: Vec::new(),
    };
    let mut admitted = 0u32;
    let mut counters = VerifyCounters::default();
    for &(query, amendment) in queries.iter().take(inputs.count as usize) {
        if !rustred::fits_storage(query.owner.len(), N)
            || query.lower.len() != query.owner.len()
            || query.upper.len() != query.owner.len()
        {
            return Err(invalid("epoch root query arity"));
        }
        // JSON escapes cost at most6 bytes per input UTF-8 byte. Geometry and
        // u64 coordinates add a fixed bound per axis, not a global ID-size cap.
        let row_bound = (query.id.len() as u64)
            .checked_mul(6)
            .and_then(|n| n.checked_add(256))
            .ok_or_else(|| invalid("epoch query row bound overflow"))?;
        input_budget.set(row_bound);
        let row = Row::deserialize(&mut input_decoder).map_err(io::Error::other)?;
        if row.amendment != amendment {
            return Err(invalid("epoch amendment root order differs"));
        }
        let phase = phase_of(query, amendment);
        let frontier = match (phase, row.domain.0, row.source_validity_unresolved) {
            (None, None, Some(true)) if (result.frontiers.len() as u64) < frontiers.count => {
                frontier_budget.set(
                    row_bound
                        .checked_add(128 * N as u64 + 1024)
                        .ok_or_else(|| invalid("epoch input frontier bound overflow"))?,
                );
                let actual = Value::deserialize(&mut frontier_decoder).map_err(io::Error::other)?;
                Some(actual)
            }
            _ => None,
        };
        check_row(
            query,
            &row,
            phase,
            frontier.as_ref(),
            store,
            p0,
            &mut admitted,
            &mut counters,
        )?;
        if let Some(actual) = frontier {
            result
                .frontiers
                .try_reserve(1)
                .map_err(|_| io::Error::other("epoch input frontier allocation"))?;
            result.frontiers.push(actual);
        }
        let mut value = super::super::super::input_row(query, row.domain.0);
        if let Some(sequence) = amendment {
            value["amendment"] = sequence.into();
        }
        if row.source_validity_unresolved == Some(true) {
            value["source_validity_unresolved"] = true.into();
        }
        result
            .rows
            .try_reserve(1)
            .map_err(|_| io::Error::other("epoch query row allocation"))?;
        result.rows.push(value);
    }
    if admitted != p0 || result.frontiers.len() as u64 != frontiers.count {
        return Err(invalid(
            "epoch root map does not introduce exactly the protected prefix",
        ));
    }
    input_budget.set(256);
    frontier_budget.set(256);
    input_decoder.end().map_err(io::Error::other)?;
    frontier_decoder.end().map_err(io::Error::other)?;
    drop(input_decoder);
    drop(frontier_decoder);
    input_reader.finish()?;
    frontier_reader.finish()?;
    Ok(result)
}

// Same strict rule for decoded disk rows and completion-time in-memory rows.
#[allow(clippy::too_many_arguments)]
fn check_row<const N: usize>(
    query: &Query,
    row: &Row,
    phase: Option<Phase>,
    frontier: Option<&Value>,
    store: &Store<N>,
    p0: u32,
    admitted: &mut u32,
    counters: &mut VerifyCounters,
) -> io::Result<()> {
    if row.amendment.is_some() && phase.is_none() {
        return Err(invalid("epoch amendment has unresolved source validity"));
    }
    if !rustred::fits_storage(query.owner.len(), N)
        || query.lower.len() != query.owner.len()
        || query.upper.len() != query.owner.len()
    {
        return Err(invalid("epoch root query arity"));
    }
    if row.id != query.id
        || matches!(row.role, Role::Auxiliary) != query.auxiliary
        || row.role_declared != query.role_declared
    {
        return Err(invalid("epoch root order or exact query role differs"));
    }
    match (phase, row.domain.0, row.source_validity_unresolved) {
        (Some(phase), Some(id), None) if row.amendment.is_some() && (id as usize) < store.len() => {
            // Query roles come from the immutable, digest-bound amendment:
            // rescue-v1 adds auxiliaries; scope extensions add required rows.
            if !query.role_declared {
                return Err(invalid("epoch amendment lacks an explicit query role"));
            }
            let domain = Domain {
                phase,
                owner: rustred::storage_array(&query.owner, false).expect("arity checked"),
                lower: rustred::storage_array::<_, N>(&query.lower, 0)
                    .expect("arity checked")
                    .to_vec(),
                upper: rustred::storage_array::<_, N>(&query.upper, Some(0))
                    .expect("arity checked")
                    .to_vec(),
                rank: query.rank,
                powers: query.powers,
            };
            let image =
                QueryImage::new(CompactDomain::try_from_domain(&domain).map_err(io::Error::other)?)
                    .map_err(io::Error::other)?;
            if verify(
                Container::Stored {
                    id,
                    domains: &store.domains,
                    published_len: store.len(),
                },
                &image,
                counters,
            )
            .is_none()
            {
                return Err(invalid("epoch amendment root does not contain query"));
            }
        }
        (Some(phase), Some(id), None) if id < p0 && id <= *admitted => {
            let domain = Domain {
                phase,
                owner: rustred::storage_array(&query.owner, false).expect("arity checked"),
                lower: rustred::storage_array::<_, N>(&query.lower, 0)
                    .expect("arity checked")
                    .to_vec(),
                upper: rustred::storage_array::<_, N>(&query.upper, Some(0))
                    .expect("arity checked")
                    .to_vec(),
                rank: query.rank,
                powers: query.powers,
            };
            let image = CompactDomain::try_from_domain(&domain).map_err(io::Error::other)?;
            if id == *admitted {
                if store.domains.get(id as usize) != Some(&image) {
                    return Err(invalid(
                        "epoch first admitting query does not equal its root",
                    ));
                }
                *admitted += 1;
            } else {
                let query_image = QueryImage::new(image).map_err(io::Error::other)?;
                if verify(
                    Container::Stored {
                        id,
                        domains: &store.domains,
                        published_len: *admitted as usize,
                    },
                    &query_image,
                    counters,
                )
                .is_none()
                {
                    return Err(invalid("epoch reused root does not contain its query"));
                }
            }
        }
        (None, None, Some(true)) if frontier == Some(&source_frontier::<N>(query)) => {}
        _ => {
            return Err(invalid(
                "epoch query root phase, prefix or source obligation differs",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate<const N: usize>(
    roots: &Roots,
    identity: &Identity<'_>,
    reducer: &RoutedCandidateReducer<N>,
    store: &Store<N>,
    p0: u32,
) -> io::Result<()> {
    validate_query_rows(
        roots,
        &identity.query_rows(identity.amendments().len()),
        store,
        p0,
        |query, amended| phase_for(identity, reducer, query, amended),
    )
}

fn validate_with_phase<const N: usize>(
    roots: &Roots,
    queries: &[Query],
    store: &Store<N>,
    p0: u32,
    phase_of: impl Fn(&Query) -> Option<Phase>,
) -> io::Result<()> {
    validate_query_rows(
        roots,
        &queries.iter().map(|q| (q, None)).collect::<Vec<_>>(),
        store,
        p0,
        |query, _| phase_of(query),
    )
}

fn validate_query_rows<const N: usize>(
    roots: &Roots,
    queries: &[(&Query, Option<u64>)],
    store: &Store<N>,
    p0: u32,
    phase_of: impl Fn(&Query, Option<u64>) -> Option<Phase>,
) -> io::Result<()> {
    if roots.rows.len() > queries.len() || p0 as usize > store.len() {
        return Err(invalid("epoch query-root inventory shape"));
    }
    let (mut admitted, mut next_frontier) = (0, 0);
    let mut counters = VerifyCounters::default();
    for (value, &(query, amendment)) in roots.rows.iter().zip(queries) {
        let row = Row::deserialize(value).map_err(io::Error::other)?;
        if row.amendment != amendment {
            return Err(invalid("epoch amendment root order differs"));
        }
        let frontier = row
            .domain
            .0
            .is_none()
            .then(|| roots.frontiers.get(next_frontier))
            .flatten();
        check_row(
            query,
            &row,
            phase_of(query, amendment),
            frontier,
            store,
            p0,
            &mut admitted,
            &mut counters,
        )?;
        next_frontier += usize::from(row.domain.0.is_none());
    }
    if admitted != p0 || next_frontier != roots.frontiers.len() {
        return Err(invalid(
            "epoch root map does not introduce exactly the protected prefix",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
