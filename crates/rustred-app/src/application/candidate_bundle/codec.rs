use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use rustred::algebra::{Coefficient, CoefficientPolynomial, IndexedCoefficientContext};
use rustred::solver::{
    AffineCase, AffineIntersection, Case, CoordinateCase, ExceptionalConditions, Integral, Power,
    RuleCandidate, SearchStats, SectorRule, SectorSolution, SectorStats, Seed, SeedSource, Term,
};
use symbolica::prelude::AtomCore;

use crate::application::{AppError, MAX_INPUT_BYTES};

use super::model::*;

pub(super) fn read(bytes: &[u8], limits: CandidateBundleLimits) -> Result<Bundle, AppError> {
    if bytes.len() > limits.bundle_byte_limit() {
        return Err(AppError::limit("candidate bundle exceeds its byte limit"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|error| AppError::input(format!("candidate bundle is not UTF-8: {error}")))?;
    let bundle: Bundle = toml::from_str(text)
        .map_err(|error| AppError::schema(format!("invalid candidate bundle: {error}")))?;
    validate(&bundle, limits)?;
    Ok(bundle)
}

pub(super) fn write(bundle: &Bundle, limits: CandidateBundleLimits) -> Result<Vec<u8>, AppError> {
    validate(bundle, limits)?;
    let output =
        toml::to_string(bundle).map_err(|error| AppError::serialization(error.to_string()))?;
    if output.len() > limits.bundle_byte_limit() {
        return Err(AppError::output_limit(
            "candidate bundle exceeds its byte limit",
        ));
    }
    Ok(output.into_bytes())
}

fn validate(bundle: &Bundle, limits: CandidateBundleLimits) -> Result<(), AppError> {
    if bundle.schema != CANDIDATE_BUNDLE_SCHEMA
        || bundle.status != STATUS
        || bundle.solver_policy != SOLVER_POLICY
    {
        return Err(AppError::schema(
            "unsupported candidate bundle schema/status/solver policy",
        ));
    }
    if bundle.family_source.len() > MAX_INPUT_BYTES {
        return Err(AppError::limit(
            "candidate family input exceeds its byte limit",
        ));
    }
    let n = bundle.root_sector.len();
    if !(1..=16).contains(&n) {
        return Err(AppError::input("candidate root arity must be 1 through 16"));
    }
    super::preparation::validate_permutation(n, bundle.permutation.as_deref())?;
    let mut budget = Ingress {
        entries: 0,
        coefficient_bytes: 0,
        limits,
    };
    budget.entries(bundle.sectors.len())?;
    let mut seen = BTreeSet::new();
    for sector in &bundle.sectors {
        if sector.sector.len() != n
            || !seen.insert(&sector.sector)
            || sector
                .sector
                .iter()
                .zip(&bundle.root_sector)
                .any(|(&active, &root)| active && !root)
        {
            return Err(AppError::input(
                "candidate sector is duplicate, wrong-arity, or outside root",
            ));
        }
        budget.entries(sector.rules.len())?;
        budget.entries(sector.finite_residuals.len())?;
        for integral in &sector.finite_residuals {
            validate_integral(integral, n)?;
            if integral.symbolic.iter().any(|&value| value)
                || integral
                    .values
                    .iter()
                    .zip(&sector.sector)
                    .any(|(&v, &active)| (v > 0) != active)
            {
                return Err(AppError::input(
                    "candidate residual is not a concrete key in its sector",
                ));
            }
        }
        for rule in &sector.rules {
            validate_integral(&rule.target, n)?;
            let case = &rule.case;
            if case.fixed_axes.len() != case.fixed_values.len()
                || case.fixed_axes.iter().any(|&axis| axis >= n)
                || case.fixed_axes.windows(2).any(|axes| axes[0] >= axes[1])
                || !matches!(case.kind.as_str(), "coordinate" | "affine")
                || (case.kind == "coordinate") != case.equations.is_empty()
            {
                return Err(AppError::input("invalid candidate case shape"));
            }
            budget.entries(case.equations.len())?;
            for equation in &case.equations {
                budget.coefficient(equation)?;
            }
            budget.entries(rule.rhs.len())?;
            budget.entries(rule.sources.len())?;
            budget.entries(rule.exclusions.len())?;
            for term in &rule.rhs {
                validate_integral(&term.integral, n)?;
                if term.integral.symbolic != rule.target.symbolic {
                    return Err(AppError::input(
                        "candidate RHS symbolic layout differs from target",
                    ));
                }
                budget.coefficient(&term.coefficient)?;
            }
            for source in &rule.sources {
                validate_integral(&source.integral, n)?;
                if source.shifts.len() != n {
                    return Err(AppError::input(
                        "candidate seed shift arity differs from root",
                    ));
                }
            }
            for branch in &rule.exclusions {
                budget.entries(branch.len())?;
                for equation in branch {
                    budget.coefficient(equation)?;
                }
            }
        }
    }
    Ok(())
}

struct Ingress {
    entries: usize,
    coefficient_bytes: usize,
    limits: CandidateBundleLimits,
}

impl Ingress {
    fn entries(&mut self, count: usize) -> Result<(), AppError> {
        self.entries = self
            .entries
            .checked_add(count)
            .ok_or_else(|| AppError::limit("candidate collection count overflow"))?;
        if self.entries > self.limits.max_collection_entries {
            return Err(AppError::limit(
                "candidate aggregate collection-entry budget exceeded",
            ));
        }
        Ok(())
    }
    fn coefficient(&mut self, expression: &str) -> Result<(), AppError> {
        self.coefficient_bytes = self
            .coefficient_bytes
            .checked_add(expression.len())
            .ok_or_else(|| AppError::limit("candidate coefficient-byte count overflow"))?;
        if expression.len() > self.limits.max_coefficient_bytes
            || self.coefficient_bytes > self.limits.max_total_coefficient_bytes
        {
            return Err(AppError::limit(
                "candidate coefficient-byte budget exceeded",
            ));
        }
        Ok(())
    }
}

fn validate_integral(value: &IntegralRecord, n: usize) -> Result<(), AppError> {
    if value.symbolic.len() != n || value.values.len() != n {
        return Err(AppError::input("candidate integral has the wrong arity"));
    }
    for (&symbolic, &value) in value.symbolic.iter().zip(&value.values) {
        Power::new(symbolic, value).map_err(|error| AppError::input(error.to_string()))?;
    }
    Ok(())
}

fn integral_record<const N: usize>(value: &Integral<N>) -> IntegralRecord {
    IntegralRecord {
        symbolic: value.powers().iter().map(|p| p.is_symbolic()).collect(),
        values: value.powers().iter().map(|p| p.value()).collect(),
    }
}

fn integral<const N: usize>(value: &IntegralRecord) -> Result<Integral<N>, AppError> {
    validate_integral(value, N)?;
    let mut powers = [Power::default(); N];
    for (axis, power) in powers.iter_mut().enumerate() {
        *power = Power::new(value.symbolic[axis], value.values[axis])
            .map_err(|error| AppError::input(error.to_string()))?;
    }
    Ok(Integral::new(powers))
}

pub(super) fn sector_record<const N: usize>(
    sector: [bool; N],
    solution: &SectorSolution<N>,
) -> SectorRecord {
    SectorRecord {
        sector: sector.to_vec(),
        finite_residuals: solution
            .finite_residuals
            .iter()
            .map(integral_record)
            .collect(),
        rules: solution
            .rules
            .iter()
            .map(|rule| {
                let candidate = &rule.candidate;
                let fixed: Vec<_> = candidate
                    .case
                    .fixed()
                    .iter()
                    .enumerate()
                    .filter_map(|(axis, value)| value.map(|v| (axis, v)))
                    .collect();
                RuleRecord {
                    case: CaseRecord {
                        kind: if candidate.case.affine().is_some() {
                            "affine"
                        } else {
                            "coordinate"
                        }
                        .into(),
                        fixed_axes: fixed.iter().map(|(axis, _)| *axis).collect(),
                        fixed_values: fixed.iter().map(|(_, value)| *value).collect(),
                        equations: candidate
                            .case
                            .affine()
                            .map(|case| {
                                case.equations()
                                    .iter()
                                    .map(|p| p.to_expression().to_canonical_string())
                                    .collect()
                            })
                            .unwrap_or_default(),
                    },
                    target: integral_record(&candidate.target),
                    rhs: candidate
                        .rhs
                        .iter()
                        .map(|term| TermRecord {
                            integral: integral_record(&term.integral),
                            coefficient: term.coefficient.to_expression().to_canonical_string(),
                        })
                        .collect(),
                    sources: candidate
                        .sources
                        .iter()
                        .map(|source| SeedRecord {
                            basis_row: source.basis_row,
                            integral: integral_record(&source.seed.integral),
                            shifts: source.seed.shifts.to_vec(),
                        })
                        .collect(),
                    exclusions: rule
                        .exceptions
                        .branches
                        .iter()
                        .map(|branch| {
                            branch
                                .iter()
                                .map(|p| p.to_expression().to_canonical_string())
                                .collect()
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

/// Rebuild ordinary transport values only. This performs no source replay and
/// never constructs a ClosedArtifact, replay certificate, or trusted matrix.
pub(super) fn solutions<const N: usize>(
    bundle: &Bundle,
    context: &IndexedCoefficientContext,
    indices: &[usize; N],
    limits: CandidateBundleLimits,
) -> Result<Vec<([bool; N], SectorSolution<N>)>, AppError> {
    validate(bundle, limits)?;
    if bundle.root_sector.len() != N {
        return Err(AppError::input("candidate reconstruction arity"));
    }
    bundle.sectors.iter().map(|record| {
        let sector: [bool; N] = record.sector.as_slice().try_into().expect("validated arity");
        let rules = record.rules.iter().map(|record| {
            let mut fixed = [None; N];
            for (&axis, &value) in record.case.fixed_axes.iter().zip(&record.case.fixed_values) {
                fixed[axis] = Some(value);
            }
            let face = CoordinateCase::new(fixed).map_err(|e| AppError::input(e.to_string()))?;
            if !face.is_in_sector(&sector) { return Err(AppError::input("candidate fixed face is outside sector")); }
            let case: Case<N> = if record.case.kind == "coordinate" { face.into() } else {
                let equations = record.case.equations.iter()
                    .map(|s| parse_polynomial(context, s, limits)).collect::<Result<Vec<_>, _>>()?;
                match AffineCase::from_coordinate(&face, &equations, indices, &sector)
                    .map_err(|e| AppError::input(e.to_string()))?
                {
                    AffineIntersection::Affine(case) if case.face().fixed() == &fixed => case.into(),
                    _ => return Err(AppError::input("saved affine case does not reconstruct its declared fixed face")),
                }
            };
            let target = integral(&record.target)?;
            if target != case.integral() { return Err(AppError::input("candidate target is not canonical for its case")); }
            let rhs = record.rhs.iter().map(|term| Ok(Term {
                integral: integral(&term.integral)?, coefficient: parse_coefficient(context, &term.coefficient, limits)?,
            })).collect::<Result<Vec<_>, AppError>>()?;
            let sources = record.sources.iter().map(|source| Ok(SeedSource {
                basis_row: source.basis_row,
                seed: Seed {
                    integral: integral(&source.integral)?,
                    shifts: source.shifts.as_slice().try_into().expect("validated seed arity"),
                },
            })).collect::<Result<Vec<_>, AppError>>()?;
            let branches = record.exclusions.iter().map(|branch| branch.iter()
                .map(|s| parse_polynomial(context, s, limits)).collect())
                .collect::<Result<Vec<Vec<_>>, _>>()?;
            Ok(SectorRule {
                candidate: RuleCandidate { case, target, rhs, sources, stats: SearchStats::default() },
                exceptions: ExceptionalConditions { branches },
            })
        }).collect::<Result<Vec<_>, AppError>>()?;
        let finite_residuals = record.finite_residuals.iter().map(integral).collect::<Result<Vec<_>, _>>()?;
        Ok((sector, SectorSolution { rules, finite_residuals, stats: SectorStats::default() }))
    }).collect()
}

fn parse_coefficient(
    context: &IndexedCoefficientContext,
    source: &str,
    limits: CandidateBundleLimits,
) -> Result<Coefficient, AppError> {
    catch_unwind(AssertUnwindSafe(|| {
        context.parse_expression_with_limits(source, limits.exact_algebra)
    }))
    .map_err(|_| AppError::input("native candidate coefficient parsing panicked"))?
    .map(|value| value.raw().clone())
    .map_err(|error| AppError::input(error.to_string()))
}

fn parse_polynomial(
    context: &IndexedCoefficientContext,
    source: &str,
    limits: CandidateBundleLimits,
) -> Result<CoefficientPolynomial, AppError> {
    let value = parse_coefficient(context, source, limits)?;
    if !value.denominator.is_one() {
        return Err(AppError::input("candidate equation is not a polynomial"));
    }
    Ok(value.numerator)
}
