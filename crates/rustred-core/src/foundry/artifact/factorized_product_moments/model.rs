//! Immutable, cold-compiled product-factorization programs.

use crate::algebra::Coefficient;
use crate::sector::{InteriorBounds, SectorInteriorDomain};

use super::super::factorized_numerator_lift::FactorizedNumeratorLiftCompilation;

#[derive(Debug)]
pub(super) struct SingletonProductBlock {
    pub(super) dependency_ordinal: usize,
    pub(super) parent_position: usize,
    pub(super) transformed_vector: usize,
    pub(super) active_power_ordinal: usize,
}

#[derive(Debug)]
pub(super) struct CorrelatedProductBlock {
    pub(super) dependency_ordinal: usize,
    pub(super) parent_positions: Box<[usize]>,
    pub(super) transformed_vectors: Box<[usize]>,
    pub(super) vector_signs: Box<[i64]>,
    /// Signs relating routed vectors to installed dependency vectors:
    /// `r_i = sign_i * t_i`.
    pub(super) active_power_start: usize,
    pub(super) moment_branches: Box<[Box<[CorrelatedMomentBranch]>]>,
}

#[derive(Clone, Debug)]
pub(super) struct CorrelatedMomentBranch {
    pub(super) coefficient: Coefficient,
    /// `None` is the affine constant; `Some(i)` multiplies dependency
    /// denominator `i` and therefore lowers that denominator power by one.
    pub(super) denominator: Option<usize>,
}

#[derive(Debug)]
pub(super) enum ProductBlockLayout {
    AllSingleton {
        singletons_by_vector: Box<[SingletonProductBlock]>,
    },
    OneCorrelated {
        correlated: CorrelatedProductBlock,
        singletons_by_vector: Box<[SingletonProductBlock]>,
    },
}

/// One scalar-product coordinate selected by a compact affine numerator
/// branch. The constant branch does not grow a moment exponent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProductMomentVariable {
    Constant,
    Radial(usize),
    Cross(usize),
}

/// Exact machine-independent exponent used by the product-moment executor.
///
/// Parent denominator powers are signed `i64`, but several routed numerator
/// coordinates can contribute to one angular incidence or dependency moment.
/// Their aggregate can therefore exceed `u64` even though every individual
/// input power is representable as `i64`.
pub(super) type MomentPower = u128;

/// One nonzero term of a routed parent denominator. Branch arrays are
/// compiled once and their width is independent of the requested rank.
#[derive(Clone, Debug)]
pub(super) struct ProductNumeratorBranch {
    pub(super) coefficient: Coefficient,
    pub(super) variable: ProductMomentVariable,
}

/// One exact root-domain preimage obligation for a nested dependency
/// denominator.  For a parent target `a`, the nested recurrence can lower the
/// base power `a[base_parent_position]` by every integer from zero through
/// `sum(-a[source])`.  Cold compilation proves both that this upper endpoint
/// is attained by direct moment branches and that angular routing cannot
/// exceed it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DependencyRootPreimage {
    base_parent_position: usize,
    dependency_bounds: InteriorBounds,
    shift_sources: Box<[usize]>,
}

impl DependencyRootPreimage {
    pub(super) fn new(
        base_parent_position: usize,
        dependency_bounds: InteriorBounds,
        shift_sources: Box<[usize]>,
    ) -> Self {
        Self {
            base_parent_position,
            dependency_bounds,
            shift_sources,
        }
    }

    pub(crate) const fn base_parent_position(&self) -> usize {
        self.base_parent_position
    }

    pub(crate) const fn dependency_bounds(&self) -> InteriorBounds {
        self.dependency_bounds
    }

    pub(crate) fn shift_sources(&self) -> &[usize] {
        &self.shift_sources
    }
}

