//! Public selection of the Ordered scheduler's weight arithmetic.
//!
//! The canonical text form (`FromStr`/`Display`) is
//! `exact | modular_sampled[:key=value,...] | modular_polynomial[:key=value,...]`
//! with keys equal to the field names of [`ModularCommon`] and of the mode's
//! options; `modular` is an alias of `modular_sampled`. `Display` prints the
//! fields that differ from their defaults; the alternate form (`{:#}`) prints
//! every field.
use std::fmt;
use std::str::FromStr;

/// How the Ordered scheduler combines frontier weights.
///
/// The scheduler's control flow (popped keys, selected rules, emitted children
/// and their exact step coefficients, guards) never depends on weight values,
/// except for exact-zero erasure of merged keys. Every mode therefore expands
/// the same keys, collects the same nonzero conditions and returns the same
/// exact terminal coefficients; the modes differ only in how the weights are
/// computed. The modular modes require a coefficient context with at most one
/// variable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrderedWeightArithmetic {
    /// Exact normalized rational functions on the frontier (historical mode).
    #[default]
    Exact,
    /// Record the weight-independent trace once, carrying small modular probes
    /// for zero decisions, then replay the trace numerically over 64-bit prime
    /// fields at sample points and interpolate every terminal coefficient per
    /// prime (Thiele continued fractions). The images are lifted over Q by the
    /// shared modular driver (CRT, rational number reconstruction, one extra
    /// confirming prime, fresh-prime verification).
    ModularSampled(ModularCommon, SampledOptions),
    /// Run scheduler passes whose weights are rational functions over
    /// `Z_p[d]` with factorized denominators (no interpolation), and lift the
    /// per-prime images with the shared modular driver. Not implemented yet:
    /// requests fail with [`crate::reduction::ReductionError::ModularWeights`].
    ModularPolynomial(ModularCommon, PolynomialOptions),
}

/// Options shared by both modular arithmetics (the lift over Q).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModularCommon {
    /// Worker threads of the request. `1` runs everything on the calling
    /// thread: no thread pool is built and no thread is spawned. Larger
    /// values use one private pool per request, never the global rayon pool.
    /// Results are bit-identical for every value.
    pub threads: usize,
    /// Seed of the prime sequences and sample points (deterministic).
    pub seed: u64,
    /// Fresh primes, never used to build the lifted result, on which every
    /// lifted coefficient must agree before it is accepted.
    pub verification_primes: usize,
    /// Upper bound on all primes drawn by one request (reconstruction,
    /// verification and rejected primes); the request fails closed beyond it.
    pub max_primes: usize,
    /// Additionally run the exact arithmetic in the same pass, compare every
    /// zero-erasure decision and the final terminal map, and fail on any
    /// difference. Diagnostic only: costs the full exact run.
    pub cross_check_exact: bool,
}

/// Parameters of [`OrderedWeightArithmetic::ModularSampled`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampledOptions {
    /// Maximum sample points evaluated by one replay batch; a batch holds
    /// `peak frontier slots x batch_points` field elements. Batches are made
    /// narrower (down to an eighth of this width) to fill the threads, and
    /// the first prime starts with half of it.
    pub batch_points: usize,
    /// Consecutive agreeing fresh points before a per-prime interpolation is
    /// accepted, and fresh points at which every lifted coefficient is
    /// checked on each verification prime.
    pub verification_points: usize,
    /// Upper bound on sample points per prime (fail closed).
    pub max_points_per_prime: usize,
}

/// Parameters of [`OrderedWeightArithmetic::ModularPolynomial`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolynomialOptions {
    /// Primes carried together, as lanes of every weight, by one pass.
    pub lanes: usize,
    /// Representation of weight denominators over `Z_p[d]`.
    pub denominators: PolynomialDenominators,
    /// Upper bound on the degree in `d` of any weight numerator or
    /// denominator (fail closed).
    pub max_degree: usize,
}

/// Denominator representation of [`OrderedWeightArithmetic::ModularPolynomial`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PolynomialDenominators {
    /// Sparse exponents over a request-wide basis of irreducible factors.
    #[default]
    Factorized,
    /// One dense polynomial per weight.
    Dense,
}

