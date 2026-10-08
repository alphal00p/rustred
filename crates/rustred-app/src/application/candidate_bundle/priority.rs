//! Explicitly checked priority candidates, not certified owner artifacts.
//!
//! The original suffix and finite leaves are preserved. The ordinary-source
//! proof stays attached to the returned bytes; the candidate codec itself does
//! not acquire source-proof authority, nor does its empty seed trace claim it.

use rustred::algebra::{
    CoefficientPolynomial, ExactAlgebraLimits, IndexedCoefficientContext, IndexedPolynomial,
};
use rustred::foundry::artifact::{
    CheckedOriginalSourceCombination, OriginalSourceCombinationLimits,
    OriginalSourceCombinationRequest, check_original_source_combination,
};
use rustred::foundry::parametric::ParametricGuardOrigin;
use rustred::identity::ParametricIbpGenerator;
use rustred::persistence::{CoefficientId, CoefficientTableBuilder};
use rustred::solver::{
    CoordinateCase, Integral, RuleCandidate, RuleDispatchPolicy, SearchStats, SectorRule, Term,
    extract_exceptions,
};
use symbolica::prelude::*;

use super::{CandidateBundleLimits, codec, order};
use crate::AppError;

mod lower_cuts;

/// Immutable, proof-bound candidate bytes. Loading the bytes alone does not
/// replay this proof or confer certification, coverage, or basis minimality.
#[derive(Debug)]
pub struct CheckedPriorityOwnerExport {
    bytes: Vec<u8>,
    proof: CheckedOriginalSourceCombination,
    base_owner_blake3: String,
    owner_blake3: String,
    dispatch_policy: RuleDispatchPolicy,
}

impl CheckedPriorityOwnerExport {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn proof(&self) -> &CheckedOriginalSourceCombination {
        &self.proof
    }
    pub fn base_owner_blake3(&self) -> &str {
        &self.base_owner_blake3
    }
    pub fn owner_blake3(&self) -> &str {
        &self.owner_blake3
    }
    pub fn dispatch_policy(&self) -> RuleDispatchPolicy {
        self.dispatch_policy
    }
}

/// Prepend one checked coordinate-domain rule to a single saved owner.
///
/// This deliberately narrow bridge accepts only declared fixed coordinates
/// and free local axes `[lower, infinity)`, with a bounded finite `lower`.
/// Missing integer boundary slices are encoded as exact exclusions, not a
/// widened application domain. Unsupported
/// guards refuse the entire export: no shared family condition is strengthened.
/// Pure base-field poles arising only from contribution weights may be
/// discharged as generic-field units after the complete identity check. Their
/// conditions and origins remain in the returned proof, not erased from it.
/// Old rules, finite terminals, order, family and solver policy remain unchanged.
/// Terminals retain their existing priority over all rules. No search, canonical
/// pivot replay, terminal creation, owner installation or closure check occurs.
/// The existing candidate codec/loader currently supports 1..=16 indices; this
/// transport boundary is not a loop-count restriction of the source constructor.
pub fn encode_checked_priority_owner<const N: usize>(
    base_owner_bytes: &[u8],
    proposal: OriginalSourceCombinationRequest,
    proof_limits: OriginalSourceCombinationLimits,
    bundle_limits: CandidateBundleLimits,
) -> Result<CheckedPriorityOwnerExport, AppError> {
    encode_checked_priority_owner_with_policy::<N>(
        base_owner_bytes,
        proposal,
        proof_limits,
        bundle_limits,
        RuleDispatchPolicy::Partition,
    )
}

