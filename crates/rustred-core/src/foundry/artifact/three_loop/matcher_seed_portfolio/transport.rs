//! Cold fixed-sample transport from one foreign matcher chart into parent K6.
//!
//! A local ordinary row is translated once by a concrete lattice sample and
//! then specialized at the zero assignment. Its surviving auxiliary powers
//! are admitted transactionally before Symbolica expands any affine
//! numerator. The complete parent row is coalesced before one authenticated
//! route, selected from the raw target, is applied uniformly to every
//! endpoint. This module cannot construct rule cells, owners, terminals,
//! closure layers, or artifacts.

use std::error::Error;
use std::fmt;

use crate::algebra::{
    Coefficient, CoefficientPolynomial, ExactAlgebraError, IndexedAlgebraError,
    IndexedAlgebraLimits, coefficient_clone_owned_retained_byte_bound,
};
use crate::family::{IntegralFamily, IntegralKey, IntegralKeyError};
use crate::foundry::artifact::{
    MultiAffineNumeratorExpansionError, MultiAffineNumeratorExpansionLimits,
    MultiAffineNumeratorFactor, try_expand_multi_affine_numerator,
};
use crate::identity::{
    IntegralShift, ParametricIbpConfig, ParametricIbpGenerator, RowId, TranslatedSourceError,
    TranslatedSourceLimits, TranslatedSourceRequest,
};
use crate::sector::symmetry::permutation::TransportError;
use crate::sector::symmetry::{
    CanonicalizationError, Canonicalizer, RoutingCoefficient, RoutingWitness,
};

use super::MatcherSeedChart;
use super::routing::{MatcherChartTransportError, MatcherChartTransportLimits};

const SPECIALIZATION_ASSIGNMENT: i64 = 0;

/// Explicit work policy for one cold fixed-sample row transport.
///
/// These bounds constrain one materialization only; they are not an
/// arbitrary-rank statement or an application-domain limit on RustRed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FixedMatcherChartRowTransportLimits {
    pub(super) translation: TranslatedSourceLimits,
    pub(super) specialization: IndexedAlgebraLimits,
    pub(super) chart_admission: MatcherChartTransportLimits,
    pub(super) expansion: MultiAffineNumeratorExpansionLimits,
    pub(super) max_surviving_local_terms: usize,
    pub(super) max_specialized_conditions: usize,
    pub(super) max_retained_condition_terms: usize,
    pub(super) max_expanded_contributions: usize,
    pub(super) max_coalesced_parent_endpoints: usize,
    pub(super) max_common_route_coordinate_cells: usize,
    pub(super) max_retained_parent_coefficient_terms: usize,
    pub(super) max_retained_parent_coefficient_clone_owned_bytes: usize,
}

impl Default for FixedMatcherChartRowTransportLimits {
    fn default() -> Self {
        Self {
            translation: TranslatedSourceLimits::default(),
            specialization: IndexedAlgebraLimits::default(),
            chart_admission: MatcherChartTransportLimits::new(1_000_000),
            expansion: MultiAffineNumeratorExpansionLimits::default(),
            max_surviving_local_terms: 1_000_000,
            max_specialized_conditions: 1_000_000,
            max_retained_condition_terms: 16_000_000,
            max_expanded_contributions: 16_000_000,
            max_coalesced_parent_endpoints: 4_000_000,
            max_common_route_coordinate_cells: 64_000_000,
            max_retained_parent_coefficient_terms: 64_000_000,
            max_retained_parent_coefficient_clone_owned_bytes: 1_000_000_000,
        }
    }
}

/// Exact chronology and common-route evidence retained with one cold row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FixedMatcherChartRowProvenance {
    diagnostic_chart_label: &'static str,
    parent_family_fingerprint: String,
    local_family_fingerprint: String,
    source_ordinal: usize,
    source_row: RowId,
    local_sample: IntegralShift,
    raw_target: IntegralKey,
    canonical_target: IntegralKey,
    common_route: RoutingWitness,
}

impl FixedMatcherChartRowProvenance {
    pub(super) const fn diagnostic_chart_label(&self) -> &'static str {
        self.diagnostic_chart_label
    }

    pub(super) fn parent_family_fingerprint(&self) -> &str {
        &self.parent_family_fingerprint
    }

    pub(super) fn local_family_fingerprint(&self) -> &str {
        &self.local_family_fingerprint
    }

    pub(super) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(super) fn source_row(&self) -> &RowId {
        &self.source_row
    }

    pub(super) fn local_sample(&self) -> &IntegralShift {
        &self.local_sample
    }

    pub(super) fn raw_target(&self) -> &IntegralKey {
        &self.raw_target
    }

    pub(super) fn canonical_target(&self) -> &IntegralKey {
        &self.canonical_target
    }

    pub(super) fn common_route(&self) -> &RoutingWitness {
        &self.common_route
    }
}

