use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};

use symbolica::prelude::AtomCore;

use crate::ibp::{IbpGenerationError, IbpGenerator, IbpIdentity};
use crate::{Integral, LinearCombination, VacuumFamily};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeedConfig {
    /// Maximum sum of powers above one on denominator lines.
    pub max_dots: u32,
    /// Maximum sum of absolute non-positive powers.
    pub max_numerator_degree: u32,
    /// Generate all non-scaleless subsectors as well as the top sector.
    pub include_subsectors: bool,
}

impl Default for SeedConfig {
    fn default() -> Self {
        Self {
            max_dots: 0,
            max_numerator_degree: 0,
            include_subsectors: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeedGenerationLimits {
    /// Maximum number of exponent vectors visited before symmetry and
    /// scaleless-sector filtering.
    pub max_candidates: u64,
}

impl Default for SeedGenerationLimits {
    fn default() -> Self {
        Self {
            max_candidates: 10_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedGenerationError {
    ExponentBound {
        field: &'static str,
        requested: u32,
        maximum: u32,
    },
    CandidateLimitExceeded {
        upper_bound: u128,
        limit: u64,
    },
}

impl std::fmt::Display for SeedGenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExponentBound {
                field,
                requested,
                maximum,
            } => write!(
                formatter,
                "seed {field} {requested} exceeds the i32 exponent limit {maximum}"
            ),
            Self::CandidateLimitExceeded { upper_bound, limit } => write!(
                formatter,
                "seed enumeration may visit {upper_bound} candidates, exceeding the configured limit {limit}"
            ),
        }
    }
}

impl std::error::Error for SeedGenerationError {}

pub fn generate_seeds(family: &VacuumFamily, config: SeedConfig) -> Vec<Integral> {
    try_generate_seeds(family, config)
        .expect("seed bounds and candidate count must be within the configured limits")
}

pub fn try_generate_seeds(
    family: &VacuumFamily,
    config: SeedConfig,
) -> Result<Vec<Integral>, SeedGenerationError> {
    try_generate_seeds_with_limits(family, config, SeedGenerationLimits::default())
}

pub fn try_generate_seeds_with_limits(
    family: &VacuumFamily,
    config: SeedConfig,
    limits: SeedGenerationLimits,
) -> Result<Vec<Integral>, SeedGenerationError> {
    // An active power is 1 + dots. i32::MAX - 1 is therefore the largest
    // dot bound representable by Integral; IBP generation performs its own
    // additional checked +1 shift.
    const MAX_DOTS: u32 = i32::MAX as u32 - 1;
    const MAX_NUMERATOR_DEGREE: u32 = i32::MAX as u32;
    if config.max_dots > MAX_DOTS {
        return Err(SeedGenerationError::ExponentBound {
            field: "max_dots",
            requested: config.max_dots,
            maximum: MAX_DOTS,
        });
    }
    if config.max_numerator_degree > MAX_NUMERATOR_DEGREE {
        return Err(SeedGenerationError::ExponentBound {
            field: "max_numerator_degree",
            requested: config.max_numerator_degree,
            maximum: MAX_NUMERATOR_DEGREE,
        });
    }

    let candidate_bound = seed_candidate_upper_bound(family, config);
    if candidate_bound > u128::from(limits.max_candidates) {
        return Err(SeedGenerationError::CandidateLimitExceeded {
            upper_bound: candidate_bound,
            limit: limits.max_candidates,
        });
    }

    let mut powers = vec![0; family.denominator_count()];
    let mut unique = HashSet::new();
    enumerate_seeds(
        family,
        config,
        0,
        config.max_dots,
        config.max_numerator_degree,
        &mut powers,
        &mut unique,
    );
    let mut seeds: Vec<_> = unique.into_iter().collect();
    seeds.sort_by(|left, right| family.compare_integrals(right, left));
    Ok(seeds)
}

fn seed_candidate_upper_bound(family: &VacuumFamily, config: SeedConfig) -> u128 {
    let propagators = family.propagator_count();
    let auxiliaries = family.denominator_count() - propagators;
    if !config.include_subsectors {
        return saturating_product(
            weak_composition_count(config.max_dots, propagators),
            weak_composition_count(config.max_numerator_degree, auxiliaries),
        );
    }

    (0..=propagators).fold(0_u128, |total, active| {
        let subsets = binomial_count(propagators as u128, active as u128);
        let dots = weak_composition_count(config.max_dots, active);
        let numerators = weak_composition_count(
            config.max_numerator_degree,
            auxiliaries + propagators - active,
        );
        total.saturating_add(saturating_product(
            subsets,
            saturating_product(dots, numerators),
        ))
    })
}

fn weak_composition_count(maximum_sum: u32, variables: usize) -> u128 {
    // Number of non-negative vectors of length `variables` with sum <= M.
    binomial_count(
        u128::from(maximum_sum).saturating_add(variables as u128),
        variables as u128,
    )
}

fn binomial_count(n: u128, k: u128) -> u128 {
    let k = k.min(n.saturating_sub(k));
    let mut result = 1_u128;
    for index in 1..=k {
        let factor = n - k + index;
        result = result
            .checked_mul(factor)
            .and_then(|value| value.checked_div(index))
            .unwrap_or(u128::MAX);
        if result == u128::MAX {
            break;
        }
    }
    result
}

fn saturating_product(left: u128, right: u128) -> u128 {
    left.saturating_mul(right)
}

fn enumerate_seeds(
    family: &VacuumFamily,
    config: SeedConfig,
    position: usize,
    remaining_dots: u32,
    remaining_numerator_degree: u32,
    powers: &mut [i32],
    output: &mut HashSet<Integral>,
) {
    if position == powers.len() {
        let candidate = Integral::new(powers.to_vec());
        if let Some(canonical) = family.canonicalize(&candidate) {
            output.insert(canonical);
        }
        return;
    }

    if family.is_propagator(position) {
        if config.include_subsectors {
            for degree in 0..=remaining_numerator_degree {
                powers[position] = -(degree as i32);
                enumerate_seeds(
                    family,
                    config,
                    position + 1,
                    remaining_dots,
                    remaining_numerator_degree - degree,
                    powers,
                    output,
                );
            }
        }
        for dots in 0..=remaining_dots {
            powers[position] = 1 + dots as i32;
            enumerate_seeds(
                family,
                config,
                position + 1,
                remaining_dots - dots,
                remaining_numerator_degree,
                powers,
                output,
            );
        }
    } else {
        for degree in 0..=remaining_numerator_degree {
            powers[position] = -(degree as i32);
            enumerate_seeds(
                family,
                config,
                position + 1,
                remaining_dots,
                remaining_numerator_degree - degree,
                powers,
                output,
            );
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReductionStats {
    pub input_equations: usize,
    pub rules: usize,
    pub dependent_equations: usize,
    pub maximum_terms: usize,
}

fn validate_integral_arity(
    family: &VacuumFamily,
    integral: &Integral,
) -> Result<(), ReductionError> {
    let expected = family.denominator_count();
    let actual = integral.powers().len();
    if actual != expected {
        return Err(ReductionError::WrongIntegralArity {
            integral: integral.clone(),
            expected,
            actual,
        });
    }
    Ok(())
}

fn canonicalize_combination(
    family: &VacuumFamily,
    combination: &LinearCombination,
) -> Result<LinearCombination, ReductionError> {
    let mut canonical = LinearCombination::new();
    for (integral, coefficient) in combination.terms() {
        validate_integral_arity(family, integral)?;
        if let Some(integral) = family.canonicalize(integral) {
            canonical.add_term(integral, coefficient.clone());
        }
    }
    Ok(canonical)
}

fn validate_and_canonicalize_identities(
    family: &VacuumFamily,
    identities: &[IbpIdentity],
) -> Result<Vec<LinearCombination>, ReductionError> {
    // A generated batch contains L^2 rows for each seed.  Cache the expected
    // rows per seed so provenance checking costs one regeneration per seed,
    // rather than one regeneration per row.
    let mut expected_by_seed: HashMap<Integral, Vec<LinearCombination>> = HashMap::new();
    let mut validated = Vec::with_capacity(identities.len());

    for identity in identities {
        validate_integral_arity(family, &identity.seed)?;
        if identity.differentiated_loop >= family.loops()
            || identity.contraction_loop >= family.loops()
        {
            return Err(ReductionError::IdentityLoopOutOfRange {
                seed: identity.seed.clone(),
                differentiated_loop: identity.differentiated_loop,
                contraction_loop: identity.contraction_loop,
                loops: family.loops(),
            });
        }

        let actual = canonicalize_combination(family, &identity.equation)?;
        if !expected_by_seed.contains_key(&identity.seed) {
            let expected = IbpGenerator::new(family)
                .try_generate_raw(&identity.seed)?
                .into_iter()
                .map(|identity| canonicalize_combination(family, &identity.equation))
                .collect::<Result<Vec<_>, _>>()?;
            expected_by_seed.insert(identity.seed.clone(), expected);
        }
        let row = identity
            .differentiated_loop
            .checked_mul(family.loops())
            .and_then(|row| row.checked_add(identity.contraction_loop));
        let Some(expected) = expected_by_seed
            .get(&identity.seed)
            .and_then(|rows| row.and_then(|row| rows.get(row)))
        else {
            // The bounds above and IbpGenerator's L^2 contract make this
            // unreachable for a valid family, but keep the public boundary
            // panic-free if either invariant ever changes.
            return Err(ReductionError::IdentityLoopOutOfRange {
                seed: identity.seed.clone(),
                differentiated_loop: identity.differentiated_loop,
                contraction_loop: identity.contraction_loop,
                loops: family.loops(),
            });
        };
        if &actual != expected {
            return Err(ReductionError::IdentityEquationMismatch {
                seed: identity.seed.clone(),
                differentiated_loop: identity.differentiated_loop,
                contraction_loop: identity.contraction_loop,
                expected: expected.clone(),
                actual,
            });
        }
        validated.push(actual);
    }

    Ok(validated)
}

#[derive(Clone, Debug)]
pub struct ReductionTable {
    family: VacuumFamily,
    rules: HashMap<Integral, LinearCombination>,
    stats: ReductionStats,
}

impl ReductionTable {
    pub fn family(&self) -> &VacuumFamily {
        &self.family
    }

    pub fn rules(&self) -> &HashMap<Integral, LinearCombination> {
        &self.rules
    }

    pub fn stats(&self) -> &ReductionStats {
        &self.stats
    }

    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ReductionError> {
        validate_integral_arity(&self.family, integral)?;
        let Some(canonical) = self.family.canonicalize(integral) else {
            return Ok(LinearCombination::new());
        };
        let mut memo = HashMap::new();
        let mut visiting = HashSet::new();
        self.reduce_canonical(&canonical, &mut memo, &mut visiting)
    }

    pub fn reduce_combination(
        &self,
        combination: &LinearCombination,
    ) -> Result<LinearCombination, ReductionError> {
        let combination = canonicalize_combination(&self.family, combination)?;
        let mut output = LinearCombination::new();
        let mut memo = HashMap::new();
        let mut visiting = HashSet::new();
        for (integral, coefficient) in combination.terms() {
            let reduction = self.reduce_canonical(integral, &mut memo, &mut visiting)?;
            output.add_scaled(&reduction, coefficient);
        }
        Ok(output)
    }

    /// Validate caller-supplied IBP metadata and equations, returning the
    /// canonical equations that passed the generator-oracle check.
    ///
    /// Integrated pipelines use this crate-internal surface before applying
    /// their boundary-aware normal forms.  Calling [`Self::validate_identities`]
    /// there would incorrectly require the sparse table alone to close rows
    /// whose one-step IBP halo is handled by an analytic boundary reducer.
    pub(crate) fn validate_identity_provenance(
        &self,
        identities: &[IbpIdentity],
    ) -> Result<Vec<LinearCombination>, ReductionError> {
        validate_and_canonicalize_identities(&self.family, identities)
    }

    /// Validate exact generated (or canonically equivalent raw) IBP rows
    /// against this table.
    ///
    /// Since [`IbpIdentity`] has public fields, callers can construct rows
    /// directly.  Validate their seed, loop labels, exponent-vector arity,
    /// and exact total-derivative equation before treating them as IBPs.
    pub fn validate_identities(&self, identities: &[IbpIdentity]) -> Result<(), ReductionError> {
        let equations = self.validate_identity_provenance(identities)?;
        for (identity, equation) in identities.iter().zip(equations) {
            let remainder = self.reduce_combination(&equation)?;
            if !remainder.is_zero() {
                return Err(ReductionError::IdentityRemainder {
                    seed: identity.seed.clone(),
                    differentiated_loop: identity.differentiated_loop,
                    contraction_loop: identity.contraction_loop,
                    remainder,
                });
            }
        }
        Ok(())
    }

    /// Write a deterministic, versioned reduction database.
    pub fn write<W: Write>(&self, writer: W) -> Result<(), ReductionCacheError> {
        self.write_with_limits(writer, ReductionCacheLimits::default())
    }

    pub fn write_with_limits<W: Write>(
        &self,
        mut writer: W,
        limits: ReductionCacheLimits,
    ) -> Result<(), ReductionCacheError> {
        validate_cache_stats(&self.stats, self.rules.len(), &self.rules)?;
        if self.stats.input_equations > limits.max_input_equations {
            return Err(ReductionCacheError::ResourceLimit(format!(
                "input equation count {} exceeds {}",
                self.stats.input_equations, limits.max_input_equations
            )));
        }
        if self.rules.len() > limits.max_rules {
            return Err(ReductionCacheError::ResourceLimit(format!(
                "rule count {} exceeds {}",
                self.rules.len(),
                limits.max_rules
            )));
        }

        let mut payload = CachePayloadWriter::new(limits);
        payload.write_string(&self.family.fingerprint())?;
        payload
            .write_u32(u32::try_from(self.family.denominator_count()).map_err(|_| {
                ReductionCacheError::InvalidFormat("too many denominators".into())
            })?)?;
        for value in [
            self.stats.input_equations,
            self.stats.rules,
            self.stats.dependent_equations,
            self.stats.maximum_terms,
        ] {
            payload.write_u64(u64::try_from(value).map_err(|_| {
                ReductionCacheError::InvalidFormat("statistic exceeds u64".into())
            })?)?;
        }

        let mut rules: Vec<_> = self.rules.iter().collect();
        rules.sort_by(|(left, _), (right, _)| left.cmp(right));
        payload.write_u64(
            u64::try_from(rules.len())
                .map_err(|_| ReductionCacheError::InvalidFormat("too many rules".into()))?,
        )?;
        let mut total_terms = 0_usize;
        for (pivot, rhs) in rules {
            validate_cached_integral(&self.family, pivot, "rule pivot")?;
            if rhs.len() > limits.max_terms_per_rule {
                return Err(ReductionCacheError::ResourceLimit(format!(
                    "rule for {pivot} has {} terms, exceeding {}",
                    rhs.len(),
                    limits.max_terms_per_rule
                )));
            }
            total_terms = total_terms.checked_add(rhs.len()).ok_or_else(|| {
                ReductionCacheError::ResourceLimit("total cached term count overflow".into())
            })?;
            if total_terms > limits.max_total_terms {
                return Err(ReductionCacheError::ResourceLimit(format!(
                    "total term count {total_terms} exceeds {}",
                    limits.max_total_terms
                )));
            }
            payload.write_integral(pivot, self.family.denominator_count())?;
            payload.write_u64(u64::try_from(rhs.len()).map_err(|_| {
                ReductionCacheError::InvalidFormat("too many terms in rule".into())
            })?)?;
            for (integral, coefficient) in rhs.terms() {
                validate_cached_integral(&self.family, integral, "right-hand-side integral")?;
                if coefficient.is_zero() {
                    return Err(ReductionCacheError::InvalidFormat(format!(
                        "cached rule for {pivot} contains an explicit zero coefficient"
                    )));
                }
                payload.write_integral(integral, self.family.denominator_count())?;
                payload.write_string(&coefficient.to_expression().to_canonical_string())?;
            }
        }

        let payload = payload.finish();
        writer.write_all(b"RUSTRED\0")?;
        write_u32(&mut writer, 2)?;
        write_u64(
            &mut writer,
            u64::try_from(payload.len()).map_err(|_| {
                ReductionCacheError::ResourceLimit("cache payload exceeds u64".into())
            })?,
        )?;
        write_u64(&mut writer, fnv1a64(&payload))?;
        writer.write_all(&payload)?;
        Ok(())
    }

    /// Load a reduction database for an already validated family.
    pub fn read<R: Read>(family: VacuumFamily, reader: R) -> Result<Self, ReductionCacheError> {
        Self::read_with_limits(family, reader, ReductionCacheLimits::default())
    }

    pub fn read_with_limits<R: Read>(
        family: VacuumFamily,
        mut reader: R,
        limits: ReductionCacheLimits,
    ) -> Result<Self, ReductionCacheError> {
        let mut magic = [0_u8; 8];
        reader.read_exact(&mut magic)?;
        if &magic != b"RUSTRED\0" {
            return Err(ReductionCacheError::InvalidFormat(
                "bad RustRed cache magic".into(),
            ));
        }
        if read_u32(&mut reader)? != 2 {
            return Err(ReductionCacheError::InvalidFormat(
                "unsupported RustRed cache version".into(),
            ));
        }
        let payload_length = usize::try_from(read_u64(&mut reader)?).map_err(|_| {
            ReductionCacheError::ResourceLimit("cache payload exceeds this platform".into())
        })?;
        if payload_length > limits.max_payload_bytes {
            return Err(ReductionCacheError::ResourceLimit(format!(
                "cache payload has {payload_length} bytes, exceeding {}",
                limits.max_payload_bytes
            )));
        }
        let expected_checksum = read_u64(&mut reader)?;
        let mut payload = Vec::new();
        payload.try_reserve_exact(payload_length).map_err(|error| {
            ReductionCacheError::ResourceLimit(format!(
                "cannot allocate {payload_length}-byte cache payload: {error}"
            ))
        })?;
        payload.resize(payload_length, 0);
        reader.read_exact(&mut payload)?;
        if fnv1a64(&payload) != expected_checksum {
            return Err(ReductionCacheError::InvalidFormat(
                "cache payload checksum mismatch".into(),
            ));
        }
        let mut trailing = [0_u8; 1];
        if reader.read(&mut trailing)? != 0 {
            return Err(ReductionCacheError::InvalidFormat(
                "trailing bytes after cache payload".into(),
            ));
        }

        let mut payload_reader = io::Cursor::new(payload.as_slice());
        let cached_family = read_string(&mut payload_reader, limits.max_string_bytes)?;
        if cached_family != family.fingerprint() {
            return Err(ReductionCacheError::InvalidFormat(format!(
                "cache family fingerprint does not match {:?}",
                family.name(),
            )));
        }
        let denominator_count = read_u32(&mut payload_reader)? as usize;
        if denominator_count != family.denominator_count() {
            return Err(ReductionCacheError::InvalidFormat(
                "cache denominator count does not match family".into(),
            ));
        }
        let stats = ReductionStats {
            input_equations: read_usize(&mut payload_reader, "input equation count")?,
            rules: read_usize(&mut payload_reader, "rule statistic")?,
            dependent_equations: read_usize(&mut payload_reader, "dependent equation count")?,
            maximum_terms: read_usize(&mut payload_reader, "maximum term count")?,
        };
        if stats.input_equations > limits.max_input_equations {
            return Err(ReductionCacheError::ResourceLimit(format!(
                "input equation count {} exceeds {}",
                stats.input_equations, limits.max_input_equations
            )));
        }
        let rule_count = read_usize(&mut payload_reader, "rule count")?;
        if rule_count > limits.max_rules {
            return Err(ReductionCacheError::ResourceLimit(format!(
                "rule count {rule_count} exceeds {}",
                limits.max_rules
            )));
        }
        let mut rules = HashMap::new();
        rules.try_reserve(rule_count).map_err(|error| {
            ReductionCacheError::ResourceLimit(format!(
                "cannot allocate {rule_count} cached rules: {error}"
            ))
        })?;
        let mut previous_pivot: Option<Integral> = None;
        let mut total_terms = 0_usize;
        for _ in 0..rule_count {
            let pivot = read_integral(&mut payload_reader, denominator_count)?;
            validate_cached_integral(&family, &pivot, "rule pivot")?;
            if previous_pivot
                .as_ref()
                .is_some_and(|previous| previous >= &pivot)
            {
                return Err(ReductionCacheError::InvalidFormat(format!(
                    "cached rule pivots are not in strict canonical order at {pivot}"
                )));
            }
            previous_pivot = Some(pivot.clone());
            let term_count = read_usize(&mut payload_reader, "term count")?;
            if term_count > limits.max_terms_per_rule {
                return Err(ReductionCacheError::ResourceLimit(format!(
                    "rule for {pivot} has {term_count} terms, exceeding {}",
                    limits.max_terms_per_rule
                )));
            }
            total_terms = total_terms.checked_add(term_count).ok_or_else(|| {
                ReductionCacheError::ResourceLimit("total cached term count overflow".into())
            })?;
            if total_terms > limits.max_total_terms {
                return Err(ReductionCacheError::ResourceLimit(format!(
                    "total term count {total_terms} exceeds {}",
                    limits.max_total_terms
                )));
            }
            let mut rhs = LinearCombination::new();
            let mut previous_integral: Option<Integral> = None;
            for _ in 0..term_count {
                let integral = read_integral(&mut payload_reader, denominator_count)?;
                validate_cached_integral(&family, &integral, "right-hand-side integral")?;
                if previous_integral
                    .as_ref()
                    .is_some_and(|previous| previous >= &integral)
                {
                    return Err(ReductionCacheError::InvalidFormat(format!(
                        "right-hand side for {pivot} is not in strict canonical order at {integral}"
                    )));
                }
                previous_integral = Some(integral.clone());
                let expression = read_string(&mut payload_reader, limits.max_string_bytes)?;
                let coefficient = family
                    .coefficients()
                    .parse(&expression)
                    .map_err(ReductionCacheError::Coefficient)?;
                if coefficient.is_zero() {
                    return Err(ReductionCacheError::InvalidFormat(format!(
                        "cached rule for {pivot} contains an explicit zero coefficient"
                    )));
                }
                let canonical_expression = coefficient.to_expression().to_canonical_string();
                if expression != canonical_expression {
                    return Err(ReductionCacheError::InvalidFormat(format!(
                        "cached coefficient for {integral} is not canonical"
                    )));
                }
                rhs.add_term(integral, coefficient);
            }
            if let Some(offending) = rhs.terms().keys().find(|integral| {
                family.compare_integrals(integral, &pivot) != std::cmp::Ordering::Less
            }) {
                return Err(ReductionCacheError::InvalidFormat(format!(
                    "non-triangular cached rule {pivot} -> ... {offending} ..."
                )));
            }
            if rules.insert(pivot.clone(), rhs).is_some() {
                return Err(ReductionCacheError::InvalidFormat(format!(
                    "duplicate cached rule for {pivot}"
                )));
            }
        }
        if payload_reader.position() != payload.len() as u64 {
            return Err(ReductionCacheError::InvalidFormat(
                "unused bytes inside cache payload".into(),
            ));
        }
        validate_cache_stats(&stats, rules.len(), &rules)?;
        Ok(Self {
            family,
            rules,
            stats,
        })
    }

    fn reduce_canonical(
        &self,
        integral: &Integral,
        memo: &mut HashMap<Integral, LinearCombination>,
        visiting: &mut HashSet<Integral>,
    ) -> Result<LinearCombination, ReductionError> {
        if let Some(cached) = memo.get(integral) {
            return Ok(cached.clone());
        }
        if !visiting.insert(integral.clone()) {
            return Err(ReductionError::CyclicRule(integral.clone()));
        }

        let result = if let Some(rule) = self.rules.get(integral) {
            let mut reduced = LinearCombination::new();
            for (rhs_integral, coefficient) in rule.terms() {
                let rhs = self.reduce_canonical(rhs_integral, memo, visiting)?;
                reduced.add_scaled(&rhs, coefficient);
            }
            reduced
        } else {
            LinearCombination::from_term(integral.clone(), self.family.coefficients().one())
        };

        visiting.remove(integral);
        memo.insert(integral.clone(), result.clone());
        Ok(result)
    }
}

pub struct SparseReducer {
    family: VacuumFamily,
}

impl SparseReducer {
    pub fn new(family: VacuumFamily) -> Self {
        Self { family }
    }

    /// Eliminate checked IBP rows into a triangular reduction table.
    ///
    /// Externally constructed identities are re-derived from their seed and
    /// loop labels, then compared after symmetry and zero-sector
    /// canonicalization.  This prevents a forged or malformed row from being
    /// silently installed as a reduction rule.
    pub fn reduce(&self, identities: &[IbpIdentity]) -> Result<ReductionTable, ReductionError> {
        let validated_equations = validate_and_canonicalize_identities(&self.family, identities)?;
        let zero_equations = validated_equations
            .iter()
            .filter(|equation| equation.is_zero())
            .count();
        let mut equations: Vec<LinearCombination> = validated_equations
            .into_iter()
            .filter(|equation| !equation.is_zero())
            .collect();
        equations.sort_by(|left, right| {
            let left = self.leading_integral(left);
            let right = self.leading_integral(right);
            match (left, right) {
                (Some(left), Some(right)) => self.family.compare_integrals(right, left),
                _ => std::cmp::Ordering::Equal,
            }
        });

        let mut table = ReductionTable {
            family: self.family.clone(),
            rules: HashMap::new(),
            stats: ReductionStats {
                input_equations: identities.len(),
                dependent_equations: zero_equations,
                ..ReductionStats::default()
            },
        };

        for mut equation in equations {
            table.stats.maximum_terms = table.stats.maximum_terms.max(equation.len());
            self.apply_known_rules(&mut equation, &table.rules);
            table.stats.maximum_terms = table.stats.maximum_terms.max(equation.len());
            if equation.is_zero() {
                table.stats.dependent_equations += 1;
                continue;
            }

            let pivot = self
                .leading_integral(&equation)
                .expect("a nonzero equation has a leading integral")
                .clone();
            let pivot_coefficient = equation
                .remove(&pivot)
                .expect("the leading integral occurs in the equation");
            let minus_one = -table.family.coefficients().one();
            let normalization = &minus_one / &pivot_coefficient;
            let rule = equation.scaled(&normalization);

            if let Some(offending) = rule.terms().keys().find(|integral| {
                self.family.compare_integrals(integral, &pivot) != std::cmp::Ordering::Less
            }) {
                return Err(ReductionError::NonTriangularRule {
                    pivot,
                    rhs: offending.clone(),
                });
            }
            table.rules.insert(pivot, rule);
        }
        table.stats.rules = table.rules.len();
        Ok(table)
    }

    fn apply_known_rules(
        &self,
        equation: &mut LinearCombination,
        rules: &HashMap<Integral, LinearCombination>,
    ) {
        loop {
            let reducible = equation
                .terms()
                .keys()
                .filter(|integral| rules.contains_key(*integral))
                .max_by(|left, right| self.family.compare_integrals(left, right))
                .cloned();
            let Some(integral) = reducible else {
                break;
            };
            let coefficient = equation
                .remove(&integral)
                .expect("selected reducible term exists");
            equation.add_scaled(
                rules
                    .get(&integral)
                    .expect("selected reducible term has a rule"),
                &coefficient,
            );
        }
    }

    fn leading_integral<'a>(&self, equation: &'a LinearCombination) -> Option<&'a Integral> {
        equation
            .terms()
            .keys()
            .max_by(|left, right| self.family.compare_integrals(left, right))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReductionError {
    WrongIntegralArity {
        integral: Integral,
        expected: usize,
        actual: usize,
    },
    IdentityLoopOutOfRange {
        seed: Integral,
        differentiated_loop: usize,
        contraction_loop: usize,
        loops: usize,
    },
    IdentityEquationMismatch {
        seed: Integral,
        differentiated_loop: usize,
        contraction_loop: usize,
        expected: LinearCombination,
        actual: LinearCombination,
    },
    IdentityGeneration(IbpGenerationError),
    NonTriangularRule {
        pivot: Integral,
        rhs: Integral,
    },
    CyclicRule(Integral),
    IdentityRemainder {
        seed: Integral,
        differentiated_loop: usize,
        contraction_loop: usize,
        remainder: LinearCombination,
    },
}

impl std::fmt::Display for ReductionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongIntegralArity {
                integral,
                expected,
                actual,
            } => write!(
                formatter,
                "integral {integral} has {actual} powers, expected {expected} for this family"
            ),
            Self::IdentityLoopOutOfRange {
                seed,
                differentiated_loop,
                contraction_loop,
                loops,
            } => write!(
                formatter,
                "IBP for seed {seed} uses derivative {differentiated_loop} and contraction {contraction_loop}, but the family has {loops} loops"
            ),
            Self::IdentityEquationMismatch {
                seed,
                differentiated_loop,
                contraction_loop,
                ..
            } => write!(
                formatter,
                "supplied IBP equation for seed {seed}, derivative {differentiated_loop}, contraction {contraction_loop} does not match the exact total-derivative row"
            ),
            Self::IdentityGeneration(error) => {
                write!(formatter, "cannot validate supplied IBP identity: {error}")
            }
            Self::NonTriangularRule { pivot, rhs } => {
                write!(
                    formatter,
                    "non-triangular reduction rule {pivot} -> ... {rhs} ..."
                )
            }
            Self::CyclicRule(integral) => write!(formatter, "cyclic reduction rule for {integral}"),
            Self::IdentityRemainder {
                seed,
                differentiated_loop,
                contraction_loop,
                ..
            } => write!(
                formatter,
                "IBP for seed {seed}, derivative {differentiated_loop}, contraction {contraction_loop} has a nonzero remainder"
            ),
        }
    }
}

impl std::error::Error for ReductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IdentityGeneration(error) => Some(error),
            _ => None,
        }
    }
}

