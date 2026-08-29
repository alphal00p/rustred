//! Bounded canonical sparse transport for Symbolica polynomials.
//!
//! Durable coefficients never pass through Symbolica's expression parser.
//! The payload is an expanded sparse numerator/denominator pair on the
//! context's already authenticated ordered variable map. Counts, exponent
//! entries, and integer magnitudes are bounded before native construction.

use std::cmp::Ordering;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::backend::integer::{from_lsf_bytes, lsf_byte_size, to_lsf_bytes};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{Integer, MultivariatePolynomial, Z};

use crate::algebra::{
    Coefficient, CoefficientContext, CoefficientPolynomial, IndexedCoefficient, IndexedPolynomial,
};

use super::super::error::ArtifactPersistenceError;
use super::binary::{Reader, Writer, check_limit, try_vec};

const RATIONAL_PAYLOAD: u8 = 1;
const POLYNOMIAL_PAYLOAD: u8 = 2;
const NONNEGATIVE_INTEGER: u8 = 0;
const NEGATIVE_INTEGER: u8 = 1;
const LENGTH_BYTES: usize = std::mem::size_of::<u64>();

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ArtifactPersistenceError> {
    left.checked_add(right)
        .ok_or(ArtifactPersistenceError::ResourceCountOverflow { resource })
}

fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ArtifactPersistenceError> {
    left.checked_mul(right)
        .ok_or(ArtifactPersistenceError::ResourceCountOverflow { resource })
}

fn primitive_magnitude_bytes(value: u128) -> usize {
    if value == 0 {
        0
    } else {
        usize::try_from((u128::BITS - value.leading_zeros()).div_ceil(u8::BITS))
            .expect("u128 byte length fits usize")
    }
}

fn integer_magnitude_bytes(value: &Integer) -> usize {
    match value {
        Integer::Single(value) => primitive_magnitude_bytes(u128::from(value.unsigned_abs())),
        Integer::Double(value) => primitive_magnitude_bytes(value.unsigned_abs()),
        Integer::Large(value) => lsf_byte_size(value),
    }
}

fn integer_payload_size(value: &Integer) -> Result<usize, ArtifactPersistenceError> {
    checked_add(
        1 + LENGTH_BYTES,
        integer_magnitude_bytes(value),
        "integer payload bytes",
    )
}

fn polynomial_payload_size(
    polynomial: &CoefficientPolynomial,
) -> Result<usize, ArtifactPersistenceError> {
    let exponent_bytes = checked_mul(
        polynomial.nvars(),
        std::mem::size_of::<u16>(),
        "polynomial exponent bytes",
    )?;
    let mut bytes = LENGTH_BYTES
        .checked_mul(2)
        .expect("two encoded lengths fit usize");
    for coefficient in &polynomial.coefficients {
        bytes = checked_add(
            bytes,
            integer_payload_size(coefficient)?,
            "polynomial payload bytes",
        )?;
        bytes = checked_add(bytes, exponent_bytes, "polynomial payload bytes")?;
    }
    Ok(bytes)
}

fn coefficient_payload_size(value: &Coefficient) -> Result<usize, ArtifactPersistenceError> {
    checked_add(
        checked_add(
            1,
            polynomial_payload_size(&value.numerator)?,
            "coefficient payload bytes",
        )?,
        polynomial_payload_size(&value.denominator)?,
        "coefficient payload bytes",
    )
}

fn bare_polynomial_payload_size(
    value: &CoefficientPolynomial,
) -> Result<usize, ArtifactPersistenceError> {
    checked_add(
        1,
        polynomial_payload_size(value)?,
        "polynomial payload bytes",
    )
}

fn write_length(
    writer: &mut Writer,
    value: usize,
    resource: &'static str,
) -> Result<(), ArtifactPersistenceError> {
    writer.u64(
        u64::try_from(value)
            .map_err(|_| ArtifactPersistenceError::ResourceCountOverflow { resource })?,
    )
}

fn primitive_magnitude(value: u128) -> ([u8; 16], usize) {
    let bytes = value.to_le_bytes();
    let len = primitive_magnitude_bytes(value);
    (bytes, len)
}