/// Measured work retained for falsifier comparisons, never as proof of
/// closure or source ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FixedMatcherChartRowTelemetry {
    translated_terms: usize,
    exact_zero_terms_pruned: usize,
    surviving_local_terms: usize,
    expanded_contributions: usize,
    raw_parent_endpoints: usize,
    canonical_parent_endpoints: usize,
    exact_parent_endpoint_cancellations: usize,
}

impl FixedMatcherChartRowTelemetry {
    pub(super) const fn translated_terms(self) -> usize {
        self.translated_terms
    }

    pub(super) const fn exact_zero_terms_pruned(self) -> usize {
        self.exact_zero_terms_pruned
    }

    pub(super) const fn surviving_local_terms(self) -> usize {
        self.surviving_local_terms
    }

    pub(super) const fn expanded_contributions(self) -> usize {
        self.expanded_contributions
    }

    pub(super) const fn raw_parent_endpoints(self) -> usize {
        self.raw_parent_endpoints
    }

    pub(super) const fn canonical_parent_endpoints(self) -> usize {
        self.canonical_parent_endpoints
    }

    pub(super) const fn exact_parent_endpoint_cancellations(self) -> usize {
        self.exact_parent_endpoint_cancellations
    }
}

/// One immutable, exactly coalesced parent row produced only for cold search
/// diagnostics. Terms are strictly ordered by canonical parent key.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ColdFixedMatcherChartParentRow {
    provenance: FixedMatcherChartRowProvenance,
    terms: Box<[(IntegralKey, Coefficient)]>,
    nonzero_conditions: Box<[CoefficientPolynomial]>,
    telemetry: FixedMatcherChartRowTelemetry,
}

impl ColdFixedMatcherChartParentRow {
    pub(super) fn provenance(&self) -> &FixedMatcherChartRowProvenance {
        &self.provenance
    }

    pub(super) fn terms(&self) -> &[(IntegralKey, Coefficient)] {
        &self.terms
    }

    pub(super) fn support(&self) -> impl ExactSizeIterator<Item = &IntegralKey> {
        self.terms.iter().map(|(key, _)| key)
    }

    pub(super) fn nonzero_conditions(&self) -> &[CoefficientPolynomial] {
        &self.nonzero_conditions
    }

    pub(super) const fn telemetry(&self) -> FixedMatcherChartRowTelemetry {
        self.telemetry
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FixedMatcherChartRowTransportError {
    Translation(TranslatedSourceError),
    IndexedAlgebra(IndexedAlgebraError),
    ChartTransport(MatcherChartTransportError),
    Expansion(MultiAffineNumeratorExpansionError),
    Canonicalization(CanonicalizationError),
    RouteTransport(TransportError),
    IntegralKey(IntegralKeyError),
    ExactAlgebra(ExactAlgebraError),
    WrongParentFamily,
    WrongCanonicalizerFamily,
    TranslatedSourceInvariant {
        detail: &'static str,
    },
    VanishingSpecializedCondition {
        condition_ordinal: usize,
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
    CommonRouteInvariant,
    RetainedCoefficientCensusInvariant {
        detail: &'static str,
    },
}

impl fmt::Display for FixedMatcherChartRowTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Translation(error) => error.fmt(formatter),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::ChartTransport(error) => error.fmt(formatter),
            Self::Expansion(error) => error.fmt(formatter),
            Self::Canonicalization(error) => error.fmt(formatter),
            Self::RouteTransport(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::WrongParentFamily => formatter
                .write_str("a fixed matcher-chart row was paired with another parent family"),
            Self::WrongCanonicalizerFamily => formatter.write_str(
                "a fixed matcher-chart row was paired with another family's canonicalizer",
            ),
            Self::TranslatedSourceInvariant { detail } => {
                write!(
                    formatter,
                    "translated matcher-chart source invariant failed: {detail}"
                )
            }
            Self::VanishingSpecializedCondition { condition_ordinal } => write!(
                formatter,
                "matcher-chart source condition {condition_ordinal} vanishes at the fixed sample"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(formatter, "could not reserve {requested} {resource}"),
            Self::CommonRouteInvariant => formatter.write_str(
                "the matcher-chart common route is not authenticated for its raw target",
            ),
            Self::RetainedCoefficientCensusInvariant { detail } => write!(
                formatter,
                "matcher-chart retained coefficient census invariant failed: {detail}"
            ),
        }
    }
}

impl Error for FixedMatcherChartRowTransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Translation(error) => Some(error),
            Self::IndexedAlgebra(error) => Some(error),
            Self::ChartTransport(error) => Some(error),
            Self::Expansion(error) => Some(error),
            Self::Canonicalization(error) => Some(error),
            Self::RouteTransport(error) => Some(error),
            Self::IntegralKey(error) => Some(error),
            Self::ExactAlgebra(error) => Some(error),
            _ => None,
        }
    }
}

