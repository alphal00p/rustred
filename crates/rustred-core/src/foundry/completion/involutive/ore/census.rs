use symbolica::prelude::Integer;

#[cfg(test)]
use crate::algebra::CoefficientPolynomial;
use crate::algebra::IndexedCoefficient;

use crate::foundry::completion::involutive::error::{check_limit, checked_add, checked_mul};
use crate::foundry::completion::involutive::{InvolutiveError, InvolutiveLimits};

use super::model::{ConsequenceProvenance, OreRow};

/// Logical sparse payload retained by all row and provenance coefficients.
///
/// Byte counts exclude allocator metadata and spare capacity; entry/cell caps
/// remain the allocation-independent authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CoefficientPayloadCensus {
    terms: usize,
    exponent_cells: usize,
    retained_bytes: usize,
}

impl CoefficientPayloadCensus {
    #[cfg(test)]
    pub(crate) const fn from_counts_for_diagnostic_test(
        terms: usize,
        exponent_cells: usize,
        retained_bytes: usize,
    ) -> Self {
        Self {
            terms,
            exponent_cells,
            retained_bytes,
        }
    }

    pub(crate) const fn terms(self) -> usize {
        self.terms
    }

    pub(crate) const fn exponent_cells(self) -> usize {
        self.exponent_cells
    }

    pub(crate) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }

    pub(crate) fn try_add(self, right: Self) -> Result<Self, InvolutiveError> {
        Ok(Self {
            terms: checked_add("Ore coefficient payload terms", self.terms, right.terms)?,
            exponent_cells: checked_add(
                "Ore coefficient payload exponent cells",
                self.exponent_cells,
                right.exponent_cells,
            )?,
            retained_bytes: checked_add(
                "Ore coefficient payload retained bytes",
                self.retained_bytes,
                right.retained_bytes,
            )?,
        })
    }

    fn try_require_consequence_limits(
        self,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        check_limit(
            "Ore consequence coefficient terms",
            self.terms,
            limits.max_consequence_coefficient_terms,
        )?;
        check_limit(
            "Ore consequence coefficient exponent cells",
            self.exponent_cells,
            limits.max_consequence_coefficient_exponent_cells,
        )?;
        check_limit(
            "Ore consequence coefficient retained bytes",
            self.retained_bytes,
            limits.max_consequence_coefficient_retained_bytes,
        )?;
        Ok(self)
    }
}

pub(super) fn coefficient_payload_census(
    row: &OreRow,
    provenance: &ConsequenceProvenance,
    limits: InvolutiveLimits,
) -> Result<CoefficientPayloadCensus, InvolutiveError> {
    let mut census = CoefficientPayloadCensus::default();
    #[cfg(test)]
    let mut diagnostic = super::super::diagnostics::coefficient_payload_is_active()
        .then(DiagnosticPayloadBuilder::new);
    for term in &row.terms {
        let coefficient = &term.coefficient;
        let single = single_coefficient_census(
            coefficient,
            #[cfg(test)]
            diagnostic.as_mut().map(|builder| {
                (
                    builder,
                    super::super::diagnostics::JanetDiagnosticCoefficientComponentKind::Row,
                )
            }),
        )?;
        census = census.try_add(single)?;
    }
    for term in &provenance.terms {
        let coefficient = &term.left_coefficient;
        let single = single_coefficient_census(
            coefficient,
            #[cfg(test)]
            diagnostic.as_mut().map(|builder| {
                (
                    builder,
                    super::super::diagnostics::JanetDiagnosticCoefficientComponentKind::Provenance,
                )
            }),
        )?;
        census = census.try_add(single)?;
    }
    #[cfg(test)]
    if let Some(mut diagnostic) = diagnostic {
        if super::super::diagnostics::try_claim_exact_denominator_detail(census, limits) {
            diagnostic.collect_exact_denominator_detail(row, provenance);
        }
        super::super::diagnostics::record_coefficient_payload(diagnostic.finish(), census, limits);
    }
    census.try_require_consequence_limits(limits)
}

