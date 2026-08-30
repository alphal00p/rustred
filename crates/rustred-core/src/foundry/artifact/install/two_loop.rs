//! Registered closure verifier for the unit-mass two-loop sunset.

use std::collections::BTreeSet;

use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, SourceViewConstruction};
use crate::sector::{InteriorBounds, Mask};

use super::super::error::ArtifactError;
use super::super::model::{
    ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof, ZeroTerminalProof,
};
use super::ClosingArtifactCandidate;

pub(super) fn validate(
    candidate: ClosingArtifactCandidate,
) -> Result<ClosedArtifact, ArtifactError> {
    if candidate.arity != 3
        || candidate.algorithm_id != super::super::two_loop::ALGORITHM_ID
        || candidate.source_relations.len() != 4
        || !candidate.rules.is_empty()
        || candidate.rule_cells.len() != 5
        || candidate.dependencies.len() != 1
        || candidate.factorization_rules.len() != 1
        || candidate.masters.len() != 2
        || candidate.zero_sectors.len() != 4
        || candidate.common_mass_homogeneity
            != Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
    {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    if candidate.supported_root_power_bounds.as_ref()
        != [
            InteriorBounds::new(i64::MIN, i64::MAX - 1),
            InteriorBounds::new(i64::MIN, i64::MAX - 1),
            InteriorBounds::new(i64::MIN, i64::MAX - 1),
        ]
    {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    validate_canonical_two_loop_family(&candidate.family)?;
    validate_two_loop_source_manifest(&candidate)?;
    validate_two_loop_terminal_manifest(&candidate)?;
    validate_two_loop_symmetry(&candidate)?;
    validate_two_loop_cells(&candidate)?;
    validate_two_loop_factorization(&candidate)?;

    let replayed_source_rows = candidate.rule_cells.iter().try_fold(0usize, |sum, cell| {
        sum.checked_add(cell.rule().replay().source_rows_used())
            .ok_or(ArtifactError::InvalidReplayEvidence {
                detail: "two-loop replayed source-row census overflowed",
            })
    })?;
    let replayed_shift_columns = candidate.rule_cells.iter().try_fold(0usize, |sum, cell| {
        sum.checked_add(cell.rule().replay().shift_columns_checked())
            .ok_or(ArtifactError::InvalidReplayEvidence {
                detail: "two-loop replayed shift-column census overflowed",
            })
    })?;
    let guard_count = candidate.rule_cells.iter().try_fold(0usize, |sum, cell| {
        sum.checked_add(cell.guards().len())
            .ok_or(ArtifactError::InvalidReplayEvidence {
                detail: "two-loop guard census overflowed",
            })
    })?;
    let validation = ArtifactValidationWitness::new(
        candidate.source_relations.len(),
        replayed_source_rows,
        replayed_shift_columns,
        candidate.rule_cells.len(),
        guard_count,
        candidate.masters.len(),
        candidate.zero_sectors.len(),
    );
    Ok(ClosedArtifact {
        schema: candidate.schema,
        algorithm_id: candidate.algorithm_id,
        arity: candidate.arity,
        supported_root_power_bounds: candidate.supported_root_power_bounds,
        family_fingerprint: candidate.family.fingerprint_owner(),
        family: candidate.family,
        context: candidate.context,
        source_relations: candidate.source_relations,
        rules: candidate.rules,
        rule_cells: candidate.rule_cells,
        canonicalizer: candidate.canonicalizer,
        dependencies: candidate.dependencies,
        factorization_rules: candidate.factorization_rules,
        masters: candidate.masters,
        zero_sectors: candidate.zero_sectors,
        common_mass_homogeneity: candidate.common_mass_homogeneity,
        validation,
    })
}

fn validate_canonical_two_loop_family(family: &IntegralFamily) -> Result<(), ArtifactError> {
    let context = family.coefficient_context();
    let zero = context.zero();
    let one = context.one();
    let two = context.integer(2);
    let minus_one = context.integer(-1);
    let dimension = context
        .parameter("d")
        .ok_or(ArtifactError::UnsupportedClosureShape)?;
    let expected = [
        (
            minus_one.clone(),
            vec![one.clone(), zero.clone(), zero.clone()],
        ),
        (
            minus_one.clone(),
            vec![zero.clone(), zero.clone(), one.clone()],
        ),
        (minus_one, vec![one, two, context.one()]),
    ];
    let valid =
        family.loop_count() == 2
            && family.external_count() == 0
            && family.denominator_count() == 3
            && context.parameter_names() == ["d"]
            && family.dimension() == &dimension
            && family.external_gram().is_empty()
            && family.power_shifts() == [zero.clone(), zero.clone(), zero]
            && family.denominators().iter().zip(expected).all(
                |(actual, (constant, coefficients))| {
                    actual.constant() == &constant && actual.coefficients() == coefficients
                },
            );
    if valid {
        Ok(())
    } else {
        Err(ArtifactError::UnsupportedClosureShape)
    }
}

fn validate_two_loop_source_manifest(
    candidate: &ClosingArtifactCandidate,
) -> Result<(), ArtifactError> {
    let expected = [
        "ordinary-ibp:0:0",
        "ordinary-ibp:0:1",
        "ordinary-ibp:1:0",
        "ordinary-ibp:1:1",
    ];
    if candidate
        .source_relations
        .iter()
        .zip(expected)
        .any(|(source, expected)| source.row_id().stable_string() != expected)
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the two-loop artifact does not retain the complete four-row ordinary source manifest",
        });
    }
    Ok(())
}