/// Checked owner export with an explicit persisted dispatch policy. A
/// post-baseline alternative retains the exact proof/case/RHS but cannot
/// split ordinary pieces, repair gaps, or override another prepared batch.
pub fn encode_checked_priority_owner_with_policy<const N: usize>(
    base_owner_bytes: &[u8],
    proposal: OriginalSourceCombinationRequest,
    proof_limits: OriginalSourceCombinationLimits,
    bundle_limits: CandidateBundleLimits,
    dispatch_policy: RuleDispatchPolicy,
) -> Result<CheckedPriorityOwnerExport, AppError> {
    if !(1..=16).contains(&N) {
        return Err(error(
            "priority owner arity must be between one and sixteen",
        ));
    }
    let base = codec::read(base_owner_bytes, bundle_limits)?;
    let physical_arity = base.root_sector.len();
    if base.sectors.len() != 1
        || !rustred::fits_storage(physical_arity, N)
        || proposal.root_sector.active_bits() != base.root_sector
        || proposal.sector.active_bits() != base.sectors[0].sector
        || proposal.ordering != order::saved_policy(&base.records)?
        || base
            .permutation
            .as_ref()
            .is_some_and(|p| p.iter().copied().ne(0..physical_arity))
    {
        return Err(error(
            "priority proposal must match one unpermuted saved owner/root/order",
        ));
    }
    let family = base
        .family
        .to_family(
            &base.coefficients,
            bundle_limits.family_limits(),
            bundle_limits.binary_limits(),
        )
        .map_err(error)?;
    if family.fingerprint() != base.family_fingerprint
        || family.denominator_count() != physical_arity
    {
        return Err(error("priority owner family fingerprint or arity mismatch"));
    }
    let generator = ParametricIbpGenerator::try_new(&family).map_err(error)?;
    let context = generator.context();
    let sector: [bool; N] = rustred::storage_array(&base.sectors[0].sector, false)
        .ok_or_else(|| error("priority owner sector arity"))?;
    let variables = codec::storage_variables::<N>(context);
    let boundary_limit = proof_limits
        .cell
        .max_guards
        .min(proof_limits.rule.max_rule_guards)
        .min(bundle_limits.max_collection_entries);
    let (face, boundary_cuts) = representable_case::<N>(&proposal, &sector, boundary_limit)?;
    let fixed: Vec<_> = proposal
        .fixed
        .iter()
        .map(|f| (f.position(), f.value()))
        .collect();
    if proposal.rhs.len() > proof_limits.cell.max_retained_terms {
        return Err(AppError::limit("priority RHS exceeds retained-term budget"));
    }
    let algebra = proof_limits.cell.indexed_algebra;
    let mut rhs = Vec::with_capacity(proposal.rhs.len());
    let mut runtime_denominators = Vec::new();
    for (shift, coefficient) in &proposal.rhs {
        if shift.values().len() != physical_arity {
            return Err(error("priority RHS shift arity mismatch"));
        }
        let (coefficient, _) = context
            .specialize_fixed_indices(coefficient, &fixed, algebra)
            .map_err(error)?;
        if coefficient.is_zero() {
            continue;
        }
        // Keep denominators of ALL surviving broad terms, even if a term later
        // vanishes on a sign cell. Runtime checks these before cancellation.
        runtime_denominators.push(
            context
                .denominator_condition_with_limits(&coefficient, algebra.exact_algebra)
                .map_err(error)?,
        );
        let mut powers = *face.integral().powers();
        for (power, &delta) in powers.iter_mut().zip(shift.values()) {
            *power = power
                .shifted(i16::try_from(delta).map_err(error)?)
                .map_err(error)?;
        }
        rhs.push(Term {
            integral: Integral::new(powers),
            coefficient: grow_coefficient(coefficient.raw(), &variables)?,
        });
    }
    if rhs.is_empty() {
        return Err(error(
            "priority bridge does not create empty-RHS rules or terminals",
        ));
    }
    let candidate = RuleCandidate {
        case: face.into(),
        target: face.integral(),
        rhs,
        sources: Vec::new(), // No fabricated canonical/seeded source provenance.
        stats: SearchStats::default(),
    };
    let proof =
        check_original_source_combination(&family, proposal, proof_limits).map_err(error)?;
    check_runtime_guards(context, &proof, &runtime_denominators, proof_limits)?;
    let base_variables = context.base().one().get_variables().len();
    let indices = std::array::from_fn(|axis| base_variables + axis);
    let mut exceptions = extract_exceptions(&candidate, &indices, &sector).map_err(error)?;
    boundary_cuts.check_existing(&exceptions.branches, boundary_limit)?;
    for boundary in boundary_cuts.polynomials(context, &sector, algebra.exact_algebra)? {
        exceptions.branches.push(vec![
            boundary
                .raw()
                .rearrange_with_growth(&variables)
                .map_err(error)?,
        ]);
    }
    let rule = SectorRule {
        dispatch_policy: RuleDispatchPolicy::Partition, // Equation-only transport below.
        candidate,
        exceptions,
    };
    let mut table = CoefficientTableBuilder::new(bundle_limits.binary_limits());
    for index in 0..base.coefficients.len() {
        let id = CoefficientId::try_from_index(index).map_err(codec::binary_error)?;
        let restored = table
            .intern(
                base.coefficients
                    .coefficient(id)
                    .map_err(codec::binary_error)?,
            )
            .map_err(codec::binary_error)?;
        if restored != id {
            return Err(error("priority base contains duplicate coefficient IDs"));
        }
    }
    let solution = rustred::solver::SectorSolution {
        order: rustred::solver::IntegralOrder::new(sector, [false; N])
            .with_physical_arity(physical_arity)
            .map_err(error)?,
        max_numerator_rank: None,
        finite_case_policy: rustred::solver::FiniteCasePolicy::default(),
        rules: vec![rule],
        finite_residuals: Vec::new(),
        stats: rustred::solver::SectorStats::default(),
    };
    let inserted = codec::physical_sector_record(physical_arity, sector, &solution, &mut table)?
        .rules
        .remove(0);
    let rule = &solution.rules[0];
    let mut records = base.records.clone();
    records.sectors[0].rules.insert(0, inserted.clone());
    for policy in &mut records.rule_dispatch {
        policy.rule += 1;
    }
    if dispatch_policy == RuleDispatchPolicy::AfterBaselinePartitionWholePiece {
        records.rule_dispatch.insert(
            0,
            super::model::RuleDispatchRecord {
                sector: 0,
                rule: 0,
                policy: 1,
            },
        );
        records.schema = super::model::DISPATCH_CANDIDATE_BUNDLE_SCHEMA.into();
    }
    let bytes = codec::write_records(
        &records,
        &base.family,
        &table.finish().map_err(codec::binary_error)?,
        bundle_limits,
    )?;
    let roundtrip = codec::read(&bytes, bundle_limits)?;
    if roundtrip.records != records || roundtrip.family != base.family {
        return Err(error(
            "priority owner metadata changed during native roundtrip",
        ));
    }
    if codec::dispatch::policy(&roundtrip.records, 0, 0) != dispatch_policy {
        return Err(error(
            "priority dispatch policy changed during native roundtrip",
        ));
    }
    for index in 0..base.coefficients.len() {
        let id = CoefficientId::try_from_index(index).map_err(codec::binary_error)?;
        if base
            .coefficients
            .coefficient(id)
            .map_err(codec::binary_error)?
            != roundtrip
                .coefficients
                .coefficient(id)
                .map_err(codec::binary_error)?
        {
            return Err(error("priority owner changed a suffix coefficient"));
        }
    }
    let restored = codec::rules::restore_rule::<N>(
        &roundtrip.sectors[0].rules[0],
        &roundtrip.coefficients,
        &variables,
        &indices,
        &sector,
    )?;
    if restored.candidate.case != rule.candidate.case
        || restored.candidate.target != rule.candidate.target
        || restored.candidate.rhs != rule.candidate.rhs
        || !restored.candidate.sources.is_empty()
        || restored.exceptions != rule.exceptions
    {
        return Err(error("priority broad rule changed during native roundtrip"));
    }
    Ok(CheckedPriorityOwnerExport {
        base_owner_blake3: blake3::hash(base_owner_bytes).to_hex().to_string(),
        owner_blake3: blake3::hash(&bytes).to_hex().to_string(),
        bytes,
        proof,
        dispatch_policy,
    })
}

