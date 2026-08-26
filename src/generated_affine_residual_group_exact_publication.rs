//! Compact routing prepared together with its sealed exact `WhenBad` owner.
//!
//! This stage performs no algebra and mutates no session.  It consumes the
//! already-sealed publication-ready value and, on success, keeps that value
//! physically paired with one packed routing word per partition leaf.  There
//! is no separately bindable manifest and no replay or schema ceremony on the
//! in-memory hot path.  Runtime authentication belongs at import boundaries
//! and at the later live-session commit boundary.

use std::fmt;
use std::mem::size_of;

use crate::generated_affine_residual_group_exact_when_bad_partition::{
    GeneratedAffineResidualGroupExactWhenBadClauseProvenance,
    GeneratedAffineResidualGroupExactWhenBadClauseSource,
    GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
};
use crate::generated_residual_affine_when_bad::AffineWhenBadArbitraryRelativeCase;

const DEFAULT_MAX_LEAVES: usize = 4_000_001;
const DEFAULT_MAX_ADDITIONAL_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_MAX_COMBINED_PEAK_BYTES: usize = 2 * 1024 * 1024 * 1024;

const TAG_BITS: u32 = 2;
const TAG_MASK: usize = 0b11;
const TAG_APPLICABLE: usize = 0;
const TAG_EXCEPTIONAL_DOMAIN: usize = 1;
const TAG_EXCEPTIONAL_LEAK: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PublicationLimits {
    pub(crate) max_leaves: usize,
    pub(crate) max_additional_retained_bytes: usize,
    pub(crate) max_combined_preparation_peak_bytes: usize,
}

impl Default for PublicationLimits {
    fn default() -> Self {
        Self {
            max_leaves: DEFAULT_MAX_LEAVES,
            max_additional_retained_bytes: DEFAULT_MAX_ADDITIONAL_BYTES,
            max_combined_preparation_peak_bytes: DEFAULT_MAX_COMBINED_PEAK_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PublicationStats {
    leaves: usize,
    applicable: usize,
    exceptional_domain: usize,
    exceptional_leak: usize,
    additional_retained_bytes: usize,
    combined_preparation_peak_bytes: usize,
}

impl PublicationStats {
    pub(crate) const fn leaves(self) -> usize {
        self.leaves
    }

    pub(crate) const fn applicable(self) -> usize {
        self.applicable
    }

    pub(crate) const fn exceptional_domain(self) -> usize {
        self.exceptional_domain
    }

    pub(crate) const fn exceptional_leak(self) -> usize {
        self.exceptional_leak
    }

    pub(crate) const fn exceptional(self) -> usize {
        self.exceptional_domain + self.exceptional_leak
    }

    pub(crate) const fn additional_retained_bytes(self) -> usize {
        self.additional_retained_bytes
    }

    pub(crate) const fn combined_preparation_peak_bytes(self) -> usize {
        self.combined_preparation_peak_bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecodedPublicationRoute {
    Applicable,
    ExceptionalDomain { clause: usize },
    ExceptionalLeak { clause: usize },
}

/// Descriptive leaf view whose exceptional variants retain a borrow from the
/// exact Ready owner.  The raw clause ordinal remains private routing data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationLeafDisposition<'publication> {
    Applicable,
    ExceptionalDomain {
        provenance: &'publication GeneratedAffineResidualGroupExactWhenBadClauseProvenance,
    },
    ExceptionalLeak {
        provenance: &'publication GeneratedAffineResidualGroupExactWhenBadClauseProvenance,
    },
}

/// One private packed word.  The slice index is the corresponding partition
/// leaf/case index, so no case identifier is duplicated here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PublicationRoute(usize);

impl PublicationRoute {
    fn decode(self) -> DecodedPublicationRoute {
        if self.0 == TAG_APPLICABLE {
            return DecodedPublicationRoute::Applicable;
        }
        let clause = self.0 >> TAG_BITS;
        match self.0 & TAG_MASK {
            TAG_EXCEPTIONAL_DOMAIN => DecodedPublicationRoute::ExceptionalDomain { clause },
            TAG_EXCEPTIONAL_LEAK => DecodedPublicationRoute::ExceptionalLeak { clause },
            _ => unreachable!("private publication-route encoding invariant"),
        }
    }
}

/// One owner-borrowed publication leaf.  Raw packed routes never cross the
/// module boundary independently of the Ready value they index.  This is an
/// inspection view; publication authority remains the whole prepared owner.
pub(crate) struct PublicationLeaf<'publication> {
    ordinal: usize,
    case: &'publication AffineWhenBadArbitraryRelativeCase,
    disposition: PublicationLeafDisposition<'publication>,
}

impl<'publication> PublicationLeaf<'publication> {
    pub(crate) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(crate) const fn case(&self) -> &'publication AffineWhenBadArbitraryRelativeCase {
        self.case
    }

    pub(crate) const fn disposition(&self) -> PublicationLeafDisposition<'publication> {
        self.disposition
    }

    pub(crate) const fn provenance(
        &self,
    ) -> Option<&'publication GeneratedAffineResidualGroupExactWhenBadClauseProvenance> {
        match self.disposition {
            PublicationLeafDisposition::Applicable => None,
            PublicationLeafDisposition::ExceptionalDomain { provenance }
            | PublicationLeafDisposition::ExceptionalLeak { provenance } => Some(provenance),
        }
    }
}

