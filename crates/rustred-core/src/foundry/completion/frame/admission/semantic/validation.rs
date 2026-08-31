//! Cold joins, aggregate accounting, and exact algebra validation.

use symbolica::prelude::{Integer, IntegerRing, MultivariatePolynomial};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::frame::{
    SourceInstanceId,
    exact::{ExactCircuitGuardOrigin, ExactTargetCircuit},
};
use crate::foundry::completion::guard::CoefficientIdealGuardAtom;
use crate::foundry::completion::stratum::TargetColumnPartition;
use crate::identity::IdentityConditionSource;

use super::error::ExactCircuitSemanticError;
use super::limits::ExactCircuitSemanticLimits;

pub(super) const CANDIDATES: &str = "semantic exact-circuit candidates";
const RESIDUAL_TERMS: &str = "semantic exact-circuit residual terms";
const SOURCE_CONTRIBUTIONS: &str = "semantic exact-circuit source contributions";
const PIVOT_GUARDS: &str = "semantic exact-circuit pivot guards";
pub(super) const NONZERO_GUARDS: &str = "semantic exact-circuit nonzero guards";
const GUARD_ORIGINS: &str = "semantic exact-circuit guard origins";
const CONDITION_SOURCES: &str = "semantic exact-circuit condition sources";
const CONDITION_SOURCE_COORDINATE_CELLS: &str =
    "semantic exact-circuit condition-source coordinate cells";
const DEPENDENCY_OWNERS: &str = "semantic exact-circuit dependency owners";
const GUARD_COEFFICIENT_EQUATIONS: &str =
    "semantic exact-circuit compiled guard coefficient equations";
const GUARD_BASE_MONOMIAL_EXPONENTS: &str =
    "semantic exact-circuit compiled guard base-monomial exponents";
const GUARD_GENERATORS: &str = "semantic exact-circuit compiled guard generators";
const GUARD_IDENTITY_BYTES: &str = "semantic exact-circuit compiled guard identity bytes";
const MODULAR_SAMPLE_POINT_ENTRIES: &str = "semantic exact-circuit modular sample-point entries";
const MODULAR_DIAGNOSTIC_ENTRIES: &str = "semantic exact-circuit modular diagnostic entries";
const EXACT_POLYNOMIALS: &str = "semantic exact-circuit exact polynomials";
const POLYNOMIAL_TERMS: &str = "semantic exact-circuit polynomial terms";
const EXPONENT_ENTRIES: &str = "semantic exact-circuit polynomial exponent entries";
const INTEGER_BITS: &str = "semantic exact-circuit integer coefficient bits";
#[derive(Default)]
pub(super) struct ContentTotals {
    residual_terms: usize,
    source_contributions: usize,
    pivot_guards: usize,
    nonzero_guards: usize,
    guard_origins: usize,
    condition_sources: usize,
    condition_source_coordinate_cells: usize,
    dependency_owners: usize,
    guard_coefficient_equations: usize,
    guard_base_monomial_exponents: usize,
    guard_generators: usize,
    guard_identity_bytes: usize,
    modular_sample_point_entries: usize,
    modular_diagnostic_entries: usize,
    exact_polynomials: usize,
    polynomial_terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
}

pub(super) fn validate_partition(
    context: &IndexedCoefficientContext,
    partition: &TargetColumnPartition<'_>,
) -> Result<(), ExactCircuitSemanticError> {
    if context.fingerprint() != partition.frame().context_fingerprint() {
        return Err(ExactCircuitSemanticError::WrongContext);
    }
    match partition.try_verify() {
        Ok(true) => Ok(()),
        Ok(false) => Err(ExactCircuitSemanticError::PartitionInvariant(
            "incoming target partition failed cold verification",
        )),
        Err(error) => Err(ExactCircuitSemanticError::PartitionVerification(error)),
    }
}

