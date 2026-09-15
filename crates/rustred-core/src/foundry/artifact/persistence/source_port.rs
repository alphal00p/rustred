//! Stored original-source combinations, never source discovery on cold load.
//! Shared parent arithmetic is regenerated once and then reused by its cells.
mod plans;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::foundry::cell::SourceViewBatch;
use crate::identity::ParametricIbpGenerator;
use crate::sector::{InteriorBounds, OrderingPolicy};

use super::super::error::{ArtifactError, ArtifactPersistenceError};
use super::super::install::{
    ClosingArtifactCandidate, SOURCE_PORT_ALGORITHM_ID as ALGORITHM_ID,
    install_source_port_with_limits, validate_zero_terminal_proofs,
};
use super::super::model::{ArtifactSchemaVersion, ClosedArtifact};
use super::super::source_port::{PreparedOriginalDomain, ReplayLimits};
use super::binary::{Reader, Writer, check_limit, try_vec};
use super::limits::ArtifactLoadLimits;
use super::{decode_source_plan, decode_terminals, encode_source_snapshot};

pub(super) fn encode(
    writer: &mut Writer,
    artifact: &ClosedArtifact,
) -> Result<(), ArtifactPersistenceError> {
    plans::encode(writer, artifact)
}

fn replay_error(error: impl std::fmt::Display) -> ArtifactPersistenceError {
    ArtifactPersistenceError::OriginalDomainReplay {
        detail: error.to_string(),
    }
}

