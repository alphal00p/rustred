//! Cold preparation shared with campaign tracing, without starting a walk.
use super::{RoutedCampaignRequest, input::Selection, prepare};
use crate::AppError;
use rustred::family::IntegralKey;
use rustred::reduction::terminal_relations::{TerminalEquation, TerminalRelationError};
use serde_json::Value;
use std::sync::atomic::AtomicBool;

pub(in crate::application) type TerminalEquationProvider =
    Box<dyn FnMut(&IntegralKey) -> Result<Vec<TerminalEquation>, TerminalRelationError>>;

pub(in crate::application) type TerminalEquationReducer =
    Box<dyn FnMut(&TerminalEquation) -> Result<TerminalEquation, TerminalEquationReductionFailure>>;

#[derive(Debug)]
pub(in crate::application) enum TerminalEquationReductionFailure {
    FiniteAdmission(rustred::solver::RootAdmissionError),
    SavedApplication(rustred::solver::CandidateRoutedError),
}

impl TerminalEquationReductionFailure {
    pub fn kind(&self) -> &'static str {
        use rustred::solver::CandidateRoutedError;
        match self {
            Self::FiniteAdmission(_) => "invalid-finite-source-admission",
            Self::SavedApplication(CandidateRoutedError::MissingOwner { .. }) => "missing-owner",
            Self::SavedApplication(CandidateRoutedError::UnsupportedSupportTransition { .. }) => {
                "unsupported-support-transition"
            }
            Self::SavedApplication(CandidateRoutedError::Cycle { .. }) => "saved-application-cycle",
            Self::SavedApplication(CandidateRoutedError::ResourceLimit { .. }) => "caller-resource-limit",
            Self::SavedApplication(_) => "saved-rule-application-unavailable",
        }
    }
}

impl std::fmt::Display for TerminalEquationReductionFailure {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FiniteAdmission(error) => write!(out, "symmetry-row finite admission: {error}"),
            Self::SavedApplication(error) => {
                write!(out, "saved-rule application of complete numerator-symmetry row: {error}")
            }
        }
    }
}

pub(in crate::application) struct PreparedTerminalEquations {
    pub provider: TerminalEquationProvider,
    pub reduce_equation: TerminalEquationReducer,
}

pub(in crate::application) fn prepare_terminal_equations(
    request: &RoutedCampaignRequest,
    family_fingerprint: &str,
    expected_digests: &[String],
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<PreparedTerminalEquations, AppError> {
    let (selection, n, limits) = Selection::parse(&request.selection_json)?;
    macro_rules! dispatch { ($($n:literal),*) => { match rustred::campaign_storage_arity(n) {
        $($n => prepare_provider::<$n>(request, &selection, limits,
            family_fingerprint, expected_digests, cancel, observer),)*
        _ => Err(AppError::input("terminal equation arity is not compiled")),
    }} }
    crate::ensure_runtime_arity(n)?;
    rustred::with_app_runtime_arities!(dispatch)
}

fn prepare_provider<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    limits: crate::CandidateOwnerLoadLimits,
    family_fingerprint: &str,
    expected_digests: &[String],
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<PreparedTerminalEquations, AppError> {
    let mut verify = |actual: Vec<String>| {
        if actual != expected_digests {
            Err("saved terminal-equation payloads differ from publication".to_string())
        } else {
            Ok(())
        }
    };
    let reducer = prepare::prepare_with_fingerprints::<N>(
        request,
        selection,
        limits,
        cancel,
        observer,
        Some(&mut verify),
    )?
    .ok_or_else(|| AppError::execution("paused while preparing saved equations"))?;
    if reducer.programs().context().family().fingerprint() != family_fingerprint {
        return Err(AppError::input(
            "saved equations and terminal family differ",
        ));
    }
    let mut application = reducer.application();
    let provider = Box::new(move |key: &IntegralKey| {
        reducer
            .terminal_identity_equations(key)
            .map(|rows| {
                rows.into_iter()
                    .map(|row| TerminalEquation {
                        terms: row.terms,
                        nonzero_conditions: row.nonzero_conditions,
                    })
                    .collect()
            })
            .map_err(|error| TerminalRelationError::Algebra(error.to_string()))
    });
    let reduce_equation = Box::new(move |equation: &TerminalEquation| {
        use rustred::solver::{CandidateEntryAdmission, FiniteRootAdmission, RootRegionInput};
        if equation.terms.is_empty() {
            return Ok(equation.clone());
        }
        // These finite roots are derived from an already exact source row, not
        // an enlargement of the user's starting scope or terminal catalogue.
        // The saved candidate rules must still satisfy all their own guards,
        // routing and strict-descent checks. Descendants are never rank-clipped.
        let inputs = equation.terms.keys().map(|key| {
            let lower: Vec<_> = (0..N)
                .map(|axis| {
                    let power = key.powers().get(axis).copied().unwrap_or(0);
                    if power > 0 {
                        power as u64 - 1
                    } else {
                        power.unsigned_abs()
                    }
                })
                .collect();
            RootRegionInput {
                support: std::array::from_fn(|axis| {
                    key.powers().get(axis).copied().unwrap_or(0) > 0
                }),
                upper: lower.iter().copied().map(Some).collect(),
                lower,
                rank: None,
                powers: Default::default(),
            }
        });
        let admission = FiniteRootAdmission::try_new(inputs, equation.terms.len())
            .map_err(TerminalEquationReductionFailure::FiniteAdmission)?;
        let reduction = application
            .reduce_linear_combination_with_entry_admission(
                equation
                    .terms
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
                CandidateEntryAdmission::ExplicitFinite(&admission),
            )
            .map_err(TerminalEquationReductionFailure::SavedApplication)?;
        let mut conditions = equation.nonzero_conditions.clone();
        for condition in reduction.nonzero_conditions() {
            if !conditions.contains(condition) {
                conditions.push(condition.clone());
            }
        }
        Ok(TerminalEquation {
            terms: reduction.terms().clone(),
            nonzero_conditions: conditions,
        })
    });
    Ok(PreparedTerminalEquations {
        provider,
        reduce_equation,
    })
}
