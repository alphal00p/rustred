use rustred::identity::ParametricRelation;
use rustred::{CoefficientPolynomial, IntegralFamily};
use symbolica::prelude::Integer;

use crate::application::MAX_OUTPUT_BYTES;
use crate::application::error::AppError;
use crate::application::model::MetadataValue;
use crate::application::options::RelationSelection;

use super::model::{ConditionOutputV1, DeriveOutputV1};

const MAX_DERIVATION_TERM_ATTEMPTS: usize = 2_000_000;
const PAYLOAD_NODE_BYTES: usize = 4_096;
const ATOM_RENDER_FACTOR: usize = 320;
const EXPONENT_RENDER_BYTES: usize = 320;
const INTEGER_STRUCTURAL_BYTES: usize = 64;

/// Bound the generator's raw addition work before it constructs a single
/// parametric coefficient. For one ordinary row there is at most one dimension
/// term and `N * (N + 1)` derivative-contraction attempts. One LI row combines
/// two external-contraction ordinary rows per loop through `N + 1` affine
/// weights. These are topology-independent worst-case counts; exact zeroes and
/// equal shifts can only reduce the actual retained support.
pub(super) fn preflight_derivation_structure(
    family: &IntegralFamily,
    selection: RelationSelection,
) -> Result<(), AppError> {
    let loops = family.loop_count();
    let externals = family.external_count();
    let denominators = family.denominator_count();
    let (requested_attempts, selected_rows) =
        derivation_structure_bounds(loops, externals, denominators, selection)?;
    if requested_attempts > MAX_DERIVATION_TERM_ATTEMPTS {
        return Err(AppError::limit(format!(
            "the selected generic derivation has a conservative {requested_attempts}-term-attempt bound (L={loops}, E={externals}, N={denominators}), exceeding the application limit {MAX_DERIVATION_TERM_ATTEMPTS}"
        )));
    }

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

fn derivation_structure_bounds(
    loops: usize,
    externals: usize,
    denominators: usize,
    selection: RelationSelection,
) -> Result<(usize, usize), AppError> {
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
    // No source barrier or LI row exists for fewer than two external momenta.
    // Return before even evaluating bounds for work that will not run.
    if matches!(selection, RelationSelection::LorentzInvariance) && li_rows == 0 {
        return Ok((0, 0));
    }
    let contractions = loops
        .checked_add(externals)
        .ok_or_else(derivation_bound_overflow)?;
    let ordinary_rows = loops
        .checked_mul(contractions)
        .ok_or_else(derivation_bound_overflow)?;
    let external_source_rows = loops
        .checked_mul(externals)
        .ok_or_else(derivation_bound_overflow)?;
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
    let external_source_attempts = external_source_rows
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
        RelationSelection::All => ordinary_attempts
            .checked_add(li_attempts)
            .ok_or_else(derivation_bound_overflow)?,
        // LI-only construction prepares exactly the L*E external-contraction
        // source rows; loop-contraction ordinary rows are neither generated nor
        // charged.
        RelationSelection::LorentzInvariance => external_source_attempts
            .checked_add(li_attempts)
            .ok_or_else(derivation_bound_overflow)?,
    };
    let selected_rows = match selection {
        RelationSelection::All => ordinary_rows
            .checked_add(li_rows)
            .ok_or_else(derivation_bound_overflow)?,
        RelationSelection::Ordinary => ordinary_rows,
        RelationSelection::LorentzInvariance => li_rows,
    };
    Ok((requested_attempts, selected_rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn li_with_zero_or_one_external_charges_no_generation_work() {
        for externals in [0, 1] {
            assert_eq!(
                derivation_structure_bounds(
                    usize::MAX,
                    externals,
                    usize::MAX,
                    RelationSelection::LorentzInvariance,
                )
                .unwrap(),
                (0, 0),
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct GeneratedPayloadCensus {
    relations: usize,
    relation_terms: usize,
    nonzero_conditions: usize,
    condition_sources: usize,
    polynomial_terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
    retained_render_bound: usize,
}

/// Census every family payload which will later cross an Atom-to-string or
/// rational-polynomial-to-Atom boundary. `AtomView::get_byte_size` measures the
/// packed native expression; the factor covers fully-qualified 256-byte labels
/// plus canonical syntax for every packed unit without rendering anything.
pub(super) fn preflight_family_payload(
    normalized: &rustred::input::Project,
    family: &IntegralFamily,
    records: &[rustred::input::LoweredDenominator],
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
        census.nonzero_conditions = checked_census_add(census.nonzero_conditions, 1)?;
        census.condition_sources =
            checked_census_add(census.condition_sources, condition.sources().len())?;
        let source_bytes = condition
            .sources()
            .len()
            .checked_mul(PAYLOAD_NODE_BYTES)
            .ok_or_else(derivation_bound_overflow)?;
        add_generated_payload_bound(
            &mut census,
            PAYLOAD_NODE_BYTES
                .checked_add(source_bytes)
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
pub(super) fn preflight_generated_relations<'a>(
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
        for condition in relation.nonzero_conditions() {
            census.nonzero_conditions = checked_census_add(census.nonzero_conditions, 1)?;
            census.condition_sources =
                checked_census_add(census.condition_sources, condition.sources().len())?;
            let source_bytes = condition
                .sources()
                .len()
                .checked_mul(PAYLOAD_NODE_BYTES)
                .ok_or_else(derivation_bound_overflow)?;
            add_generated_payload_bound(
                &mut census,
                PAYLOAD_NODE_BYTES
                    .checked_add(source_bytes)
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
            "application algebraic output payload has a conservative {}-byte retained/render bound after {} relations, {} terms, {} polynomial terms, {} exponent entries, {} integer bits, {} nonzero conditions, and {} condition sources; the application limit is {MAX_OUTPUT_BYTES} bytes",
            census.retained_render_bound,
            census.relations,
            census.relation_terms,
            census.polynomial_terms,
            census.exponent_entries,
            census.integer_bits,
            census.nonzero_conditions,
            census.condition_sources,
        )));
    }
    Ok(())
}

pub(super) fn derivation_bound_overflow() -> AppError {
    AppError::limit("application derivation resource accounting overflowed usize".to_owned())
}

/// Reject an output whose serialized representation could cross the wire
/// limit before asking TOML to allocate its result buffer. TOML basic-string
/// escaping expands one UTF-8 input byte by at most six ASCII bytes. The
/// deliberately loose per-node allowance covers keys, punctuation, decimal
/// ordinals, array/table headers, and whitespace independently of payload
/// length. An exact post-render check enforces the advertised wire bound.
pub(super) fn preflight_output_bound(output: &DeriveOutputV1) -> Result<(), AppError> {
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