/// Exact executor-safe subset of a mathematical factorized sector.
///
/// Its rectangular hull is useful only as a lookup filter.  The sparse rows
/// retain the coupled inactive-rank inequalities that ensure every nested
/// lower-artifact target stays inside that dependency's certified root.  In
/// particular, this type must not be replaced by independently tightened
/// coordinate bounds: aggregate numerator ranks make the exact preimage
/// nonrectangular.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ProductApplicationDomain {
    hull: SectorInteriorDomain,
    dependency_preimages: Box<[DependencyRootPreimage]>,
}

impl ProductApplicationDomain {
    pub(super) fn new(
        hull: SectorInteriorDomain,
        dependency_preimages: Box<[DependencyRootPreimage]>,
    ) -> Self {
        Self {
            hull,
            dependency_preimages,
        }
    }

    pub(crate) fn hull(&self) -> &SectorInteriorDomain {
        &self.hull
    }

    pub(crate) fn dependency_preimages(&self) -> &[DependencyRootPreimage] {
        &self.dependency_preimages
    }

    pub(crate) fn contains(&self, powers: &[i64]) -> Result<bool, crate::sector::Error> {
        if !self.hull.contains(powers)? {
            return Ok(false);
        }
        Ok(self.dependency_preimages.iter().all(|preimage| {
            let base = i128::from(powers[preimage.base_parent_position]);
            let maximum_shift = preimage.shift_sources.iter().fold(0_i128, |sum, &source| {
                // The hull has already proved every source inactive, hence
                // `powers[source] <= 0` and this subtraction cannot overflow
                // the deliberately wider accumulator.
                sum - i128::from(powers[source])
            });
            let minimum_nested = base - maximum_shift;
            let maximum_nested = base;
            i128::from(preimage.dependency_bounds.lower()) <= minimum_nested
                && maximum_nested <= i128::from(preimage.dependency_bounds.upper())
        }))
    }

    /// Whether every point of one rectangular carrier is in this exact
    /// nonrectangular domain.
    pub(crate) fn covers_domain(&self, target: &SectorInteriorDomain) -> bool {
        if target.arity() != self.hull.arity()
            || target.sector() != self.hull.sector()
            || target
                .bounds()
                .iter()
                .zip(self.hull.bounds())
                .any(|(&target, &hull)| {
                    target.lower() < hull.lower() || hull.upper() < target.upper()
                })
        {
            return false;
        }
        self.dependency_preimages.iter().all(|preimage| {
            let minimum_nested = preimage.shift_sources.iter().fold(
                i128::from(target.bounds()[preimage.base_parent_position].lower()),
                |sum, &source| sum + i128::from(target.bounds()[source].lower()),
            );
            let maximum_nested = i128::from(target.bounds()[preimage.base_parent_position].upper());
            i128::from(preimage.dependency_bounds.lower()) <= minimum_nested
                && maximum_nested <= i128::from(preimage.dependency_bounds.upper())
        })
    }

    /// Test a raw-domain rectangle after the authenticated permutation whose
    /// `source_for_target[t]` gives the raw coordinate feeding owner
    /// coordinate `t`.  This is allocation-free on immutable-owner lookup.
    pub(crate) fn covers_transported_domain(
        &self,
        raw: &SectorInteriorDomain,
        source_for_target: &[usize],
    ) -> bool {
        let arity = self.hull.arity();
        if raw.arity() != arity || source_for_target.len() != arity {
            return false;
        }
        for (owner_position, &raw_position) in source_for_target.iter().enumerate() {
            let Some(raw_bounds) = raw.bounds().get(raw_position).copied() else {
                return false;
            };
            let owner_bounds = self.hull.bounds()[owner_position];
            if raw_bounds.lower() < owner_bounds.lower()
                || owner_bounds.upper() < raw_bounds.upper()
                || raw.sector().active_bits()[raw_position]
                    != self.hull.sector().active_bits()[owner_position]
            {
                return false;
            }
        }
        self.dependency_preimages.iter().all(|preimage| {
            let base_raw = source_for_target[preimage.base_parent_position];
            let minimum_nested = preimage.shift_sources.iter().fold(
                i128::from(raw.bounds()[base_raw].lower()),
                |sum, &owner_source| {
                    let raw_source = source_for_target[owner_source];
                    sum + i128::from(raw.bounds()[raw_source].lower())
                },
            );
            let maximum_nested = i128::from(raw.bounds()[base_raw].upper());
            i128::from(preimage.dependency_bounds.lower()) <= minimum_nested
                && maximum_nested <= i128::from(preimage.dependency_bounds.upper())
        })
    }
}