impl Default for ModularCommon {
    fn default() -> Self {
        Self {
            threads: 1,
            seed: 0x6f72_6465_7265_6421,
            verification_primes: 1,
            max_primes: 256,
            cross_check_exact: false,
        }
    }
}

impl Default for SampledOptions {
    fn default() -> Self {
        Self {
            batch_points: 128,
            verification_points: 2,
            max_points_per_prime: 1 << 16,
        }
    }
}

impl Default for PolynomialOptions {
    fn default() -> Self {
        Self {
            lanes: 4,
            denominators: PolynomialDenominators::Factorized,
            max_degree: 1 << 14,
        }
    }
}

/// Upper bound on [`ModularCommon::threads`].
const MAX_THREADS: usize = 4096;

impl OrderedWeightArithmetic {
    /// `ModularSampled` with default options.
    pub fn modular_sampled() -> Self {
        Self::ModularSampled(ModularCommon::default(), SampledOptions::default())
    }

    /// `ModularPolynomial` with default options.
    pub fn modular_polynomial() -> Self {
        Self::ModularPolynomial(ModularCommon::default(), PolynomialOptions::default())
    }

    /// Canonical mode name: `exact`, `modular_sampled` or `modular_polynomial`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::ModularSampled(..) => "modular_sampled",
            Self::ModularPolynomial(..) => "modular_polynomial",
        }
    }

    /// Options of the modular lift, if any.
    pub fn common(&self) -> Option<&ModularCommon> {
        match self {
            Self::Exact => None,
            Self::ModularSampled(common, _) | Self::ModularPolynomial(common, _) => Some(common),
        }
    }

    /// Worker threads used by the arithmetic (`1` for `Exact`).
    pub fn threads(&self) -> usize {
        self.common().map_or(1, |common| common.threads)
    }

    /// The same arithmetic with `threads` workers; `Exact` is unchanged.
    pub fn with_threads(mut self, threads: usize) -> Self {
        if let Self::ModularSampled(common, _) | Self::ModularPolynomial(common, _) = &mut self {
            common.threads = threads;
        }
        self
    }

    /// Check every option; requests validate the selected arithmetic again
    /// before any work.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Exact => Ok(()),
            Self::ModularSampled(common, sampled) => {
                common.validate()?;
                sampled.validate()
            }
            Self::ModularPolynomial(common, polynomial) => {
                common.validate()?;
                polynomial.validate()
            }
        }
    }
}

impl ModularCommon {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.threads == 0 || self.threads > MAX_THREADS {
            return Err(format!("modular threads must be in 1..={MAX_THREADS}"));
        }
        if self.verification_primes == 0 {
            return Err("modular verification_primes must be positive".into());
        }
        // One prime builds a candidate, one confirms it, the rest verify.
        if self.max_primes < self.verification_primes + 2 {
            return Err("modular max_primes must be at least verification_primes + 2".into());
        }
        Ok(())
    }
}

impl SampledOptions {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.batch_points < 2 || self.batch_points > 1 << 16 {
            return Err("modular batch_points must be in 2..=65536".into());
        }
        if self.verification_points == 0 {
            return Err("modular verification_points must be positive".into());
        }
        if self.max_points_per_prime < self.verification_points + 1 {
            return Err("modular max_points_per_prime must exceed verification_points".into());
        }
        Ok(())
    }
}

impl PolynomialOptions {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.lanes == 0 || self.lanes > 64 {
            return Err("modular_polynomial lanes must be in 1..=64".into());
        }
        if self.max_degree == 0 {
            return Err("modular_polynomial max_degree must be positive".into());
        }
        Ok(())
    }
}

/// Error of [`OrderedWeightArithmetic::from_str`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseOrderedWeightArithmeticError(String);

impl fmt::Display for ParseOrderedWeightArithmeticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid ordered weight arithmetic: {}", self.0)
    }
}

impl std::error::Error for ParseOrderedWeightArithmeticError {}

impl PolynomialDenominators {
    fn name(self) -> &'static str {
        match self {
            Self::Factorized => "factorized",
            Self::Dense => "dense",
        }
    }
}

const COMMON_KEYS: &str = "threads, seed, verification_primes, max_primes, cross_check_exact";

fn parse_count(key: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|e| format!("{key}={value}: {e}"))
}

