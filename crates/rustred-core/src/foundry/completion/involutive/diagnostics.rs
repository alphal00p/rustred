//! Test-only structured progress snapshots for bounded release diagnostics.
//!
//! Production completion has no observer, callback, formatting, or
//! thread-local access. Ignored release tests opt in explicitly and consume a
//! single snapshot after a typed stop.

use std::cell::Cell;
use std::cell::RefCell;

use super::janet::JanetDivisionEpoch;
use super::{
    CoefficientPayloadCensus, InvolutiveLimits, InvolutiveWorkCensus, JanetBasisEpoch,
    JanetInitialReductionCensus,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JanetDiagnosticPhase {
    ChartLift,
    InitialPreprocessing,
    InitialAutoreduction,
    Completion,
    CompletionAutoreduction,
}

/// Exact construction seam responsible for a diagnosed coefficient payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JanetDiagnosticCoefficientSite {
    ChartLiftSource,
    InitialHeadReduction,
    AutoreductionMaterialization,
    MonicNormalization,
    Prolongation,
    NormalFormCancellation,
    DirectAxpy,
}

impl JanetDiagnosticCoefficientSite {
    const fn name(self) -> &'static str {
        match self {
            Self::ChartLiftSource => "chart_lift_source",
            Self::InitialHeadReduction => "initial_head_reduction",
            Self::AutoreductionMaterialization => "autoreduction_materialization",
            Self::MonicNormalization => "monic_normalization",
            Self::Prolongation => "prolongation",
            Self::NormalFormCancellation => "normal_form_cancellation",
            Self::DirectAxpy => "direct_axpy",
        }
    }
}

/// Stable, self-labeling attempt counts for every exact construction seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticCoefficientSiteCounts {
    pub(crate) chart_lift_source: usize,
    pub(crate) initial_head_reduction: usize,
    pub(crate) autoreduction_materialization: usize,
    pub(crate) monic_normalization: usize,
    pub(crate) prolongation: usize,
    pub(crate) normal_form_cancellation: usize,
    pub(crate) direct_axpy: usize,
}

impl JanetDiagnosticCoefficientSiteCounts {
    fn saturating_increment(&mut self, site: JanetDiagnosticCoefficientSite) {
        let count = match site {
            JanetDiagnosticCoefficientSite::ChartLiftSource => &mut self.chart_lift_source,
            JanetDiagnosticCoefficientSite::InitialHeadReduction => {
                &mut self.initial_head_reduction
            }
            JanetDiagnosticCoefficientSite::AutoreductionMaterialization => {
                &mut self.autoreduction_materialization
            }
            JanetDiagnosticCoefficientSite::MonicNormalization => &mut self.monic_normalization,
            JanetDiagnosticCoefficientSite::Prolongation => &mut self.prolongation,
            JanetDiagnosticCoefficientSite::NormalFormCancellation => {
                &mut self.normal_form_cancellation
            }
            JanetDiagnosticCoefficientSite::DirectAxpy => &mut self.direct_axpy,
        };
        *count = count.saturating_add(1);
    }
}

/// Logical payload of one numerator or denominator polynomial.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticPolynomialPayload {
    pub(crate) terms: usize,
    pub(crate) exponent_cells: usize,
    pub(crate) retained_bytes: usize,
}

impl JanetDiagnosticPolynomialPayload {
    pub(crate) fn saturating_add_assign(&mut self, right: Self) {
        self.terms = self.terms.saturating_add(right.terms);
        self.exponent_cells = self.exponent_cells.saturating_add(right.exponent_cells);
        self.retained_bytes = self.retained_bytes.saturating_add(right.retained_bytes);
    }

    fn from_census(census: CoefficientPayloadCensus) -> Self {
        Self {
            terms: census.terms(),
            exponent_cells: census.exponent_cells(),
            retained_bytes: census.retained_bytes(),
        }
    }
}

/// Payload split for either the Ore row or its independently replayable provenance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticCoefficientComponent {
    pub(crate) coefficients: usize,
    pub(crate) coefficient_wrapper_bytes: usize,
    pub(crate) numerator: JanetDiagnosticPolynomialPayload,
    pub(crate) denominator: JanetDiagnosticPolynomialPayload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JanetDiagnosticCoefficientComponentKind {
    Row,
    Provenance,
}

