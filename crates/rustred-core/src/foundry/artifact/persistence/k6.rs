//! Durable replay plans for dynamically discovered K=6 rule cells.
//!
//! Search chronology is intentionally absent.  Every cell stores only the
//! canonical translated-source requests and exact derivation inputs needed to
//! regenerate it.  The complete semantic snapshot is then compared byte for
//! byte at the untrusted load boundary before the generic installer receives
//! the cell.

use std::sync::Arc;

use crate::foundry::cell::{
    FixedIndexRestriction, RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewBatch,
    SourceViewConstruction,
};
use crate::foundry::parametric::{ParametricRuleLimits, derive_sector_monotone_rule_for_target};
use crate::identity::{
    IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits, TranslatedSourceRequest,
};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::super::MAX_PUBLISHED_K6_RULE_CELLS;
use super::super::error::{ArtifactError, ArtifactPersistenceError};
use super::super::install::{ClosingArtifactCandidate, install_persisted_k6};
use super::super::model::{ArtifactSchemaVersion, ClosedArtifact, CommonMassHomogeneityProof};
use super::super::three_loop::{
    ALGORITHM_ID, derive_k6_terminal_authority_with_ordering_and_limits,
};
use super::binary::{Reader, Writer, try_vec};
use super::semantic::{encode_i64_slice, encode_rule_snapshot};
use super::{
    ArtifactLoadLimits, K6_CLOSURE_HEADER, K6_REGENERATED_RULE_CELL_PLAN, decode_source_plan,
    encode_family, encode_source_snapshot, encode_with_limits,
};

const DIRECT_SOURCES: u8 = 0;
const FIXED_SOURCES: u8 = 1;
const RESIDUAL_SOURCES: u8 = 2;

pub(super) fn encode(
    writer: &mut Writer,
    artifact: &ClosedArtifact,
) -> Result<(), ArtifactPersistenceError> {
    let mut header = writer.child();
    encode_registered_header(&mut header, artifact)?;
    writer.u16(K6_CLOSURE_HEADER)?;
    writer.bytes(&header.finish(), "K6 closure header")?;
    writer.usize(artifact.rule_cells().len(), "K6 artifact rule cells")?;
    for cell in artifact.rule_cells() {
        let mut plan = writer.child();
        encode_cell_plan(&mut plan, cell)?;
        let bytes = plan.finish();
        writer.charge_witness_payload(bytes.len())?;
        writer.u16(K6_REGENERATED_RULE_CELL_PLAN)?;
        writer.bytes(&bytes, "K6 rule-cell plan")?;
    }
    Ok(())
}

fn encode_registered_header(
    writer: &mut Writer,
    artifact: &ClosedArtifact,
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(
        artifact.supported_root_power_bounds().len(),
        "K6 root-power bounds",
    )?;
    for bounds in artifact.supported_root_power_bounds() {
        writer.i64(bounds.lower())?;
        writer.i64(bounds.upper())?;
    }
    let canonicalizer =
        artifact
            .canonicalizer()
            .ok_or(ArtifactPersistenceError::UnsupportedFeature {
                detail: "K6 closure has no canonical symmetry owner",
            })?;
    writer.string(
        &canonicalizer.ordering().stable_id(),
        "K6 canonical ordering",
    )?;
    writer.usize(canonicalizer.generator_count(), "K6 canonical generators")?;
    writer.usize(canonicalizer.group_order(), "K6 canonical group elements")?;
    for mapping in canonicalizer.group_elements() {
        writer.usize(mapping.len(), "K6 canonical permutation arity")?;
        for &source in mapping {
            writer.usize(source, "K6 canonical permutation source")?;
        }
    }
    writer.usize(artifact.dependencies().len(), "K6 dependencies")?;
    for dependency in artifact.dependencies() {
        let mut nested = writer.child();
        super::encode_into_writer(dependency, &mut nested)?;
        let durable = nested.finish();
        writer.bytes(&durable, "K6 dependency artifact")?;
    }
    writer.usize(artifact.factorization_rules().len(), "K6 factorizations")?;
    for factorization in artifact.factorization_rules() {
        encode_interior_domain(writer, factorization.application_domain())?;
        super::coefficient::encode_base_coefficient(writer, factorization.normalization())?;
        writer.usize(
            factorization.loop_basis().dimension(),
            "K6 loop-basis dimension",
        )?;
        encode_i64_slice(writer, factorization.loop_basis().row_major())?;
        writer.usize(factorization.factors().len(), "K6 factorization factors")?;
        for factor in factorization.factors() {
            writer.usize(factor.dependency_ordinal(), "K6 dependency ordinal")?;
            writer.usize(factor.parent_positions().len(), "K6 parent positions")?;
            for &position in factor.parent_positions() {
                writer.usize(position, "K6 parent position")?;
            }
            writer.usize(
                factor.transformed_loop_positions().len(),
                "K6 transformed loop positions",
            )?;
            for &position in factor.transformed_loop_positions() {
                writer.usize(position, "K6 transformed loop position")?;
            }
        }
    }
    Ok(())
}

