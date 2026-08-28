//! Certified finite-box reduction for the equal-mass three-loop vacuum.
//!
//! Within the configured total-dot and numerator-degree box this layer
//! validates every symmetry-unique non-scaleless target at construction time.
//! Missing sparse rules are errors, never implicitly promoted to masters.
//! The five permitted terminals are fixed **candidate** masters for this
//! bounded certificate; this module does not claim that they form a proven
//! minimal master basis of the unrestricted three-loop family.

use std::fmt;

use crate::ibp::{IbpGenerator, IbpIdentity};
use crate::legacy_oracle_support::validate_reduction_table_identity_provenance;
use crate::reduction::{
    ReductionError, ReductionStats, ReductionTable, SeedConfig, SeedGenerationError,
    SeedGenerationLimits, SparseReducer, try_generate_seeds_with_limits,
};
use crate::three_loop::equal_mass_three_loop_tetrahedron;
use crate::three_loop_boundary::{
    ThreeLoopBoundaryConfig, ThreeLoopBoundaryError, ThreeLoopBoundaryReducer,
};
use crate::{FamilyError, IbpGenerationError, Integral, LinearCombination, VacuumFamily};

/// Coverage and resource bounds of one certified three-loop table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreeLoopReductionConfig {
    /// Maximum `sum(max(a_i-1,0))` accepted by the public reduction surface.
    pub max_dots: u32,
    /// Maximum total degree carried by non-positive denominator powers.
    pub max_numerator_degree: u32,
    /// Maximum number of raw seed candidates visited before symmetry and zero
    /// sector filtering.
    pub max_seed_candidates: usize,
    /// Maximum recurrence steps in a one-loop tadpole factor.
    pub max_tadpole_steps: usize,
    /// Dot coverage of the cached two-loop pipeline used by paw boundaries.
    pub max_two_loop_dots: u32,
    /// Seed-candidate limit of the induced two-loop pipeline.
    pub max_two_loop_seed_candidates: usize,
    /// Analytic two-loop boundary iteration limit.
    pub max_two_loop_boundary_terms: usize,
}

impl Default for ThreeLoopReductionConfig {
    fn default() -> Self {
        Self {
            // A nontrivial, fast default containing both dotted integrals and
            // irreducible-numerator representatives.  Larger finite boxes are
            // accepted and receive the same exhaustive construction check.
            max_dots: 1,
            max_numerator_degree: 1,
            max_seed_candidates: 1_000,
            max_tadpole_steps: 10_000,
            // Generated IBPs have a one-dot halo around the advertised box,
            // including in paw boundary sectors.
            max_two_loop_dots: 2,
            max_two_loop_seed_candidates: 100,
            max_two_loop_boundary_terms: 1_000_000,
        }
    }
}

impl ThreeLoopReductionConfig {
    fn boundary(self) -> Result<ThreeLoopBoundaryConfig, ThreeLoopPipelineError> {
        let (_, induced_numerator_halo) = induced_two_loop_halo(self)?;
        Ok(ThreeLoopBoundaryConfig {
            // A generated IBP can create one additional inactive numerator
            // power beyond the advertised target box.  The analytic boundary
            // layer must close that halo rather than promote it to a master.
            max_numerator_degree: induced_numerator_halo,
            max_polynomial_terms: 1_000_000,
            max_polynomial_operations: 10_000_000,
            max_angular_terms: 1_000_000,
            max_tadpole_steps: self.max_tadpole_steps,
            max_two_loop_dots: self.max_two_loop_dots,
            max_two_loop_seed_candidates: self.max_two_loop_seed_candidates,
            max_two_loop_boundary_terms: self.max_two_loop_boundary_terms,
        })
    }
}

/// A sparse three-loop table closed by exact factorized-sector formulae.
#[derive(Clone, Debug)]
pub struct ThreeLoopReductionPipeline {
    table: ReductionTable,
    boundary: ThreeLoopBoundaryReducer,
    config: ThreeLoopReductionConfig,
    masters: [Integral; 5],
}

impl ThreeLoopReductionPipeline {
    /// Build and certify the built-in equal-mass tetrahedron family.
    pub fn build(config: ThreeLoopReductionConfig) -> Result<Self, ThreeLoopPipelineError> {
        Self::build_for_family(equal_mass_three_loop_tetrahedron()?, config)
    }

