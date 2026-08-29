//! Scalar-product coordinates and derivative contractions.

use crate::algebra::Coefficient;

use super::error::IntegralFamilyError;
use super::model::{
    ContractionMomentum, DenominatorExpansion, IntegralFamily, ScalarProductCoordinate,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DerivativeCacheCensus {
    pub(super) contractions: usize,
    pub(super) coefficient_cells: usize,
}

impl IntegralFamily {
    /// Return the deterministic position of a typed scalar product.
    pub fn coordinate_index(
        &self,
        coordinate: ScalarProductCoordinate,
    ) -> Result<usize, IntegralFamilyError> {
        match coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                for index in [left, right] {
                    if index >= self.loop_count() {
                        return Err(IntegralFamilyError::LoopMomentumOutOfRange {
                            index,
                            loops: self.loop_count(),
                        });
                    }
                }
                Ok(self.loop_loop_coordinate_index(left, right))
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => {
                if loop_index >= self.loop_count() {
                    return Err(IntegralFamilyError::LoopMomentumOutOfRange {
                        index: loop_index,
                        loops: self.loop_count(),
                    });
                }
                if external_index >= self.external_count() {
                    return Err(IntegralFamilyError::ExternalMomentumOutOfRange {
                        index: external_index,
                        externals: self.external_count(),
                    });
                }
                Ok(self.loop_external_coordinate_index(loop_index, external_index))
            }
        }
    }

    /// Express one scalar-product coordinate in the denominator basis.
    pub fn scalar_product_expansion(
        &self,
        coordinate: usize,
    ) -> Result<DenominatorExpansion, IntegralFamilyError> {
        let Some(inverse_row) = self.inverse_basis.get(coordinate) else {
            return Err(IntegralFamilyError::ScalarProductOutOfRange {
                index: coordinate,
                scalar_products: self.coordinates.len(),
            });
        };
        let mut denominator_coefficients = try_family_vec(
            "scalar-product expansion denominator coefficients",
            inverse_row.len(),
        )?;
        // The caller-sized Rust vector is reserved fallibly before cloning.
        // Symbolica's public coefficient representation does not expose a
        // fallible deep-clone operation for each polynomial payload.
        denominator_coefficients.extend(inverse_row.iter().cloned());
        let mut constant = self.coefficients.zero();
        for (coefficient, denominator) in denominator_coefficients.iter().zip(&self.denominators) {
            let contribution = self.coefficients.try_mul(
                coefficient,
                &denominator.constant,
                self.limits.exact_algebra,
            )?;
            constant =
                self.coefficients
                    .try_sub(&constant, &contribution, self.limits.exact_algebra)?;
        }
        Ok(DenominatorExpansion {
            constant,
            denominator_coefficients,
        })
    }

    /// Return the cached affine image of `q . d D_r / d k_i`.
    pub fn derivative_contraction(
        &self,
        denominator: usize,
        differentiated_loop: usize,
        contraction: ContractionMomentum,
    ) -> Result<&DenominatorExpansion, IntegralFamilyError> {
        if denominator >= self.denominator_count() {
            return Err(IntegralFamilyError::DenominatorOutOfRange {
                index: denominator,
                denominators: self.denominator_count(),
            });
        }
        if differentiated_loop >= self.loop_count() {
            return Err(IntegralFamilyError::LoopMomentumOutOfRange {
                index: differentiated_loop,
                loops: self.loop_count(),
            });
        }
        let contraction_index = match contraction {
            ContractionMomentum::Loop(index) => {
                if index >= self.loop_count() {
                    return Err(IntegralFamilyError::LoopMomentumOutOfRange {
                        index,
                        loops: self.loop_count(),
                    });
                }
                index
            }
            ContractionMomentum::External(index) => {
                if index >= self.external_count() {
                    return Err(IntegralFamilyError::ExternalMomentumOutOfRange {
                        index,
                        externals: self.external_count(),
                    });
                }
                self.loop_count() + index
            }
        };
        Ok(&self.derivative_contractions[denominator][differentiated_loop][contraction_index])
    }

    pub(super) fn build_derivative_contractions(
        &self,
    ) -> Result<Vec<Vec<Vec<DenominatorExpansion>>>, IntegralFamilyError> {
        let mut by_denominator = try_family_vec(
            "derivative-contraction denominator rows",
            self.denominator_count(),
        )?;
        for denominator in 0..self.denominator_count() {
            let mut by_loop = try_family_vec(
                "derivative-contraction differentiated-loop rows",
                self.loop_count(),
            )?;
            for differentiated_loop in 0..self.loop_count() {
                let mut by_contraction = try_family_vec(
                    "derivative-contraction affine expansions",
                    self.contractions.len(),
                )?;
                for &contraction in &self.contractions {
                    let (constant, scalar_coefficients) =
                        self.direct_derivative(denominator, differentiated_loop, contraction)?;
                    by_contraction
                        .push(self.rewrite_scalar_affine(constant, &scalar_coefficients)?);
                }
                by_loop.push(by_contraction);
            }
            by_denominator.push(by_loop);
        }
        Ok(by_denominator)
    }

    pub(super) fn direct_derivative(
        &self,
        denominator: usize,
        differentiated_loop: usize,
        contraction: ContractionMomentum,
    ) -> Result<(Coefficient, Vec<Coefficient>), IntegralFamilyError> {
        let mut constant = self.coefficients.zero();
        let mut scalar_coefficients = try_zero_coefficients(
            &self.coefficients,
            "direct-derivative scalar coefficients",
            self.coordinates.len(),
        )?;
        for (coordinate_index, coordinate) in self.coordinates.iter().copied().enumerate() {
            let coefficient = &self.denominators[denominator].coefficients[coordinate_index];
            if coefficient.is_zero() {
                continue;
            }
            match coordinate {
                ScalarProductCoordinate::LoopLoop { left, right } => {
                    if differentiated_loop == left {
                        self.add_dot_with_loop(
                            &mut scalar_coefficients,
                            contraction,
                            right,
                            coefficient,
                        )?;
                    }
                    if differentiated_loop == right {
                        self.add_dot_with_loop(
                            &mut scalar_coefficients,
                            contraction,
                            left,
                            coefficient,
                        )?;
                    }
                }
                ScalarProductCoordinate::LoopExternal {
                    loop_index,
                    external_index,
                } => {
                    if differentiated_loop == loop_index {
                        self.add_dot_with_external(
                            &mut constant,
                            &mut scalar_coefficients,
                            contraction,
                            external_index,
                            coefficient,
                        )?;
                    }
                }
            }
        }
        Ok((constant, scalar_coefficients))
    }

    fn add_dot_with_loop(
        &self,
        scalar_coefficients: &mut [Coefficient],
        contraction: ContractionMomentum,
        loop_index: usize,
        coefficient: &Coefficient,
    ) -> Result<(), IntegralFamilyError> {
        let coordinate = match contraction {
            ContractionMomentum::Loop(other) => self.loop_loop_coordinate_index(loop_index, other),
            ContractionMomentum::External(external_index) => {
                self.loop_external_coordinate_index(loop_index, external_index)
            }
        };
        scalar_coefficients[coordinate] = self.coefficients.try_add(
            &scalar_coefficients[coordinate],
            coefficient,
            self.limits.exact_algebra,
        )?;
        Ok(())
    }

    fn add_dot_with_external(
        &self,
        constant: &mut Coefficient,
        scalar_coefficients: &mut [Coefficient],
        contraction: ContractionMomentum,
        external_index: usize,
        coefficient: &Coefficient,
    ) -> Result<(), IntegralFamilyError> {
        match contraction {
            ContractionMomentum::Loop(loop_index) => {
                let coordinate = self.loop_external_coordinate_index(loop_index, external_index);
                scalar_coefficients[coordinate] = self.coefficients.try_add(
                    &scalar_coefficients[coordinate],
                    coefficient,
                    self.limits.exact_algebra,
                )?;
            }
            ContractionMomentum::External(other) => {
                let contribution = self.coefficients.try_mul(
                    coefficient,
                    &self.external_gram[other][external_index],
                    self.limits.exact_algebra,
                )?;
                *constant = self.coefficients.try_add(
                    constant,
                    &contribution,
                    self.limits.exact_algebra,
                )?;
            }
        }
        Ok(())
    }

    fn rewrite_scalar_affine(
        &self,
        direct_constant: Coefficient,
        scalar_coefficients: &[Coefficient],
    ) -> Result<DenominatorExpansion, IntegralFamilyError> {
        let mut denominator_coefficients = try_zero_coefficients(
            &self.coefficients,
            "derivative-contraction denominator coefficients",
            self.denominator_count(),
        )?;
        for (scalar_product, scalar_coefficient) in scalar_coefficients.iter().enumerate() {
            if scalar_coefficient.is_zero() {
                continue;
            }
            for (target, inverse_coefficient) in
                self.inverse_basis[scalar_product].iter().enumerate()
            {
                let contribution = self.coefficients.try_mul(
                    scalar_coefficient,
                    inverse_coefficient,
                    self.limits.exact_algebra,
                )?;
                denominator_coefficients[target] = self.coefficients.try_add(
                    &denominator_coefficients[target],
                    &contribution,
                    self.limits.exact_algebra,
                )?;
            }
        }
        let mut constant = direct_constant;
        for (coefficient, denominator) in denominator_coefficients.iter().zip(&self.denominators) {
            let contribution = self.coefficients.try_mul(
                coefficient,
                &denominator.constant,
                self.limits.exact_algebra,
            )?;
            constant =
                self.coefficients
                    .try_sub(&constant, &contribution, self.limits.exact_algebra)?;
        }
        Ok(DenominatorExpansion {
            constant,
            denominator_coefficients,
        })
    }

    fn loop_loop_coordinate_index(&self, left: usize, right: usize) -> usize {
        let (left, right) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        let total = triangular(self.loop_count())
            .expect("the family constructor proved the loop-loop count representable");
        let remaining = triangular(self.loop_count() - left)
            .expect("a smaller triangular count is representable");
        total - remaining + (right - left)
    }

    fn loop_external_coordinate_index(&self, loop_index: usize, external_index: usize) -> usize {
        let loop_loop = triangular(self.loop_count())
            .expect("the family constructor proved the loop-loop count representable");
        loop_loop + loop_index * self.external_count() + external_index
    }
}

