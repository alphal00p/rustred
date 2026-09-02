//! Deterministic durable ownership for sealed closing artifacts.
//!
//! Schema v4 persists exact family constructor inputs, one explicit ordering
//! authority, tagged derivation
//! plans with complete semantic witnesses, rule plans, and terminals. Loading
//! first bounds every byte-level shape, independently regenerates the tagged
//! canonical ordinary source plan, compares the full retained semantics, then
//! derives/replays rules and invokes the closing installer exactly once.

mod binary;
mod coefficient;
mod limits;
mod semantic;
mod two_loop;

use std::collections::BTreeSet;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::foundry::parametric::derive_sector_interior_rule;
use crate::identity::{ParametricIbpGenerator, ParametricRelation};
use crate::sector::{Mask, OrderingPolicy};

use super::error::{ArtifactError, ArtifactPersistenceError};
use super::install::{ClosingArtifactCandidate, install};
use super::model::{
    ArtifactSchemaVersion, ClosedArtifact, CommonMassHomogeneityProof, ZeroSectorTerminal,
    ZeroTerminalProof,
};
use super::one_loop::ALGORITHM_ID as ONE_LOOP_ALGORITHM_ID;
use super::two_loop::{
    ALGORITHM_ID as TWO_LOOP_ALGORITHM_ID, derive_two_loop_unit_mass_sunset_with_limits,
};
use binary::{Reader, Writer, try_vec};
use coefficient::{decode_base_coefficient, encode_base_coefficient};
pub use limits::{ArtifactEncodingLimits, ArtifactLoadLimits};
use semantic::{
    decode_bool_vec, decode_i64_vec, decode_owned_string, encode_bool_slice,
    encode_condition_source, encode_i64_slice, encode_integral_key, encode_row_id,
    encode_rule_snapshot,
};

const MAGIC: &[u8; 8] = b"RRIBP\0\r\n";
const SECTION_COUNT: u32 = 5;
const METADATA_SECTION: u16 = 1;
const FAMILY_SECTION: u16 = 2;
const SOURCES_SECTION: u16 = 3;
const RULES_SECTION: u16 = 4;
const TERMINALS_SECTION: u16 = 5;

/// Generate every ordinary `L*(L+E)` source row in canonical ordinal order.
/// The plan applies unchanged to the future four-row and nine-row families.
/// Translated-source plans will receive a distinct tag and payload grammar.
const COMPLETE_ORDINARY_SOURCE_PLAN: u16 = 1;
const INTERIOR_FIRST_DESCENDING_RULE_PLAN: u16 = 1;
const REGISTERED_RULE_CELL_PLAN: u16 = 2;
const TWO_LOOP_CLOSURE_HEADER: u16 = 0x200;

fn write_section(
    output: &mut Writer,
    tag: u16,
    section: Writer,
) -> Result<(), ArtifactPersistenceError> {
    output.u16(tag)?;
    output.bytes(&section.finish(), "section bytes")
}

pub(super) fn encode(artifact: &ClosedArtifact) -> Result<Vec<u8>, ArtifactPersistenceError> {
    encode_with_limits(artifact, ArtifactEncodingLimits::default())
}

pub(super) fn encode_with_limits(
    artifact: &ClosedArtifact,
    limits: ArtifactEncodingLimits,
) -> Result<Vec<u8>, ArtifactPersistenceError> {
    let mut output = Writer::new(limits);
    encode_into_writer(artifact, &mut output)?;
    Ok(output.finish())
}