fn encode_cell_plan(writer: &mut Writer, cell: &RuleCell) -> Result<(), ArtifactPersistenceError> {
    let sources = cell.sources();
    writer.usize(sources.provenance().len(), "K6 cell source requests")?;
    for provenance in sources.provenance() {
        if provenance.symmetry().is_some() {
            return Err(ArtifactPersistenceError::UnsupportedFeature {
                detail: "K6 persistence does not admit detached source symmetry tags",
            });
        }
        writer.usize(
            provenance.translated().source_ordinal(),
            "K6 source ordinal",
        )?;
        encode_i64_slice(writer, provenance.translated().offset().values())?;
    }
    match sources.construction() {
        SourceViewConstruction::Direct => writer.u8(DIRECT_SOURCES)?,
        SourceViewConstruction::FixedIndexSpecialization(evidence) => {
            writer.u8(FIXED_SOURCES)?;
            encode_fixed(writer, evidence.fixed_restrictions())?;
        }
        SourceViewConstruction::ResidualProjection(evidence) => {
            writer.u8(RESIDUAL_SOURCES)?;
            encode_interior_domain(writer, evidence.domain())?;
            encode_fixed(writer, evidence.fixed_restrictions())?;
        }
    }
    encode_i64_slice(writer, cell.rule().concrete_replay().anchor().powers())?;
    encode_i64_slice(writer, cell.rule().pivot().values())?;
    encode_monotone_domain(writer, cell.application_domain())?;
    encode_fixed(writer, cell.fixed_restrictions())?;
    writer.usize(cell.pruned_rhs_ordinals().len(), "K6 pruned RHS ordinals")?;
    for &ordinal in cell.pruned_rhs_ordinals() {
        writer.usize(ordinal, "K6 pruned RHS ordinal")?;
    }
    let snapshot = encode_cell_witness(writer, cell)?;
    writer.bytes(&snapshot, "K6 exact cell witness")
}

#[allow(clippy::too_many_arguments)]
pub(super) fn decode(
    parent: &Reader<'_>,
    family_bytes: &[u8],
    sources_bytes: &[u8],
    rules_bytes: &[u8],
    terminals_bytes: &[u8],
    ordering: OrderingPolicy,
    expected_family_fingerprint: &str,
    expected_context_fingerprint: &str,
    limits: ArtifactLoadLimits,
    original_bytes: &[u8],
) -> Result<ClosedArtifact, ArtifactPersistenceError> {
    let authority = derive_k6_terminal_authority_with_ordering_and_limits(
        ordering,
        limits.family,
        limits.source_generation,
        limits.rule_derivation,
    )
    .map_err(ArtifactPersistenceError::from)?;
    let parts = authority.into_artifact_parts();
    if parts.arity != 6
        || parts.family.fingerprint() != expected_family_fingerprint
        || parts.context.fingerprint() != expected_context_fingerprint
    {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 registered family metadata",
        });
    }

    let replay = parent.replay_writer();
    let mut expected_family = replay.child();
    encode_family(&mut expected_family, &parts.family)?;
    if expected_family.finish().as_slice() != family_bytes {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 family witness",
        });
    }

    let generator =
        ParametricIbpGenerator::try_new_with_config(&parts.family, limits.source_generation)
            .map_err(ArtifactError::from)?;
    let prepared = generator
        .prepare_ordinary_ibp()
        .map_err(ArtifactError::from)?;
    if prepared.len() != 9 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 ordinary source count",
        });
    }
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows).map_err(ArtifactError::from)?;
    let source_plan = decode_source_plan(parent, sources_bytes, 6)?;
    if source_plan.expected_rows != 9 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 source-plan row count",
        });
    }
    let mut expected_sources = replay.child();
    encode_source_snapshot(&mut expected_sources, completed.relations())?;
    if expected_sources.finish().as_slice() != source_plan.semantic_witness {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 source derivation witness",
        });
    }

    let mut rule_reader = parent.child(rules_bytes);
    if rule_reader.u16()? != K6_CLOSURE_HEADER {
        return Err(ArtifactPersistenceError::UnsupportedFeature {
            detail: "unknown K6 closure header",
        });
    }
    let header = rule_reader.bytes("K6 closure header", limits.max_artifact_bytes)?;
    let supported_root_power_bounds = decode_root_bounds(parent, header, &parts, ordering)?;
    let cell_count = rule_reader.count("K6 artifact rule cells")?;
    preflight_k6_rule_cell_count(cell_count)?;
    if cell_count == 0 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 empty rule-cell set",
        });
    }
    let mut rule_cells = try_vec(cell_count, "K6 artifact rule cells")?;
    for _ in 0..cell_count {
        if rule_reader.u16()? != K6_REGENERATED_RULE_CELL_PLAN {
            return Err(ArtifactPersistenceError::UnsupportedFeature {
                detail: "unknown K6 rule-cell derivation plan",
            });
        }
        let plan = rule_reader.bytes("K6 rule-cell plan", limits.max_artifact_bytes)?;
        rule_reader.charge_witness_payload(plan.len())?;
        rule_cells.push(Arc::new(decode_cell(
            parent,
            plan,
            &generator,
            &completed,
            parts
                .canonicalizer
                .as_ref()
                .ok_or(ArtifactPersistenceError::SemanticMismatch {
                    field: "K6 canonicalizer",
                })?,
            &parts.zero_sectors,
            ordering,
            limits.translated_sources,
            limits.rule_derivation,
            limits.rule_cells,
        )?));
    }
    rule_reader.finish()?;
    drop(generator);
    // Delay this owned transfer until every cold replay helper has finished
    // borrowing the complete registered terminal payload.
    let factorized_product_programs = parts.factorized_product_programs;

    let candidate = ClosingArtifactCandidate {
        schema: ArtifactSchemaVersion::CURRENT,
        algorithm_id: ALGORITHM_ID,
        arity: parts.arity,
        ordering,
        supported_root_power_bounds,
        family: parts.family,
        context: parts.context,
        source_relations: completed.into_relations(),
        rules: Vec::new(),
        rule_cells,
        canonicalizer: parts.canonicalizer,
        dependencies: parts.dependencies,
        factorization_rules: parts.factorization_rules,
        masters: parts.masters,
        zero_sectors: parts.zero_sectors,
        common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
    };

    let mut expected_terminals = replay.child();
    // A temporary sealed shell is unnecessary: terminal bytes depend only on
    // these three candidate fields.
    encode_candidate_terminals(&mut expected_terminals, &candidate)?;
    if expected_terminals.finish().as_slice() != terminals_bytes {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 terminal witness",
        });
    }

    let artifact = install_persisted_k6(
        candidate,
        factorized_product_programs,
        limits.source_generation,
        limits.translated_sources,
        limits.cover_replay,
    )
    .map_err(ArtifactPersistenceError::from)?;
    let regenerated = encode_with_limits(&artifact, limits.replay_encoding())?;
    if regenerated.as_slice() != original_bytes {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 complete artifact witness",
        });
    }
    Ok(artifact)
}

