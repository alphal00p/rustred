//! Integrated finite-box reduction for the equal-mass two-loop vacuum family.
//!
//! [`TwoLoopReductionPipeline`] composes a sparse top-sector IBP table with
//! the analytic boundary reducer.  Unlike [`crate::ReductionTable`] by itself,
//! it never interprets an absent rule as a new master: the only unreduced
//! integrals admitted in a successful result are
//!
//! ```text
//! S = I(1, 1, 1),  P = I(0, 1, 1).
//! ```
//!
//! Coverage is finite and explicit.  Construction validates every
//! symmetry-unique top-sector integral in the advertised dot box, so an
//! incomplete sparse table fails before it can be used.

use std::fmt;

use crate::families::equal_mass_two_loop_vacuum;
use crate::two_loop::{
    TwoLoopBoundaryConfig, TwoLoopBoundaryError, TwoLoopBoundaryReducer, pair_sector_work_estimate,
};
use crate::{
    FamilyError, IbpGenerationError, IbpGenerator, IbpIdentity, Integral, LinearCombination,
    ReductionError, ReductionStats, ReductionTable, SeedConfig, SeedGenerationError,
    SeedGenerationLimits, SparseReducer, VacuumFamily, try_generate_seeds_with_limits,
};

/// Coverage and resource limits for an integrated two-loop reduction.
///
/// `max_dots` is the sum of positive powers above one.  The numerator degree
/// is the sum of absolute negative powers; for this three-propagator family it
/// can occur only in a boundary sector.  Empty and one-line sectors are
/// scaleless and are returned as zero without applying these bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwoLoopReductionConfig {
    pub max_dots: u32,
    pub max_numerator_degree: u32,
    /// Upper bound on seed candidates enumerated while constructing the table.
    /// The estimate is conservative and is checked before allocating seeds.
    pub max_seed_candidates: usize,
    /// Upper bound on a conservative iteration estimate for one analytic
    /// boundary reduction.  This protects against pathological integer powers.
    pub max_boundary_terms: usize,
}

impl Default for TwoLoopReductionConfig {
    fn default() -> Self {
        Self {
            // Covers every positive triple in the first acceptance cube
            // [-2,4]^3: (4-1) + (4-1) + (4-1) = 9.
            max_dots: 9,
            max_numerator_degree: 2,
            max_seed_candidates: 10_000,
            max_boundary_terms: 1_000_000,
        }
    }
}

/// A complete reduction pipeline over one advertised finite index box.
#[derive(Clone, Debug)]
pub struct TwoLoopReductionPipeline {
    table: ReductionTable,
    config: TwoLoopReductionConfig,
    sunset_master: Integral,
    product_master: Integral,
}

impl TwoLoopReductionPipeline {
    /// Generate a deterministic sparse table for the built-in equal-mass
    /// family and validate its claimed coverage.
    pub fn build(config: TwoLoopReductionConfig) -> Result<Self, TwoLoopPipelineError> {
        let family = equal_mass_two_loop_vacuum()?;
        Self::build_for_family(family, config)
    }

    /// Generate a deterministic sparse table for a compatible equal-mass
    /// two-loop family.
    ///
    /// This accepts both the standard denominators `D_i` and a uniformly
    /// reversed convention `-D_i`; [`TwoLoopBoundaryReducer::new`] performs
    /// the exact topology and common-sign validation.
    pub fn build_for_family(
        family: VacuumFamily,
        config: TwoLoopReductionConfig,
    ) -> Result<Self, TwoLoopPipelineError> {
        validate_config(config)?;
        TwoLoopBoundaryReducer::new(&family)?;
        let limits = SeedGenerationLimits {
            max_candidates: u64::try_from(config.max_seed_candidates).unwrap_or(u64::MAX),
        };
        let seeds = try_generate_seeds_with_limits(
            &family,
            SeedConfig {
                // IBPs generated at the edge also provide a one-step halo.
                max_dots: config.max_dots,
                max_numerator_degree: 0,
                include_subsectors: true,
            },
            limits,
        )?;
        if seeds.len() > config.max_seed_candidates {
            return Err(TwoLoopPipelineError::ResourceLimit {
                resource: "generated seed count",
                requested: seeds.len() as u128,
                limit: config.max_seed_candidates as u128,
            });
        }
        let identities = IbpGenerator::new(&family).try_generate_for_seeds(&seeds)?;
        let table = SparseReducer::new(family).reduce(&identities)?;
        Self::from_table(table, config)
    }

