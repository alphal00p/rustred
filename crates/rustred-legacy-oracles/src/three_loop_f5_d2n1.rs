//! Exact wrapper for the induced three-loop F5 `D2/N1` target manifest.
//!
//! The implementation delegates exact reduction to [`ThreeLoopReductionPipeline`].
//! Its five surviving outputs are *candidate* terminals of the bounded pipeline,
//! not a proof that they are masters of the unrestricted family.  Source weights
//! and exceptional factors are inherited from that pipeline rather than exposed
//! as a standalone certificate here: the current [`rustred::ReductionTable`] does
//! not persist source-row weights or a separate exceptional-factor list.  Thus
//! this wrapper certifies exact deterministic reconstruction and native-row
//! replay over `Q(d,m2)`; its formulae retain the usual caveat that denominator
//! factors introduced by exact pivots must be nonzero.

use std::fmt;

use crate::three_loop::equal_mass_three_loop_tetrahedron;
use crate::{ThreeLoopPipelineError, ThreeLoopReductionConfig, ThreeLoopReductionPipeline};
use rustred::{
    FamilyError, IbpGenerationError, IbpGenerator, Integral, LinearCombination, ReductionStats,
    VacuumFamily,
};

/// Fixed-mask F5 targets: five positive lines, line five inactive with one
/// numerator power, and total dot degree two.
pub const THREE_LOOP_F5_D2N1_LABELLED_TARGET_POWERS: [[i32; 6]; 15] = [
    [3, 1, 1, 1, 1, -1],
    [1, 3, 1, 1, 1, -1],
    [1, 1, 3, 1, 1, -1],
    [1, 1, 1, 3, 1, -1],
    [1, 1, 1, 1, 3, -1],
    [2, 2, 1, 1, 1, -1],
    [2, 1, 2, 1, 1, -1],
    [2, 1, 1, 2, 1, -1],
    [2, 1, 1, 1, 2, -1],
    [1, 2, 2, 1, 1, -1],
    [1, 2, 1, 2, 1, -1],
    [1, 2, 1, 1, 2, -1],
    [1, 1, 2, 2, 1, -1],
    [1, 1, 2, 1, 2, -1],
    [1, 1, 1, 2, 2, -1],
];

/// The six fixed-mask orbits under [`THREE_LOOP_F5_D2N1_STABILIZER`].
pub const THREE_LOOP_F5_D2N1_CANONICAL_REPRESENTATIVE_POWERS: [[i32; 6]; 6] = [
    [1, 1, 1, 1, 3, -1],
    [1, 1, 1, 2, 2, -1],
    [1, 1, 2, 1, 2, -1],
    [1, 1, 2, 2, 1, -1],
    [2, 1, 1, 1, 2, -1],
    [3, 1, 1, 1, 1, -1],
];

/// Exact order-four power-permutation stabilizer of the inactive F5 line.
/// A transformed vector has `output[i] = input[permutation[i]]`.
pub const THREE_LOOP_F5_D2N1_STABILIZER: [[usize; 6]; 4] = [
    [0, 1, 2, 3, 4, 5],
    [0, 2, 1, 4, 3, 5],
    [0, 3, 4, 1, 2, 5],
    [0, 4, 3, 2, 1, 5],
];

pub const THREE_LOOP_F5_D2N1_TARGETS: usize = 15;
pub const THREE_LOOP_F5_D2N1_ORBITS: usize = 6;
pub const THREE_LOOP_F5_D2N1_IBPS_PER_TARGET: usize = 9;
pub const THREE_LOOP_F5_D2N1_NATIVE_IDENTITIES: usize =
    THREE_LOOP_F5_D2N1_TARGETS * THREE_LOOP_F5_D2N1_IBPS_PER_TARGET;

/// The fixed finite-box configuration used by this exact wrapper.
pub fn three_loop_f5_d2n1_pipeline_config() -> ThreeLoopReductionConfig {
    ThreeLoopReductionConfig {
        max_dots: 2,
        max_numerator_degree: 1,
        max_seed_candidates: 2_320,
        max_two_loop_dots: 3,
        max_two_loop_seed_candidates: 192,
        ..ThreeLoopReductionConfig::default()
    }
}

/// Exact reductions of the complete induced F5 `D2/N1` target domain.
#[derive(Clone, Debug)]
pub struct ThreeLoopF5D2N1Reducer {
    pipeline: ThreeLoopReductionPipeline,
    family_fingerprint: String,
    config: ThreeLoopReductionConfig,
    stats: ReductionStats,
    targets: Vec<Integral>,
    reductions: Vec<LinearCombination>,
}