pub(super) fn checked_derivative_cache_census(
    scalar_products: usize,
    loops: usize,
    externals: usize,
) -> Result<DerivativeCacheCensus, IntegralFamilyError> {
    let contraction_momenta =
        loops
            .checked_add(externals)
            .ok_or(IntegralFamilyError::ResourceCountOverflow {
                resource: "family derivative contraction momenta",
            })?;
    let contractions = scalar_products
        .checked_mul(loops)
        .and_then(|count| count.checked_mul(contraction_momenta))
        .ok_or(IntegralFamilyError::ResourceCountOverflow {
            resource: "family derivative contractions",
        })?;
    let coefficients_per_contraction =
        scalar_products
            .checked_add(1)
            .ok_or(IntegralFamilyError::ResourceCountOverflow {
                resource: "family derivative contraction coefficient cells",
            })?;
    let coefficient_cells = contractions
        .checked_mul(coefficients_per_contraction)
        .ok_or(IntegralFamilyError::ResourceCountOverflow {
            resource: "family derivative contraction coefficient cells",
        })?;
    Ok(DerivativeCacheCensus {
        contractions,
        coefficient_cells,
    })
}

fn try_family_vec<T>(
    resource: &'static str,
    requested: usize,
) -> Result<Vec<T>, IntegralFamilyError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(requested)
        .map_err(|_| IntegralFamilyError::AllocationFailure {
            resource,
            requested,
        })?;
    Ok(values)
}

