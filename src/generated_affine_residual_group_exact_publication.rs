//! Algebra-free distillation of a sealed exact `WhenBad` owner into compact
//! application-event state.
//!
//! Preparation retains the session authority, canonical loci, final relative
//! cases, and one packed outcome tag per partition leaf. It does not mutate a
//! session.

use std::fmt;
use std::mem::size_of;

use crate::ParametricPolynomial;
use crate::generated_affine_residual_group_exact_when_bad_partition::{
    GeneratedAffineResidualGroupExactWhenBadClauseSource,
    GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
};
use crate::generated_residual_affine_when_bad::{
    AffineWhenBadArbitraryRelativeCase, AffineWhenBadArbitraryRelativePredicate,
};
use crate::solver::exact_session::GeneratedAffineResidualGroupExactSessionRecenterReady;

const DEFAULT_MAX_LEAVES: usize = 4_000_001;
const DEFAULT_MAX_ADDITIONAL_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_MAX_COMBINED_PEAK_BYTES: usize = 2 * 1024 * 1024 * 1024;

const TAG_APPLICABLE: u8 = 0;
const TAG_EXCEPTIONAL_DOMAIN: u8 = 1;
const TAG_EXCEPTIONAL_LEAK: u8 = 2;

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
    ExceptionalDomain,
    ExceptionalLeak,
}

/// The application-only outcome for one partition leaf.  Derivation
/// transcripts and derivation-local source ordinals are intentionally absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationLeafDisposition {
    Applicable,
    ExceptionalDomain,
    ExceptionalLeak,
}

/// One private packed tag.  The slice index is the corresponding partition
/// leaf/case index, so no case identifier is duplicated here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PublicationRoute(u8);

impl PublicationRoute {
    fn decode(self) -> DecodedPublicationRoute {
        match self.0 {
            TAG_APPLICABLE => DecodedPublicationRoute::Applicable,
            TAG_EXCEPTIONAL_DOMAIN => DecodedPublicationRoute::ExceptionalDomain,
            TAG_EXCEPTIONAL_LEAK => DecodedPublicationRoute::ExceptionalLeak,
            _ => unreachable!("private publication-route encoding invariant"),
        }
    }
}

/// One owner-borrowed publication leaf.  Raw packed routes never cross the
/// module boundary independently of the Ready value they index.  This is an
/// inspection view; publication authority remains the whole prepared owner.
#[derive(Clone, Copy)]
pub(crate) struct PublicationLeaf<'publication> {
    ordinal: usize,
    case: &'publication AffineWhenBadArbitraryRelativeCase,
    disposition: PublicationLeafDisposition,
}

impl<'publication> PublicationLeaf<'publication> {
    pub(crate) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(crate) const fn case(&self) -> &'publication AffineWhenBadArbitraryRelativeCase {
        self.case
    }

    pub(crate) const fn disposition(&self) -> PublicationLeafDisposition {
        self.disposition
    }
}

/// Compact event payload after every derivation-only shell has been discarded.
/// Application needs only the source-neutral relative partition and one
/// directly encoded outcome per leaf.
pub(crate) struct PublicationPayload {
    loci: Vec<ParametricPolynomial>,
    cases: Vec<AffineWhenBadArbitraryRelativeCase>,
    routes: Box<[PublicationRoute]>,
}

impl PublicationPayload {
    pub(crate) fn loci(&self) -> &[ParametricPolynomial] {
        &self.loci
    }

    pub(crate) fn cases(&self) -> &[AffineWhenBadArbitraryRelativeCase] {
        &self.cases
    }

    pub(crate) fn deep_owned_retained_byte_bound(&self) -> Result<usize, PublicationError> {
        compact_payload_deep_owned_retained_byte_bound(
            &self.loci,
            self.loci.capacity(),
            &self.cases,
            self.cases.capacity(),
            self.routes.len(),
        )
    }

