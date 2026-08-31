//! Exact coefficient systems for indexed-polynomial specialization.
//!
//! An indexed guard is a polynomial in algebraically independent base-field
//! parameters and integral indices.  After fixing the indices, that guard is
//! the zero base-field polynomial exactly when every coefficient of every
//! base-parameter monomial vanishes.  Symbolica owns the sparse polynomial
//! split; this module only authenticates and orders its result.

use std::collections::BTreeSet;

use symbolica::prelude::{Factorize, Integer};

use crate::algebra::{IndexedAlgebraError, IndexedAlgebraLimits};

use super::limits::{ceil_log2, check_limit, integer_magnitude_bits};

use super::{IndexedCoefficientContext, IndexedPolynomial};

/// Cold-path admission envelope for exact guard-locus decomposition.
///
/// Polynomial factorization has no useful allocation-free preflight. RustRed
/// therefore accepts it only for deliberately small univariate systems. The
/// work envelope includes the worst-case subset count in Symbolica's modular
/// factor recombination and a conservative coefficient-limb charge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedGuardLimits {
    /// Largest sparse guard admitted before splitting by base monomial.
    pub max_input_terms: usize,
    /// Largest simultaneous index-polynomial coefficient system.
    pub max_coefficient_equations: usize,
    /// Largest degree admitted to the univariate GCD/factor lane.
    pub max_univariate_degree: usize,
    /// Aggregate coefficient-magnitude bits admitted before and after native
    /// GCD/factorization.
    pub max_total_integer_bits: usize,
    /// Largest subset count admitted for modular-factor recombination.
    pub max_factor_recombination_subsets: usize,
    /// Largest coefficient-limb-weighted dense GCD/recombination work bound.
    pub max_gcd_factor_work: usize,
    /// Aggregate sparse terms admitted in a GCD or factor output.
    pub max_factor_terms: usize,
}

impl Default for IndexedGuardLimits {
    fn default() -> Self {
        Self {
            max_input_terms: 65_536,
            max_coefficient_equations: 4_096,
            max_univariate_degree: 16,
            max_total_integer_bits: 65_536,
            max_factor_recombination_subsets: 65_536,
            max_gcd_factor_work: 64_000_000,
            max_factor_terms: 4_096,
        }
    }
}

/// One coefficient in the expansion of an indexed polynomial in base
/// parameters.  `index_polynomial` has zero exponent in every base variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BaseCoefficientEquation {
    base_monomial: Box<[u16]>,
    index_polynomial: IndexedPolynomial,
}

impl BaseCoefficientEquation {
    // The completion subsystem is deliberately compiled before its staged
    // semantic-admission entry point is promoted into the public foundry.
    #[allow(dead_code)]
    pub(crate) fn base_monomial(&self) -> &[u16] {
        &self.base_monomial
    }

    #[allow(dead_code)]
    pub(crate) fn index_polynomial(&self) -> &IndexedPolynomial {
        &self.index_polynomial
    }

    /// A nonzero constant equation is an immediate certificate that the
    /// simultaneous zero locus, and hence the guard's exceptional locus, is
    /// empty.  The converse need not hold: several nonconstant equations can
    /// also generate the unit ideal.
    pub(crate) fn is_nonzero_constant(&self) -> bool {
        self.index_polynomial.is_nonzero_constant()
    }
}

/// Exact simultaneous index-polynomial equations defining where one guard
/// specializes to the zero polynomial in the base parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BaseCoefficientSystem {
    equations: Box<[BaseCoefficientEquation]>,
}

/// Exact integer zero set of a coefficient system which is either already
/// inconsistent or depends on one integral index only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UnivariateIntegerZeroSet {
    Empty,
    Finite {
        index_position: usize,
        roots: Box<[Integer]>,
    },
}

/// Result of the deliberately narrow exact integer-locus lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IntegerZeroSetResolution {
    IdenticallyZero,
    UnsupportedMultivariate,
    Exact(UnivariateIntegerZeroSet),
}

impl UnivariateIntegerZeroSet {
    pub(crate) fn index_position(&self) -> Option<usize> {
        match self {
            Self::Empty => None,
            Self::Finite { index_position, .. } => Some(*index_position),
        }
    }

    pub(crate) fn roots(&self) -> &[Integer] {
        match self {
            Self::Empty => &[],
            Self::Finite { roots, .. } => roots,
        }
    }
}

impl BaseCoefficientSystem {
    #[allow(dead_code)]
    pub(crate) fn equations(&self) -> &[BaseCoefficientEquation] {
        &self.equations
    }