/// Move-only preparation result.  Keeping the Ready owner and routes in one
/// value keeps raw route mixing out of the normal API.  The later commit must
/// consume this whole value rather than rebuilding it from inspection views.
pub(crate) struct PreparedPublication {
    ready: GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
    routes: Box<[PublicationRoute]>,
    stats: PublicationStats,
}

impl PreparedPublication {
    pub(crate) fn prepare(
        ready: GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
        limits: PublicationLimits,
    ) -> Result<Self, PublicationFailure> {
        // This path contains no Symbolica/native call or caller callback.
        // Programmer panics are deliberately not converted into recoverable
        // operational errors; doing so would hide bugs behind a retry API.
        match prepare_routes(&ready, limits) {
            Ok((routes, stats)) => Ok(Self {
                ready,
                routes,
                stats,
            }),
            Err(error) => Err(PublicationFailure { error, ready }),
        }
    }

    pub(crate) const fn ready(
        &self,
    ) -> &GeneratedAffineResidualGroupExactWhenBadReadyForPublication {
        &self.ready
    }

    pub(crate) fn leaves(&self) -> impl ExactSizeIterator<Item = PublicationLeaf<'_>> + '_ {
        let cases = self.ready.partition().cases();
        debug_assert_eq!(self.routes.len(), cases.len());
        let provenance = self.ready.clause_provenance();
        self.routes
            .iter()
            .copied()
            .zip(cases)
            .enumerate()
            .map(move |(ordinal, (route, case))| {
                let disposition = match route.decode() {
                    DecodedPublicationRoute::Applicable => PublicationLeafDisposition::Applicable,
                    DecodedPublicationRoute::ExceptionalDomain { clause } => {
                        PublicationLeafDisposition::ExceptionalDomain {
                            provenance: provenance
                                .get(clause)
                                .expect("private publication route must retain valid provenance"),
                        }
                    }
                    DecodedPublicationRoute::ExceptionalLeak { clause } => {
                        PublicationLeafDisposition::ExceptionalLeak {
                            provenance: provenance
                                .get(clause)
                                .expect("private publication route must retain valid provenance"),
                        }
                    }
                };
                PublicationLeaf {
                    ordinal,
                    case,
                    disposition,
                }
            })
    }

    pub(crate) const fn stats(&self) -> PublicationStats {
        self.stats
    }
}

impl fmt::Debug for PreparedPublication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedPublication")
            .field("stats", &self.stats)
            .field("private_ready", &"<redacted>")
            .field("private_routes", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PublicationError {
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        requested_leaves: usize,
    },
    EncodingOverflow {
        clause: usize,
    },
}

impl fmt::Display for PublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "publication resource {resource} requested {requested}, limit {limit}",
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "publication resource count overflow for {resource}"
                )
            }
            Self::AllocationFailure { requested_leaves } => write!(
                formatter,
                "publication allocation failed for {requested_leaves} leaves",
            ),
            Self::EncodingOverflow { clause } => {
                write!(formatter, "publication route cannot encode clause {clause}")
            }
        }
    }
}

impl std::error::Error for PublicationError {}

/// Operational preparation failure retaining the exact move-only input for
/// inspection or retry.
pub(crate) struct PublicationFailure {
    error: PublicationError,
    ready: GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
}

impl PublicationFailure {
    pub(crate) const fn error(&self) -> &PublicationError {
        &self.error
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PublicationError,
        GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
    ) {
        (self.error, self.ready)
    }
}

impl fmt::Debug for PublicationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicationFailure")
            .field("error", &self.error)
            .field("private_ready", &"<redacted>")
            .finish()
    }
}

