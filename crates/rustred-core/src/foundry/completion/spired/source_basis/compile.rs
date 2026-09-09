use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::SelfRing;
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::identity::{CompletedIbpSourceRows, IndexShift};

use super::super::SpiredExecutionCase;
use super::{
    SpiredRawSourceReconstructionTerm, SpiredSourceBasis, SpiredSourceBasisError,
    SpiredSourceBasisLimits, SpiredSourceBasisProvenanceTerm, SpiredSourceBasisRow,
    SpiredSourceBasisTerm,
};

const SOURCE_ROWS: &str = "source-preconditioner ordinary rows";
const PHYSICAL_COLUMNS: &str = "source-preconditioner physical columns";
const INPUT_NONZEROS: &str = "source-preconditioner input nonzero entries";
const AUGMENTED_COLUMNS: &str = "source-preconditioner augmented columns";
const NATIVE_DENSE_BOUND: &str = "source-preconditioner native dense-entry bound";
const NATIVE_RETAINED_NONZEROS: &str = "source-preconditioner native retained nonzero entries";
const BASIS_ROWS: &str = "source-preconditioner basis rows";
const BASIS_TERMS: &str = "source-preconditioner basis physical entries";
const PROVENANCE_TERMS: &str = "source-preconditioner provenance entries";
const REVERSE_SPAN_TERMS: &str = "source-preconditioner reverse-span entries";
const POLYNOMIAL_OPERATIONS: &str = "source-preconditioner polynomial operations";

type NativeField = RationalPolynomialField<IntegerRing, u16>;

/// Build Gregor's sector/order-local sparse RREF source basis.
///
/// The returned rows have polynomial coefficients and non-unit pivots: each
/// normalized Symbolica RREF row is multiplied by the LCM of every physical
/// and provenance denominator, then divided by their common Symbolica
/// polynomial content. Forward provenance and a checked reverse transform
/// prove equality with the ordinary-source span over the generic
/// rational-function field.
///
/// Crucially, `case` supplies only its sector and ordering: this routine never
/// prunes a term against the target, coordinate bounds, or guards. In
/// particular, inactive-denominator activating terms remain present. The
/// basis nevertheless retains a mandatory raw fallback policy because its
/// fraction-field transform can lose rank after exceptional specialization.
pub(crate) fn try_precondition_spired_ordinary_sources(
    context: &IndexedCoefficientContext,
    sources: &CompletedIbpSourceRows,
    case: &SpiredExecutionCase,
    limits: SpiredSourceBasisLimits,
) -> Result<SpiredSourceBasis, SpiredSourceBasisError> {
    validate_scope(context, sources, case)?;
    let source_count = sources.source_row_count();
    check_limit(SOURCE_ROWS, source_count, limits.max_source_rows)?;
    if source_count == 0 {
        return Err(SpiredSourceBasisError::EmptySourceCorpus);
    }

    let (columns, column_lookup, input_nonzeros) =
        collect_ordered_columns(context, sources, case, limits)?;
    let column_count = columns.len();
    check_limit(
        INPUT_NONZEROS,
        input_nonzeros,
        limits.max_input_nonzero_entries,
    )?;

    let independent =
        select_independent_rows(context, sources, &column_lookup, column_count, limits)?;
    check_limit(BASIS_ROWS, independent.len(), limits.max_basis_rows)?;
    let augmented_columns = checked_add(AUGMENTED_COLUMNS, column_count, independent.len())?;
    check_limit(
        AUGMENTED_COLUMNS,
        augmented_columns,
        limits.max_augmented_columns,
    )?;
    let dense_bound = checked_mul(NATIVE_DENSE_BOUND, independent.len(), augmented_columns)?;
    check_limit(
        NATIVE_DENSE_BOUND,
        dense_bound,
        limits.max_native_dense_entry_bound,
    )?;

    let native_augmented_columns = checked_u32(AUGMENTED_COLUMNS, augmented_columns)?;
    let mut reducer = call_native("constructing the exact source-basis reducer", || {
        SparseRowReducer::new(native_augmented_columns, NativeField::new(Z), LuLMode::None)
    })?;
    for (basis_input_ordinal, &source_ordinal) in independent.iter().enumerate() {
        let source =
            sources
                .source_relation(source_ordinal)
                .ok_or(SpiredSourceBasisError::Invariant {
                    detail: "selected ordinary source row disappeared",
                })?;
        let (mut values, mut column_ids) = raw_row(context, source, &column_lookup, limits)?;
        values.push(context.one().raw().clone());
        column_ids.push(checked_u32(
            AUGMENTED_COLUMNS,
            checked_add(AUGMENTED_COLUMNS, column_count, basis_input_ordinal)?,
        )?);
        let pivot = call_native("adding an exact source row to Symbolica RREF", || {
            reducer.add_row(&values, &column_ids)
        })?
        .ok_or(SpiredSourceBasisError::Invariant {
            detail: "an exactly independent ordinary row became dependent after provenance augmentation",
        })? as usize;
        if pivot >= column_count {
            return Err(SpiredSourceBasisError::Invariant {
                detail: "an independent ordinary row pivoted on provenance",
            });
        }
        check_native_payload(&reducer, limits)?;
    }
    call_native("back-substituting the exact source basis", || {
        reducer.back_substitute()
    })?;
    check_native_payload(&reducer, limits)?;

    let rows = extract_cleared_rows(context, sources, &columns, &independent, &reducer, limits)?;
    let raw_reconstruction =
        build_and_verify_reverse_span(context, sources, &column_lookup, &rows, limits)?;
    let dropped_dependent_rows =
        source_count
            .checked_sub(independent.len())
            .ok_or(SpiredSourceBasisError::Invariant {
                detail: "source-basis rank exceeds the ordinary source count",
            })?;
    let basis = SpiredSourceBasis::new(
        sources.identity_owner(),
        case.stratum().domain().sector().clone(),
        case.ordering(),
        columns,
        independent,
        rows,
        raw_reconstruction,
        dropped_dependent_rows,
    );
    let zero_offset = vec![0_i64; context.index_count()];
    basis.try_verify_translated_provenance(context, sources, &zero_offset, limits)?;
    Ok(basis)
}