fn encode_into_writer(
    artifact: &ClosedArtifact,
    output: &mut Writer,
) -> Result<(), ArtifactPersistenceError> {
    if artifact.schema() != ArtifactSchemaVersion::CURRENT {
        return Err(ArtifactPersistenceError::UnsupportedSchema {
            actual: artifact.schema().as_u32(),
        });
    }
    if !matches!(
        artifact.algorithm_id(),
        ONE_LOOP_ALGORITHM_ID | TWO_LOOP_ALGORITHM_ID
    ) {
        return Err(ArtifactPersistenceError::UnsupportedFeature {
            detail: "schema-v4 has no registered durable rule-cell grammar for this closing algorithm",
        });
    }
    output.raw(MAGIC)?;
    output.u32(artifact.schema().as_u32())?;
    output.u32(SECTION_COUNT)?;

    let mut metadata = output.child();
    metadata.string(artifact.algorithm_id(), "algorithm identifier")?;
    metadata.usize(artifact.arity(), "artifact arity")?;
    metadata.string(artifact.family_fingerprint(), "family fingerprint")?;
    metadata.string(artifact.context_fingerprint(), "context fingerprint")?;
    metadata.string(
        artifact.ordering().stable_id().as_str(),
        "artifact ordering identifier",
    )?;
    write_section(output, METADATA_SECTION, metadata)?;

    let mut family = output.child();
    encode_family(&mut family, artifact.family())?;
    write_section(output, FAMILY_SECTION, family)?;

    let mut sources = output.child();
    sources.usize(1, "source derivation plans")?;
    let mut source_plan = sources.child();
    source_plan.usize(artifact.source_relations().len(), "ordinary source rows")?;
    let mut source_snapshot = source_plan.child();
    encode_source_snapshot(&mut source_snapshot, artifact.source_relations())?;
    let source_snapshot = source_snapshot.finish();
    source_plan.charge_witness_payload(source_snapshot.len())?;
    source_plan.bytes(&source_snapshot, "source semantic witness")?;
    sources.u16(COMPLETE_ORDINARY_SOURCE_PLAN)?;
    sources.bytes(&source_plan.finish(), "source-plan bytes")?;
    write_section(output, SOURCES_SECTION, sources)?;

    let mut rules = output.child();
    if artifact.algorithm_id() == TWO_LOOP_ALGORITHM_ID {
        two_loop::encode(&mut rules, artifact)?;
    } else {
        rules.usize(artifact.rules().len(), "artifact rules")?;
        for rule in artifact.rules() {
            // Schema-v4 derivation plan: deterministic first-descending
            // interior rule from the independently regenerated source set.
            let mut plan = rules.child();
            encode_integral_key(&mut plan, rule.anchor())?;
            plan.string(&rule.ordering().stable_id(), "rule ordering identifier")?;
            let snapshot = encode_rule_snapshot(rule, &rules)?;
            rules.charge_witness_payload(snapshot.len())?;
            plan.bytes(&snapshot, "rule snapshot bytes")?;
            rules.u16(INTERIOR_FIRST_DESCENDING_RULE_PLAN)?;
            rules.bytes(&plan.finish(), "rule-plan bytes")?;
        }
    }
    write_section(output, RULES_SECTION, rules)?;

    let mut terminals = output.child();
    encode_terminals(&mut terminals, artifact)?;
    write_section(output, TERMINALS_SECTION, terminals)?;
    Ok(())
}

struct EncodedSourcePlan<'input> {
    expected_rows: usize,
    semantic_witness: &'input [u8],
}

struct EncodedRulePlan<'input> {
    anchor: Vec<i64>,
    ordering: OrderingPolicy,
    semantic_witness: &'input [u8],
}