    /// Generate a sparse table, then validate its full advertised finite box.
    pub fn build_for_family(
        family: VacuumFamily,
        config: ThreeLoopReductionConfig,
    ) -> Result<Self, ThreeLoopPipelineError> {
        validate_config(config)?;
        // Authenticate the family as the built-in equal-mass tetrahedron
        // before enumerating targets, generating any three-loop IBP, or
        // entering sparse elimination.  Besides avoiding wasted work, this
        // prevents an unrelated seed/resource error from masking a topology
        // mismatch on this topology-specific public surface.
        let boundary = ThreeLoopBoundaryReducer::new(family.clone(), config.boundary()?)?;
        // Public targets and solver seeds are distinct sets.  Tree and paw
        // IBPs cannot activate an inactive propagator (every raising term is
        // proportional to that propagator's current index), so such rows can
        // only stay in a factorized sector or enter a proper subsector.  They
        // cannot provide a pivot in masks 43, 31, or 63 and belong entirely to
        // the exact boundary service.
        let targets = certified_targets(&family, config)?;
        let seeds = targets
            .iter()
            .filter(|seed| is_genuine_seed(&family, seed))
            .cloned()
            .collect::<Vec<_>>();
        let identities = IbpGenerator::new(&family).try_generate_for_seeds(&seeds)?;
        let table = SparseReducer::new(family).reduce(&identities)?;
        let pipeline = Self::assemble_validated(table, boundary, config);
        pipeline.validate_targets(&targets)?;
        // Validate exactly the equations used to build the table.  This path
        // intentionally permits the one-step IBP halo outside the input box.
        pipeline.validate_identities_unbounded(&identities)?;
        Ok(pipeline)
    }

    /// Compose an externally loaded sparse table with boundary closure and
    /// check target coverage, without claiming algebraic provenance.
    ///
    /// A serialized [`ReductionTable`] currently stores triangular rules but
    /// not their input-row hashes or elimination derivation.  Coverage cannot
    /// prove that an arbitrary loaded coefficient is a consequence of IBP.
    /// Replaying authenticated identities with [`Self::validate_identities`]
    /// can detect an incompatible table, but cannot prove how arbitrary loaded
    /// rules were derived.  A certified loaded path will require either a
    /// deterministic rebuild-and-compare or a persisted derivation proof.
    pub fn from_table_unchecked(
        table: ReductionTable,
        config: ThreeLoopReductionConfig,
    ) -> Result<Self, ThreeLoopPipelineError> {
        validate_config(config)?;
        let boundary = ThreeLoopBoundaryReducer::new(table.family().clone(), config.boundary()?)?;
        let pipeline = Self::assemble_validated(table, boundary, config);
        pipeline.validate_coverage()?;
        Ok(pipeline)
    }

    fn assemble_validated(
        table: ReductionTable,
        boundary: ThreeLoopBoundaryReducer,
        config: ThreeLoopReductionConfig,
    ) -> Self {
        // Representatives of the five fixed candidate master classes:
        // three tadpoles, sunset*tadpole, the four-edge cycle, the five-edge
        // sector, and the tetrahedron top sector.
        let masters = [
            Integral::from([1, 1, 1, 0, 0, 0]), // mask 7
            Integral::from([1, 1, 1, 1, 0, 0]), // mask 15
            Integral::from([1, 1, 0, 1, 0, 1]), // mask 43
            Integral::from([1, 1, 1, 1, 1, 0]), // mask 31
            Integral::from([1, 1, 1, 1, 1, 1]), // mask 63
        ];
        Self {
            table,
            boundary,
            config,
            masters,
        }
    }

    pub fn family(&self) -> &VacuumFamily {
        self.table.family()
    }

    pub fn table(&self) -> &ReductionTable {
        &self.table
    }

    pub fn boundary(&self) -> &ThreeLoopBoundaryReducer {
        &self.boundary
    }

    pub fn config(&self) -> ThreeLoopReductionConfig {
        self.config
    }

    pub fn stats(&self) -> &ReductionStats {
        self.table.stats()
    }

    /// The five candidate terminals accepted by this bounded certificate.
    ///
    /// This is not a claim of minimality for the unrestricted family.
    pub fn masters(&self) -> &[Integral; 5] {
        &self.masters
    }

    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ThreeLoopPipelineError> {
        validate_arity(integral)?;
        self.validate_input_coverage(integral)?;
        self.reduce_integral_unbounded(integral)
    }

    pub fn reduce_combination(
        &self,
        combination: &LinearCombination,
    ) -> Result<LinearCombination, ThreeLoopPipelineError> {
        let mut output = LinearCombination::new();
        for (integral, coefficient) in combination.terms() {
            let reduction = self.reduce_integral(integral)?;
            output.add_scaled(&reduction, coefficient);
        }
        Ok(output)
    }