/// Largest single rational-function coefficient in one attempted consequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticMaxCoefficient {
    pub(crate) component: JanetDiagnosticCoefficientComponentKind,
    pub(crate) ordinal: usize,
    pub(crate) total: JanetDiagnosticPolynomialPayload,
    pub(crate) numerator: JanetDiagnosticPolynomialPayload,
    pub(crate) denominator: JanetDiagnosticPolynomialPayload,
}

/// Bounded observations of canonical denominator reuse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticDenominatorStats {
    pub(crate) instances: usize,
    pub(crate) unit_instances: usize,
    pub(crate) nonunit_instances: usize,
    pub(crate) max_nonunit: JanetDiagnosticPolynomialPayload,
    pub(crate) exact_tracked_instances: usize,
    pub(crate) exact_distinct_representatives: usize,
    pub(crate) exact_confirmed_shared_instances: usize,
    pub(crate) exact_hash_collisions_skipped: usize,
    pub(crate) exact_oversized_or_budget_skips: usize,
    pub(crate) exact_hashed_terms: usize,
    pub(crate) exact_hashed_exponent_cells: usize,
    pub(crate) exact_hashed_retained_bytes: usize,
    pub(crate) exact_equality_terms: usize,
    pub(crate) exact_equality_exponent_cells: usize,
    pub(crate) exact_equality_retained_bytes: usize,
    /// Exact hashing/equality was claimed for this payload. Cheap shape fields
    /// are populated for every attempt, but this expensive detail is collected
    /// only once for the first limit-exceeding payload in a diagnostic run.
    pub(crate) exact_tracking_attempted: bool,
    pub(crate) exact_tracking_truncated: bool,
}

/// Test-only detailed payload assembled by the ordinary exact census pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticCoefficientPayload {
    pub(crate) total: JanetDiagnosticPolynomialPayload,
    pub(crate) row: JanetDiagnosticCoefficientComponent,
    pub(crate) provenance: JanetDiagnosticCoefficientComponent,
    pub(crate) max_single_coefficient: Option<JanetDiagnosticMaxCoefficient>,
    pub(crate) denominators: JanetDiagnosticDenominatorStats,
}

/// All resource-limit predicates for an attempted payload, evaluated without
/// changing the authoritative terms -> cells -> bytes rejection order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticCoefficientLimitExcess {
    pub(crate) terms: bool,
    pub(crate) exponent_cells: bool,
    pub(crate) retained_bytes: bool,
}