impl ThreeLoopF5D2N1Reducer {
    pub const SCHEMA: &'static str = "rustred-three-loop-f5-d2n1-v1";

    /// Build against RustRed's authenticated equal-mass tetrahedron family.
    pub fn build() -> Result<Self, ThreeLoopF5D2N1Error> {
        Self::build_for_family(equal_mass_three_loop_tetrahedron()?)
    }

    /// Build against an explicitly supplied family.  The delegated pipeline
    /// authenticates its routing, masses, signs, and complete S4 symmetry.
    pub fn build_for_family(family: VacuumFamily) -> Result<Self, ThreeLoopF5D2N1Error> {
        Self::build_impl(family)
    }

    fn build_impl(family: VacuumFamily) -> Result<Self, ThreeLoopF5D2N1Error> {
        validate_frozen_manifest()?;
        let config = three_loop_f5_d2n1_pipeline_config();
        let pipeline = ThreeLoopReductionPipeline::build_for_family(family, config)?;
        validate_family_stabilizer(pipeline.family())?;
        let targets = labelled_targets();
        let mut reductions = Vec::with_capacity(targets.len());
        for target in &targets {
            let reduction = pipeline.reduce_integral(target)?;
            if let Some(unexpected) = reduction
                .terms()
                .keys()
                .find(|integral| !pipeline.masters().contains(integral))
            {
                return Err(ThreeLoopF5D2N1Error::UnexpectedTerminal {
                    target: target.clone(),
                    terminal: unexpected.clone(),
                });
            }
            reductions.push(reduction);
        }
        let reducer = Self {
            family_fingerprint: pipeline.family().fingerprint(),
            stats: pipeline.stats().clone(),
            pipeline,
            config,
            targets,
            reductions,
        };
        reducer.validate_native_target_identities()?;
        Ok(reducer)
    }