/// Immutable program derived from one already authenticated factorization
/// recipe and its family. It deliberately owns no durable algebra payload:
/// artifact loading recompiles this process-local capability from the durable
/// rule, family, and lower artifacts after their normal authentication.
#[derive(Debug)]
pub(crate) struct FactorizedProductMomentProgram {
    pub(super) family_fingerprint: std::sync::Arc<String>,
    pub(super) routing: FactorizedNumeratorLiftCompilation,
    pub(super) layout: ProductBlockLayout,
    pub(super) active_parent_positions: Box<[usize]>,
    pub(super) edges: Box<[(usize, usize)]>,
    pub(super) numerator_branches: Box<[Box<[ProductNumeratorBranch]>]>,
    pub(super) normalization: Coefficient,
    pub(super) application_domain: ProductApplicationDomain,
}

impl FactorizedProductMomentProgram {
    /// Rectangular lookup hull only. Exact authority is
    /// [`Self::exact_application_domain`].
    pub(crate) fn application_hull(&self) -> &SectorInteriorDomain {
        self.application_domain.hull()
    }

    pub(crate) fn exact_application_domain(&self) -> &ProductApplicationDomain {
        &self.application_domain
    }

    pub(crate) fn contains(&self, powers: &[i64]) -> Result<bool, crate::sector::Error> {
        self.application_domain.contains(powers)
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    #[cfg(test)]
    pub(crate) fn branch_width(&self, parent_position: usize) -> Option<usize> {
        self.numerator_branches
            .get(parent_position)
            .map(|row| row.len())
    }

    pub(crate) fn loop_factor_count(&self) -> usize {
        self.routing.routing().signed_loop_basis().len().isqrt()
    }

    #[cfg(test)]
    pub(crate) fn cross_coordinate_count(&self) -> usize {
        self.edges.len()
    }

    #[cfg(test)]
    pub(crate) fn singleton_factor_count(&self) -> usize {
        match &self.layout {
            ProductBlockLayout::AllSingleton {
                singletons_by_vector,
            }
            | ProductBlockLayout::OneCorrelated {
                singletons_by_vector,
                ..
            } => singletons_by_vector.len(),
        }
    }

    #[cfg(test)]
    pub(crate) fn correlated_factor_loop_count(&self) -> Option<usize> {
        match &self.layout {
            ProductBlockLayout::AllSingleton { .. } => None,
            ProductBlockLayout::OneCorrelated { correlated, .. } => {
                Some(correlated.transformed_vectors.len())
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn signed_loop_basis(&self) -> &[i64] {
        self.routing.routing().signed_loop_basis()
    }
}

/// Runtime guards introduced by isotropic angular recurrences. They state the
/// generic-dimensional meromorphic domain represented by the exact result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductMomentGuard {
    vector: usize,
    rank: MomentPower,
    nonzero_polynomial: Coefficient,
}

impl ProductMomentGuard {
    pub(super) fn new(vector: usize, rank: MomentPower, nonzero_polynomial: Coefficient) -> Self {
        Self {
            vector,
            rank,
            nonzero_polynomial,
        }
    }

    #[cfg(test)]
    pub(crate) fn vector(&self) -> usize {
        self.vector
    }

    #[cfg(test)]
    pub(crate) fn rank(&self) -> MomentPower {
        self.rank
    }

    #[cfg(test)]
    pub(crate) fn nonzero_polynomial(&self) -> &Coefficient {
        &self.nonzero_polynomial
    }
}