    pub(crate) fn leaf(&self, ordinal: usize) -> Option<PublicationLeaf<'_>> {
        let route = self.routes.get(ordinal)?.decode();
        let case = self.cases.get(ordinal)?;
        let disposition = match route {
            DecodedPublicationRoute::Applicable => PublicationLeafDisposition::Applicable,
            DecodedPublicationRoute::ExceptionalDomain => {
                PublicationLeafDisposition::ExceptionalDomain
            }
            DecodedPublicationRoute::ExceptionalLeak => PublicationLeafDisposition::ExceptionalLeak,
        };
        Some(PublicationLeaf {
            ordinal,
            case,
            disposition,
        })
    }

    pub(crate) fn leaves(&self) -> impl ExactSizeIterator<Item = PublicationLeaf<'_>> + '_ {
        debug_assert_eq!(self.routes.len(), self.cases.len());
        (0..self.routes.len()).map(|ordinal| {
            self.leaf(ordinal)
                .expect("private publication leaf/case lengths diverged")
        })
    }
}

impl fmt::Debug for PublicationPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicationPayload")
            .field("loci", &self.loci.len())
            .field("cases", &self.cases.len())
            .field("private_routes", &"<redacted>")
            .finish()
    }
}

/// Move-only preparation result.  The session Ready authority and compact
/// event payload remain physically paired.  The later commit consumes this
/// whole value; inspection views carry no publication authority.
pub(crate) struct PreparedPublication {
    ready: GeneratedAffineResidualGroupExactSessionRecenterReady,
    payload: PublicationPayload,
    pivot_term_ordinal: usize,
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
            Ok((routes, stats)) => {
                let pivot_term_ordinal = ready.pivot_term_ordinal();
                let expected_payload_deep =
                    match compact_ready_payload_deep_owned_retained_byte_bound(&ready, routes.len())
                    {
                        Ok(bytes) => bytes,
                        Err(error) => return Err(PublicationFailure { error, ready }),
                    };
                let (materialized, partition) = ready.into_publication_parts();
                let (loci, cases) = partition.into_application_parts();
                let plan = materialized.into_condition_plan_for_publication();
                let ready = plan.into_ready().into_ready();
                let payload = PublicationPayload {
                    loci,
                    cases,
                    routes,
                };
                debug_assert_eq!(
                    payload.deep_owned_retained_byte_bound(),
                    Ok(expected_payload_deep)
                );
                Ok(Self {
                    ready,
                    payload,
                    pivot_term_ordinal,
                    stats,
                })
            }
            Err(error) => Err(PublicationFailure { error, ready }),
        }
    }

    pub(crate) const fn ready(&self) -> &GeneratedAffineResidualGroupExactSessionRecenterReady {
        &self.ready
    }

    pub(crate) fn leaves(&self) -> impl ExactSizeIterator<Item = PublicationLeaf<'_>> + '_ {
        self.payload.leaves()
    }

    pub(crate) const fn stats(&self) -> PublicationStats {
        self.stats
    }

    pub(crate) const fn pivot_term_ordinal(&self) -> usize {
        self.pivot_term_ordinal
    }

    pub(crate) const fn payload(&self) -> &PublicationPayload {
        &self.payload
    }

    pub(crate) fn into_parts_for_session(
        self,
    ) -> (
        GeneratedAffineResidualGroupExactSessionRecenterReady,
        PublicationPayload,
        usize,
    ) {
        (self.ready, self.payload, self.pivot_term_ordinal)
    }
}

impl fmt::Debug for PreparedPublication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedPublication")
            .field("stats", &self.stats)
            .field("private_ready", &"<redacted>")
            .field("private_payload", &"<redacted>")
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

fn compact_ready_payload_deep_owned_retained_byte_bound(
    ready: &GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
    route_len: usize,
) -> Result<usize, PublicationError> {
    compact_payload_deep_owned_retained_byte_bound(
        ready.partition().structural_loci(),
        ready.partition().structural_loci_capacity(),
        ready.partition().cases(),
        ready.partition().cases_capacity(),
        route_len,
    )
}

