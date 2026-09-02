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
    /// Largest number of index variables admitted to one native
    /// multivariate factorization in the separable-factor lane.
    pub max_factor_variables: usize,
    /// Largest total degree admitted to one native multivariate
    /// factorization in the separable-factor lane.
    pub max_factor_total_degree: usize,
    /// Largest dense monomial box admitted before a native multivariate
    /// factorization. This bounds the product of `(degree + 1)` over the
    /// participating index variables even when the input itself is sparse.
    pub max_factor_dense_slots: usize,
    /// Aggregate coefficient-magnitude bits admitted before and after native
    /// GCD/factorization.
    pub max_total_integer_bits: usize,
    /// Largest subset count admitted for modular-factor recombination.
    pub max_factor_recombination_subsets: usize,
    /// Largest coefficient-limb-weighted dense GCD/recombination work bound.
    pub max_gcd_factor_work: usize,
    /// Aggregate sparse terms admitted in a GCD or factor output.
    pub max_factor_terms: usize,
    /// Aggregate number of all-equation substitutions admitted while proving
    /// candidate root hyperplanes exact.  This is charged before entering
    /// Symbolica's allocation-owning `replace` operation.
    pub max_exact_hyperplane_replay_substitutions: usize,
    /// Aggregate input terms traversed by those substitutions. Replacing one
    /// variable by an integer cannot increase the number of input monomials,
    /// so this is a conservative allocation/work preflight for the complete
    /// all-equation replay.
    pub max_exact_hyperplane_replay_terms: usize,
    /// Aggregate coefficient-bit work admitted for all-equation root replay.
    /// The preflight charges prospective substituted coefficient size and
    /// binary-exponentiation depth for every input monomial.
    pub max_exact_hyperplane_replay_work: usize,
}

impl Default for IndexedGuardLimits {
    fn default() -> Self {
        Self {
            max_input_terms: 65_536,
            max_coefficient_equations: 4_096,
            max_univariate_degree: 16,
            max_factor_variables: 8,
            max_factor_total_degree: 16,
            max_factor_dense_slots: 4_096,
            max_total_integer_bits: 65_536,
            max_factor_recombination_subsets: 65_536,
            max_gcd_factor_work: 64_000_000,
            max_factor_terms: 4_096,
            max_exact_hyperplane_replay_substitutions: 65_536,
            max_exact_hyperplane_replay_terms: 16_000_000,
            max_exact_hyperplane_replay_work: 64_000_000,
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

/// One separable integer hyperplane covering a guard's exceptional locus.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct IntegerRootHyperplane {
    index_position: usize,
    root: Integer,
}

impl IntegerRootHyperplane {
    pub(crate) const fn index_position(&self) -> usize {
        self.index_position
    }

    pub(crate) const fn root(&self) -> &Integer {
        &self.root
    }
}

/// Bounded exact/separable information about a guard locus in one integer
/// domain. A conservative cover proves only that the exceptional locus is a
/// subset of the listed hyperplanes. Exact hyperplanes additionally replay as
/// identically zero in every base-coefficient equation, so their union equals
/// the exceptional locus inside the caller's domain. Coupled irreducible
/// geometry remains fail-closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IntegerZeroLocusDomainResolution {
    IdenticallyZero,
    MissesDomain,
    IntersectsExactHyperplanes(Box<[IntegerRootHyperplane]>),
    IntersectsConservativeCover(Box<[IntegerRootHyperplane]>),
    UnsupportedCoupled,
}

impl UnivariateIntegerZeroSet {
    #[cfg(test)]
    pub(crate) fn index_position(&self) -> Option<usize> {
        match self {
            Self::Empty => None,
            Self::Finite { index_position, .. } => Some(*index_position),
        }
    }

