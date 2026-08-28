//! Stable typed V2 family fingerprinting.

use std::fmt;
use std::fmt::Write as _;

use symbolica::prelude::Integer;

use crate::algebra::{Coefficient, CoefficientContext};

use super::error::{IntegralFamilyError, check_family_limit};
use super::model::{
    AffineDenominator, BasePolynomial, IntegralFamilyFingerprintStats, IntegralFamilyLimits,
};

pub(super) fn preflight_family_identity_strings(
    name: &str,
    loop_momenta: &[String],
    external_momenta: &[String],
    parameters: &[String],
    limit: usize,
) -> Result<(), IntegralFamilyError> {
    check_family_limit("family fingerprint bytes", name.len(), limit)?;
    for value in loop_momenta
        .iter()
        .chain(external_momenta)
        .chain(parameters)
    {
        check_family_limit("family fingerprint string bytes", value.len(), limit)?;
    }
    Ok(())
}

const INTEGRAL_FAMILY_FINGERPRINT_V2_SCHEMA: &str = "rustred-integral-family-v2;";

#[allow(clippy::too_many_arguments)]
pub(super) fn build_family_fingerprint(
    name: &str,
    loop_momenta: &[String],
    external_momenta: &[String],
    coefficients: &CoefficientContext,
    dimension: &Coefficient,
    denominators: &[AffineDenominator],
    external_gram: &[Vec<Coefficient>],
    power_shifts: &[Coefficient],
    limits: IntegralFamilyLimits,
) -> Result<(String, IntegralFamilyFingerprintStats), IntegralFamilyError> {
    let mut census = FamilyFingerprintCensus::new(limits);
    encode_family_fingerprint(
        &mut census,
        name,
        loop_momenta,
        external_momenta,
        coefficients,
        dimension,
        denominators,
        external_gram,
        power_shifts,
    )?;
    let stats = census.finish();

    let mut writer = FamilyFingerprintWriter::try_new(stats.encoded_bytes)?;
    encode_family_fingerprint(
        &mut writer,
        name,
        loop_momenta,
        external_momenta,
        coefficients,
        dimension,
        denominators,
        external_gram,
        power_shifts,
    )?;
    let fingerprint = writer.finish()?;
    Ok((fingerprint, stats))
}

/// Typed V2 grammar. All variable-size strings are byte-length delimited and
/// all collection shapes precede their payload. A rational coefficient is the
/// ordered pair of its authenticated numerator and denominator sparse
/// polynomials; every Integer is an explicit sign plus uppercase hexadecimal
/// magnitude, independent of Symbolica expression printers and symbol ids.
#[allow(clippy::too_many_arguments)]
fn encode_family_fingerprint(
    sink: &mut impl FamilyFingerprintSink,
    name: &str,
    loop_momenta: &[String],
    external_momenta: &[String],
    coefficients: &CoefficientContext,
    dimension: &Coefficient,
    denominators: &[AffineDenominator],
    external_gram: &[Vec<Coefficient>],
    power_shifts: &[Coefficient],
) -> Result<(), IntegralFamilyError> {
    sink.literal(INTEGRAL_FAMILY_FINGERPRINT_V2_SCHEMA)?;
    sink.literal("N")?;
    encode_fingerprint_string(sink, name)?;
    encode_fingerprint_string_list(sink, "L", loop_momenta)?;
    encode_fingerprint_string_list(sink, "E", external_momenta)?;
    encode_fingerprint_string_list(sink, "P", coefficients.parameter_names())?;

    sink.literal("Q")?;
    sink.usize_value(loop_momenta.len())?;
    sink.literal(",")?;
    sink.usize_value(external_momenta.len())?;
    sink.literal(",")?;
    sink.usize_value(denominators.len())?;
    sink.literal(";")?;

    sink.literal("M;")?;
    encode_fingerprint_coefficient(sink, dimension)?;
    sink.literal("D")?;
    sink.usize_value(denominators.len())?;
    sink.literal(";")?;
    for denominator in denominators {
        encode_fingerprint_coefficient(sink, denominator.constant())?;
        for coefficient in denominator.coefficients() {
            encode_fingerprint_coefficient(sink, coefficient)?;
        }
    }

    let gram_entries = external_gram
        .iter()
        .try_fold(0usize, |count, row| count.checked_add(row.len()))
        .ok_or(IntegralFamilyError::ResourceCountOverflow {
            resource: "family fingerprint external Gram entries",
        })?;
    sink.literal("G")?;
    sink.usize_value(gram_entries)?;
    sink.literal(";")?;
    for coefficient in external_gram.iter().flatten() {
        encode_fingerprint_coefficient(sink, coefficient)?;
    }

    sink.literal("U")?;
    sink.usize_value(power_shifts.len())?;
    sink.literal(";")?;
    for shift in power_shifts {
        encode_fingerprint_coefficient(sink, shift)?;
    }
    Ok(())
}

