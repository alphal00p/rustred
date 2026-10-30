//! Public selection of the Ordered scheduler's weight arithmetic.

/// How the Ordered scheduler combines frontier weights.
///
/// The scheduler's control flow (popped keys, selected rules, emitted children
/// and their exact step coefficients, guards) never depends on weight values,
/// except for exact-zero erasure of merged keys. Both modes therefore expand
/// the same keys, collect the same nonzero conditions and return the same
/// exact terminal coefficients; they differ only in how the weights are
/// computed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrderedWeightArithmetic {
    /// Exact normalized rational functions on the frontier (historical mode).
    #[default]
    Exact,
    /// Record the weight-independent trace once, carrying small modular probes
    /// for zero decisions, then replay the trace numerically over 64-bit prime
    /// fields at many sample points and reconstruct every terminal coefficient
    /// as an exact rational function (Thiele interpolation per prime, CRT and
    /// rational number reconstruction, fresh-prime verification).
    ///
    /// Requires a coefficient context with at most one variable.
    Modular(ModularWeightOptions),
}

/// Parameters of [`OrderedWeightArithmetic::Modular`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModularWeightOptions {
    /// Sample points carried by one numeric replay of the recorded trace.
    pub batch_points: usize,
    /// Worker threads of the private replay pool (each owns one batch).
    pub threads: usize,
    /// Seed for prime offsets and sample points.
    pub seed: u64,
    /// Consecutive agreeing fresh points before a per-prime Thiele
    /// interpolation is accepted.
    pub verification_points: usize,
    /// Independent fresh primes checked against the lifted result over Q.
    pub verification_primes: usize,
    /// Upper bound on the number of reconstruction primes (fail closed).
    pub max_primes: usize,
    /// Upper bound on sample points per prime (fail closed).
    pub max_points_per_prime: usize,
    /// Additionally run the exact arithmetic in the same pass, compare every
    /// zero-erasure decision and the final terminal map, and fail on any
    /// difference. Diagnostic only: costs the full exact run.
    pub cross_check_exact: bool,
}

impl Default for ModularWeightOptions {
    fn default() -> Self {
        Self {
            batch_points: 64,
            threads: 1,
            seed: 0x6f72_6465_7265_6421,
            verification_points: 2,
            verification_primes: 1,
            max_primes: 256,
            max_points_per_prime: 1 << 16,
            cross_check_exact: false,
        }
    }
}

impl ModularWeightOptions {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.batch_points == 0 || self.batch_points > 1 << 16 {
            return Err("modular batch_points must be in 1..=65536");
        }
        if self.threads == 0 {
            return Err("modular threads must be positive");
        }
        if self.verification_points == 0 {
            return Err("modular verification_points must be positive");
        }
        if self.verification_primes == 0 {
            return Err("modular verification_primes must be positive");
        }
        if self.max_primes < 2 {
            return Err("modular max_primes must be at least 2");
        }
        if self.max_points_per_prime < self.verification_points + 1 {
            return Err("modular max_points_per_prime is too small");
        }
        Ok(())
    }
}
