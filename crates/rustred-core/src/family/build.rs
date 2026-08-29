//! Authenticated integral-family construction.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use crate::algebra::{
    Coefficient, CoefficientContext, CoefficientPolynomial, ExactAlgebraError, ExactAlgebraLimits,
};

use super::error::{IntegralFamilyError, check_family_limit};
use super::exact::{coefficients_are_equal, invert_symbolic_matrix};
use super::fingerprint::{build_family_fingerprint, preflight_family_identity_strings};
use super::kinematics::{
    build_contractions, build_coordinates, checked_derivative_cache_census,
    checked_scalar_product_count,
};
use super::model::{
    AffineDenominator, CoefficientLocation, FamilyDomain, FamilyNonZeroCondition, IntegralFamily,
    IntegralFamilyLimits,
};

impl IntegralFamily {
    /// Construct and exactly authenticate a complete affine denominator basis.
    #[allow(clippy::too_many_arguments)]
    pub fn new<'name>(
        name: impl Into<Cow<'name, str>>,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        coefficients: CoefficientContext,
        dimension: Coefficient,
        denominators: Vec<AffineDenominator>,
        external_gram: Vec<Vec<Coefficient>>,
        power_shifts: Vec<Coefficient>,
    ) -> Result<Self, IntegralFamilyError> {
        Self::new_with_limits(
            name,
            loop_momenta,
            external_momenta,
            coefficients,
            dimension,
            denominators,
            external_gram,
            power_shifts,
            IntegralFamilyLimits::default(),
        )
    }

    /// Construct a family under explicit exact-algebra and allocation limits.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_limits<'name>(
        name: impl Into<Cow<'name, str>>,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        coefficients: CoefficientContext,
        dimension: Coefficient,
        denominators: Vec<AffineDenominator>,
        external_gram: Vec<Vec<Coefficient>>,
        power_shifts: Vec<Coefficient>,
        limits: IntegralFamilyLimits,
    ) -> Result<Self, IntegralFamilyError> {
        let name = name.into();
        check_family_limit(
            "family fingerprint bytes",
            name.len(),
            limits.max_fingerprint_bytes,
        )?;
        let loops = loop_momenta.len();
        let externals = external_momenta.len();
        let scalar_products = checked_scalar_product_count(loops, externals)?;
        check_family_limit(
            "family scalar products",
            scalar_products,
            limits.max_scalar_products,
        )?;
        let augmented_columns =
            scalar_products
                .checked_mul(2)
                .ok_or(IntegralFamilyError::ResourceCountOverflow {
                    resource: "augmented family matrix entries",
                })?;
        let matrix_entries = scalar_products.checked_mul(augmented_columns).ok_or(
            IntegralFamilyError::ResourceCountOverflow {
                resource: "augmented family matrix entries",
            },
        )?;
        check_family_limit(
            "augmented family matrix entries",
            matrix_entries,
            limits.max_matrix_entries,
        )?;
        let derivative_cache = checked_derivative_cache_census(scalar_products, loops, externals)?;
        check_family_limit(
            "family derivative contractions",
            derivative_cache.contractions,
            limits.max_derivative_contractions,
        )?;
        check_family_limit(
            "family derivative contraction coefficient cells",
            derivative_cache.coefficient_cells,
            limits.max_derivative_contraction_coefficient_cells,
        )?;
        if loops == 0 {
            return Err(IntegralFamilyError::NoLoopMomenta);
        }
        preflight_family_identity_strings(
            name.as_ref(),
            &loop_momenta,
            &external_momenta,
            coefficients.parameter_names(),
            limits.max_fingerprint_bytes,
        )?;
        let name = retain_family_name(name)?;
        validate_momentum_labels(&loop_momenta, &external_momenta)?;
        let coordinates = build_coordinates(loops, externals, scalar_products)?;
        let contractions = build_contractions(loops, externals)?;

        if denominators.len() != scalar_products {
            return Err(IntegralFamilyError::WrongDenominatorCount {
                expected: scalar_products,
                actual: denominators.len(),
            });
        }
        if power_shifts.len() != scalar_products {
            return Err(IntegralFamilyError::WrongPowerShiftCount {
                expected: scalar_products,
                actual: power_shifts.len(),
            });
        }
        if external_gram.len() != externals {
            return Err(IntegralFamilyError::WrongExternalGramRowCount {
                expected: externals,
                actual: external_gram.len(),
            });
        }
        for (row, values) in external_gram.iter().enumerate() {
            if values.len() != externals {
                return Err(IntegralFamilyError::WrongExternalGramColumnCount {
                    row,
                    expected: externals,
                    actual: values.len(),
                });
            }
        }
        for (denominator, affine) in denominators.iter().enumerate() {
            if affine.coefficients.len() != scalar_products {
                return Err(IntegralFamilyError::WrongDenominatorRowSize {
                    denominator,
                    expected: scalar_products,
                    actual: affine.coefficients.len(),
                });
            }
        }

        let mut input_denominators = Vec::new();
        validate_and_retain_input_denominator(
            &coefficients,
            &dimension,
            CoefficientLocation::Dimension,
            limits.exact_algebra,
            &mut input_denominators,
        )?;
        for (denominator, affine) in denominators.iter().enumerate() {
            validate_and_retain_input_denominator(
                &coefficients,
                &affine.constant,
                CoefficientLocation::DenominatorConstant { denominator },
                limits.exact_algebra,
                &mut input_denominators,
            )?;
            for (coordinate, coefficient) in affine.coefficients.iter().enumerate() {
                validate_and_retain_input_denominator(
                    &coefficients,
                    coefficient,
                    CoefficientLocation::DenominatorCoefficient {
                        denominator,
                        coordinate,
                    },
                    limits.exact_algebra,
                    &mut input_denominators,
                )?;
            }
        }
        for (row, values) in external_gram.iter().enumerate() {
            for (column, coefficient) in values.iter().enumerate() {
                validate_and_retain_input_denominator(
                    &coefficients,
                    coefficient,
                    CoefficientLocation::ExternalGram { row, column },
                    limits.exact_algebra,
                    &mut input_denominators,
                )?;
            }
        }
        for (denominator, power_shift) in power_shifts.iter().enumerate() {
            validate_and_retain_input_denominator(
                &coefficients,
                power_shift,
                CoefficientLocation::PowerShift { denominator },
                limits.exact_algebra,
                &mut input_denominators,
            )?;
        }
        for row in 0..externals {
            for column in row + 1..externals {
                if !coefficients_are_equal(
                    &coefficients,
                    &external_gram[row][column],
                    &external_gram[column][row],
                    limits.exact_algebra,
                )? {
                    return Err(IntegralFamilyError::AsymmetricExternalGram { row, column });
                }
            }
        }

        // Every coefficient is now authenticated on the exact ordered base
        // map. Census the complete typed identity before any GMP formatting or
        // user-sized fingerprint allocation is attempted.
        let fingerprint = build_family_fingerprint(
            &name,
            &loop_momenta,
            &external_momenta,
            &coefficients,
            &dimension,
            &denominators,
            &external_gram,
            &power_shifts,
            limits,
        )?;

        // Symbolica owns the determinant, inverse, and both identity products.
        // RustRed lends the already-authenticated denominator rows directly,
        // so the matrix boundary can admit retained bytes before making its
        // single fallible native-input clone. It then retains the certified
        // inverse row orientation.
        let (inverse_basis, basis_determinant) =
            invert_symbolic_matrix(&coefficients, &denominators, limits)?;
        let determinant_nonzero = make_family_nonzero_condition(
            CoefficientLocation::BasisDeterminantNumerator,
            basis_determinant.numerator.clone(),
        );
        // Canonicalize the domain to one condition per polynomial. Keep its
        // deterministic order by placing the determinant-bearing condition
        // last, whether it is new or merges an input denominator.
        if let Some(position) = input_denominators
            .iter_mut()
            .position(|condition| condition.polynomial == determinant_nonzero.polynomial)
        {
            let mut merged = input_denominators.remove(position);
            merge_family_condition(&mut merged, &determinant_nonzero);
            input_denominators.push(merged);
        } else {
            input_denominators.push(determinant_nonzero);
        }
        let domain = FamilyDomain {
            conditions: input_denominators,
            basis_determinant,
        };

        let mut family = Self {
            name,
            // The full identity already lives in a fallibly reserved String.
            // Arc::new adds only its fixed-size owner without reallocating or
            // copying that caller-sized buffer.
            fingerprint: Arc::new(fingerprint),
            loop_momenta,
            external_momenta,
            coefficients,
            dimension,
            coordinates,
            contractions,
            denominators,
            external_gram,
            power_shifts,
            limits,
            inverse_basis,
            domain,
            derivative_contractions: Vec::new(),
        };
        family.derivative_contractions = family.build_derivative_contractions()?;
        Ok(family)
    }
}

