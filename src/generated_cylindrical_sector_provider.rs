//! Concrete application of generated cylindrical sector coverage.
//!
//! This is the anchor-free counterpart of the legacy parametric sector
//! provider.  It owns one fully replayed, product-free cylindrical coverage
//! certificate per installed sector and publishes only concrete reductions
//! selected by that coverage.  It performs no identity search, master
//! inference, topology dispatch, or loop-count dispatch.
//!
//! A successful query crosses three independent authority boundaries: the
//! global MTBDD selects an ordinal, the coverage certificate checks the exact
//! candidate-local `WhenBad` leaf, and
//! [`ConcreteReduction::apply_generated_cylindrical`] checks that local leaf
//! again before specializing the retained identity.  The selected proof is an
//! `Arc`, so query application never deep-clones a `WhenBad` certificate.

use std::cmp::Ordering;
use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus};
use crate::{
    ConcreteIntegralKey, ConcreteReduction, GeneratedCylindricalSectorCoverageCertificate,
    GeneratedCylindricalSectorCoverageError, GeneratedCylindricalSectorLeafDisposition,
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext, ParametricRuleError,
    SectorFoundationError, SectorMask,
};

/// Stable schema for the generated cylindrical coverage/application bridge.
pub const GENERATED_CYLINDRICAL_SECTOR_RULE_PROVIDER_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-sector-rule-provider-v1";

/// Aggregate retained-proof and concrete-query budgets.
///
/// Coverage-owned policies remain independently binding on every certificate.
/// These fields cap the sum retained by one provider, so installing many
/// individually admissible certificates cannot bypass a family-wide budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalSectorRuleProviderLimits {
    pub max_sector_certificates: usize,
    pub max_certificate_slot_bytes: usize,
    pub max_total_family_fingerprint_bytes: usize,
    pub max_total_context_fingerprint_bytes: usize,
    pub max_total_sector_mask_bytes: usize,
    pub max_total_attempts: usize,
    pub max_total_certified_attempts: usize,
    pub max_total_unsupported_attempts: usize,
    pub max_total_attempt_arc_reference_bytes: usize,
    pub max_total_unique_persistent_source_references: usize,
    pub max_total_candidate_retained_payload_bytes: usize,
    pub max_total_persistent_source_retained_bytes: usize,
    pub max_total_when_bad_binding_bytes: usize,
    pub max_total_when_bad_retained_core_bytes: usize,
    pub max_total_when_bad_guard_origin_retained_bytes: usize,
    pub max_total_when_bad_condition_terms: usize,
    pub max_total_when_bad_condition_bytes: usize,
    pub max_total_when_bad_leak_event_retained_bytes: usize,
    pub max_total_base_structural_loci: usize,
    pub max_total_base_structural_locus_terms: usize,
    pub max_total_base_structural_locus_bytes: usize,
    pub max_total_normalized_clauses: usize,
    pub max_total_normalized_literals: usize,
    pub max_total_normalized_clause_source_references: usize,
    pub max_total_normalized_factor_references: usize,
    pub max_total_decision_atoms: usize,
    pub max_total_decision_nodes: usize,
    pub max_total_decision_terminals: usize,
    pub max_queries: usize,
    pub max_unsupported_ordinals_per_query: usize,
}