impl From<IbpGenerationError> for ReductionError {
    fn from(value: IbpGenerationError) -> Self {
        Self::IdentityGeneration(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReductionCacheLimits {
    pub max_payload_bytes: usize,
    pub max_string_bytes: usize,
    pub max_input_equations: usize,
    pub max_rules: usize,
    pub max_terms_per_rule: usize,
    pub max_total_terms: usize,
}

impl Default for ReductionCacheLimits {
    fn default() -> Self {
        Self {
            max_payload_bytes: 256 * 1024 * 1024,
            max_string_bytes: 16 * 1024 * 1024,
            max_input_equations: 100_000_000,
            max_rules: 2_000_000,
            max_terms_per_rule: 1_000_000,
            max_total_terms: 10_000_000,
        }
    }
}

#[derive(Debug)]
pub enum ReductionCacheError {
    Io(io::Error),
    InvalidFormat(String),
    ResourceLimit(String),
    Coefficient(String),
}

impl std::fmt::Display for ReductionCacheError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "reduction cache I/O error: {error}"),
            Self::InvalidFormat(error) => write!(formatter, "invalid reduction cache: {error}"),
            Self::ResourceLimit(error) => write!(formatter, "reduction cache limit: {error}"),
            Self::Coefficient(error) => write!(formatter, "invalid cached coefficient: {error}"),
        }
    }
}