pub(super) fn decode(
    bytes: &[u8],
    limits: ArtifactLoadLimits,
) -> Result<ClosedArtifact, ArtifactPersistenceError> {
    let mut input = Reader::root(bytes, limits)?;
    if input.fixed(MAGIC.len())? != MAGIC {
        return Err(ArtifactPersistenceError::InvalidMagic);
    }
    let schema = input.u32()?;
    if schema != ArtifactSchemaVersion::CURRENT.as_u32() {
        return Err(ArtifactPersistenceError::UnsupportedSchema { actual: schema });
    }
    let section_count = input.u32()?;
    if section_count != SECTION_COUNT {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "section count",
        });
    }

    let metadata_bytes = input.section(METADATA_SECTION)?;
    let family_bytes = input.section(FAMILY_SECTION)?;
    let sources_bytes = input.section(SOURCES_SECTION)?;
    let rules_bytes = input.section(RULES_SECTION)?;
    let terminals_bytes = input.section(TERMINALS_SECTION)?;
    input.finish()?;

    let mut metadata = input.child(metadata_bytes);
    let algorithm = decode_owned_string(&mut metadata, "algorithm identifier")?;
    let algorithm_id = match algorithm.as_str() {
        ONE_LOOP_ALGORITHM_ID => ONE_LOOP_ALGORITHM_ID,
        TWO_LOOP_ALGORITHM_ID => TWO_LOOP_ALGORITHM_ID,
        _ => {
            return Err(ArtifactPersistenceError::UnsupportedFeature {
                detail: "unknown closing algorithm identifier",
            });
        }
    };
    let arity = metadata.count("artifact arity")?;
    if arity > limits.max_index_arity {
        return Err(ArtifactPersistenceError::ResourceLimit {
            resource: "artifact arity",
            requested: arity,
            limit: limits.max_index_arity,
        });
    }
    let expected_arity = if algorithm_id == ONE_LOOP_ALGORITHM_ID {
        1
    } else {
        3
    };
    if arity != expected_arity {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: if algorithm_id == ONE_LOOP_ALGORITHM_ID {
                "one-loop algorithm arity"
            } else {
                "two-loop algorithm arity"
            },
        });
    }
    let expected_family_fingerprint = decode_owned_string(&mut metadata, "family fingerprint")?;
    let expected_context_fingerprint = decode_owned_string(&mut metadata, "context fingerprint")?;
    let ordering =
        OrderingPolicy::try_from_stable_id(metadata.string("artifact ordering identifier")?)
            .map_err(ArtifactError::from)?;
    ordering.require_arity(arity).map_err(ArtifactError::from)?;
    metadata.finish()?;

    if algorithm_id == TWO_LOOP_ALGORITHM_ID {
        // K=3 schema-v4 owns complete cell/projection/factorization snapshots,
        // including its installer-compiled typed master-product embeddings.
        // At this untrusted boundary only, regenerate the registered exact
        // foundry plan and compare its complete deterministic encoding. The
        // returned sealed artifact is then reused without authentication or
        // foundry work in reducer hot paths.
        let artifact = derive_two_loop_unit_mass_sunset_with_limits(
            limits.family,
            limits.source_generation,
            limits.rule_derivation,
        )
        .map_err(ArtifactPersistenceError::from)?;
        if artifact.family_fingerprint() != expected_family_fingerprint
            || artifact.context_fingerprint() != expected_context_fingerprint
            || artifact.ordering() != ordering
        {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "two-loop metadata witness",
            });
        }
        let regenerated = encode_with_limits(&artifact, limits.replay_encoding())?;
        if regenerated.as_slice() != bytes {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "two-loop complete artifact witness",
            });
        }
        return Ok(artifact);
    }

    // Parse and globally charge all opaque coefficient-bearing witnesses
    // before family reconstruction or source/rule native work.
    let source_plan = decode_source_plan(&input, sources_bytes, arity)?;
    let encoded_rule_plans = decode_rule_plans(&input, rules_bytes, arity)?;

    let mut family_reader = input.child(family_bytes);
    let family = decode_one_loop_family(&mut family_reader)?;
    family_reader.finish()?;
    if family.denominator_count() != arity || family.fingerprint() != expected_family_fingerprint {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "family fingerprint or arity",
        });
    }

    let generator = ParametricIbpGenerator::try_new_with_config(&family, limits.source_generation)
        .map_err(ArtifactError::from)?;
    if generator.context().fingerprint() != expected_context_fingerprint {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "indexed coefficient context",
        });
    }
    let batch = generator
        .prepare_ordinary_ibp()
        .map_err(ArtifactError::from)?;
    if batch.len() != source_plan.expected_rows {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "ordinary source-plan row count",
        });
    }
    let mut rows = try_vec(batch.len(), "regenerated ordinary source rows")?;
    for ordinal in 0..batch.len() {
        rows.push(batch.generate(ordinal));
    }
    let source_relations = batch
        .complete(rows)
        .map_err(ArtifactError::from)?
        .into_relations();

    // One replay writer shares its aggregate encoding budget across the
    // independently regenerated source witness and every derived rule.
    let replay_writer = input.replay_writer();
    let mut regenerated_sources = replay_writer.child();
    encode_source_snapshot(&mut regenerated_sources, &source_relations)?;
    if regenerated_sources.finish().as_slice() != source_plan.semantic_witness {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "source derivation witness",
        });
    }

    let mut rules = try_vec(encoded_rule_plans.len(), "artifact rules")?;
    for encoded in encoded_rule_plans {
        let rule = derive_sector_interior_rule(
            generator.context(),
            &source_relations,
            &encoded.anchor,
            encoded.ordering,
            limits.rule_derivation,
        )
        .map_err(ArtifactError::from)?;
        if encode_rule_snapshot(&rule, &replay_writer)?.as_slice() != encoded.semantic_witness {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "rule snapshot",
            });
        }
        rules.push(rule);
    }
    let context = generator.context().clone();
    drop(generator);

    let mut terminal_reader = input.child(terminals_bytes);
    let decoded_terminals = decode_terminals(&mut terminal_reader, arity)?;
    terminal_reader.finish()?;

    install(ClosingArtifactCandidate {
        schema: ArtifactSchemaVersion::CURRENT,
        algorithm_id,
        arity,
        ordering,
        supported_root_power_bounds: vec![crate::sector::InteriorBounds::new(i64::MIN, i64::MAX)]
            .into_boxed_slice(),
        family,
        context,
        source_relations,
        rules,
        rule_cells: Vec::new(),
        canonicalizer: None,
        dependencies: Vec::new(),
        factorization_rules: Vec::new(),
        masters: decoded_terminals.masters,
        zero_sectors: decoded_terminals.zero_sectors,
        common_mass_homogeneity: decoded_terminals.common_mass_homogeneity,
    })
    .map_err(ArtifactPersistenceError::from)
}

