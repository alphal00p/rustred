//! Source-neutral mapping of inherited `NonZero` predicates through a refined
//! residual affine locus.
//!
//! A proof-bearing caller will authenticate and order the source predicates,
//! then supply their typed locators here.  Until that owner is connected, the
//! compilation entry point remains private to this algebra-worker module and
//! none of its durable values can be consumed as proof authority.  The worker
//! replays the caller-owned
//! compact child plan, preflights the complete predicate stream before the
//! first Symbolica substitution, executes the sealed simultaneous-composition
//! tokens, and delegates every canonical algebraic comparison to the shared
//! condition accumulator.  It owns no generated-case authority and cannot
//! consume a target or publish a recurrence.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::generated_residual_affine_condition_accumulator::{
    GeneratedResidualAffineConditionAccumulatorCertificate,
    GeneratedResidualAffineConditionAccumulatorError,
    GeneratedResidualAffineConditionAccumulatorLimits, GeneratedResidualAffineConditionInput,
    GeneratedResidualAffineConditionScope, GeneratedResidualAffineConditionSourceLocator,
    accumulate_generated_residual_affine_conditions,
};
use crate::parametric_coefficient::{
    PreparedResidualAffineCompactGuardComposition, ResidualAffineCompactCompositionPlan,
    ResidualAffineCompactMapView, residual_affine_composition_output_retained_byte_envelope,
};
use crate::{
    ParametricCoefficientContext, ParametricPolynomial, ResidualUnitAffineCompositionError,
    ResidualUnitAffinePolynomialCompositionLimits, ResidualUnitAffinePolynomialCompositionStats,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_CASE_MAPPED_NONZERO_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-case-mapped-nonzero-v1";

#[cfg(test)]
std::thread_local! {
    static MAPPED_NONZERO_EXECUTIONS_FOR_TEST: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MAPPED_NONZERO_PANIC_AFTER_EXECUTIONS_FOR_TEST: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn reset_mapped_nonzero_execution_probe_for_test() {
    MAPPED_NONZERO_EXECUTIONS_FOR_TEST.with(|count| count.set(0));
    MAPPED_NONZERO_PANIC_AFTER_EXECUTIONS_FOR_TEST.with(|target| target.set(None));
}

#[cfg(test)]
fn mapped_nonzero_executions_for_test() -> usize {
    MAPPED_NONZERO_EXECUTIONS_FOR_TEST.with(std::cell::Cell::get)
}

#[cfg(test)]
fn inject_mapped_nonzero_panic_after_executions_for_test(executions: usize) {
    MAPPED_NONZERO_PANIC_AFTER_EXECUTIONS_FOR_TEST.with(|target| target.set(Some(executions)));
}

#[cfg(test)]
fn note_mapped_nonzero_execution_for_test() {
    MAPPED_NONZERO_EXECUTIONS_FOR_TEST.with(|count| count.set(count.get().saturating_add(1)));
    MAPPED_NONZERO_PANIC_AFTER_EXECUTIONS_FOR_TEST.with(|target| {
        if target.get() == Some(mapped_nonzero_executions_for_test()) {
            target.set(None);
            panic!("injected mapped-NonZero compiler panic");
        }
    });
}

#[cfg(not(test))]
fn note_mapped_nonzero_execution_for_test() {}

/// One already-authenticated predicate in caller-defined source order.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualCaseMappedNonZeroInput<'source> {
    polynomial: &'source ParametricPolynomial,
    source: GeneratedResidualAffineConditionSourceLocator,
}

impl<'source> GeneratedAffineResidualCaseMappedNonZeroInput<'source> {
    pub(crate) const fn new(
        polynomial: &'source ParametricPolynomial,
        source: GeneratedResidualAffineConditionSourceLocator,
    ) -> Self {
        Self { polynomial, source }
    }

    pub(crate) const fn polynomial(self) -> &'source ParametricPolynomial {
        self.polynomial
    }

    pub(crate) const fn source(self) -> GeneratedResidualAffineConditionSourceLocator {
        self.source
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseMappedNonZeroInput<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseMappedNonZeroInput")
            .field("polynomial", &"<redacted>")
            .field("source", &self.source)
            .finish()
    }
}

/// Complete-stream limits.  The composition totals are aggregate across all
/// predicates; the child composition limit remains an independent per-call
/// ceiling and is intersected with the still-unspent aggregate allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseMappedNonZeroLimits {
    pub(crate) composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub(crate) accumulator: GeneratedResidualAffineConditionAccumulatorLimits,
    pub(crate) max_source_inputs: usize,
    pub(crate) max_prepared_token_bytes: usize,
    pub(crate) max_mapped_output_slot_bytes: usize,
    pub(crate) max_total_source_terms: usize,
    pub(crate) max_total_source_exponent_entries: usize,
    pub(crate) max_total_expanded_contributions: usize,
    pub(crate) max_total_output_term_bound: usize,
    pub(crate) max_total_output_terms: usize,
    pub(crate) max_total_output_exponent_entry_bound: usize,
    pub(crate) max_total_output_exponent_entries: usize,
    pub(crate) max_total_power_calls: usize,
    pub(crate) max_total_native_power_heap_pairs: usize,
    pub(crate) max_total_multiplication_term_pairs: usize,
    pub(crate) max_total_addition_term_visits: usize,
    pub(crate) max_total_native_integer_bit_work: usize,
    pub(crate) max_total_integer_bit_work: usize,
    pub(crate) max_mapped_output_retained_byte_bound: usize,
    pub(crate) max_mapped_output_observed_retained_bytes: usize,
    pub(crate) max_retained_owned_bytes: usize,
    pub(crate) max_compilation_owned_peak_upper_bound: usize,
}

impl Default for GeneratedAffineResidualCaseMappedNonZeroLimits {
    fn default() -> Self {
        const LARGE: u64 = 64_000_000_000;
        const HUGE: u64 = 64_000_000_000_000;
        Self {
            composition: ResidualUnitAffinePolynomialCompositionLimits::default(),
            accumulator: GeneratedResidualAffineConditionAccumulatorLimits::default(),
            max_source_inputs: 192_000_000,
            max_prepared_token_bytes: portable_usize(16 * 1024 * 1024 * 1024),
            max_mapped_output_slot_bytes: portable_usize(16 * 1024 * 1024 * 1024),
            max_total_source_terms: portable_usize(LARGE),
            max_total_source_exponent_entries: portable_usize(HUGE),
            max_total_expanded_contributions: portable_usize(LARGE),
            max_total_output_term_bound: portable_usize(LARGE),
            max_total_output_terms: portable_usize(LARGE),
            max_total_output_exponent_entry_bound: portable_usize(HUGE),
            max_total_output_exponent_entries: portable_usize(HUGE),
            max_total_power_calls: portable_usize(HUGE),
            max_total_native_power_heap_pairs: portable_usize(HUGE),
            max_total_multiplication_term_pairs: portable_usize(HUGE),
            max_total_addition_term_visits: portable_usize(HUGE),
            max_total_native_integer_bit_work: portable_usize(HUGE),
            max_total_integer_bit_work: portable_usize(HUGE),
            max_mapped_output_retained_byte_bound: portable_usize(32 * 1024 * 1024 * 1024),
            max_mapped_output_observed_retained_bytes: portable_usize(32 * 1024 * 1024 * 1024),
            max_retained_owned_bytes: portable_usize(64 * 1024 * 1024 * 1024),
            // The nested accumulator's default aggregate temporary census is
            // intentionally much larger than ordinary resident memory.  This
            // private worker therefore preserves that finite contract; a
            // source-owning coordinator must install the job's RAM budget.
            max_compilation_owned_peak_upper_bound: portable_usize(256_000_000_000_000),
        }
    }
}

/// Aggregate composition census.  Prospective output sizes are bounds;
/// execution output sizes are observed values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseMappedNonZeroCompositionStats {
    source_terms: usize,
    source_exponent_entries: usize,
    expanded_contribution_bound: usize,
    output_term_bound: usize,
    output_terms: usize,
    output_exponent_entry_bound: usize,
    output_exponent_entries: usize,
    power_calls: usize,
    native_power_heap_pairs: usize,
    multiplication_term_pairs: usize,
    addition_term_visits: usize,
    largest_kronecker_exponent_bits: usize,
    largest_integer_coefficient_bit_bound: usize,
    native_integer_bit_work: usize,
    integer_bit_work: usize,
}

macro_rules! mapped_nonzero_composition_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualCaseMappedNonZeroCompositionStats {
    mapped_nonzero_composition_stats_getters!(
        source_terms,
        source_exponent_entries,
        expanded_contribution_bound,
        output_term_bound,
        output_terms,
        output_exponent_entry_bound,
        output_exponent_entries,
        power_calls,
        native_power_heap_pairs,
        multiplication_term_pairs,
        addition_term_visits,
        largest_kronecker_exponent_bits,
        largest_integer_coefficient_bit_bound,
        native_integer_bit_work,
        integer_bit_work,
    );
}