impl std::error::Error for ReductionCacheError {}

impl From<io::Error> for ReductionCacheError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

fn write_u32(writer: &mut impl Write, value: u32) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn write_u64(writer: &mut impl Write, value: u64) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> io::Result<u64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_usize(reader: &mut impl Read, description: &str) -> Result<usize, ReductionCacheError> {
    usize::try_from(read_u64(reader)?).map_err(|_| {
        ReductionCacheError::InvalidFormat(format!("{description} exceeds this platform"))
    })
}

fn read_string(
    reader: &mut impl Read,
    maximum_bytes: usize,
) -> Result<String, ReductionCacheError> {
    let length = read_usize(reader, "string length")?;
    if length > maximum_bytes {
        return Err(ReductionCacheError::ResourceLimit(format!(
            "cache string has {length} bytes, exceeding {maximum_bytes}"
        )));
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|error| {
        ReductionCacheError::ResourceLimit(format!(
            "cannot allocate {length}-byte cache string: {error}"
        ))
    })?;
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|error| ReductionCacheError::InvalidFormat(error.to_string()))
}

fn read_integral(
    reader: &mut impl Read,
    denominator_count: usize,
) -> Result<Integral, ReductionCacheError> {
    let mut powers = Vec::with_capacity(denominator_count);
    for _ in 0..denominator_count {
        let mut bytes = [0; 4];
        reader.read_exact(&mut bytes)?;
        powers.push(i32::from_le_bytes(bytes));
    }
    Ok(Integral::new(powers))
}

