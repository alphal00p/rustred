//! Public options and verdicts of the per-sector master count.

/// Seeded sampling policy for [`super::MasterCounter`].
///
/// Every sample uses its own prime and its own random kinematic point and
/// Euler exponents. Primes and points are a deterministic function of `seed`,
/// the sample index, and the number of points already drawn for that sample.
#[derive(Clone, Copy, Debug)]
pub struct MasterCountOptions {
    /// Independent modular samples, from two to [`super::MAX_SAMPLES`].
    pub samples: usize,
    /// Seed of the deterministic prime and point streams.
    pub seed: u64,
    /// Points one sample may draw for one sector before inadmissible
    /// kinematics or unlucky Euler exponents exhaust it.
    pub max_attempts: usize,
}

impl Default for MasterCountOptions {
    fn default() -> Self {
        Self {
            samples: 2,
            seed: 0,
            max_attempts: 16,
        }
    }
}

/// Number of master integrals whose top sector is one sector, without
/// symmetries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MasterCount {
    /// The sector is scaleless (exact ProvedZero from the zero-sector Analyzer, or G_S identically zero).
    Zero,
    /// Confident count of masters with top sector S.
    Counted(usize),
    /// No reliable verdict; the numbers are diagnostics only.
    NoVerdict {
        /// Morse count of one sample; `None` when its critical locus is not
        /// isolated.
        morse: Option<usize>,
        /// Moebius-inverted torus Euler count of one sample.
        euler: Option<i64>,
        reason: NoVerdictReason,
    },
}

/// Why a sector received no count, from the most to the least specific.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoVerdictReason {
    /// The external Gram determinant vanishes identically, so the parametric
    /// count can be below what momentum-space IBP reaches.
    GramSingular,
    /// The critical locus of `G_S` off `G_S = 0` is positive-dimensional.
    MorseNonIsolated,
    /// The Morse and Euler counts differ in some sample.
    MorseEulerMismatch,
    /// The Moebius-inverted Euler count is negative in some sample.
    NegativeEuler,
    /// Each sample is self-consistent, but the samples disagree.
    SampleDisagreement,
}