pub(super) fn validate_candidate(
    context: &IndexedCoefficientContext,
    partition: &TargetColumnPartition<'_>,
    circuit: &ExactTargetCircuit,
    candidate: usize,
    limits: ExactCircuitSemanticLimits,
    totals: &mut ContentTotals,
) -> Result<(), ExactCircuitSemanticError> {
    let fail = |detail| ExactCircuitSemanticError::CandidateJoin { candidate, detail };
    if circuit.stratum_id() != partition.stratum_id() {
        return Err(fail("decorated stratum identity differs"));
    }
    if circuit.owner_snapshot_id() != partition.snapshot_id() {
        return Err(fail("immutable owner snapshot identity differs"));
    }
    if circuit.target_column() != partition.target_column()
        || circuit.modular_diagnostics().target_column != partition.target_column()
        || circuit.modular_diagnostics().forbidden_columns.as_ref() != partition.forbidden_columns()
    {
        return Err(fail("target column or forbidden-column diagnostics differ"));
    }
    if partition.frame().columns().get(partition.target_column()) != Some(circuit.target_shift()) {
        return Err(fail("target shift differs from the physical frame"));
    }
    if circuit.target_shift().len() != context.index_count() {
        return Err(fail("target shift has the wrong indexed arity"));
    }

    charge(
        MODULAR_SAMPLE_POINT_ENTRIES,
        &mut totals.modular_sample_point_entries,
        circuit.sample_fingerprint().point().len(),
        limits.max_modular_sample_point_entries,
    )?;
    let diagnostics = circuit.modular_diagnostics();
    let diagnostic_entries = diagnostics
        .forbidden_columns
        .len()
        .checked_add(diagnostics.forbidden_pivot_columns.len())
        .and_then(|total| total.checked_add(diagnostics.augmented_pivot_columns.len()))
        .and_then(|total| total.checked_add(diagnostics.forbidden_independent_source_rows.len()))
        .and_then(|total| total.checked_add(diagnostics.augmented_independent_source_rows.len()))
        .ok_or(ExactCircuitSemanticError::ResourceCountOverflow {
            resource: MODULAR_DIAGNOSTIC_ENTRIES,
        })?;
    charge(
        MODULAR_DIAGNOSTIC_ENTRIES,
        &mut totals.modular_diagnostic_entries,
        diagnostic_entries,
        limits.max_modular_diagnostic_entries,
    )?;

    charge(
        RESIDUAL_TERMS,
        &mut totals.residual_terms,
        circuit.residual_terms().len(),
        limits.max_residual_terms,
    )?;
    charge(
        SOURCE_CONTRIBUTIONS,
        &mut totals.source_contributions,
        circuit.source_combination().len(),
        limits.max_source_contributions,
    )?;
    charge(
        PIVOT_GUARDS,
        &mut totals.pivot_guards,
        circuit.pivot_guards().len(),
        limits.max_pivot_guards,
    )?;
    charge(
        NONZERO_GUARDS,
        &mut totals.nonzero_guards,
        circuit.nonzero_guards().len(),
        limits.max_nonzero_guards,
    )?;

    let mut previous_column = None;
    for term in circuit.residual_terms() {
        if previous_column.is_some_and(|previous| previous >= term.physical_column()) {
            return Err(fail(
                "residual physical columns are not strictly increasing",
            ));
        }
        previous_column = Some(term.physical_column());
        let descriptor = partition
            .allowed_descriptor(term.physical_column())
            .ok_or_else(|| fail("a residual is absent from the allowed partition block"))?;
        if partition.frame().columns().get(term.physical_column()) != Some(term.shift())
            || descriptor.descent() != term.descent()
            || descriptor.proper_subsector_owners() != term.proper_subsector_owners()
            || term.coefficient().is_zero()
        {
            return Err(fail(
                "a residual payload differs from its allowed-column descriptor",
            ));
        }
        charge(
            DEPENDENCY_OWNERS,
            &mut totals.dependency_owners,
            term.proper_subsector_owners().len(),
            limits.max_dependency_owners,
        )?;
        validate_coefficient(context, term.coefficient(), candidate, limits, totals)?;
    }

    let mut previous_row = None;
    let mut source_terms = 0usize;
    for contribution in circuit.source_combination() {
        if previous_row.is_some_and(|previous| previous >= contribution.frame_row_ordinal()) {
            return Err(fail("source-combination rows are not strictly increasing"));
        }
        previous_row = Some(contribution.frame_row_ordinal());
        validate_frame_source(
            partition,
            contribution.frame_row_ordinal(),
            contribution.source_instance(),
            candidate,
        )?;
        source_terms = checked_add(
            SOURCE_CONTRIBUTIONS,
            source_terms,
            partition
                .frame()
                .column_indices_for_row(contribution.frame_row_ordinal())
                .ok_or_else(|| fail("source-combination row has invalid CSR bounds"))?
                .len(),
        )?;
        if contribution.coefficient().is_zero() {
            return Err(fail("source combination retains a zero multiplier"));
        }
        validate_coefficient(
            context,
            contribution.coefficient(),
            candidate,
            limits,
            totals,
        )?;
    }

    for pivot in circuit.pivot_guards() {
        validate_frame_source(
            partition,
            pivot.frame_row_ordinal(),
            pivot.source_instance(),
            candidate,
        )?;
        // A forward-elimination pivot can be fill introduced: its provenance
        // row need not contain the pivot column before earlier rows are
        // eliminated.  It must, however, be one of the projected physical
        // columns (the target or a forbidden column).
        if !is_projected_physical_column(partition, pivot.physical_pivot_column())
            || pivot.coefficient().is_zero()
            || pivot.nonzero_polynomial().is_zero()
        {
            return Err(fail(
                "pivot evidence is outside the projected target/forbidden columns or has a zero payload",
            ));
        }
        validate_coefficient(context, pivot.coefficient(), candidate, limits, totals)?;
        validate_polynomial(
            context,
            pivot.nonzero_polynomial(),
            candidate,
            limits,
            totals,
        )?;
    }

    for guard in circuit.nonzero_guards() {
        if guard.polynomial().is_zero() || guard.polynomial().is_nonzero_constant() {
            return Err(fail("nonzero guard is zero or a discarded literal unit"));
        }
        validate_polynomial(context, guard.polynomial(), candidate, limits, totals)?;
        charge(
            GUARD_ORIGINS,
            &mut totals.guard_origins,
            guard.origins().len(),
            limits.max_guard_origins,
        )?;
        if guard.origins().is_empty() {
            return Err(fail("nonzero guard has no exact origin"));
        }
        for origin in guard.origins() {
            validate_origin(partition, origin, candidate, limits, totals)?;
        }
    }

    let replay = circuit.replay();
    if replay.source_contributions() != circuit.source_combination().len()
        || replay.source_terms() != source_terms
        || replay.physical_columns() != partition.frame().columns().len()
    {
        return Err(fail(
            "exact replay witness counts disagree with the joined frame",
        ));
    }
    let minimum_operations = replay
        .source_terms()
        .checked_add(replay.physical_columns())
        .ok_or(ExactCircuitSemanticError::ResourceCountOverflow {
            resource: "semantic exact-circuit replay operations",
        })?;
    let maximum_operations = replay
        .source_terms()
        .checked_mul(2)
        .and_then(|value| value.checked_add(replay.physical_columns()))
        .ok_or(ExactCircuitSemanticError::ResourceCountOverflow {
            resource: "semantic exact-circuit replay operations",
        })?;
    if !(minimum_operations..=maximum_operations).contains(&replay.exact_operations()) {
        return Err(fail("exact replay operation count is inconsistent"));
    }
    Ok(())
}