fn validate_scope(
    context: &IndexedCoefficientContext,
    sources: &CompletedIbpSourceRows,
    case: &SpiredExecutionCase,
) -> Result<(), SpiredSourceBasisError> {
    if !sources.is_complete_ordinary() {
        return Err(SpiredSourceBasisError::IncompleteOrdinarySourceLayout);
    }
    if sources.family_fingerprint() != case.stratum().family_fingerprint() {
        return Err(SpiredSourceBasisError::WrongFamily);
    }
    if sources.context_fingerprint() != context.fingerprint()
        || case.stratum().context_fingerprint() != context.fingerprint()
    {
        return Err(SpiredSourceBasisError::WrongContext);
    }
    Ok(())
}

fn collect_ordered_columns(
    context: &IndexedCoefficientContext,
    sources: &CompletedIbpSourceRows,
    case: &SpiredExecutionCase,
    limits: SpiredSourceBasisLimits,
) -> Result<(Vec<IndexShift>, BTreeMap<IndexShift, usize>, usize), SpiredSourceBasisError> {
    let mut unique = BTreeSet::new();
    let mut input_nonzeros = 0usize;
    for source_ordinal in 0..sources.source_row_count() {
        let source =
            sources
                .source_relation(source_ordinal)
                .ok_or(SpiredSourceBasisError::Invariant {
                    detail: "ordinary source chronology has a missing row",
                })?;
        source.validate_context(context)?;
        if source.terms().is_empty() {
            return Err(SpiredSourceBasisError::EmptySourceRow { source_ordinal });
        }
        input_nonzeros = checked_add(INPUT_NONZEROS, input_nonzeros, source.terms().len())?;
        for (shift, coefficient) in source.terms() {
            context.validate_with_limits(coefficient, limits.exact_algebra)?;
            unique.insert(shift.clone());
        }
        for condition in source.nonzero_conditions() {
            context
                .validate_polynomial_with_limits(condition.polynomial(), limits.exact_algebra)?;
        }
    }
    check_limit(PHYSICAL_COLUMNS, unique.len(), limits.max_physical_columns)?;
    if unique.is_empty() {
        return Err(SpiredSourceBasisError::Invariant {
            detail: "nonempty source rows produced no physical columns",
        });
    }
    let mut keyed = try_vec(PHYSICAL_COLUMNS, unique.len())?;
    for shift in unique {
        let key = case
            .ordering()
            .shift_complexity_key(case.stratum().domain().sector(), shift.values())?;
        keyed.push((shift, key));
    }
    keyed.sort_by(|left, right| right.1.cmp(&left.1));
    let mut columns = try_vec(PHYSICAL_COLUMNS, keyed.len())?;
    columns.extend(keyed.into_iter().map(|(shift, _)| shift));
    let mut lookup = BTreeMap::new();
    for (column, shift) in columns.iter().enumerate() {
        if lookup.insert(shift.clone(), column).is_some() {
            return Err(SpiredSourceBasisError::Invariant {
                detail: "ordered physical columns contain a duplicate shift",
            });
        }
    }
    Ok((columns, lookup, input_nonzeros))
}