macro_rules! from_transport_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for FixedMatcherChartRowTransportError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

from_transport_error!(TranslatedSourceError, Translation);
from_transport_error!(IndexedAlgebraError, IndexedAlgebra);
from_transport_error!(MatcherChartTransportError, ChartTransport);
from_transport_error!(MultiAffineNumeratorExpansionError, Expansion);
from_transport_error!(CanonicalizationError, Canonicalization);
from_transport_error!(TransportError, RouteTransport);
from_transport_error!(IntegralKeyError, IntegralKey);
from_transport_error!(ExactAlgebraError, ExactAlgebra);

#[derive(Debug)]
struct SpecializedLocalTerm {
    local_key: IntegralKey,
    coefficient: Coefficient,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RetainedCoefficientWeight {
    terms: usize,
    clone_owned_bytes: usize,
}

impl RetainedCoefficientWeight {
    fn checked_add(self, other: Self) -> Result<Self, FixedMatcherChartRowTransportError> {
        Ok(Self {
            terms: self.terms.checked_add(other.terms).ok_or(
                FixedMatcherChartRowTransportError::ResourceCountOverflow {
                    resource: "matcher-chart retained coefficient terms",
                },
            )?,
            clone_owned_bytes: self
                .clone_owned_bytes
                .checked_add(other.clone_owned_bytes)
                .ok_or(FixedMatcherChartRowTransportError::ResourceCountOverflow {
                    resource: "matcher-chart retained coefficient clone-owned bytes",
                })?,
        })
    }

    fn checked_sub(self, other: Self) -> Result<Self, FixedMatcherChartRowTransportError> {
        Ok(Self {
            terms: self.terms.checked_sub(other.terms).ok_or(
                FixedMatcherChartRowTransportError::RetainedCoefficientCensusInvariant {
                    detail: "the retained coefficient-term count underflowed",
                },
            )?,
            clone_owned_bytes: self
                .clone_owned_bytes
                .checked_sub(other.clone_owned_bytes)
                .ok_or(
                    FixedMatcherChartRowTransportError::RetainedCoefficientCensusInvariant {
                        detail: "the retained coefficient-byte count underflowed",
                    },
                )?,
        })
    }
}

struct RetainedCoefficientCensus {
    current: RetainedCoefficientWeight,
    max_terms: usize,
    max_clone_owned_bytes: usize,
}

impl RetainedCoefficientCensus {
    fn empty(limits: FixedMatcherChartRowTransportLimits) -> Self {
        Self {
            current: RetainedCoefficientWeight::default(),
            max_terms: limits.max_retained_parent_coefficient_terms,
            max_clone_owned_bytes: limits.max_retained_parent_coefficient_clone_owned_bytes,
        }
    }

    fn admit_additional(
        &self,
        additional: RetainedCoefficientWeight,
    ) -> Result<(), FixedMatcherChartRowTransportError> {
        self.admit(self.current.checked_add(additional)?)
    }

    fn replace(
        &mut self,
        old: RetainedCoefficientWeight,
        new: RetainedCoefficientWeight,
    ) -> Result<(), FixedMatcherChartRowTransportError> {
        let prospective = self.current.checked_sub(old)?.checked_add(new)?;
        self.admit(prospective)?;
        self.current = prospective;
        Ok(())
    }

    fn retain(
        &mut self,
        weight: RetainedCoefficientWeight,
    ) -> Result<(), FixedMatcherChartRowTransportError> {
        self.replace(RetainedCoefficientWeight::default(), weight)
    }

    fn release(
        &mut self,
        weight: RetainedCoefficientWeight,
    ) -> Result<(), FixedMatcherChartRowTransportError> {
        self.replace(weight, RetainedCoefficientWeight::default())
    }

    /// Return the native subcall allowance when `duplicated` is already
    /// represented in this outer census but will also be counted as borrowed
    /// input by the topology-neutral expansion primitive.
    fn subcall_term_limit_excluding(
        &self,
        duplicated: RetainedCoefficientWeight,
    ) -> Result<usize, FixedMatcherChartRowTransportError> {
        let external = self.current.checked_sub(duplicated)?;
        self.max_terms.checked_sub(external.terms).ok_or(
            FixedMatcherChartRowTransportError::RetainedCoefficientCensusInvariant {
                detail: "the external coefficient-term count exceeds its admitted maximum",
            },
        )
    }