/// Exact execution transcript plus prospective memory admission.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseMappedNonZeroStats {
    source_inputs: usize,
    plan_replay_scratch_byte_envelope: usize,
    prepared_token_byte_envelope: usize,
    prepared_token_bytes: usize,
    mapped_output_slot_byte_envelope: usize,
    mapped_output_slot_bytes: usize,
    preflight: GeneratedAffineResidualCaseMappedNonZeroCompositionStats,
    execution: GeneratedAffineResidualCaseMappedNonZeroCompositionStats,
    executed_inputs: usize,
    mapped_zero_inputs: usize,
    mapped_output_retained_byte_bound: usize,
    mapped_output_observed_retained_bytes: usize,
    admitted_accumulator_temporary_byte_envelope: usize,
    accumulator_temporary_byte_envelope: usize,
    accumulator_retained_bytes: usize,
    retained_owned_bytes: usize,
    admitted_compilation_owned_peak_upper_bound: usize,
    compilation_owned_peak_upper_bound: usize,
}

impl GeneratedAffineResidualCaseMappedNonZeroStats {
    pub(crate) const fn source_inputs(self) -> usize {
        self.source_inputs
    }

    pub(crate) const fn plan_replay_scratch_byte_envelope(self) -> usize {
        self.plan_replay_scratch_byte_envelope
    }

    pub(crate) const fn prepared_token_byte_envelope(self) -> usize {
        self.prepared_token_byte_envelope
    }

    pub(crate) const fn prepared_token_bytes(self) -> usize {
        self.prepared_token_bytes
    }

    pub(crate) const fn mapped_output_slot_byte_envelope(self) -> usize {
        self.mapped_output_slot_byte_envelope
    }

    pub(crate) const fn mapped_output_slot_bytes(self) -> usize {
        self.mapped_output_slot_bytes
    }

    pub(crate) const fn preflight(
        self,
    ) -> GeneratedAffineResidualCaseMappedNonZeroCompositionStats {
        self.preflight
    }

    pub(crate) const fn execution(
        self,
    ) -> GeneratedAffineResidualCaseMappedNonZeroCompositionStats {
        self.execution
    }

    pub(crate) const fn executed_inputs(self) -> usize {
        self.executed_inputs
    }

    pub(crate) const fn mapped_zero_inputs(self) -> usize {
        self.mapped_zero_inputs
    }

    pub(crate) const fn mapped_output_retained_byte_bound(self) -> usize {
        self.mapped_output_retained_byte_bound
    }

    pub(crate) const fn mapped_output_observed_retained_bytes(self) -> usize {
        self.mapped_output_observed_retained_bytes
    }

    pub(crate) const fn admitted_accumulator_temporary_byte_envelope(self) -> usize {
        self.admitted_accumulator_temporary_byte_envelope
    }

    pub(crate) const fn accumulator_temporary_byte_envelope(self) -> usize {
        self.accumulator_temporary_byte_envelope
    }

    pub(crate) const fn accumulator_retained_bytes(self) -> usize {
        self.accumulator_retained_bytes
    }

    pub(crate) const fn retained_owned_bytes(self) -> usize {
        self.retained_owned_bytes
    }

    pub(crate) const fn admitted_compilation_owned_peak_upper_bound(self) -> usize {
        self.admitted_compilation_owned_peak_upper_bound
    }

    pub(crate) const fn compilation_owned_peak_upper_bound(self) -> usize {
        self.compilation_owned_peak_upper_bound
    }
}

/// Durable canonical inherited conditions after successful mapping.
#[derive(PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseMappedNonZeroCertificate {
    schema: &'static str,
    conditions: GeneratedResidualAffineConditionAccumulatorCertificate,
    limits: GeneratedAffineResidualCaseMappedNonZeroLimits,
    stats: GeneratedAffineResidualCaseMappedNonZeroStats,
}

impl GeneratedAffineResidualCaseMappedNonZeroCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn conditions(
        &self,
    ) -> &GeneratedResidualAffineConditionAccumulatorCertificate {
        &self.conditions
    }

    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCaseMappedNonZeroLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCaseMappedNonZeroStats {
        self.stats
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn is_branch_pruning_authority(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseMappedNonZeroCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseMappedNonZeroCertificate")
            .field("schema", &self.schema)
            .field("conditions", &self.conditions)
            .field("limits", &self.limits)
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .finish()
    }
}

/// Non-authoritative first-witness diagnostic for an inherited predicate that
/// became identically zero on the child locus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseMappedNonZeroEmptyDiagnostic {
    input_ordinal: usize,
    source: GeneratedResidualAffineConditionSourceLocator,
    limits: GeneratedAffineResidualCaseMappedNonZeroLimits,
    stats: GeneratedAffineResidualCaseMappedNonZeroStats,
}

impl GeneratedAffineResidualCaseMappedNonZeroEmptyDiagnostic {
    pub(crate) const fn input_ordinal(self) -> usize {
        self.input_ordinal
    }

    pub(crate) const fn source(self) -> GeneratedResidualAffineConditionSourceLocator {
        self.source
    }

    pub(crate) const fn limits(self) -> GeneratedAffineResidualCaseMappedNonZeroLimits {
        self.limits
    }

    pub(crate) const fn stats(self) -> GeneratedAffineResidualCaseMappedNonZeroStats {
        self.stats
    }

    pub(crate) const fn targets_consumed(self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(self) -> bool {
        false
    }

    pub(crate) const fn is_branch_pruning_authority(self) -> bool {
        false
    }

    pub(crate) const fn infers_master(self) -> bool {
        false
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseMappedNonZeroOutcome {
    Ready(GeneratedAffineResidualCaseMappedNonZeroCertificate),
    ProvedEmptyDiagnostic(GeneratedAffineResidualCaseMappedNonZeroEmptyDiagnostic),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseMappedNonZeroError {
    InvalidSource {
        input_ordinal: usize,
    },
    SourceBinding {
        input_ordinal: usize,
    },
    InternalInvariant {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Composition(ResidualUnitAffineCompositionError),
    Accumulator(GeneratedResidualAffineConditionAccumulatorError),
    CompilerPanic {
        stage: &'static str,
    },
}

impl fmt::Display for GeneratedAffineResidualCaseMappedNonZeroError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSource { input_ordinal } => write!(
                formatter,
                "mapped NonZero input {input_ordinal} is not an inherited target or exceptional predicate"
            ),
            Self::SourceBinding { input_ordinal } => write!(
                formatter,
                "mapped NonZero input {input_ordinal} violates source ordering or source-class invariants"
            ),
            Self::InternalInvariant { resource } => {
                write!(
                    formatter,
                    "mapped NonZero internal invariant failed for {resource}"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "mapped NonZero {resource} needs {requested} units, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "mapped NonZero {resource} count overflowed usize"
                )
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "mapped NonZero {resource} allocation of {requested} entries failed after bounded preflight"
            ),
            Self::Composition(error) => error.fmt(formatter),
            Self::Accumulator(error) => error.fmt(formatter),
            Self::CompilerPanic { stage } => {
                write!(formatter, "mapped NonZero compiler panicked during {stage}")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineResidualCaseMappedNonZeroError {}

impl From<ResidualUnitAffineCompositionError> for GeneratedAffineResidualCaseMappedNonZeroError {
    fn from(value: ResidualUnitAffineCompositionError) -> Self {
        Self::Composition(value)
    }
}

impl From<GeneratedResidualAffineConditionAccumulatorError>
    for GeneratedAffineResidualCaseMappedNonZeroError
{
    fn from(value: GeneratedResidualAffineConditionAccumulatorError) -> Self {
        Self::Accumulator(value)
    }
}

struct PreparedMappedNonZero<'prepared> {
    source: GeneratedResidualAffineConditionSourceLocator,
    token: PreparedResidualAffineCompactGuardComposition<'prepared>,
    output_retained_byte_envelope: usize,
}

struct MappedNonZero {
    source: GeneratedResidualAffineConditionSourceLocator,
    polynomial: ParametricPolynomial,
}

/// Map an authenticated, source-ordered inherited `NonZero` stream through
/// one already-compiled child affine plan.
fn compile_generated_affine_residual_case_mapped_nonzero<'prepared>(
    context: &'prepared ParametricCoefficientContext,
    child_geometry: ResidualAffineCompactMapView<'_>,
    child_plan: &'prepared ResidualAffineCompactCompositionPlan,
    inputs: &'prepared [GeneratedAffineResidualCaseMappedNonZeroInput<'prepared>],
    limits: GeneratedAffineResidualCaseMappedNonZeroLimits,
) -> Result<
    GeneratedAffineResidualCaseMappedNonZeroOutcome,
    GeneratedAffineResidualCaseMappedNonZeroError,
> {
    catch_unwind(AssertUnwindSafe(|| {
        compile_generated_affine_residual_case_mapped_nonzero_inner(
            context,
            child_geometry,
            child_plan,
            inputs,
            limits,
        )
    }))
    .map_err(
        |_| GeneratedAffineResidualCaseMappedNonZeroError::CompilerPanic {
            stage: "compilation boundary",
        },
    )?
}

fn compile_generated_affine_residual_case_mapped_nonzero_inner<'prepared>(
    context: &'prepared ParametricCoefficientContext,
    child_geometry: ResidualAffineCompactMapView<'_>,
    child_plan: &'prepared ResidualAffineCompactCompositionPlan,
    inputs: &'prepared [GeneratedAffineResidualCaseMappedNonZeroInput<'prepared>],
    limits: GeneratedAffineResidualCaseMappedNonZeroLimits,
) -> Result<
    GeneratedAffineResidualCaseMappedNonZeroOutcome,
    GeneratedAffineResidualCaseMappedNonZeroError,
> {
    check_limit("source inputs", inputs.len(), limits.max_source_inputs)?;
    for (resource, limit) in [
        (
            "accumulator condition inputs",
            limits.accumulator.max_condition_inputs,
        ),
        (
            "accumulator source inputs",
            limits.accumulator.max_source_inputs,
        ),
        (
            "accumulator condition sources",
            limits.accumulator.max_condition_sources,
        ),
    ] {
        check_limit(resource, inputs.len(), limit)?;
    }
    validate_source_stream(inputs)?;

    let mut stats = GeneratedAffineResidualCaseMappedNonZeroStats {
        source_inputs: inputs.len(),
        ..GeneratedAffineResidualCaseMappedNonZeroStats::default()
    };
    stats.plan_replay_scratch_byte_envelope = plan_replay_scratch_byte_envelope(child_geometry)?;
    stats.prepared_token_byte_envelope = capacity_byte_envelope::<PreparedMappedNonZero<'_>>(
        inputs.len(),
        "prepared token byte envelope",
    )?;
    check_limit(
        "prepared token bytes",
        stats.prepared_token_byte_envelope,
        limits.max_prepared_token_bytes,
    )?;
    stats.mapped_output_slot_byte_envelope =
        capacity_byte_envelope::<MappedNonZero>(inputs.len(), "mapped output slot byte envelope")?;
    check_limit(
        "mapped output slot bytes",
        stats.mapped_output_slot_byte_envelope,
        limits.max_mapped_output_slot_bytes,
    )?;