fn validate_cached_integral(
    family: &VacuumFamily,
    integral: &Integral,
    description: &str,
) -> Result<(), ReductionCacheError> {
    match family.canonicalize(integral) {
        None => Err(ReductionCacheError::InvalidFormat(format!(
            "{description} {integral} is scaleless"
        ))),
        Some(canonical) if canonical != *integral => Err(ReductionCacheError::InvalidFormat(
            format!("{description} {integral} is not symmetry-canonical (expected {canonical})"),
        )),
        Some(_) => Ok(()),
    }
}

fn validate_cache_stats(
    stats: &ReductionStats,
    rule_count: usize,
    rules: &HashMap<Integral, LinearCombination>,
) -> Result<(), ReductionCacheError> {
    if stats.rules != rule_count {
        return Err(ReductionCacheError::InvalidFormat(
            "cache rule statistic is inconsistent".into(),
        ));
    }
    let accounted_equations = stats
        .rules
        .checked_add(stats.dependent_equations)
        .ok_or_else(|| {
            ReductionCacheError::InvalidFormat("cache equation statistics overflow".into())
        })?;
    if stats.input_equations != accounted_equations {
        return Err(ReductionCacheError::InvalidFormat(format!(
            "cache equation statistics are inconsistent: {} inputs != {} rules + {} dependent",
            stats.input_equations, stats.rules, stats.dependent_equations
        )));
    }
    let largest_serialized_equation = rules
        .values()
        .map(|rhs| rhs.len().saturating_add(1))
        .max()
        .unwrap_or(0);
    if stats.maximum_terms < largest_serialized_equation {
        return Err(ReductionCacheError::InvalidFormat(format!(
            "maximum-term statistic {} is smaller than cached rule size {largest_serialized_equation}",
            stats.maximum_terms
        )));
    }
    Ok(())
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

struct CachePayloadWriter {
    bytes: Vec<u8>,
    limits: ReductionCacheLimits,
}

impl CachePayloadWriter {
    fn new(limits: ReductionCacheLimits) -> Self {
        Self {
            bytes: Vec::new(),
            limits,
        }
    }

    fn write_u32(&mut self, value: u32) -> Result<(), ReductionCacheError> {
        self.extend(&value.to_le_bytes())
    }

    fn write_u64(&mut self, value: u64) -> Result<(), ReductionCacheError> {
        self.extend(&value.to_le_bytes())
    }

    fn write_string(&mut self, value: &str) -> Result<(), ReductionCacheError> {
        if value.len() > self.limits.max_string_bytes {
            return Err(ReductionCacheError::ResourceLimit(format!(
                "cache string has {} bytes, exceeding {}",
                value.len(),
                self.limits.max_string_bytes
            )));
        }
        self.write_u64(
            u64::try_from(value.len()).map_err(|_| {
                ReductionCacheError::ResourceLimit("cache string exceeds u64".into())
            })?,
        )?;
        self.extend(value.as_bytes())
    }

    fn write_integral(
        &mut self,
        integral: &Integral,
        denominator_count: usize,
    ) -> Result<(), ReductionCacheError> {
        if integral.powers().len() != denominator_count {
            return Err(ReductionCacheError::InvalidFormat(
                "integral has the wrong exponent-vector length".into(),
            ));
        }
        for &power in integral.powers() {
            self.extend(&power.to_le_bytes())?;
        }
        Ok(())
    }

    fn extend(&mut self, bytes: &[u8]) -> Result<(), ReductionCacheError> {
        let new_length = self.bytes.len().checked_add(bytes.len()).ok_or_else(|| {
            ReductionCacheError::ResourceLimit("cache payload size overflow".into())
        })?;
        if new_length > self.limits.max_payload_bytes {
            return Err(ReductionCacheError::ResourceLimit(format!(
                "cache payload exceeds {} bytes",
                self.limits.max_payload_bytes
            )));
        }
        self.bytes.try_reserve(bytes.len()).map_err(|error| {
            ReductionCacheError::ResourceLimit(format!(
                "cannot grow cache payload to {new_length} bytes: {error}"
            ))
        })?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