    /// Validate caller-supplied identities inside the advertised input box.
    pub fn validate_identities(
        &self,
        identities: &[IbpIdentity],
    ) -> Result<(), ThreeLoopPipelineError> {
        for identity in identities {
            validate_arity(&identity.seed)?;
            self.validate_input_coverage(&identity.seed)?;
        }
        self.validate_identities_unbounded(identities)
    }

    fn validate_identities_unbounded(
        &self,
        identities: &[IbpIdentity],
    ) -> Result<(), ThreeLoopPipelineError> {
        // Provenance validation deliberately precedes all remainder work:
        // `IbpIdentity` has public fields, so an algebraic zero alone is not
        // evidence that the caller supplied the claimed total derivative.
        // Canonicalizing here also preserves acceptance of genuine raw rows.
        let equations = validate_reduction_table_identity_provenance(&self.table, identities)?;
        for (identity, equation) in identities.iter().zip(equations) {
            // Reduce the complete equation before applying the terminal
            // whitelist.  Individual terms can contain the one-step IBP halo
            // (dots or numerators just outside the advertised input box) and
            // may be absent from the sparse pivot table, yet cancel exactly
            // against the other terms of the same identity.  Rejecting such a
            // term in isolation would turn an algebraic zero into a spurious
            // `UnresolvedIntegral` error.
            let sparse_remainder = self.table.reduce_combination(&equation)?;
            let remainder = if sparse_remainder.is_zero() {
                sparse_remainder
            } else {
                self.normalize_sparse_result(&identity.seed, &sparse_remainder)?
            };
            if !remainder.is_zero() {
                return Err(ThreeLoopPipelineError::IdentityRemainder {
                    seed: identity.seed.clone(),
                    differentiated_loop: identity.differentiated_loop,
                    contraction_loop: identity.contraction_loop,
                    remainder,
                });
            }
        }
        Ok(())
    }