fn select_independent_rows(
    context: &IndexedCoefficientContext,
    sources: &CompletedIbpSourceRows,
    column_lookup: &BTreeMap<IndexShift, usize>,
    column_count: usize,
    limits: SpiredSourceBasisLimits,
) -> Result<Vec<usize>, SpiredSourceBasisError> {
    let dense_bound = checked_mul(NATIVE_DENSE_BOUND, sources.source_row_count(), column_count)?;
    check_limit(
        NATIVE_DENSE_BOUND,
        dense_bound,
        limits.max_native_dense_entry_bound,
    )?;
    let native_column_count = checked_u32(PHYSICAL_COLUMNS, column_count)?;
    let mut reducer = call_native("constructing the exact source-rank reducer", || {
        SparseRowReducer::new(native_column_count, NativeField::new(Z), LuLMode::None)
    })?;
    let mut independent = try_vec(BASIS_ROWS, sources.source_row_count())?;
    for source_ordinal in 0..sources.source_row_count() {
        let source =
            sources
                .source_relation(source_ordinal)
                .ok_or(SpiredSourceBasisError::Invariant {
                    detail: "ordinary source chronology has a missing row during rank selection",
                })?;
        let (values, columns) = raw_row(context, source, column_lookup, limits)?;
        if call_native("selecting an exact independent source row", || {
            reducer.add_row(&values, &columns)
        })?
        .is_some()
        {
            independent.push(source_ordinal);
        }
        check_native_payload(&reducer, limits)?;
    }
    if independent.is_empty() {
        return Err(SpiredSourceBasisError::Invariant {
            detail: "nonzero ordinary rows have exact rank zero",
        });
    }
    Ok(independent)
}

fn raw_row(
    context: &IndexedCoefficientContext,
    source: &crate::identity::ParametricRelation,
    column_lookup: &BTreeMap<IndexShift, usize>,
    limits: SpiredSourceBasisLimits,
) -> Result<(Vec<crate::algebra::Coefficient>, Vec<u32>), SpiredSourceBasisError> {
    let mut entries = try_vec(INPUT_NONZEROS, source.terms().len())?;
    for (shift, coefficient) in source.terms() {
        context.validate_with_limits(coefficient, limits.exact_algebra)?;
        let column = *column_lookup
            .get(shift)
            .ok_or(SpiredSourceBasisError::Invariant {
                detail: "ordinary source shift is absent from the physical-column registry",
            })?;
        entries.push((column, coefficient.raw().clone()));
    }
    entries.sort_by_key(|entry| entry.0);
    let mut values = try_vec(INPUT_NONZEROS, entries.len())?;
    let mut columns = try_vec(INPUT_NONZEROS, entries.len())?;
    for (column, value) in entries {
        values.push(value);
        columns.push(checked_u32(PHYSICAL_COLUMNS, column)?);
    }
    Ok((values, columns))
}