    /// Compose an existing sparse table with the exact boundary reducer.
    ///
    /// The table family and every top-sector target in `config` are validated.
    /// A table with a missing rule anywhere in that box returns
    /// [`TwoLoopPipelineError::UnresolvedIntegral`] instead of promoting the
    /// missing integral to a master.
    pub fn from_table(
        table: ReductionTable,
        config: TwoLoopReductionConfig,
    ) -> Result<Self, TwoLoopPipelineError> {
        validate_config(config)?;
        TwoLoopBoundaryReducer::new(table.family())?;
        let pipeline = Self {
            table,
            config,
            sunset_master: Integral::from([1, 1, 1]),
            product_master: Integral::from([0, 1, 1]),
        };
        pipeline.validate_top_sector_coverage()?;
        Ok(pipeline)
    }

    pub fn family(&self) -> &VacuumFamily {
        self.table.family()
    }

    pub fn table(&self) -> &ReductionTable {
        &self.table
    }

    pub fn config(&self) -> TwoLoopReductionConfig {
        self.config
    }

    pub fn stats(&self) -> &ReductionStats {
        self.table.stats()
    }

    /// Fixed sunset master `S = I(1,1,1)`.
    pub fn sunset_master(&self) -> &Integral {
        &self.sunset_master
    }

    /// Fixed factorized master `P = I(0,1,1)`.
    pub fn product_master(&self) -> &Integral {
        &self.product_master
    }

    /// Reduce an integral exactly to `S` and `P` inside the advertised box.
    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, TwoLoopPipelineError> {
        let active_lines = validate_integral_arity(integral)?;
        if active_lines <= 1 {
            // These sectors are scaleless independently of the magnitude of
            // their integer powers, and require no symbolic recurrence.
            return Ok(LinearCombination::new());
        }
        self.validate_input_coverage(integral)?;

        if active_lines == 2 {
            self.validate_boundary_resources(integral)?;
            return Ok(self.boundary_reducer()?.reduce_integral(integral)?);
        }

        let sparse_normal_form = self.table.reduce_integral(integral)?;
        self.normalize_sparse_result(integral, &sparse_normal_form)
    }

    /// Reduce a linear combination term by term to the two fixed masters.
    pub fn reduce_combination(
        &self,
        combination: &LinearCombination,
    ) -> Result<LinearCombination, TwoLoopPipelineError> {
        let mut output = LinearCombination::new();
        for (integral, coefficient) in combination.terms() {
            let reduction = self.reduce_integral(integral)?;
            output.add_scaled(&reduction, coefficient);
        }
        Ok(output)
    }

    /// Validate generated or externally supplied IBP identities through the
    /// integrated top-plus-boundary reduction.
    pub fn validate_identities(
        &self,
        identities: &[IbpIdentity],
    ) -> Result<(), TwoLoopPipelineError> {
        // `IbpIdentity` is publicly constructible.  Authenticate every row
        // against the exact total-derivative generator before a vanishing
        // expression is accepted as an IBP certificate.  The returned
        // equations are canonical, so raw and symmetry-equivalent generated
        // rows remain valid inputs.
        let equations = self.table.validate_identity_provenance(identities)?;
        for (identity, equation) in identities.iter().zip(equations) {
            let remainder = self.reduce_combination(&equation)?;
            if !remainder.is_zero() {
                return Err(TwoLoopPipelineError::IdentityRemainder {
                    seed: identity.seed.clone(),
                    differentiated_loop: identity.differentiated_loop,
                    contraction_loop: identity.contraction_loop,
                    remainder,
                });
            }
        }
        Ok(())
    }

    fn validate_top_sector_coverage(&self) -> Result<(), TwoLoopPipelineError> {
        // `include_subsectors = false` enumerates precisely the positive-index
        // top sector; symmetry canonicalization removes duplicate targets.
        let targets = try_generate_seeds_with_limits(
            self.family(),
            SeedConfig {
                max_dots: self.config.max_dots,
                max_numerator_degree: 0,
                include_subsectors: false,
            },
            SeedGenerationLimits {
                max_candidates: u64::try_from(self.config.max_seed_candidates).unwrap_or(u64::MAX),
            },
        )?;
        for target in targets {
            let sparse_normal_form = self.table.reduce_integral(&target)?;
            self.normalize_sparse_result(&target, &sparse_normal_form)?;
        }
        Ok(())
    }