fn decode_root_bounds(
    parent: &Reader<'_>,
    header: &[u8],
    parts: &super::super::terminal::TerminalArtifactParts,
    ordering: OrderingPolicy,
) -> Result<Box<[InteriorBounds]>, ArtifactPersistenceError> {
    let mut reader = parent.child(header);
    let count = reader.count("K6 root-power bounds")?;
    check_persistence_limit(
        "K6 root-power bounds",
        count,
        reader.limits().max_index_arity,
    )?;
    if count != 6 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 root-power bound arity",
        });
    }
    let mut bounds = try_vec(count, "K6 root-power bounds")?;
    for _ in 0..count {
        let lower = reader.i64()?;
        let upper = reader.i64()?;
        if lower > upper || lower > 0 || upper < 1 {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "K6 root-power bounds",
            });
        }
        bounds.push(InteriorBounds::new(lower, upper));
    }
    // The registered K4 action is transitive on all six propagator slots.
    // Reduction canonicalizes only after admitting a target against this root
    // box, so the admitted universe itself must be invariant under that action.
    if bounds.windows(2).any(|pair| pair[0] != pair[1]) {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 symmetry-invariant root-power bounds",
        });
    }
    // Compare the remaining immutable terminal authority byte-for-byte.  The
    // root box is closure output, while symmetry/dependencies/factorizations
    // are registered input and must not be caller-authored.
    let mut expected = parent.replay_writer().child();
    expected.usize(bounds.len(), "K6 root-power bounds")?;
    for bound in &bounds {
        expected.i64(bound.lower())?;
        expected.i64(bound.upper())?;
    }
    encode_authority_tail(&mut expected, parts, ordering)?;
    if expected.finish().as_slice() != header {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 terminal authority header",
        });
    }
    Ok(bounds.into_boxed_slice())
}

fn encode_authority_tail(
    writer: &mut Writer,
    parts: &super::super::terminal::TerminalArtifactParts,
    ordering: OrderingPolicy,
) -> Result<(), ArtifactPersistenceError> {
    let canonicalizer =
        parts
            .canonicalizer
            .as_ref()
            .ok_or(ArtifactPersistenceError::SemanticMismatch {
                field: "K6 canonicalizer",
            })?;
    writer.string(&ordering.stable_id(), "K6 canonical ordering")?;
    writer.usize(canonicalizer.generator_count(), "K6 canonical generators")?;
    writer.usize(canonicalizer.group_order(), "K6 canonical group elements")?;
    for mapping in canonicalizer.group_elements() {
        writer.usize(mapping.len(), "K6 canonical permutation arity")?;
        for &source in mapping {
            writer.usize(source, "K6 canonical permutation source")?;
        }
    }
    writer.usize(parts.dependencies.len(), "K6 dependencies")?;
    for dependency in &parts.dependencies {
        let mut nested = writer.child();
        super::encode_into_writer(dependency, &mut nested)?;
        let durable = nested.finish();
        writer.bytes(&durable, "K6 dependency artifact")?;
    }
    writer.usize(parts.factorization_rules.len(), "K6 factorizations")?;
    for factorization in &parts.factorization_rules {
        encode_interior_domain(writer, factorization.application_domain())?;
        super::coefficient::encode_base_coefficient(writer, factorization.normalization())?;
        writer.usize(
            factorization.loop_basis().dimension(),
            "K6 loop-basis dimension",
        )?;
        encode_i64_slice(writer, factorization.loop_basis().row_major())?;
        writer.usize(factorization.factors().len(), "K6 factorization factors")?;
        for factor in factorization.factors() {
            writer.usize(factor.dependency_ordinal(), "K6 dependency ordinal")?;
            writer.usize(factor.parent_positions().len(), "K6 parent positions")?;
            for &position in factor.parent_positions() {
                writer.usize(position, "K6 parent position")?;
            }
            writer.usize(
                factor.transformed_loop_positions().len(),
                "K6 transformed loop positions",
            )?;
            for &position in factor.transformed_loop_positions() {
                writer.usize(position, "K6 transformed loop position")?;
            }
        }
    }
    Ok(())
}