fn validate_frame_source(
    partition: &TargetColumnPartition<'_>,
    row: usize,
    source: &SourceInstanceId,
    candidate: usize,
) -> Result<(), ExactCircuitSemanticError> {
    if partition.frame().source_instances().get(row) == Some(source)
        && partition.frame().source_for_row(row).is_some()
    {
        Ok(())
    } else {
        Err(ExactCircuitSemanticError::CandidateJoin {
            candidate,
            detail: "source provenance differs from physical frame chronology",
        })
    }
}

fn validate_origin(
    partition: &TargetColumnPartition<'_>,
    origin: &ExactCircuitGuardOrigin,
    candidate: usize,
    limits: ExactCircuitSemanticLimits,
    totals: &mut ContentTotals,
) -> Result<(), ExactCircuitSemanticError> {
    let fail = |detail| ExactCircuitSemanticError::CandidateJoin { candidate, detail };
    match origin {
        ExactCircuitGuardOrigin::SourceCondition {
            frame_row_ordinal,
            source_instance,
            condition_ordinal,
            condition_sources,
        } => {
            validate_frame_source(partition, *frame_row_ordinal, source_instance, candidate)?;
            let source = partition
                .frame()
                .source_for_row(*frame_row_ordinal)
                .ok_or_else(|| fail("guard condition row is unavailable"))?;
            let condition = source
                .nonzero_conditions()
                .get(*condition_ordinal)
                .ok_or_else(|| fail("guard condition ordinal is out of range"))?;
            if !condition.sources().iter().eq(condition_sources.iter()) {
                return Err(fail(
                    "guard condition provenance differs from source chronology",
                ));
            }
            charge(
                CONDITION_SOURCES,
                &mut totals.condition_sources,
                condition_sources.len(),
                limits.max_condition_sources,
            )?;
            for source in condition_sources {
                charge(
                    CONDITION_SOURCE_COORDINATE_CELLS,
                    &mut totals.condition_source_coordinate_cells,
                    identity_source_coordinate_cells(source),
                    limits.max_condition_source_coordinate_cells,
                )?;
            }
            Ok(())
        }
        ExactCircuitGuardOrigin::SourceCoefficientDenominator {
            frame_row_ordinal,
            source_instance,
            physical_column,
        } => {
            validate_frame_source(partition, *frame_row_ordinal, source_instance, candidate)?;
            let structural = partition
                .frame()
                .column_indices_for_row(*frame_row_ordinal)
                .ok_or_else(|| fail("guard source row has invalid CSR bounds"))?;
            if structural
                .iter()
                .any(|&column| column as usize == *physical_column)
            {
                Ok(())
            } else {
                Err(fail(
                    "guard source coefficient column is absent from its row",
                ))
            }
        }
        ExactCircuitGuardOrigin::ReducerPivotNumerator {
            frame_row_ordinal,
            source_instance,
            physical_pivot_column,
        }
        | ExactCircuitGuardOrigin::ReducerPivotDenominator {
            frame_row_ordinal,
            source_instance,
            physical_pivot_column,
        } => {
            validate_frame_source(partition, *frame_row_ordinal, source_instance, candidate)?;
            if is_projected_physical_column(partition, *physical_pivot_column) {
                Ok(())
            } else {
                Err(fail("guard reducer pivot is outside the projected columns"))
            }
        }
        ExactCircuitGuardOrigin::SourceMultiplierDenominator {
            frame_row_ordinal,
            source_instance,
        } => validate_frame_source(partition, *frame_row_ordinal, source_instance, candidate),
        ExactCircuitGuardOrigin::ResidualCoefficientDenominator { physical_column } => {
            if partition.allowed_descriptor(*physical_column).is_some() {
                Ok(())
            } else {
                Err(fail(
                    "guard residual origin is not an allowed physical column",
                ))
            }
        }
    }
}