fn single_coefficient_census(
    coefficient: &IndexedCoefficient,
    #[cfg(test)] mut diagnostic: Option<(
        &mut DiagnosticPayloadBuilder,
        super::super::diagnostics::JanetDiagnosticCoefficientComponentKind,
    )>,
) -> Result<CoefficientPayloadCensus, InvolutiveError> {
    let mut census = CoefficientPayloadCensus {
        retained_bytes: std::mem::size_of::<IndexedCoefficient>(),
        ..CoefficientPayloadCensus::default()
    };
    #[cfg(test)]
    let ordinal = diagnostic
        .as_mut()
        .map(|(builder, component)| builder.begin_coefficient(*component));
    #[cfg(test)]
    let mut numerator = CoefficientPayloadCensus::default();
    #[cfg(test)]
    let mut denominator = CoefficientPayloadCensus::default();
    #[cfg(test)]
    let mut is_numerator = true;
    for polynomial in [&coefficient.raw().numerator, &coefficient.raw().denominator] {
        #[cfg(test)]
        let before = census;
        census.terms = checked_add(
            "Ore coefficient payload terms",
            census.terms,
            polynomial.coefficients.len(),
        )?;
        census.exponent_cells = checked_add(
            "Ore coefficient payload exponent cells",
            census.exponent_cells,
            polynomial.exponents.len(),
        )?;
        census.retained_bytes = checked_add(
            "Ore coefficient payload retained bytes",
            census.retained_bytes,
            checked_add(
                "Ore coefficient payload retained bytes",
                checked_mul(
                    "Ore coefficient payload retained bytes",
                    polynomial.coefficients.len(),
                    std::mem::size_of::<Integer>(),
                )?,
                checked_mul(
                    "Ore coefficient payload retained bytes",
                    polynomial.exponents.len(),
                    std::mem::size_of::<u16>(),
                )?,
            )?,
        )?;
        for integer in &polynomial.coefficients {
            let Integer::Large(value) = integer else {
                continue;
            };
            let bits = usize::try_from(value.significant_bits()).map_err(|_| {
                InvolutiveError::ResourceCountOverflow {
                    resource: "Ore coefficient payload retained bytes",
                }
            })?;
            census.retained_bytes = checked_add(
                "Ore coefficient payload retained bytes",
                census.retained_bytes,
                checked_add("Ore coefficient payload retained bytes", bits, 7)? / 8,
            )?;
        }
        #[cfg(test)]
        {
            let part = CoefficientPayloadCensus {
                terms: census.terms - before.terms,
                exponent_cells: census.exponent_cells - before.exponent_cells,
                retained_bytes: census.retained_bytes - before.retained_bytes,
            };
            if is_numerator {
                numerator = part;
                if let Some((builder, component)) = diagnostic.as_mut() {
                    builder.observe_numerator(*component, part);
                }
                is_numerator = false;
            } else {
                denominator = part;
                if let Some((builder, component)) = diagnostic.as_mut() {
                    builder.observe_denominator(*component, polynomial, part);
                }
            }
        }
    }
    #[cfg(test)]
    if let Some((builder, component)) = diagnostic {
        builder.observe_coefficient(
            component,
            ordinal.expect("active coefficient diagnostics must assign an ordinal"),
            census,
            numerator,
            denominator,
        );
    }
    Ok(census)
}

#[cfg(test)]
const MAX_EXACT_DENOMINATOR_REPRESENTATIVES: usize = 256;
#[cfg(test)]
const MAX_EXACT_DENOMINATOR_TRACKED_INSTANCES: usize = 256;
#[cfg(test)]
const MAX_EXACT_DENOMINATOR_HASH_TERMS: usize = 262_144;
#[cfg(test)]
const MAX_EXACT_DENOMINATOR_HASH_EXPONENT_CELLS: usize = 1_048_576;
#[cfg(test)]
const MAX_EXACT_DENOMINATOR_HASH_RETAINED_BYTES: usize = 16 * 1_024 * 1_024;
#[cfg(test)]
const MAX_EXACT_DENOMINATOR_EQUALITY_TERMS: usize = 262_144;
#[cfg(test)]
const MAX_EXACT_DENOMINATOR_EQUALITY_EXPONENT_CELLS: usize = 1_048_576;
#[cfg(test)]
const MAX_EXACT_DENOMINATOR_EQUALITY_RETAINED_BYTES: usize = 16 * 1_024 * 1_024;