fn extract_cleared_rows(
    context: &IndexedCoefficientContext,
    sources: &CompletedIbpSourceRows,
    physical_columns: &[IndexShift],
    independent: &[usize],
    reducer: &SparseRowReducer<NativeField>,
    limits: SpiredSourceBasisLimits,
) -> Result<Vec<SpiredSourceBasisRow>, SpiredSourceBasisError> {
    if reducer.u().nrows() as usize != independent.len() {
        return Err(SpiredSourceBasisError::Invariant {
            detail: "back-substituted source basis has the wrong row count",
        });
    }
    let mut rows = try_vec(BASIS_ROWS, independent.len())?;
    let mut physical_entries = 0usize;
    let mut provenance_entries = 0usize;
    for pivot_column in 0..physical_columns.len() {
        let Some(native_row) = reducer.pivots().get(pivot_column).copied().flatten() else {
            continue;
        };
        let native_row = native_row as usize;
        let start =
            *reducer
                .u()
                .row_ptrs()
                .get(native_row)
                .ok_or(SpiredSourceBasisError::Invariant {
                    detail: "source-basis U row has no start pointer",
                })?;
        let end = *reducer.u().row_ptrs().get(native_row + 1).ok_or(
            SpiredSourceBasisError::Invariant {
                detail: "source-basis U row has no end pointer",
            },
        )?;
        let native_values =
            reducer
                .u()
                .values()
                .get(start..end)
                .ok_or(SpiredSourceBasisError::Invariant {
                    detail: "source-basis U row has invalid value bounds",
                })?;
        let native_columns =
            reducer
                .u()
                .col_idcs()
                .get(start..end)
                .ok_or(SpiredSourceBasisError::Invariant {
                    detail: "source-basis U row has invalid column bounds",
                })?;
        let mut admitted = try_vec(BASIS_TERMS, native_values.len())?;
        for value in native_values {
            admitted.push(
                context.admit_native_result_with_limits(value.clone(), limits.exact_algebra)?,
            );
        }
        let cleared = clear_row_denominators(context, &admitted, limits)?;
        let mut terms = Vec::new();
        let mut provenance = Vec::new();
        terms.try_reserve(native_values.len()).map_err(|_| {
            SpiredSourceBasisError::AllocationFailure {
                resource: BASIS_TERMS,
                requested: native_values.len(),
            }
        })?;
        provenance.try_reserve(independent.len()).map_err(|_| {
            SpiredSourceBasisError::AllocationFailure {
                resource: PROVENANCE_TERMS,
                requested: independent.len(),
            }
        })?;
        for (&column, coefficient) in native_columns.iter().zip(cleared) {
            let column = column as usize;
            if column < physical_columns.len() {
                physical_entries = checked_add(BASIS_TERMS, physical_entries, 1)?;
                check_limit(BASIS_TERMS, physical_entries, limits.max_basis_term_entries)?;
                terms.push(SpiredSourceBasisTerm::new(
                    physical_columns[column].clone(),
                    coefficient,
                ));
            } else {
                let selected_position = column.checked_sub(physical_columns.len()).ok_or(
                    SpiredSourceBasisError::Invariant {
                        detail: "provenance column moved before the physical block",
                    },
                )?;
                let source_ordinal = *independent.get(selected_position).ok_or(
                    SpiredSourceBasisError::Invariant {
                        detail: "source-basis provenance column is outside selected chronology",
                    },
                )?;
                let source_row = sources.source_row_id(source_ordinal).ok_or(
                    SpiredSourceBasisError::Invariant {
                        detail: "source-basis provenance names a missing ordinary row",
                    },
                )?;
                provenance_entries = checked_add(PROVENANCE_TERMS, provenance_entries, 1)?;
                check_limit(
                    PROVENANCE_TERMS,
                    provenance_entries,
                    limits.max_provenance_entries,
                )?;
                provenance.push(SpiredSourceBasisProvenanceTerm::new(
                    source_ordinal,
                    source_row.clone(),
                    coefficient,
                ));
            }
        }
        if terms.first().map(|term| term.shift().values())
            != Some(physical_columns[pivot_column].values())
            || terms
                .first()
                .is_some_and(|term| term.coefficient().is_zero())
            || provenance.is_empty()
        {
            return Err(SpiredSourceBasisError::Invariant {
                detail: "cleared source-basis row lost its physical pivot or ordinary provenance",
            });
        }
        rows.push(SpiredSourceBasisRow::new(pivot_column, terms, provenance));
    }
    if rows.len() != independent.len() {
        return Err(SpiredSourceBasisError::Invariant {
            detail: "physical pivot map does not cover every independent source row",
        });
    }
    Ok(rows)
}