impl Default for GeneratedCylindricalSectorRuleProviderLimits {
    fn default() -> Self {
        Self {
            max_sector_certificates: 1_000_000,
            max_certificate_slot_bytes: portable_limit(512u128 * 1024 * 1024),
            max_total_family_fingerprint_bytes: 64 * 1024 * 1024,
            max_total_context_fingerprint_bytes: 64 * 1024 * 1024,
            max_total_sector_mask_bytes: 128 * 1024 * 1024,
            max_total_attempts: 16_000_000,
            max_total_certified_attempts: 16_000_000,
            max_total_unsupported_attempts: 16_000_000,
            max_total_attempt_arc_reference_bytes: portable_limit(2u128 * 1024 * 1024 * 1024),
            max_total_unique_persistent_source_references: 16_000_000,
            max_total_candidate_retained_payload_bytes: portable_limit(16u128 * 1024 * 1024 * 1024),
            max_total_persistent_source_retained_bytes: portable_limit(64u128 * 1024 * 1024 * 1024),
            max_total_when_bad_binding_bytes: portable_limit(8u128 * 1024 * 1024 * 1024),
            max_total_when_bad_retained_core_bytes: portable_limit(64u128 * 1024 * 1024 * 1024),
            max_total_when_bad_guard_origin_retained_bytes: portable_limit(
                32u128 * 1024 * 1024 * 1024,
            ),
            max_total_when_bad_condition_terms: 256_000_000,
            max_total_when_bad_condition_bytes: portable_limit(32u128 * 1024 * 1024 * 1024),
            max_total_when_bad_leak_event_retained_bytes: portable_limit(
                32u128 * 1024 * 1024 * 1024,
            ),
            max_total_base_structural_loci: 128_000_000,
            max_total_base_structural_locus_terms: 256_000_000,
            max_total_base_structural_locus_bytes: portable_limit(32u128 * 1024 * 1024 * 1024),
            max_total_normalized_clauses: 256_000_000,
            max_total_normalized_literals: 512_000_000,
            max_total_normalized_clause_source_references: 512_000_000,
            max_total_normalized_factor_references: 512_000_000,
            max_total_decision_atoms: 128_000_000,
            max_total_decision_nodes: 256_000_000,
            max_total_decision_terminals: 128_000_000,
            max_queries: 100_000_000,
            max_unsupported_ordinals_per_query: 16_000_000,
        }
    }
}

/// Immutable aggregate census of the exact certificate payloads charged to
/// one provider.
///
/// Nested source allocations may be shared by `Arc`.  Their coverage-local
/// censuses are deliberately summed conservatively here; the resulting byte
/// fields are safe charged upper bounds rather than claims that every nested
/// allocation is distinct across sectors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedCylindricalSectorRuleProviderBuildStats {
    sector_certificates: usize,
    certificate_slot_bytes: usize,
    family_fingerprint_bytes: usize,
    context_fingerprint_bytes: usize,
    sector_mask_bytes: usize,
    attempts: usize,
    certified_attempts: usize,
    unsupported_attempts: usize,
    attempt_arc_reference_bytes: usize,
    unique_persistent_source_references: usize,
    candidate_retained_payload_bytes: usize,
    persistent_source_retained_bytes: usize,
    when_bad_binding_bytes: usize,
    when_bad_retained_core_bytes: usize,
    when_bad_guard_origin_retained_bytes: usize,
    when_bad_condition_terms: usize,
    when_bad_condition_bytes: usize,
    when_bad_leak_event_retained_bytes: usize,
    base_structural_loci: usize,
    base_structural_locus_terms: usize,
    base_structural_locus_bytes: usize,
    normalized_clauses: usize,
    normalized_literals: usize,
    normalized_clause_source_references: usize,
    normalized_factor_references: usize,
    decision_atoms: usize,
    decision_nodes: usize,
    decision_terminals: usize,
}

macro_rules! build_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedCylindricalSectorRuleProviderBuildStats {
    build_stats_getters!(
        sector_certificates,
        certificate_slot_bytes,
        family_fingerprint_bytes,
        context_fingerprint_bytes,
        sector_mask_bytes,
        attempts,
        certified_attempts,
        unsupported_attempts,
        attempt_arc_reference_bytes,
        unique_persistent_source_references,
        candidate_retained_payload_bytes,
        persistent_source_retained_bytes,
        when_bad_binding_bytes,
        when_bad_retained_core_bytes,
        when_bad_guard_origin_retained_bytes,
        when_bad_condition_terms,
        when_bad_condition_bytes,
        when_bad_leak_event_retained_bytes,
        base_structural_loci,
        base_structural_locus_terms,
        base_structural_locus_bytes,
        normalized_clauses,
        normalized_literals,
        normalized_clause_source_references,
        normalized_factor_references,
        decision_atoms,
        decision_nodes,
        decision_terminals,
    );
}