#[cfg(test)]
struct DiagnosticPayloadBuilder {
    payload: super::super::diagnostics::JanetDiagnosticCoefficientPayload,
}

#[cfg(test)]
impl DiagnosticPayloadBuilder {
    fn new() -> Self {
        Self {
            payload: Default::default(),
        }
    }

    fn begin_coefficient(
        &mut self,
        component: super::super::diagnostics::JanetDiagnosticCoefficientComponentKind,
    ) -> usize {
        let target = self.component_mut(component);
        let ordinal = target.coefficients;
        target.coefficients = target.coefficients.saturating_add(1);
        target.coefficient_wrapper_bytes = target
            .coefficient_wrapper_bytes
            .saturating_add(std::mem::size_of::<IndexedCoefficient>());
        ordinal
    }

    fn observe_numerator(
        &mut self,
        component: super::super::diagnostics::JanetDiagnosticCoefficientComponentKind,
        census: CoefficientPayloadCensus,
    ) {
        self.component_mut(component)
            .numerator
            .saturating_add_assign(diagnostic_payload(census));
    }

    fn observe_denominator(
        &mut self,
        component: super::super::diagnostics::JanetDiagnosticCoefficientComponentKind,
        polynomial: &CoefficientPolynomial,
        census: CoefficientPayloadCensus,
    ) {
        let payload = diagnostic_payload(census);
        self.component_mut(component)
            .denominator
            .saturating_add_assign(payload);
        let denominators = &mut self.payload.denominators;
        denominators.instances = denominators.instances.saturating_add(1);
        if polynomial.is_one() {
            denominators.unit_instances = denominators.unit_instances.saturating_add(1);
        } else {
            denominators.nonunit_instances = denominators.nonunit_instances.saturating_add(1);
            if polynomial_payload_rank(payload) > polynomial_payload_rank(denominators.max_nonunit)
            {
                denominators.max_nonunit = payload;
            }
        }
    }

    fn observe_coefficient(
        &mut self,
        component: super::super::diagnostics::JanetDiagnosticCoefficientComponentKind,
        ordinal: usize,
        total: CoefficientPayloadCensus,
        numerator: CoefficientPayloadCensus,
        denominator: CoefficientPayloadCensus,
    ) {
        let candidate = super::super::diagnostics::JanetDiagnosticMaxCoefficient {
            component,
            ordinal,
            total: diagnostic_payload(total),
            numerator: diagnostic_payload(numerator),
            denominator: diagnostic_payload(denominator),
        };
        if self
            .payload
            .max_single_coefficient
            .is_none_or(|current| max_coefficient_rank(candidate) > max_coefficient_rank(current))
        {
            self.payload.max_single_coefficient = Some(candidate);
        }
    }

    fn collect_exact_denominator_detail(
        &mut self,
        row: &OreRow,
        provenance: &ConsequenceProvenance,
    ) {
        let mut tracker = DiagnosticDenominatorTracker::new(self.payload.denominators);
        if tracker.is_enabled() {
            for denominator in row
                .terms
                .iter()
                .map(|term| &term.coefficient.raw().denominator)
                .chain(
                    provenance
                        .terms
                        .iter()
                        .map(|term| &term.left_coefficient.raw().denominator),
                )
            {
                tracker.observe(denominator);
            }
        }
        self.payload.denominators = tracker.finish();
    }

    fn finish(self) -> super::super::diagnostics::JanetDiagnosticCoefficientPayload {
        self.payload
    }

    fn component_mut(
        &mut self,
        component: super::super::diagnostics::JanetDiagnosticCoefficientComponentKind,
    ) -> &mut super::super::diagnostics::JanetDiagnosticCoefficientComponent {
        match component {
            super::super::diagnostics::JanetDiagnosticCoefficientComponentKind::Row => {
                &mut self.payload.row
            }
            super::super::diagnostics::JanetDiagnosticCoefficientComponentKind::Provenance => {
                &mut self.payload.provenance
            }
        }
    }
}