fn decode_source_plan<'input>(
    parent: &Reader<'input>,
    bytes: &'input [u8],
    arity: usize,
) -> Result<EncodedSourcePlan<'input>, ArtifactPersistenceError> {
    let mut reader = parent.child(bytes);
    let plan_count = reader.count("source derivation plans")?;
    if plan_count != 1 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "source derivation plan count",
        });
    }
    let plan_tag = reader.u16()?;
    let plan_bytes = reader.bytes("source-plan bytes", parent.limits().max_artifact_bytes)?;
    if plan_tag != COMPLETE_ORDINARY_SOURCE_PLAN {
        return Err(ArtifactPersistenceError::UnsupportedFeature {
            detail: "unknown source derivation plan",
        });
    }
    let mut plan = reader.child(plan_bytes);
    let expected_rows = plan.count("ordinary source rows")?;
    if arity == 1 && expected_rows != 1 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "one-loop ordinary source count",
        });
    }
    let semantic_witness = plan.bytes(
        "source semantic witness",
        parent.limits().max_artifact_bytes,
    )?;
    plan.charge_witness_payload(semantic_witness.len())?;
    plan.finish()?;
    reader.finish()?;
    Ok(EncodedSourcePlan {
        expected_rows,
        semantic_witness,
    })
}

fn decode_rule_plans<'input>(
    parent: &Reader<'input>,
    bytes: &'input [u8],
    arity: usize,
) -> Result<Vec<EncodedRulePlan<'input>>, ArtifactPersistenceError> {
    let mut reader = parent.child(bytes);
    let rule_count = reader.count("artifact rules")?;
    if arity == 1 && rule_count != 1 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "one-loop rule count",
        });
    }
    let mut plans = try_vec(rule_count, "artifact rules")?;
    for _ in 0..rule_count {
        let plan_tag = reader.u16()?;
        let plan_bytes = reader.bytes("rule-plan bytes", parent.limits().max_artifact_bytes)?;
        if plan_tag != INTERIOR_FIRST_DESCENDING_RULE_PLAN {
            return Err(ArtifactPersistenceError::UnsupportedFeature {
                detail: "unknown rule derivation plan",
            });
        }
        let mut plan = reader.child(plan_bytes);
        let anchor_len = plan.count("rule anchor")?;
        if anchor_len != arity {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "rule anchor arity",
            });
        }
        let mut anchor = try_vec(anchor_len, "rule anchor")?;
        for _ in 0..anchor_len {
            anchor.push(plan.i64()?);
        }
        let ordering = OrderingPolicy::try_from_stable_id(plan.string("rule ordering identifier")?)
            .map_err(ArtifactError::from)?;
        let semantic_witness =
            plan.bytes("rule snapshot bytes", parent.limits().max_artifact_bytes)?;
        plan.charge_witness_payload(semantic_witness.len())?;
        plan.finish()?;
        plans.push(EncodedRulePlan {
            anchor,
            ordering,
            semantic_witness,
        });
    }
    reader.finish()?;
    Ok(plans)
}