fn decode_cell(
    parent: &Reader<'_>,
    bytes: &[u8],
    generator: &ParametricIbpGenerator<'_>,
    completed: &crate::identity::CompletedIbpSourceRows,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[super::super::model::ZeroSectorTerminal],
    ordering: OrderingPolicy,
    translated_source_limits: TranslatedSourceLimits,
    rule_limits: ParametricRuleLimits,
    cell_limits: RuleCellLimits,
) -> Result<RuleCell, ArtifactPersistenceError> {
    let preflight_request_count = preflight_cell_source_requests(
        parent,
        bytes,
        completed.relations().len(),
        translated_source_limits,
        cell_limits,
    )?;
    let mut reader = parent.child(bytes);
    let request_count = reader.count("K6 cell source requests")?;
    debug_assert_eq!(request_count, preflight_request_count);
    if request_count == 0 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 empty cell source span",
        });
    }
    let mut requests = try_vec(request_count, "K6 cell source requests")?;
    for _ in 0..request_count {
        let ordinal = reader.count("K6 source ordinal")?;
        let offset = IntegralShift::try_new(decode_exact_i64_array::<6>(
            &mut reader,
            "K6 source offset",
            "K6 source-offset arity",
        )?)
        .map_err(ArtifactError::from)?;
        requests.push(TranslatedSourceRequest::new(ordinal, offset));
    }
    let construction = reader.u8()?;
    let construction_domain = if construction == RESIDUAL_SOURCES {
        Some(decode_interior_domain(&mut reader, 6)?)
    } else {
        None
    };
    let construction_fixed = if construction == FIXED_SOURCES || construction == RESIDUAL_SOURCES {
        decode_fixed(&mut reader, 6, cell_limits.max_fixed_restrictions)?
    } else if construction == DIRECT_SOURCES {
        Box::new([])
    } else {
        return Err(ArtifactPersistenceError::UnsupportedFeature {
            detail: "unknown K6 source-view construction",
        });
    };
    let anchor =
        decode_exact_i64_array::<6>(&mut reader, "K6 rule anchor", "K6 rule derivation arity")?;
    let target_shift = decode_exact_i64_array::<6>(
        &mut reader,
        "K6 rule target shift",
        "K6 rule derivation arity",
    )?;
    let application_sector = decode_mask(&mut reader, 6)?;
    let application_bounds = decode_bounds(&mut reader, 6, "K6 monotone bounds")?;
    let fixed = decode_fixed(&mut reader, 6, cell_limits.max_fixed_restrictions)?;
    let pruned = decode_pruned_ordinals(&mut reader, cell_limits.max_pruned_terms)?;
    let expected_snapshot =
        reader.bytes("K6 exact cell witness", parent.limits().max_artifact_bytes)?;
    reader.finish()?;

    let selected = generator
        .translate_selected_completed_source_rows(completed, requests, translated_source_limits)
        .map_err(ArtifactError::from)?;
    let sources = match construction {
        DIRECT_SOURCES | FIXED_SOURCES => {
            SourceViewBatch::try_from_complete_selected(selected, construction_fixed)
                .map_err(ArtifactError::from)?
        }
        RESIDUAL_SOURCES => SourceViewBatch::try_project_complete_residual(
            selected.into_translated_batch(),
            generator.context(),
            construction_domain.expect("residual construction decoded a domain"),
            construction_fixed.iter().copied(),
            canonicalizer,
            &zero_sectors
                .iter()
                .map(|terminal| terminal.sector().clone())
                .collect::<Vec<_>>(),
            cell_limits,
        )
        .map_err(ArtifactError::from)?,
        _ => unreachable!(),
    };
    let rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &anchor,
        &target_shift,
        ordering,
        rule_limits,
    )
    .map_err(ArtifactError::from)?;
    let rhs_shifts = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let application = SectorMonotoneDomain::try_new_for_rule(
        application_sector,
        application_bounds,
        rule.pivot().values(),
        &rhs_shifts,
    )
    .map_err(ArtifactError::from)?;
    let cell = RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        fixed.iter().copied(),
        pruned,
        cell_limits,
    )
    .map_err(ArtifactError::from)?;
    let actual_snapshot = encode_cell_witness(&parent.replay_writer(), &cell)?;
    if actual_snapshot.as_slice() != expected_snapshot {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 rule-cell replay witness",
        });
    }
    Ok(cell)
}

/// Parse the source-request prefix without retaining any caller-sized
/// container. Durable K6 plans are emitted in exact offset-major/source-minor
/// order, so distinct offsets and duplicate pairs can be checked online.
fn preflight_cell_source_requests(
    parent: &Reader<'_>,
    bytes: &[u8],
    source_count: usize,
    translated: TranslatedSourceLimits,
    cells: RuleCellLimits,
) -> Result<usize, ArtifactPersistenceError> {
    let mut reader = parent.child(bytes);
    let request_count = reader.count("K6 cell source requests")?;
    if request_count == 0 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 empty cell source span",
        });
    }
    for (resource, limit) in [
        (
            "K6 requested source translations",
            translated.max_requested_source_translations,
        ),
        ("K6 translated sources", translated.max_translated_sources),
        ("K6 cell source views", cells.max_source_views),
    ] {
        check_persistence_limit(resource, request_count, limit)?;
    }
    let coordinate_cells =
        request_count
            .checked_mul(6)
            .ok_or(ArtifactPersistenceError::ResourceCountOverflow {
                resource: "K6 retained source-offset coordinate cells",
            })?;
    check_persistence_limit(
        "K6 retained source-offset coordinate cells",
        coordinate_cells,
        translated.max_retained_index_coordinate_cells,
    )?;

    let mut previous: Option<([i64; 6], usize)> = None;
    let mut distinct_offsets = 0usize;
    for _ in 0..request_count {
        let ordinal = reader.count("K6 source ordinal")?;
        if ordinal >= source_count {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "K6 source ordinal",
            });
        }
        let offset =
            decode_exact_i64_array::<6>(&mut reader, "K6 source offset", "K6 source-offset arity")?;
        if let Some((previous_offset, previous_ordinal)) = previous {
            if (offset, ordinal) <= (previous_offset, previous_ordinal) {
                return Err(ArtifactPersistenceError::SemanticMismatch {
                    field: "K6 canonical source-request order",
                });
            }
            if offset != previous_offset {
                distinct_offsets = distinct_offsets.checked_add(1).ok_or(
                    ArtifactPersistenceError::ResourceCountOverflow {
                        resource: "K6 canonical source offsets",
                    },
                )?;
            }
        } else {
            distinct_offsets = 1;
        }
        check_persistence_limit(
            "K6 canonical source offsets",
            distinct_offsets,
            translated.max_requested_offsets,
        )?;
        previous = Some((offset, ordinal));
    }
    Ok(request_count)
}