    fn normalize_sparse_result(
        &self,
        requested: &Integral,
        sparse_normal_form: &LinearCombination,
    ) -> Result<LinearCombination, TwoLoopPipelineError> {
        let boundary = self.boundary_reducer()?;
        let mut result = LinearCombination::new();
        for (integral, coefficient) in sparse_normal_form.terms() {
            // The analytic boundary formula may perform work cubic in a large
            // inactive numerator degree.  Enforce its cap before invoking it;
            // checking after `try_reduce_integral` would make the limit purely
            // cosmetic for sparse-table output terms.
            if integral.powers().iter().filter(|power| **power > 0).count() <= 2 {
                self.validate_boundary_resources(integral)?;
            }
            match boundary.try_reduce_integral(integral)? {
                Some(reduction) => {
                    result.add_scaled(&reduction, coefficient);
                }
                None if integral == &self.sunset_master => {
                    result.add_term(self.sunset_master.clone(), coefficient.clone());
                }
                None => {
                    return Err(TwoLoopPipelineError::UnresolvedIntegral {
                        requested: requested.clone(),
                        unresolved: integral.clone(),
                    });
                }
            }
        }
        debug_assert!(result
            .terms()
            .keys()
            .all(|integral| integral == &self.sunset_master || integral == &self.product_master));
        Ok(result)
    }

    fn validate_input_coverage(&self, integral: &Integral) -> Result<(), TwoLoopPipelineError> {
        let (dots, numerator_degree) = integral_degrees(integral);
        if dots > u64::from(self.config.max_dots)
            || numerator_degree > u64::from(self.config.max_numerator_degree)
        {
            return Err(TwoLoopPipelineError::OutOfCoverage {
                integral: integral.clone(),
                dots,
                numerator_degree,
                max_dots: self.config.max_dots,
                max_numerator_degree: self.config.max_numerator_degree,
            });
        }
        Ok(())
    }

    fn validate_boundary_resources(&self, integral: &Integral) -> Result<(), TwoLoopPipelineError> {
        let requested = boundary_work_estimate(integral);
        if requested > self.config.max_boundary_terms as u128 {
            return Err(TwoLoopPipelineError::ResourceLimit {
                resource: "boundary formula iteration estimate",
                requested,
                limit: self.config.max_boundary_terms as u128,
            });
        }
        Ok(())
    }

    fn boundary_reducer(&self) -> Result<TwoLoopBoundaryReducer<'_>, TwoLoopPipelineError> {
        Ok(TwoLoopBoundaryReducer::new_with_config(
            self.family(),
            TwoLoopBoundaryConfig {
                max_formula_iterations: self.config.max_boundary_terms,
            },
        )?)
    }
}

/// Errors from finite-box construction, coverage checks, or exact reduction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TwoLoopPipelineError {
    Family(FamilyError),
    Boundary(TwoLoopBoundaryError),
    Reduction(ReductionError),
    SeedGeneration(SeedGenerationError),
    IbpGeneration(IbpGenerationError),
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
    IdentityRemainder {
        seed: Integral,
        differentiated_loop: usize,
        contraction_loop: usize,
        remainder: LinearCombination,
    },
}

impl fmt::Display for TwoLoopPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => {
                write!(formatter, "cannot construct the two-loop family: {error}")
            }
            Self::Boundary(error) => {
                write!(formatter, "two-loop boundary reduction failed: {error}")
            }
            Self::Reduction(error) => {
                write!(formatter, "sparse two-loop reduction failed: {error}")
            }
            Self::SeedGeneration(error) => {
                write!(formatter, "two-loop seed generation failed: {error}")
            }
            Self::IbpGeneration(error) => {
                write!(formatter, "two-loop IBP generation failed: {error}")
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
                "two-loop {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::UnresolvedIntegral {
                requested,
                unresolved,
            } => write!(
                formatter,
                "the sparse table leaves {unresolved} unresolved while reducing {requested}; the only accepted top-sector master is I(1,1,1)"
            ),
            Self::IdentityRemainder {
                seed,
                differentiated_loop,
                contraction_loop,
                ..
            } => write!(
                formatter,
                "integrated reduction leaves a nonzero IBP remainder for seed {seed}, derivative {differentiated_loop}, contraction {contraction_loop}"
            ),
        }
    }
}