fn clear_row_denominators(
    context: &IndexedCoefficientContext,
    values: &[IndexedCoefficient],
    limits: SpiredSourceBasisLimits,
) -> Result<Vec<IndexedPolynomial>, SpiredSourceBasisError> {
    let mut work = PolynomialWork::new(context, limits);
    let mut denominator =
        context.numerator_condition_with_limits(&context.one(), limits.exact_algebra)?;
    for value in values {
        let next = context.denominator_condition_with_limits(value, limits.exact_algebra)?;
        denominator = work.lcm(&denominator, &next)?;
    }
    let scale = context.coefficient_from_polynomial_sealed(&denominator)?;
    let mut cleared = try_vec(BASIS_TERMS, values.len())?;
    for value in values {
        work.charge()?;
        let scaled = context.mul_with_limits(&scale, value, limits.exact_algebra)?;
        if !scaled.raw().denominator.is_one() {
            return Err(SpiredSourceBasisError::Invariant {
                detail: "LCM scaling left a rational source-basis coefficient",
            });
        }
        cleared.push(context.numerator_condition_with_limits(&scaled, limits.exact_algebra)?);
    }
    work.primitive_normalize(cleared)
}

fn build_and_verify_reverse_span(
    context: &IndexedCoefficientContext,
    sources: &CompletedIbpSourceRows,
    column_lookup: &BTreeMap<IndexShift, usize>,
    basis_rows: &[SpiredSourceBasisRow],
    limits: SpiredSourceBasisLimits,
) -> Result<Vec<Box<[SpiredRawSourceReconstructionTerm]>>, SpiredSourceBasisError> {
    let mut all = try_vec(REVERSE_SPAN_TERMS, sources.source_row_count())?;
    let mut retained = 0usize;
    for source_ordinal in 0..sources.source_row_count() {
        let source =
            sources
                .source_relation(source_ordinal)
                .ok_or(SpiredSourceBasisError::Invariant {
                    detail: "ordinary source disappeared during reverse-span construction",
                })?;
        let mut reconstruction = try_vec(REVERSE_SPAN_TERMS, basis_rows.len())?;
        for (basis_row_ordinal, basis_row) in basis_rows.iter().enumerate() {
            let pivot_shift = basis_row
                .terms()
                .first()
                .ok_or(SpiredSourceBasisError::Invariant {
                    detail: "source-basis row has no physical pivot",
                })?
                .shift();
            let raw = source
                .terms()
                .iter()
                .find(|(shift, _)| shift.values() == pivot_shift.values())
                .map(|(_, coefficient)| coefficient.clone())
                .unwrap_or_else(|| context.zero());
            if raw.is_zero() {
                continue;
            }
            let pivot =
                context.coefficient_from_polynomial_sealed(basis_row.pivot_coefficient())?;
            let coefficient = context.div_with_limits(&raw, &pivot, limits.exact_algebra)?;
            if !coefficient.is_zero() {
                retained = checked_add(REVERSE_SPAN_TERMS, retained, 1)?;
                check_limit(
                    REVERSE_SPAN_TERMS,
                    retained,
                    limits.max_reverse_span_entries,
                )?;
                reconstruction.push(SpiredRawSourceReconstructionTerm::new(
                    basis_row_ordinal,
                    coefficient,
                ));
            }
        }
        verify_raw_reconstruction(
            context,
            source_ordinal,
            source,
            column_lookup,
            basis_rows,
            &reconstruction,
            limits,
        )?;
        all.push(reconstruction.into_boxed_slice());
    }
    Ok(all)
}