fn decode_exact_i64_array<const N: usize>(
    reader: &mut Reader<'_>,
    resource: &'static str,
    arity_field: &'static str,
) -> Result<[i64; N], ArtifactPersistenceError> {
    let count = reader.count(resource)?;
    check_persistence_limit(resource, count, reader.limits().max_index_arity)?;
    if count != N {
        return Err(ArtifactPersistenceError::SemanticMismatch { field: arity_field });
    }
    let mut values = [0_i64; N];
    for value in &mut values {
        *value = reader.i64()?;
    }
    Ok(values)
}

fn check_persistence_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ArtifactPersistenceError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(ArtifactPersistenceError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn preflight_k6_rule_cell_count(cell_count: usize) -> Result<(), ArtifactPersistenceError> {
    check_persistence_limit(
        "K6 artifact rule cells",
        cell_count,
        MAX_PUBLISHED_K6_RULE_CELLS,
    )
}

/// Compact complete cell witness. Source relations and residual dispositions
/// are deliberately absent because the load plan independently regenerates
/// them from canonical ordinary rows; duplicating them in every cell would
/// make a large K6 artifact scale with repeated source payload rather than
/// executable rules.
fn encode_cell_witness(
    parent: &Writer,
    cell: &RuleCell,
) -> Result<Vec<u8>, ArtifactPersistenceError> {
    let mut writer = parent.child();
    let rule = encode_rule_snapshot(cell.rule(), &writer)?;
    writer.bytes(&rule, "K6 exact rule witness")?;
    encode_interior_domain(&mut writer, cell.proof_domain())?;
    encode_monotone_domain(&mut writer, cell.application_domain())?;
    writer.u8(match cell.domain_proof() {
        RuleCellDomainProof::TightenedOriginalInterior => 0,
        RuleCellDomainProof::ReprovedSectorMonotone => 1,
    })?;
    encode_fixed(&mut writer, cell.fixed_restrictions())?;
    writer.usize(cell.pruned_rhs_ordinals().len(), "K6 witness pruned RHS")?;
    for &ordinal in cell.pruned_rhs_ordinals() {
        writer.usize(ordinal, "K6 witness pruned RHS ordinal")?;
    }
    writer.usize(cell.terms().len(), "K6 witness retained RHS")?;
    for term in cell.terms() {
        writer.usize(term.source_rhs_ordinal(), "K6 witness retained RHS ordinal")?;
    }
    writer.usize(cell.guards().len(), "K6 witness guards")?;
    for guard in cell.guards() {
        writer.usize(guard.source_guard_ordinal(), "K6 witness guard ordinal")?;
        super::coefficient::encode_indexed_polynomial(&mut writer, guard.polynomial())?;
    }
    Ok(writer.finish())
}

fn encode_candidate_terminals(
    writer: &mut Writer,
    candidate: &ClosingArtifactCandidate,
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(candidate.masters.len(), "master terminals")?;
    for master in &candidate.masters {
        super::semantic::encode_integral_key(writer, master)?;
    }
    writer.usize(candidate.zero_sectors.len(), "zero-sector terminals")?;
    for terminal in &candidate.zero_sectors {
        super::semantic::encode_bool_slice(writer, terminal.sector().active_bits())?;
        writer.u8(match terminal.proof() {
            super::super::model::ZeroTerminalProof::ScalelessVacuumPolynomial => 0,
            super::super::model::ZeroTerminalProof::LeePomeranskyRankDeficiency => 1,
        })?;
    }
    writer.u8(1)
}

fn encode_interior_domain(
    writer: &mut Writer,
    domain: &SectorInteriorDomain,
) -> Result<(), ArtifactPersistenceError> {
    super::semantic::encode_bool_slice(writer, domain.sector().active_bits())?;
    writer.usize(domain.bounds().len(), "K6 interior bounds")?;
    for bound in domain.bounds() {
        writer.i64(bound.lower())?;
        writer.i64(bound.upper())?;
    }
    Ok(())
}

fn encode_monotone_domain(
    writer: &mut Writer,
    domain: &SectorMonotoneDomain,
) -> Result<(), ArtifactPersistenceError> {
    super::semantic::encode_bool_slice(writer, domain.sector().active_bits())?;
    writer.usize(domain.bounds().len(), "K6 monotone bounds")?;
    for bound in domain.bounds() {
        writer.i64(bound.lower())?;
        writer.i64(bound.upper())?;
    }
    Ok(())
}

fn decode_interior_domain(
    reader: &mut Reader<'_>,
    arity: usize,
) -> Result<SectorInteriorDomain, ArtifactPersistenceError> {
    let sector = decode_mask(reader, arity)?;
    let bounds = decode_bounds(reader, arity, "K6 interior bounds")?;
    SectorInteriorDomain::try_new(sector, bounds)
        .map_err(ArtifactError::from)
        .map_err(Into::into)
}

fn decode_mask(reader: &mut Reader<'_>, arity: usize) -> Result<Mask, ArtifactPersistenceError> {
    if arity != 6 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 sector arity",
        });
    }
    let count = reader.count("K6 sector mask")?;
    check_persistence_limit("K6 sector mask", count, reader.limits().max_index_arity)?;
    if count != arity {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 sector arity",
        });
    }
    let mut active = [false; 6];
    for value in &mut active {
        *value = match reader.u8()? {
            0 => false,
            1 => true,
            _ => {
                return Err(ArtifactPersistenceError::SemanticMismatch {
                    field: "boolean encoding",
                });
            }
        };
    }
    Mask::try_new(active)
        .map_err(ArtifactError::from)
        .map_err(Into::into)
}