/// Transactional runtime decision census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedCylindricalSectorRuleProviderStats {
    queries: usize,
    reductions: usize,
    uncovered: usize,
    unsupported: usize,
}

impl GeneratedCylindricalSectorRuleProviderStats {
    pub const fn queries(self) -> usize {
        self.queries
    }

    pub const fn reductions(self) -> usize {
        self.reductions
    }

    pub const fn uncovered(self) -> usize {
        self.uncovered
    }

    pub const fn unsupported(self) -> usize {
        self.unsupported
    }
}

/// Topology- and loop-count-independent provider for product-free cylindrical
/// sector coverage.
pub struct GeneratedCylindricalSectorRuleProvider<'family> {
    schema: &'static str,
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    ordering_policy: IntegralOrderingPolicy,
    // Kept as a fallibly reserved sorted vector. A `BTreeMap` would require
    // infallible node allocation and a second owned sector mask per entry.
    certificates: Vec<GeneratedCylindricalSectorCoverageCertificate>,
    limits: GeneratedCylindricalSectorRuleProviderLimits,
    build_stats: GeneratedCylindricalSectorRuleProviderBuildStats,
    stats: GeneratedCylindricalSectorRuleProviderStats,
}

impl<'family> GeneratedCylindricalSectorRuleProvider<'family> {
    pub const SCHEMA: &'static str = GENERATED_CYLINDRICAL_SECTOR_RULE_PROVIDER_V1_SCHEMA;

    /// Validate, replay, sort, and take ownership of complete coverage
    /// certificates. No cross-certificate shared-row-span premise is made.
    pub fn try_new(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        ordering_policy: IntegralOrderingPolicy,
        certificates: impl IntoIterator<Item = GeneratedCylindricalSectorCoverageCertificate>,
        limits: GeneratedCylindricalSectorRuleProviderLimits,
    ) -> Result<Self, GeneratedCylindricalSectorRuleProviderError> {
        validate_family_context(family, context)?;

        let mut retained = Vec::new();
        let mut build_stats = GeneratedCylindricalSectorRuleProviderBuildStats::default();
        for certificate in certificates {
            let requested = checked_add(
                "generated cylindrical sector certificates",
                retained.len(),
                1,
            )?;
            check_limit(
                "generated cylindrical sector certificates",
                requested,
                limits.max_sector_certificates,
            )?;
            validate_certificate_scope(family, context, ordering_policy, &certificate)?;
            census_certificate(&mut build_stats, &certificate, limits)?;
            let minimum_slot_bytes = checked_mul(
                "generated cylindrical provider certificate slot bytes",
                requested,
                size_of::<GeneratedCylindricalSectorCoverageCertificate>(),
            )?;
            check_limit(
                "generated cylindrical provider certificate slot bytes",
                minimum_slot_bytes,
                limits.max_certificate_slot_bytes,
            )?;
            try_reserve_exact(
                "generated cylindrical provider certificate slots",
                &mut retained,
                1,
            )?;
            let slot_bytes = checked_mul(
                "generated cylindrical provider certificate slot bytes",
                retained.capacity(),
                size_of::<GeneratedCylindricalSectorCoverageCertificate>(),
            )?;
            check_limit(
                "generated cylindrical provider certificate slot bytes",
                slot_bytes,
                limits.max_certificate_slot_bytes,
            )?;
            retained.push(certificate);
        }
        build_stats.certificate_slot_bytes = checked_mul(
            "generated cylindrical provider certificate slot bytes",
            retained.capacity(),
            size_of::<GeneratedCylindricalSectorCoverageCertificate>(),
        )?;

        retained.sort_unstable_by(|left, right| left.sector().cmp(right.sector()));
        for pair in retained.windows(2) {
            if pair[0].sector() == pair[1].sector() {
                return Err(
                    GeneratedCylindricalSectorRuleProviderError::DuplicateSector {
                        sector: copy_sector(pair[0].sector())?,
                    },
                );
            }
        }

        // Every aggregate retention bound and duplicate-sector check has
        // succeeded before the first potentially expensive proof replay.
        for certificate in &retained {
            certificate.replay(family, context)?;
        }

        Ok(Self {
            schema: GENERATED_CYLINDRICAL_SECTOR_RULE_PROVIDER_V1_SCHEMA,
            family,
            context,
            ordering_policy,
            certificates: retained,
            limits,
            build_stats,
            stats: GeneratedCylindricalSectorRuleProviderStats::default(),
        })
    }

