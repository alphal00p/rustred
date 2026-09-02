//! Registered closure verifier for a fully published K6 sector-wave chain.

use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::FULL_RANK_ORBITS;
use crate::foundry::completion::frame::admission::ExactOwnerCoverStatus;
use crate::foundry::completion::source_discovery::ClosedSectorClosureWave;
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::sector::{InteriorBounds, Mask};

use super::super::error::ArtifactError;
use super::super::model::{ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof};
use super::ClosingArtifactCandidate;

mod source_authentication;

pub(super) use source_authentication::authenticate_canonical_source_views;
#[cfg(test)]
pub(crate) use source_authentication::authenticate_rule_cell_source_views;

const WAVE_WIDTHS: [usize; 4] = [2, 2, 1, 1];
const MAX_PUBLISHED_RULE_CELLS: usize = 1_000_000;

/// Validate proof-chain topology and transfer every retained executable cell
/// into the generic candidate before generic source/descent replay runs.
pub(super) fn prepare(
    candidate: &mut ClosingArtifactCandidate,
    waves: &[ClosedSectorClosureWave],
) -> Result<(), ArtifactError> {
    validate_candidate_shell(candidate)?;
    validate_wave_chain_root(candidate, waves)?;

    let mut lower = vec![None; candidate.arity];
    let mut upper = vec![None; candidate.arity];
    let mut cell_count = 0usize;
    let mut orbit_start = 0usize;
    let mut predecessor = waves
        .first()
        .ok_or(invalid("published K6 wave chain is empty"))?
        .predecessor();

    for (wave_ordinal, (wave, &expected_width)) in waves.iter().zip(WAVE_WIDTHS.iter()).enumerate()
    {
        if !wave.predecessor().same_authority_as(predecessor)
            || wave.layers().len() != expected_width
            || wave.successor().closed_layer_count()
                != wave
                    .predecessor()
                    .closed_layer_count()
                    .checked_add(expected_width)
                    .ok_or(invalid("published K6 predecessor layer count overflowed"))?
        {
            return Err(invalid(
                "published K6 wave has a foreign predecessor or wrong width",
            ));
        }
        let expected_end = orbit_start
            .checked_add(expected_width)
            .ok_or(invalid("published K6 orbit manifest overflowed"))?;
        let mut expected_sectors = FULL_RANK_ORBITS[orbit_start..expected_end]
            .iter()
            .map(|orbit| Mask::try_from_indices(&orbit.representative))
            .collect::<Result<Vec<_>, _>>()?;
        expected_sectors.sort();

        for (layer_ordinal, (layer, expected_sector)) in wave
            .layers()
            .iter()
            .zip(expected_sectors.iter())
            .enumerate()
        {
            if layer.sector() != expected_sector
                || layer.ordering() != candidate.ordering
                || !layer
                    .predecessor_snapshot()
                    .same_authority_as(wave.predecessor())
                || layer.family_fingerprint() != candidate.family.fingerprint()
                || layer.context_fingerprint() != candidate.context.fingerprint()
            {
                return Err(ArtifactError::InvalidOrderingAuthority {
                    detail: "a published K6 layer differs from its manifest or wave authority",
                    ordinal: wave_ordinal * 2 + layer_ordinal,
                });
            }
            let executable = layer.executable_cover().executable_cover();
            let proof = executable.proof_cover();
            if proof.status() != ExactOwnerCoverStatus::Closed
                || !proof.uncovered_partition().boxes().is_empty()
                || !proof.missing_terminals().is_empty()
                || !proof.guard_incomplete_owners().is_empty()
                || proof.owner_snapshot_id() != wave.predecessor().id()
            {
                return Err(invalid(
                    "a published K6 layer no longer proves an exact zero-uncovered cover",
                ));
            }
            validate_layer_domain(
                layer.sector(),
                layer.proven_domain().bounds(),
                &mut lower,
                &mut upper,
            )?;
            validate_terminal_ownership(candidate, expected_sector, executable.terminals())?;
            for owner in executable.owners() {
                cell_count = cell_count
                    .checked_add(owner.executable_candidates().len())
                    .ok_or(invalid("published K6 rule-cell count overflowed"))?;
                if cell_count > MAX_PUBLISHED_RULE_CELLS {
                    return Err(invalid(
                        "published K6 rule-cell count exceeds the installation limit",
                    ));
                }
            }
        }
        predecessor = wave.successor();
        orbit_start = expected_end;
    }
    if waves.len() != WAVE_WIDTHS.len()
        || orbit_start != FULL_RANK_ORBITS.len()
        || predecessor.closed_layer_count() != FULL_RANK_ORBITS.len()
    {
        return Err(invalid(
            "published K6 waves do not consume the complete six-orbit manifest",
        ));
    }

    validate_coordinate_transitivity(candidate)?;
    let common_lower = common_symmetric_endpoint(&lower)?;
    let common_upper = common_symmetric_endpoint(&upper)?;
    if common_lower > 0 || common_upper < 1 {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "published K6 root-power bounds do not span sector boundaries",
        });
    }
    let mut bounds = Vec::new();
    bounds
        .try_reserve_exact(candidate.arity)
        .map_err(|_| invalid("could not reserve published K6 root-power bounds"))?;
    bounds.resize(
        candidate.arity,
        InteriorBounds::new(common_lower, common_upper),
    );

    let mut cells = Vec::new();
    cells
        .try_reserve_exact(cell_count)
        .map_err(|_| invalid("could not reserve published K6 rule-cell owners"))?;
    for wave in waves {
        for layer in wave.layers() {
            for owner in layer.executable_cover().executable_cover().owners() {
                cells.extend(
                    owner
                        .executable_candidates()
                        .iter()
                        .map(|candidate| candidate.cell_owner().clone()),
                );
            }
        }
    }
    if cells.len() != cell_count || cells.is_empty() {
        return Err(invalid(
            "published K6 executable-cell ownership census changed during assembly",
        ));
    }
    candidate.supported_root_power_bounds = bounds.into_boxed_slice();
    candidate.rule_cells = cells;
    Ok(())
}

