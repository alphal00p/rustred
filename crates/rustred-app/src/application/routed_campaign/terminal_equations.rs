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

/// Failure to apply a supplied exact identity through an authenticated saved
/// candidate program. Failure never promotes an unresolved integral to master.
#[derive(Debug)]
pub enum TerminalEquationReductionFailure {
    /// One of the finitely supplied source keys failed coordinate admission.
    FiniteAdmission(rustred::solver::RootAdmissionError),
    /// Saved-rule, routing, arithmetic or operational application failure.
    SavedApplication(rustred::solver::CandidateRoutedError),
}

impl TerminalEquationReductionFailure {
    pub fn kind(&self) -> &'static str {
        use rustred::solver::CandidateRoutedError;
        match self {
            Self::FiniteAdmission(_) => "invalid-finite-source-admission",
            Self::SavedApplication(CandidateRoutedError::MissingOwner { .. }) => "missing-owner",
            Self::SavedApplication(CandidateRoutedError::UnsupportedSupportTransition {
                ..
            }) => "unsupported-support-transition",
            Self::SavedApplication(CandidateRoutedError::Cycle { .. }) => "saved-application-cycle",
            Self::SavedApplication(CandidateRoutedError::ResourceLimit { .. }) => {
                "caller-resource-limit"
            }
            Self::SavedApplication(_) => "saved-rule-application-unavailable",
        }
    }
}

impl std::fmt::Display for TerminalEquationReductionFailure {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FiniteAdmission(error) => write!(out, "symmetry-row finite admission: {error}"),
            Self::SavedApplication(error) => {
                write!(
                    out,
                    "saved-rule application of complete numerator-symmetry row: {error}"
                )
            }
        }
    }
}

impl std::error::Error for TerminalEquationReductionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FiniteAdmission(error) => Some(error),
            Self::SavedApplication(error) => Some(error),
        }
    }
}

pub(in crate::application) struct PreparedTerminalEquations {
    pub provider: TerminalEquationProvider,
    pub reduce_equation: TerminalEquationReducer,
}

/// Generation-free exact application of saved candidate equations.
///
/// Preparation authenticates the family, every payload digest, saved overlays,
/// routes and contexts exactly as the native finite-refinement provider does.
/// The immutable programs and the coefficient application cache are retained
/// between calls. This object does not certify the source rules or claim
/// closure, independence or coverage of a user-supplied starting domain.
pub struct PreparedSavedIntegralEquations {
    prepared: PreparedTerminalEquations,
}

impl PreparedSavedIntegralEquations {
    /// Apply the complete homogeneous identity and coalesce its exact terminal
    /// coefficients. Source guards are preserved and application guards added.
    ///
    /// Root admission consists of the finite exact keys in this identity, not
    /// the saved campaign's requested starting scope. This is appropriate for
    /// independently established symmetry/source identities; it must not be
    /// advertised as admitting arbitrary physics inputs. Descendants retain
    /// the existing rule, routing and descent checks and are never rank-clipped.
    pub fn reduce_equation(
        &mut self,
        equation: &TerminalEquation,
    ) -> Result<TerminalEquation, TerminalEquationReductionFailure> {
        (self.prepared.reduce_equation)(equation)
    }

    /// Return directly applicable saved equations and finite verified routing
    /// identities at one key. This does not recursively apply their RHS.
    pub fn equations(
        &mut self,
        key: &IntegralKey,
    ) -> Result<Vec<TerminalEquation>, TerminalRelationError> {
        (self.prepared.provider)(key)
    }
}

/// Prepare a caller-owned identity application object without generating rules
/// or reading/modifying a campaign traversal ledger. Resource grants come from
/// `request`; payload paths resolve against its `owner_base`.
pub fn prepare_saved_integral_equations(
    request: &RoutedCampaignRequest,
    family_fingerprint: &str,
    expected_digests: &[String],
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<PreparedSavedIntegralEquations, AppError> {
    prepare_terminal_equations(
        request,
        family_fingerprint,
        expected_digests,
        cancel,
        observer,
    )
    .map(|prepared| PreparedSavedIntegralEquations { prepared })
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

#[cfg(test)]
mod public_facade_tests {
    use super::*;

    #[test]
    fn crate_root_facade_preserves_input_admission_before_any_native_preparation() {
        let request = crate::RoutedCampaignRequest::new("{}".into(), String::new());
        let cancel = AtomicBool::new(false);
        let observed = std::sync::atomic::AtomicUsize::new(0);
        let result: Result<crate::PreparedSavedIntegralEquations, AppError> =
            crate::prepare_saved_integral_equations(
                &request,
                "missing-family",
                &[],
                &cancel,
                &|_| {
                    observed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                },
            );
        assert!(result.is_err());
        assert!(!cancel.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(observed.load(std::sync::atomic::Ordering::Relaxed), 0);
    }
}