fn validate_two_loop_terminal_manifest(
    candidate: &ClosingArtifactCandidate,
) -> Result<(), ArtifactError> {
    let expected_masters = BTreeSet::from([
        IntegralKey::try_new([0, 1, 1])?,
        IntegralKey::try_new([1, 1, 1])?,
    ]);
    if candidate.masters != expected_masters {
        return Err(ArtifactError::InvalidMasterManifest);
    }
    let expected_zero = [
        Mask::try_from_indices(&[0, 0, 0])?,
        Mask::try_from_indices(&[0, 0, 1])?,
        Mask::try_from_indices(&[0, 1, 0])?,
        Mask::try_from_indices(&[1, 0, 0])?,
    ];
    if candidate.zero_sectors.iter().any(|terminal| {
        let expected_proof = if terminal.sector().active_bits().iter().any(|&active| active) {
            ZeroTerminalProof::LeePomeranskyRankDeficiency
        } else {
            ZeroTerminalProof::ScalelessVacuumPolynomial
        };
        terminal.proof() != expected_proof || !expected_zero.contains(terminal.sector())
    }) || expected_zero.iter().any(|expected| {
        candidate
            .zero_sectors
            .iter()
            .filter(|terminal| terminal.sector() == expected)
            .count()
            != 1
    }) {
        return Err(ArtifactError::InvalidZeroTerminal);
    }
    Ok(())
}

fn validate_two_loop_symmetry(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    let owner = candidate
        .canonicalizer
        .as_ref()
        .ok_or(ArtifactError::InvalidCanonicalizer)?;
    let expected = [
        &[0, 1, 2][..],
        &[0, 2, 1][..],
        &[1, 0, 2][..],
        &[1, 2, 0][..],
        &[2, 0, 1][..],
        &[2, 1, 0][..],
    ];
    if owner.generator_count() != 2
        || owner.group_order() != expected.len()
        || owner.group_elements().ne(expected)
    {
        return Err(ArtifactError::InvalidCanonicalizer);
    }
    Ok(())
}