    /// Cheap sufficient certificate for an empty exceptional locus.  A later
    /// Gröbner/Diophantine stratum owner may prove additional systems empty.
    pub(crate) fn has_nonzero_constant_equation(&self) -> bool {
        self.equations
            .iter()
            .any(BaseCoefficientEquation::is_nonzero_constant)
    }
}

impl IndexedCoefficientContext {
    /// Expand `value` in the authenticated base variables and retain its exact
    /// index-polynomial coefficients in deterministic monomial order.
    ///
    /// This is a cold foundry operation.  The input is authenticated once;
    /// Symbolica's native `to_multivariate_polynomial_list` performs the
    /// sparse split instead of RustRed duplicating polynomial arithmetic.
    pub(crate) fn base_coefficient_system(
        &self,
        value: &IndexedPolynomial,
        limits: IndexedAlgebraLimits,
        guard_limits: IndexedGuardLimits,
    ) -> Result<BaseCoefficientSystem, IndexedAlgebraError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        check_limit(
            "guard coefficient split input terms",
            value.raw().nterms(),
            guard_limits.max_input_terms,
        )?;
        check_limit(
            "guard coefficient split input integer bits",
            total_integer_bits(&value.raw().coefficients)?,
            guard_limits.max_total_integer_bits,
        )?;

        let base_count = self.base().variables().len();
        let mut base_positions = Vec::new();
        base_positions.try_reserve_exact(base_count).map_err(|_| {
            IndexedAlgebraError::AllocationFailure {
                resource: "base-coefficient variable positions",
                requested: base_count,
            }
        })?;
        base_positions.extend(0..base_count);