/// One fully materialized coefficient payload observed at an exact boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticCoefficientAttempt {
    pub(crate) sequence: usize,
    pub(crate) phase: JanetDiagnosticPhase,
    pub(crate) site: JanetDiagnosticCoefficientSite,
    pub(crate) payload: JanetDiagnosticCoefficientPayload,
    pub(crate) exceeds: JanetDiagnosticCoefficientLimitExcess,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticBasis {
    pub(crate) rows: usize,
    pub(crate) revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticCheckpoint {
    pub(crate) phase: JanetDiagnosticPhase,
    pub(crate) lifted_rows: usize,
    pub(crate) initial_reduction: Option<JanetInitialReductionCensus>,
    pub(crate) initial_basis: Option<JanetDiagnosticBasis>,
    pub(crate) current_basis: Option<JanetDiagnosticBasis>,
    pub(crate) autoreduction_pass: Option<usize>,
    /// Exact at a work-ledger cap; otherwise the most recent coarse phase,
    /// pass, or epoch checkpoint.
    pub(crate) work_at_last_checkpoint: InvolutiveWorkCensus,
    pub(crate) coefficient_payload_attempts: usize,
    pub(crate) coefficient_payload_attempts_by_site: JanetDiagnosticCoefficientSiteCounts,
    pub(crate) last_coefficient_payload: Option<JanetDiagnosticCoefficientAttempt>,
    pub(crate) peak_coefficient_payload: Option<JanetDiagnosticCoefficientAttempt>,
    exact_denominator_detail_claimed: bool,
}

thread_local! {
    static CHECKPOINT: RefCell<Option<JanetDiagnosticCheckpoint>> = const { RefCell::new(None) };
    static COEFFICIENT_SITE: Cell<Option<JanetDiagnosticCoefficientSite>> = const { Cell::new(None) };
}

pub(crate) fn begin() {
    CHECKPOINT.with(|slot| {
        *slot.borrow_mut() = Some(JanetDiagnosticCheckpoint {
            phase: JanetDiagnosticPhase::ChartLift,
            lifted_rows: 0,
            initial_reduction: None,
            initial_basis: None,
            current_basis: None,
            autoreduction_pass: None,
            work_at_last_checkpoint: InvolutiveWorkCensus::default(),
            coefficient_payload_attempts: 0,
            coefficient_payload_attempts_by_site: JanetDiagnosticCoefficientSiteCounts::default(),
            last_coefficient_payload: None,
            peak_coefficient_payload: None,
            exact_denominator_detail_claimed: false,
        });
    });
}

pub(crate) fn take() -> Option<JanetDiagnosticCheckpoint> {
    CHECKPOINT.with(|slot| slot.borrow_mut().take())
}

pub(crate) fn record_lifted_rows(rows: usize) {
    update(|checkpoint| {
        checkpoint.phase = JanetDiagnosticPhase::InitialPreprocessing;
        checkpoint.lifted_rows = rows;
    });
}

pub(super) fn record_initial_basis(
    census: JanetInitialReductionCensus,
    epoch: &JanetBasisEpoch,
    work: InvolutiveWorkCensus,
) {
    let basis = basis(epoch);
    update(|checkpoint| {
        checkpoint.phase = JanetDiagnosticPhase::InitialAutoreduction;
        checkpoint.initial_reduction = Some(census);
        checkpoint.initial_basis = Some(basis);
        checkpoint.current_basis = Some(basis);
        checkpoint.autoreduction_pass = None;
        checkpoint.work_at_last_checkpoint = work;
    });
}

pub(super) fn record_autoreduction_division_pass(
    epoch: &JanetDivisionEpoch,
    pass: usize,
    work: InvolutiveWorkCensus,
) {
    update(|checkpoint| {
        checkpoint.current_basis = Some(division_basis(epoch));
        checkpoint.autoreduction_pass = Some(pass);
        checkpoint.work_at_last_checkpoint = work;
    });
}

pub(super) fn record_completion_epoch(epoch: &JanetBasisEpoch, work: InvolutiveWorkCensus) {
    update(|checkpoint| {
        checkpoint.phase = JanetDiagnosticPhase::Completion;
        checkpoint.current_basis = Some(basis(epoch));
        checkpoint.autoreduction_pass = None;
        checkpoint.work_at_last_checkpoint = work;
    });
}

pub(super) fn record_completion_division_autoreduction(
    epoch: &JanetDivisionEpoch,
    work: InvolutiveWorkCensus,
) {
    update(|checkpoint| {
        checkpoint.phase = JanetDiagnosticPhase::CompletionAutoreduction;
        checkpoint.current_basis = Some(division_basis(epoch));
        checkpoint.autoreduction_pass = None;
        checkpoint.work_at_last_checkpoint = work;
    });
}

pub(super) fn record_work_at_typed_stop(work: InvolutiveWorkCensus) {
    update(|checkpoint| checkpoint.work_at_last_checkpoint = work);
}

/// Attribute any payload completed by `operation` to one exact construction
/// seam. An inactive diagnostic takes the closure directly and does not touch
/// the site TLS. The restoration guard remains correct across unwinding.
pub(super) fn with_coefficient_site<Result>(
    site: JanetDiagnosticCoefficientSite,
    operation: impl FnOnce() -> Result,
) -> Result {
    if !is_active() {
        return operation();
    }

    struct Restore(Option<JanetDiagnosticCoefficientSite>);
    impl Drop for Restore {
        fn drop(&mut self) {
            COEFFICIENT_SITE.with(|slot| slot.set(self.0));
        }
    }

    let previous = COEFFICIENT_SITE.with(|slot| slot.replace(Some(site)));
    let _restore = Restore(previous);
    operation()
}

pub(super) fn coefficient_payload_is_active() -> bool {
    is_active() && COEFFICIENT_SITE.with(|slot| slot.get().is_some())
}

/// Claim the sole expensive denominator-detail pass for this diagnostic run.
///
/// The claim is deliberately made only for the first payload that violates at
/// least one consequence limit. Ordinary attempts therefore add no heap
/// allocation and no second polynomial traversal. A failed detail allocation
/// consumes the claim and is reported as truncated; it can never affect the
/// authoritative algebra result.
pub(super) fn try_claim_exact_denominator_detail(
    total: CoefficientPayloadCensus,
    limits: InvolutiveLimits,
) -> bool {
    if !coefficient_payload_is_active() || !coefficient_limit_excess(total, limits).any() {
        return false;
    }
    CHECKPOINT.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(checkpoint) = slot.as_mut() else {
            return false;
        };
        if checkpoint.exact_denominator_detail_claimed {
            false
        } else {
            checkpoint.exact_denominator_detail_claimed = true;
            true
        }
    })
}

