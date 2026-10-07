//! Storage padding at the runtime bridge boundary. The physical family and
//! its identities, cuts and zero certificates are never padded.

use super::*;
use symbolica::poly::PolyVariable;

fn check<const N: usize>(family: &IntegralFamily) -> Result<usize, SolverError> {
    let arity = family.denominator_count();
    if arity == 0 || arity > N {
        return Err(SolverError::InvalidInput(format!(
            "solver capacity {N} cannot hold {arity} denominator coordinates"
        )));
    }
    Ok(arity)
}

fn padded<T: Clone>(
    values: &[T],
    arity: usize,
    capacity: usize,
    zero: T,
) -> Result<Vec<T>, SolverError> {
    if values.len() != arity {
        return Err(SolverError::InvalidInput(format!(
            "expected {arity} physical coordinates, got {}",
            values.len()
        )));
    }
    let mut result = values.to_vec();
    result.resize(capacity, zero);
    Ok(result)
}

/// Use inline capacity N for a smaller physical family. Returned powers and
/// coefficient variable maps retain the physical family's exact arity.
pub fn solve_parametric_with_capacity<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    sector: &[bool],
    fixed: &[Option<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    let arity = check::<N>(family)?;
    let sector = padded(sector, arity, N, false)?;
    let fixed = padded(fixed, arity, N, Some(0))?;
    let mut result = parametric::<N>(family, cuts, &sector, &fixed, options)?;
    if arity < N {
        resize_solution(&mut result, N, arity)?;
    }
    Ok(result)
}

/// Reduce a family with at most N coordinates, using the same exact search
/// and replay as the exact-arity entry point.
pub fn solve_laporta_with_capacity<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    targets: &[Vec<i16>],
    preferred: &[Vec<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    let arity = check::<N>(family)?;
    let pad = |keys: &[Vec<i16>]| {
        keys.iter()
            .map(|key| padded(key, arity, N, 0))
            .collect::<Result<Vec<_>, _>>()
    };
    let mut result = laporta::<N>(family, cuts, &pad(targets)?, &pad(preferred)?, options)?;
    if arity < N {
        resize_solution(&mut result, N, arity)?;
    }
    Ok(result)
}

/// Certify a physical-arity solution using larger storage. Original IBP
/// identities are generated from the unmodified physical family.
pub fn certify_laporta_with_capacity<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    solution: &DynamicSolution,
    include_lorentz: bool,
) -> Result<ReductionCertificate, SolverError> {
    let arity = check::<N>(family)?;
    if arity == N {
        return certificate::certify::<N>(family, cuts, solution, include_lorentz);
    }
    let mut padded = solution.clone();
    resize_solution(&mut padded, arity, N)
        .map_err(|error| SolverError::Certification(error.to_string()))?;
    certificate::certify::<N>(family, cuts, &padded, include_lorentz)
}

fn resize<T: Clone + PartialEq>(
    values: &mut Vec<T>,
    from: usize,
    to: usize,
    zero: T,
) -> Result<(), SolverError> {
    if values.len() != from || (to < from && values[to..].iter().any(|value| *value != zero)) {
        return Err(SolverError::InvalidInput(
            "invalid or nonzero padded coordinate".into(),
        ));
    }
    values.resize(to, zero);
    Ok(())
}

fn physical_polynomial(polynomial: &mut CoefficientPolynomial) -> Result<(), SolverError> {
    // The padding constructor uses fresh Temporary variables. Physical index
    // contexts use named symbols. Reject dependence before removing metadata.
    let variables: Vec<_> = polynomial
        .variables()
        .iter()
        .filter(|variable| !matches!(variable, PolyVariable::Temporary(_)))
        .cloned()
        .collect();
    if variables.len() != polynomial.nvars() {
        *polynomial = polynomial
            .rearrange_with_growth(&variables)
            .map_err(SolverError::InvalidInput)?;
    }
    Ok(())
}

fn resize_rule(rule: &mut DynamicRule, from: usize, to: usize) -> Result<(), SolverError> {
    let zero = DynamicPower {
        symbolic: false,
        value: 0,
    };
    resize(&mut rule.sector, from, to, false)?;
    resize(&mut rule.target, from, to, zero)?;
    for term in &mut rule.rhs {
        resize(&mut term.powers, from, to, zero)?;
        if to < from {
            physical_polynomial(&mut term.coefficient.numerator)?;
            physical_polynomial(&mut term.coefficient.denominator)?;
            // Variable maps were changed together without altering values.
        }
    }
    if to < from {
        for polynomial in rule
            .nonzero_conditions
            .iter_mut()
            .chain(rule.exceptions.iter_mut().flatten())
        {
            physical_polynomial(polynomial)?;
        }
    }
    Ok(())
}

fn resize_solution(
    solution: &mut DynamicSolution,
    from: usize,
    to: usize,
) -> Result<(), SolverError> {
    for key in solution.requested.iter_mut().chain(&mut solution.residuals) {
        resize(key, from, to, 0)?;
    }
    for rule in &mut solution.rules {
        resize_rule(rule, from, to)?;
    }
    for derivation in &mut solution.derivation {
        resize_rule(&mut derivation.rule, from, to)?;
        if let RuleOrigin::Identities { seeds } = &mut derivation.origin {
            for key in seeds {
                resize(key, from, to, 0)?;
            }
        }
    }
    if let Some(basis) = &mut solution.basis_change {
        for key in &mut basis.replaced {
            resize(key, from, to, 0)?;
        }
        for master in &mut basis.preferred {
            resize(&mut master.integral, from, to, 0)?;
        }
        for rule in &mut basis.original {
            resize_rule(rule, from, to)?;
        }
        if to < from {
            for polynomial in &mut basis.conditions {
                physical_polynomial(polynomial)?;
            }
        }
    }
    if to < from {
        solution.index_variables.truncate(to);
    }
    Ok(())
}