    #[cfg(test)]
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
        let mut replay_substitutions = 0usize;
        let mut replay_terms = 0usize;
        let mut replay_work = 0usize;
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
            let Some(root) = exact_linear_integer_root(&factor, variable_position) else {
                continue;
            };
            charge_exact_hyperplane_replay(
                system,
                base_count,
                std::iter::once((index_position, &root)),
                &mut replay_substitutions,
                &mut replay_terms,
                &mut replay_work,
                limits,
            )?;
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
            if !replays {
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

    /// Conservatively prove that the simultaneous zero locus of `system`
    /// misses a caller-owned integer domain.
    ///
    /// `domain_contains_root(position, root)` must return `true` exactly when
    /// the supplied integer root belongs to the domain at that index
    /// position. The callback lets the sector and foundry layers retain
    /// ownership of their different box-coordinate conventions.
    ///
    /// The exact univariate lane is used first. For a genuinely multivariate
    /// system, RustRed asks Symbolica to factor individual coefficient
    /// equations. One equation that decomposes completely into nonzero
    /// constant or univariate irreducible factors and has no integer root in
    /// the domain is enough to prove that the *simultaneous* locus is empty.
    /// Coupled irreducible factors and every other unresolved case fail
    /// closed and return `false`.
    pub(crate) fn integer_zero_locus_misses_domain(
        &self,
        system: &BaseCoefficientSystem,
        limits: IndexedGuardLimits,
        domain_contains_root: impl FnMut(usize, &Integer) -> bool,
    ) -> Result<bool, IndexedAlgebraError> {
        Ok(matches!(
            self.integer_zero_locus_domain_resolution(system, limits, domain_contains_root)?,
            IntegerZeroLocusDomainResolution::MissesDomain
        ))
    }

    /// Resolve a bounded separable hyperplane cover of the exceptional locus
    /// which actually intersects a caller-owned integer domain.
    ///
    /// Symbolica performs every GCD and factorization. For a multivariate base
    /// coefficient system, one completely separable coefficient equation is
    /// enough: its root hyperplanes conservatively cover the simultaneous
    /// zero locus. Coupled irreducible factors remain typed unsupported.
    pub(crate) fn integer_zero_locus_domain_resolution(
        &self,
        system: &BaseCoefficientSystem,
        limits: IndexedGuardLimits,
        mut domain_contains_root: impl FnMut(usize, &Integer) -> bool,
    ) -> Result<IntegerZeroLocusDomainResolution, IndexedAlgebraError> {
        match self.univariate_integer_zero_set(system, limits)? {
            IntegerZeroSetResolution::IdenticallyZero => {
                return Ok(IntegerZeroLocusDomainResolution::IdenticallyZero);
            }
            IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Empty) => {
                return Ok(IntegerZeroLocusDomainResolution::MissesDomain);
            }
            IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Finite {
                index_position,
                roots,
            }) => {
                let intersections = roots
                    .into_vec()
                    .into_iter()
                    .filter(|root| domain_contains_root(index_position, root))
                    .map(|root| IntegerRootHyperplane {
                        index_position,
                        root,
                    })
                    .collect::<Vec<_>>();
                return Ok(if intersections.is_empty() {
                    IntegerZeroLocusDomainResolution::MissesDomain
                } else {
                    IntegerZeroLocusDomainResolution::IntersectsExactHyperplanes(
                        intersections.into_boxed_slice(),
                    )
                });
            }
            IntegerZeroSetResolution::UnsupportedMultivariate => {}
        }

        let base_count = self.base().variables().len();
        let mut charged_work = 0usize;
        let mut replay_substitutions = 0usize;
        let mut replay_terms = 0usize;
        let mut replay_work = 0usize;
        let mut conservative_cover = None;
        for equation in &system.equations {
            self.validate_polynomial_context(&equation.index_polynomial)?;
            let polynomial = equation.index_polynomial.raw();
            if polynomial.is_zero() {
                continue;
            }
            if polynomial.is_constant() {
                // The cheap system-wide check normally catches this. Keep the
                // local branch so the certificate remains fail-safe if the
                // system representation is extended later.
                return Ok(IntegerZeroLocusDomainResolution::MissesDomain);
            }

            let factor_work = separable_factor_work(polynomial, base_count, limits)?;
            charged_work = charged_work.checked_add(factor_work).ok_or(
                IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard separable factor work",
                },
            )?;
            check_limit(
                "guard separable factor work",
                charged_work,
                limits.max_gcd_factor_work,
            )?;