fn decode_bounds(
    reader: &mut Reader<'_>,
    arity: usize,
    resource: &'static str,
) -> Result<Vec<InteriorBounds>, ArtifactPersistenceError> {
    let count = reader.count(resource)?;
    check_persistence_limit(resource, count, reader.limits().max_index_arity)?;
    if count != arity {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 domain-bound arity",
        });
    }
    let mut bounds = try_vec(count, resource)?;
    for _ in 0..count {
        bounds.push(InteriorBounds::new(reader.i64()?, reader.i64()?));
    }
    Ok(bounds)
}

fn encode_fixed(
    writer: &mut Writer,
    fixed: &[FixedIndexRestriction],
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(fixed.len(), "K6 fixed restrictions")?;
    for restriction in fixed {
        writer.usize(restriction.position(), "K6 fixed position")?;
        writer.i64(restriction.value())?;
    }
    Ok(())
}

fn decode_fixed(
    reader: &mut Reader<'_>,
    arity: usize,
    limit: usize,
) -> Result<Box<[FixedIndexRestriction]>, ArtifactPersistenceError> {
    let count = reader.count("K6 fixed restrictions")?;
    check_persistence_limit("K6 fixed restrictions", count, limit)?;
    if count > arity {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 fixed restriction count",
        });
    }
    let mut fixed = try_vec(count, "K6 fixed restrictions")?;
    for _ in 0..count {
        let position = reader.count("K6 fixed position")?;
        if position >= arity {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "K6 fixed position",
            });
        }
        fixed.push(FixedIndexRestriction::new(position, reader.i64()?));
    }
    if fixed
        .windows(2)
        .any(|pair| pair[0].position() >= pair[1].position())
    {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "K6 fixed restriction order",
        });
    }
    Ok(fixed.into_boxed_slice())
}