fn verify_raw_reconstruction(
    context: &IndexedCoefficientContext,
    source_ordinal: usize,
    source: &crate::identity::ParametricRelation,
    column_lookup: &BTreeMap<IndexShift, usize>,
    basis_rows: &[SpiredSourceBasisRow],
    reconstruction: &[SpiredRawSourceReconstructionTerm],
    limits: SpiredSourceBasisLimits,
) -> Result<(), SpiredSourceBasisError> {
    let mut actual: BTreeMap<usize, IndexedCoefficient> = BTreeMap::new();
    let mut operations = 0usize;
    for contribution in reconstruction {
        let row = basis_rows.get(contribution.basis_row_ordinal()).ok_or(
            SpiredSourceBasisError::Invariant {
                detail: "reverse-span term names a missing basis row",
            },
        )?;
        for term in row.terms() {
            operations = charge_replay(operations, limits)?;
            let polynomial = context.coefficient_from_polynomial_sealed(term.coefficient())?;
            let product = context.mul_with_limits(
                contribution.coefficient(),
                &polynomial,
                limits.exact_algebra,
            )?;
            let column = basis_rows_column(term, column_lookup)?;
            accumulate(
                context,
                &mut actual,
                column,
                product,
                limits,
                &mut operations,
            )?;
        }
    }
    for column in 0..column_lookup.len() {
        let expected = source.terms().iter().find_map(|(shift, coefficient)| {
            (column_lookup.get(shift) == Some(&column)).then_some(coefficient)
        });
        let actual = actual.remove(&column);
        if match (expected, actual.as_ref()) {
            (None, None) => false,
            (Some(expected), Some(actual)) => expected != actual,
            (Some(expected), None) => !expected.is_zero(),
            (None, Some(actual)) => !actual.is_zero(),
        } {
            return Err(SpiredSourceBasisError::ReplayMismatch {
                row_ordinal: source_ordinal,
                detail: "ordinary row is not in the exact generic span of the optimized basis",
            });
        }
    }
    if !actual.is_empty() {
        return Err(SpiredSourceBasisError::Invariant {
            detail: "reverse-span replay retained an unknown physical column",
        });
    }
    Ok(())
}

fn basis_rows_column(
    term: &SpiredSourceBasisTerm,
    column_lookup: &BTreeMap<IndexShift, usize>,
) -> Result<usize, SpiredSourceBasisError> {
    column_lookup
        .iter()
        .find_map(|(shift, &column)| (shift.values() == term.shift().values()).then_some(column))
        .ok_or(SpiredSourceBasisError::Invariant {
            detail: "basis term shift is absent from its physical-column registry",
        })
}

fn accumulate(
    context: &IndexedCoefficientContext,
    accumulators: &mut BTreeMap<usize, IndexedCoefficient>,
    column: usize,
    value: IndexedCoefficient,
    limits: SpiredSourceBasisLimits,
    operations: &mut usize,
) -> Result<(), SpiredSourceBasisError> {
    if value.is_zero() {
        return Ok(());
    }
    if let Some(previous) = accumulators.remove(&column) {
        *operations = charge_replay(*operations, limits)?;
        let sum = context.add_with_limits(&previous, &value, limits.exact_algebra)?;
        if !sum.is_zero() {
            accumulators.insert(column, sum);
        }
    } else {
        accumulators.insert(column, value);
    }
    Ok(())
}

struct PolynomialWork<'context> {
    context: &'context IndexedCoefficientContext,
    limits: SpiredSourceBasisLimits,
    operations: usize,
}

impl<'context> PolynomialWork<'context> {
    const fn new(
        context: &'context IndexedCoefficientContext,
        limits: SpiredSourceBasisLimits,
    ) -> Self {
        Self {
            context,
            limits,
            operations: 0,
        }
    }

    fn charge(&mut self) -> Result<(), SpiredSourceBasisError> {
        self.operations = checked_add(POLYNOMIAL_OPERATIONS, self.operations, 1)?;
        check_limit(
            POLYNOMIAL_OPERATIONS,
            self.operations,
            self.limits.max_polynomial_operations,
        )
    }