            let factors =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| polynomial.factor()))
                    .map_err(|_| {
                        IndexedAlgebraError::Symbolica(
                    "Symbolica panicked while factoring a multivariate indexed guard coefficient"
                        .to_owned(),
                )
                    })?;
            if factors.is_empty() {
                continue;
            }
            validate_factorization_output(&factors, limits)?;

            let mut equation_is_separable = true;
            let mut intersections = Vec::new();
            intersections
                .try_reserve_exact(factors.len())
                .map_err(|_| IndexedAlgebraError::AllocationFailure {
                    resource: "guard separable root hyperplanes",
                    requested: factors.len(),
                })?;
            for (factor, _multiplicity) in &factors {
                if factor.is_zero() {
                    equation_is_separable = false;
                    break;
                }
                let support = index_support(factor, base_count);
                let Some(index_position) = (match support.as_slice() {
                    [] => continue,
                    [position] => Some(*position),
                    _ => None,
                }) else {
                    // Symbolica found a genuinely coupled irreducible factor.
                    equation_is_separable = false;
                    break;
                };
                let variable_position = base_count + index_position;
                if factor.degree(variable_position) != 1 {
                    // Symbolica's complete factorization contract implies
                    // that an irreducible higher-degree univariate factor has
                    // no integer (and hence no rational) root.
                    continue;
                }
                let root = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    exact_linear_integer_root(factor, variable_position)
                }))
                .map_err(|_| {
                    IndexedAlgebraError::Symbolica(
                        "Symbolica panicked while extracting a separable guard root".to_owned(),
                    )
                })?;
                let Some(root) = root else {
                    // A nonintegral rational root does not meet an integer
                    // domain.
                    continue;
                };
                let replays = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    factor.replace(variable_position, &root).is_zero()
                }))
                .map_err(|_| {
                    IndexedAlgebraError::Symbolica(
                        "Symbolica panicked while replaying a separable guard root".to_owned(),
                    )
                })?;
                if !replays {
                    equation_is_separable = false;
                    break;
                }
                if domain_contains_root(index_position, &root) {
                    intersections.push(IntegerRootHyperplane {
                        index_position,
                        root,
                    });
                }
            }
            if !equation_is_separable {
                continue;
            }

            intersections.sort_unstable();
            intersections.dedup();
            if intersections.is_empty() {
                return Ok(IntegerZeroLocusDomainResolution::MissesDomain);
            }

            // Factorization of this equation proves that the simultaneous
            // exceptional locus is contained in the union of these
            // hyperplanes. Cell splitting needs the converse as well: every
            // retained hyperplane must make *all* base-coefficient equations
            // vanish identically. Without that replay the cover may strictly
            // overapproximate a codimension-two (or coupled) locus.
            charge_exact_hyperplane_replay(
                system,
                base_count,
                intersections
                    .iter()
                    .map(|root| (root.index_position(), root.root())),
                &mut replay_substitutions,
                &mut replay_terms,
                &mut replay_work,
                limits,
            )?;
            let mut every_hyperplane_is_exact = true;
            for root in &intersections {
                let variable_position = base_count + root.index_position;
                let replays_all = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    system.equations.iter().all(|candidate| {
                        candidate
                            .index_polynomial
                            .raw()
                            .replace(variable_position, &root.root)
                            .is_zero()
                    })
                }))
                .map_err(|_| {
                    IndexedAlgebraError::Symbolica(
                        "Symbolica panicked while proving an exact exceptional hyperplane"
                            .to_owned(),
                    )
                })?;
                if !replays_all {
                    every_hyperplane_is_exact = false;
                    break;
                }
            }
            if every_hyperplane_is_exact {
                return Ok(
                    IntegerZeroLocusDomainResolution::IntersectsExactHyperplanes(
                        intersections.into_boxed_slice(),
                    ),
                );
            }
            if conservative_cover
                .as_ref()
                .is_none_or(|existing: &Vec<IntegerRootHyperplane>| {
                    intersections.len() < existing.len()
                        || (intersections.len() == existing.len()
                            && intersections.as_slice() < existing.as_slice())
                })
            {
                conservative_cover = Some(intersections);
            }
        }
        Ok(match conservative_cover {
            Some(roots) => IntegerZeroLocusDomainResolution::IntersectsConservativeCover(
                roots.into_boxed_slice(),
            ),
            None => IntegerZeroLocusDomainResolution::UnsupportedCoupled,
        })
    }
}