pub(super) fn retain_family_name(name: Cow<'_, str>) -> Result<String, IntegralFamilyError> {
    match name {
        Cow::Owned(name) => Ok(name),
        Cow::Borrowed(name) => try_copy_family_string(name, "family name"),
    }
}

fn try_copy_family_string(
    source: &str,
    resource: &'static str,
) -> Result<String, IntegralFamilyError> {
    let mut target = String::new();
    target
        .try_reserve_exact(source.len())
        .map_err(|_| IntegralFamilyError::AllocationFailure {
            resource,
            requested: source.len(),
        })?;
    target.push_str(source);
    Ok(target)
}

fn validate_momentum_labels(
    loop_momenta: &[String],
    external_momenta: &[String],
) -> Result<(), IntegralFamilyError> {
    if loop_momenta.is_empty() {
        return Err(IntegralFamilyError::NoLoopMomenta);
    }
    let mut loops = HashSet::new();
    loops
        .try_reserve(loop_momenta.len())
        .map_err(|_| IntegralFamilyError::AllocationFailure {
            resource: "loop momentum label set",
            requested: loop_momenta.len(),
        })?;
    for (index, label) in loop_momenta.iter().enumerate() {
        if label.trim().is_empty() {
            return Err(IntegralFamilyError::EmptyMomentumLabel {
                role: "loop",
                index,
            });
        }
        if !loops.insert(label.as_str()) {
            return Err(IntegralFamilyError::DuplicateMomentumLabel {
                role: "loop",
                label: try_copy_family_string(label, "duplicate loop momentum label")?,
            });
        }
    }
    let mut externals = HashSet::new();
    externals.try_reserve(external_momenta.len()).map_err(|_| {
        IntegralFamilyError::AllocationFailure {
            resource: "external momentum label set",
            requested: external_momenta.len(),
        }
    })?;
    for (index, label) in external_momenta.iter().enumerate() {
        if label.trim().is_empty() {
            return Err(IntegralFamilyError::EmptyMomentumLabel {
                role: "external",
                index,
            });
        }
        if loops.contains(label.as_str()) {
            return Err(IntegralFamilyError::MomentumLabelOverlap {
                label: try_copy_family_string(label, "overlapping momentum label")?,
            });
        }
        if !externals.insert(label.as_str()) {
            return Err(IntegralFamilyError::DuplicateMomentumLabel {
                role: "external",
                label: try_copy_family_string(label, "duplicate external momentum label")?,
            });
        }
    }
    Ok(())
}