    pub const fn family(&self) -> &IntegralFamily {
        self.family
    }

    pub const fn context(&self) -> &ParametricCoefficientContext {
        self.context
    }

    pub const fn ordering_policy(&self) -> IntegralOrderingPolicy {
        self.ordering_policy
    }

    pub fn certificates(&self) -> &[GeneratedCylindricalSectorCoverageCertificate] {
        &self.certificates
    }

    pub fn certificate_for_sector(
        &self,
        sector: &SectorMask,
    ) -> Option<&GeneratedCylindricalSectorCoverageCertificate> {
        self.certificates
            .binary_search_by(|certificate| certificate.sector().cmp(sector))
            .ok()
            .map(|ordinal| &self.certificates[ordinal])
    }

    pub const fn limits(&self) -> GeneratedCylindricalSectorRuleProviderLimits {
        self.limits
    }

    pub const fn build_stats(&self) -> GeneratedCylindricalSectorRuleProviderBuildStats {
        self.build_stats
    }

    pub const fn stats(&self) -> GeneratedCylindricalSectorRuleProviderStats {
        self.stats
    }

    /// Replay every retained coverage proof and recompute the complete
    /// provider-owned aggregate census. Runtime query statistics are ignored.
    pub fn replay(&self) -> Result<(), GeneratedCylindricalSectorRuleProviderError> {
        if self.schema != GENERATED_CYLINDRICAL_SECTOR_RULE_PROVIDER_V1_SCHEMA {
            return Err(GeneratedCylindricalSectorRuleProviderError::SchemaMismatch);
        }
        validate_family_context(self.family, self.context)?;

        let mut replayed_stats = GeneratedCylindricalSectorRuleProviderBuildStats::default();
        replayed_stats.certificate_slot_bytes = checked_mul(
            "generated cylindrical provider certificate slot bytes",
            self.certificates.capacity(),
            size_of::<GeneratedCylindricalSectorCoverageCertificate>(),
        )?;
        check_limit(
            "generated cylindrical provider certificate slot bytes",
            replayed_stats.certificate_slot_bytes,
            self.limits.max_certificate_slot_bytes,
        )?;

        let mut previous: Option<&SectorMask> = None;
        for certificate in &self.certificates {
            validate_certificate_scope(
                self.family,
                self.context,
                self.ordering_policy,
                certificate,
            )?;
            if let Some(previous_sector) = previous {
                match previous_sector.cmp(certificate.sector()) {
                    Ordering::Less => {}
                    Ordering::Equal => {
                        return Err(
                            GeneratedCylindricalSectorRuleProviderError::DuplicateSector {
                                sector: copy_sector(certificate.sector())?,
                            },
                        );
                    }
                    Ordering::Greater => {
                        return Err(
                            GeneratedCylindricalSectorRuleProviderError::ReplayMismatch {
                                detail: "retained sector certificates are not sorted",
                            },
                        );
                    }
                }
            }
            census_certificate(&mut replayed_stats, certificate, self.limits)?;
            certificate.replay(self.family, self.context)?;
            previous = Some(certificate.sector());
        }
        if replayed_stats != self.build_stats {
            return Err(
                GeneratedCylindricalSectorRuleProviderError::ReplayMismatch {
                    detail: "aggregate provider build census differs",
                },
            );
        }
        Ok(())
    }