#[cfg(test)]
fn diagnostic_payload(
    census: CoefficientPayloadCensus,
) -> super::super::diagnostics::JanetDiagnosticPolynomialPayload {
    super::super::diagnostics::JanetDiagnosticPolynomialPayload {
        terms: census.terms,
        exponent_cells: census.exponent_cells,
        retained_bytes: census.retained_bytes,
    }
}

#[cfg(test)]
fn max_coefficient_rank(
    coefficient: super::super::diagnostics::JanetDiagnosticMaxCoefficient,
) -> (usize, usize, usize, usize, usize) {
    (
        coefficient.total.retained_bytes,
        coefficient.total.exponent_cells,
        coefficient.total.terms,
        match coefficient.component {
            super::super::diagnostics::JanetDiagnosticCoefficientComponentKind::Row => 1,
            super::super::diagnostics::JanetDiagnosticCoefficientComponentKind::Provenance => 0,
        },
        usize::MAX - coefficient.ordinal,
    )
}

#[cfg(test)]
struct DiagnosticDenominatorTracker<'payload> {
    representatives: std::collections::HashMap<u64, DiagnosticDenominatorRepresentative<'payload>>,
    stats: super::super::diagnostics::JanetDiagnosticDenominatorStats,
    enabled: bool,
}

#[cfg(test)]
struct DiagnosticDenominatorRepresentative<'payload> {
    polynomial: &'payload CoefficientPolynomial,
    payload: super::super::diagnostics::JanetDiagnosticPolynomialPayload,
}

