//! Typed references into the program's shared native Symbolica atom table.
//!
//! There is no independent integer or sparse-polynomial wire representation.
//! Native coefficients are imported and structurally checked once by the table
//! owner. References retain payload kind and exact variable-map binding; indexed
//! values also pass the existing context-admission seam. Generated producers
//! supply normalized coefficients, so loading runs no per-reference GCD. The
//! native table import is not a hostile-input parser.

use super::super::error::ArtifactPersistenceError;
use super::binary::{Reader, Writer, check_limit};
use crate::algebra::{
    Coefficient, CoefficientContext, CoefficientPolynomial, IndexedCoefficient,
    IndexedCoefficientContext, IndexedPolynomial,
};
use crate::persistence::CoefficientId;
use std::sync::Arc;
use symbolica::prelude::{Integer, PolyVariable, Z};

const RATIONAL_PAYLOAD: u8 = 1;
const POLYNOMIAL_PAYLOAD: u8 = 2;
const INTEGER_PAYLOAD: u8 = 3;

fn encode_reference(
    writer: &mut Writer,
    kind: u8,
    value: &Coefficient,
) -> Result<(), ArtifactPersistenceError> {
    let terms = if kind == POLYNOMIAL_PAYLOAD {
        value.numerator.nterms()
    } else {
        value
            .numerator
            .nterms()
            .checked_add(value.denominator.nterms())
            .ok_or(ArtifactPersistenceError::ResourceCountOverflow {
                resource: "coefficient polynomial terms",
            })?
    };
    check_limit(
        "coefficient polynomial terms",
        terms,
        writer.limits().max_collection_entries,
    )?;
    let id = writer.intern_coefficient(value)?;
    writer.u8(kind)?;
    writer.u32(id.index() as u32)
}

/// Store an arbitrary-width integer using Symbolica's native constant field.
pub(super) fn encode_integer(
    writer: &mut Writer,
    value: &Integer,
) -> Result<(), ArtifactPersistenceError> {
    let polynomial = CoefficientPolynomial::new_zero(&Z).constant(value.clone());
    encode_reference(writer, INTEGER_PAYLOAD, &polynomial.into())
}

pub(super) fn encode_base_coefficient(
    writer: &mut Writer,
    value: &Coefficient,
) -> Result<(), ArtifactPersistenceError> {
    encode_reference(writer, RATIONAL_PAYLOAD, value)
}

pub(super) fn encode_indexed_coefficient(
    writer: &mut Writer,
    value: &IndexedCoefficient,
) -> Result<(), ArtifactPersistenceError> {
    encode_base_coefficient(writer, value.raw())
}

pub(super) fn encode_indexed_polynomial(
    writer: &mut Writer,
    value: &IndexedPolynomial,
) -> Result<(), ArtifactPersistenceError> {
    encode_base_polynomial(writer, value.raw())
}

pub(super) fn encode_base_polynomial(
    writer: &mut Writer,
    value: &CoefficientPolynomial,
) -> Result<(), ArtifactPersistenceError> {
    encode_reference(writer, POLYNOMIAL_PAYLOAD, &value.clone().into())
}

fn decode_reference<'reader>(
    reader: &'reader mut Reader<'_>,
    expected_kind: u8,
    field: &'static str,
) -> Result<&'reader Coefficient, ArtifactPersistenceError> {
    if reader.u8()? != expected_kind {
        return Err(ArtifactPersistenceError::InvalidCoefficient { field });
    }
    let id = CoefficientId::try_from_index(reader.u32()? as usize)
        .map_err(|_| ArtifactPersistenceError::InvalidCoefficient { field })?;
    let limits = reader.limits();
    let value = reader.native_coefficient(id)?;
    // The table owns full sparse validation. These cheap checks respect a
    // narrower reference-reader term policy before cloning native payloads.
    for polynomial in [&value.numerator, &value.denominator] {
        check_limit(
            "polynomial terms",
            polynomial.nterms(),
            limits.family.exact_algebra.max_polynomial_terms,
        )?;
    }
    Ok(value)
}

fn require_map(
    value: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    field: &'static str,
) -> Result<(), ArtifactPersistenceError> {
    if value.numerator.variables() != variables || value.denominator.variables() != variables {
        return Err(ArtifactPersistenceError::InvalidCoefficient { field });
    }
    Ok(())
}

pub(super) fn decode_integer(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<Integer, ArtifactPersistenceError> {
    let value = decode_reference(reader, INTEGER_PAYLOAD, field)?;
    if !value.numerator.variables().is_empty()
        || !value.denominator.variables().is_empty()
        || !value.denominator.is_one()
        || !value.numerator.is_constant()
    {
        return Err(ArtifactPersistenceError::InvalidCoefficient { field });
    }
    Ok(value.numerator.get_constant())
}

pub(super) fn decode_base_polynomial(
    reader: &mut Reader<'_>,
    variables: &Arc<Vec<PolyVariable>>,
    field: &'static str,
) -> Result<CoefficientPolynomial, ArtifactPersistenceError> {
    let value = decode_reference(reader, POLYNOMIAL_PAYLOAD, field)?;
    require_map(value, variables, field)?;
    if !value.denominator.is_one() {
        return Err(ArtifactPersistenceError::InvalidCoefficient { field });
    }
    Ok(value.numerator.clone())
}

pub(super) fn decode_base_coefficient(
    reader: &mut Reader<'_>,
    context: &CoefficientContext,
    field: &'static str,
) -> Result<Coefficient, ArtifactPersistenceError> {
    let value = decode_reference(reader, RATIONAL_PAYLOAD, field)?;
    require_map(value, context.variables(), field)?;
    Ok(value.clone())
}

pub(super) fn decode_indexed_coefficient(
    reader: &mut Reader<'_>,
    context: &IndexedCoefficientContext,
    field: &'static str,
) -> Result<IndexedCoefficient, ArtifactPersistenceError> {
    let limits = reader.limits().family.exact_algebra;
    let value = decode_reference(reader, RATIONAL_PAYLOAD, field)?;
    let template = context.one();
    require_map(value, template.raw().get_variables(), field)?;
    context
        .admit_native_result_with_limits(value.clone(), limits)
        .map_err(|_| ArtifactPersistenceError::InvalidCoefficient { field })
}

pub(super) fn decode_indexed_polynomial(
    reader: &mut Reader<'_>,
    context: &IndexedCoefficientContext,
    field: &'static str,
) -> Result<IndexedPolynomial, ArtifactPersistenceError> {
    let limits = reader.limits().family.exact_algebra;
    let template = context.one();
    let polynomial = decode_base_polynomial(reader, template.raw().get_variables(), field)?;
    context
        .admit_native_polynomial_result_with_limits(polynomial, limits)
        .map_err(|_| ArtifactPersistenceError::InvalidCoefficient { field })
}

#[cfg(test)]
#[path = "coefficient/decoded_tests.rs"]
mod decoded_tests;