/// Record a completed attempted payload before the existing authoritative
/// retained-shape checks. This observer is nonsemantic: saturating telemetry
/// cannot introduce a new completion failure.
pub(super) fn record_coefficient_payload(
    mut payload: JanetDiagnosticCoefficientPayload,
    total: CoefficientPayloadCensus,
    limits: InvolutiveLimits,
) {
    let Some(site) = COEFFICIENT_SITE.with(|slot| slot.get()) else {
        return;
    };
    payload.total = JanetDiagnosticPolynomialPayload::from_census(total);
    update(|checkpoint| {
        checkpoint.coefficient_payload_attempts =
            checkpoint.coefficient_payload_attempts.saturating_add(1);
        checkpoint
            .coefficient_payload_attempts_by_site
            .saturating_increment(site);
        let attempt = JanetDiagnosticCoefficientAttempt {
            sequence: checkpoint.coefficient_payload_attempts,
            phase: checkpoint.phase,
            site,
            payload,
            exceeds: coefficient_limit_excess(total, limits),
        };
        checkpoint.last_coefficient_payload = Some(attempt);
        if checkpoint
            .peak_coefficient_payload
            .is_none_or(|peak| payload_rank(attempt) > payload_rank(peak))
        {
            checkpoint.peak_coefficient_payload = Some(attempt);
        }
    });
}

fn coefficient_limit_excess(
    total: CoefficientPayloadCensus,
    limits: InvolutiveLimits,
) -> JanetDiagnosticCoefficientLimitExcess {
    JanetDiagnosticCoefficientLimitExcess {
        terms: total.terms() > limits.max_consequence_coefficient_terms,
        exponent_cells: total.exponent_cells() > limits.max_consequence_coefficient_exponent_cells,
        retained_bytes: total.retained_bytes() > limits.max_consequence_coefficient_retained_bytes,
    }
}

impl JanetDiagnosticCoefficientLimitExcess {
    const fn any(self) -> bool {
        self.terms || self.exponent_cells || self.retained_bytes
    }
}

fn payload_rank(attempt: JanetDiagnosticCoefficientAttempt) -> (usize, usize, usize, usize) {
    (
        attempt.payload.total.retained_bytes,
        attempt.payload.total.exponent_cells,
        attempt.payload.total.terms,
        attempt.sequence,
    )
}

fn is_active() -> bool {
    CHECKPOINT.with(|slot| slot.borrow().is_some())
}

fn basis(epoch: &JanetBasisEpoch) -> JanetDiagnosticBasis {
    division_basis(epoch.division())
}

fn division_basis(epoch: &JanetDivisionEpoch) -> JanetDiagnosticBasis {
    JanetDiagnosticBasis {
        rows: epoch.elements().len(),
        revision: epoch.epoch().revision(),
    }
}

