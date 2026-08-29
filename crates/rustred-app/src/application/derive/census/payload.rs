use rustred::algebra::CoefficientPolynomial;
use rustred::family::IntegralFamily;
use rustred::identity::ParametricRelation;
use symbolica::prelude::Integer;

use crate::application::MAX_OUTPUT_BYTES;
use crate::application::error::AppError;

use super::derivation_bound_overflow;

const PAYLOAD_NODE_BYTES: usize = 4_096;
const ATOM_RENDER_FACTOR: usize = 320;
const EXPONENT_RENDER_BYTES: usize = 320;
const INTEGER_STRUCTURAL_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::application::derive) struct GeneratedPayloadCensus {
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
pub(in crate::application::derive) fn preflight_family_payload(
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
pub(in crate::application::derive) fn preflight_generated_relations<'a>(
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