fn validate_two_loop_cells(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    let cells = &candidate.rule_cells;
    validate_cell_shape(
        &cells[0],
        4,
        false,
        &[1, 1, 1],
        &[
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
        ],
        &[0, 0, 1],
        &[
            &[0, 1, -1],
            &[0, 0, 0],
            &[0, -1, 1],
            &[-1, 1, 0],
            &[-1, 0, 1],
        ],
        &[],
        &[],
    )?;
    validate_cell_shape(
        &cells[1],
        1,
        false,
        &[0, 1, 1],
        &[
            InteriorBounds::new(i64::MIN + 1, -1),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(2, i64::MAX),
        ],
        &[-1, 0, 0],
        &[&[0, 0, 0], &[0, 0, -1], &[0, -1, 0], &[1, 0, -1]],
        &[],
        &[],
    )?;
    validate_cell_shape(
        &cells[2],
        1,
        false,
        &[0, 1, 1],
        &[
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(2, i64::MAX),
        ],
        &[-1, 0, 0],
        &[&[0, 0, 0], &[0, 0, -1], &[0, -1, 0], &[1, 0, -1]],
        &[FixedIndexRestriction::new(0, 0)],
        &[3],
    )?;
    validate_cell_shape(
        &cells[3],
        4,
        true,
        &[0, 1, 1],
        &[
            InteriorBounds::new(i64::MIN + 1, -1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
        &[-1, 0, 0],
        &[&[0, 0, 0], &[1, 0, 0]],
        &[
            FixedIndexRestriction::new(1, 1),
            FixedIndexRestriction::new(2, 1),
        ],
        &[],
    )?;
    validate_cell_shape(
        &cells[4],
        4,
        true,
        &[0, 1, 1],
        &[
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
        &[-1, 0, 0],
        &[&[0, 0, 0], &[1, 0, 0]],
        &[
            FixedIndexRestriction::new(0, 0),
            FixedIndexRestriction::new(1, 1),
            FixedIndexRestriction::new(2, 1),
        ],
        &[1],
    )?;
    validate_two_loop_guards(candidate)?;
    Ok(())
}

fn validate_two_loop_guards(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    let context = &candidate.context;
    let n0 = context.index(0)?;
    let n1 = context.index(1)?;
    let n2 = context.index(2)?;
    let one = context.one();
    let d = context.lift(
        &candidate
            .family
            .coefficient_context()
            .parameter("d")
            .ok_or(ArtifactError::UnsupportedClosureShape)?,
    )?;
    let indexed =
        |coefficient| context.numerator_condition_with_limits(coefficient, Default::default());
    let scaled = |scale, value| context.mul(&context.integer(scale), value);
    let n2_minus_one = context.sub(&n2, &one)?;
    let one_minus_n2 = context.sub(&one, &n2)?;
    let d_minus_n0_minus_one = context.sub(&context.sub(&d, &n0)?, &one)?;
    let d_minus_one = context.sub(&d, &one)?;
    let expected = vec![
        vec![
            indexed(&scaled(-2, &n0)?)?,
            indexed(&n1)?,
            indexed(&scaled(-3, &n2)?)?,
            indexed(&scaled(2, &n0)?)?,
            indexed(&scaled(3, &n2)?)?,
        ],
        vec![indexed(&one_minus_n2)?, indexed(&n2_minus_one)?],
        vec![indexed(&one_minus_n2)?, indexed(&n2_minus_one)?],
        vec![indexed(&d_minus_n0_minus_one)?],
        vec![indexed(&d_minus_one)?],
    ];
    for (cell_ordinal, (cell, expected)) in candidate.rule_cells.iter().zip(expected).enumerate() {
        if cell.guards().len() != expected.len() {
            return Err(ArtifactError::UnprovedGuardApplicability {
                guard_ordinal: cell_ordinal.saturating_mul(1_000_000),
            });
        }
        for (guard_ordinal, (guard, expected)) in cell.guards().iter().zip(expected).enumerate() {
            context.validate_polynomial_context(guard.polynomial())?;
            if guard.polynomial() != &expected {
                return Err(ArtifactError::UnprovedGuardApplicability {
                    guard_ordinal: cell_ordinal
                        .saturating_mul(1_000_000)
                        .saturating_add(guard_ordinal),
                });
            }
        }
    }
    // The registered domains make n0,n1,n2 strictly positive in the top
    // cell and n2>=2 in both pair cells. The remaining corner guards are
    // monic in the indeterminate d (or d-1), hence cannot be the zero
    // polynomial in Q(d) at any integer numerator power.
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_cell_shape(
    cell: &RuleCell,
    source_count: usize,
    projected: bool,
    sector: &[i64],
    bounds: &[InteriorBounds],
    pivot: &[i64],
    rhs: &[&[i64]],
    fixed: &[FixedIndexRestriction],
    pruned: &[usize],
) -> Result<(), ArtifactError> {
    let construction_matches = matches!(
        (projected, cell.sources().construction()),
        (false, SourceViewConstruction::Direct)
            | (true, SourceViewConstruction::ResidualProjection(_))
    );
    if cell.sources().len() != source_count
        || !construction_matches
        || cell.application_domain().sector() != &Mask::try_from_indices(sector)?
        || cell.application_domain().bounds() != bounds
        || cell.rule().pivot().values() != pivot
        || cell.rule().right_hand_side().len() != rhs.len()
        || cell
            .rule()
            .right_hand_side()
            .iter()
            .zip(rhs)
            .any(|(term, expected)| term.shift().values() != *expected)
        || cell.fixed_restrictions() != fixed
        || cell.pruned_rhs_ordinals() != pruned
        || cell.terms().len() != rhs.len() - pruned.len()
    {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "a generated two-loop rule cell differs from its registered exact closure shape",
        });
    }
    Ok(())
}

fn validate_two_loop_factorization(
    candidate: &ClosingArtifactCandidate,
) -> Result<(), ArtifactError> {
    let dependency = &candidate.dependencies[0];
    let rule = &candidate.factorization_rules[0];
    let valid = dependency.algorithm_id() == super::super::one_loop::ALGORITHM_ID
        && dependency.arity() == 1
        && rule.normalization() == &candidate.family.coefficient_context().one()
        && rule.factors().len() == 2
        && rule.factors()[0].dependency_ordinal() == 0
        && rule.factors()[0].parent_positions() == [1]
        && rule.factors()[1].dependency_ordinal() == 0
        && rule.factors()[1].parent_positions() == [2]
        && rule.master_embeddings().len() == 1
        && rule.master_embeddings()[0].raw_parent_master().powers() == [0, 1, 1]
        && rule.master_embeddings()[0].parent_terminal().powers() == [0, 1, 1];
    if valid {
        Ok(())
    } else {
        Err(ArtifactError::InvalidFactorization {
            detail: "the two-loop pinch is not the registered product of two one-loop tadpoles",
        })
    }
}