    let certificate_extra = certificate_outer_bytes()?;
    let empty_diagnostic_retained_bytes =
        size_of::<GeneratedAffineResidualCaseMappedNonZeroEmptyDiagnostic>();
    let final_retained_bound = checked_add(
        "mapped NonZero retained owned bytes",
        limits.accumulator.max_retained_bytes,
        certificate_extra,
    )?;
    check_limit(
        "retained owned bytes",
        final_retained_bound,
        limits.max_retained_owned_bytes,
    )?;
    let early_owned_peak = stats
        .plan_replay_scratch_byte_envelope
        .max(stats.prepared_token_byte_envelope)
        .max(final_retained_bound)
        .max(empty_diagnostic_retained_bytes);
    check_limit(
        "compilation owned peak upper bound",
        early_owned_peak,
        limits.max_compilation_owned_peak_upper_bound,
    )?;

    child_plan.replay(context, child_geometry)?;
    stats.compilation_owned_peak_upper_bound = stats.plan_replay_scratch_byte_envelope;

    let mut prepared = Vec::new();
    prepared.try_reserve_exact(inputs.len()).map_err(|_| {
        GeneratedAffineResidualCaseMappedNonZeroError::AllocationFailure {
            resource: "prepared composition tokens",
            requested: inputs.len(),
        }
    })?;
    stats.prepared_token_bytes = checked_mul(
        "prepared token bytes",
        prepared.capacity(),
        size_of::<PreparedMappedNonZero<'_>>(),
    )?;
    check_limit(
        "prepared token bytes",
        stats.prepared_token_bytes,
        limits.max_prepared_token_bytes,
    )?;
    if stats.prepared_token_bytes > stats.prepared_token_byte_envelope {
        return Err(
            GeneratedAffineResidualCaseMappedNonZeroError::InternalInvariant {
                resource: "prepared composition token capacity envelope",
            },
        );
    }
    stats.compilation_owned_peak_upper_bound = stats
        .compilation_owned_peak_upper_bound
        .max(stats.prepared_token_bytes);

    for input in inputs {
        let child_limits = remaining_composition_limits(limits, stats.preflight)?;
        let token = context.prepare_guard_on_residual_affine_compact_composition_plan(
            input.polynomial,
            child_plan,
            child_limits,
        )?;
        let prospective = token.stats();
        merge_composition_stats(&mut stats.preflight, prospective, limits)?;
        let output_envelope =
            residual_affine_composition_output_retained_byte_envelope(prospective)?;
        stats.mapped_output_retained_byte_bound = bounded_add(
            "mapped output retained byte bound",
            stats.mapped_output_retained_byte_bound,
            output_envelope,
            limits.max_mapped_output_retained_byte_bound,
        )?;
        prepared.push(PreparedMappedNonZero {
            source: input.source,
            token,
            output_retained_byte_envelope: output_envelope,
        });
    }
    if prepared.len() != inputs.len() {
        return Err(
            GeneratedAffineResidualCaseMappedNonZeroError::InternalInvariant {
                resource: "prepared composition token count",
            },
        );
    }

    let mapping_live_bound = checked_sum(
        "mapping compilation owned peak upper bound",
        [
            stats.prepared_token_byte_envelope,
            stats.mapped_output_slot_byte_envelope,
            stats.mapped_output_retained_byte_bound,
            empty_diagnostic_retained_bytes,
        ],
    )?;
    stats.admitted_accumulator_temporary_byte_envelope = limits
        .accumulator
        .max_associate_combined_temporary_byte_envelope
        .max(
            limits
                .accumulator
                .max_base_associate_combined_temporary_byte_envelope,
        );
    let accumulator_live_bound = checked_sum(
        "accumulator compilation owned peak upper bound",
        [
            stats.mapped_output_slot_byte_envelope,
            stats.mapped_output_retained_byte_bound,
            limits.accumulator.max_retained_bytes,
            stats.admitted_accumulator_temporary_byte_envelope,
            certificate_extra,
        ],
    )?;
    stats.admitted_compilation_owned_peak_upper_bound = early_owned_peak
        .max(mapping_live_bound)
        .max(accumulator_live_bound)
        .max(final_retained_bound);
    check_limit(
        "compilation owned peak upper bound",
        stats.admitted_compilation_owned_peak_upper_bound,
        limits.max_compilation_owned_peak_upper_bound,
    )?;

    let mut mapped = Vec::new();
    mapped.try_reserve_exact(inputs.len()).map_err(|_| {
        GeneratedAffineResidualCaseMappedNonZeroError::AllocationFailure {
            resource: "mapped predicate outputs",
            requested: inputs.len(),
        }
    })?;
    stats.mapped_output_slot_bytes = checked_mul(
        "mapped output slot bytes",
        mapped.capacity(),
        size_of::<MappedNonZero>(),
    )?;
    check_limit(
        "mapped output slot bytes",
        stats.mapped_output_slot_bytes,
        limits.max_mapped_output_slot_bytes,
    )?;
    if stats.mapped_output_slot_bytes > stats.mapped_output_slot_byte_envelope {
        return Err(
            GeneratedAffineResidualCaseMappedNonZeroError::InternalInvariant {
                resource: "mapped output slot capacity envelope",
            },
        );
    }
    stats.compilation_owned_peak_upper_bound =
        stats.compilation_owned_peak_upper_bound.max(checked_add(
            "mapping observed compilation owned peak upper bound",
            stats.prepared_token_bytes,
            stats.mapped_output_slot_bytes,
        )?);

