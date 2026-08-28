use std::collections::{BTreeMap, BTreeSet};

use rustred::{
    CoefficientLocation, CoefficientPolynomial, GuardOrigin, IntegralFamily, ParallelExecution,
    ParallelExecutionError, ParametricIbpGenerator, ParametricNonZeroCondition, ParametricRelation,
    ParametricRowId, ScalarProductCoordinate,
};
use serde::Serialize;
use symbolica::prelude::{AtomCore, Integer};

use super::error::AppError;
use super::model::{LoweredProject, MetadataValue};
use super::options::{InputFormat, RelationSelection};
use super::producer::ProducerOutputV1;
use super::{DeriveRequest, DeriveResult, MAX_OUTPUT_BYTES};

pub(crate) const OUTPUT_SCHEMA: &str = "rustred.derive-output.toml.v1";
const MAX_DERIVATION_TERM_ATTEMPTS: usize = 2_000_000;
const PAYLOAD_NODE_BYTES: usize = 4_096;
const ATOM_RENDER_FACTOR: usize = 320;
const EXPONENT_RENDER_BYTES: usize = 320;
const INTEGER_STRUCTURAL_BYTES: usize = 64;
const EQUATION_CONVENTION: &str =
    "sum(term.coefficient * I(n + term.shift) for term in relation.terms) = 0";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct DeriveOutputV1 {
    schema: &'static str,
    status: &'static str,
    equation_convention: &'static str,
    relation_selection: &'static str,
    target_disposition: &'static str,
    producer: ProducerOutputV1,
    provenance: ProvenanceOutputV1,
    family: FamilyOutputV1,
    target: TargetOutputV1,
    coordinates: Vec<CoordinateOutputV1>,
    denominators: Vec<DenominatorOutputV1>,
    external_gram: Vec<ExternalGramOutputV1>,
    domain_conditions: Vec<ConditionOutputV1>,
    relation_counts: RelationCountsOutputV1,
    relations: Vec<RelationOutputV1>,
}

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

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ProvenanceOutputV1 {
    requested_input_format: &'static str,
    detected_input_form: &'static str,
    input_schema: String,
    parameter_source: String,
    input_parameters: Vec<String>,
    canonical_integral: String,
    metadata: BTreeMap<String, MetadataValue>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct FamilyOutputV1 {
    name: String,
    fingerprint: String,
    parametric_context_fingerprint: String,
    parameters: Vec<String>,
    dimension: String,
    loop_momenta: Vec<String>,
    external_momenta: Vec<String>,
    denominator_count: usize,
    index_symbols: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct TargetOutputV1 {
    present: bool,
    disposition: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    powers: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    numerator: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct CoordinateOutputV1 {
    ordinal: usize,
    kind: &'static str,
    left: String,
    right: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct DenominatorOutputV1 {
    ordinal: usize,
    id: String,
    source_expression: String,
    normalized_expression: String,
    power_shift: String,
    constant: String,
    coefficients: Vec<AffineCoefficientOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct AffineCoefficientOutputV1 {
    coordinate: usize,
    value: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ExternalGramOutputV1 {
    row: usize,
    column: usize,
    left: String,
    right: String,
    value: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct ConditionOutputV1 {
    expression: String,
    sources: Vec<String>,
    origins: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct RelationCountsOutputV1 {
    generated_ordinary: usize,
    generated_li: usize,
    emitted_ordinary: usize,
    emitted_li: usize,
    emitted_total: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct RelationOutputV1 {
    ordinal: usize,
    stable_id: String,
    id: RowIdOutputV1,
    terms: Vec<RelationTermOutputV1>,
    nonzero_conditions: Vec<ConditionOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct RowIdOutputV1 {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    contraction_momentum: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    differentiated_loop: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_external: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    second_external: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct RelationTermOutputV1 {
    shift: Vec<i64>,
    coefficient: String,
}

pub(crate) fn build_output(
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
            let generated = generator
                .generate_with_execution(&execution)
                .map_err(|error| {
                    AppError::derivation(format!("cannot generate parametric IBP/LI rows: {error}"))
                })?;
            let (_, ordinary, li) = generated.into_parts();
            (ordinary, li)
        }
        RelationSelection::Ordinary => (
            generator
                .generate_ordinary_ibp_with_execution(&execution)
                .map_err(|error| {
                    AppError::derivation(format!("cannot generate parametric IBP rows: {error}"))
                })?,
            Vec::new(),
        ),
        RelationSelection::LorentzInvariance => (
            Vec::new(),
            generator
                .generate_lorentz_invariance_with_execution(&execution)
                .map_err(|error| {
                    AppError::derivation(format!("cannot generate parametric LI rows: {error}"))
                })?,
        ),
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
    let relations: Vec<RelationOutputV1> = execution.map_ordered(relation_count, |ordinal| {
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

pub(crate) fn serialize_output(output: &DeriveOutputV1) -> Result<String, AppError> {
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

/// Bound the generator's raw addition work before it constructs a single
/// parametric coefficient. For one ordinary row there is at most one dimension
/// term and `N * (N + 1)` derivative-contraction attempts. One LI row combines
/// two external-contraction ordinary rows per loop through `N + 1` affine
/// weights. These are topology-independent worst-case counts; exact zeroes and
/// equal shifts can only reduce the actual retained support.
fn preflight_derivation_structure(
    family: &IntegralFamily,
    selection: RelationSelection,
) -> Result<(), AppError> {
    let loops = family.loop_count();
    let externals = family.external_count();
    let denominators = family.denominator_count();
    let contractions = loops
        .checked_add(externals)
        .ok_or_else(derivation_bound_overflow)?;
    let ordinary_rows = loops
        .checked_mul(contractions)
        .ok_or_else(derivation_bound_overflow)?;
    let external_predecessor = externals.saturating_sub(1);
    let li_rows = if externals % 2 == 0 {
        (externals / 2)
            .checked_mul(external_predecessor)
            .ok_or_else(derivation_bound_overflow)?
    } else {
        externals
            .checked_mul(external_predecessor / 2)
            .ok_or_else(derivation_bound_overflow)?
    };
    let denominator_successor = denominators
        .checked_add(1)
        .ok_or_else(derivation_bound_overflow)?;
    let ordinary_attempts_per_row = denominators
        .checked_mul(denominator_successor)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(derivation_bound_overflow)?;
    let ordinary_attempts = ordinary_rows
        .checked_mul(ordinary_attempts_per_row)
        .ok_or_else(derivation_bound_overflow)?;
    let li_attempts_per_row = loops
        .checked_mul(2)
        .and_then(|value| value.checked_mul(denominator_successor))
        .and_then(|value| value.checked_mul(ordinary_attempts_per_row))
        .ok_or_else(derivation_bound_overflow)?;
    let li_attempts = li_rows
        .checked_mul(li_attempts_per_row)
        .ok_or_else(derivation_bound_overflow)?;
    let requested_attempts = match selection {
        RelationSelection::Ordinary => ordinary_attempts,
        // LI construction derives the external-contraction ordinary rows as
        // authenticated sources even when those rows are not emitted.
        RelationSelection::All | RelationSelection::LorentzInvariance => ordinary_attempts
            .checked_add(li_attempts)
            .ok_or_else(derivation_bound_overflow)?,
    };
    if requested_attempts > MAX_DERIVATION_TERM_ATTEMPTS {
        return Err(AppError::limit(format!(
            "the selected generic derivation has a conservative {requested_attempts}-term-attempt bound (L={loops}, E={externals}, N={denominators}), exceeding the application limit {MAX_DERIVATION_TERM_ATTEMPTS}"
        )));
    }

    let selected_rows = match selection {
        RelationSelection::All => ordinary_rows
            .checked_add(li_rows)
            .ok_or_else(derivation_bound_overflow)?,
        RelationSelection::Ordinary => ordinary_rows,
        RelationSelection::LorentzInvariance => li_rows,
    };
    let minimum_render_bound = selected_rows
        .checked_mul(4_096)
        .ok_or_else(derivation_bound_overflow)?;
    if minimum_render_bound > MAX_OUTPUT_BYTES {
        return Err(AppError::output_limit(format!(
            "the selected derivation has {selected_rows} rows whose minimum conservative render bound is {minimum_render_bound} bytes, exceeding the {MAX_OUTPUT_BYTES}-byte application output limit"
        )));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default)]
struct GeneratedPayloadCensus {
    relations: usize,
    relation_terms: usize,
    guard_conditions: usize,
    guard_origins: usize,
    polynomial_terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
    retained_render_bound: usize,
}

/// Census every family payload which will later cross an Atom-to-string or
/// rational-polynomial-to-Atom boundary. `AtomView::get_byte_size` measures the
/// packed native expression; the factor covers fully-qualified 256-byte labels
/// plus canonical syntax for every packed unit without rendering anything.
fn preflight_family_payload(
    normalized: &rustred::NormalizedProjectInputV1,
    family: &IntegralFamily,
    records: &[rustred::LoweredSymbolicaDenominatorV1],
) -> Result<GeneratedPayloadCensus, AppError> {
    let mut census = GeneratedPayloadCensus::default();
    add_generated_payload_bound(&mut census, PAYLOAD_NODE_BYTES)?;
    census_atom_payload(
        normalized.canonical_atom().as_view().get_byte_size(),
        &mut census,
    )?;
    census_atom_payload(
        normalized.target().numerator().as_view().get_byte_size(),
        &mut census,
    )?;
    for record in records {
        census_atom_payload(record.source().as_view().get_byte_size(), &mut census)?;
        census_atom_payload(
            record.normalized_expression().as_view().get_byte_size(),
            &mut census,
        )?;
    }

    census_coefficient(family.dimension(), &mut census)?;
    for denominator in family.denominators() {
        census_coefficient(denominator.constant(), &mut census)?;
        for coefficient in denominator.coefficients() {
            census_coefficient(coefficient, &mut census)?;
        }
    }
    for row in family.external_gram() {
        for value in row {
            census_coefficient(value, &mut census)?;
        }
    }
    for shift in family.power_shifts() {
        census_coefficient(shift, &mut census)?;
    }
    for condition in family.domain().conditions() {
        census.guard_conditions = checked_census_add(census.guard_conditions, 1)?;
        census.guard_origins = checked_census_add(census.guard_origins, condition.origins().len())?;
        let origin_bytes = condition
            .origins()
            .len()
            .checked_mul(PAYLOAD_NODE_BYTES)
            .ok_or_else(derivation_bound_overflow)?;
        add_generated_payload_bound(
            &mut census,
            PAYLOAD_NODE_BYTES
                .checked_add(origin_bytes)
                .ok_or_else(derivation_bound_overflow)?,
        )?;
        census_polynomial(
            condition.polynomial(),
            &mut census,
            EXPONENT_RENDER_BYTES,
            INTEGER_STRUCTURAL_BYTES,
        )?;
    }
    Ok(census)
}

fn census_atom_payload(
    packed_byte_size: usize,
    census: &mut GeneratedPayloadCensus,
) -> Result<(), AppError> {
    let render_bound = packed_byte_size
        .checked_mul(ATOM_RENDER_FACTOR)
        .and_then(|value| value.checked_add(PAYLOAD_NODE_BYTES))
        .ok_or_else(derivation_bound_overflow)?;
    add_generated_payload_bound(census, render_bound)
}

fn census_coefficient(
    coefficient: &rustred::algebra::Coefficient,
    census: &mut GeneratedPayloadCensus,
) -> Result<(), AppError> {
    census_polynomial(
        &coefficient.numerator,
        census,
        EXPONENT_RENDER_BYTES,
        INTEGER_STRUCTURAL_BYTES,
    )?;
    census_polynomial(
        &coefficient.denominator,
        census,
        EXPONENT_RENDER_BYTES,
        INTEGER_STRUCTURAL_BYTES,
    )
}

/// Inspect the exact sparse rational-polynomial payload without calling
/// `to_expression` or allocating canonical strings. Dense exponent slots are
/// charged at a label-plus-syntax render allowance, while GMP values are
/// charged for both retained limbs and a conservative decimal rendering.
fn preflight_generated_relations<'a>(
    relations: impl Iterator<Item = &'a ParametricRelation>,
    mut census: GeneratedPayloadCensus,
) -> Result<(), AppError> {
    for relation in relations {
        census.relations = checked_census_add(census.relations, 1)?;
        add_generated_payload_bound(&mut census, PAYLOAD_NODE_BYTES)?;
        for (shift, coefficient) in relation.terms() {
            census.relation_terms = checked_census_add(census.relation_terms, 1)?;
            let shift_bytes = shift
                .values()
                .len()
                .checked_mul(32)
                .ok_or_else(derivation_bound_overflow)?;
            add_generated_payload_bound(
                &mut census,
                PAYLOAD_NODE_BYTES
                    .checked_add(shift_bytes)
                    .ok_or_else(derivation_bound_overflow)?,
            )?;
            census_polynomial(
                &coefficient.raw().numerator,
                &mut census,
                EXPONENT_RENDER_BYTES,
                INTEGER_STRUCTURAL_BYTES,
            )?;
            census_polynomial(
                &coefficient.raw().denominator,
                &mut census,
                EXPONENT_RENDER_BYTES,
                INTEGER_STRUCTURAL_BYTES,
            )?;
        }
        for condition in relation.guarded_nonzero_conditions() {
            census.guard_conditions = checked_census_add(census.guard_conditions, 1)?;
            census.guard_origins =
                checked_census_add(census.guard_origins, condition.origins().len())?;
            let origin_bytes = condition
                .origins()
                .len()
                .checked_mul(PAYLOAD_NODE_BYTES)
                .ok_or_else(derivation_bound_overflow)?;
            add_generated_payload_bound(
                &mut census,
                PAYLOAD_NODE_BYTES
                    .checked_add(origin_bytes)
                    .ok_or_else(derivation_bound_overflow)?,
            )?;
            census_polynomial(
                condition.polynomial().raw(),
                &mut census,
                EXPONENT_RENDER_BYTES,
                INTEGER_STRUCTURAL_BYTES,
            )?;
        }
    }
    Ok(())
}

fn census_polynomial(
    polynomial: &CoefficientPolynomial,
    census: &mut GeneratedPayloadCensus,
    exponent_render_bytes: usize,
    integer_structural_bytes: usize,
) -> Result<(), AppError> {
    let terms = polynomial.nterms();
    census.polynomial_terms = checked_census_add(census.polynomial_terms, terms)?;
    census.exponent_entries =
        checked_census_add(census.exponent_entries, polynomial.exponents.len())?;
    let exponent_bytes = polynomial
        .exponents
        .len()
        .checked_mul(
            std::mem::size_of::<u16>()
                .checked_add(exponent_render_bytes)
                .ok_or_else(derivation_bound_overflow)?,
        )
        .ok_or_else(derivation_bound_overflow)?;
    let integer_slots = polynomial
        .coefficients
        .len()
        .checked_mul(
            std::mem::size_of::<Integer>()
                .checked_add(integer_structural_bytes)
                .ok_or_else(derivation_bound_overflow)?,
        )
        .ok_or_else(derivation_bound_overflow)?;
    add_generated_payload_bound(
        census,
        512usize
            .checked_add(exponent_bytes)
            .and_then(|value| value.checked_add(integer_slots))
            .ok_or_else(derivation_bound_overflow)?,
    )?;
    for coefficient in &polynomial.coefficients {
        let bits = integer_significant_bits(coefficient).ok_or_else(derivation_bound_overflow)?;
        census.integer_bits = checked_census_add(census.integer_bits, bits)?;
        // Retained magnitude needs ceil(bits/8); decimal rendering including a
        // sign is always shorter than bits+2 bytes for nonzero integers.
        let magnitude_bytes = bits
            .checked_add(7)
            .and_then(|value| value.checked_div(8))
            .ok_or_else(derivation_bound_overflow)?;
        let render_bytes = bits
            .max(1)
            .checked_add(2)
            .ok_or_else(derivation_bound_overflow)?;
        add_generated_payload_bound(
            census,
            magnitude_bytes
                .checked_add(render_bytes)
                .ok_or_else(derivation_bound_overflow)?,
        )?;
    }
    Ok(())
}

fn checked_census_add(left: usize, right: usize) -> Result<usize, AppError> {
    left.checked_add(right)
        .ok_or_else(derivation_bound_overflow)
}

fn integer_significant_bits(value: &Integer) -> Option<usize> {
    let bits = match value {
        Integer::Single(value) => u64::BITS - value.unsigned_abs().leading_zeros(),
        Integer::Double(value) => u128::BITS - value.unsigned_abs().leading_zeros(),
        Integer::Large(value) => return usize::try_from(value.significant_bits()).ok(),
    };
    usize::try_from(bits).ok()
}

fn add_generated_payload_bound(
    census: &mut GeneratedPayloadCensus,
    additional: usize,
) -> Result<(), AppError> {
    census.retained_render_bound = census
        .retained_render_bound
        .checked_add(additional)
        .ok_or_else(derivation_bound_overflow)?;
    if census.retained_render_bound > MAX_OUTPUT_BYTES {
        return Err(AppError::output_limit(format!(
            "application algebraic output payload has a conservative {}-byte retained/render bound after {} relations, {} terms, {} polynomial terms, {} exponent entries, {} integer bits, {} guard conditions, and {} guard origins; the application limit is {MAX_OUTPUT_BYTES} bytes",
            census.retained_render_bound,
            census.relations,
            census.relation_terms,
            census.polynomial_terms,
            census.exponent_entries,
            census.integer_bits,
            census.guard_conditions,
            census.guard_origins,
        )));
    }
    Ok(())
}

fn derivation_bound_overflow() -> AppError {
    AppError::limit("application derivation resource accounting overflowed usize".to_owned())
}

fn target_output(normalized: &rustred::NormalizedProjectInputV1) -> TargetOutputV1 {
    let target = normalized.target();
    TargetOutputV1 {
        present: true,
        disposition: target.derive_disposition(),
        powers: Some(target.powers().to_vec()),
        numerator: Some(target.numerator().to_canonical_string()),
    }
}

fn coordinate_outputs(family: &IntegralFamily) -> Vec<CoordinateOutputV1> {
    family
        .coordinates()
        .iter()
        .copied()
        .enumerate()
        .map(|(ordinal, coordinate)| match coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                let left_label = family.loop_momenta()[left].clone();
                let right_label = family.loop_momenta()[right].clone();
                CoordinateOutputV1 {
                    ordinal,
                    kind: "loop_loop",
                    left: left_label,
                    right: right_label,
                }
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => {
                let left_label = family.loop_momenta()[loop_index].clone();
                let right_label = family.external_momenta()[external_index].clone();
                CoordinateOutputV1 {
                    ordinal,
                    kind: "loop_external",
                    left: left_label,
                    right: right_label,
                }
            }
        })
        .collect()
}

fn denominator_outputs(
    family: &IntegralFamily,
    records: &[rustred::LoweredSymbolicaDenominatorV1],
) -> Vec<DenominatorOutputV1> {
    records
        .iter()
        .zip(family.denominators())
        .zip(family.power_shifts())
        .enumerate()
        .map(
            |(ordinal, ((record, denominator), power_shift))| DenominatorOutputV1 {
                ordinal,
                id: record.id().to_owned(),
                source_expression: record.source().to_canonical_string(),
                normalized_expression: record.normalized_expression().to_canonical_string(),
                power_shift: power_shift.to_expression().to_canonical_string(),
                constant: denominator.constant().to_expression().to_canonical_string(),
                coefficients: denominator
                    .coefficients()
                    .iter()
                    .enumerate()
                    .map(|(coordinate, coefficient)| AffineCoefficientOutputV1 {
                        coordinate,
                        value: coefficient.to_expression().to_canonical_string(),
                    })
                    .collect(),
            },
        )
        .collect()
}

fn external_gram_outputs(family: &IntegralFamily) -> Vec<ExternalGramOutputV1> {
    let mut output = Vec::new();
    for (row, values) in family.external_gram().iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            output.push(ExternalGramOutputV1 {
                row,
                column,
                left: family.external_momenta()[row].clone(),
                right: family.external_momenta()[column].clone(),
                value: value.to_expression().to_canonical_string(),
            });
        }
    }
    output
}

fn family_domain_outputs(family: &IntegralFamily) -> Vec<ConditionOutputV1> {
    let mut merged: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
    for condition in family.domain().conditions() {
        let expression = condition.polynomial().to_expression().to_canonical_string();
        let entry = merged.entry(expression).or_default();
        entry.0.insert(coefficient_location(&condition.source()));
        entry
            .1
            .extend(condition.origins().iter().map(GuardOrigin::stable_string));
    }
    merged
        .into_iter()
        .map(|(expression, (sources, origins))| ConditionOutputV1 {
            expression,
            sources: sources.into_iter().collect(),
            origins: origins.into_iter().collect(),
        })
        .collect()
}

fn coefficient_location(location: &CoefficientLocation) -> String {
    location.stable_string()
}

fn relation_output(ordinal: usize, relation: &ParametricRelation) -> RelationOutputV1 {
    RelationOutputV1 {
        ordinal,
        stable_id: relation.row_id().stable_string(),
        id: row_id_output(relation.row_id()),
        terms: relation
            .terms()
            .iter()
            .map(|(shift, coefficient)| RelationTermOutputV1 {
                shift: shift.values().to_vec(),
                coefficient: coefficient.to_expression().to_canonical_string(),
            })
            .collect(),
        nonzero_conditions: relation_conditions(relation.guarded_nonzero_conditions()),
    }
}

fn row_id_output(row: &ParametricRowId) -> RowIdOutputV1 {
    match row {
        ParametricRowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } => RowIdOutputV1 {
            kind: "ordinary_ibp",
            contraction_momentum: Some(*contraction_momentum),
            differentiated_loop: Some(*differentiated_loop),
            first_external: None,
            second_external: None,
            label: None,
        },
        ParametricRowId::LorentzInvariance {
            first_external,
            second_external,
        } => RowIdOutputV1 {
            kind: "lorentz_invariance",
            contraction_momentum: None,
            differentiated_loop: None,
            first_external: Some(*first_external),
            second_external: Some(*second_external),
            label: None,
        },
        ParametricRowId::Derived { label } => RowIdOutputV1 {
            kind: "derived",
            contraction_momentum: None,
            differentiated_loop: None,
            first_external: None,
            second_external: None,
            label: Some(label.to_string()),
        },
    }
}

fn relation_conditions(conditions: &[ParametricNonZeroCondition]) -> Vec<ConditionOutputV1> {
    let mut merged: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for condition in conditions {
        merged
            .entry(condition.polynomial().to_expression().to_canonical_string())
            .or_default()
            .extend(condition.origins().iter().map(GuardOrigin::stable_string));
    }
    merged
        .into_iter()
        .map(|(expression, origins)| ConditionOutputV1 {
            expression,
            sources: Vec::new(),
            origins: origins.into_iter().collect(),
        })
        .collect()
}

/// Reject an output whose serialized representation could cross the wire
/// limit before asking TOML to allocate its result buffer.  TOML basic-string
/// escaping expands one UTF-8 input byte by at most six ASCII bytes.  The
/// deliberately loose per-node allowance covers keys, punctuation, decimal
/// ordinals, array/table headers, and whitespace independently of payload
/// length.  An exact post-render check enforces the advertised wire bound.
fn preflight_output_bound(output: &DeriveOutputV1) -> Result<(), AppError> {
    const NODE_OVERHEAD: usize = 4_096;
    const TOML_ESCAPE_FACTOR: usize = 6;

    let mut bound = NODE_OVERHEAD;
    let mut add_node = |strings: &[&str], integer_values: usize| -> Result<(), AppError> {
        bound = bound
            .checked_add(NODE_OVERHEAD)
            .and_then(|value| value.checked_add(integer_values.checked_mul(32)?))
            .ok_or_else(output_bound_overflow)?;
        for value in strings {
            let escaped = value
                .len()
                .checked_mul(TOML_ESCAPE_FACTOR)
                .ok_or_else(output_bound_overflow)?;
            bound = bound
                .checked_add(escaped)
                .ok_or_else(output_bound_overflow)?;
        }
        if bound > MAX_OUTPUT_BYTES {
            return Err(output_bound_error(bound));
        }
        Ok(())
    };

    add_node(
        &[
            output.schema,
            output.status,
            output.equation_convention,
            output.relation_selection,
            output.target_disposition,
            output.producer.name,
            output.producer.rustred_version,
            output.producer.symbolica_version,
            output.producer.expression_format,
            output.provenance.requested_input_format,
            output.provenance.detected_input_form,
            &output.provenance.input_schema,
            &output.provenance.parameter_source,
            &output.provenance.canonical_integral,
            &output.family.name,
            &output.family.fingerprint,
            &output.family.parametric_context_fingerprint,
            &output.family.dimension,
            output.target.disposition,
        ],
        8,
    )?;
    for parameter in &output.provenance.input_parameters {
        add_node(&[parameter], 0)?;
    }
    for (key, value) in &output.provenance.metadata {
        match value {
            MetadataValue::String(value) => add_node(&[key, value], 0)?,
            MetadataValue::StringArray(values) => {
                add_node(&[key], 0)?;
                for value in values {
                    add_node(&[value], 0)?;
                }
            }
        }
    }
    for value in output
        .family
        .parameters
        .iter()
        .chain(&output.family.loop_momenta)
        .chain(&output.family.external_momenta)
        .chain(&output.family.index_symbols)
    {
        add_node(&[value], 0)?;
    }
    if let Some(numerator) = &output.target.numerator {
        add_node(&[numerator], 0)?;
    }
    if let Some(powers) = &output.target.powers {
        add_node(&[], powers.len())?;
    }
    for coordinate in &output.coordinates {
        add_node(&[coordinate.kind, &coordinate.left, &coordinate.right], 1)?;
    }
    for denominator in &output.denominators {
        add_node(
            &[
                &denominator.id,
                &denominator.source_expression,
                &denominator.normalized_expression,
                &denominator.power_shift,
                &denominator.constant,
            ],
            1,
        )?;
        for coefficient in &denominator.coefficients {
            add_node(&[&coefficient.value], 1)?;
        }
    }
    for gram in &output.external_gram {
        add_node(&[&gram.left, &gram.right, &gram.value], 2)?;
    }
    for condition in &output.domain_conditions {
        add_condition_bound(condition, &mut add_node)?;
    }
    for relation in &output.relations {
        add_node(
            &[
                &relation.stable_id,
                relation.id.kind,
                relation.id.label.as_deref().unwrap_or(""),
            ],
            6,
        )?;
        for term in &relation.terms {
            add_node(&[&term.coefficient], term.shift.len())?;
        }
        for condition in &relation.nonzero_conditions {
            add_condition_bound(condition, &mut add_node)?;
        }
    }
    Ok(())
}

fn add_condition_bound(
    condition: &ConditionOutputV1,
    add_node: &mut impl FnMut(&[&str], usize) -> Result<(), AppError>,
) -> Result<(), AppError> {
    add_node(&[&condition.expression], 0)?;
    for source in &condition.sources {
        add_node(&[source], 0)?;
    }
    for origin in &condition.origins {
        add_node(&[origin], 0)?;
    }
    Ok(())
}

fn output_bound_overflow() -> AppError {
    AppError::output_limit("TOML output-size accounting overflowed usize".to_owned())
}

fn output_bound_error(bound: usize) -> AppError {
    AppError::output_limit(format!(
        "TOML output has a conservative {bound}-byte render bound, exceeding the {MAX_OUTPUT_BYTES}-byte application limit"
    ))
}