#[cfg(test)]
impl<'payload> DiagnosticDenominatorTracker<'payload> {
    fn new(mut stats: super::super::diagnostics::JanetDiagnosticDenominatorStats) -> Self {
        stats.exact_tracking_attempted = true;
        let mut representatives = std::collections::HashMap::new();
        let reserve = stats
            .nonunit_instances
            .min(MAX_EXACT_DENOMINATOR_REPRESENTATIVES)
            .min(MAX_EXACT_DENOMINATOR_TRACKED_INSTANCES);
        // Every later insertion is guarded by both bounds, so one successful
        // fallible reserve is the only allocation this detail pass can make.
        let enabled = reserve == 0 || representatives.try_reserve(reserve).is_ok();
        if !enabled {
            stats.exact_tracking_truncated = true;
            stats.exact_oversized_or_budget_skips = stats
                .exact_oversized_or_budget_skips
                .saturating_add(stats.nonunit_instances);
        }
        Self {
            representatives,
            stats,
            enabled,
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn observe(&mut self, polynomial: &'payload CoefficientPolynomial) {
        use std::hash::{Hash, Hasher};

        if polynomial.is_one() {
            return;
        }
        let terms = polynomial.coefficients.len();
        let exponent_cells = polynomial.exponents.len();
        // Reject an oversized denominator from exact tracking using O(1)
        // shape fields before scanning any large-integer payload a second
        // time. The ordinary authoritative census has already scanned it.
        if !self.enabled
            || self.stats.exact_tracked_instances >= MAX_EXACT_DENOMINATOR_TRACKED_INSTANCES
            || self.representatives.len() >= MAX_EXACT_DENOMINATOR_REPRESENTATIVES
            || exceeds_budget(
                self.stats.exact_hashed_terms,
                terms,
                MAX_EXACT_DENOMINATOR_HASH_TERMS,
            )
            || exceeds_budget(
                self.stats.exact_hashed_exponent_cells,
                exponent_cells,
                MAX_EXACT_DENOMINATOR_HASH_EXPONENT_CELLS,
            )
        {
            self.skip();
            return;
        }
        let payload = diagnostic_polynomial_payload(polynomial);
        if exceeds_budget(
            self.stats.exact_hashed_retained_bytes,
            payload.retained_bytes,
            MAX_EXACT_DENOMINATOR_HASH_RETAINED_BYTES,
        ) {
            self.skip();
            return;
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        polynomial.hash(&mut hasher);
        let hash = hasher.finish();
        self.stats.exact_hashed_terms = self.stats.exact_hashed_terms.saturating_add(payload.terms);
        self.stats.exact_hashed_exponent_cells = self
            .stats
            .exact_hashed_exponent_cells
            .saturating_add(payload.exponent_cells);
        self.stats.exact_hashed_retained_bytes = self
            .stats
            .exact_hashed_retained_bytes
            .saturating_add(payload.retained_bytes);

        if let Some(representative) = self.representatives.get(&hash) {
            let representative_payload = representative.payload;
            let equality_terms = payload.terms.saturating_add(representative_payload.terms);
            let equality_cells = payload
                .exponent_cells
                .saturating_add(representative_payload.exponent_cells);
            let equality_bytes = payload
                .retained_bytes
                .saturating_add(representative_payload.retained_bytes);
            if exceeds_budget(
                self.stats.exact_equality_terms,
                equality_terms,
                MAX_EXACT_DENOMINATOR_EQUALITY_TERMS,
            ) || exceeds_budget(
                self.stats.exact_equality_exponent_cells,
                equality_cells,
                MAX_EXACT_DENOMINATOR_EQUALITY_EXPONENT_CELLS,
            ) || exceeds_budget(
                self.stats.exact_equality_retained_bytes,
                equality_bytes,
                MAX_EXACT_DENOMINATOR_EQUALITY_RETAINED_BYTES,
            ) {
                self.skip();
                return;
            }
            self.stats.exact_equality_terms = self
                .stats
                .exact_equality_terms
                .saturating_add(equality_terms);
            self.stats.exact_equality_exponent_cells = self
                .stats
                .exact_equality_exponent_cells
                .saturating_add(equality_cells);
            self.stats.exact_equality_retained_bytes = self
                .stats
                .exact_equality_retained_bytes
                .saturating_add(equality_bytes);
            if representative.polynomial == polynomial {
                self.stats.exact_tracked_instances =
                    self.stats.exact_tracked_instances.saturating_add(1);
                self.stats.exact_confirmed_shared_instances = self
                    .stats
                    .exact_confirmed_shared_instances
                    .saturating_add(1);
            } else {
                self.stats.exact_hash_collisions_skipped =
                    self.stats.exact_hash_collisions_skipped.saturating_add(1);
                self.stats.exact_tracking_truncated = true;
            }
            return;
        }

        self.representatives.insert(
            hash,
            DiagnosticDenominatorRepresentative {
                polynomial,
                payload,
            },
        );
        self.stats.exact_tracked_instances = self.stats.exact_tracked_instances.saturating_add(1);
        self.stats.exact_distinct_representatives =
            self.stats.exact_distinct_representatives.saturating_add(1);
    }

    fn skip(&mut self) {
        self.stats.exact_oversized_or_budget_skips =
            self.stats.exact_oversized_or_budget_skips.saturating_add(1);
        self.stats.exact_tracking_truncated = true;
    }

    fn finish(self) -> super::super::diagnostics::JanetDiagnosticDenominatorStats {
        self.stats
    }
}

#[cfg(test)]
fn diagnostic_polynomial_payload(
    polynomial: &CoefficientPolynomial,
) -> super::super::diagnostics::JanetDiagnosticPolynomialPayload {
    let coefficient_bytes = polynomial
        .coefficients
        .len()
        .saturating_mul(std::mem::size_of::<Integer>());
    let exponent_bytes = polynomial
        .exponents
        .len()
        .saturating_mul(std::mem::size_of::<u16>());
    let mut retained_bytes = coefficient_bytes.saturating_add(exponent_bytes);
    for coefficient in &polynomial.coefficients {
        let Integer::Large(value) = coefficient else {
            continue;
        };
        let bits = usize::try_from(value.significant_bits()).unwrap_or(usize::MAX);
        retained_bytes = retained_bytes.saturating_add(bits.saturating_add(7) / 8);
    }
    super::super::diagnostics::JanetDiagnosticPolynomialPayload {
        terms: polynomial.coefficients.len(),
        exponent_cells: polynomial.exponents.len(),
        retained_bytes,
    }
}

#[cfg(test)]
fn polynomial_payload_rank(
    payload: super::super::diagnostics::JanetDiagnosticPolynomialPayload,
) -> (usize, usize, usize) {
    (
        payload.retained_bytes,
        payload.exponent_cells,
        payload.terms,
    )
}

#[cfg(test)]
fn exceeds_budget(current: usize, additional: usize, limit: usize) -> bool {
    additional > limit.saturating_sub(current)
}
