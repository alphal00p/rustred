use crate::foundry::cell::{
    ResidualTermDisposition, RuleCell, RuleCellDomainProof, SourceViewBatch, SourceViewConstruction,
};
use crate::foundry::completion::stratum::StratumRegistryError;
use crate::foundry::parametric::{ParametricGuardOrigin, ParametricRule};

use super::algebra::{
    append_coefficient, append_identity_condition_source, append_index_shift, append_integral_key,
    append_interior_domain, append_monotone_descent, append_monotone_domain,
    append_parametric_relation, append_polynomial, append_shift_descent,
    append_translated_provenance,
};
use super::encoder::BoundedContentHasher;

pub(super) fn append_rule_cell(
    output: &mut BoundedContentHasher,
    cell: &RuleCell,
) -> Result<(), StratumRegistryError> {
    append_rule_cell_with_first_guard_override(output, cell, None)
}

pub(super) fn append_rule_cell_with_first_guard_override(
    output: &mut BoundedContentHasher,
    cell: &RuleCell,
    first_guard_override: Option<&crate::algebra::IndexedPolynomial>,
) -> Result<(), StratumRegistryError> {
    append_parametric_rule(output, cell.rule())?;
    append_source_batch(output, cell.sources())?;
    append_interior_domain(output, cell.proof_domain())?;
    append_monotone_domain(output, cell.application_domain())?;
    output.tag(match cell.domain_proof() {
        RuleCellDomainProof::TightenedOriginalInterior => 0,
        RuleCellDomainProof::ReprovedSectorMonotone => 1,
    })?;

    output.count(cell.fixed_restrictions().len())?;
    for fixed in cell.fixed_restrictions() {
        output.usize(fixed.position())?;
        output.i64(fixed.value())?;
    }
    output.count(cell.pruned_rhs_ordinals().len())?;
    for &ordinal in cell.pruned_rhs_ordinals() {
        output.usize(ordinal)?;
    }
    output.count(cell.terms().len())?;
    for term in cell.terms() {
        output.usize(term.source_rhs_ordinal())?;
        append_monotone_descent(output, term.descent())?;
    }
    output.count(cell.guards().len())?;
    for (ordinal, guard) in cell.guards().iter().enumerate() {
        output.usize(guard.source_guard_ordinal())?;
        append_polynomial(
            output,
            if ordinal == 0 {
                first_guard_override.unwrap_or_else(|| guard.polynomial())
            } else {
                guard.polynomial()
            },
        )?;
    }
    Ok(())
}

fn append_parametric_rule(
    output: &mut BoundedContentHasher,
    rule: &ParametricRule,
) -> Result<(), StratumRegistryError> {
    output.text(rule.family_fingerprint())?;
    output.text(rule.context_fingerprint())?;
    append_interior_domain(output, rule.domain())?;
    output.text(rule.ordering().stable_id())?;
    append_index_shift(output, rule.pivot())?;

    output.count(rule.right_hand_side().len())?;
    for term in rule.right_hand_side() {
        append_index_shift(output, term.shift())?;
        append_coefficient(output, term.coefficient())?;
        append_shift_descent(output, term.descent())?;
    }

    output.count(rule.elimination_pivot_guards().len())?;
    for guard in rule.elimination_pivot_guards() {
        output.usize(guard.source_ordinal())?;
        output.text(&guard.row_id().stable_string())?;
        output.usize(guard.pivot_column())?;
        append_index_shift(output, guard.pivot_shift())?;
        append_coefficient(output, guard.coefficient())?;
        append_polynomial(output, guard.nonzero_polynomial())?;
    }

    output.count(rule.nonzero_guards().len())?;
    for guard in rule.nonzero_guards() {
        append_polynomial(output, guard.polynomial())?;
        output.count(guard.origins().len())?;
        for origin in guard.origins() {
            append_parametric_guard_origin(output, origin)?;
        }
    }

    output.count(rule.source_combination().len())?;
    for source in rule.source_combination() {
        output.usize(source.source_ordinal())?;
        output.text(&source.row_id().stable_string())?;
        append_coefficient(output, source.coefficient())?;
    }

    let replay = rule.replay();
    output.usize(replay.source_rows_used())?;
    output.usize(replay.shift_columns_checked())?;
    output.usize(replay.exact_operations())?;
    let concrete = rule.concrete_replay();
    append_integral_key(output, concrete.anchor())?;
    output.usize(concrete.source_contributions_checked())?;
    output.usize(concrete.source_terms_checked())?;
    output.usize(concrete.right_hand_side_terms_checked())?;
    output.usize(concrete.integral_keys_checked())?;
    output.usize(concrete.nonzero_guards_checked())?;
    output.usize(concrete.exact_operations())?;
    output.usize(concrete.peak_retained_coefficient_terms())?;

    match rule.sector_monotone_admission() {
        Some(admission) => {
            output.tag(1)?;
            append_monotone_domain(output, admission.domain())?;
            append_index_shift(output, admission.pivot())?;
            output.count(admission.dependencies().len())?;
            for dependency in admission.dependencies() {
                output.usize(dependency.right_hand_side_ordinal())?;
                append_index_shift(output, dependency.pivot_shift())?;
                append_index_shift(output, dependency.shift())?;
                append_monotone_descent(output, dependency.descent())?;
            }
        }
        None => output.tag(0)?,
    }
    Ok(())
}