        let split = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            value
                .raw()
                .to_multivariate_polynomial_list(&base_positions, true)
        }))
        .map_err(|_| {
            IndexedAlgebraError::Symbolica(
                "Symbolica panicked while splitting indexed guard base coefficients".to_owned(),
            )
        })?;
        debug_assert!(split.len() <= value.raw().coefficients.len());
        check_limit(
            "guard coefficient equations",
            split.len(),
            guard_limits.max_coefficient_equations,
        )?;

        let mut equations = Vec::new();
        equations.try_reserve_exact(split.len()).map_err(|_| {
            IndexedAlgebraError::AllocationFailure {
                resource: "base-coefficient equations",
                requested: split.len(),
            }
        })?;
        for (monomial, index_polynomial) in split {
            debug_assert_eq!(monomial.len(), value.raw().variables.len());
            debug_assert!(monomial[base_count..].iter().all(|&power| power == 0));
            debug_assert!(
                index_polynomial
                    .exponents_iter()
                    .all(|exponents| { exponents[..base_count].iter().all(|&power| power == 0) })
            );

            let mut base_monomial = Vec::new();
            base_monomial.try_reserve_exact(base_count).map_err(|_| {
                IndexedAlgebraError::AllocationFailure {
                    resource: "base-coefficient monomial exponents",
                    requested: base_count,
                }
            })?;
            base_monomial.extend_from_slice(&monomial[..base_count]);
            equations.push(BaseCoefficientEquation {
                base_monomial: base_monomial.into_boxed_slice(),
                index_polynomial: IndexedPolynomial {
                    raw: index_polynomial,
                    context: self.fingerprint.clone(),
                },
            });
        }
        equations.sort_unstable_by(|left, right| left.base_monomial.cmp(&right.base_monomial));

        Ok(BaseCoefficientSystem {
            equations: equations.into_boxed_slice(),
        })
    }

    /// Resolve the exact common integer roots when `system` depends on at
    /// most one index. Identically-zero and genuinely multivariate systems
    /// remain distinct typed outcomes outside this deliberately narrow lane.
    ///
    /// Symbolica computes the polynomial GCD and factorization. RustRed only
    /// extracts integer roots from irreducible linear factors and replays
    /// every candidate against every original coefficient equation.
    pub(crate) fn univariate_integer_zero_set(
        &self,
        system: &BaseCoefficientSystem,
        limits: IndexedGuardLimits,
    ) -> Result<IntegerZeroSetResolution, IndexedAlgebraError> {
        if system.equations.is_empty() {
            return Ok(IntegerZeroSetResolution::IdenticallyZero);
        }
        for equation in &system.equations {
            self.validate_polynomial_context(&equation.index_polynomial)?;
        }
        if system.has_nonzero_constant_equation() {
            return Ok(IntegerZeroSetResolution::Exact(
                UnivariateIntegerZeroSet::Empty,
            ));
        }

        let base_count = self.base().variables().len();
        let mut support = BTreeSet::new();
        for equation in &system.equations {
            for exponents in equation.index_polynomial.raw().exponents_iter() {
                for (position, &power) in exponents[base_count..].iter().enumerate() {
                    if power != 0 {
                        support.insert(position);
                    }
                }
            }
        }
        if support.len() != 1 {
            return Ok(IntegerZeroSetResolution::UnsupportedMultivariate);
        }
        let index_position = *support.first().expect("one-element support");
        let variable_position = base_count + index_position;
        let max_degree = system
            .equations
            .iter()
            .map(|equation| usize::from(equation.index_polynomial.raw().degree(variable_position)))
            .max()
            .unwrap_or(0);
        check_limit(
            "guard univariate degree",
            max_degree,
            limits.max_univariate_degree,
        )?;
        let coefficient_bits = system
            .equations
            .iter()
            .try_fold(0usize, |total, equation| {
                total
                    .checked_add(total_integer_bits(
                        &equation.index_polynomial.raw().coefficients,
                    )?)
                    .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                        resource: "guard univariate coefficient bits",
                    })
            })?;
        check_limit(
            "guard univariate coefficient bits",
            coefficient_bits,
            limits.max_total_integer_bits,
        )?;
        let coefficient_limbs = integer_limb_count(max_integer_bits(
            system
                .equations
                .iter()
                .flat_map(|equation| equation.index_polynomial.raw().coefficients.iter()),
        ))?;
        let dense_slots =
            max_degree
                .checked_add(1)
                .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard gcd/factor work",
                })?;
        let dense_square = dense_slots.checked_mul(dense_slots).ok_or(
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard gcd/factor work",
            },
        )?;
        let gcd_work =
            checked_work_product(&[dense_square, system.equations.len(), coefficient_limbs])?;
        check_limit(
            "guard gcd/factor work",
            gcd_work,
            limits.max_gcd_factor_work,
        )?;

        let common = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut equations = system.equations.iter();
            let first = equations.next().expect("nonempty coefficient system");
            let mut common = first.index_polynomial.raw().clone();
            for equation in equations {
                common = common.gcd(equation.index_polynomial.raw());
                if common.is_constant() {
                    break;
                }
            }
            common
        }))
        .map_err(|_| {
            IndexedAlgebraError::Symbolica(
                "Symbolica panicked while computing an indexed guard coefficient GCD".to_owned(),
            )
        })?;
        if common.is_constant() {
            return Ok(IntegerZeroSetResolution::Exact(
                UnivariateIntegerZeroSet::Empty,
            ));
        }
        validate_factor_payload(&common, limits)?;
        let common_degree = usize::from(common.degree(variable_position));
        let recombination_subsets = 2usize
            .checked_pow(u32::try_from(common_degree).map_err(|_| {
                IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard factor recombination subsets",
                }
            })?)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard factor recombination subsets",
            })?;
        check_limit(
            "guard factor recombination subsets",
            recombination_subsets,
            limits.max_factor_recombination_subsets,
        )?;
        let common_max_bits = max_integer_bits(common.coefficients.iter());
        // A coarse Mignotte bound gives every coefficient of an integer
        // factor at most 2^degree * sqrt(degree+1) times the input height.
        // The extra full log2(degree+1) and two sign/slack bits deliberately
        // overbound the square-root term before native factorization.
        let prospective_coefficient_bits = common_max_bits
            .checked_add(common_degree)
            .and_then(|value| value.checked_add(ceil_log2(common_degree.saturating_add(1))))
            .and_then(|value| value.checked_add(2))
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard prospective factor integer bits",
            })?;
        let prospective_factor_terms = common_degree
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard prospective factor terms",
            })?;
        check_limit(
            "guard prospective factor terms",
            prospective_factor_terms,
            limits.max_factor_terms,
        )?;
        let prospective_factor_bits = prospective_factor_terms
            .checked_mul(prospective_coefficient_bits)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard prospective factor integer bits",
            })?;
        check_limit(
            "guard prospective factor integer bits",
            prospective_factor_bits,
            limits.max_total_integer_bits,
        )?;
        let common_dense_slots =
            common_degree
                .checked_add(1)
                .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard gcd/factor work",
                })?;
        let common_dense_square = common_dense_slots.checked_mul(common_dense_slots).ok_or(
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard gcd/factor work",
            },
        )?;
        let factor_work = checked_work_product(&[
            recombination_subsets,
            common_dense_square,
            integer_limb_count(prospective_coefficient_bits)?,
        ])?;
        let work = gcd_work.checked_add(factor_work).ok_or(
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard gcd/factor work",
            },
        )?;
        check_limit("guard gcd/factor work", work, limits.max_gcd_factor_work)?;

        let mut roots = Vec::new();
        let factors = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| common.factor()))
            .map_err(|_| {
                IndexedAlgebraError::Symbolica(
                    "Symbolica panicked while factoring an indexed guard coefficient".to_owned(),
                )
            })?;
        let factor_terms = factors.iter().try_fold(0usize, |total, (factor, _)| {
            total
                .checked_add(factor.nterms())
                .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard factor terms",
                })
        })?;
        check_limit("guard factor terms", factor_terms, limits.max_factor_terms)?;
        let factor_bits = factors.iter().try_fold(0usize, |total, (factor, _)| {
            total
                .checked_add(total_integer_bits(&factor.coefficients)?)
                .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard factor integer bits",
                })
        })?;
        check_limit(
            "guard factor integer bits",
            factor_bits,
            limits.max_total_integer_bits,
        )?;
        for (factor, _multiplicity) in factors {
            if factor.is_constant() || factor.degree(variable_position) != 1 {
                continue;
            }
            debug_assert!(
                (0..factor.nvars()).all(|position| {
                    position == variable_position || factor.degree(position) == 0
                })
            );
            let univariate = factor.to_univariate_from_univariate(variable_position);
            debug_assert_eq!(univariate.coefficients.len(), 2);
            let constant = univariate.coefficients[0].clone();
            let leading = univariate.coefficients[1].clone();
            let numerator = -constant;
            let root = numerator.clone() / leading.clone();
            let replays = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                system.equations.iter().all(|equation| {
                    equation
                        .index_polynomial
                        .raw()
                        .replace(variable_position, &root)
                        .is_zero()
                })
            }))
            .map_err(|_| {
                IndexedAlgebraError::Symbolica(
                    "Symbolica panicked while replaying an exceptional integer root".to_owned(),
                )
            })?;
            if root.clone() * leading != numerator || !replays {
                continue;
            }
            roots
                .try_reserve_exact(1)
                .map_err(|_| IndexedAlgebraError::AllocationFailure {
                    resource: "univariate exceptional integer roots",
                    requested: roots.len().saturating_add(1),
                })?;
            roots.push(root);
        }
        roots.sort_unstable();
        roots.dedup();
        if roots.is_empty() {
            Ok(IntegerZeroSetResolution::Exact(
                UnivariateIntegerZeroSet::Empty,
            ))
        } else {
            Ok(IntegerZeroSetResolution::Exact(
                UnivariateIntegerZeroSet::Finite {
                    index_position,
                    roots: roots.into_boxed_slice(),
                },
            ))
        }
    }
}