fn validate_and_retain_input_denominator(
    context: &CoefficientContext,
    coefficient: &Coefficient,
    location: CoefficientLocation,
    limits: ExactAlgebraLimits,
    conditions: &mut Vec<FamilyNonZeroCondition>,
) -> Result<(), IntegralFamilyError> {
    if let Err(error) = context.validate_with_limits(coefficient, limits) {
        if matches!(error, ExactAlgebraError::VariableMapMismatch { .. }) {
            return Err(IntegralFamilyError::ForeignCoefficientContext { location });
        }
        return Err(IntegralFamilyError::InvalidCoefficient { location, error });
    }
    if !coefficient.denominator.is_one() {
        let condition = make_family_nonzero_condition(location, coefficient.denominator.clone());
        if let Some(existing) = conditions
            .iter_mut()
            .find(|existing| existing.polynomial == condition.polynomial)
        {
            merge_family_condition(existing, &condition);
        } else {
            conditions.push(condition);
        }
    }
    Ok(())
}

fn make_family_nonzero_condition(
    source: CoefficientLocation,
    polynomial: CoefficientPolynomial,
) -> FamilyNonZeroCondition {
    FamilyNonZeroCondition {
        polynomial,
        sources: BTreeSet::from([source]),
    }
}

fn merge_family_condition(target: &mut FamilyNonZeroCondition, source: &FamilyNonZeroCondition) {
    debug_assert_eq!(target.polynomial, source.polynomial);
    target.sources.extend(source.sources.iter().cloned());
}