    fn coverage_for_indices(
        &self,
        indices: &[i64],
    ) -> Option<&GeneratedCylindricalSectorCoverageCertificate> {
        self.certificates
            .binary_search_by(|certificate| {
                compare_sector_to_indices(certificate.sector(), indices)
            })
            .ok()
            .map(|ordinal| &self.certificates[ordinal])
    }

    fn decide(
        &self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, GeneratedCylindricalSectorRuleProviderError> {
        if integral.powers().len() != self.context.index_count() {
            return Err(GeneratedCylindricalSectorRuleProviderError::WrongArity {
                expected: self.context.index_count(),
                actual: integral.powers().len(),
            });
        }
        let Some(coverage) = self.coverage_for_indices(integral.powers()) else {
            return Ok(ConcreteRuleDecision::Terminal(
                ConcreteTerminalStatus::Uncovered,
            ));
        };
        let classification =
            match coverage.classification_for_indices(self.context, integral.powers())? {
                Some(classification) => classification,
                None => {
                    return Err(
                        GeneratedCylindricalSectorRuleProviderError::CoveragePointMissing {
                            sector: copy_sector(coverage.sector())?,
                        },
                    );
                }
            };

        match classification {
            GeneratedCylindricalSectorLeafDisposition::DescendingRule {
                candidate_ordinal,
                candidate,
            } => {
                let retained = coverage.selected_candidate(candidate_ordinal).ok_or(
                    GeneratedCylindricalSectorRuleProviderError::CoverageSelectionMismatch {
                        ordinal: candidate_ordinal,
                    },
                )?;
                if !Arc::ptr_eq(retained, candidate) {
                    return Err(
                        GeneratedCylindricalSectorRuleProviderError::CoverageSelectionMismatch {
                            ordinal: candidate_ordinal,
                        },
                    );
                }
                let reduction = ConcreteReduction::apply_generated_cylindrical(
                    Arc::clone(candidate),
                    self.context,
                    integral.powers(),
                )?;
                Ok(ConcreteRuleDecision::Reduction(reduction))
            }
            GeneratedCylindricalSectorLeafDisposition::Uncovered => Ok(
                ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered),
            ),
            GeneratedCylindricalSectorLeafDisposition::Unsupported { candidate_ordinals } => {
                check_limit(
                    "unsupported candidate ordinals per cylindrical provider query",
                    candidate_ordinals.len(),
                    self.limits.max_unsupported_ordinals_per_query,
                )?;
                let mut retained_ordinals = Vec::new();
                try_reserve_exact(
                    "unsupported candidate ordinals for cylindrical provider query",
                    &mut retained_ordinals,
                    candidate_ordinals.len(),
                )?;
                retained_ordinals.extend_from_slice(candidate_ordinals);
                Err(
                    GeneratedCylindricalSectorRuleProviderError::UnsupportedLeaf {
                        sector: copy_sector(coverage.sector())?,
                        candidate_ordinals: retained_ordinals,
                    },
                )
            }
        }
    }
}

impl ConcreteRuleProvider for GeneratedCylindricalSectorRuleProvider<'_> {
    type Error = GeneratedCylindricalSectorRuleProviderError;

    fn index_arity(&self) -> usize {
        self.context.index_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        let queries = checked_add(
            "generated cylindrical sector provider queries",
            self.stats.queries,
            1,
        )?;
        check_limit(
            "generated cylindrical sector provider queries",
            queries,
            self.limits.max_queries,
        )?;

        let decision = self.decide(integral);
        let mut next_stats = self.stats;
        let commit = match &decision {
            Ok(ConcreteRuleDecision::Reduction(_)) => {
                next_stats.queries = queries;
                next_stats.reductions = checked_add(
                    "generated cylindrical sector provider reductions",
                    next_stats.reductions,
                    1,
                )?;
                true
            }
            Ok(ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)) => {
                next_stats.queries = queries;
                next_stats.uncovered = checked_add(
                    "generated cylindrical sector provider uncovered decisions",
                    next_stats.uncovered,
                    1,
                )?;
                true
            }
            Err(GeneratedCylindricalSectorRuleProviderError::UnsupportedLeaf { .. }) => {
                next_stats.queries = queries;
                next_stats.unsupported = checked_add(
                    "generated cylindrical sector provider unsupported decisions",
                    next_stats.unsupported,
                    1,
                )?;
                true
            }
            _ => false,
        };
        if commit {
            self.stats = next_stats;
        }
        decision
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalSectorRuleProviderError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongOrdering {
        expected: IntegralOrderingPolicy,
        actual: IntegralOrderingPolicy,
    },
    WrongArity {
        expected: usize,
        actual: usize,
    },
    DuplicateSector {
        sector: SectorMask,
    },
    CoveragePointMissing {
        sector: SectorMask,
    },
    CoverageSelectionMismatch {
        ordinal: usize,
    },
    UnsupportedLeaf {
        sector: SectorMask,
        candidate_ordinals: Vec<usize>,
    },
    ReplayMismatch {
        detail: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Coverage(GeneratedCylindricalSectorCoverageError),
    Rule(ParametricRuleError),
    Sector(SectorFoundationError),
}

