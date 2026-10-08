//! Cold preparation shared with campaign tracing, without starting a walk.
use super::{RoutedCampaignRequest, input::Selection, prepare};
use crate::AppError;
use rustred::family::IntegralKey;
use rustred::reduction::terminal_relations::{TerminalEquation, TerminalRelationError};
use serde_json::Value;
use std::sync::atomic::AtomicBool;

pub(in crate::application) type TerminalEquationProvider =
    Box<dyn FnMut(&IntegralKey) -> Result<Vec<TerminalEquation>, TerminalRelationError>>;

pub(in crate::application) fn prepare_terminal_equations(
    request: &RoutedCampaignRequest,
    family_fingerprint: &str,
    expected_digests: &[String],
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<TerminalEquationProvider, AppError> {
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
) -> Result<TerminalEquationProvider, AppError> {
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
    Ok(Box::new(move |key| {
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
    }))
}