    for (input_ordinal, prepared) in prepared.into_iter().enumerate() {
        let prospective = prepared.token.stats();
        let (polynomial, observed) = prepared.token.execute()?.into_parts();
        note_mapped_nonzero_execution_for_test();
        if !execution_fits_preflight(observed, prospective) {
            return Err(
                GeneratedAffineResidualCaseMappedNonZeroError::InternalInvariant {
                    resource: "composition execution/preflight census",
                },
            );
        }
        merge_composition_stats(&mut stats.execution, observed, limits)?;
        stats.executed_inputs = checked_add("executed inputs", stats.executed_inputs, 1)?;
        let observed_bytes = polynomial.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCaseMappedNonZeroError::ResourceCountOverflow {
                resource: "mapped output observed retained bytes",
            },
        )?;
        if observed_bytes > prepared.output_retained_byte_envelope {
            return Err(
                GeneratedAffineResidualCaseMappedNonZeroError::InternalInvariant {
                    resource: "mapped polynomial retained-byte envelope",
                },
            );
        }
        stats.mapped_output_observed_retained_bytes = bounded_add(
            "mapped output observed retained bytes",
            stats.mapped_output_observed_retained_bytes,
            observed_bytes,
            limits.max_mapped_output_observed_retained_bytes,
        )?;
        if stats.mapped_output_observed_retained_bytes > stats.mapped_output_retained_byte_bound {
            return Err(
                GeneratedAffineResidualCaseMappedNonZeroError::InternalInvariant {
                    resource: "aggregate mapped polynomial retained-byte envelope",
                },
            );
        }
        stats.compilation_owned_peak_upper_bound =
            stats.compilation_owned_peak_upper_bound.max(checked_sum(
                "mapping observed compilation owned peak upper bound",
                [
                    stats.prepared_token_bytes,
                    stats.mapped_output_slot_bytes,
                    stats.mapped_output_observed_retained_bytes,
                ],
            )?);

        if polynomial.is_zero() {
            stats.mapped_zero_inputs = 1;
            stats.retained_owned_bytes = empty_diagnostic_retained_bytes;
            stats.compilation_owned_peak_upper_bound =
                stats.compilation_owned_peak_upper_bound.max(checked_sum(
                    "mapped-zero observed compilation owned peak upper bound",
                    [
                        stats.prepared_token_bytes,
                        stats.mapped_output_slot_bytes,
                        stats.mapped_output_observed_retained_bytes,
                        stats.retained_owned_bytes,
                    ],
                )?);
            check_limit(
                "retained owned bytes",
                stats.retained_owned_bytes,
                limits.max_retained_owned_bytes,
            )?;
            if stats.compilation_owned_peak_upper_bound
                > stats.admitted_compilation_owned_peak_upper_bound
            {
                return Err(
                    GeneratedAffineResidualCaseMappedNonZeroError::InternalInvariant {
                        resource: "mapped-zero compilation owned peak admission",
                    },
                );
            }
            return Ok(
                GeneratedAffineResidualCaseMappedNonZeroOutcome::ProvedEmptyDiagnostic(
                    GeneratedAffineResidualCaseMappedNonZeroEmptyDiagnostic {
                        input_ordinal,
                        source: prepared.source,
                        limits,
                        stats,
                    },
                ),
            );
        }
        mapped.push(MappedNonZero {
            source: prepared.source,
            polynomial,
        });
    }

    if stats.executed_inputs != inputs.len()
        || !execution_aggregate_fits_preflight(stats.execution, stats.preflight)
    {
        return Err(
            GeneratedAffineResidualCaseMappedNonZeroError::InternalInvariant {
                resource: "aggregate composition execution/preflight census",
            },
        );
    }

    let conditions = accumulate_generated_residual_affine_conditions(
        context,
        child_plan.free_positions(),
        mapped.iter().map(|mapped| {
            GeneratedResidualAffineConditionInput::new(
                &mapped.polynomial,
                GeneratedResidualAffineConditionScope::InheritedTargetPremise,
                mapped.source,
                None,
            )
        }),
        limits.accumulator,
    )?;
    stats.accumulator_retained_bytes = conditions.stats().retained_bytes();
    stats.accumulator_temporary_byte_envelope = conditions
        .stats()
        .associate_combined_temporary_byte_envelope()
        .max(
            conditions
                .stats()
                .base_associate_combined_temporary_byte_envelope(),
        );
    stats.compilation_owned_peak_upper_bound =
        stats.compilation_owned_peak_upper_bound.max(checked_sum(
            "accumulator observed compilation owned peak upper bound",
            [
                stats.mapped_output_slot_bytes,
                stats.mapped_output_observed_retained_bytes,
                stats.accumulator_retained_bytes,
                stats.accumulator_temporary_byte_envelope,
                certificate_extra,
            ],
        )?);
    stats.retained_owned_bytes = checked_add(
        "mapped NonZero retained owned bytes",
        stats.accumulator_retained_bytes,
        certificate_extra,
    )?;
    check_limit(
        "retained owned bytes",
        stats.retained_owned_bytes,
        limits.max_retained_owned_bytes,
    )?;
    stats.compilation_owned_peak_upper_bound = stats
        .compilation_owned_peak_upper_bound
        .max(stats.retained_owned_bytes);
    check_limit(
        "compilation owned peak upper bound",
        stats.compilation_owned_peak_upper_bound,
        limits.max_compilation_owned_peak_upper_bound,
    )?;

    Ok(GeneratedAffineResidualCaseMappedNonZeroOutcome::Ready(
        GeneratedAffineResidualCaseMappedNonZeroCertificate {
            schema: GENERATED_AFFINE_RESIDUAL_CASE_MAPPED_NONZERO_V1_SCHEMA,
            conditions,
            limits,
            stats,
        },
    ))
}

fn is_inherited_nonzero_source(source: GeneratedResidualAffineConditionSourceLocator) -> bool {
    matches!(
        source,
        GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard { .. }
            | GeneratedResidualAffineConditionSourceLocator::ExceptionalNonZeroPredicate { .. }
    )
}

fn validate_source_stream(
    inputs: &[GeneratedAffineResidualCaseMappedNonZeroInput<'_>],
) -> Result<(), GeneratedAffineResidualCaseMappedNonZeroError> {
    let mut saw_exceptional = false;
    let mut last_target = None;
    let mut last_exceptional = None;
    for (input_ordinal, input) in inputs.iter().enumerate() {
        if !is_inherited_nonzero_source(input.source) {
            return Err(
                GeneratedAffineResidualCaseMappedNonZeroError::InvalidSource { input_ordinal },
            );
        }
        if input.polynomial.is_zero() {
            return Err(
                GeneratedAffineResidualCaseMappedNonZeroError::SourceBinding { input_ordinal },
            );
        }
        match input.source {
            GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                entry_ordinal,
                ..
            } => {
                if saw_exceptional
                    || last_target.is_some_and(|prior| entry_ordinal <= prior)
                    || input.polynomial.is_nonzero_constant()
                {
                    return Err(
                        GeneratedAffineResidualCaseMappedNonZeroError::SourceBinding {
                            input_ordinal,
                        },
                    );
                }
                last_target = Some(entry_ordinal);
            }
            GeneratedResidualAffineConditionSourceLocator::ExceptionalNonZeroPredicate {
                predicate_ordinal,
                ..
            } => {
                saw_exceptional = true;
                if last_exceptional.is_some_and(|prior| predicate_ordinal <= prior) {
                    return Err(
                        GeneratedAffineResidualCaseMappedNonZeroError::SourceBinding {
                            input_ordinal,
                        },
                    );
                }
                last_exceptional = Some(predicate_ordinal);
            }
            _ => unreachable!("source class was checked above"),
        }
    }
    Ok(())
}

fn plan_replay_scratch_byte_envelope(
    geometry: ResidualAffineCompactMapView<'_>,
) -> Result<usize, GeneratedAffineResidualCaseMappedNonZeroError> {
    let free_slots = checked_add(
        "plan replay scratch byte envelope",
        geometry.free_positions().len(),
        1,
    )?;
    let entries = checked_add(
        "plan replay scratch byte envelope",
        geometry.ambient_arity(),
        free_slots,
    )?;
    checked_add(
        "plan replay scratch byte envelope",
        checked_mul(
            "plan replay scratch byte envelope",
            2,
            size_of::<Vec<usize>>(),
        )?,
        checked_mul(
            "plan replay scratch byte envelope",
            checked_mul("plan replay scratch byte envelope", entries, 2)?,
            size_of::<usize>(),
        )?,
    )
}

fn certificate_outer_bytes() -> Result<usize, GeneratedAffineResidualCaseMappedNonZeroError> {
    size_of::<GeneratedAffineResidualCaseMappedNonZeroCertificate>()
        .checked_sub(size_of::<
            GeneratedResidualAffineConditionAccumulatorCertificate,
        >())
        .ok_or(
            GeneratedAffineResidualCaseMappedNonZeroError::ResourceCountOverflow {
                resource: "mapped NonZero certificate outer bytes",
            },
        )
}

fn remaining_composition_limits(
    limits: GeneratedAffineResidualCaseMappedNonZeroLimits,
    prior: GeneratedAffineResidualCaseMappedNonZeroCompositionStats,
) -> Result<
    ResidualUnitAffinePolynomialCompositionLimits,
    GeneratedAffineResidualCaseMappedNonZeroError,