fn identity_source_coordinate_cells(source: &IdentityConditionSource) -> usize {
    match source {
        IdentityConditionSource::RelationInputTermDenominator { shift, .. }
        | IdentityConditionSource::RelationCollectedTermDenominator { shift, .. } => shift.len(),
        IdentityConditionSource::RelationTranslation { offset, .. }
        | IdentityConditionSource::IndexTranslation { offset } => offset.len(),
        IdentityConditionSource::FamilyInputCoefficientDenominator { .. }
        | IdentityConditionSource::FamilyBasisDeterminantNumerator
        | IdentityConditionSource::RelationConditionAttached { .. }
        | IdentityConditionSource::RelationScaleFactorDenominator { .. } => 0,
    }
}

fn is_projected_physical_column(
    partition: &TargetColumnPartition<'_>,
    physical_column: usize,
) -> bool {
    physical_column == partition.target_column()
        || partition
            .forbidden_columns()
            .binary_search(&physical_column)
            .is_ok()
}

fn validate_coefficient(
    context: &IndexedCoefficientContext,
    coefficient: &IndexedCoefficient,
    candidate: usize,
    limits: ExactCircuitSemanticLimits,
    totals: &mut ContentTotals,
) -> Result<(), ExactCircuitSemanticError> {
    context
        .validate_with_limits(coefficient, limits.exact_algebra)
        .map_err(|error| ExactCircuitSemanticError::IndexedAlgebra { candidate, error })?;
    charge_polynomial(&coefficient.raw().numerator, limits, totals)?;
    charge_polynomial(&coefficient.raw().denominator, limits, totals)
}