fn parse_seed(value: &str) -> Result<u64, String> {
    match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => u64::from_str_radix(&hex.replace('_', ""), 16),
        None => value.parse::<u64>(),
    }
    .map_err(|e| format!("seed={value}: {e}"))
}

fn parse_flag(key: &str, value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("{key}={value}: expected true or false")),
    }
}

/// Assign one shared option; `Ok(false)` when `key` is not a shared option.
fn assign_common(common: &mut ModularCommon, key: &str, value: &str) -> Result<bool, String> {
    match key {
        "threads" => common.threads = parse_count(key, value)?,
        "seed" => common.seed = parse_seed(value)?,
        "verification_primes" => common.verification_primes = parse_count(key, value)?,
        "max_primes" => common.max_primes = parse_count(key, value)?,
        // `cross_check` is the historical spelling.
        "cross_check_exact" | "cross_check" => common.cross_check_exact = parse_flag(key, value)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn assign_sampled(sampled: &mut SampledOptions, key: &str, value: &str) -> Result<bool, String> {
    match key {
        // `batch`, `checks` and `max_points` are the historical spellings.
        "batch_points" | "batch" => sampled.batch_points = parse_count(key, value)?,
        "verification_points" | "checks" => sampled.verification_points = parse_count(key, value)?,
        "max_points_per_prime" | "max_points" => {
            sampled.max_points_per_prime = parse_count(key, value)?
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn assign_polynomial(
    polynomial: &mut PolynomialOptions,
    key: &str,
    value: &str,
) -> Result<bool, String> {
    match key {
        "lanes" => polynomial.lanes = parse_count(key, value)?,
        "denominators" => {
            polynomial.denominators = match value.to_ascii_lowercase().as_str() {
                "factorized" => PolynomialDenominators::Factorized,
                "dense" => PolynomialDenominators::Dense,
                _ => {
                    return Err(format!(
                        "denominators={value}: expected factorized or dense"
                    ));
                }
            }
        }
        "max_degree" => polynomial.max_degree = parse_count(key, value)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn parse(text: &str) -> Result<OrderedWeightArithmetic, String> {
    let text = text.trim();
    let (mode, options) = match text.split_once(':') {
        Some((mode, options)) => (mode.trim(), Some(options)),
        None => (text, None),
    };
    let mut items = Vec::new();
    for item in options
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| format!("expected key=value, found {item:?}"))?;
        let key = key.trim().to_ascii_lowercase();
        if items.iter().any(|(seen, _): &(String, &str)| *seen == key) {
            return Err(format!("option {key} given twice"));
        }
        items.push((key, value.trim()));
    }
    let arithmetic = match mode.to_ascii_lowercase().as_str() {
        "exact" => {
            if let Some((key, _)) = items.first() {
                return Err(format!("exact takes no options, found {key}"));
            }
            OrderedWeightArithmetic::Exact
        }
        "modular_sampled" | "modular" => {
            let (mut common, mut sampled) = (ModularCommon::default(), SampledOptions::default());
            for (key, value) in &items {
                if !assign_common(&mut common, key, value)?
                    && !assign_sampled(&mut sampled, key, value)?
                {
                    return Err(format!(
                        "unknown modular_sampled option {key:?}; expected one of {COMMON_KEYS}, \
                         batch_points, verification_points, max_points_per_prime"
                    ));
                }
            }
            OrderedWeightArithmetic::ModularSampled(common, sampled)
        }
        "modular_polynomial" => {
            let (mut common, mut polynomial) =
                (ModularCommon::default(), PolynomialOptions::default());
            for (key, value) in &items {
                if !assign_common(&mut common, key, value)?
                    && !assign_polynomial(&mut polynomial, key, value)?
                {
                    return Err(format!(
                        "unknown modular_polynomial option {key:?}; expected one of {COMMON_KEYS}, \
                         lanes, denominators, max_degree"
                    ));
                }
            }
            OrderedWeightArithmetic::ModularPolynomial(common, polynomial)
        }
        _ => {
            return Err(format!(
                "unknown mode {mode:?}; expected exact, modular_sampled[:key=value,...] \
                 or modular_polynomial[:key=value,...]"
            ));
        }
    };
    arithmetic.validate()?;
    Ok(arithmetic)
}

impl FromStr for OrderedWeightArithmetic {
    type Err = ParseOrderedWeightArithmeticError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse(text).map_err(ParseOrderedWeightArithmeticError)
    }
}

/// `key=value` items of one mode in field order: every field, or only those
/// that differ from their defaults.
struct Items {
    all: bool,
    items: Vec<String>,
}

impl Items {
    fn push(&mut self, key: &str, value: impl ToString, default: impl ToString) {
        let (value, default) = (value.to_string(), default.to_string());
        if self.all || value != default {
            self.items.push(format!("{key}={value}"));
        }
    }

    fn common(&mut self, common: &ModularCommon) {
        let defaults = ModularCommon::default();
        self.push("threads", common.threads, defaults.threads);
        self.push(
            "seed",
            format!("{:#x}", common.seed),
            format!("{:#x}", defaults.seed),
        );
        self.push(
            "verification_primes",
            common.verification_primes,
            defaults.verification_primes,
        );
        self.push("max_primes", common.max_primes, defaults.max_primes);
        self.push(
            "cross_check_exact",
            common.cross_check_exact,
            defaults.cross_check_exact,
        );
    }

    fn of(arithmetic: &OrderedWeightArithmetic, all: bool) -> Vec<String> {
        let mut items = Self { all, items: vec![] };
        match arithmetic {
            OrderedWeightArithmetic::Exact => {}
            OrderedWeightArithmetic::ModularSampled(common, sampled) => {
                let defaults = SampledOptions::default();
                items.common(common);
                items.push("batch_points", sampled.batch_points, defaults.batch_points);
                items.push(
                    "verification_points",
                    sampled.verification_points,
                    defaults.verification_points,
                );
                items.push(
                    "max_points_per_prime",
                    sampled.max_points_per_prime,
                    defaults.max_points_per_prime,
                );
            }
            OrderedWeightArithmetic::ModularPolynomial(common, polynomial) => {
                let defaults = PolynomialOptions::default();
                items.common(common);
                items.push("lanes", polynomial.lanes, defaults.lanes);
                items.push(
                    "denominators",
                    polynomial.denominators.name(),
                    defaults.denominators.name(),
                );
                items.push("max_degree", polynomial.max_degree, defaults.max_degree);
            }
        }
        items.items
    }
}

impl fmt::Display for OrderedWeightArithmetic {
    /// Canonical text: non-default options only, or every option with `{:#}`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())?;
        let items = Items::of(self, f.alternate());
        if !items.is_empty() {
            write!(f, ":{}", items.join(","))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> OrderedWeightArithmetic {
        text.parse().unwrap()
    }

    #[test]
    fn defaults_are_exact_and_single_threaded() {
        assert_eq!(
            OrderedWeightArithmetic::default(),
            OrderedWeightArithmetic::Exact
        );
        assert_eq!(OrderedWeightArithmetic::default().threads(), 1);
        let common = ModularCommon::default();
        assert_eq!(
            (
                common.threads,
                common.verification_primes,
                common.max_primes,
                common.cross_check_exact
            ),
            (1, 1, 256, false)
        );
        let sampled = SampledOptions::default();
        assert_eq!(
            (
                sampled.batch_points,
                sampled.verification_points,
                sampled.max_points_per_prime
            ),
            (128, 2, 65536)
        );
        let polynomial = PolynomialOptions::default();
        assert_eq!(polynomial.denominators, PolynomialDenominators::Factorized);
        for arithmetic in [
            OrderedWeightArithmetic::Exact,
            OrderedWeightArithmetic::modular_sampled(),
            OrderedWeightArithmetic::modular_polynomial(),
        ] {
            arithmetic.validate().unwrap();
        }
    }

    #[test]
    fn canonical_text_round_trips() {
        assert_eq!(parse("exact"), OrderedWeightArithmetic::Exact);
        assert_eq!(parse(" Exact "), OrderedWeightArithmetic::Exact);
        assert_eq!(
            parse("modular_sampled"),
            OrderedWeightArithmetic::modular_sampled()
        );
        assert_eq!(parse("modular"), OrderedWeightArithmetic::modular_sampled());
        assert_eq!(
            parse("modular_polynomial"),
            OrderedWeightArithmetic::modular_polynomial()
        );
        let sampled = parse(
            "modular_sampled:threads=4,seed=0x2a,verification_primes=2,max_primes=40,\
             cross_check_exact=true,batch_points=16,verification_points=3,max_points_per_prime=4096",
        );
        assert_eq!(
            sampled,
            OrderedWeightArithmetic::ModularSampled(
                ModularCommon {
                    threads: 4,
                    seed: 42,
                    verification_primes: 2,
                    max_primes: 40,
                    cross_check_exact: true,
                },
                SampledOptions {
                    batch_points: 16,
                    verification_points: 3,
                    max_points_per_prime: 4096,
                }
            )
        );
        let polynomial =
            parse("modular_polynomial:lanes=8,denominators=dense,max_degree=900,seed=7");
        assert_eq!(
            polynomial,
            OrderedWeightArithmetic::ModularPolynomial(
                ModularCommon {
                    seed: 7,
                    ..Default::default()
                },
                PolynomialOptions {
                    lanes: 8,
                    denominators: PolynomialDenominators::Dense,
                    max_degree: 900,
                }
            )
        );
        for arithmetic in [
            OrderedWeightArithmetic::Exact,
            OrderedWeightArithmetic::modular_sampled(),
            OrderedWeightArithmetic::modular_polynomial(),
            sampled,
            polynomial,
        ] {
            assert_eq!(parse(&arithmetic.to_string()), arithmetic);
            assert_eq!(parse(&format!("{arithmetic:#}")), arithmetic);
        }
        assert_eq!(OrderedWeightArithmetic::Exact.to_string(), "exact");
        assert_eq!(
            OrderedWeightArithmetic::modular_sampled().to_string(),
            "modular_sampled"
        );
        assert_eq!(
            parse("modular:threads=4,cross_check=1").to_string(),
            "modular_sampled:threads=4,cross_check_exact=true"
        );
        assert_eq!(
            format!("{:#}", OrderedWeightArithmetic::modular_sampled()),
            "modular_sampled:threads=1,seed=0x6f72646572656421,verification_primes=1,\
             max_primes=256,cross_check_exact=false,batch_points=128,verification_points=2,\
             max_points_per_prime=65536"
        );
        assert_eq!(
            parse("modular_sampled:threads=3").with_threads(9),
            parse("modular_sampled:threads=9")
        );
        assert_eq!(
            OrderedWeightArithmetic::Exact.with_threads(9),
            OrderedWeightArithmetic::Exact
        );
    }

    #[test]
    fn historical_modular_spellings_are_accepted() {
        assert_eq!(
            parse("modular:threads=16,batch=32,checks=3,max_points=1000,cross_check=0,seed=5"),
            OrderedWeightArithmetic::ModularSampled(
                ModularCommon {
                    threads: 16,
                    seed: 5,
                    ..Default::default()
                },
                SampledOptions {
                    batch_points: 32,
                    verification_points: 3,
                    max_points_per_prime: 1000,
                }
            )
        );
    }

    #[test]
    fn invalid_text_is_rejected_with_a_reason() {
        for (text, reason) in [
            ("fast", "unknown mode"),
            ("exact:threads=2", "exact takes no options"),
            ("modular_sampled:threads", "expected key=value"),
            ("modular_sampled:lanes=2", "unknown modular_sampled option"),
            (
                "modular_polynomial:batch_points=2",
                "unknown modular_polynomial option",
            ),
            ("modular_sampled:threads=0", "threads must be in"),
            ("modular_sampled:threads=x", "threads=x"),
            ("modular_sampled:threads=2,threads=3", "given twice"),
            (
                "modular_sampled:cross_check_exact=maybe",
                "expected true or false",
            ),
            (
                "modular_sampled:max_primes=2",
                "max_primes must be at least",
            ),
            ("modular_sampled:batch_points=0", "batch_points"),
            (
                "modular_polynomial:denominators=sparse",
                "expected factorized or dense",
            ),
            ("modular_polynomial:lanes=0", "lanes"),
        ] {
            let error = text
                .parse::<OrderedWeightArithmetic>()
                .expect_err(text)
                .to_string();
            assert!(error.contains(reason), "{text}: {error}");
        }
    }
}