fn grow_coefficient(
    value: &rustred::algebra::Coefficient,
    variables: &[PolyVariable],
) -> Result<rustred::algebra::Coefficient, AppError> {
    use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
    Ok(rustred::algebra::Coefficient::from_num_den(
        value
            .numerator
            .rearrange_with_growth(variables)
            .map_err(error)?,
        value
            .denominator
            .rearrange_with_growth(variables)
            .map_err(error)?,
        &Z,
        true,
    ))
}

fn representable_case<const N: usize>(
    proposal: &OriginalSourceCombinationRequest,
    sector: &[bool; N],
    boundary_limit: usize,
) -> Result<(CoordinateCase<N>, lower_cuts::LowerCuts), AppError> {
    let n = proposal.lower.len();
    if !rustred::fits_storage(n, N) || proposal.upper.len() != n || proposal.fixed.len() > n {
        return Err(error("priority coordinate-domain arity mismatch"));
    }
    let mut fixed = std::array::from_fn(|axis| (axis >= n).then_some(0));
    for restriction in &proposal.fixed {
        let axis = restriction.position();
        if axis >= n || fixed[axis].is_some() {
            return Err(error(
                "priority fixed coordinate is duplicate or out of range",
            ));
        }
        let value = restriction.value();
        let local = u64::try_from(if sector[axis] {
            i128::from(value) - 1
        } else {
            -i128::from(value)
        })
        .map_err(error)?;
        if proposal.lower[axis] != local || proposal.upper[axis] != Some(local) {
            return Err(error(
                "priority declared fixed face differs from its exact bounds",
            ));
        }
        fixed[axis] = Some(i16::try_from(value).map_err(error)?);
    }
    let boundary_cuts = lower_cuts::LowerCuts::new(
        &rustred::storage_array::<_, N>(&proposal.lower, 0).expect("validated priority arity"),
        &rustred::storage_array::<_, N>(&proposal.upper, Some(0))
            .expect("validated priority arity"),
        &fixed,
        sector,
        boundary_limit,
    )?;
    let face = CoordinateCase::new(fixed).map_err(error)?;
    if !face.is_in_sector(sector) {
        return Err(error("priority fixed face is outside its owner"));
    }
    Ok((face, boundary_cuts))
}