pub(super) fn seal(candidate: ClosingArtifactCandidate) -> Result<ClosedArtifact, ArtifactError> {
    validate_candidate_shell(&candidate)?;
    if candidate.supported_root_power_bounds.len() != 6 || candidate.rule_cells.is_empty() {
        return Err(invalid("prepared K6 artifact payload is incomplete"));
    }
    let expected_rows = [
        "ordinary-ibp:0:0",
        "ordinary-ibp:0:1",
        "ordinary-ibp:0:2",
        "ordinary-ibp:1:0",
        "ordinary-ibp:1:1",
        "ordinary-ibp:1:2",
        "ordinary-ibp:2:0",
        "ordinary-ibp:2:1",
        "ordinary-ibp:2:2",
    ];
    if candidate
        .source_relations
        .iter()
        .map(|row| row.row_id().stable_string())
        .ne(expected_rows.map(str::to_owned))
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the K6 artifact does not retain the canonical nine-row ordinary source manifest",
        });
    }

    let replayed_source_rows =
        checked_cell_sum(&candidate, |cell| cell.rule().replay().source_rows_used())?;
    let replayed_shift_columns = checked_cell_sum(&candidate, |cell| {
        cell.rule().replay().shift_columns_checked()
    })?;
    let guards = checked_cell_sum(&candidate, |cell| cell.guards().len())?;
    let validation = ArtifactValidationWitness::new(
        candidate.source_relations.len(),
        replayed_source_rows,
        replayed_shift_columns,
        candidate.rule_cells.len(),
        guards,
        candidate.masters.len(),
        candidate.zero_sectors.len(),
    );
    Ok(ClosedArtifact {
        schema: candidate.schema,
        algorithm_id: candidate.algorithm_id,
        arity: candidate.arity,
        ordering: candidate.ordering,
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

fn validate_candidate_shell(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    if candidate.algorithm_id != super::super::three_loop::ALGORITHM_ID
        || candidate.arity != 6
        || candidate.source_relations.len() != 9
        || !candidate.rules.is_empty()
        || candidate.dependencies.len() != 2
        || candidate.factorization_rules.len() != 3
        || candidate.masters.len() != 6
        || candidate.common_mass_homogeneity
            != Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
    {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    Ok(())
}

fn validate_wave_chain_root(
    candidate: &ClosingArtifactCandidate,
    waves: &[ClosedSectorClosureWave],
) -> Result<(), ArtifactError> {
    if waves.len() != WAVE_WIDTHS.len() {
        return Err(invalid(
            "published K6 wave count differs from the registered manifest",
        ));
    }
    let authority = Arc::new(
        super::super::three_loop::derive_k6_terminal_authority_with_ordering(candidate.ordering)?,
    );
    let expected =
        ImmutableOwnerSnapshot::try_from_terminal_authority(authority, Default::default())
            .map_err(|_| invalid("could not reconstruct the authenticated K6 root predecessor"))?;
    let actual = waves
        .first()
        .ok_or(invalid("published K6 wave chain is empty"))?
        .predecessor();
    if actual != &expected
        || actual.closed_layer_count() != 0
        || actual.family_fingerprint() != candidate.family.fingerprint()
        || actual.context_fingerprint() != candidate.context.fingerprint()
        || actual.canonicalizer_ordering() != Some(candidate.ordering)
    {
        return Err(invalid(
            "published K6 waves do not descend from the registered terminal root",
        ));
    }
    Ok(())
}

fn validate_layer_domain(
    sector: &Mask,
    bounds: &[InteriorBounds],
    lower: &mut [Option<i64>],
    upper: &mut [Option<i64>],
) -> Result<(), ArtifactError> {
    if bounds.len() != sector.arity() {
        return Err(invalid("published K6 layer domain has the wrong arity"));
    }
    for (coordinate, (&active, bound)) in sector.active_bits().iter().zip(bounds).enumerate() {
        if active {
            if bound.lower() != 1 {
                return Err(invalid(
                    "published K6 active-sector domain does not start at one",
                ));
            }
            merge_endpoint(&mut upper[coordinate], bound.upper())?;
        } else {
            if bound.upper() != 0 {
                return Err(invalid(
                    "published K6 inactive-sector domain does not end at zero",
                ));
            }
            merge_endpoint(&mut lower[coordinate], bound.lower())?;
        }
    }
    Ok(())
}

fn merge_endpoint(slot: &mut Option<i64>, endpoint: i64) -> Result<(), ArtifactError> {
    if slot.is_some_and(|retained| retained != endpoint) {
        return Err(invalid(
            "published K6 layers disagree on a shared root-power endpoint",
        ));
    }
    *slot = Some(endpoint);
    Ok(())
}

fn common_symmetric_endpoint(endpoints: &[Option<i64>]) -> Result<i64, ArtifactError> {
    let common = endpoints.iter().flatten().next().copied().ok_or(invalid(
        "published K6 layers provide no source-safe power endpoint",
    ))?;
    if endpoints
        .iter()
        .flatten()
        .any(|&endpoint| endpoint != common)
    {
        return Err(invalid(
            "coordinate-symmetric K6 layers disagree on a source-safe power endpoint",
        ));
    }
    Ok(common)
}

fn validate_coordinate_transitivity(
    candidate: &ClosingArtifactCandidate,
) -> Result<(), ArtifactError> {
    let canonicalizer = candidate
        .canonicalizer
        .as_ref()
        .ok_or(ArtifactError::InvalidCanonicalizer)?;
    let mut reached = vec![false; candidate.arity];
    for permutation in canonicalizer.group_elements() {
        let Some(&image) = permutation.first() else {
            return Err(ArtifactError::InvalidCanonicalizer);
        };
        let Some(slot) = reached.get_mut(image) else {
            return Err(ArtifactError::InvalidCanonicalizer);
        };
        *slot = true;
    }
    if reached.iter().all(|&is_reached| is_reached) {
        Ok(())
    } else {
        Err(ArtifactError::InvalidCanonicalizer)
    }
}

fn validate_terminal_ownership(
    candidate: &ClosingArtifactCandidate,
    sector: &Mask,
    terminals: &[IntegralKey],
) -> Result<(), ArtifactError> {
    let representative =
        IntegralKey::try_new(sector.active_bits().iter().map(|&active| i64::from(active)))?;
    let canonicalizer = candidate
        .canonicalizer
        .as_ref()
        .ok_or(ArtifactError::InvalidCanonicalizer)?;
    let expected = canonicalizer
        .canonicalize(&representative)?
        .canonical()
        .clone();
    if !candidate.masters.contains(&expected)
        || !terminals.iter().any(|terminal| {
            canonicalizer
                .canonicalize(terminal)
                .is_ok_and(|canonical| canonical.canonical() == &expected)
        })
        || terminals.iter().any(|terminal| {
            canonicalizer
                .canonicalize(terminal)
                .map_or(true, |canonical| {
                    !candidate.masters.contains(canonical.canonical())
                })
        })
    {
        return Err(ArtifactError::InvalidMasterManifest);
    }
    Ok(())
}

fn checked_cell_sum(
    candidate: &ClosingArtifactCandidate,
    value: impl Fn(&crate::foundry::cell::RuleCell) -> usize,
) -> Result<usize, ArtifactError> {
    candidate.rule_cells.iter().try_fold(0usize, |sum, cell| {
        sum.checked_add(value(cell))
            .ok_or(ArtifactError::InvalidReplayEvidence {
                detail: "K6 artifact validation census overflowed",
            })
    })
}

const fn invalid(detail: &'static str) -> ArtifactError {
    ArtifactError::InvalidClosurePublication { detail }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::algebra::IndexedCoefficientContext;
    use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
    use crate::foundry::artifact::model::{ArtifactSchemaVersion, CommonMassHomogeneityProof};
    use crate::foundry::cell::SourceViewConstruction;
    use crate::identity::{
        IdentityConditionSource, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator,
        ParametricNonZeroCondition, ParametricRelation, RelationBuilder, RelationLimits,
    };
    use crate::sector::{InteriorBounds, OrderingPolicy};

    use super::super::{ClosingArtifactCandidate, validate_generic_bindings};
    use super::{
        WAVE_WIDTHS, authenticate_rule_cell_source_views, common_symmetric_endpoint,
        merge_endpoint, seal,
    };

    #[test]
    fn registered_wave_widths_cover_every_k6_orbit_once() {
        assert_eq!(WAVE_WIDTHS.iter().sum::<usize>(), 6);
        assert_eq!(
            WAVE_WIDTHS,
            crate::foundry::campaign::K6_FULL_RANK_WAVE_WIDTHS
        );
    }

    #[test]
    fn shared_domain_endpoint_merge_is_exact_and_fail_closed() {
        let mut endpoint = None;
        merge_endpoint(&mut endpoint, 17).unwrap();
        merge_endpoint(&mut endpoint, 17).unwrap();
        assert_eq!(endpoint, Some(17));
        assert!(merge_endpoint(&mut endpoint, 18).is_err());
        assert_eq!(
            common_symmetric_endpoint(&[Some(-17), None, Some(-17)]).unwrap(),
            -17,
        );
        assert!(common_symmetric_endpoint(&[Some(-17), None, Some(-18)]).is_err());
    }

    #[test]
    fn derived_root_bounds_precede_generic_validation_without_false_publication() {
        let ordering = OrderingPolicy::default();
        let authority =
            crate::foundry::artifact::derive_k6_terminal_authority_with_ordering(ordering).unwrap();
        let parts = authority.into_artifact_parts();
        let generator = ParametricIbpGenerator::try_new_with_config(
            &parts.family,
            ParametricIbpConfig::default(),
        )
        .unwrap();
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len())
            .map(|ordinal| prepared.generate(ordinal))
            .collect();
        let source_relations = prepared.complete(rows).unwrap().into_relations();
        drop(generator);
        let mut candidate = ClosingArtifactCandidate {
            schema: ArtifactSchemaVersion::CURRENT,
            algorithm_id: crate::foundry::artifact::three_loop::ALGORITHM_ID,
            arity: parts.arity,
            ordering,
            supported_root_power_bounds: Vec::new().into_boxed_slice(),
            family: parts.family,
            context: parts.context,
            source_relations,
            rules: Vec::new(),
            rule_cells: Vec::new(),
            canonicalizer: parts.canonicalizer,
            dependencies: parts.dependencies,
            factorization_rules: parts.factorization_rules,
            masters: parts.masters,
            zero_sectors: parts.zero_sectors,
            common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
        };

        assert!(validate_generic_bindings(&candidate).is_err());
        candidate.supported_root_power_bounds =
            vec![InteriorBounds::new(i64::MIN, i64::MAX - 1); 6].into_boxed_slice();
        validate_generic_bindings(&candidate).unwrap();
        assert!(
            seal(candidate).is_err(),
            "a validated shell without published executable owners must never seal",
        );
    }

    #[test]
    fn canonical_source_join_rejects_forged_source_ordinal_row_offset_and_symmetry() {
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let source = artifact.rule_cells[cell_ordinal].sources().provenance()[0]
                .translated()
                .source_ordinal();
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_translated_source_ordinal_for_artifact_test(0, (source + 1) % 4);
        });
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let foreign_row = artifact.source_relations[1].row_id().clone();
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_translated_source_row_for_artifact_test(0, foreign_row);
        });
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let arity = artifact.arity;
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_translated_source_offset_for_artifact_test(
                    0,
                    IntegralShift::try_new(
                        std::iter::once(37).chain(std::iter::repeat_n(0, arity - 1)),
                    )
                    .unwrap(),
                );
        });
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .attach_unregistered_source_symmetry_for_artifact_test(0, 0);
        });
    }

    #[test]
    fn canonical_source_join_rejects_mutated_coefficients_and_conditions() {
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let forged = rebuild_relation(
                &artifact.rule_cells[cell_ordinal].sources().relations()[0],
                &artifact.context,
                RelationMutation::FirstCoefficient,
            );
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_source_relation_for_artifact_test(0, forged);
        });
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let forged = rebuild_relation(
                &artifact.rule_cells[cell_ordinal].sources().relations()[0],
                &artifact.context,
                RelationMutation::AdditionalCondition,
            );
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_source_relation_for_artifact_test(0, forged);
        });
    }

    #[test]
    fn canonical_source_join_authenticates_residual_original_before_projection_replay() {
        let mut artifact = derive_two_loop_unit_mass_sunset().unwrap();
        let cell_ordinal = artifact
            .rule_cells
            .iter()
            .position(|cell| {
                matches!(
                    cell.sources().construction(),
                    SourceViewConstruction::ResidualProjection(_)
                )
            })
            .expect("the registered sunset contains a residual source projection");
        let SourceViewConstruction::ResidualProjection(evidence) =
            artifact.rule_cells[cell_ordinal].sources().construction()
        else {
            unreachable!();
        };
        let forged = rebuild_relation(
            &evidence.original_relations()[0],
            &artifact.context,
            RelationMutation::FirstCoefficient,
        );
        Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
            .unwrap()
            .replace_residual_original_relation_for_artifact_test(0, forged);

        assert_sunset_source_authentication_fails(&artifact);
    }

    fn assert_sunset_source_mutant_rejected(
        mutate: impl FnOnce(&mut crate::foundry::artifact::ClosedArtifact, usize),
    ) {
        let mut artifact = derive_two_loop_unit_mass_sunset().unwrap();
        let cell_ordinal = artifact
            .rule_cells
            .iter()
            .position(|cell| {
                !matches!(
                    cell.sources().construction(),
                    SourceViewConstruction::ResidualProjection(_)
                )
            })
            .expect("the registered sunset contains a direct source view");
        mutate(&mut artifact, cell_ordinal);
        assert_sunset_source_authentication_fails(&artifact);
    }

    fn assert_sunset_source_authentication_fails(
        artifact: &crate::foundry::artifact::ClosedArtifact,
    ) {
        let generator = ParametricIbpGenerator::try_new_with_config(
            &artifact.family,
            ParametricIbpConfig::default(),
        )
        .unwrap();
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len())
            .map(|ordinal| prepared.generate(ordinal))
            .collect();
        let completed = prepared.complete(rows).unwrap();
        assert!(
            authenticate_rule_cell_source_views(&generator, &completed, &artifact.rule_cells,)
                .is_err()
        );
    }

    #[derive(Clone, Copy)]
    enum RelationMutation {
        FirstCoefficient,
        AdditionalCondition,
    }

    fn rebuild_relation(
        original: &ParametricRelation,
        context: &IndexedCoefficientContext,
        mutation: RelationMutation,
    ) -> ParametricRelation {
        let limits = RelationLimits::default();
        let mut builder = RelationBuilder::new(
            original.family_fingerprint_owner(),
            original.row_id().clone(),
            context,
        );
        for condition in original.nonzero_conditions() {
            builder
                .add_sealed_nonzero_condition(context, condition.clone(), limits)
                .unwrap();
        }
        for (ordinal, (shift, coefficient)) in original.terms().iter().enumerate() {
            let coefficient =
                if ordinal == 0 && matches!(mutation, RelationMutation::FirstCoefficient) {
                    context
                        .neg_with_limits(coefficient, limits.arithmetic.exact_algebra)
                        .unwrap()
                } else {
                    coefficient.clone()
                };
            builder
                .add_sealed_term(context, shift.clone(), coefficient, limits)
                .unwrap();
        }
        if matches!(mutation, RelationMutation::AdditionalCondition) {
            let index = context.index(0).unwrap();
            let polynomial = context
                .numerator_condition_with_limits(&index, limits.arithmetic.exact_algebra)
                .unwrap();
            let condition = ParametricNonZeroCondition::from_authenticated_with_limits(
                polynomial,
                [IdentityConditionSource::IndexTranslation {
                    offset: vec![97; context.index_count()].into_boxed_slice(),
                }],
                limits.identity_conditions,
            )
            .unwrap();
            builder
                .add_sealed_nonzero_condition(context, condition, limits)
                .unwrap();
        }
        let rebuilt = builder.finish();
        match mutation {
            RelationMutation::FirstCoefficient => {
                assert_ne!(rebuilt.terms(), original.terms());
                assert_eq!(rebuilt.nonzero_conditions(), original.nonzero_conditions());
            }
            RelationMutation::AdditionalCondition => {
                assert_eq!(rebuilt.terms(), original.terms());
                assert_ne!(rebuilt.nonzero_conditions(), original.nonzero_conditions());
            }
        }
        rebuilt
    }
}