fn encode_integer(writer: &mut Writer, value: &Integer) -> Result<(), ArtifactPersistenceError> {
    writer.u8(if value.is_negative() {
        NEGATIVE_INTEGER
    } else {
        NONNEGATIVE_INTEGER
    })?;
    match value {
        Integer::Single(value) => {
            let (bytes, len) = primitive_magnitude(u128::from(value.unsigned_abs()));
            write_length(writer, len, "integer magnitude bytes")?;
            writer.raw(&bytes[..len])
        }
        Integer::Double(value) => {
            let (bytes, len) = primitive_magnitude(value.unsigned_abs());
            write_length(writer, len, "integer magnitude bytes")?;
            writer.raw(&bytes[..len])
        }
        Integer::Large(value) => {
            let bytes = to_lsf_bytes(value);
            write_length(writer, bytes.len(), "integer magnitude bytes")?;
            writer.raw(&bytes)
        }
    }
}

fn encode_polynomial_body(
    writer: &mut Writer,
    polynomial: &CoefficientPolynomial,
) -> Result<(), ArtifactPersistenceError> {
    write_length(writer, polynomial.nterms(), "polynomial terms")?;
    write_length(writer, polynomial.nvars(), "polynomial variables")?;
    for (coefficient, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        encode_integer(writer, coefficient)?;
        for &exponent in exponents {
            writer.u16(exponent)?;
        }
    }
    Ok(())
}

fn encode_payload(
    writer: &mut Writer,
    size: usize,
    encode: impl FnOnce(&mut Writer) -> Result<(), ArtifactPersistenceError>,
) -> Result<(), ArtifactPersistenceError> {
    writer.charge_coefficient_payload(size)?;
    let mut payload = writer.child();
    encode(&mut payload)?;
    let payload = payload.finish();
    debug_assert_eq!(payload.len(), size);
    writer.bytes(&payload, "coefficient payload bytes")
}

pub(super) fn encode_base_coefficient(
    writer: &mut Writer,
    value: &Coefficient,
) -> Result<(), ArtifactPersistenceError> {
    let terms = checked_add(
        value.numerator.nterms(),
        value.denominator.nterms(),
        "coefficient polynomial terms",
    )?;
    check_limit(
        "coefficient polynomial terms",
        terms,
        writer.limits().max_collection_entries,
    )?;
    let size = coefficient_payload_size(value)?;
    encode_payload(writer, size, |payload| {
        payload.u8(RATIONAL_PAYLOAD)?;
        encode_polynomial_body(payload, &value.numerator)?;
        encode_polynomial_body(payload, &value.denominator)
    })
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
    check_limit(
        "polynomial terms",
        value.nterms(),
        writer.limits().max_collection_entries,
    )?;
    let size = bare_polynomial_payload_size(value)?;
    encode_payload(writer, size, |payload| {
        payload.u8(POLYNOMIAL_PAYLOAD)?;
        encode_polynomial_body(payload, value)
    })
}