fn encode_fingerprint_string_list(
    sink: &mut impl FamilyFingerprintSink,
    marker: &'static str,
    values: &[String],
) -> Result<(), IntegralFamilyError> {
    sink.literal(marker)?;
    sink.usize_value(values.len())?;
    sink.literal(";")?;
    for value in values {
        encode_fingerprint_string(sink, value)?;
    }
    Ok(())
}

fn encode_fingerprint_string(
    sink: &mut impl FamilyFingerprintSink,
    value: &str,
) -> Result<(), IntegralFamilyError> {
    sink.usize_value(value.len())?;
    sink.literal(":")?;
    sink.literal(value)?;
    sink.literal(";")
}

fn encode_fingerprint_coefficient(
    sink: &mut impl FamilyFingerprintSink,
    coefficient: &Coefficient,
) -> Result<(), IntegralFamilyError> {
    sink.literal("R")?;
    encode_fingerprint_polynomial(sink, &coefficient.numerator)?;
    encode_fingerprint_polynomial(sink, &coefficient.denominator)
}

fn encode_fingerprint_polynomial(
    sink: &mut impl FamilyFingerprintSink,
    polynomial: &BasePolynomial,
) -> Result<(), IntegralFamilyError> {
    let variables = polynomial.variables.len();
    sink.literal("Y")?;
    sink.usize_value(variables)?;
    sink.literal(",")?;
    sink.usize_value(polynomial.coefficients.len())?;
    sink.literal(";")?;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        sink.polynomial_term()?;
        sink.integer_value(coefficient)?;
        sink.literal("X")?;
        let start =
            term.checked_mul(variables)
                .ok_or(IntegralFamilyError::ResourceCountOverflow {
                    resource: "family fingerprint polynomial exponent offset",
                })?;
        let end =
            start
                .checked_add(variables)
                .ok_or(IntegralFamilyError::ResourceCountOverflow {
                    resource: "family fingerprint polynomial exponent offset",
                })?;
        let exponents = polynomial.exponents.get(start..end).ok_or_else(|| {
            IntegralFamilyError::InternalVerificationFailure {
                detail: "authenticated fingerprint polynomial has a malformed exponent layout"
                    .to_owned(),
            }
        })?;
        for (position, &exponent) in exponents.iter().enumerate() {
            if position != 0 {
                sink.literal(",")?;
            }
            sink.exponent_value(exponent)?;
        }
        sink.literal(";")?;
    }
    Ok(())
}

trait FamilyFingerprintSink {
    fn literal(&mut self, value: &str) -> Result<(), IntegralFamilyError>;
    fn usize_value(&mut self, value: usize) -> Result<(), IntegralFamilyError>;
    fn polynomial_term(&mut self) -> Result<(), IntegralFamilyError>;
    fn exponent_value(&mut self, value: u16) -> Result<(), IntegralFamilyError>;
    fn integer_value(&mut self, value: &Integer) -> Result<(), IntegralFamilyError>;
}

struct FamilyFingerprintCensus {
    limits: IntegralFamilyLimits,
    stats: IntegralFamilyFingerprintStats,
}

impl FamilyFingerprintCensus {
    const fn new(limits: IntegralFamilyLimits) -> Self {
        Self {
            limits,
            stats: IntegralFamilyFingerprintStats {
                encoded_bytes: 0,
                encoding_work: 0,
                polynomial_terms: 0,
                exponent_entries: 0,
                integer_bits: 0,
            },
        }
    }

    const fn finish(self) -> IntegralFamilyFingerprintStats {
        self.stats
    }

    fn add_bytes(&mut self, additional: usize) -> Result<(), IntegralFamilyError> {
        self.stats.encoded_bytes = checked_bounded_fingerprint_add(
            "family fingerprint bytes",
            self.stats.encoded_bytes,
            additional,
            self.limits.max_fingerprint_bytes,
        )?;
        self.add_work(additional)
    }

    fn add_work(&mut self, additional: usize) -> Result<(), IntegralFamilyError> {
        self.stats.encoding_work = checked_bounded_fingerprint_add(
            "family fingerprint encoding work",
            self.stats.encoding_work,
            additional,
            self.limits.max_fingerprint_encoding_work,
        )?;
        Ok(())
    }
}

impl FamilyFingerprintSink for FamilyFingerprintCensus {
    fn literal(&mut self, value: &str) -> Result<(), IntegralFamilyError> {
        self.add_bytes(value.len())
    }

    fn usize_value(&mut self, value: usize) -> Result<(), IntegralFamilyError> {
        self.add_bytes(decimal_digits_usize(value))
    }

    fn polynomial_term(&mut self) -> Result<(), IntegralFamilyError> {
        self.stats.polynomial_terms = checked_bounded_fingerprint_add(
            "family fingerprint polynomial terms",
            self.stats.polynomial_terms,
            1,
            self.limits.max_fingerprint_polynomial_terms,
        )?;
        self.add_work(1)
    }