fn prepare_routes(
    ready: &GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
    limits: PublicationLimits,
) -> Result<(Box<[PublicationRoute]>, PublicationStats), PublicationError> {
    let classifications = ready.partition().classifications();
    let provenance = ready.clause_provenance();
    let leaves = classifications.len();

    assert!(
        leaves > 0,
        "sealed publication-ready partition has no leaves"
    );
    check_limit("leaves", leaves, limits.max_leaves)?;

    let payload_bytes = checked_mul(leaves, size_of::<PublicationRoute>(), "route payload bytes")?;
    let header_delta = size_of::<PreparedPublication>()
        .checked_sub(size_of::<
            GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
        >())
        .ok_or(PublicationError::ResourceCountOverflow {
            resource: "prepared-publication header delta",
        })?;
    let additional_retained_bytes = checked_add(
        header_delta,
        payload_bytes,
        "additional retained publication bytes",
    )?;
    check_limit(
        "additional retained bytes",
        additional_retained_bytes,
        limits.max_additional_retained_bytes,
    )?;

    let ready_bytes = ready.stats().retained_owned_logical_bytes();
    let prepared_retained_bytes = checked_add(
        ready_bytes,
        additional_retained_bytes,
        "combined prepared retained bytes",
    )?;
    let construction_bytes = checked_add(
        ready_bytes,
        checked_add(
            size_of::<Vec<PublicationRoute>>(),
            payload_bytes,
            "route construction bytes",
        )?,
        "combined route construction bytes",
    )?;
    let combined_preparation_peak_bytes = prepared_retained_bytes.max(construction_bytes);
    check_limit(
        "combined preparation peak bytes",
        combined_preparation_peak_bytes,
        limits.max_combined_preparation_peak_bytes,
    )?;

    let mut routes = Vec::new();
    routes
        .try_reserve_exact(leaves)
        .map_err(|_| PublicationError::AllocationFailure {
            requested_leaves: leaves,
        })?;
    if routes.capacity() != leaves {
        return Err(PublicationError::AllocationFailure {
            requested_leaves: leaves,
        });
    }

    let mut applicable = 0usize;
    let mut exceptional_domain = 0usize;
    let mut exceptional_leak = 0usize;
    for classification in classifications {
        let route = match classification.decisive_clause_ordinal() {
            None => {
                applicable = checked_add(applicable, 1, "applicable leaves")?;
                PublicationRoute(TAG_APPLICABLE)
            }
            Some(clause) => {
                let source = provenance
                    .get(clause)
                    .expect("sealed decisive clause must have provenance");
                let tag = exceptional_route_tag(source.source());
                match tag {
                    TAG_EXCEPTIONAL_DOMAIN => {
                        exceptional_domain =
                            checked_add(exceptional_domain, 1, "exceptional-domain leaves")?;
                        PublicationRoute(encode_exceptional(clause, tag)?)
                    }
                    TAG_EXCEPTIONAL_LEAK => {
                        exceptional_leak =
                            checked_add(exceptional_leak, 1, "exceptional-leak leaves")?;
                        PublicationRoute(encode_exceptional(clause, tag)?)
                    }
                    _ => unreachable!("private exceptional-route tag invariant"),
                }
            }
        };
        routes.push(route);
    }

    assert!(
        applicable > 0,
        "sealed publication-ready partition has no applicable leaf"
    );
    debug_assert_eq!(routes.len(), leaves);
    debug_assert_eq!(applicable + exceptional_domain + exceptional_leak, leaves);
    Ok((
        routes.into_boxed_slice(),
        PublicationStats {
            leaves,
            applicable,
            exceptional_domain,
            exceptional_leak,
            additional_retained_bytes,
            combined_preparation_peak_bytes,
        },
    ))
}

fn exceptional_route_tag(source: GeneratedAffineResidualGroupExactWhenBadClauseSource) -> usize {
    match source {
        GeneratedAffineResidualGroupExactWhenBadClauseSource::RecenteredRowGuard { .. }
        | GeneratedAffineResidualGroupExactWhenBadClauseSource::DenominatorIdentity { .. } => {
            TAG_EXCEPTIONAL_DOMAIN
        }
        GeneratedAffineResidualGroupExactWhenBadClauseSource::RetainedBoundary { .. } => {
            TAG_EXCEPTIONAL_LEAK
        }
    }
}

fn encode_exceptional(clause: usize, tag: usize) -> Result<usize, PublicationError> {
    clause
        .checked_mul(1usize << TAG_BITS)
        .and_then(|shifted| shifted.checked_add(tag))
        .ok_or(PublicationError::EncodingOverflow { clause })
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, PublicationError> {
    left.checked_add(right)
        .ok_or(PublicationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, PublicationError> {
    left.checked_mul(right)
        .ok_or(PublicationError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), PublicationError> {
    if requested > limit {
        return Err(PublicationError::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    Ok(())
}

#[cfg(test)]
pub(crate) const fn publication_route_word_bytes_for_test() -> usize {
    size_of::<PublicationRoute>()
}

#[cfg(test)]
pub(crate) fn publication_clause_source_is_domain_for_test(
    source: GeneratedAffineResidualGroupExactWhenBadClauseSource,
) -> bool {
    exceptional_route_tag(source) == TAG_EXCEPTIONAL_DOMAIN
}

#[cfg(test)]
mod encoding_tests {
    use super::*;

    #[test]
    fn packed_route_boundary_is_checked_and_round_trips() {
        let maximum_clause = (usize::MAX - TAG_EXCEPTIONAL_LEAK) / (1usize << TAG_BITS);
        for (tag, expected) in [
            (
                TAG_EXCEPTIONAL_DOMAIN,
                DecodedPublicationRoute::ExceptionalDomain {
                    clause: maximum_clause,
                },
            ),
            (
                TAG_EXCEPTIONAL_LEAK,
                DecodedPublicationRoute::ExceptionalLeak {
                    clause: maximum_clause,
                },
            ),
        ] {
            let encoded = encode_exceptional(maximum_clause, tag).unwrap();
            assert_eq!(PublicationRoute(encoded).decode(), expected);
        }
        assert!(matches!(
            encode_exceptional(maximum_clause + 1, TAG_EXCEPTIONAL_DOMAIN),
            Err(PublicationError::EncodingOverflow { .. })
        ));
    }
}