impl fmt::Display for GeneratedCylindricalSectorRuleProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("generated cylindrical sector provider schema mismatch")
            }
            Self::WrongFamily => {
                formatter.write_str("generated cylindrical sector provider family mismatch")
            }
            Self::WrongContext => {
                formatter.write_str("generated cylindrical sector provider context mismatch")
            }
            Self::WrongOrdering { expected, actual } => write!(
                formatter,
                "generated cylindrical sector provider ordering is {actual:?}, expected {expected:?}"
            ),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "generated cylindrical sector provider arity is {actual}, expected {expected}"
            ),
            Self::DuplicateSector { sector } => {
                write!(
                    formatter,
                    "duplicate generated cylindrical coverage for {sector}"
                )
            }
            Self::CoveragePointMissing { sector } => write!(
                formatter,
                "generated cylindrical coverage for {sector} did not classify its own integer point"
            ),
            Self::CoverageSelectionMismatch { ordinal } => write!(
                formatter,
                "generated cylindrical coverage selected a detached or non-certified candidate {ordinal}"
            ),
            Self::UnsupportedLeaf {
                sector,
                candidate_ordinals,
            } => write!(
                formatter,
                "generated cylindrical sector {sector} remains unsupported after candidates {candidate_ordinals:?}"
            ),
            Self::ReplayMismatch { detail } => {
                write!(
                    formatter,
                    "generated cylindrical sector provider replay mismatch: {detail}"
                )
            }
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "generated cylindrical sector provider {resource} count overflowed usize"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "generated cylindrical sector provider {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "generated cylindrical sector provider could not allocate {requested} entries for {resource}"
            ),
            Self::Coverage(error) => error.fmt(formatter),
            Self::Rule(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedCylindricalSectorRuleProviderError {}

impl From<GeneratedCylindricalSectorCoverageError> for GeneratedCylindricalSectorRuleProviderError {
    fn from(value: GeneratedCylindricalSectorCoverageError) -> Self {
        Self::Coverage(value)
    }
}

impl From<ParametricRuleError> for GeneratedCylindricalSectorRuleProviderError {
    fn from(value: ParametricRuleError) -> Self {
        Self::Rule(value)
    }
}

impl From<SectorFoundationError> for GeneratedCylindricalSectorRuleProviderError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

fn census_certificate(
    aggregate: &mut GeneratedCylindricalSectorRuleProviderBuildStats,
    certificate: &GeneratedCylindricalSectorCoverageCertificate,
    limits: GeneratedCylindricalSectorRuleProviderLimits,
) -> Result<(), GeneratedCylindricalSectorRuleProviderError> {
    aggregate.sector_certificates = bounded_add(
        "generated cylindrical sector certificates",
        aggregate.sector_certificates,
        1,
        limits.max_sector_certificates,
    )?;
    aggregate.family_fingerprint_bytes = bounded_add(
        "generated cylindrical coverage family fingerprint bytes",
        aggregate.family_fingerprint_bytes,
        certificate.family_fingerprint().len(),
        limits.max_total_family_fingerprint_bytes,
    )?;
    aggregate.context_fingerprint_bytes = bounded_add(
        "generated cylindrical coverage context fingerprint bytes",
        aggregate.context_fingerprint_bytes,
        certificate.context_fingerprint().len(),
        limits.max_total_context_fingerprint_bytes,
    )?;
    let sector_bytes = certificate.sector().owned_retained_byte_bound().ok_or(
        GeneratedCylindricalSectorRuleProviderError::ResourceCountOverflow {
            resource: "generated cylindrical coverage sector-mask bytes",
        },
    )?;
    aggregate.sector_mask_bytes = bounded_add(
        "generated cylindrical coverage sector-mask bytes",
        aggregate.sector_mask_bytes,
        sector_bytes,
        limits.max_total_sector_mask_bytes,
    )?;

    let stats = certificate.stats();
    macro_rules! charge {
        ($field:ident, $getter:ident, $limit:ident, $resource:literal) => {
            aggregate.$field =
                bounded_add($resource, aggregate.$field, stats.$getter(), limits.$limit)?;
        };
    }
    charge!(
        attempts,
        attempts,
        max_total_attempts,
        "generated cylindrical coverage attempts"
    );
    charge!(
        certified_attempts,
        certified_attempts,
        max_total_certified_attempts,
        "generated cylindrical certified attempts"
    );
    charge!(
        unsupported_attempts,
        unsupported_attempts,
        max_total_unsupported_attempts,
        "generated cylindrical unsupported attempts"
    );
    charge!(
        attempt_arc_reference_bytes,
        attempt_arc_reference_bytes,
        max_total_attempt_arc_reference_bytes,
        "generated cylindrical attempt Arc reference bytes"
    );
    charge!(
        unique_persistent_source_references,
        unique_persistent_sources,
        max_total_unique_persistent_source_references,
        "generated cylindrical unique persistent-source references"
    );
    charge!(
        candidate_retained_payload_bytes,
        candidate_retained_payload_bytes,
        max_total_candidate_retained_payload_bytes,
        "generated cylindrical candidate retained payload bytes"
    );
    charge!(
        persistent_source_retained_bytes,
        persistent_source_retained_bytes,
        max_total_persistent_source_retained_bytes,
        "generated cylindrical persistent-source retained bytes"
    );
    charge!(
        when_bad_binding_bytes,
        when_bad_binding_bytes,
        max_total_when_bad_binding_bytes,
        "generated cylindrical WhenBad binding bytes"
    );
    charge!(
        when_bad_retained_core_bytes,
        when_bad_retained_core_bytes,
        max_total_when_bad_retained_core_bytes,
        "generated cylindrical WhenBad retained core bytes"
    );
    charge!(
        when_bad_guard_origin_retained_bytes,
        when_bad_guard_origin_retained_bytes,
        max_total_when_bad_guard_origin_retained_bytes,
        "generated cylindrical WhenBad guard-origin retained bytes"
    );
    charge!(
        when_bad_condition_terms,
        when_bad_condition_terms,
        max_total_when_bad_condition_terms,
        "generated cylindrical WhenBad condition terms"
    );
    charge!(
        when_bad_condition_bytes,
        when_bad_condition_bytes,
        max_total_when_bad_condition_bytes,
        "generated cylindrical WhenBad condition bytes"
    );
    charge!(
        when_bad_leak_event_retained_bytes,
        when_bad_leak_event_retained_bytes,
        max_total_when_bad_leak_event_retained_bytes,
        "generated cylindrical WhenBad leak-event retained bytes"
    );
    charge!(
        base_structural_loci,
        base_structural_loci,
        max_total_base_structural_loci,
        "generated cylindrical base structural loci"
    );
    charge!(
        base_structural_locus_terms,
        base_structural_locus_terms,
        max_total_base_structural_locus_terms,
        "generated cylindrical base structural-locus terms"
    );
    charge!(
        base_structural_locus_bytes,
        base_structural_locus_bytes,
        max_total_base_structural_locus_bytes,
        "generated cylindrical base structural-locus bytes"
    );
    charge!(
        normalized_clauses,
        normalized_clauses,
        max_total_normalized_clauses,
        "generated cylindrical normalized clauses"
    );
    charge!(
        normalized_literals,
        normalized_literals,
        max_total_normalized_literals,
        "generated cylindrical normalized literals"
    );
    charge!(
        normalized_clause_source_references,
        normalized_clause_source_references,
        max_total_normalized_clause_source_references,
        "generated cylindrical normalized clause-source references"
    );
    charge!(
        normalized_factor_references,
        normalized_factor_references,
        max_total_normalized_factor_references,
        "generated cylindrical normalized factor references"
    );
    charge!(
        decision_atoms,
        decision_atoms,
        max_total_decision_atoms,
        "generated cylindrical decision atoms"
    );
    charge!(
        decision_nodes,
        decision_nodes,
        max_total_decision_nodes,
        "generated cylindrical decision nodes"
    );
    charge!(
        decision_terminals,
        decision_terminals,
        max_total_decision_terminals,
        "generated cylindrical decision terminals"
    );
    Ok(())
}

fn validate_family_context(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), GeneratedCylindricalSectorRuleProviderError> {
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
    {
        return Err(GeneratedCylindricalSectorRuleProviderError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(GeneratedCylindricalSectorRuleProviderError::WrongArity {
            expected: family.denominator_count(),
            actual: context.index_count(),
        });
    }
    Ok(())
}

fn validate_certificate_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    ordering_policy: IntegralOrderingPolicy,
    certificate: &GeneratedCylindricalSectorCoverageCertificate,
) -> Result<(), GeneratedCylindricalSectorRuleProviderError> {
    if certificate.family_fingerprint() != family.fingerprint_ref() {
        return Err(GeneratedCylindricalSectorRuleProviderError::WrongFamily);
    }
    if certificate.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedCylindricalSectorRuleProviderError::WrongContext);
    }
    if certificate.sector().arity() != context.index_count() {
        return Err(GeneratedCylindricalSectorRuleProviderError::WrongArity {
            expected: context.index_count(),
            actual: certificate.sector().arity(),
        });
    }
    if certificate.ordering_policy() != ordering_policy {
        return Err(GeneratedCylindricalSectorRuleProviderError::WrongOrdering {
            expected: ordering_policy,
            actual: certificate.ordering_policy(),
        });
    }
    Ok(())
}