fn total_integer_bits(values: &[Integer]) -> Result<usize, IndexedAlgebraError> {
    values.iter().try_fold(0usize, |total, value| {
        let bits = usize::try_from(integer_magnitude_bits(value)).map_err(|_| {
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard integer coefficient bits",
            }
        })?;
        total
            .checked_add(bits)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard integer coefficient bits",
            })
    })
}

fn max_integer_bits<'a>(values: impl IntoIterator<Item = &'a Integer>) -> usize {
    values
        .into_iter()
        .map(integer_magnitude_bits)
        .map(|bits| usize::try_from(bits).unwrap_or(usize::MAX))
        .max()
        .unwrap_or(0)
}

fn integer_limb_count(bits: usize) -> Result<usize, IndexedAlgebraError> {
    let word_bits = usize::BITS as usize;
    bits.checked_add(word_bits.saturating_sub(1))
        .map(|rounded| rounded / word_bits)
        .map(|limbs| limbs.max(1))
        .ok_or(IndexedAlgebraError::ResourceCountOverflow {
            resource: "guard coefficient limbs",
        })
}

fn checked_work_product(factors: &[usize]) -> Result<usize, IndexedAlgebraError> {
    factors.iter().try_fold(1usize, |product, &factor| {
        product
            .checked_mul(factor)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard gcd/factor work",
            })
    })
}

fn validate_factor_payload(
    polynomial: &crate::algebra::CoefficientPolynomial,
    limits: IndexedGuardLimits,
) -> Result<(), IndexedAlgebraError> {
    check_limit(
        "guard factor terms",
        polynomial.nterms(),
        limits.max_factor_terms,
    )?;
    check_limit(
        "guard factor integer bits",
        total_integer_bits(&polynomial.coefficients)?,
        limits.max_total_integer_bits,
    )
}
