//! Shared native rule transport for sector candidates and partial overlays.
//! These records carry no replay, descent, terminal or coverage authority.

use rustred::algebra::{Coefficient, CoefficientPolynomial};
use rustred::persistence::{CoefficientId, CoefficientTableBuilder, DecodedCoefficientTable};
use rustred::solver::{
    AffineCase, AffineIntersection, Case, CoordinateCase, ExceptionalConditions, RuleCandidate,
    SearchStats, SectorRule, Seed, SeedSource, Term,
};
use symbolica::prelude::PolyVariable;

use super::super::model::{CaseRecord, RuleRecord, SeedRecord, TermRecord};
use super::{
    binary_error, integral, integral_record, intern_coefficient, intern_polynomial,
    validate_integral,
};
use crate::AppError;

pub(in crate::application::candidate_bundle) fn case_record<const N: usize>(
    case: &Case<N>,
    table: &mut CoefficientTableBuilder,
) -> Result<CaseRecord, AppError> {
    let fixed: Vec<_> = case
        .fixed()
        .iter()
        .enumerate()
        .filter_map(|(axis, value)| value.map(|v| (axis, v)))
        .collect();
    Ok(CaseRecord {
        kind: if case.affine().is_some() {
            "affine"
        } else {
            "coordinate"
        }
        .into(),
        fixed_axes: fixed.iter().map(|(axis, _)| *axis).collect(),
        fixed_values: fixed.iter().map(|(_, value)| *value).collect(),
        equations: case
            .affine()
            .map(|case| {
                case.equations()
                    .iter()
                    .map(|p| intern_polynomial(table, p))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default(),
    })
}

pub(in crate::application::candidate_bundle) fn rule_record<const N: usize>(
    rule: &SectorRule<N>,
    table: &mut CoefficientTableBuilder,
) -> Result<RuleRecord, AppError> {
    // Legacy equation records have no dispatch field. Only the program-level
    // policy transport may deliberately attach it; never silently strip one.
    if rule.dispatch_policy != rustred::solver::RuleDispatchPolicy::Partition {
        return Err(AppError::input(
            "rule-only transport cannot preserve dispatch policy",
        ));
    }
    let candidate = &rule.candidate;
    Ok(RuleRecord {
        case: case_record(&candidate.case, table)?,
        target: integral_record(&candidate.target),
        rhs: candidate
            .rhs
            .iter()
            .map(|term| {
                Ok(TermRecord {
                    integral: integral_record(&term.integral),
                    coefficient: intern_coefficient(table, &term.coefficient)?,
                })
            })
            .collect::<Result<_, AppError>>()?,
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
                    .map(|p| intern_polynomial(table, p))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<_, AppError>>()?,
    })
}

pub(in crate::application::candidate_bundle) fn validate_case(
    case: &CaseRecord,
    n: usize,
) -> Result<(), AppError> {
    if case.fixed_axes.len() != case.fixed_values.len()
        || case.fixed_axes.iter().any(|&axis| axis >= n)
        || case.fixed_axes.windows(2).any(|axes| axes[0] >= axes[1])
        || !matches!(case.kind.as_str(), "coordinate" | "affine")
        || (case.kind == "coordinate") != case.equations.is_empty()
    {
        return Err(AppError::input("invalid candidate case shape"));
    }
    Ok(())
}

pub(in crate::application::candidate_bundle) fn validate_rule(
    rule: &RuleRecord,
    n: usize,
) -> Result<(), AppError> {
    validate_integral(&rule.target, n)?;
    validate_case(&rule.case, n)?;
    for term in &rule.rhs {
        validate_integral(&term.integral, n)?;
        if term.integral.symbolic != rule.target.symbolic {
            return Err(AppError::input(
                "candidate RHS symbolic layout differs from target",
            ));
        }
    }
    for source in &rule.sources {
        validate_integral(&source.integral, n)?;
        if source.shifts.len() != n {
            return Err(AppError::input(
                "candidate seed shift arity differs from root",
            ));
        }
    }
    Ok(())
}

pub(in crate::application::candidate_bundle) fn validate_rule_ids(
    rule: &RuleRecord,
    count: usize,
) -> Result<(), AppError> {
    if rule
        .case
        .equations
        .iter()
        .chain(rule.rhs.iter().map(|term| &term.coefficient))
        .chain(rule.exclusions.iter().flatten())
        .any(|&id| id as usize >= count)
    {
        return Err(AppError::input(
            "candidate coefficient ID is outside its table",
        ));
    }
    Ok(())
}

pub(in crate::application::candidate_bundle) fn restore_case<const N: usize>(
    record: &CaseRecord,
    table: &DecodedCoefficientTable,
    variables: &[PolyVariable],
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<Case<N>, AppError> {
    validate_case(record, N)?;
    let mut fixed = [None; N];
    for (&axis, &value) in record.fixed_axes.iter().zip(&record.fixed_values) {
        fixed[axis] = Some(value);
    }
    let face = CoordinateCase::new(fixed).map_err(|e| AppError::input(e.to_string()))?;
    if !face.is_in_sector(sector) {
        return Err(AppError::input("candidate fixed face is outside sector"));
    }
    if record.kind == "coordinate" {
        return Ok(face.into());
    }
    let equations = record
        .equations
        .iter()
        .map(|&id| polynomial(table, variables, id))
        .collect::<Result<Vec<_>, _>>()?;
    match AffineCase::from_coordinate(&face, &equations, indices, sector)
        .map_err(|e| AppError::input(e.to_string()))?
    {
        AffineIntersection::Affine(case) if case.face().fixed() == &fixed => Ok(case.into()),
        _ => Err(AppError::input(
            "saved affine case does not reconstruct its declared fixed face",
        )),
    }
}

pub(in crate::application::candidate_bundle) fn restore_rule<const N: usize>(
    record: &RuleRecord,
    table: &DecodedCoefficientTable,
    variables: &[PolyVariable],
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<SectorRule<N>, AppError> {
    validate_rule(record, record.target.values.len())?;
    if !rustred::fits_storage(record.target.values.len(), N) {
        return Err(AppError::input("candidate rule exceeds storage capacity"));
    }
    validate_rule_ids(record, table.len())?;
    let mut padded_case = record.case.clone();
    for axis in record.target.values.len()..N {
        padded_case.fixed_axes.push(axis);
        padded_case.fixed_values.push(0);
    }
    let case = restore_case(&padded_case, table, variables, indices, sector)?;
    let target = integral(&record.target)?;
    if target != case.integral() {
        return Err(AppError::input(
            "candidate target is not canonical for its case",
        ));
    }
    let rhs = record
        .rhs
        .iter()
        .map(|term| {
            Ok(Term {
                integral: integral(&term.integral)?,
                coefficient: coefficient(table, variables, term.coefficient)?.clone(),
            })
        })
        .collect::<Result<_, AppError>>()?;
    let sources = record
        .sources
        .iter()
        .map(|source| {
            Ok(SeedSource {
                basis_row: source.basis_row,
                seed: Seed {
                    integral: integral(&source.integral)?,
                    shifts: rustred::storage_array(&source.shifts, 0)
                        .expect("validated seed arity"),
                },
            })
        })
        .collect::<Result<_, AppError>>()?;
    let branches = record
        .exclusions
        .iter()
        .map(|branch| {
            branch
                .iter()
                .map(|&id| polynomial(table, variables, id))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<_, AppError>>()?;
    Ok(SectorRule {
        dispatch_policy: Default::default(),
        candidate: RuleCandidate {
            case,
            target,
            rhs,
            sources,
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions { branches },
    })
}

fn coefficient(
    table: &DecodedCoefficientTable,
    variables: &[PolyVariable],
    id: u32,
) -> Result<Coefficient, AppError> {
    let id = CoefficientId::try_from_index(id as usize).map_err(binary_error)?;
    let value = table.coefficient(id).map_err(binary_error)?;
    let physical_variables = value.get_variables().as_slice();
    if physical_variables != variables
        && (!cfg!(feature = "capacity-dispatch")
            || !variables.starts_with(physical_variables)
            || variables[physical_variables.len()..]
                .iter()
                .any(|v| !matches!(v, PolyVariable::Temporary(_))))
    {
        return Err(AppError::input(
            "candidate coefficient has the wrong indexed variable map",
        ));
    }
    if value.get_variables().as_slice() == variables {
        Ok(value.clone())
    } else {
        let numerator = value
            .numerator
            .rearrange_with_growth(variables)
            .map_err(AppError::input)?;
        let denominator = value
            .denominator
            .rearrange_with_growth(variables)
            .map_err(AppError::input)?;
        use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
        Ok(Coefficient::from_num_den(
            numerator,
            denominator,
            &symbolica::domains::integer::Z,
            true,
        ))
    }
}

fn polynomial(
    table: &DecodedCoefficientTable,
    variables: &[PolyVariable],
    id: u32,
) -> Result<CoefficientPolynomial, AppError> {
    let value = coefficient(table, variables, id)?;
    if !value.denominator.is_one() {
        return Err(AppError::input("candidate equation is not a polynomial"));
    }
    Ok(value.numerator.clone())
}