fn decode_integer(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<Integer, ArtifactPersistenceError> {
    let sign = reader.u8()?;
    if sign != NONNEGATIVE_INTEGER && sign != NEGATIVE_INTEGER {
        return Err(ArtifactPersistenceError::InvalidCoefficient { field });
    }
    let magnitude = reader.bytes(
        "integer magnitude bytes",
        reader.limits().max_coefficient_bytes,
    )?;
    if magnitude.last() == Some(&0) || (magnitude.is_empty() && sign == NEGATIVE_INTEGER) {
        return Err(ArtifactPersistenceError::NonCanonicalCoefficient { field });
    }
    let mut value = if magnitude.is_empty() {
        Integer::Single(0)
    } else {
        Integer::from(from_lsf_bytes(magnitude))
    };
    if sign == NEGATIVE_INTEGER {
        value = -value;
    }
    Ok(value)
}

fn decode_polynomial_body(
    reader: &mut Reader<'_>,
    context: &CoefficientContext,
    field: &'static str,
) -> Result<CoefficientPolynomial, ArtifactPersistenceError> {
    let term_count = reader.count("polynomial terms")?;
    check_limit(
        "polynomial terms",
        term_count,
        reader.limits().family.exact_algebra.max_polynomial_terms,
    )?;
    let variable_count = reader.count("polynomial variables")?;
    if variable_count != context.variables().len() {
        return Err(ArtifactPersistenceError::InvalidCoefficient { field });
    }
    let exponent_entries = checked_mul(term_count, variable_count, "polynomial exponent entries")?;
    check_limit(
        "polynomial exponent entries",
        exponent_entries,
        reader.limits().max_collection_entries,
    )?;

    let mut polynomial = MultivariatePolynomial::new(&Z, None, context.variables().clone());
    polynomial
        .coefficients
        .try_reserve_exact(term_count)
        .map_err(|_| ArtifactPersistenceError::AllocationFailure {
            resource: "polynomial coefficients",
            requested: term_count,
        })?;
    polynomial
        .exponents
        .try_reserve_exact(exponent_entries)
        .map_err(|_| ArtifactPersistenceError::AllocationFailure {
            resource: "polynomial exponents",
            requested: exponent_entries,
        })?;
    let mut exponents = try_vec(variable_count, "polynomial term exponents")?;
    exponents.resize(variable_count, 0);
    let mut previous: Option<Vec<u16>> = None;
    for _ in 0..term_count {
        let coefficient = decode_integer(reader, field)?;
        if coefficient.cmp(&Integer::Single(0)) == Ordering::Equal {
            return Err(ArtifactPersistenceError::NonCanonicalCoefficient { field });
        }
        for exponent in &mut exponents {
            *exponent = reader.u16()?;
            if *exponent > reader.limits().family.exact_algebra.max_exponent {
                return Err(ArtifactPersistenceError::ResourceLimit {
                    resource: "coefficient exponent",
                    requested: usize::from(*exponent),
                    limit: usize::from(reader.limits().family.exact_algebra.max_exponent),
                });
            }
        }
        if previous
            .as_deref()
            .is_some_and(|prior| prior >= exponents.as_slice())
        {
            return Err(ArtifactPersistenceError::NonCanonicalCoefficient { field });
        }
        polynomial.append_monomial_back(coefficient, &exponents);
        if let Some(prior) = &mut previous {
            prior.copy_from_slice(&exponents);
        } else {
            previous = Some(exponents.clone());
        }
    }
    Ok(polynomial)
}

pub(super) fn decode_base_coefficient(
    reader: &mut Reader<'_>,
    context: &CoefficientContext,
    field: &'static str,
) -> Result<Coefficient, ArtifactPersistenceError> {
    let payload = reader.coefficient_payload(field)?;
    let mut payload_reader = reader.child(payload);
    if payload_reader.u8()? != RATIONAL_PAYLOAD {
        return Err(ArtifactPersistenceError::InvalidCoefficient { field });
    }
    let numerator = decode_polynomial_body(&mut payload_reader, context, field)?;
    let denominator = decode_polynomial_body(&mut payload_reader, context, field)?;
    payload_reader.finish()?;
    let raw = Coefficient {
        numerator,
        denominator,
    };
    context
        .validate_with_limits(&raw, reader.limits().family.exact_algebra)
        .map_err(|_| ArtifactPersistenceError::InvalidCoefficient { field })?;
    let normalization_operations = checked_mul(
        raw.numerator.nterms().max(1),
        raw.denominator.nterms().max(1),
        "coefficient normalization term operations",
    )?;
    check_limit(
        "coefficient normalization term operations",
        normalization_operations,
        reader.limits().family.exact_algebra.max_term_operations,
    )?;
    let normalized = catch_unwind(AssertUnwindSafe(|| {
        <Coefficient as FromNumeratorAndDenominator<_, _, u16>>::from_num_den(
            raw.numerator.clone(),
            raw.denominator.clone(),
            &Z,
            true,
        )
    }))
    .map_err(|_| ArtifactPersistenceError::InvalidCoefficient { field })?;
    if normalized != raw {
        return Err(ArtifactPersistenceError::NonCanonicalCoefficient { field });
    }
    Ok(raw)
}