fn encode_family(
    writer: &mut Writer,
    family: &IntegralFamily,
) -> Result<(), ArtifactPersistenceError> {
    // Structural prelude: the decoder can reject an algorithm-incompatible
    // shape before allocating labels or constructing one Symbolica value.
    writer.usize(family.loop_momenta().len(), "loop momentum labels")?;
    writer.usize(family.external_momenta().len(), "external momentum labels")?;
    writer.usize(
        family.coefficient_context().parameter_names().len(),
        "coefficient parameter names",
    )?;
    writer.usize(family.denominators().len(), "family denominators")?;
    writer.usize(family.external_gram().len(), "external Gram rows")?;
    writer.usize(family.power_shifts().len(), "family power shifts")?;
    for denominator in family.denominators() {
        writer.usize(denominator.coefficients().len(), "denominator coefficients")?;
    }
    for row in family.external_gram() {
        writer.usize(row.len(), "external Gram columns")?;
    }

    writer.string(family.name(), "family name")?;
    encode_strings_without_count(writer, family.loop_momenta(), "loop momentum labels")?;
    encode_strings_without_count(
        writer,
        family.external_momenta(),
        "external momentum labels",
    )?;
    encode_strings_without_count(
        writer,
        family.coefficient_context().parameter_names(),
        "coefficient parameter names",
    )?;
    encode_base_coefficient(writer, family.dimension())?;
    for denominator in family.denominators() {
        encode_base_coefficient(writer, denominator.constant())?;
        for coefficient in denominator.coefficients() {
            encode_base_coefficient(writer, coefficient)?;
        }
    }
    for row in family.external_gram() {
        for coefficient in row {
            encode_base_coefficient(writer, coefficient)?;
        }
    }
    for shift in family.power_shifts() {
        encode_base_coefficient(writer, shift)?;
    }
    Ok(())
}

fn encode_strings_without_count(
    writer: &mut Writer,
    values: &[String],
    resource: &'static str,
) -> Result<(), ArtifactPersistenceError> {
    for value in values {
        writer.string(value, resource)?;
    }
    Ok(())
}

fn decode_strings(
    reader: &mut Reader<'_>,
    count: usize,
    resource: &'static str,
) -> Result<Vec<String>, ArtifactPersistenceError> {
    let mut values = try_vec(count, resource)?;
    for _ in 0..count {
        values.push(decode_owned_string(reader, resource)?);
    }
    Ok(values)
}

fn decode_one_loop_family(
    reader: &mut Reader<'_>,
) -> Result<IntegralFamily, ArtifactPersistenceError> {
    let loop_count = reader.count("loop momentum labels")?;
    let external_count = reader.count("external momentum labels")?;
    let parameter_count = reader.count("coefficient parameter names")?;
    let denominator_count = reader.count("family denominators")?;
    let gram_rows = reader.count("external Gram rows")?;
    let power_shift_count = reader.count("family power shifts")?;
    if (
        loop_count,
        external_count,
        parameter_count,
        denominator_count,
        gram_rows,
        power_shift_count,
    ) != (1, 0, 1, 1, 0, 1)
    {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "one-loop family structural prelude",
        });
    }
    let denominator_coefficient_count = reader.count("denominator coefficients")?;
    if denominator_coefficient_count != 1 {
        return Err(ArtifactPersistenceError::SemanticMismatch {
            field: "one-loop denominator shape",
        });
    }

    let name = decode_owned_string(reader, "family name")?;
    let loop_momenta = decode_strings(reader, loop_count, "loop momentum labels")?;
    let external_momenta = decode_strings(reader, external_count, "external momentum labels")?;
    let parameter_names = decode_strings(reader, parameter_count, "coefficient parameter names")?;
    let coefficient_context =
        CoefficientContext::try_new(parameter_names).map_err(ArtifactError::from)?;
    let dimension = decode_base_coefficient(reader, &coefficient_context, "family dimension")?;
    let constant = decode_base_coefficient(reader, &coefficient_context, "denominator constant")?;
    let coefficient =
        decode_base_coefficient(reader, &coefficient_context, "denominator coefficient")?;
    let power_shift = decode_base_coefficient(reader, &coefficient_context, "family power shift")?;
    IntegralFamily::new_with_limits(
        name,
        loop_momenta,
        external_momenta,
        coefficient_context,
        dimension,
        vec![AffineDenominator::new(constant, vec![coefficient])],
        Vec::new(),
        vec![power_shift],
        reader.limits().family,
    )
    .map_err(ArtifactError::from)
    .map_err(ArtifactPersistenceError::from)
}

fn encode_source_snapshot(
    writer: &mut Writer,
    sources: &[ParametricRelation],
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(sources.len(), "source relations")?;
    for source in sources {
        encode_row_id(writer, source.row_id())?;
        writer.usize(source.terms().len(), "source relation terms")?;
        for (shift, coefficient) in source.terms() {
            encode_i64_slice(writer, shift.values())?;
            coefficient::encode_indexed_coefficient(writer, coefficient)?;
        }
        writer.usize(
            source.nonzero_conditions().len(),
            "source relation conditions",
        )?;
        for condition in source.nonzero_conditions() {
            coefficient::encode_indexed_polynomial(writer, condition.polynomial())?;
            writer.usize(condition.sources().len(), "condition provenance sources")?;
            for provenance in condition.sources() {
                encode_condition_source(writer, provenance)?;
            }
        }
    }
    Ok(())
}

