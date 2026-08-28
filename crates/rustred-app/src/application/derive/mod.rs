mod census;
mod model;

use rustred::parametric_ibp::{CompletedIbpSourceRows, PreparedIbpSourceBatch};
use rustred::{
    ParallelExecution, ParallelExecutionError, ParametricIbpError, ParametricIbpGenerator,
    ParametricRelation,
};
use symbolica::prelude::AtomCore;

use super::error::AppError;
use super::model::LoweredProject;
use super::options::{InputFormat, RelationSelection};
use super::producer::ProducerOutputV1;
use super::{DeriveRequest, DeriveResult, MAX_OUTPUT_BYTES};
use census::{
    derivation_bound_overflow, preflight_derivation_structure, preflight_family_payload,
    preflight_generated_relations, preflight_output_bound,
};
use model::{
    DeriveOutputV1, FamilyOutputV1, ProvenanceOutputV1, RelationCountsOutputV1, coordinate_outputs,
    denominator_outputs, external_gram_outputs, family_domain_outputs, relation_output,
    target_output,
};

pub(crate) const OUTPUT_SCHEMA: &str = "rustred.derive-output.toml.v1";
const EQUATION_CONVENTION: &str =
    "sum(term.coefficient * I(n + term.shift) for term in relation.terms) = 0";

pub(super) fn derive_request(request: DeriveRequest) -> Result<DeriveResult, AppError> {
    let prepared = super::input::prepare_input(&request.source, request.input_format)?;
    let lowered = super::lowering::lower_project(prepared)?;
    let output = build_output(
        lowered,
        request.input_format,
        request.relations,
        request.n_cores,
    )?;
    let serialized = serialize_output(&output)?;
    Ok(DeriveResult::new(OUTPUT_SCHEMA, "ok", serialized))
}

fn execute_source_batch(
    batch: PreparedIbpSourceBatch<'_, '_>,
    execution: &ParallelExecution,
) -> Result<CompletedIbpSourceRows, ParametricIbpError> {
    let rows = execution.map_ordered(batch.len(), |ordinal| batch.generate(ordinal));
    batch.complete(rows)
}

fn execute_ordinary_batch(
    generator: &ParametricIbpGenerator<'_>,
    execution: &ParallelExecution,
) -> Result<CompletedIbpSourceRows, ParametricIbpError> {
    execute_source_batch(generator.prepare_ordinary_ibp()?, execution)
}

fn execute_li_batch(
    generator: &ParametricIbpGenerator<'_>,
    execution: &ParallelExecution,
    sources: &CompletedIbpSourceRows,
) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
    let batch = generator.prepare_lorentz_invariance(sources)?;
    execution
        .map_ordered(batch.len(), |ordinal| batch.generate(ordinal))
        .into_iter()
        .collect()
}

fn execute_external_source_batch(
    generator: &ParametricIbpGenerator<'_>,
    execution: &ParallelExecution,
) -> Result<CompletedIbpSourceRows, ParametricIbpError> {
    execute_source_batch(generator.prepare_external_ibp_sources()?, execution)
}