    fn subcall_byte_limit_excluding(
        &self,
        duplicated: RetainedCoefficientWeight,
    ) -> Result<usize, FixedMatcherChartRowTransportError> {
        let external = self.current.checked_sub(duplicated)?;
        self.max_clone_owned_bytes
            .checked_sub(external.clone_owned_bytes)
            .ok_or(
                FixedMatcherChartRowTransportError::RetainedCoefficientCensusInvariant {
                    detail: "the external coefficient-byte count exceeds its admitted maximum",
                },
            )
    }

    fn verify_exact<'coefficient>(
        &self,
        coefficients: impl IntoIterator<Item = &'coefficient Coefficient>,
    ) -> Result<(), FixedMatcherChartRowTransportError> {
        if retained_coefficients_weight(coefficients)? == self.current {
            Ok(())
        } else {
            Err(
                FixedMatcherChartRowTransportError::RetainedCoefficientCensusInvariant {
                    detail: "the final coefficient owners do not match the retained census",
                },
            )
        }
    }

    fn admit(
        &self,
        prospective: RetainedCoefficientWeight,
    ) -> Result<(), FixedMatcherChartRowTransportError> {
        check_limit(
            "matcher-chart retained coefficient terms",
            prospective.terms,
            self.max_terms,
        )?;
        check_limit(
            "matcher-chart retained coefficient clone-owned bytes",
            prospective.clone_owned_bytes,
            self.max_clone_owned_bytes,
        )
    }
}

fn retained_coefficient_weight(
    coefficient: &Coefficient,
) -> Result<RetainedCoefficientWeight, FixedMatcherChartRowTransportError> {
    let terms = coefficient
        .numerator
        .nterms()
        .checked_add(coefficient.denominator.nterms())
        .ok_or(FixedMatcherChartRowTransportError::ResourceCountOverflow {
            resource: "matcher-chart retained coefficient terms",
        })?;
    let clone_owned_bytes = coefficient_clone_owned_retained_byte_bound(coefficient).ok_or(
        FixedMatcherChartRowTransportError::ResourceCountOverflow {
            resource: "matcher-chart retained coefficient clone-owned bytes",
        },
    )?;
    Ok(RetainedCoefficientWeight {
        terms,
        clone_owned_bytes,
    })
}

fn retained_coefficients_weight<'coefficient>(
    coefficients: impl IntoIterator<Item = &'coefficient Coefficient>,
) -> Result<RetainedCoefficientWeight, FixedMatcherChartRowTransportError> {
    coefficients.into_iter().try_fold(
        RetainedCoefficientWeight::default(),
        |weight, coefficient| weight.checked_add(retained_coefficient_weight(coefficient)?),
    )
}