pub(super) fn decode<'input>(
    parent: &Reader<'input>,
    family_bytes: &'input [u8],
    sources_bytes: &'input [u8],
    rules_bytes: &'input [u8],
    terminals_bytes: &'input [u8],
    arity: usize,
    ordering: OrderingPolicy,
    expected_family_fingerprint: &str,
    expected_context_fingerprint: &str,
    limits: ArtifactLoadLimits,
    original_bytes: &[u8],
) -> Result<ClosedArtifact, ArtifactPersistenceError> {
    let source_plan = decode_source_plan(parent, sources_bytes, arity)?;
    let mut family_reader = parent.child(family_bytes);
    let family = super::family::decode(
        &mut family_reader,
        super::family::FamilyGrammar::CompleteVacuum { arity },
    )?;
    family_reader.finish()?;
    if family.fingerprint() != expected_family_fingerprint {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "combined family fingerprint",
        });
    }
    let expected_rows = family.loop_count().checked_mul(family.loop_count()).ok_or(
        ArtifactPersistenceError::ResourceCountOverflow {
            resource: "ordinary source rows",
        },
    )?;
    if source_plan.expected_rows != expected_rows {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "complete ordinary source count",
        });
    }
    check_limit(
        "complete ordinary source rows",
        expected_rows,
        limits.rule_derivation.max_source_rows,
    )?;
    let generator = ParametricIbpGenerator::try_new_with_config(&family, limits.source_generation)
        .map_err(ArtifactError::from)?;
    if generator.context().fingerprint() != expected_context_fingerprint {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "combined indexed coefficient context",
        });
    }
    // Bound/decode all cell, coefficient and terminal payloads before any
    // original-row multiplication. Every child shares the input budgets.
    let mut rules_reader = parent.child(rules_bytes);
    let (parent_plans, cell_plans) =
        plans::decode(&mut rules_reader, generator.context(), expected_rows)?;
    rules_reader.finish()?;
    let mut terminal_reader = parent.child(terminals_bytes);
    let terminals = decode_terminals(&mut terminal_reader, arity)?;
    terminal_reader.finish()?;
    // No untrusted zero label may erase a physical column during replay.
    // The same final installer independently revalidates this zero census.
    validate_zero_terminal_proofs(&family, &terminals.zero_sectors)?;
    let zeros: Vec<_> = terminals
        .zero_sectors
        .iter()
        .map(|terminal| terminal.sector().active_bits())
        .collect();

    let batch = generator
        .prepare_ordinary_ibp()
        .map_err(ArtifactError::from)?;
    if batch.len() != expected_rows {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "regenerated ordinary source count",
        });
    }
    let mut generated = try_vec(batch.len(), "regenerated ordinary source rows")?;
    for ordinal in 0..batch.len() {
        generated.push(batch.generate(ordinal));
    }
    let completed = batch.complete(generated).map_err(ArtifactError::from)?;
    let mut source_witness = parent.replay_writer();
    encode_source_snapshot(&mut source_witness, completed.relations())?;
    if source_witness.finish() != source_plan.semantic_witness {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "combined ordinary source witness",
        });
    }

    let replay_limits = ReplayLimits {
        rule: limits.rule_derivation,
        cell: limits.rule_cells,
        geometry: limits.cover_replay.geometry(),
    };
    let mut prepared = try_vec(parent_plans.len(), "prepared original parents")?;
    for plan in parent_plans {
        let selected = generator
            .translate_selected_completed_source_rows(
                &completed,
                plan.requests.iter().map(|(request, _)| request.clone()),
                limits.translated_sources,
            )
            .map_err(ArtifactError::from)?;
        if !selected
            .requests()
            .iter()
            .eq(plan.requests.iter().map(|(request, _)| request))
        {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "translated source request chronology",
            });
        }
        let ordinals: Vec<_> = (0..selected.len()).collect();
        let sources = SourceViewBatch::try_select(
            selected.into_translated_batch(),
            &ordinals,
            limits.rule_cells,
        )
        .map_err(ArtifactError::from)?;
        let contributions = sources
            .relations()
            .iter()
            .zip(plan.requests)
            .enumerate()
            .map(|(ordinal, (relation, (_, weight)))| (ordinal, relation.row_id().clone(), weight))
            .collect();
        prepared.push(
            PreparedOriginalDomain::try_new(
                generator.context(),
                Arc::new(sources),
                contributions,
                plan.fixed,
                plan.affine,
                std::sync::Arc::from([]),
                plan.conditions,
                replay_limits,
            )
            .map_err(replay_error)?,
        );
    }
    let mut cells = try_vec(cell_plans.len(), "replayed original cells")?;
    for plan in cell_plans {
        let parent =
            prepared
                .get(plan.parent)
                .ok_or(ArtifactPersistenceError::SemanticMismatch {
                    field: "combined parent reference",
                })?;
        cells.push(
            parent
                .verify_cell(
                    generator.context(),
                    ordering,
                    &plan.sector,
                    &zeros,
                    plan.application,
                    plan.rhs,
                )
                .map_err(replay_error)?,
        );
    }
    let context = generator.context().clone();
    drop(generator);
    // Discard shared arithmetic maps, not the unchanged source Arcs now owned
    // by actual cells. This is the same producer/installer as generation.
    drop(prepared);
    let artifact = install_source_port_with_limits(
        ClosingArtifactCandidate {
            schema: ArtifactSchemaVersion::CURRENT,
            algorithm_id: ALGORITHM_ID,
            arity,
            ordering,
            supported_root_power_bounds: vec![InteriorBounds::new(i64::MIN, i64::MAX); arity]
                .into_boxed_slice(),
            family,
            context,
            source_relations: completed.into_relations(),
            rules: Vec::new(),
            rule_cells: cells,
            canonicalizer: None,
            dependencies: Vec::new(),
            factorization_rules: Vec::new(),
            masters: terminals.masters,
            zero_sectors: terminals.zero_sectors,
            common_mass_homogeneity: terminals.common_mass_homogeneity,
        },
        limits.cover_replay.geometry(),
    )?;
    // Deterministic encoding validates canonical sharing/order and all stored
    // semantic payloads, without discovery or reconstructing any other plan.
    if super::encode_with_limits(&artifact, limits.replay_encoding())? != original_bytes {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "combined canonical artifact payload",
        });
    }
    Ok(artifact)
}