fn build_output(
    project: LoweredProject,
    requested_input_format: InputFormat,
    selection: RelationSelection,
    n_cores: usize,
) -> Result<DeriveOutputV1, AppError> {
    let (input_form, input_schema, metadata, lowered) = project.into_parts();
    let normalized = lowered.normalized();
    let family = lowered.family();
    if lowered.denominators().len() != family.denominator_count() {
        return Err(AppError::derivation(format!(
            "lowering returned {} denominator records for a {}-denominator family",
            lowered.denominators().len(),
            family.denominator_count()
        )));
    }

    let payload_census = preflight_family_payload(normalized, family, lowered.denominators())?;
    preflight_derivation_structure(family, selection)?;
    let generator = ParametricIbpGenerator::try_new(&family).map_err(|error| {
        AppError::derivation(format!("cannot initialize IBP generation: {error}"))
    })?;
    // The family, complete Symbolica variable map, and every structural
    // preflight are fixed on the coordinator before any worker is created.
    // One execution object is then reused by generation and rendering.
    let execution = ParallelExecution::try_new(n_cores).map_err(|error| match error {
        error @ ParallelExecutionError::MulticoreRequiresSymbolicaLicense { .. } => {
            AppError::license(error.to_string())
        }
        error => AppError::execution(error.to_string()),
    })?;
    let (ordinary, li) = match selection {
        RelationSelection::All => {
            let ordinary = execute_ordinary_batch(&generator, &execution).map_err(|error| {
                AppError::derivation(format!("cannot generate parametric IBP/LI rows: {error}"))
            })?;
            // LI rows are exact linear combinations of external-contraction
            // ordinary rows, so the complete ordered ordinary phase is a
            // barrier before this second ordinal batch is prepared.
            let li = execute_li_batch(&generator, &execution, &ordinary).map_err(|error| {
                AppError::derivation(format!("cannot generate parametric IBP/LI rows: {error}"))
            })?;
            (ordinary.into_relations(), li)
        }
        RelationSelection::Ordinary => {
            let ordinary = execute_ordinary_batch(&generator, &execution).map_err(|error| {
                AppError::derivation(format!("cannot generate parametric IBP rows: {error}"))
            })?;
            (ordinary.into_relations(), Vec::new())
        }
        RelationSelection::LorentzInvariance => {
            if family.external_count() < 2 {
                (Vec::new(), Vec::new())
            } else {
                let sources =
                    execute_external_source_batch(&generator, &execution).map_err(|error| {
                        AppError::derivation(format!("cannot generate parametric LI rows: {error}"))
                    })?;
                let li = execute_li_batch(&generator, &execution, &sources).map_err(|error| {
                    AppError::derivation(format!("cannot generate parametric LI rows: {error}"))
                })?;
                (Vec::new(), li)
            }
        }
    };
    preflight_generated_relations(ordinary.iter().chain(&li), payload_census)?;

    // No Symbolica expression rendering occurs before both structural and
    // actual sparse-payload preflights have accepted the generated rows.
    let generated_ordinary = ordinary.len();
    let generated_li = li.len();
    let emitted_ordinary = generated_ordinary;
    let emitted_li = generated_li;
    let relation_count = generated_ordinary
        .checked_add(generated_li)
        .ok_or_else(derivation_bound_overflow)?;
    let relations = execution.map_ordered(relation_count, |ordinal| {
        let relation = if ordinal < ordinary.len() {
            &ordinary[ordinal]
        } else {
            &li[ordinal - ordinary.len()]
        };
        relation_output(ordinal, relation)
    });
    debug_assert_eq!(relations.len(), relation_count);
    let coordinates = coordinate_outputs(&family);
    let denominators = denominator_outputs(family, lowered.denominators());
    let external_gram = external_gram_outputs(family);
    let domain_conditions = family_domain_outputs(family);
    let family_output = FamilyOutputV1 {
        name: family.name().to_owned(),
        fingerprint: family.fingerprint(),
        parametric_context_fingerprint: generator.context().fingerprint().to_owned(),
        parameters: normalized.operational_parameter_names().to_vec(),
        dimension: family.dimension().to_expression().to_canonical_string(),
        loop_momenta: family.loop_momenta().to_vec(),
        external_momenta: family.external_momenta().to_vec(),
        denominator_count: family.denominator_count(),
        index_symbols: (0..family.denominator_count())
            .map(|position| format!("n{position}"))
            .collect(),
    };
    Ok(DeriveOutputV1 {
        schema: OUTPUT_SCHEMA,
        status: "ok",
        equation_convention: EQUATION_CONVENTION,
        relation_selection: selection.as_str(),
        target_disposition: "not_processed_by_derive",
        producer: ProducerOutputV1::current(),
        provenance: ProvenanceOutputV1 {
            requested_input_format: requested_input_format.as_str(),
            detected_input_form: input_form,
            input_schema,
            parameter_source: normalized.parameter_source().stable_id().to_owned(),
            input_parameters: normalized.parameter_names().to_vec(),
            canonical_integral: normalized.canonical_string(),
            metadata,
        },
        family: family_output,
        target: target_output(normalized),
        coordinates,
        denominators,
        external_gram,
        domain_conditions,
        relation_counts: RelationCountsOutputV1 {
            generated_ordinary,
            generated_li,
            emitted_ordinary,
            emitted_li,
            emitted_total: emitted_ordinary
                .checked_add(emitted_li)
                .ok_or_else(derivation_bound_overflow)?,
        },
        relations,
    })
}

fn serialize_output(output: &DeriveOutputV1) -> Result<String, AppError> {
    preflight_output_bound(output)?;
    let mut serialized = toml::to_string_pretty(output).map_err(|error| {
        AppError::serialization(format!(
            "cannot serialize deterministic TOML output: {error}"
        ))
    })?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    if serialized.len() > MAX_OUTPUT_BYTES {
        return Err(AppError::output_limit(format!(
            "TOML output needs {} bytes, exceeding the {MAX_OUTPUT_BYTES}-byte application limit",
            serialized.len()
        )));
    }
    Ok(serialized)
}