fn decode_pruned_ordinals(
    reader: &mut Reader<'_>,
    limit: usize,
) -> Result<Vec<usize>, ArtifactPersistenceError> {
    let count = reader.count("K6 pruned RHS ordinals")?;
    check_persistence_limit("K6 pruned RHS ordinals", count, limit)?;
    let mut pruned = try_vec(count, "K6 pruned RHS ordinals")?;
    for _ in 0..count {
        pruned.push(reader.count("K6 pruned RHS ordinal")?);
    }
    Ok(pruned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundry::artifact::ArtifactEncodingLimits;
    use crate::foundry::artifact::three_loop::derive_k6_terminal_authority_with_ordering;
    use crate::foundry::parametric::derive_sector_interior_rule;
    use crate::identity::ParametricIbpConfig;

    fn decode_error(bytes: &[u8], limits: ArtifactLoadLimits) -> ArtifactPersistenceError {
        match ClosedArtifact::decode_durable_with_limits(bytes, limits) {
            Ok(_) => panic!("caller-tightened K6 load unexpectedly succeeded"),
            Err(error) => error,
        }
    }

    fn synthetic_exact_artifact() -> ClosedArtifact {
        let ordering = OrderingPolicy::default();
        let parts = derive_k6_terminal_authority_with_ordering(ordering)
            .unwrap()
            .into_artifact_parts();
        let generator = ParametricIbpGenerator::try_new_with_config(
            &parts.family,
            ParametricIbpConfig::default(),
        )
        .unwrap();
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len())
            .map(|ordinal| prepared.generate(ordinal))
            .collect();
        let completed = prepared.complete(rows).unwrap();
        // The complete ordinary K=6 source has positive and negative unit
        // shifts in every coordinate.  An all-active depth-two point is
        // therefore a genuine interior anchor independent of which terminal
        // sector is used to prove the deliberately tiny persisted root box.
        let anchor = [2; 6];
        let seed = derive_sector_interior_rule(
            generator.context(),
            completed.relations(),
            &anchor,
            ordering,
            ParametricRuleLimits::default(),
        )
        .unwrap();
        let target_shift = seed.pivot().values().to_vec();
        let zero = IntegralShift::try_new([0; 6]).unwrap();
        let selected = generator
            .translate_selected_completed_source_rows(
                &completed,
                (0..9).map(|ordinal| TranslatedSourceRequest::new(ordinal, zero.clone())),
                TranslatedSourceLimits::default(),
            )
            .unwrap();
        let sources = SourceViewBatch::try_from_complete_selected(selected, Box::new([])).unwrap();
        let rule = derive_sector_monotone_rule_for_target(
            generator.context(),
            sources.relations(),
            &anchor,
            &target_shift,
            ordering,
            ParametricRuleLimits::default(),
        )
        .unwrap();
        let application = rule.sector_monotone_admission().unwrap().domain().clone();
        let cell = RuleCell::try_refined(
            generator.context(),
            rule,
            sources,
            application,
            [],
            [],
            Default::default(),
        )
        .unwrap();
        drop(generator);
        let factorized_product_programs = parts.factorized_product_programs;
        install_persisted_k6(
            ClosingArtifactCandidate {
                schema: ArtifactSchemaVersion::CURRENT,
                algorithm_id: ALGORITHM_ID,
                arity: parts.arity,
                ordering,
                supported_root_power_bounds: vec![InteriorBounds::new(0, 1); 6].into_boxed_slice(),
                family: parts.family,
                context: parts.context,
                source_relations: completed.into_relations(),
                rules: Vec::new(),
                rule_cells: vec![Arc::new(cell)],
                canonicalizer: parts.canonicalizer,
                dependencies: parts.dependencies,
                factorization_rules: parts.factorization_rules,
                masters: parts.masters,
                zero_sectors: parts.zero_sectors,
                common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
            },
            factorized_product_programs,
            ParametricIbpConfig::default(),
            TranslatedSourceLimits::default(),
            Default::default(),
        )
        .unwrap()
    }

    #[test]
    fn synthetic_exact_k6_artifact_roundtrips_canonically_and_rejects_tampering() {
        let artifact = synthetic_exact_artifact();
        let first = artifact.encode_durable().unwrap();
        let second = synthetic_exact_artifact().encode_durable().unwrap();
        assert_eq!(first, second);
        let loaded = ClosedArtifact::decode_durable(&first).unwrap();
        assert_eq!(loaded.algorithm_id(), ALGORITHM_ID);
        assert_eq!(loaded.encode_durable().unwrap(), first);

        let mut tampered = first;
        let last = tampered.last_mut().unwrap();
        *last ^= 1;
        assert!(ClosedArtifact::decode_durable(&tampered).is_err());
    }

    #[test]
    fn cold_k6_replay_obeys_caller_translation_cell_and_cover_limits() {
        let bytes = synthetic_exact_artifact().encode_durable().unwrap();

        let mut translation_limited = ArtifactLoadLimits::default();
        translation_limited
            .translated_sources
            .max_requested_source_translations = 8;
        assert_eq!(
            decode_error(&bytes, translation_limited),
            ArtifactPersistenceError::ResourceLimit {
                resource: "K6 requested source translations",
                requested: 9,
                limit: 8,
            }
        );

        let mut offset_limited = ArtifactLoadLimits::default();
        offset_limited.translated_sources.max_requested_offsets = 0;
        assert_eq!(
            decode_error(&bytes, offset_limited),
            ArtifactPersistenceError::ResourceLimit {
                resource: "K6 canonical source offsets",
                requested: 1,
                limit: 0,
            }
        );

        let mut coordinate_limited = ArtifactLoadLimits::default();
        coordinate_limited
            .translated_sources
            .max_retained_index_coordinate_cells = 53;
        assert_eq!(
            decode_error(&bytes, coordinate_limited),
            ArtifactPersistenceError::ResourceLimit {
                resource: "K6 retained source-offset coordinate cells",
                requested: 54,
                limit: 53,
            }
        );

        let mut source_view_limited = ArtifactLoadLimits::default();
        source_view_limited.rule_cells.max_source_views = 8;
        assert_eq!(
            decode_error(&bytes, source_view_limited),
            ArtifactPersistenceError::ResourceLimit {
                resource: "K6 cell source views",
                requested: 9,
                limit: 8,
            }
        );

        let mut cell_limited = ArtifactLoadLimits::default();
        cell_limited.rule_cells.max_retained_terms = 0;
        assert!(ClosedArtifact::decode_durable_with_limits(&bytes, cell_limited).is_err());

        let mut cover_limited = ArtifactLoadLimits::default();
        cover_limited.cover_replay.max_requested_boxes = 0;
        assert_eq!(
            decode_error(&bytes, cover_limited),
            ArtifactPersistenceError::ResourceLimit {
                resource: "requested structural cover boxes",
                requested: 1,
                limit: 0,
            }
        );
    }

    #[test]
    fn cold_k6_cover_replay_limits_are_typed_and_exactly_one_below() {
        let bytes = synthetic_exact_artifact().encode_durable().unwrap();
        type Tighten = fn(&mut ArtifactLoadLimits);
        let cases: [(Tighten, &'static str, usize, usize); 6] = [
            (
                |limits: &mut ArtifactLoadLimits| limits.cover_replay.max_arity = 5,
                "completion coordinate arity",
                6,
                5,
            ),
            (
                |limits: &mut ArtifactLoadLimits| limits.cover_replay.max_requested_boxes = 0,
                "requested structural cover boxes",
                1,
                0,
            ),
            (
                |limits: &mut ArtifactLoadLimits| {
                    limits.cover_replay.max_requested_box_coordinate_cells = 11
                },
                "requested structural-cover coordinate cells",
                12,
                11,
            ),
            (
                |limits: &mut ArtifactLoadLimits| limits.cover_replay.max_uncovered_boxes = 0,
                "uncovered lattice boxes",
                1,
                0,
            ),
            (
                |limits: &mut ArtifactLoadLimits| {
                    limits.cover_replay.max_uncovered_box_coordinate_cells = 11
                },
                "uncovered-box coordinate cells",
                12,
                11,
            ),
            (
                |limits: &mut ArtifactLoadLimits| limits.cover_replay.max_split_operations = 0,
                "structural-box split operations",
                1,
                0,
            ),
        ];
        for (tighten, resource, requested, limit) in cases {
            let mut limits = ArtifactLoadLimits::default();
            tighten(&mut limits);
            assert_eq!(
                decode_error(&bytes, limits),
                ArtifactPersistenceError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                },
                "typed one-below failure changed for {resource}",
            );
        }
    }

    #[test]
    fn cold_k6_vector_and_cell_container_preflights_precede_retention() {
        let parent = Reader::root(&[], ArtifactLoadLimits::default()).unwrap();

        let mut wrong_width = Writer::new(ArtifactEncodingLimits::default());
        wrong_width.usize(5, "test vector").unwrap();
        for _ in 0..5 {
            wrong_width.i64(0).unwrap();
        }
        let bytes = wrong_width.finish();
        let mut reader = parent.child(&bytes);
        assert_eq!(
            decode_exact_i64_array::<6>(&mut reader, "test vector", "test arity"),
            Err(ArtifactPersistenceError::SemanticMismatch {
                field: "test arity",
            })
        );

        let mut fixed = Writer::new(ArtifactEncodingLimits::default());
        fixed.usize(1, "K6 fixed restrictions").unwrap();
        fixed.usize(0, "K6 fixed position").unwrap();
        fixed.i64(0).unwrap();
        let bytes = fixed.finish();
        let mut reader = parent.child(&bytes);
        assert_eq!(
            decode_fixed(&mut reader, 6, 0),
            Err(ArtifactPersistenceError::ResourceLimit {
                resource: "K6 fixed restrictions",
                requested: 1,
                limit: 0,
            })
        );

        let mut pruned = Writer::new(ArtifactEncodingLimits::default());
        pruned.usize(1, "K6 pruned RHS ordinals").unwrap();
        pruned.usize(0, "K6 pruned RHS ordinal").unwrap();
        let bytes = pruned.finish();
        let mut reader = parent.child(&bytes);
        assert_eq!(
            decode_pruned_ordinals(&mut reader, 0),
            Err(ArtifactPersistenceError::ResourceLimit {
                resource: "K6 pruned RHS ordinals",
                requested: 1,
                limit: 0,
            })
        );

        assert!(preflight_k6_rule_cell_count(MAX_PUBLISHED_K6_RULE_CELLS).is_ok());
        assert_eq!(
            preflight_k6_rule_cell_count(MAX_PUBLISHED_K6_RULE_CELLS + 1),
            Err(ArtifactPersistenceError::ResourceLimit {
                resource: "K6 artifact rule cells",
                requested: MAX_PUBLISHED_K6_RULE_CELLS + 1,
                limit: MAX_PUBLISHED_K6_RULE_CELLS,
            })
        );
    }

    #[test]
    fn cold_k6_header_rejects_root_bounds_that_are_not_symmetry_invariant() {
        let ordering = OrderingPolicy::default();
        let parts = derive_k6_terminal_authority_with_ordering(ordering)
            .unwrap()
            .into_artifact_parts();
        let parent = Reader::root(&[], ArtifactLoadLimits::default()).unwrap();
        let mut header = parent.replay_writer().child();
        header.usize(6, "K6 root-power bounds").unwrap();
        for ordinal in 0..6 {
            header.i64(if ordinal == 0 { -1 } else { 0 }).unwrap();
            header.i64(1).unwrap();
        }
        encode_authority_tail(&mut header, &parts, ordering).unwrap();

        assert!(matches!(
            decode_root_bounds(&parent, &header.finish(), &parts, ordering),
            Err(ArtifactPersistenceError::SemanticMismatch {
                field: "K6 symmetry-invariant root-power bounds",
            })
        ));
    }

    #[test]
    fn cold_cell_replay_rejects_a_forged_witness_without_an_outer_digest() {
        let artifact = synthetic_exact_artifact();
        let mut plan_writer = Writer::new(ArtifactEncodingLimits::default());
        encode_cell_plan(&mut plan_writer, &artifact.rule_cells()[0]).unwrap();
        let mut forged_plan = plan_writer.finish();
        // The exact witness is the final length-delimited field in the cell
        // plan. Mutating its payload while calling `decode_cell` directly
        // bypasses artifact framing and its outer checksum entirely.
        *forged_plan.last_mut().unwrap() ^= 1;

        let generator = ParametricIbpGenerator::try_new_with_config(
            artifact.family(),
            ParametricIbpConfig::default(),
        )
        .unwrap();
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len())
            .map(|ordinal| prepared.generate(ordinal))
            .collect();
        let completed = prepared.complete(rows).unwrap();
        let parent = Reader::root(&[], ArtifactLoadLimits::default()).unwrap();
        assert!(matches!(
            decode_cell(
                &parent,
                &forged_plan,
                &generator,
                &completed,
                artifact.canonicalizer().unwrap(),
                artifact.zero_sectors(),
                artifact.ordering(),
                TranslatedSourceLimits::default(),
                ParametricRuleLimits::default(),
                RuleCellLimits::default(),
            ),
            Err(ArtifactPersistenceError::SemanticMismatch {
                field: "K6 rule-cell replay witness",
            })
        ));
    }
}