pub(super) fn charge_compiled_guard_atom(
    atom: &CoefficientIdealGuardAtom,
    limits: ExactCircuitSemanticLimits,
    totals: &mut ContentTotals,
) -> Result<(), ExactCircuitSemanticError> {
    charge_polynomial(
        atom.predicate().representative_guard().raw(),
        limits,
        totals,
    )?;
    charge(
        GUARD_IDENTITY_BYTES,
        &mut totals.guard_identity_bytes,
        atom.predicate().representative_identity().predicate().len(),
        limits.max_guard_identity_bytes,
    )?;

    let generators = atom.id().generators();
    charge(
        GUARD_GENERATORS,
        &mut totals.guard_generators,
        generators.len(),
        limits.max_guard_generators,
    )?;
    for generator in generators {
        charge(
            GUARD_IDENTITY_BYTES,
            &mut totals.guard_identity_bytes,
            generator.predicate().len(),
            limits.max_guard_identity_bytes,
        )?;
    }

    let equations = atom.coefficient_system().equations();
    charge(
        GUARD_COEFFICIENT_EQUATIONS,
        &mut totals.guard_coefficient_equations,
        equations.len(),
        limits.max_guard_coefficient_equations,
    )?;
    for equation in equations {
        charge(
            GUARD_BASE_MONOMIAL_EXPONENTS,
            &mut totals.guard_base_monomial_exponents,
            equation.base_monomial().len(),
            limits.max_guard_base_monomial_exponents,
        )?;
        charge_polynomial(equation.index_polynomial().raw(), limits, totals)?;
    }
    Ok(())
}

fn validate_polynomial(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    candidate: usize,
    limits: ExactCircuitSemanticLimits,
    totals: &mut ContentTotals,
) -> Result<(), ExactCircuitSemanticError> {
    context
        .validate_polynomial_with_limits(polynomial, limits.exact_algebra)
        .map_err(|error| ExactCircuitSemanticError::IndexedAlgebra { candidate, error })?;
    charge_polynomial(polynomial.raw(), limits, totals)
}

fn charge_polynomial(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    limits: ExactCircuitSemanticLimits,
    totals: &mut ContentTotals,
) -> Result<(), ExactCircuitSemanticError> {
    charge(
        EXACT_POLYNOMIALS,
        &mut totals.exact_polynomials,
        1,
        limits.max_exact_polynomials,
    )?;
    charge(
        POLYNOMIAL_TERMS,
        &mut totals.polynomial_terms,
        polynomial.coefficients.len(),
        limits.max_polynomial_terms,
    )?;
    charge(
        EXPONENT_ENTRIES,
        &mut totals.exponent_entries,
        polynomial.exponents.len(),
        limits.max_exponent_entries,
    )?;
    for coefficient in &polynomial.coefficients {
        let bits = usize::try_from(integer_magnitude_bits(coefficient)).map_err(|_| {
            ExactCircuitSemanticError::ResourceCountOverflow {
                resource: INTEGER_BITS,
            }
        })?;
        charge(
            INTEGER_BITS,
            &mut totals.integer_bits,
            bits,
            limits.max_integer_coefficient_bits,
        )?;
    }
    Ok(())
}

fn integer_magnitude_bits(value: &Integer) -> u64 {
    match value {
        Integer::Single(value) => u64::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u64::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u64::from(value.significant_bits()),
    }
}

fn charge(
    resource: &'static str,
    total: &mut usize,
    increment: usize,
    limit: usize,
) -> Result<(), ExactCircuitSemanticError> {
    *total = checked_add(resource, *total, increment)?;
    check_limit(resource, *total, limit)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactCircuitSemanticError> {
    left.checked_add(right)
        .ok_or(ExactCircuitSemanticError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactCircuitSemanticError> {
    if requested > limit {
        Err(ExactCircuitSemanticError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ExactCircuitSemanticError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        ExactCircuitSemanticError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}
