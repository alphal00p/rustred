//! Optional fresh original-source request, never a mutation of a sealed proof.
//!
//! Elimination/weight/replay conditions remain in diagnostics. Only after the
//! unchanged complete composition has succeeded may an ordinary identity frame
//! ask the native checker to regenerate final source/weight/RHS conditions with
//! their true origins. Every original caller/source assumption stays retained,
//! even when its row is unused. No origin-string filtering is used.
use super::*;
use rustred::algebra::IndexedCoefficientContext;

pub fn enabled(r: &Value) -> Result<bool> {
    r.get("fresh_original_source_certificate")
        .map_or(Ok(false), |value| {
            value
                .as_bool()
                .ok_or_else(|| "fresh_original_source_certificate must be boolean".into())
        })
}

/// `request` must be the newly assembled result of the normal `Span::compose`
/// and full-original-product equality, not an existing checked/sealed object.
pub fn fresh(
    c: &IndexedCoefficientContext,
    span: &source::Span,
    request: OriginalSourceCombinationRequest,
    limits: project::Limits,
) -> project::Result<OriginalSourceCombinationRequest> {
    span.validate_fresh_ordinary(c, limits)?;
    // Copy only the original assumption inventory. Native source checking then
    // replays these canonical final contributions and obtains all real weight
    // and RHS denominator origins itself. The default request is untouched.
    Ok(OriginalSourceCombinationRequest {
        retained_conditions: span.guards.iter().map(|g| g.polynomial.clone()).collect(),
        ..request
    })
}

#[cfg(test)]
#[path = "certificate/tests.rs"]
mod tests;