fn encode_terminals(
    writer: &mut Writer,
    artifact: &ClosedArtifact,
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(artifact.masters().len(), "master terminals")?;
    for master in artifact.masters() {
        encode_integral_key(writer, master)?;
    }
    writer.usize(artifact.zero_sectors().len(), "zero-sector terminals")?;
    for terminal in artifact.zero_sectors() {
        encode_bool_slice(writer, terminal.sector().active_bits())?;
        writer.u8(match terminal.proof() {
            ZeroTerminalProof::ScalelessVacuumPolynomial => 0,
            ZeroTerminalProof::LeePomeranskyRankDeficiency => 1,
        })?;
    }
    writer.u8(match artifact.common_mass_homogeneity() {
        None => 0,
        Some(CommonMassHomogeneityProof::UniformVacuumMassSquared) => 1,
    })
}

struct DecodedTerminals {
    masters: BTreeSet<IntegralKey>,
    zero_sectors: Vec<ZeroSectorTerminal>,
    common_mass_homogeneity: Option<CommonMassHomogeneityProof>,
}

fn decode_terminals(
    reader: &mut Reader<'_>,
    arity: usize,
) -> Result<DecodedTerminals, ArtifactPersistenceError> {
    let master_count = reader.count("master terminals")?;
    let mut masters = BTreeSet::new();
    for _ in 0..master_count {
        let powers = decode_i64_vec(reader, "master powers")?;
        if powers.len() != arity {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "master arity",
            });
        }
        let key = IntegralKey::try_new(powers).map_err(ArtifactError::from)?;
        if !masters.insert(key) {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "duplicate master terminal",
            });
        }
    }
    let zero_count = reader.count("zero-sector terminals")?;
    let mut zero_sectors = try_vec(zero_count, "zero-sector terminals")?;
    for _ in 0..zero_count {
        let active = decode_bool_vec(reader, "zero-sector mask")?;
        if active.len() != arity {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "zero-sector arity",
            });
        }
        let sector = Mask::try_new(active).map_err(ArtifactError::from)?;
        let proof = match reader.u8()? {
            0 => ZeroTerminalProof::ScalelessVacuumPolynomial,
            1 => ZeroTerminalProof::LeePomeranskyRankDeficiency,
            _ => {
                return Err(ArtifactPersistenceError::UnsupportedFeature {
                    detail: "unknown zero-terminal proof",
                });
            }
        };
        zero_sectors.push(ZeroSectorTerminal::new(sector, proof));
    }
    let common_mass_homogeneity = match reader.u8()? {
        0 => None,
        1 => Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
        _ => {
            return Err(ArtifactPersistenceError::UnsupportedFeature {
                detail: "unknown homogeneity proof",
            });
        }
    };
    Ok(DecodedTerminals {
        masters,
        zero_sectors,
        common_mass_homogeneity,
    })
}

#[cfg(test)]
mod aggregate_budget_tests {
    use super::*;
    use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;

    #[test]
    fn nested_documents_share_the_root_aggregate_coefficient_budget() {
        let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
        let mut lower = 0usize;
        let mut upper = ArtifactEncodingLimits::default().max_total_coefficient_bytes;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if encode_with_limits(
                &artifact,
                ArtifactEncodingLimits {
                    max_total_coefficient_bytes: middle,
                    ..ArtifactEncodingLimits::default()
                },
            )
            .is_ok()
            {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(lower > 0);

        let root = Writer::new(ArtifactEncodingLimits {
            max_total_coefficient_bytes: lower,
            ..ArtifactEncodingLimits::default()
        });
        let mut first = root.child();
        encode_into_writer(&artifact, &mut first).unwrap();
        let mut second = root.child();
        assert!(matches!(
            encode_into_writer(&artifact, &mut second),
            Err(ArtifactPersistenceError::ResourceLimit {
                resource: "aggregate coefficient bytes",
                ..
            })
        ));
    }

    #[test]
    fn unregistered_k6_rule_cell_grammar_fails_before_emitting_v4_bytes() {
        let mut artifact = derive_one_loop_unit_mass_tadpole().unwrap();
        artifact.algorithm_id = super::super::three_loop::ALGORITHM_ID;
        assert_eq!(
            encode(&artifact).unwrap_err(),
            ArtifactPersistenceError::UnsupportedFeature {
                detail: "schema-v4 has no registered durable rule-cell grammar for this closing algorithm",
            }
        );
    }
}