fn try_zero_coefficients(
    context: &crate::algebra::CoefficientContext,
    resource: &'static str,
    requested: usize,
) -> Result<Vec<Coefficient>, IntegralFamilyError> {
    let mut values = try_family_vec(resource, requested)?;
    // The Rust-owned cell buffer is now fully reserved. Symbolica's public
    // coefficient API has no fallible constructor for each exact zero's
    // internal polynomial storage; the family-level cell census is therefore
    // the truthful prospective boundary available here.
    values.resize_with(requested, || context.zero());
    Ok(values)
}

fn triangular(value: usize) -> Option<usize> {
    let successor = value.checked_add(1)?;
    let (left, right) = if value % 2 == 0 {
        (value / 2, successor)
    } else {
        (value, successor / 2)
    };
    left.checked_mul(right)
}

pub(super) fn checked_scalar_product_count(
    loops: usize,
    externals: usize,
) -> Result<usize, IntegralFamilyError> {
    if loops == 0 {
        return Err(IntegralFamilyError::NoLoopMomenta);
    }
    let loop_loop = triangular(loops)
        .ok_or(IntegralFamilyError::ScalarProductCountOverflow { loops, externals })?;
    let loop_external = loops
        .checked_mul(externals)
        .ok_or(IntegralFamilyError::ScalarProductCountOverflow { loops, externals })?;
    loop_loop
        .checked_add(loop_external)
        .ok_or(IntegralFamilyError::ScalarProductCountOverflow { loops, externals })
}

pub(super) fn build_coordinates(
    loops: usize,
    externals: usize,
    capacity: usize,
) -> Result<Vec<ScalarProductCoordinate>, IntegralFamilyError> {
    let mut coordinates = try_family_vec("family scalar-product coordinates", capacity)?;
    for left in 0..loops {
        for right in left..loops {
            coordinates.push(ScalarProductCoordinate::LoopLoop { left, right });
        }
    }
    for loop_index in 0..loops {
        for external_index in 0..externals {
            coordinates.push(ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            });
        }
    }
    Ok(coordinates)
}

pub(super) fn build_contractions(
    loops: usize,
    externals: usize,
) -> Result<Vec<ContractionMomentum>, IntegralFamilyError> {
    let requested =
        loops
            .checked_add(externals)
            .ok_or(IntegralFamilyError::ResourceCountOverflow {
                resource: "family contraction momenta",
            })?;
    let mut contractions = try_family_vec("family contraction momenta", requested)?;
    contractions.extend((0..loops).map(ContractionMomentum::Loop));
    contractions.extend((0..externals).map(ContractionMomentum::External));
    Ok(contractions)
}