> {
    let mut child = limits.composition;
    macro_rules! clamp {
        ($field:ident, $used:ident, $total:ident, $resource:literal) => {
            child.$field = child
                .$field
                .min(remaining($resource, limits.$total, prior.$used)?);
        };
    }
    clamp!(
        max_source_terms,
        source_terms,
        max_total_source_terms,
        "total source terms"
    );
    clamp!(
        max_source_exponent_entries,
        source_exponent_entries,
        max_total_source_exponent_entries,
        "total source exponent entries"
    );
    clamp!(
        max_expanded_contributions,
        expanded_contribution_bound,
        max_total_expanded_contributions,
        "total expanded contributions"
    );
    clamp!(
        max_output_terms,
        output_term_bound,
        max_total_output_term_bound,
        "total output term bound"
    );
    clamp!(
        max_output_exponent_entries,
        output_exponent_entry_bound,
        max_total_output_exponent_entry_bound,
        "total output exponent entry bound"
    );
    clamp!(
        max_power_calls,
        power_calls,
        max_total_power_calls,
        "total power calls"
    );
    clamp!(
        max_native_power_heap_pairs,
        native_power_heap_pairs,
        max_total_native_power_heap_pairs,
        "total native power heap pairs"
    );
    clamp!(
        max_multiplication_term_pairs,
        multiplication_term_pairs,
        max_total_multiplication_term_pairs,
        "total multiplication term pairs"
    );
    clamp!(
        max_addition_term_visits,
        addition_term_visits,
        max_total_addition_term_visits,
        "total addition term visits"
    );
    clamp!(
        max_native_integer_bit_work,
        native_integer_bit_work,
        max_total_native_integer_bit_work,
        "total native integer bit work"
    );
    clamp!(
        max_integer_bit_work,
        integer_bit_work,
        max_total_integer_bit_work,
        "total integer bit work"
    );
    Ok(child)
}

fn merge_composition_stats(
    aggregate: &mut GeneratedAffineResidualCaseMappedNonZeroCompositionStats,
    item: ResidualUnitAffinePolynomialCompositionStats,
    limits: GeneratedAffineResidualCaseMappedNonZeroLimits,
) -> Result<(), GeneratedAffineResidualCaseMappedNonZeroError> {
    macro_rules! add {
        ($field:ident, $value:expr, $limit:ident, $resource:literal) => {
            aggregate.$field = bounded_add($resource, aggregate.$field, $value, limits.$limit)?;
        };
    }
    add!(
        source_terms,
        item.source_terms(),
        max_total_source_terms,
        "total source terms"
    );
    add!(
        source_exponent_entries,
        item.source_exponent_entries(),
        max_total_source_exponent_entries,
        "total source exponent entries"
    );
    add!(
        expanded_contribution_bound,
        item.expanded_contribution_bound(),
        max_total_expanded_contributions,
        "total expanded contributions"
    );
    add!(
        output_term_bound,
        item.expanded_contribution_bound(),
        max_total_output_term_bound,
        "total output term bound"
    );
    add!(
        output_terms,
        item.output_terms(),
        max_total_output_terms,
        "total output terms"
    );
    add!(
        output_exponent_entry_bound,
        item.output_exponent_entry_bound(),
        max_total_output_exponent_entry_bound,
        "total output exponent entry bound"
    );
    add!(
        output_exponent_entries,
        item.output_exponent_entries(),
        max_total_output_exponent_entries,
        "total output exponent entries"
    );
    add!(
        power_calls,
        item.power_calls(),
        max_total_power_calls,
        "total power calls"
    );
    add!(
        native_power_heap_pairs,
        item.native_power_heap_pair_bound(),
        max_total_native_power_heap_pairs,
        "total native power heap pairs"
    );
    add!(
        multiplication_term_pairs,
        item.multiplication_term_pair_bound(),
        max_total_multiplication_term_pairs,
        "total multiplication term pairs"
    );
    add!(
        addition_term_visits,
        item.addition_term_visit_bound(),
        max_total_addition_term_visits,
        "total addition term visits"
    );
    aggregate.largest_kronecker_exponent_bits = aggregate
        .largest_kronecker_exponent_bits
        .max(item.largest_kronecker_exponent_bits());
    aggregate.largest_integer_coefficient_bit_bound = aggregate
        .largest_integer_coefficient_bit_bound
        .max(item.largest_integer_coefficient_bit_bound());
    add!(
        native_integer_bit_work,
        item.native_integer_bit_work_bound(),
        max_total_native_integer_bit_work,
        "total native integer bit work"
    );
    add!(
        integer_bit_work,
        item.integer_bit_work_bound(),
        max_total_integer_bit_work,
        "total integer bit work"
    );
    Ok(())
}

fn execution_fits_preflight(
    actual: ResidualUnitAffinePolynomialCompositionStats,
    prospective: ResidualUnitAffinePolynomialCompositionStats,
) -> bool {
    actual.source_terms() == prospective.source_terms()
        && actual.source_exponent_entries() == prospective.source_exponent_entries()
        && actual.expanded_contribution_bound() == prospective.expanded_contribution_bound()
        && actual.output_exponent_entry_bound() == prospective.output_exponent_entry_bound()
        && actual.power_calls() == prospective.power_calls()
        && actual.native_power_heap_pair_bound() == prospective.native_power_heap_pair_bound()
        && actual.multiplication_term_pair_bound() == prospective.multiplication_term_pair_bound()
        && actual.addition_term_visit_bound() == prospective.addition_term_visit_bound()
        && actual.largest_kronecker_exponent_bits() == prospective.largest_kronecker_exponent_bits()
        && actual.largest_integer_coefficient_bit_bound()
            == prospective.largest_integer_coefficient_bit_bound()
        && actual.native_integer_bit_work_bound() == prospective.native_integer_bit_work_bound()
        && actual.output_terms() <= prospective.expanded_contribution_bound()
        && actual.output_exponent_entries() <= prospective.output_exponent_entry_bound()
        && actual.integer_bit_work_bound() <= prospective.integer_bit_work_bound()
}

fn execution_aggregate_fits_preflight(
    actual: GeneratedAffineResidualCaseMappedNonZeroCompositionStats,
    prospective: GeneratedAffineResidualCaseMappedNonZeroCompositionStats,
) -> bool {
    actual.source_terms == prospective.source_terms
        && actual.source_exponent_entries == prospective.source_exponent_entries
        && actual.expanded_contribution_bound == prospective.expanded_contribution_bound
        && actual.output_term_bound == prospective.output_term_bound
        && actual.output_exponent_entry_bound == prospective.output_exponent_entry_bound
        && actual.power_calls == prospective.power_calls
        && actual.native_power_heap_pairs == prospective.native_power_heap_pairs
        && actual.multiplication_term_pairs == prospective.multiplication_term_pairs
        && actual.addition_term_visits == prospective.addition_term_visits
        && actual.largest_kronecker_exponent_bits == prospective.largest_kronecker_exponent_bits
        && actual.largest_integer_coefficient_bit_bound
            == prospective.largest_integer_coefficient_bit_bound
        && actual.native_integer_bit_work == prospective.native_integer_bit_work
        && actual.output_terms <= prospective.output_term_bound
        && actual.output_exponent_entries <= prospective.output_exponent_entry_bound
        && actual.integer_bit_work <= prospective.integer_bit_work
}

fn capacity_byte_envelope<T>(
    entries: usize,
    resource: &'static str,
) -> Result<usize, GeneratedAffineResidualCaseMappedNonZeroError> {
    checked_mul(resource, checked_mul(resource, entries, 2)?, size_of::<T>())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCaseMappedNonZeroError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(
            GeneratedAffineResidualCaseMappedNonZeroError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    }
}

