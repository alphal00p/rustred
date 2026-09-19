//! Topology-independent family geometry sharing a program's native atom table.
//!
//! Labels are structural metadata; every mathematical coefficient is a native
//! table reference. Original input text may be retained separately as provenance
//! but is not needed to reconstruct the family.

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily, IntegralFamilyLimits};

use super::error::check_limit;
use super::{
    BinaryIoError, BinaryIoLimits, CoefficientId, CoefficientTableBuilder, DecodedCoefficientTable,
};

/// The input geometry of any complete affine denominator family. This record
/// does not encode, discover, or certify any reduction rules.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct NativeFamilyRecord {
    name: String,
    loop_momenta: Vec<String>,
    external_momenta: Vec<String>,
    parameters: Vec<String>,
    dimension: u32,
    denominators: Vec<DenominatorRecord>,
    external_gram: Vec<Vec<u32>>,
    power_shifts: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct DenominatorRecord {
    constant: u32,
    coefficients: Vec<u32>,
}

impl NativeFamilyRecord {
    /// Intern family coefficients into the same dictionary as its rules.
    pub fn from_family(
        family: &IntegralFamily,
        table: &mut CoefficientTableBuilder,
    ) -> Result<Self, BinaryIoError> {
        let mut intern = |value: &Coefficient| table.intern(value).map(|id| id.index() as u32);
        let dimension = intern(family.dimension())?;
        let denominators = family
            .denominators()
            .iter()
            .map(|denominator| {
                Ok(DenominatorRecord {
                    constant: intern(denominator.constant())?,
                    coefficients: denominator
                        .coefficients()
                        .iter()
                        .map(&mut intern)
                        .collect::<Result<_, BinaryIoError>>()?,
                })
            })
            .collect::<Result<_, BinaryIoError>>()?;
        let external_gram = family
            .external_gram()
            .iter()
            .map(|row| {
                row.iter()
                    .map(&mut intern)
                    .collect::<Result<_, BinaryIoError>>()
            })
            .collect::<Result<_, BinaryIoError>>()?;
        let power_shifts = family
            .power_shifts()
            .iter()
            .map(&mut intern)
            .collect::<Result<_, BinaryIoError>>()?;
        Ok(Self {
            name: family.name().to_owned(),
            loop_momenta: family.loop_momenta().to_vec(),
            external_momenta: family.external_momenta().to_vec(),
            parameters: family.coefficient_context().parameter_names().to_vec(),
            dimension,
            denominators,
            external_gram,
            power_shifts,
        })
    }

    pub fn arity(&self) -> usize {
        self.denominators.len()
    }

    /// Validate structural shape before cloning any native coefficient arrays.
    /// Native state/atom import remains a trusted-generated-data boundary.
    pub fn validate_shape(
        &self,
        family_limits: IntegralFamilyLimits,
        io_limits: BinaryIoLimits,
    ) -> Result<(), BinaryIoError> {
        let loops = self.loop_momenta.len();
        let externals = self.external_momenta.len();
        let coordinates = loops
            .checked_add(1)
            .and_then(|next| loops.checked_mul(next))
            .map(|square| square / 2)
            .and_then(|count| {
                loops
                    .checked_mul(externals)
                    .and_then(|mixed| count.checked_add(mixed))
            })
            .ok_or(BinaryIoError::Invalid("family coordinate count overflow"))?;
        if loops == 0
            || self.denominators.len() != coordinates
            || self
                .denominators
                .iter()
                .any(|d| d.coefficients.len() != coordinates)
            || self.external_gram.len() != externals
            || self.external_gram.iter().any(|row| row.len() != externals)
            || self.power_shifts.len() != coordinates
        {
            return Err(BinaryIoError::Invalid(
                "inconsistent complete family geometry",
            ));
        }
        check_limit(
            "family scalar products",
            coordinates,
            family_limits.max_scalar_products,
        )?;
        let matrix_entries = coordinates
            .checked_mul(coordinates)
            .ok_or(BinaryIoError::Invalid("family matrix size overflow"))?;
        check_limit(
            "family matrix entries",
            matrix_entries.checked_mul(2).ok_or(BinaryIoError::Invalid(
                "augmented family matrix size overflow",
            ))?,
            family_limits.max_matrix_entries,
        )?;
        let coefficient_count = externals
            .checked_mul(externals)
            .and_then(|gram| matrix_entries.checked_add(gram))
            .and_then(|count| {
                coordinates
                    .checked_mul(2)
                    .and_then(|extra| count.checked_add(extra))
            })
            .and_then(|count| count.checked_add(1))
            .ok_or(BinaryIoError::Invalid("family coefficient count overflow"))?;
        check_limit(
            "family coefficient entries",
            coefficient_count,
            io_limits.max_collection_entries,
        )?;
        let label_bytes = std::iter::once(&self.name)
            .chain(&self.loop_momenta)
            .chain(&self.external_momenta)
            .chain(&self.parameters)
            .try_fold(0usize, |sum, label| sum.checked_add(label.len()))
            .ok_or(BinaryIoError::Invalid("family label byte count overflow"))?;
        check_limit(
            "family labels",
            label_bytes,
            family_limits
                .max_fingerprint_bytes
                .min(io_limits.max_program_bytes),
        )?;
        check_limit(
            "family parameters",
            self.parameters.len(),
            io_limits.max_collection_entries,
        )?;
        Ok(())
    }

    /// Reconstruct through the existing exact family constructor. It validates
    /// context binding, basis invertibility, Gram symmetry and domain conditions;
    /// no source-expression parsing or topology-specific factory is involved.
    pub fn to_family(
        &self,
        table: &DecodedCoefficientTable,
        family_limits: IntegralFamilyLimits,
        io_limits: BinaryIoLimits,
    ) -> Result<IntegralFamily, BinaryIoError> {
        self.validate_shape(family_limits, io_limits)?;
        let context = CoefficientContext::try_new(self.parameters.clone())
            .map_err(|error| BinaryIoError::Native(error.to_string()))?;
        let coefficient = |index: u32| -> Result<Coefficient, BinaryIoError> {
            let value = table.coefficient(CoefficientId::try_from_index(index as usize)?)?;
            if value.numerator.variables() != context.variables()
                || value.denominator.variables() != context.variables()
            {
                return Err(BinaryIoError::Invalid(
                    "family coefficient variable map differs from its declared context",
                ));
            }
            Ok(value.clone())
        };
        let dimension = coefficient(self.dimension)?;
        let denominators = self
            .denominators
            .iter()
            .map(|denominator| {
                Ok(AffineDenominator::new(
                    coefficient(denominator.constant)?,
                    denominator
                        .coefficients
                        .iter()
                        .map(|&id| coefficient(id))
                        .collect::<Result<_, BinaryIoError>>()?,
                ))
            })
            .collect::<Result<_, BinaryIoError>>()?;
        let gram = self
            .external_gram
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&id| coefficient(id))
                    .collect::<Result<_, BinaryIoError>>()
            })
            .collect::<Result<_, BinaryIoError>>()?;
        let shifts = self
            .power_shifts
            .iter()
            .map(|&id| coefficient(id))
            .collect::<Result<_, BinaryIoError>>()?;
        IntegralFamily::new_with_limits(
            self.name.clone(),
            self.loop_momenta.clone(),
            self.external_momenta.clone(),
            context,
            dimension,
            denominators,
            gram,
            shifts,
            family_limits,
        )
        .map_err(|error| BinaryIoError::Native(format!("family admission: {error}")))
    }
}

#[cfg(test)]
mod tests;