/// Transport one selected row from one already compiled foreign chart.
///
/// `local_sample` is the only nonzero index assignment accepted by this API:
/// it is applied by translated-source construction. Every translated indexed
/// coefficient and condition is then specialized at the all-zero assignment,
/// preventing accidental `c(n+s)|_{n=s}` double translation by construction.
pub(super) fn try_transport_fixed_matcher_chart_row(
    parent: &IntegralFamily,
    chart: &MatcherSeedChart,
    canonicalizer: &Canonicalizer,
    source_ordinal: usize,
    local_sample: IntegralShift,
    limits: FixedMatcherChartRowTransportLimits,
) -> Result<ColdFixedMatcherChartParentRow, FixedMatcherChartRowTransportError> {
    if chart.parent_family_fingerprint != parent.fingerprint() {
        return Err(FixedMatcherChartRowTransportError::WrongParentFamily);
    }
    if canonicalizer.family_fingerprint() != parent.fingerprint() {
        return Err(FixedMatcherChartRowTransportError::WrongCanonicalizerFamily);
    }

    let local = chart.completion.family();
    let generator = ParametricIbpGenerator::try_new_with_config(
        local,
        ParametricIbpConfig::default(),
    )
    .map_err(
        |_| FixedMatcherChartRowTransportError::TranslatedSourceInvariant {
            detail: "the sealed local chart no longer constructs its indexed source context",
        },
    )?;
    let translated = generator.translate_selected_completed_source_rows(
        &chart.ordinary,
        [TranslatedSourceRequest::new(
            source_ordinal,
            local_sample.clone(),
        )],
        limits.translation,
    )?;
    if translated.len() != 1 || translated.requests().len() != 1 {
        return Err(
            FixedMatcherChartRowTransportError::TranslatedSourceInvariant {
                detail: "one selected request did not produce exactly one translated source",
            },
        );
    }
    let request = &translated.requests()[0];
    let source = &translated.sources()[0];
    if request.source_ordinal() != source_ordinal
        || request.offset() != &local_sample
        || source.provenance().source_ordinal() != source_ordinal
        || source.provenance().offset() != &local_sample
    {
        return Err(
            FixedMatcherChartRowTransportError::TranslatedSourceInvariant {
                detail: "selected source/request provenance does not replay the fixed sample",
            },
        );
    }

    let zero_assignment = vec![SPECIALIZATION_ASSIGNMENT; local.denominator_count()];
    let mut specialized_conditions = Vec::new();
    for (condition_ordinal, condition) in source.nonzero_conditions().iter().enumerate() {
        let polynomial = generator.context().specialize_polynomial_sealed(
            condition.polynomial(),
            &zero_assignment,
            limits.specialization,
        )?;
        retain_specialized_condition(
            polynomial,
            condition_ordinal,
            &mut specialized_conditions,
            limits,
        )?;
    }

    check_limit(
        "matcher-chart translated source terms",
        source.terms().len(),
        limits.max_surviving_local_terms,
    )?;
    let mut specialized = Vec::new();
    try_reserve_exact(
        &mut specialized,
        source.terms().len(),
        "matcher-chart specialized local terms",
    )?;
    let mut coefficient_census = RetainedCoefficientCensus::empty(limits);
    let mut exact_zero_terms_pruned = 0usize;
    let mut next_denominator_condition_ordinal = source.nonzero_conditions().len();
    for (shift, indexed_coefficient) in source.terms() {
        let (coefficient, denominator_condition) = generator.context().specialize_sealed(
            indexed_coefficient,
            &zero_assignment,
            limits.specialization,
        )?;
        if let Some(condition) = denominator_condition {
            let ordinal = next_denominator_condition_ordinal;
            next_denominator_condition_ordinal = next_denominator_condition_ordinal
                .checked_add(1)
                .ok_or(FixedMatcherChartRowTransportError::ResourceCountOverflow {
                    resource: "matcher-chart specialized condition ordinals",
                })?;
            retain_specialized_condition(condition, ordinal, &mut specialized_conditions, limits)?;
        }
        if coefficient.is_zero() {
            exact_zero_terms_pruned = exact_zero_terms_pruned.checked_add(1).ok_or(
                FixedMatcherChartRowTransportError::ResourceCountOverflow {
                    resource: "matcher-chart pruned zero terms",
                },
            )?;
            continue;
        }
        coefficient_census.retain(retained_coefficient_weight(&coefficient)?)?;
        specialized.push(SpecializedLocalTerm {
            local_key: IntegralKey::try_new(shift.values().iter().copied())?,
            coefficient,
        });
    }
    let surviving_local_terms = specialized.len();

    // Admission is a complete preflight barrier: no Symbolica affine
    // expansion is attempted until every surviving endpoint has proved that
    // all local auxiliary powers are nonpositive and within the caller's
    // finite work policy.
    let mut admissions = Vec::new();
    try_reserve_exact(
        &mut admissions,
        specialized.len(),
        "matcher-chart parent transport admissions",
    )?;
    for term in &specialized {
        admissions.push(
            chart
                .routing
                .try_admit_numerator_only_transport(&term.local_key, limits.chart_admission)?,
        );
    }

    let local_target = IntegralKey::try_new(local_sample.values().iter().copied())?;
    let target_admission = chart
        .routing
        .try_admit_numerator_only_transport(&local_target, limits.chart_admission)?;
    let raw_target = target_admission.parent_physical_key().clone();
    let canonicalization = canonicalizer.canonicalize(&raw_target)?;
    if !canonicalizer.authenticates_route(canonicalization.route())
        || !canonicalization
            .route()
            .verify(&raw_target, canonicalization.canonical())
        || canonicalization.route().coefficient() != RoutingCoefficient::One
    {
        return Err(FixedMatcherChartRowTransportError::CommonRouteInvariant);
    }
    let common_route = canonicalization.route().clone();
    let canonical_target = canonicalization.canonical().clone();

    let mut contributions = Vec::new();
    for (term, admission) in specialized.into_iter().zip(admissions) {
        let term_weight = retained_coefficient_weight(&term.coefficient)?;
        // Admit the complete clone payload while the affine relations are
        // still borrowed. No Symbolica work may begin before these factor
        // owners fit beside the specialized row and prior contributions.
        let factor_weight = numerator_factor_weight(chart, &term.local_key)?;
        coefficient_census.admit_additional(factor_weight)?;
        let factors = numerator_factors(chart, &term.local_key)?;
        coefficient_census.retain(factor_weight)?;
        let remaining_contributions = limits
            .max_expanded_contributions
            .checked_sub(contributions.len())
            .ok_or(
                FixedMatcherChartRowTransportError::RetainedCoefficientCensusInvariant {
                    detail: "expanded contributions exceeded their admitted maximum",
                },
            )?;
        let mut expansion_limits = limits.expansion;
        let row_capacity_is_tighter = remaining_contributions < expansion_limits.max_endpoints;
        expansion_limits.max_endpoints =
            expansion_limits.max_endpoints.min(remaining_contributions);
        expansion_limits.max_retained_coefficient_terms = expansion_limits
            .max_retained_coefficient_terms
            .min(coefficient_census.subcall_term_limit_excluding(factor_weight)?);
        expansion_limits.max_retained_coefficient_clone_owned_bytes = expansion_limits
            .max_retained_coefficient_clone_owned_bytes
            .min(coefficient_census.subcall_byte_limit_excluding(factor_weight)?);
        let endpoints = try_expand_multi_affine_numerator(
            parent,
            admission.parent_physical_key(),
            &factors,
            expansion_limits,
        )
        .map_err(|error| {
            map_row_capacity_expansion_error(
                error,
                contributions.len(),
                remaining_contributions,
                row_capacity_is_tighter,
                limits,
            )
        })?;
        let requested = contributions.len().checked_add(endpoints.len()).ok_or(
            FixedMatcherChartRowTransportError::ResourceCountOverflow {
                resource: "matcher-chart expanded contributions",
            },
        )?;
        check_limit(
            "matcher-chart expanded contributions",
            requested,
            limits.max_expanded_contributions,
        )?;
        contributions
            .try_reserve_exact(endpoints.len())
            .map_err(|_| FixedMatcherChartRowTransportError::AllocationFailure {
                resource: "matcher-chart expanded contributions",
                requested,
            })?;
        let endpoint_weight =
            retained_coefficients_weight(endpoints.iter().map(|endpoint| endpoint.coefficient()))?;
        coefficient_census.retain(endpoint_weight)?;
        for endpoint in endpoints {
            let endpoint_weight = retained_coefficient_weight(endpoint.coefficient())?;
            let coefficient = parent.coefficient_context().try_mul(
                &term.coefficient,
                endpoint.coefficient(),
                limits.expansion.exact_algebra,
            )?;
            let coefficient_weight = retained_coefficient_weight(&coefficient)?;
            coefficient_census.admit_additional(coefficient_weight)?;
            if !coefficient.is_zero() {
                coefficient_census.replace(endpoint_weight, coefficient_weight)?;
                contributions.push((endpoint.key().clone(), coefficient));
            } else {
                coefficient_census
                    .replace(endpoint_weight, RetainedCoefficientWeight::default())?;
            }
        }
        drop(factors);
        coefficient_census.release(factor_weight)?;
        drop(term);
        coefficient_census.release(term_weight)?;
    }
    let expanded_contributions = contributions.len();
    let (raw_terms, exact_parent_endpoint_cancellations) =
        coalesce_parent_terms(parent, contributions, limits, &mut coefficient_census)?;
    let canonical_terms = apply_common_route(parent, &common_route, raw_terms, limits)?;
    coefficient_census.verify_exact(
        canonical_terms
            .terms
            .iter()
            .map(|(_, coefficient)| coefficient),
    )?;

    Ok(ColdFixedMatcherChartParentRow {
        provenance: FixedMatcherChartRowProvenance {
            diagnostic_chart_label: chart.diagnostic_label,
            parent_family_fingerprint: parent.fingerprint().to_owned(),
            local_family_fingerprint: local.fingerprint().to_owned(),
            source_ordinal,
            source_row: source.provenance().source_row().clone(),
            local_sample,
            raw_target,
            canonical_target,
            common_route,
        },
        telemetry: FixedMatcherChartRowTelemetry {
            translated_terms: source.terms().len(),
            exact_zero_terms_pruned,
            surviving_local_terms,
            expanded_contributions,
            raw_parent_endpoints: canonical_terms.raw_endpoint_count,
            canonical_parent_endpoints: canonical_terms.terms.len(),
            exact_parent_endpoint_cancellations,
        },
        terms: canonical_terms.terms.into_boxed_slice(),
        nonzero_conditions: specialized_conditions.into_boxed_slice(),
    })
}