fn compare_sector_to_indices(sector: &SectorMask, indices: &[i64]) -> Ordering {
    sector
        .active_bits()
        .iter()
        .copied()
        .zip(indices.iter().map(|&index| index >= 1))
        .find_map(|(left, right)| (left != right).then(|| left.cmp(&right)))
        .unwrap_or_else(|| sector.arity().cmp(&indices.len()))
}

fn copy_sector(
    sector: &SectorMask,
) -> Result<SectorMask, GeneratedCylindricalSectorRuleProviderError> {
    Ok(SectorMask::try_new(sector.active_bits().iter().copied())?)
}

const fn portable_limit(preferred: u128) -> usize {
    if preferred > usize::MAX as u128 {
        usize::MAX
    } else {
        preferred as usize
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalSectorRuleProviderError> {
    left.checked_add(right)
        .ok_or(GeneratedCylindricalSectorRuleProviderError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalSectorRuleProviderError> {
    left.checked_mul(right)
        .ok_or(GeneratedCylindricalSectorRuleProviderError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedCylindricalSectorRuleProviderError> {
    if requested > limit {
        Err(GeneratedCylindricalSectorRuleProviderError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalSectorRuleProviderError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedCylindricalSectorRuleProviderError> {
    values.try_reserve_exact(additional).map_err(|_| {
        GeneratedCylindricalSectorRuleProviderError::AllocationFailure {
            resource,
            requested: additional,
        }
    })
}
