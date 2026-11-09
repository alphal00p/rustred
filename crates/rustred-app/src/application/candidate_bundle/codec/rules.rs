//! Shared native rule transport for sector candidates and partial overlays.
//! These records carry no replay, descent, terminal or coverage authority.

use rustred::persistence::CoefficientTableBuilder;
use rustred::propagator::bundle_wire::{self, Restore};
use rustred::solver::{Case, SectorRule};
use symbolica::prelude::PolyVariable;

use super::super::model::{CaseRecord, RuleRecord, SeedRecord, TermRecord};
use super::{integral_record, intern_coefficient, intern_polynomial};
use crate::AppError;

/// Local decoder seam: both stores use the existing native Symbolica codec
/// (rustred-core's bundle restore reads either). Rule reconstruction owns its
/// output; no lazy reference escapes admission.
pub(in crate::application::candidate_bundle) use bundle_wire::CoefficientSource as NativeCoefficientSource;

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
    Ok(bundle_wire::validate_case(case, n)?)
}

pub(in crate::application::candidate_bundle) fn validate_rule(
    rule: &RuleRecord,
    n: usize,
) -> Result<(), AppError> {
    Ok(bundle_wire::validate_rule(rule, n)?)
}

pub(in crate::application::candidate_bundle) fn validate_rule_ids(
    rule: &RuleRecord,
    count: usize,
) -> Result<(), AppError> {
    Ok(bundle_wire::validate_rule_ids(rule, count)?)
}

pub(in crate::application::candidate_bundle) fn restore_case<const N: usize>(
    record: &CaseRecord,
    table: &impl NativeCoefficientSource,
    variables: &[PolyVariable],
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<Case<N>, AppError> {
    let restore = Restore {
        table,
        variables,
        indices,
    };
    Ok(restore.case(record, sector)?)
}

pub(in crate::application::candidate_bundle) fn restore_rule<const N: usize>(
    record: &RuleRecord,
    table: &impl NativeCoefficientSource,
    variables: &[PolyVariable],
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<SectorRule<N>, AppError> {
    let restore = Restore {
        table,
        variables,
        indices,
    };
    Ok(restore.rule(record, sector)?)
}