fn check_runtime_guards(
    context: &IndexedCoefficientContext,
    proof: &CheckedOriginalSourceCombination,
    runtime_denominators: &[IndexedPolynomial],
    limits: OriginalSourceCombinationLimits,
) -> Result<(), AppError> {
    for (ordinal, cell) in proof.cells().enumerate() {
        let (lower, upper) = proof
            .cell_bounds(ordinal)
            .expect("checked cell has exact bounds");
        let mut fixed = Vec::new();
        for axis in 0..lower.len() {
            if upper[axis] == Some(lower[axis]) {
                let value = if proof.sector().active_bits()[axis] {
                    i128::from(lower[axis]) + 1
                } else {
                    -i128::from(lower[axis])
                };
                fixed.push((axis, i64::try_from(value).map_err(error)?));
            }
        }
        let denominators = runtime_denominators
            .iter()
            .map(|p| {
                let p = context
                    .specialize_fixed_polynomial(p, &fixed, limits.cell.indexed_algebra)
                    .map_err(error)?;
                if p.is_zero() {
                    return Err(error(
                        "priority runtime denominator vanishes on a checked cell",
                    ));
                }
                Ok(primitive(&p))
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        for checked_guard in cell.rule().nonzero_guards() {
            let guard = context
                .specialize_fixed_polynomial(
                    checked_guard.polynomial(),
                    &fixed,
                    limits.cell.indexed_algebra,
                )
                .map_err(error)?;
            if guard.is_nonzero_constant()
                || is_generic_field_weight_pole(
                    context,
                    &guard,
                    checked_guard.origins(),
                    limits.cell.indexed_algebra.exact_algebra,
                )?
            {
                continue;
            }
            if guard.is_zero() || !denominators.contains(&primitive(&guard)) {
                return Err(error(
                    "priority proof guard is not retained by a surviving runtime RHS denominator",
                ));
            }
        }
    }
    Ok(())
}

// A nonzero element of Q(base parameters) is a unit. This does NOT discharge
// source hypotheses, caller-retained conditions, final/RHS poles, or a guard
// still depending on a free integral index. Origins come only from the checked
// original product; deduplicated guards must have exclusively weight origins.
// Exact cell specialization precedes this check, so no unspecialized index
// dependency is silently ignored. The checked proof itself is never modified.
fn is_generic_field_weight_pole(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    origins: &[ParametricGuardOrigin],
    limits: ExactAlgebraLimits,
) -> Result<bool, AppError> {
    context
        .validate_polynomial_with_limits(polynomial, limits)
        .map_err(error)?;
    if polynomial.is_zero()
        || origins.is_empty()
        || !origins.iter().all(|origin| {
            matches!(
                origin,
                ParametricGuardOrigin::SourceCombinationDenominator { .. }
            )
        })
    {
        return Ok(false);
    }
    // IndexedCoefficientContext authenticates the map as base variables
    // followed by every integral-index variable. Use native polynomial degree,
    // not display parsing or factor/implication guesses.
    let base_count = context.base().one().get_variables().len();
    Ok((0..context.index_count()).all(|axis| polynomial.raw().degree(base_count + axis) == 0))
}

// Native primitive associates only: no string parsing, factor guesses or
// general polynomial implication. Inputs were admitted under exact limits.
fn primitive(value: &IndexedPolynomial) -> CoefficientPolynomial {
    let mut p = value.raw().clone().make_primitive();
    if p.lcoeff().is_negative() {
        p = p.mul_coeff(Integer::from(-1));
    }
    p
}

fn error(error: impl std::fmt::Display) -> AppError {
    AppError::input(error.to_string())
}

#[cfg(test)]
mod tests;