fn update(mut operation: impl FnMut(&mut JanetDiagnosticCheckpoint)) {
    CHECKPOINT.with(|slot| {
        if let Some(checkpoint) = slot.borrow_mut().as_mut() {
            operation(checkpoint);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_in_checkpoint_is_thread_local_consumable_and_resettable() {
        assert_eq!(take(), None);
        begin();
        record_lifted_rows(9);
        let checkpoint = take().expect("an opted-in checkpoint must exist");
        assert_eq!(checkpoint.phase, JanetDiagnosticPhase::InitialPreprocessing);
        assert_eq!(checkpoint.lifted_rows, 9);
        assert_eq!(take(), None);

        begin();
        assert_eq!(take().unwrap().phase, JanetDiagnosticPhase::ChartLift);
        assert_eq!(take(), None);
    }

    #[test]
    fn coefficient_site_scopes_restore_and_peak_is_independent_of_last() {
        assert_eq!(take(), None);
        let limits = InvolutiveLimits::default();
        let payload = JanetDiagnosticCoefficientPayload::default();
        begin();
        with_coefficient_site(JanetDiagnosticCoefficientSite::Prolongation, || {
            record_coefficient_payload(
                payload,
                CoefficientPayloadCensus::from_counts_for_diagnostic_test(1, 2, 10),
                limits,
            );
            with_coefficient_site(JanetDiagnosticCoefficientSite::MonicNormalization, || {
                record_coefficient_payload(
                    payload,
                    CoefficientPayloadCensus::from_counts_for_diagnostic_test(3, 6, 30),
                    limits,
                );
            });
            record_coefficient_payload(
                payload,
                CoefficientPayloadCensus::from_counts_for_diagnostic_test(2, 4, 20),
                limits,
            );
        });
        let checkpoint = take().unwrap();
        assert_eq!(checkpoint.coefficient_payload_attempts, 3);
        assert_eq!(
            checkpoint.last_coefficient_payload.unwrap().site,
            JanetDiagnosticCoefficientSite::Prolongation
        );
        assert_eq!(
            checkpoint.peak_coefficient_payload.unwrap().site,
            JanetDiagnosticCoefficientSite::MonicNormalization
        );
        assert_eq!(
            checkpoint
                .peak_coefficient_payload
                .unwrap()
                .payload
                .total
                .retained_bytes,
            30
        );
    }

    #[test]
    fn inactive_coefficient_observer_is_a_noop() {
        assert_eq!(take(), None);
        assert!(!coefficient_payload_is_active());
        let result = with_coefficient_site(
            JanetDiagnosticCoefficientSite::NormalFormCancellation,
            || 17,
        );
        assert_eq!(result, 17);
        record_coefficient_payload(
            JanetDiagnosticCoefficientPayload::default(),
            CoefficientPayloadCensus::from_counts_for_diagnostic_test(1, 1, 1),
            InvolutiveLimits::default(),
        );
        assert_eq!(take(), None);
    }

    #[test]
    fn coefficient_site_scope_restores_across_unwinding() {
        assert_eq!(take(), None);
        begin();
        with_coefficient_site(JanetDiagnosticCoefficientSite::Prolongation, || {
            let unwind = std::panic::catch_unwind(|| {
                with_coefficient_site(JanetDiagnosticCoefficientSite::MonicNormalization, || {
                    panic!("diagnostic restoration sentinel");
                });
            });
            assert!(unwind.is_err());
            record_coefficient_payload(
                JanetDiagnosticCoefficientPayload::default(),
                CoefficientPayloadCensus::from_counts_for_diagnostic_test(1, 1, 1),
                InvolutiveLimits::default(),
            );
        });
        assert!(!coefficient_payload_is_active());
        let checkpoint = take().unwrap();
        assert_eq!(
            checkpoint.last_coefficient_payload.unwrap().site,
            JanetDiagnosticCoefficientSite::Prolongation
        );
    }

    #[test]
    fn site_counts_are_named_and_self_labeling() {
        let mut counts = JanetDiagnosticCoefficientSiteCounts::default();
        counts.saturating_increment(JanetDiagnosticCoefficientSite::ChartLiftSource);
        counts.saturating_increment(JanetDiagnosticCoefficientSite::DirectAxpy);
        assert_eq!(counts.chart_lift_source, 1);
        assert_eq!(counts.direct_axpy, 1);
        assert!(format!("{counts:?}").contains(JanetDiagnosticCoefficientSite::DirectAxpy.name()));
    }
}