    fn lcm(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, SpiredSourceBasisError> {
        self.context
            .validate_polynomial_with_limits(left, self.limits.exact_algebra)?;
        self.context
            .validate_polynomial_with_limits(right, self.limits.exact_algebra)?;
        if left.is_zero() || right.is_zero() {
            return Err(SpiredSourceBasisError::Invariant {
                detail: "a normalized rational coefficient has a zero denominator",
            });
        }
        if left.raw().is_one() {
            return Ok(right.clone());
        }
        if right.raw().is_one() {
            return Ok(left.clone());
        }
        self.charge()?;
        let gcd = call_native("computing a source-basis denominator GCD", || {
            left.raw().gcd(right.raw())
        })?;
        let gcd = self
            .context
            .admit_native_polynomial_result_with_limits(gcd, self.limits.exact_algebra)?;
        let left = self.context.coefficient_from_polynomial_sealed(left)?;
        let right = self.context.coefficient_from_polynomial_sealed(right)?;
        let gcd = self.context.coefficient_from_polynomial_sealed(&gcd)?;
        self.charge()?;
        let quotient = self
            .context
            .div_with_limits(&left, &gcd, self.limits.exact_algebra)?;
        self.charge()?;
        let product = self
            .context
            .mul_with_limits(&quotient, &right, self.limits.exact_algebra)?;
        if !product.raw().denominator.is_one() {
            return Err(SpiredSourceBasisError::Invariant {
                detail: "Symbolica polynomial LCM retained a denominator",
            });
        }
        Ok(self
            .context
            .numerator_condition_with_limits(&product, self.limits.exact_algebra)?)
    }

    /// Remove the common polynomial content from one denominator-cleared row.
    ///
    /// Symbolica owns both the multivariate GCD and the exact divisions. The
    /// first nonzero entry seeds the content so zero provenance entries do not
    /// collapse it spuriously.
    fn primitive_normalize(
        &mut self,
        values: Vec<IndexedPolynomial>,
    ) -> Result<Vec<IndexedPolynomial>, SpiredSourceBasisError> {
        let nonzero_count = values.iter().filter(|value| !value.is_zero()).count();
        let mut native_values = try_vec(BASIS_TERMS, nonzero_count)?;
        for value in &values {
            if value.is_zero() {
                continue;
            }
            self.context
                .validate_polynomial_with_limits(value, self.limits.exact_algebra)?;
            native_values.push(value.raw().clone());
        }
        if native_values.is_empty() {
            return Err(SpiredSourceBasisError::Invariant {
                detail: "denominator clearing produced an identically zero source-basis row",
            });
        }
        self.charge()?;
        let common = call_native("computing source-basis polynomial content", || {
            crate::algebra::CoefficientPolynomial::gcd_multiple(native_values)
        })?;
        let common = self
            .context
            .admit_native_polynomial_result_with_limits(common, self.limits.exact_algebra)?;
        if common.is_zero() {
            return Err(SpiredSourceBasisError::Invariant {
                detail: "denominator clearing produced an identically zero source-basis row",
            });
        }
        if common.is_nonzero_constant() && common.raw().is_one() {
            return Ok(values);
        }

        let mut primitive = try_vec(BASIS_TERMS, values.len())?;
        for value in values {
            if value.is_zero() {
                primitive.push(value);
                continue;
            }
            self.charge()?;
            let quotient = call_native("dividing source-basis polynomial content", || {
                value.raw().try_div(common.raw())
            })?
            .ok_or(SpiredSourceBasisError::Invariant {
                detail: "Symbolica source-basis content did not divide its row",
            })?;
            primitive.push(
                self.context.admit_native_polynomial_result_with_limits(
                    quotient,
                    self.limits.exact_algebra,
                )?,
            );
        }
        Ok(primitive)
    }
}

fn check_native_payload(
    reducer: &SparseRowReducer<NativeField>,
    limits: SpiredSourceBasisLimits,
) -> Result<(), SpiredSourceBasisError> {
    let retained = checked_add(
        NATIVE_RETAINED_NONZEROS,
        reducer.u().nvalues(),
        reducer.l().nvalues(),
    )?;
    check_limit(
        NATIVE_RETAINED_NONZEROS,
        retained,
        limits.max_native_retained_nonzero_entries,
    )
}

fn charge_replay(
    current: usize,
    limits: SpiredSourceBasisLimits,
) -> Result<usize, SpiredSourceBasisError> {
    let next = checked_add("source-preconditioner replay exact operations", current, 1)?;
    check_limit(
        "source-preconditioner replay exact operations",
        next,
        limits.max_replay_exact_operations,
    )?;
    Ok(next)
}

fn call_native<T>(
    operation: &'static str,
    work: impl FnOnce() -> T,
) -> Result<T, SpiredSourceBasisError> {
    catch_unwind(AssertUnwindSafe(work))
        .map_err(|_| SpiredSourceBasisError::NativePanic { operation })
}

fn checked_u32(resource: &'static str, value: usize) -> Result<u32, SpiredSourceBasisError> {
    u32::try_from(value).map_err(|_| SpiredSourceBasisError::ResourceCountOverflow { resource })
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredSourceBasisError> {
    left.checked_add(right)
        .ok_or(SpiredSourceBasisError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredSourceBasisError> {
    left.checked_mul(right)
        .ok_or(SpiredSourceBasisError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredSourceBasisError> {
    if requested > limit {
        Err(SpiredSourceBasisError::ResourceLimit {
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
) -> Result<Vec<T>, SpiredSourceBasisError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| SpiredSourceBasisError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