/// Charge the complete root-by-equation replay before any Symbolica
/// substitution is entered.  The counters span the whole locus-resolution
/// call, rather than resetting for each factorized coefficient equation.
fn charge_exact_hyperplane_replay<'a, I>(
    system: &BaseCoefficientSystem,
    base_count: usize,
    mut roots: I,
    aggregate_substitutions: &mut usize,
    aggregate_terms: &mut usize,
    aggregate_work: &mut usize,
    limits: IndexedGuardLimits,
) -> Result<(), IndexedAlgebraError>
where
    I: Clone + ExactSizeIterator<Item = (usize, &'a Integer)>,
{
    let root_count = roots.len();
    let substitutions = root_count.checked_mul(system.equations.len()).ok_or(
        IndexedAlgebraError::ResourceCountOverflow {
            resource: "guard exact-hyperplane replay substitutions",
        },
    )?;
    let terms_per_root = system
        .equations
        .iter()
        .try_fold(0usize, |total, equation| {
            total
                .checked_add(equation.index_polynomial.raw().nterms())
                .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard exact-hyperplane replay terms",
                })
        })?;
    let terms = root_count.checked_mul(terms_per_root).ok_or(
        IndexedAlgebraError::ResourceCountOverflow {
            resource: "guard exact-hyperplane replay terms",
        },
    )?;
    let work = roots.try_fold(0usize, |root_total, (index_position, root)| {
        let root_bits = usize::try_from(integer_magnitude_bits(root)).map_err(|_| {
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard exact-hyperplane replay work",
            }
        })?;
        system
            .equations
            .iter()
            .try_fold(root_total, |equation_total, equation| {
                equation
                    .index_polynomial
                    .raw()
                    .coefficients
                    .iter()
                    .zip(equation.index_polynomial.raw().exponents_iter())
                    .try_fold(equation_total, |term_total, (coefficient, exponents)| {
                        let variable_position = base_count.checked_add(index_position).ok_or(
                            IndexedAlgebraError::ResourceCountOverflow {
                                resource: "guard exact-hyperplane replay work",
                            },
                        )?;
                        let exponent = usize::from(exponents[variable_position]);
                        let powered_bits = exponent.checked_mul(root_bits).ok_or(
                            IndexedAlgebraError::ResourceCountOverflow {
                                resource: "guard exact-hyperplane replay work",
                            },
                        )?;
                        let coefficient_bits = usize::try_from(integer_magnitude_bits(coefficient))
                            .map_err(|_| IndexedAlgebraError::ResourceCountOverflow {
                                resource: "guard exact-hyperplane replay work",
                            })?;
                        let substituted_bits = coefficient_bits
                            .checked_add(powered_bits)
                            .and_then(|value| value.checked_add(1))
                            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                                resource: "guard exact-hyperplane replay work",
                            })?;
                        let exponentiation_depth = ceil_log2(exponent.saturating_add(1))
                            .checked_add(1)
                            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                                resource: "guard exact-hyperplane replay work",
                            })?;
                        let term_work = substituted_bits.checked_mul(exponentiation_depth).ok_or(
                            IndexedAlgebraError::ResourceCountOverflow {
                                resource: "guard exact-hyperplane replay work",
                            },
                        )?;
                        term_total.checked_add(term_work).ok_or(
                            IndexedAlgebraError::ResourceCountOverflow {
                                resource: "guard exact-hyperplane replay work",
                            },
                        )
                    })
            })
    })?;
    let prospective_substitutions = aggregate_substitutions.checked_add(substitutions).ok_or(
        IndexedAlgebraError::ResourceCountOverflow {
            resource: "guard exact-hyperplane replay substitutions",
        },
    )?;
    let prospective_terms =
        aggregate_terms
            .checked_add(terms)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard exact-hyperplane replay terms",
            })?;
    let prospective_work =
        aggregate_work
            .checked_add(work)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard exact-hyperplane replay work",
            })?;
    check_limit(
        "guard exact-hyperplane replay substitutions",
        prospective_substitutions,
        limits.max_exact_hyperplane_replay_substitutions,
    )?;
    check_limit(
        "guard exact-hyperplane replay terms",
        prospective_terms,
        limits.max_exact_hyperplane_replay_terms,
    )?;
    check_limit(
        "guard exact-hyperplane replay work",
        prospective_work,
        limits.max_exact_hyperplane_replay_work,
    )?;
    *aggregate_substitutions = prospective_substitutions;
    *aggregate_terms = prospective_terms;
    *aggregate_work = prospective_work;
    Ok(())
}