fn append_parametric_guard_origin(
    output: &mut BoundedContentHasher,
    origin: &ParametricGuardOrigin,
) -> Result<(), StratumRegistryError> {
    match origin {
        ParametricGuardOrigin::SourceCondition {
            source_ordinal,
            row_id,
            condition_ordinal,
            condition_sources,
        } => {
            output.tag(0)?;
            output.usize(*source_ordinal)?;
            output.text(&row_id.stable_string())?;
            output.usize(*condition_ordinal)?;
            output.count(condition_sources.len())?;
            for source in condition_sources {
                append_identity_condition_source(output, source)?;
            }
            Ok(())
        }
        ParametricGuardOrigin::SourceCoefficientDenominator {
            source_ordinal,
            row_id,
            shift,
        } => {
            output.tag(1)?;
            output.usize(*source_ordinal)?;
            output.text(&row_id.stable_string())?;
            append_index_shift(output, shift)
        }
        ParametricGuardOrigin::ReducerPivotNumerator {
            source_ordinal,
            row_id,
            pivot_column,
            pivot_shift,
        } => {
            output.tag(2)?;
            output.usize(*source_ordinal)?;
            output.text(&row_id.stable_string())?;
            output.usize(*pivot_column)?;
            append_index_shift(output, pivot_shift)
        }
        ParametricGuardOrigin::ReducerPivotDenominator {
            source_ordinal,
            row_id,
            pivot_column,
            pivot_shift,
        } => {
            output.tag(3)?;
            output.usize(*source_ordinal)?;
            output.text(&row_id.stable_string())?;
            output.usize(*pivot_column)?;
            append_index_shift(output, pivot_shift)
        }
        ParametricGuardOrigin::RuleCoefficientDenominator { shift } => {
            output.tag(4)?;
            append_index_shift(output, shift)
        }
        ParametricGuardOrigin::SourceCombinationDenominator {
            source_ordinal,
            row_id,
        } => {
            output.tag(5)?;
            output.usize(*source_ordinal)?;
            output.text(&row_id.stable_string())
        }
    }
}

fn append_source_batch(
    output: &mut BoundedContentHasher,
    sources: &SourceViewBatch,
) -> Result<(), StratumRegistryError> {
    output.text(sources.family_fingerprint())?;
    output.text(sources.context_fingerprint())?;
    output.count(sources.relations().len())?;
    for relation in sources.relations() {
        append_parametric_relation(output, relation)?;
    }
    output.count(sources.provenance().len())?;
    for provenance in sources.provenance() {
        append_translated_provenance(output, provenance.translated())?;
        match provenance.symmetry() {
            Some(symmetry) => {
                output.tag(1)?;
                output.usize(symmetry.group_element())?;
            }
            None => output.tag(0)?,
        }
    }

    match sources.construction() {
        SourceViewConstruction::Direct => output.tag(0)?,
        SourceViewConstruction::ResidualProjection(evidence) => {
            output.tag(1)?;
            append_interior_domain(output, evidence.domain())?;
            output.count(evidence.fixed_restrictions().len())?;
            for fixed in evidence.fixed_restrictions() {
                output.usize(fixed.position())?;
                output.i64(fixed.value())?;
            }
            output.count(evidence.original_relations().len())?;
            for relation in evidence.original_relations() {
                append_parametric_relation(output, relation)?;
            }
            output.count(evidence.term_projections().len())?;
            for relation_terms in evidence.term_projections() {
                output.count(relation_terms.len())?;
                for term in relation_terms.iter() {
                    super::algebra::append_i64_slice(output, term.source_shift())?;
                    match term.disposition() {
                        ResidualTermDisposition::CoefficientZero => output.tag(0)?,
                        ResidualTermDisposition::ProvedZero { zero_sector } => {
                            output.tag(1)?;
                            super::algebra::append_mask(output, zero_sector)?;
                        }
                        ResidualTermDisposition::Routed {
                            group_element,
                            projected_shift,
                        } => {
                            output.tag(2)?;
                            output.usize(*group_element)?;
                            super::algebra::append_i64_slice(output, projected_shift)?;
                        }
                    }
                }
            }
            output.count(evidence.stabilizer_group_elements().len())?;
            for &group in evidence.stabilizer_group_elements() {
                output.usize(group)?;
            }
        }
    }
    Ok(())
}