impl std::error::Error for TwoLoopPipelineError {}

impl From<FamilyError> for TwoLoopPipelineError {
    fn from(value: FamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<TwoLoopBoundaryError> for TwoLoopPipelineError {
    fn from(value: TwoLoopBoundaryError) -> Self {
        Self::Boundary(value)
    }
}

impl From<ReductionError> for TwoLoopPipelineError {
    fn from(value: ReductionError) -> Self {
        Self::Reduction(value)
    }
}

impl From<SeedGenerationError> for TwoLoopPipelineError {
    fn from(value: SeedGenerationError) -> Self {
        Self::SeedGeneration(value)
    }
}

impl From<IbpGenerationError> for TwoLoopPipelineError {
    fn from(value: IbpGenerationError) -> Self {
        Self::IbpGeneration(value)
    }
}

fn validate_config(config: TwoLoopReductionConfig) -> Result<(), TwoLoopPipelineError> {
    // An active seed power is 1+dots and IBP generation may raise it once.
    let maximum_dots = i32::MAX as u32 - 2;
    if config.max_dots > maximum_dots {
        return Err(TwoLoopPipelineError::ResourceLimit {
            resource: "configured dot degree",
            requested: u128::from(config.max_dots),
            limit: u128::from(maximum_dots),
        });
    }
    if config.max_numerator_degree > i32::MAX as u32 {
        return Err(TwoLoopPipelineError::ResourceLimit {
            resource: "configured numerator degree",
            requested: u128::from(config.max_numerator_degree),
            limit: i32::MAX as u128,
        });
    }

    let candidates = seed_candidate_upper_bound(config.max_dots);
    if candidates > config.max_seed_candidates as u128 {
        return Err(TwoLoopPipelineError::ResourceLimit {
            resource: "seed candidate upper bound",
            requested: candidates,
            limit: config.max_seed_candidates as u128,
        });
    }
    Ok(())
}

fn validate_integral_arity(integral: &Integral) -> Result<usize, TwoLoopPipelineError> {
    if integral.powers().len() != 3 {
        return Err(TwoLoopBoundaryError::WrongIntegralArity {
            actual: integral.powers().len(),
        }
        .into());
    }
    Ok(integral.powers().iter().filter(|power| **power > 0).count())
}

fn integral_degrees(integral: &Integral) -> (u64, u64) {
    let mut dots = 0_u64;
    let mut numerator_degree = 0_u64;
    for &power in integral.powers() {
        if power > 0 {
            dots += u64::try_from(i64::from(power) - 1).expect("a positive i32 fits in u64");
        } else if power < 0 {
            numerator_degree += i64::from(power).unsigned_abs();
        }
    }
    (dots, numerator_degree)
}

fn boundary_work_estimate(integral: &Integral) -> u128 {
    let mut active = integral.powers().iter().copied().filter(|power| *power > 0);
    let left = active.next().unwrap_or(0);
    let right = active.next().unwrap_or(0);
    let inactive = integral
        .powers()
        .iter()
        .copied()
        .find(|power| *power <= 0)
        .unwrap_or(0);
    pair_sector_work_estimate(inactive, left, right)
}

/// Conservative number of raw `max_numerator_degree=0` seed candidates for
/// three propagators, before scaleless removal and symmetry canonicalization.
fn seed_candidate_upper_bound(max_dots: u32) -> u128 {
    let dots = u128::from(max_dots);
    let one_line = dots + 1;
    let two_line = (dots + 2).saturating_mul(dots + 1) / 2;
    let three_line = (dots + 3).saturating_mul(dots + 2).saturating_mul(dots + 1) / 6;
    1_u128
        .saturating_add(3_u128.saturating_mul(one_line))
        .saturating_add(3_u128.saturating_mul(two_line))
        .saturating_add(three_line)
}