    fn reduce_integral_unbounded(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ThreeLoopPipelineError> {
        validate_arity(integral)?;
        if self.family().canonicalize(integral).is_none() {
            return Ok(LinearCombination::new());
        }

        // Boundary classification and its resource preflights precede sparse
        // table work.  This keeps factorized sectors out of an unnecessary
        // Laporta traversal and makes configured analytic limits effective at
        // the true operation boundary.
        if let Some(reduction) = self.try_boundary(integral)? {
            return Ok(reduction);
        }

        // Prefer a known sparse rule.  This is important for the one-dot halo
        // generated around the advertised target box; an absent rule remains
        // visible below and is then offered to the exact boundary reducer.
        let sparse = self.table.reduce_integral(integral)?;
        self.normalize_sparse_result(integral, &sparse)
    }

    fn normalize_sparse_result(
        &self,
        requested: &Integral,
        sparse: &LinearCombination,
    ) -> Result<LinearCombination, ThreeLoopPipelineError> {
        let mut result = LinearCombination::new();
        for (integral, coefficient) in sparse.terms() {
            if let Some(reduction) = self.try_boundary(integral)? {
                result.add_scaled(&reduction, coefficient);
            } else if self.masters[2..].contains(integral) {
                result.add_term(integral.clone(), coefficient.clone());
            } else {
                return Err(ThreeLoopPipelineError::UnresolvedIntegral {
                    requested: requested.clone(),
                    unresolved: integral.clone(),
                });
            }
        }
        if let Some(unexpected) = result
            .terms()
            .keys()
            .find(|integral| !self.masters.contains(integral))
        {
            return Err(ThreeLoopPipelineError::UnexpectedBoundaryMaster {
                requested: requested.clone(),
                unexpected: unexpected.clone(),
            });
        }
        Ok(result)
    }

    /// Close all factorized tree and paw sectors, including polynomial
    /// numerator powers.  The boundary reducer classifies the sector orbit
    /// before doing expansion work and returns `None` for genuine
    /// four-/five-/six-line sectors.
    fn try_boundary(
        &self,
        integral: &Integral,
    ) -> Result<Option<LinearCombination>, ThreeLoopPipelineError> {
        self.boundary
            .try_reduce_integral(integral)
            .map_err(Into::into)
    }

    fn validate_coverage(&self) -> Result<(), ThreeLoopPipelineError> {
        let targets = certified_targets(self.family(), self.config)?;
        self.validate_targets(&targets)
    }

    fn validate_targets(&self, targets: &[Integral]) -> Result<(), ThreeLoopPipelineError> {
        for target in targets {
            self.reduce_integral(&target)?;
        }
        Ok(())
    }

    fn validate_input_coverage(&self, integral: &Integral) -> Result<(), ThreeLoopPipelineError> {
        let (dots, numerator_degree) = integral_degrees(integral);
        if dots > u64::from(self.config.max_dots)
            || numerator_degree > u64::from(self.config.max_numerator_degree)
        {
            return Err(ThreeLoopPipelineError::OutOfCoverage {
                integral: integral.clone(),
                dots,
                numerator_degree,
                max_dots: self.config.max_dots,
                max_numerator_degree: self.config.max_numerator_degree,
            });
        }
        Ok(())
    }
}

/// Canonical Boolean sector mask, independent of unequal dots/numerators.
/// Full-exponent canonicalization can choose a different labelled mask from
/// the representative used to classify the sector orbit.
fn canonical_sector_mask(family: &VacuumFamily, integral: &Integral) -> Option<u8> {
    let boolean = Integral::new(
        integral
            .powers()
            .iter()
            .map(|power| i32::from(*power > 0))
            .collect::<Vec<_>>(),
    );
    family.canonicalize(&boolean).map(|representative| {
        representative
            .powers()
            .iter()
            .enumerate()
            .fold(0_u8, |mask, (position, power)| {
                mask | (u8::from(*power > 0) << position)
            })
    })
}

fn is_genuine_seed(family: &VacuumFamily, integral: &Integral) -> bool {
    matches!(
        canonical_sector_mask(family, integral),
        Some(43) | Some(31) | Some(63)
    )
}

fn certified_targets(
    family: &VacuumFamily,
    config: ThreeLoopReductionConfig,
) -> Result<Vec<Integral>, ThreeLoopPipelineError> {
    Ok(try_generate_seeds_with_limits(
        family,
        SeedConfig {
            max_dots: config.max_dots,
            max_numerator_degree: config.max_numerator_degree,
            include_subsectors: true,
        },
        SeedGenerationLimits {
            max_candidates: u64::try_from(config.max_seed_candidates).unwrap_or(u64::MAX),
        },
    )?)
}

fn validate_config(config: ThreeLoopReductionConfig) -> Result<(), ThreeLoopPipelineError> {
    // IBP generation can raise a seed exponent by one.
    let maximum_dots = i32::MAX as u32 - 2;
    if config.max_dots > maximum_dots {
        return Err(ThreeLoopPipelineError::ResourceLimit {
            resource: "configured dot degree",
            requested: u128::from(config.max_dots),
            limit: u128::from(maximum_dots),
        });
    }
    if config.max_seed_candidates == 0 {
        return Err(ThreeLoopPipelineError::ResourceLimit {
            resource: "seed candidates",
            requested: 1,
            limit: 0,
        });
    }
    induced_two_loop_halo(config)?;
    Ok(())
}

/// Compute the one-IBP halo delegated to the induced two-loop paw reducer.
///
/// Perform the additions in `u128` before converting back to the public `u32`
/// fields.  This makes both dot and numerator limits true construction
/// preflights rather than relying on saturating arithmetic or on a later
/// failure inside the nested two-loop build.
fn induced_two_loop_halo(
    config: ThreeLoopReductionConfig,
) -> Result<(u32, u32), ThreeLoopPipelineError> {
    const MAX_TWO_LOOP_DOTS: u32 = i32::MAX as u32 - 2;
    const MAX_TWO_LOOP_NUMERATORS: u32 = i32::MAX as u32;

    let requested_dots = u128::from(config.max_dots) + 1;
    if requested_dots > u128::from(MAX_TWO_LOOP_DOTS) {
        return Err(ThreeLoopPipelineError::ResourceLimit {
            resource: "induced two-loop dot coverage",
            requested: requested_dots,
            limit: u128::from(MAX_TWO_LOOP_DOTS),
        });
    }
    let required_dots = requested_dots as u32;
    if config.max_two_loop_dots < required_dots {
        return Err(ThreeLoopPipelineError::ResourceLimit {
            resource: "induced two-loop dot coverage",
            requested: requested_dots,
            limit: u128::from(config.max_two_loop_dots),
        });
    }
    if config.max_two_loop_dots > MAX_TWO_LOOP_DOTS {
        return Err(ThreeLoopPipelineError::ResourceLimit {
            resource: "configured two-loop dot coverage",
            requested: u128::from(config.max_two_loop_dots),
            limit: u128::from(MAX_TWO_LOOP_DOTS),
        });
    }

    let requested_numerators = u128::from(config.max_numerator_degree) + 1;
    if requested_numerators > u128::from(MAX_TWO_LOOP_NUMERATORS) {
        return Err(ThreeLoopPipelineError::ResourceLimit {
            resource: "induced two-loop numerator coverage",
            requested: requested_numerators,
            limit: u128::from(MAX_TWO_LOOP_NUMERATORS),
        });
    }
    Ok((required_dots, requested_numerators as u32))
}

fn validate_arity(integral: &Integral) -> Result<(), ThreeLoopPipelineError> {
    if integral.powers().len() != 6 {
        return Err(ThreeLoopPipelineError::WrongIntegralArity {
            actual: integral.powers().len(),
        });
    }
    Ok(())
}

fn integral_degrees(integral: &Integral) -> (u64, u64) {
    integral
        .powers()
        .iter()
        .fold((0_u64, 0_u64), |(mut dots, mut numerators), &power| {
            if power > 0 {
                dots += (i64::from(power) - 1) as u64;
            } else if power < 0 {
                numerators += i64::from(power).unsigned_abs();
            }
            (dots, numerators)
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeLoopPipelineError {
    Family(FamilyError),
    Boundary(ThreeLoopBoundaryError),
    Reduction(ReductionError),
    SeedGeneration(SeedGenerationError),
    IbpGeneration(IbpGenerationError),
    WrongIntegralArity {
        actual: usize,
    },
    OutOfCoverage {
        integral: Integral,
        dots: u64,
        numerator_degree: u64,
        max_dots: u32,
        max_numerator_degree: u32,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    UnresolvedIntegral {
        requested: Integral,
        unresolved: Integral,
    },
    UnexpectedBoundaryMaster {
        requested: Integral,
        unexpected: Integral,
    },
    IdentityRemainder {
        seed: Integral,
        differentiated_loop: usize,
        contraction_loop: usize,
        remainder: LinearCombination,
    },
}

impl fmt::Display for ThreeLoopPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => write!(formatter, "three-loop family error: {error}"),
            Self::Boundary(error) => write!(formatter, "three-loop boundary error: {error}"),
            Self::Reduction(error) => {
                write!(formatter, "three-loop sparse reduction error: {error}")
            }
            Self::SeedGeneration(error) => write!(formatter, "three-loop seed error: {error}"),
            Self::IbpGeneration(error) => write!(formatter, "three-loop IBP error: {error}"),
            Self::WrongIntegralArity { actual } => {
                write!(
                    formatter,
                    "a three-loop tetrahedron integral needs six powers, received {actual}"
                )
            }
            Self::OutOfCoverage {
                integral,
                dots,
                numerator_degree,
                max_dots,
                max_numerator_degree,
            } => write!(
                formatter,
                "{integral} has dot degree {dots} and numerator degree {numerator_degree}, outside coverage dots <= {max_dots}, numerators <= {max_numerator_degree}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "three-loop {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::UnresolvedIntegral {
                requested,
                unresolved,
            } => write!(
                formatter,
                "the sparse table leaves {unresolved} unresolved while reducing {requested}; it is not one of the five fixed candidates"
            ),
            Self::UnexpectedBoundaryMaster {
                requested,
                unexpected,
            } => write!(
                formatter,
                "boundary reduction of {requested} emitted unregistered master {unexpected}"
            ),
            Self::IdentityRemainder {
                seed,
                differentiated_loop,
                contraction_loop,
                ..
            } => write!(
                formatter,
                "three-loop IBP for seed {seed}, derivative {differentiated_loop}, contraction {contraction_loop} has a nonzero certified remainder"
            ),
        }
    }
}

impl std::error::Error for ThreeLoopPipelineError {}

impl From<FamilyError> for ThreeLoopPipelineError {
    fn from(value: FamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<ThreeLoopBoundaryError> for ThreeLoopPipelineError {
    fn from(value: ThreeLoopBoundaryError) -> Self {
        Self::Boundary(value)
    }
}

impl From<ReductionError> for ThreeLoopPipelineError {
    fn from(value: ReductionError) -> Self {
        Self::Reduction(value)
    }
}

impl From<SeedGenerationError> for ThreeLoopPipelineError {
    fn from(value: SeedGenerationError) -> Self {
        Self::SeedGeneration(value)
    }
}

impl From<IbpGenerationError> for ThreeLoopPipelineError {
    fn from(value: IbpGenerationError) -> Self {
        Self::IbpGeneration(value)
    }
}