fn exact_linear_integer_root(
    factor: &crate::algebra::CoefficientPolynomial,
    variable_position: usize,
) -> Option<Integer> {
    let univariate = factor.to_univariate_from_univariate(variable_position);
    debug_assert_eq!(univariate.coefficients.len(), 2);
    let constant = univariate.coefficients[0].clone();
    let leading = univariate.coefficients[1].clone();
    let numerator = -constant;
    let root = numerator.clone() / leading.clone();
    (root.clone() * leading == numerator).then_some(root)
}

fn index_support(
    polynomial: &crate::algebra::CoefficientPolynomial,
    base_count: usize,
) -> Vec<usize> {
    (base_count..polynomial.nvars())
        .filter(|&position| polynomial.degree(position) != 0)
        .map(|position| position - base_count)
        .collect()
}

fn separable_factor_work(
    polynomial: &crate::algebra::CoefficientPolynomial,
    base_count: usize,
    limits: IndexedGuardLimits,
) -> Result<usize, IndexedAlgebraError> {
    validate_factor_payload(polynomial, limits)?;
    let degrees = (base_count..polynomial.nvars())
        .map(|position| usize::from(polynomial.degree(position)))
        .filter(|&degree| degree != 0)
        .collect::<Vec<_>>();
    check_limit(
        "guard factor variables",
        degrees.len(),
        limits.max_factor_variables,
    )?;
    let max_degree = degrees.iter().copied().max().unwrap_or(0);
    check_limit(
        "guard factor per-variable degree",
        max_degree,
        limits.max_univariate_degree,
    )?;
    let total_degree = degrees.iter().try_fold(0usize, |total, &degree| {
        total
            .checked_add(degree)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard factor total degree",
            })
    })?;
    check_limit(
        "guard factor total degree",
        total_degree,
        limits.max_factor_total_degree,
    )?;
    let dense_slots = degrees.iter().try_fold(1usize, |slots, &degree| {
        slots
            .checked_mul(degree.checked_add(1).ok_or(
                IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard factor dense slots",
                },
            )?)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard factor dense slots",
            })
    })?;
    check_limit(
        "guard factor dense slots",
        dense_slots,
        limits.max_factor_dense_slots,
    )?;
    let recombination_subsets = 2usize
        .checked_pow(u32::try_from(total_degree).map_err(|_| {
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

    // A product of `total_degree` factors can each occupy at most the input
    // dense monomial box. This intentionally loose envelope prevents native
    // factorization from being entered when a sparse input could expand into
    // an unbounded output.
    let prospective_factor_terms = dense_slots.checked_mul(total_degree.max(1)).ok_or(
        IndexedAlgebraError::ResourceCountOverflow {
            resource: "guard prospective factor terms",
        },
    )?;
    check_limit(
        "guard prospective factor terms",
        prospective_factor_terms,
        limits.max_factor_terms,
    )?;
    let prospective_coefficient_bits = max_integer_bits(polynomial.coefficients.iter())
        .checked_add(total_degree)
        .and_then(|value| value.checked_add(ceil_log2(dense_slots)))
        .and_then(|value| value.checked_add(2))
        .ok_or(IndexedAlgebraError::ResourceCountOverflow {
            resource: "guard prospective factor integer bits",
        })?;
    let prospective_total_bits = prospective_factor_terms
        .checked_mul(prospective_coefficient_bits)
        .ok_or(IndexedAlgebraError::ResourceCountOverflow {
            resource: "guard prospective factor integer bits",
        })?;
    check_limit(
        "guard prospective factor integer bits",
        prospective_total_bits,
        limits.max_total_integer_bits,
    )?;

    let dense_square =
        dense_slots
            .checked_mul(dense_slots)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard separable factor work",
            })?;
    checked_work_product(&[
        recombination_subsets,
        dense_square,
        integer_limb_count(prospective_coefficient_bits)?,
        degrees.len().max(1),
    ])
}

fn validate_factorization_output(
    factors: &[(crate::algebra::CoefficientPolynomial, usize)],
    limits: IndexedGuardLimits,
) -> Result<(), IndexedAlgebraError> {
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
    )
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