fn numerator_factors(
    chart: &MatcherSeedChart,
    local_key: &IntegralKey,
) -> Result<Vec<MultiAffineNumeratorFactor>, FixedMatcherChartRowTransportError> {
    let physical_count = chart.routing.physical_parent_slots().len();
    let auxiliary_count = local_key.powers().len().saturating_sub(physical_count);
    let mut factors = Vec::new();
    try_reserve_exact(
        &mut factors,
        auxiliary_count,
        "matcher-chart affine numerator factors",
    )?;
    for (local_slot, &power) in local_key.powers().iter().enumerate().skip(physical_count) {
        if power == 0 {
            continue;
        }
        if power > 0 {
            // This should have been rejected transactionally before any call
            // reaches factor construction.
            return Err(
                MatcherChartTransportError::PositiveAuxiliaryPole { local_slot, power }.into(),
            );
        }
        let relation = chart.routing.local_to_parent().get(local_slot).ok_or(
            FixedMatcherChartRowTransportError::TranslatedSourceInvariant {
                detail: "an admitted auxiliary slot has no parent affine relation",
            },
        )?;
        factors.push(MultiAffineNumeratorFactor::try_new(
            relation.constant().clone(),
            relation.denominator_coefficients().iter().cloned(),
            power.unsigned_abs(),
        )?);
    }
    Ok(factors)
}