    fn exponent_value(&mut self, value: u16) -> Result<(), IntegralFamilyError> {
        self.stats.exponent_entries = checked_bounded_fingerprint_add(
            "family fingerprint exponent entries",
            self.stats.exponent_entries,
            1,
            self.limits.max_fingerprint_exponent_entries,
        )?;
        self.add_work(1)?;
        self.add_bytes(decimal_digits_usize(usize::from(value)))
    }

    fn integer_value(&mut self, value: &Integer) -> Result<(), IntegralFamilyError> {
        let bits = family_fingerprint_integer_bits(value)?;
        self.stats.integer_bits = checked_bounded_fingerprint_add(
            "family fingerprint integer bits",
            self.stats.integer_bits,
            bits,
            self.limits.max_fingerprint_integer_bits,
        )?;
        self.add_work(bits)?;
        let hexadecimal_digits = if bits == 0 {
            1
        } else {
            bits.checked_add(3)
                .ok_or(IntegralFamilyError::ResourceCountOverflow {
                    resource: "family fingerprint hexadecimal digits",
                })?
                / 4
        };
        // `I`, explicit sign, magnitude, and `;`.
        self.add_bytes(hexadecimal_digits.checked_add(3).ok_or(
            IntegralFamilyError::ResourceCountOverflow {
                resource: "family fingerprint integer bytes",
            },
        )?)
    }
}

struct FamilyFingerprintWriter {
    output: String,
    expected_bytes: usize,
}

impl FamilyFingerprintWriter {
    fn try_new(expected_bytes: usize) -> Result<Self, IntegralFamilyError> {
        let mut output = String::new();
        output.try_reserve_exact(expected_bytes).map_err(|_| {
            IntegralFamilyError::AllocationFailure {
                resource: "family fingerprint",
                requested: expected_bytes,
            }
        })?;
        Ok(Self {
            output,
            expected_bytes,
        })
    }

    fn finish(self) -> Result<String, IntegralFamilyError> {
        if self.output.len() != self.expected_bytes {
            return Err(IntegralFamilyError::InternalVerificationFailure {
                detail: "family fingerprint census differs from encoded byte length".to_owned(),
            });
        }
        Ok(self.output)
    }

    fn formatting_failure() -> IntegralFamilyError {
        IntegralFamilyError::InternalVerificationFailure {
            detail: "family fingerprint exceeded its authenticated byte census".to_owned(),
        }
    }
}

impl fmt::Write for FamilyFingerprintWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let requested = self
            .output
            .len()
            .checked_add(value.len())
            .ok_or(fmt::Error)?;
        if requested > self.expected_bytes {
            return Err(fmt::Error);
        }
        self.output.push_str(value);
        Ok(())
    }
}

impl FamilyFingerprintSink for FamilyFingerprintWriter {
    fn literal(&mut self, value: &str) -> Result<(), IntegralFamilyError> {
        self.write_str(value)
            .map_err(|_| Self::formatting_failure())
    }

    fn usize_value(&mut self, value: usize) -> Result<(), IntegralFamilyError> {
        write!(self, "{value}").map_err(|_| Self::formatting_failure())
    }

    fn polynomial_term(&mut self) -> Result<(), IntegralFamilyError> {
        Ok(())
    }

    fn exponent_value(&mut self, value: u16) -> Result<(), IntegralFamilyError> {
        write!(self, "{value}").map_err(|_| Self::formatting_failure())
    }

    fn integer_value(&mut self, value: &Integer) -> Result<(), IntegralFamilyError> {
        let result = match value {
            Integer::Single(value) => {
                let sign = if *value < 0 { '-' } else { '+' };
                write!(self, "I{sign}{:X};", value.unsigned_abs())
            }
            Integer::Double(value) => {
                let sign = if *value < 0 { '-' } else { '+' };
                write!(self, "I{sign}{:X};", value.unsigned_abs())
            }
            // Rug's hexadecimal formatter emits a leading minus followed by
            // the magnitude, so no proportional GMP clone is needed here.
            Integer::Large(value) if value.is_negative() => write!(self, "I{value:X};"),
            Integer::Large(value) => write!(self, "I+{value:X};"),
        };
        result.map_err(|_| Self::formatting_failure())
    }
}

const fn decimal_digits_usize(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        value.ilog10() as usize + 1
    }
}

fn family_fingerprint_integer_bits(value: &Integer) -> Result<usize, IntegralFamilyError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| IntegralFamilyError::ResourceCountOverflow {
        resource: "family fingerprint integer bits",
    })
}

fn checked_bounded_fingerprint_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, IntegralFamilyError> {
    let requested = current
        .checked_add(additional)
        .ok_or(IntegralFamilyError::ResourceCountOverflow { resource })?;
    check_family_limit(resource, requested, limit)?;
    Ok(requested)
}