    pub fn family(&self) -> &VacuumFamily {
        self.pipeline.family()
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn config(&self) -> ThreeLoopReductionConfig {
        self.config
    }

    pub fn stats(&self) -> &ReductionStats {
        &self.stats
    }

    /// The five candidate terminals of the delegated bounded pipeline.
    pub fn candidates(&self) -> &[Integral; 5] {
        self.pipeline.masters()
    }

    pub fn targets(&self) -> &[Integral] {
        &self.targets
    }

    pub fn reductions(&self) -> &[LinearCombination] {
        &self.reductions
    }

    /// Reduce exactly an S4 image of the fixed-mask F5 `D2/N1` manifest.
    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ThreeLoopF5D2N1Error> {
        let index = orient_to_fixed_manifest(self.family(), integral)?;
        Ok(self.reductions[index].clone())
    }

    /// Regenerate and validate all `15 * 3^2 = 135` native target identities.
    pub fn validate_native_target_identities(&self) -> Result<(), ThreeLoopF5D2N1Error> {
        let generator = IbpGenerator::new(self.family());
        let mut identities = Vec::with_capacity(THREE_LOOP_F5_D2N1_NATIVE_IDENTITIES);
        for target in &self.targets {
            identities.extend(generator.try_generate_raw(target)?);
        }
        if identities.len() != THREE_LOOP_F5_D2N1_NATIVE_IDENTITIES {
            return Err(ThreeLoopF5D2N1Error::ManifestMismatch);
        }
        self.pipeline.validate_identities(&identities)?;
        Ok(())
    }

    /// Rebuild the complete deterministic pipeline and compare all frozen
    /// wrapper state before replaying the 135 target identities once more.
    pub fn replay(&self) -> Result<(), ThreeLoopF5D2N1Error> {
        let rebuilt = Self::build_impl(self.family().clone())?;
        if rebuilt.family_fingerprint != self.family_fingerprint
            || rebuilt.config != self.config
            || rebuilt.stats != self.stats
            || rebuilt.targets != self.targets
            || rebuilt.reductions != self.reductions
        {
            return Err(ThreeLoopF5D2N1Error::ReplayMismatch);
        }
        rebuilt.validate_native_target_identities()
    }
}

fn labelled_targets() -> Vec<Integral> {
    THREE_LOOP_F5_D2N1_LABELLED_TARGET_POWERS
        .into_iter()
        .map(Integral::from)
        .collect()
}

fn transform(integral: &Integral, permutation: &[usize]) -> Integral {
    Integral::new(
        permutation
            .iter()
            .map(|&position| integral.powers()[position])
            .collect::<Vec<_>>(),
    )
}

fn orient_to_fixed_manifest(
    family: &VacuumFamily,
    integral: &Integral,
) -> Result<usize, ThreeLoopF5D2N1Error> {
    if integral.powers().len() != 6 {
        return Err(ThreeLoopF5D2N1Error::WrongIntegralArity {
            actual: integral.powers().len(),
        });
    }
    let targets = labelled_targets();
    family
        .symmetries()
        .iter()
        .find_map(|permutation| {
            let oriented = transform(integral, permutation);
            targets.iter().position(|target| target == &oriented)
        })
        .ok_or_else(|| ThreeLoopF5D2N1Error::OutsideManifest {
            integral: integral.clone(),
        })
}

fn validate_frozen_manifest() -> Result<(), ThreeLoopF5D2N1Error> {
    let targets = labelled_targets();
    if targets.len() != THREE_LOOP_F5_D2N1_TARGETS
        || targets.iter().any(|target| {
            target.powers()[5] != -1
                || target.powers().iter().take(5).any(|power| *power <= 0)
                || target
                    .powers()
                    .iter()
                    .map(|power| (*power - 1).max(0))
                    .sum::<i32>()
                    != 2
        })
    {
        return Err(ThreeLoopF5D2N1Error::ManifestMismatch);
    }
    let mut representatives = targets
        .iter()
        .map(|target| {
            THREE_LOOP_F5_D2N1_STABILIZER
                .iter()
                .map(|permutation| transform(target, permutation))
                .min()
                .ok_or(ThreeLoopF5D2N1Error::ManifestMismatch)
        })
        .collect::<Result<Vec<_>, _>>()?;
    representatives.sort();
    representatives.dedup();
    let expected = THREE_LOOP_F5_D2N1_CANONICAL_REPRESENTATIVE_POWERS
        .into_iter()
        .map(Integral::from)
        .collect::<std::collections::BTreeSet<_>>();
    if representatives
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        != expected
    {
        return Err(ThreeLoopF5D2N1Error::ManifestMismatch);
    }
    Ok(())
}

fn validate_family_stabilizer(family: &VacuumFamily) -> Result<(), ThreeLoopF5D2N1Error> {
    let actual = family
        .symmetries()
        .iter()
        .filter(|permutation| permutation[5] == 5)
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let expected = THREE_LOOP_F5_D2N1_STABILIZER
        .iter()
        .map(|permutation| permutation.to_vec())
        .collect::<std::collections::BTreeSet<_>>();
    if actual != expected {
        return Err(ThreeLoopF5D2N1Error::ManifestMismatch);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeLoopF5D2N1Error {
    Family(FamilyError),
    Pipeline(ThreeLoopPipelineError),
    IbpGeneration(IbpGenerationError),
    WrongIntegralArity {
        actual: usize,
    },
    OutsideManifest {
        integral: Integral,
    },
    UnexpectedTerminal {
        target: Integral,
        terminal: Integral,
    },
    ManifestMismatch,
    ReplayMismatch,
}

impl fmt::Display for ThreeLoopF5D2N1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => write!(formatter, "three-loop F5 D2/N1 family error: {error}"),
            Self::Pipeline(error) => {
                write!(formatter, "three-loop F5 D2/N1 pipeline error: {error}")
            }
            Self::IbpGeneration(error) => {
                write!(formatter, "three-loop F5 D2/N1 IBP error: {error}")
            }
            Self::WrongIntegralArity { actual } => write!(
                formatter,
                "an F5 D2/N1 integral needs six powers, received {actual}"
            ),
            Self::OutsideManifest { integral } => {
                write!(
                    formatter,
                    "{integral} is outside the induced F5 D2/N1 manifest"
                )
            }
            Self::UnexpectedTerminal { target, terminal } => write!(
                formatter,
                "reduction of F5 D2/N1 target {target} emitted unexpected terminal {terminal}"
            ),
            Self::ManifestMismatch => formatter.write_str("frozen F5 D2/N1 manifest mismatch"),
            Self::ReplayMismatch => formatter.write_str("F5 D2/N1 deterministic replay mismatch"),
        }
    }
}

impl std::error::Error for ThreeLoopF5D2N1Error {}

impl From<FamilyError> for ThreeLoopF5D2N1Error {
    fn from(value: FamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<ThreeLoopPipelineError> for ThreeLoopF5D2N1Error {
    fn from(value: ThreeLoopPipelineError) -> Self {
        Self::Pipeline(value)
    }
}

impl From<IbpGenerationError> for ThreeLoopF5D2N1Error {
    fn from(value: IbpGenerationError) -> Self {
        Self::IbpGeneration(value)
    }
}