/// Census exactly the coefficient owners that `numerator_factors` will clone
/// without performing those clones. This is the aggregate admission barrier
/// for the primitive's borrowed affine input payload.
fn numerator_factor_weight(
    chart: &MatcherSeedChart,
    local_key: &IntegralKey,
) -> Result<RetainedCoefficientWeight, FixedMatcherChartRowTransportError> {
    let physical_count = chart.routing.physical_parent_slots().len();
    local_key
        .powers()
        .iter()
        .enumerate()
        .skip(physical_count)
        .try_fold(
            RetainedCoefficientWeight::default(),
            |weight, (local_slot, &power)| {
                if power == 0 {
                    return Ok(weight);
                }
                if power > 0 {
                    return Err(MatcherChartTransportError::PositiveAuxiliaryPole {
                        local_slot,
                        power,
                    }
                    .into());
                }
                let relation = chart.routing.local_to_parent().get(local_slot).ok_or(
                    FixedMatcherChartRowTransportError::TranslatedSourceInvariant {
                        detail: "an admitted auxiliary slot has no parent affine relation",
                    },
                )?;
                let relation_weight = retained_coefficients_weight(
                    std::iter::once(relation.constant()).chain(relation.denominator_coefficients()),
                )?;
                weight.checked_add(relation_weight)
            },
        )
}

fn map_row_capacity_expansion_error(
    error: MultiAffineNumeratorExpansionError,
    retained_contributions: usize,
    remaining_contributions: usize,
    row_capacity_is_tighter: bool,
    limits: FixedMatcherChartRowTransportLimits,
) -> FixedMatcherChartRowTransportError {
    match error {
        MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: "multi-affine endpoints",
            requested,
            limit,
        } if row_capacity_is_tighter && limit == remaining_contributions => {
            let Some(requested) = retained_contributions.checked_add(requested) else {
                return FixedMatcherChartRowTransportError::ResourceCountOverflow {
                    resource: "matcher-chart expanded contributions",
                };
            };
            FixedMatcherChartRowTransportError::ResourceLimit {
                resource: "matcher-chart expanded contributions",
                requested,
                limit: limits.max_expanded_contributions,
            }
        }
        error => FixedMatcherChartRowTransportError::Expansion(error),
    }
}

fn retain_specialized_condition(
    polynomial: CoefficientPolynomial,
    condition_ordinal: usize,
    retained: &mut Vec<CoefficientPolynomial>,
    limits: FixedMatcherChartRowTransportLimits,
) -> Result<(), FixedMatcherChartRowTransportError> {
    if polynomial.is_zero() {
        return Err(
            FixedMatcherChartRowTransportError::VanishingSpecializedCondition { condition_ordinal },
        );
    }
    if polynomial.is_constant() || retained.contains(&polynomial) {
        return Ok(());
    }
    let requested = retained.len().checked_add(1).ok_or(
        FixedMatcherChartRowTransportError::ResourceCountOverflow {
            resource: "matcher-chart specialized conditions",
        },
    )?;
    check_limit(
        "matcher-chart specialized conditions",
        requested,
        limits.max_specialized_conditions,
    )?;
    let condition_terms = retained
        .iter()
        .try_fold(polynomial.nterms(), |count, condition| {
            count.checked_add(condition.nterms()).ok_or(
                FixedMatcherChartRowTransportError::ResourceCountOverflow {
                    resource: "matcher-chart retained condition terms",
                },
            )
        })?;
    check_limit(
        "matcher-chart retained condition terms",
        condition_terms,
        limits.max_retained_condition_terms,
    )?;
    retained.try_reserve_exact(1).map_err(|_| {
        FixedMatcherChartRowTransportError::AllocationFailure {
            resource: "matcher-chart specialized conditions",
            requested,
        }
    })?;
    retained.push(polynomial);
    Ok(())
}