fn compact_payload_deep_owned_retained_byte_bound(
    loci: &[ParametricPolynomial],
    locus_capacity: usize,
    cases: &[AffineWhenBadArbitraryRelativeCase],
    case_capacity: usize,
    route_len: usize,
) -> Result<usize, PublicationError> {
    let mut bytes = 0usize;
    bytes = checked_add(
        bytes,
        checked_mul(
            locus_capacity,
            size_of::<ParametricPolynomial>(),
            "compact publication locus buffer bytes",
        )?,
        "compact publication payload bytes",
    )?;
    for locus in loci {
        let deep = locus
            .owned_retained_byte_bound()
            .and_then(|bound| bound.checked_sub(size_of::<ParametricPolynomial>()))
            .ok_or(PublicationError::ResourceCountOverflow {
                resource: "compact publication locus payload bytes",
            })?;
        bytes = checked_add(bytes, deep, "compact publication payload bytes")?;
    }
    bytes = checked_add(
        bytes,
        checked_mul(
            case_capacity,
            size_of::<AffineWhenBadArbitraryRelativeCase>(),
            "compact publication case buffer bytes",
        )?,
        "compact publication payload bytes",
    )?;
    for case in cases {
        bytes = checked_add(
            bytes,
            checked_mul(
                case.predicate_capacity(),
                size_of::<AffineWhenBadArbitraryRelativePredicate>(),
                "compact publication predicate buffer bytes",
            )?,
            "compact publication payload bytes",
        )?;
    }
    checked_add(
        bytes,
        checked_mul(
            route_len,
            size_of::<PublicationRoute>(),
            "compact publication route payload bytes",
        )?,
        "compact publication payload bytes",
    )
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
    let header_delta = size_of::<PreparedPublication>().saturating_sub(size_of::<
        GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
    >());
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
    let cases = ready.partition().cases();
    assert_eq!(
        classifications.len(),
        cases.len(),
        "sealed publication classifications and cases diverged"
    );
    for (classification, case) in classifications.iter().zip(cases) {
        assert_eq!(
            classification.case(),
            case.id(),
            "sealed publication route lost its case binding"
        );
        let route = match classification.decisive_clause_ordinal() {
            None => {
                applicable = checked_add(applicable, 1, "applicable leaves")?;
                PublicationRoute(TAG_APPLICABLE)
            }
            Some(clause) => {
                let source = provenance
                    .get(clause)
                    .expect("sealed decisive clause must have provenance");
                match source.source() {
                    GeneratedAffineResidualGroupExactWhenBadClauseSource::RecenteredRowGuard {
                        ..
                    }
                    | GeneratedAffineResidualGroupExactWhenBadClauseSource::DenominatorIdentity {
                        ..
                    } => {
                        exceptional_domain =
                            checked_add(exceptional_domain, 1, "exceptional-domain leaves")?;
                        PublicationRoute(TAG_EXCEPTIONAL_DOMAIN)
                    }
                    GeneratedAffineResidualGroupExactWhenBadClauseSource::RetainedBoundary {
                        ..
                    } => {
                        exceptional_leak =
                            checked_add(exceptional_leak, 1, "exceptional-leak leaves")?;
                        PublicationRoute(TAG_EXCEPTIONAL_LEAK)
                    }
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
pub(crate) const fn publication_route_tag_bytes_for_test() -> usize {
    size_of::<PublicationRoute>()
}

#[cfg(test)]
mod encoding_tests {
    use super::*;

    #[test]
    fn packed_route_tags_round_trip() {
        for (tag, expected) in [
            (TAG_APPLICABLE, DecodedPublicationRoute::Applicable),
            (
                TAG_EXCEPTIONAL_DOMAIN,
                DecodedPublicationRoute::ExceptionalDomain,
            ),
            (
                TAG_EXCEPTIONAL_LEAK,
                DecodedPublicationRoute::ExceptionalLeak,
            ),
        ] {
            assert_eq!(PublicationRoute(tag).decode(), expected);
        }
    }
}
