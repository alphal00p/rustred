mod census;
mod model;

use rustred::campaign::{ParallelExecution, ParallelExecutionError};
use rustred::identity::{
    CompletedIbpSourceRows, ParametricIbpError, ParametricIbpGenerator, ParametricRelation,
    PreparedIbpSourceBatch,
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
    admitted_results: usize,
    failure_context: &'static str,
) -> Result<CompletedIbpSourceRows, AppError> {
    validate_exact_batch_count(failure_context, batch.len(), admitted_results)?;
    let rows = execution
        .map_ordered(admitted_results, |ordinal| batch.generate(ordinal))
        .map_err(parallel_execution_error)?;
    batch
        .complete(rows)
        .map_err(|error| AppError::derivation(format!("{failure_context}: {error}")))
}

fn execute_ordinary_batch(
    generator: &ParametricIbpGenerator<'_>,
    execution: &ParallelExecution,
    admitted_results: usize,
    failure_context: &'static str,
) -> Result<CompletedIbpSourceRows, AppError> {
    let batch = generator
        .prepare_ordinary_ibp()
        .map_err(|error| AppError::derivation(format!("{failure_context}: {error}")))?;
    execute_source_batch(batch, execution, admitted_results, failure_context)
}

fn execute_li_batch(
    generator: &ParametricIbpGenerator<'_>,
    execution: &ParallelExecution,
    sources: &CompletedIbpSourceRows,
    admitted_results: usize,
    failure_context: &'static str,
) -> Result<Vec<ParametricRelation>, AppError> {
    let batch = generator
        .prepare_lorentz_invariance(sources)
        .map_err(|error| AppError::derivation(format!("{failure_context}: {error}")))?;
    validate_exact_batch_count(failure_context, batch.len(), admitted_results)?;
    let rows = execution
        .map_ordered(admitted_results, |ordinal| batch.generate(ordinal))
        .map_err(parallel_execution_error)?;
    rows.into_iter()
        .collect::<Result<Vec<_>, ParametricIbpError>>()
        .map_err(|error| AppError::derivation(format!("{failure_context}: {error}")))
}

fn execute_external_source_batch(
    generator: &ParametricIbpGenerator<'_>,
    execution: &ParallelExecution,
    admitted_results: usize,
    failure_context: &'static str,
) -> Result<CompletedIbpSourceRows, AppError> {
    let batch = generator
        .prepare_external_ibp_sources()
        .map_err(|error| AppError::derivation(format!("{failure_context}: {error}")))?;
    execute_source_batch(batch, execution, admitted_results, failure_context)
}

fn parallel_execution_error(error: ParallelExecutionError) -> AppError {
    match error {
        error @ ParallelExecutionError::MulticoreRequiresSymbolicaLicense { .. } => {
            AppError::license(error.to_string())
        }
        error @ ParallelExecutionError::OrderedResultCeilingExceeded { .. } => {
            AppError::internal_invariant(error.to_string())
        }
        error => AppError::execution(error.to_string()),
    }
}

fn validate_exact_batch_count(
    batch: &'static str,
    actual_results: usize,
    admitted_results: usize,
) -> Result<(), AppError> {
    if actual_results != admitted_results {
        return Err(AppError::internal_invariant(format!(
            "{batch} prepared {actual_results} ordered results but structural preflight admitted exactly {admitted_results}"
        )));
    }
    Ok(())
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
    let structure_census = preflight_derivation_structure(family, selection)?;
    let generator = ParametricIbpGenerator::try_new(&family).map_err(|error| {
        AppError::derivation(format!("cannot initialize IBP generation: {error}"))
    })?;
    // The family, complete Symbolica variable map, and every structural
    // preflight are fixed on the coordinator before any worker is created.
    // One execution object is then reused by generation and rendering.
    let execution = ParallelExecution::try_new(n_cores, structure_census.ordered_result_ceiling())
        .map_err(parallel_execution_error)?;
    let (ordinary, li) = match selection {
        RelationSelection::All => {
            let ordinary = execute_ordinary_batch(
                &generator,
                &execution,
                structure_census.ordinary_source_rows,
                "cannot generate parametric IBP/LI rows",
            )?;
            // LI rows are exact linear combinations of external-contraction
            // ordinary rows, so the complete ordered ordinary phase is a
            // barrier before this second ordinal batch is prepared.
            let li = execute_li_batch(
                &generator,
                &execution,
                &ordinary,
                structure_census.lorentz_invariance_rows,
                "cannot generate parametric IBP/LI rows",
            )?;
            (ordinary.into_relations(), li)
        }
        RelationSelection::Ordinary => {
            let ordinary = execute_ordinary_batch(
                &generator,
                &execution,
                structure_census.ordinary_source_rows,
                "cannot generate parametric IBP rows",
            )?;
            (ordinary.into_relations(), Vec::new())
        }
        RelationSelection::LorentzInvariance => {
            if family.external_count() < 2 {
                (Vec::new(), Vec::new())
            } else {
                let sources = execute_external_source_batch(
                    &generator,
                    &execution,
                    structure_census.external_source_rows,
                    "cannot generate parametric LI rows",
                )?;
                let li = execute_li_batch(
                    &generator,
                    &execution,
                    &sources,
                    structure_census.lorentz_invariance_rows,
                    "cannot generate parametric LI rows",
                )?;
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
    validate_exact_batch_count(
        "relation rendering",
        relation_count,
        structure_census.emitted_relation_rows,
    )?;
    let relations = execution
        .map_ordered(structure_census.emitted_relation_rows, |ordinal| {
            let relation = if ordinal < ordinary.len() {
                &ordinary[ordinal]
            } else {
                &li[ordinal - ordinary.len()]
            };
            relation_output(ordinal, relation)
        })
        .map_err(parallel_execution_error)?;
    debug_assert_eq!(relations.len(), relation_count);
    let coordinates = coordinate_outputs(&family);
    let denominators = denominator_outputs(family, lowered.denominators());
    let external_gram = external_gram_outputs(family);
    let domain_conditions = family_domain_outputs(family);
    let family_output = FamilyOutputV1 {
        name: family.name().to_owned(),
        fingerprint: family.fingerprint().to_owned(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::error::AppErrorKind;

    #[test]
    fn exact_batch_admission_accepts_the_censused_count() {
        assert_eq!(validate_exact_batch_count("ordinary rows", 8, 8), Ok(()));
    }

    #[test]
    fn exact_batch_admission_rejects_one_below_the_censused_count() {
        let error = validate_exact_batch_count("ordinary rows", 7, 8).unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::InternalInvariant);
        assert_eq!(
            error.message(),
            "ordinary rows prepared 7 ordered results but structural preflight admitted exactly 8"
        );
    }
}