fn remaining(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, GeneratedAffineResidualCaseMappedNonZeroError> {
    limit.checked_sub(used).ok_or(
        GeneratedAffineResidualCaseMappedNonZeroError::ResourceLimit {
            resource,
            requested: used,
            limit,
        },
    )
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualCaseMappedNonZeroError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseMappedNonZeroError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualCaseMappedNonZeroError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseMappedNonZeroError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualCaseMappedNonZeroError::ResourceCountOverflow { resource })
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedAffineResidualCaseMappedNonZeroError> {
    values
        .into_iter()
        .try_fold(0usize, |sum, value| checked_add(resource, sum, value))
}

const fn portable_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::generated_residual_affine_condition_accumulator::GeneratedResidualAffineConditionInputClass;
    use crate::parametric_coefficient::ResidualAffineCompactCompositionPlanLimits;
    use symbolica::domains::integer::Integer;

    struct OwnedGeometry {
        fingerprint: String,
        ambient_arity: usize,
        constants: Vec<Integer>,
        free_positions: Vec<usize>,
        linear: Vec<Integer>,
    }

    impl OwnedGeometry {
        fn identity(context: &ParametricCoefficientContext) -> Self {
            let arity = context.index_count();
            let mut linear = Vec::with_capacity(arity * arity);
            for row in 0..arity {
                for column in 0..arity {
                    linear.push(Integer::from(i64::from(row == column)));
                }
            }
            Self {
                fingerprint: context.fingerprint().to_owned(),
                ambient_arity: arity,
                constants: vec![Integer::from(0); arity],
                free_positions: (0..arity).collect(),
                linear,
            }
        }

        fn fixed(context: &ParametricCoefficientContext, constants: &[i64]) -> Self {
            Self {
                fingerprint: context.fingerprint().to_owned(),
                ambient_arity: context.index_count(),
                constants: constants.iter().copied().map(Integer::from).collect(),
                free_positions: Vec::new(),
                linear: Vec::new(),
            }
        }

        fn partial(
            context: &ParametricCoefficientContext,
            constants: &[i64],
            free_positions: &[usize],
            linear: &[i64],
        ) -> Self {
            Self {
                fingerprint: context.fingerprint().to_owned(),
                ambient_arity: context.index_count(),
                constants: constants.iter().copied().map(Integer::from).collect(),
                free_positions: free_positions.to_vec(),
                linear: linear.iter().copied().map(Integer::from).collect(),
            }
        }

        fn view(&self) -> ResidualAffineCompactMapView<'_> {
            ResidualAffineCompactMapView::new(
                &self.fingerprint,
                self.ambient_arity,
                &self.constants,
                &self.free_positions,
                &self.linear,
            )
        }
    }

    fn context(scope: &str, arity: usize) -> ParametricCoefficientContext {
        ParametricCoefficientContext::try_new(&CoefficientContext::new(["theta"]), scope, arity)
            .unwrap()
    }

    fn polynomial(
        context: &ParametricCoefficientContext,
        value: &crate::ParametricCoefficient,
    ) -> ParametricPolynomial {
        context.numerator_condition(value).unwrap()
    }

    fn target_input(
        polynomial: &ParametricPolynomial,
        ordinal: usize,
    ) -> GeneratedAffineResidualCaseMappedNonZeroInput<'_> {
        GeneratedAffineResidualCaseMappedNonZeroInput::new(
            polynomial,
            GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                entry_ordinal: ordinal,
                structural_locus_ordinal: 0,
            },
        )
    }

    fn exceptional_input(
        polynomial: &ParametricPolynomial,
        ordinal: usize,
    ) -> GeneratedAffineResidualCaseMappedNonZeroInput<'_> {
        GeneratedAffineResidualCaseMappedNonZeroInput::new(
            polynomial,
            GeneratedResidualAffineConditionSourceLocator::ExceptionalNonZeroPredicate {
                predicate_ordinal: ordinal,
                locus_ordinal: ordinal + 10,
            },
        )
    }

    fn compile_identity<'a>(
        context: &'a ParametricCoefficientContext,
        geometry: &OwnedGeometry,
        plan: &'a ResidualAffineCompactCompositionPlan,
        inputs: &'a [GeneratedAffineResidualCaseMappedNonZeroInput<'a>],
        limits: GeneratedAffineResidualCaseMappedNonZeroLimits,
    ) -> Result<
        GeneratedAffineResidualCaseMappedNonZeroOutcome,
        GeneratedAffineResidualCaseMappedNonZeroError,
    > {
        compile_generated_affine_residual_case_mapped_nonzero(
            context,
            geometry.view(),
            plan,
            inputs,
            limits,
        )
    }

    #[test]
    fn first_mapped_zero_returns_source_ordered_non_authoritative_diagnostic() {
        let context = context("mapped-nonzero-zero", 1);
        let geometry = OwnedGeometry::fixed(&context, &[1]);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let n0 = polynomial(&context, &context.index(0).unwrap());
        let n0_minus_one = polynomial(
            &context,
            &context
                .sub(&context.index(0).unwrap(), &context.one())
                .unwrap(),
        );
        let twice_n0_minus_two = polynomial(
            &context,
            &context
                .mul(
                    &context.integer(2),
                    &context
                        .sub(&context.index(0).unwrap(), &context.one())
                        .unwrap(),
                )
                .unwrap(),
        );
        let inputs = [
            target_input(&n0, 0),
            target_input(&n0_minus_one, 2),
            exceptional_input(&twice_n0_minus_two, 5),
        ];

        reset_mapped_nonzero_execution_probe_for_test();
        let outcome = compile_identity(
            &context,
            &geometry,
            &plan,
            &inputs,
            GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseMappedNonZeroOutcome::ProvedEmptyDiagnostic(diagnostic) =
            outcome
        else {
            panic!("expected mapped-zero diagnostic")
        };
        assert_eq!(diagnostic.input_ordinal(), 1);
        assert_eq!(diagnostic.source(), inputs[1].source());
        assert_eq!(diagnostic.stats().source_inputs(), 3);
        assert_eq!(diagnostic.stats().executed_inputs(), 2);
        assert_eq!(diagnostic.stats().mapped_zero_inputs(), 1);
        assert_eq!(mapped_nonzero_executions_for_test(), 2);
        assert_eq!(diagnostic.targets_consumed(), 0);
        assert!(!diagnostic.publishes_rule());
        assert!(!diagnostic.is_branch_pruning_authority());
        assert!(!diagnostic.infers_master());
    }

    #[test]
    fn mapped_zero_diagnostic_peak_has_exact_and_pre_execution_one_below_gates() {
        let context = context("mapped-nonzero-zero-peak", 1);
        let geometry = OwnedGeometry::fixed(&context, &[1]);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let n0 = polynomial(&context, &context.index(0).unwrap());
        let n0_minus_one = polynomial(
            &context,
            &context
                .sub(&context.index(0).unwrap(), &context.one())
                .unwrap(),
        );
        let inputs = [target_input(&n0, 0), target_input(&n0_minus_one, 1)];

        // Keep the unused downstream accumulator allowance from dominating
        // this mapped-zero path's live-set admission.
        let mut accounting_limits = GeneratedAffineResidualCaseMappedNonZeroLimits::default();
        accounting_limits.accumulator.max_retained_bytes = 0;
        accounting_limits
            .accumulator
            .max_associate_combined_temporary_byte_envelope = 0;
        accounting_limits
            .accumulator
            .max_base_associate_combined_temporary_byte_envelope = 0;

        let baseline =
            compile_identity(&context, &geometry, &plan, &inputs, accounting_limits).unwrap();
        let GeneratedAffineResidualCaseMappedNonZeroOutcome::ProvedEmptyDiagnostic(baseline) =
            baseline
        else {
            panic!("expected mapped-zero accounting baseline")
        };
        let stats = baseline.stats();
        let mapping_live_bound = checked_sum(
            "test mapped-zero mapping live bound",
            [
                stats.prepared_token_byte_envelope(),
                stats.mapped_output_slot_byte_envelope(),
                stats.mapped_output_retained_byte_bound(),
                size_of::<GeneratedAffineResidualCaseMappedNonZeroEmptyDiagnostic>(),
            ],
        )
        .unwrap();
        assert_eq!(
            stats.admitted_compilation_owned_peak_upper_bound(),
            mapping_live_bound
        );
        assert!(
            stats.compilation_owned_peak_upper_bound()
                <= stats.admitted_compilation_owned_peak_upper_bound()
        );

        let mut exact = accounting_limits;
        exact.max_compilation_owned_peak_upper_bound = mapping_live_bound;
        let exact_outcome = compile_identity(&context, &geometry, &plan, &inputs, exact).unwrap();
        let GeneratedAffineResidualCaseMappedNonZeroOutcome::ProvedEmptyDiagnostic(exact) =
            exact_outcome
        else {
            panic!("expected mapped-zero exact-boundary success")
        };
        assert!(
            exact.stats().compilation_owned_peak_upper_bound()
                <= exact.stats().admitted_compilation_owned_peak_upper_bound()
        );

        let mut one_below = accounting_limits;
        one_below.max_compilation_owned_peak_upper_bound = mapping_live_bound - 1;
        reset_mapped_nonzero_execution_probe_for_test();
        assert_eq!(
            compile_identity(&context, &geometry, &plan, &inputs, one_below).unwrap_err(),
            GeneratedAffineResidualCaseMappedNonZeroError::ResourceLimit {
                resource: "compilation owned peak upper bound",
                requested: mapping_live_bound,
                limit: mapping_live_bound - 1,
            }
        );
        assert_eq!(mapped_nonzero_executions_for_test(), 0);
    }

    #[test]
    fn constants_and_parameter_or_index_associates_use_the_correct_fields() {
        let context = context("mapped-nonzero-fields", 1);
        let geometry = OwnedGeometry::identity(&context);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let theta = context
            .lift(&context.base().parameter("theta").unwrap())
            .unwrap();
        let theta_plus_one = context.add(&theta, &context.one()).unwrap();
        let minus_two_theta = context
            .neg(&context.mul(&context.integer(2), &theta).unwrap())
            .unwrap();
        let one_minus_n0 = context
            .sub(&context.one(), &context.index(0).unwrap())
            .unwrap();
        let indexed = context.mul(&theta, &one_minus_n0).unwrap();
        let theta_squared = context.mul(&theta, &theta).unwrap();
        let indexed_scaled = context.mul(&theta_squared, &one_minus_n0).unwrap();
        let polynomials = [
            polynomial(&context, &theta),
            polynomial(&context, &theta_plus_one),
            polynomial(&context, &minus_two_theta),
            polynomial(&context, &indexed),
            polynomial(&context, &indexed_scaled),
            polynomial(&context, &context.integer(2)),
        ];
        let inputs = [
            target_input(&polynomials[0], 0),
            target_input(&polynomials[1], 1),
            target_input(&polynomials[3], 3),
            exceptional_input(&polynomials[2], 2),
            exceptional_input(&polynomials[4], 4),
            exceptional_input(&polynomials[5], 5),
        ];

        let outcome = compile_identity(
            &context,
            &geometry,
            &plan,
            &inputs,
            GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseMappedNonZeroOutcome::Ready(certificate) = outcome else {
            panic!("expected ready mapped conditions")
        };
        assert_eq!(
            certificate.schema(),
            GENERATED_AFFINE_RESIDUAL_CASE_MAPPED_NONZERO_V1_SCHEMA
        );
        assert_eq!(certificate.conditions().inputs().len(), 6);
        assert_eq!(certificate.conditions().rows().len(), 3);
        assert_eq!(certificate.conditions().stats().unique_base_rows(), 2);
        assert_eq!(
            certificate
                .conditions()
                .stats()
                .unique_index_dependent_rows(),
            1
        );
        assert_eq!(
            certificate
                .conditions()
                .stats()
                .discharged_nonzero_constants(),
            1
        );
        assert_eq!(
            certificate.conditions().rows()[0].source_input_ordinals(),
            &[0, 3]
        );
        assert_eq!(
            certificate.conditions().rows()[1].source_input_ordinals(),
            &[1]
        );
        assert_eq!(
            certificate.conditions().rows()[2].source_input_ordinals(),
            &[2, 4]
        );
        assert!(matches!(
            certificate.conditions().inputs()[0].class(),
            GeneratedResidualAffineConditionInputClass::BaseAssumption { row_ordinal: 0 }
        ));
        assert!(matches!(
            certificate.conditions().inputs()[2].class(),
            GeneratedResidualAffineConditionInputClass::IndexDependent { row_ordinal: 2 }
        ));
        assert!(matches!(
            certificate.conditions().inputs()[5].class(),
            GeneratedResidualAffineConditionInputClass::DischargedNonzeroIntegerConstant
        ));
        for (ordinal, input) in certificate.conditions().inputs().iter().enumerate() {
            assert_eq!(input.ordinal(), ordinal);
            assert_eq!(input.source().locator(), inputs[ordinal].source());
            assert_eq!(
                input.scope(),
                GeneratedResidualAffineConditionScope::InheritedTargetPremise
            );
        }
        assert_eq!(certificate.stats().executed_inputs(), inputs.len());
        assert_eq!(certificate.stats().mapped_zero_inputs(), 0);
        assert_eq!(certificate.targets_consumed(), 0);
        assert!(!certificate.publishes_rule());
        assert!(!certificate.is_branch_pruning_authority());
        assert!(!certificate.infers_master());
    }

    #[test]
    fn nontrivial_partial_map_discharges_and_merges_only_after_symbolica_substitution() {
        let context = context("mapped-nonzero-partial", 2);
        // n0 -> 2, n1 -> the sole child free coordinate.
        let geometry = OwnedGeometry::partial(&context, &[2, 0], &[1], &[0, 1]);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let theta = context
            .lift(&context.base().parameter("theta").unwrap())
            .unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n0_plus_theta = context.add(&n0, &theta).unwrap();
        let theta_n1 = context.mul(&theta, &n1).unwrap();
        let minus_two_n0_plus_theta = context.mul(&context.integer(-2), &n0_plus_theta).unwrap();
        let theta_squared_n1 = context
            .mul(&context.mul(&theta, &theta).unwrap(), &n1)
            .unwrap();
        let polynomials = [
            polynomial(&context, &n0),
            polynomial(&context, &n0_plus_theta),
            polynomial(&context, &theta_n1),
            polynomial(&context, &minus_two_n0_plus_theta),
            polynomial(&context, &theta_squared_n1),
        ];
        let inputs = [
            target_input(&polynomials[0], 0),
            target_input(&polynomials[1], 3),
            target_input(&polynomials[2], 8),
            exceptional_input(&polynomials[3], 4),
            exceptional_input(&polynomials[4], 7),
        ];

        let outcome = compile_identity(
            &context,
            &geometry,
            &plan,
            &inputs,
            GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseMappedNonZeroOutcome::Ready(certificate) = outcome else {
            panic!("expected mapped conditions")
        };
        let conditions = certificate.conditions();
        assert_eq!(conditions.inputs().len(), 5);
        assert_eq!(conditions.rows().len(), 2);
        assert_eq!(conditions.stats().discharged_nonzero_constants(), 1);
        assert_eq!(conditions.stats().unique_base_rows(), 1);
        assert_eq!(conditions.stats().unique_index_dependent_rows(), 1);
        assert!(matches!(
            conditions.inputs()[0].class(),
            GeneratedResidualAffineConditionInputClass::DischargedNonzeroIntegerConstant
        ));
        assert!(matches!(
            conditions.inputs()[1].class(),
            GeneratedResidualAffineConditionInputClass::BaseAssumption { row_ordinal: 0 }
        ));
        assert!(matches!(
            conditions.inputs()[2].class(),
            GeneratedResidualAffineConditionInputClass::IndexDependent { row_ordinal: 1 }
        ));
        assert_eq!(conditions.rows()[0].source_input_ordinals(), &[1, 3]);
        assert_eq!(conditions.rows()[1].source_input_ordinals(), &[2, 4]);
        assert!(
            conditions
                .stats()
                .associate_combined_temporary_byte_envelope()
                > 0
        );
        assert!(
            certificate.stats().mapped_output_observed_retained_bytes()
                <= certificate.stats().mapped_output_retained_byte_bound()
        );
        assert!(
            certificate.stats().accumulator_temporary_byte_envelope()
                <= certificate
                    .stats()
                    .admitted_accumulator_temporary_byte_envelope()
        );

        let mut one_below = GeneratedAffineResidualCaseMappedNonZeroLimits::default();
        one_below
            .accumulator
            .max_associate_combined_temporary_byte_envelope = conditions
            .stats()
            .associate_combined_temporary_byte_envelope()
            - 1;
        reset_mapped_nonzero_execution_probe_for_test();
        assert!(matches!(
            compile_identity(&context, &geometry, &plan, &inputs, one_below),
            Err(GeneratedAffineResidualCaseMappedNonZeroError::Accumulator(
                GeneratedResidualAffineConditionAccumulatorError::ParametricCoefficient(
                    crate::ParametricCoefficientError::ResourceLimit { .. }
                )
            )) | Err(GeneratedAffineResidualCaseMappedNonZeroError::Accumulator(
                GeneratedResidualAffineConditionAccumulatorError::ResourceLimit { .. }
            ))
        ));
        assert_eq!(mapped_nonzero_executions_for_test(), inputs.len());
    }

    #[test]
    fn source_binding_and_plan_replay_have_deterministic_precedence() {
        let context = context("mapped-nonzero-source-binding", 1);
        let geometry = OwnedGeometry::identity(&context);
        let mismatched = OwnedGeometry::fixed(&context, &[0]);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let n0 = polynomial(&context, &context.index(0).unwrap());
        let zero = polynomial(&context, &context.zero());
        let valid = [target_input(&n0, 0)];
        assert!(matches!(
            compile_identity(
                &context,
                &mismatched,
                &plan,
                &valid,
                GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
            ),
            Err(GeneratedAffineResidualCaseMappedNonZeroError::Composition(
                ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch
            ))
        ));

        let invalid_locator = [GeneratedAffineResidualCaseMappedNonZeroInput::new(
            &n0,
            GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                guard_ordinal: 0,
            },
        )];
        assert_eq!(
            compile_identity(
                &context,
                &mismatched,
                &plan,
                &invalid_locator,
                GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
            )
            .unwrap_err(),
            GeneratedAffineResidualCaseMappedNonZeroError::InvalidSource { input_ordinal: 0 }
        );
        let preexisting_zero = [target_input(&zero, 0)];
        assert_eq!(
            compile_identity(
                &context,
                &mismatched,
                &plan,
                &preexisting_zero,
                GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
            )
            .unwrap_err(),
            GeneratedAffineResidualCaseMappedNonZeroError::SourceBinding { input_ordinal: 0 }
        );
        let out_of_order = [exceptional_input(&n0, 1), target_input(&n0, 2)];
        assert_eq!(
            compile_identity(
                &context,
                &mismatched,
                &plan,
                &out_of_order,
                GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
            )
            .unwrap_err(),
            GeneratedAffineResidualCaseMappedNonZeroError::SourceBinding { input_ordinal: 1 }
        );
    }

    #[test]
    fn invalid_source_is_rejected_before_plan_replay_or_symbolica_execution() {
        let context = context("mapped-nonzero-invalid-source", 1);
        let geometry = OwnedGeometry::identity(&context);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let value = polynomial(&context, &context.one());
        let inputs = [GeneratedAffineResidualCaseMappedNonZeroInput::new(
            &value,
            GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                guard_ordinal: 0,
            },
        )];
        reset_mapped_nonzero_execution_probe_for_test();
        let error = compile_identity(
            &context,
            &geometry,
            &plan,
            &inputs,
            GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            GeneratedAffineResidualCaseMappedNonZeroError::InvalidSource { input_ordinal: 0 }
        );
        assert_eq!(mapped_nonzero_executions_for_test(), 0);
    }

    #[test]
    fn complete_stream_resource_failure_precedes_every_symbolica_execution() {
        let context = context("mapped-nonzero-preflight", 1);
        let geometry = OwnedGeometry::identity(&context);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let first = polynomial(&context, &context.index(0).unwrap());
        let second = polynomial(
            &context,
            &context
                .add(&context.index(0).unwrap(), &context.one())
                .unwrap(),
        );
        let inputs = [target_input(&first, 0), target_input(&second, 1)];
        let success = compile_identity(
            &context,
            &geometry,
            &plan,
            &inputs,
            GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseMappedNonZeroOutcome::Ready(success) = success else {
            panic!("expected ready baseline")
        };

        let mut limits = GeneratedAffineResidualCaseMappedNonZeroLimits::default();
        limits.max_mapped_output_retained_byte_bound = success
            .stats()
            .mapped_output_retained_byte_bound()
            .saturating_sub(1);
        reset_mapped_nonzero_execution_probe_for_test();
        assert!(matches!(
            compile_identity(&context, &geometry, &plan, &inputs, limits),
            Err(
                GeneratedAffineResidualCaseMappedNonZeroError::ResourceLimit {
                    resource: "mapped output retained byte bound",
                    ..
                }
            )
        ));
        assert_eq!(mapped_nonzero_executions_for_test(), 0);

        let mut limits = GeneratedAffineResidualCaseMappedNonZeroLimits::default();
        limits.max_total_source_terms = success.stats().preflight().source_terms() - 1;
        reset_mapped_nonzero_execution_probe_for_test();
        assert!(matches!(
            compile_identity(&context, &geometry, &plan, &inputs, limits),
            Err(GeneratedAffineResidualCaseMappedNonZeroError::Composition(
                ResidualUnitAffineCompositionError::ResourceLimit { .. }
            )) | Err(GeneratedAffineResidualCaseMappedNonZeroError::ResourceLimit { .. })
        ));
        assert_eq!(mapped_nonzero_executions_for_test(), 0);

        let mut limits = GeneratedAffineResidualCaseMappedNonZeroLimits::default();
        limits.max_compilation_owned_peak_upper_bound = success
            .stats()
            .admitted_compilation_owned_peak_upper_bound()
            .saturating_sub(1);
        reset_mapped_nonzero_execution_probe_for_test();
        assert!(matches!(
            compile_identity(&context, &geometry, &plan, &inputs, limits),
            Err(
                GeneratedAffineResidualCaseMappedNonZeroError::ResourceLimit {
                    resource: "compilation owned peak upper bound",
                    ..
                }
            )
        ));
        assert_eq!(mapped_nonzero_executions_for_test(), 0);
    }

    #[test]
    fn vector_and_replay_capacity_envelopes_have_exact_and_one_below_gates() {
        let context = context("mapped-nonzero-capacities", 1);
        let geometry = OwnedGeometry::identity(&context);
        let mismatched = OwnedGeometry::fixed(&context, &[0]);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let n0 = context.index(0).unwrap();
        let values = [
            polynomial(&context, &n0),
            polynomial(&context, &context.add(&n0, &context.one()).unwrap()),
            polynomial(&context, &context.add(&n0, &context.integer(2)).unwrap()),
        ];
        let inputs = [
            target_input(&values[0], 0),
            target_input(&values[1], 2),
            exceptional_input(&values[2], 4),
        ];
        let baseline = compile_identity(
            &context,
            &geometry,
            &plan,
            &inputs,
            GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseMappedNonZeroOutcome::Ready(baseline) = baseline else {
            panic!("expected capacity baseline")
        };
        let stats = baseline.stats();
        assert_eq!(
            stats.prepared_token_byte_envelope(),
            2 * inputs.len() * size_of::<PreparedMappedNonZero<'_>>()
        );
        assert_eq!(
            stats.mapped_output_slot_byte_envelope(),
            2 * inputs.len() * size_of::<MappedNonZero>()
        );
        assert!(stats.prepared_token_bytes() <= stats.prepared_token_byte_envelope());
        assert!(stats.mapped_output_slot_bytes() <= stats.mapped_output_slot_byte_envelope());
        assert_eq!(
            stats.plan_replay_scratch_byte_envelope(),
            plan_replay_scratch_byte_envelope(geometry.view()).unwrap()
        );

        let mut exact = GeneratedAffineResidualCaseMappedNonZeroLimits::default();
        exact.max_prepared_token_bytes = stats.prepared_token_byte_envelope();
        exact.max_mapped_output_slot_bytes = stats.mapped_output_slot_byte_envelope();
        assert!(compile_identity(&context, &geometry, &plan, &inputs, exact).is_ok());

        for (resource, one_below) in [
            {
                let mut limits = GeneratedAffineResidualCaseMappedNonZeroLimits::default();
                limits.max_prepared_token_bytes = stats.prepared_token_byte_envelope() - 1;
                ("prepared token bytes", limits)
            },
            {
                let mut limits = GeneratedAffineResidualCaseMappedNonZeroLimits::default();
                limits.max_mapped_output_slot_bytes = stats.mapped_output_slot_byte_envelope() - 1;
                ("mapped output slot bytes", limits)
            },
        ] {
            reset_mapped_nonzero_execution_probe_for_test();
            assert!(matches!(
                compile_identity(&context, &mismatched, &plan, &inputs, one_below),
                Err(GeneratedAffineResidualCaseMappedNonZeroError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
            assert_eq!(mapped_nonzero_executions_for_test(), 0);
        }
    }

    #[test]
    fn empty_stream_is_a_canonical_non_authoritative_success() {
        let context = context("mapped-nonzero-empty", 1);
        let geometry = OwnedGeometry::identity(&context);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let outcome = compile_identity(
            &context,
            &geometry,
            &plan,
            &[],
            GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseMappedNonZeroOutcome::Ready(certificate) = outcome else {
            panic!("empty inherited stream cannot prove an empty locus")
        };
        assert!(certificate.conditions().inputs().is_empty());
        assert!(certificate.conditions().rows().is_empty());
        assert_eq!(certificate.stats().executed_inputs(), 0);
        assert!(!certificate.is_branch_pruning_authority());
        assert!(!certificate.infers_master());
    }

    #[test]
    fn panic_after_first_mapped_output_is_contained_by_the_boundary() {
        let context = context("mapped-nonzero-panic", 1);
        let geometry = OwnedGeometry::identity(&context);
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                geometry.view(),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let first = polynomial(&context, &context.index(0).unwrap());
        let second = polynomial(
            &context,
            &context
                .add(&context.index(0).unwrap(), &context.one())
                .unwrap(),
        );
        let inputs = [target_input(&first, 0), target_input(&second, 1)];
        reset_mapped_nonzero_execution_probe_for_test();
        inject_mapped_nonzero_panic_after_executions_for_test(1);
        assert!(matches!(
            compile_identity(
                &context,
                &geometry,
                &plan,
                &inputs,
                GeneratedAffineResidualCaseMappedNonZeroLimits::default(),
            ),
            Err(
                GeneratedAffineResidualCaseMappedNonZeroError::CompilerPanic {
                    stage: "compilation boundary"
                }
            )
        ));
        assert_eq!(mapped_nonzero_executions_for_test(), 1);
    }
}