struct CoalescedParentTerms {
    raw_endpoint_count: usize,
    terms: Vec<(IntegralKey, Coefficient)>,
}

fn coalesce_parent_terms(
    parent: &IntegralFamily,
    mut contributions: Vec<(IntegralKey, Coefficient)>,
    limits: FixedMatcherChartRowTransportLimits,
    coefficient_census: &mut RetainedCoefficientCensus,
) -> Result<(Vec<(IntegralKey, Coefficient)>, usize), FixedMatcherChartRowTransportError> {
    contributions.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut coalesced: Vec<(IntegralKey, Coefficient)> = Vec::new();
    let mut exact_cancellations = 0usize;
    try_reserve_exact(
        &mut coalesced,
        contributions
            .len()
            .min(limits.max_coalesced_parent_endpoints),
        "matcher-chart coalesced parent endpoints",
    )?;
    for (key, coefficient) in contributions {
        if let Some((previous_key, previous_coefficient)) = coalesced.last_mut()
            && previous_key == &key
        {
            let previous_weight = retained_coefficient_weight(previous_coefficient)?;
            let incoming_weight = retained_coefficient_weight(&coefficient)?;
            let replaced_weight = previous_weight.checked_add(incoming_weight)?;
            let sum = parent.coefficient_context().try_add(
                previous_coefficient,
                &coefficient,
                limits.expansion.exact_algebra,
            )?;
            let sum_weight = retained_coefficient_weight(&sum)?;
            coefficient_census.admit_additional(sum_weight)?;
            if sum.is_zero() {
                coefficient_census
                    .replace(replaced_weight, RetainedCoefficientWeight::default())?;
                exact_cancellations = exact_cancellations.checked_add(1).ok_or(
                    FixedMatcherChartRowTransportError::ResourceCountOverflow {
                        resource: "matcher-chart exact parent endpoint cancellations",
                    },
                )?;
                coalesced.pop();
            } else {
                coefficient_census.replace(replaced_weight, sum_weight)?;
                *previous_coefficient = sum;
            }
            continue;
        }
        let requested = coalesced.len().checked_add(1).ok_or(
            FixedMatcherChartRowTransportError::ResourceCountOverflow {
                resource: "matcher-chart coalesced parent endpoints",
            },
        )?;
        check_limit(
            "matcher-chart coalesced parent endpoints",
            requested,
            limits.max_coalesced_parent_endpoints,
        )?;
        coalesced.push((key, coefficient));
    }
    coefficient_census.verify_exact(coalesced.iter().map(|(_, coefficient)| coefficient))?;
    Ok((coalesced, exact_cancellations))
}

fn apply_common_route(
    parent: &IntegralFamily,
    route: &RoutingWitness,
    raw_terms: Vec<(IntegralKey, Coefficient)>,
    limits: FixedMatcherChartRowTransportLimits,
) -> Result<CoalescedParentTerms, FixedMatcherChartRowTransportError> {
    let raw_endpoint_count = raw_terms.len();
    let coordinate_cells = raw_endpoint_count
        .checked_mul(parent.denominator_count())
        .ok_or(FixedMatcherChartRowTransportError::ResourceCountOverflow {
            resource: "matcher-chart common-route coordinate cells",
        })?;
    check_limit(
        "matcher-chart common-route coordinate cells",
        coordinate_cells,
        limits.max_common_route_coordinate_cells,
    )?;
    let mut routed = Vec::new();
    try_reserve_exact(
        &mut routed,
        raw_endpoint_count,
        "matcher-chart common-routed endpoints",
    )?;
    for (key, coefficient) in raw_terms {
        let mut powers = vec![0_i64; parent.denominator_count()];
        route.transport_into(key.powers(), &mut powers)?;
        routed.push((IntegralKey::try_new(powers)?, coefficient));
    }
    routed.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    if routed.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(FixedMatcherChartRowTransportError::CommonRouteInvariant);
    }
    Ok(CoalescedParentTerms {
        raw_endpoint_count,
        terms: routed,
    })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FixedMatcherChartRowTransportError> {
    if requested > limit {
        Err(FixedMatcherChartRowTransportError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_reserve_exact<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), FixedMatcherChartRowTransportError> {
    let requested = values
        .len()
        .checked_add(additional)
        .ok_or(FixedMatcherChartRowTransportError::ResourceCountOverflow { resource })?;
    values.try_reserve_exact(additional).map_err(|_| {
        FixedMatcherChartRowTransportError::AllocationFailure {
            resource,
            requested,
        }
    })
}

#[cfg(test)]
mod tests;
