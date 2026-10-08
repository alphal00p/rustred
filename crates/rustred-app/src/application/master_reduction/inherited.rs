//! Frozen authority for already-imported rows; never a provider configuration.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InheritedEquationAuthority {
    provider_binding: String,
    saved_rule_assistance: bool,
    circuit_symmetry_assistance: bool,
    normalization_profile: String,
    source_manifest_blake3: String,
}

fn record(report: &Value) -> Result<InheritedEquationAuthority, AppError> {
    serde_json::from_value(report["inherited_equation_authority"].clone())
        .map_err(|_| AppError::input("missing or invalid inherited equation authority"))
}

fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}

/// Authenticate against the source's original finite inventory before extension.
/// Subsequent publications carry this frozen record, not an expanded provider.
pub(super) fn capture(
    session: &TerminalRelationSession,
    source: &Value,
    destination: &mut Value,
) -> Result<(), AppError> {
    let source_profile = profile::from_report(source)?;
    let inherited = if source.get("inherited_equation_authority").is_some() {
        validate(session, source)?;
        record(source)?
    } else {
        let mut original = MasterReductionOptions::new("unused", "unused");
        original.saved_rule_assistance = source["saved_rule_assistance"].as_bool().unwrap_or(false);
        original.circuit_symmetry_assistance = source["circuit_symmetry_assistance"]
            .as_bool()
            .unwrap_or(false);
        let binding = assistance::expected_binding(&original, session, source)?;
        if session.assistance_binding() != binding.as_deref() {
            return Err(AppError::input(
                "source manifest and native equation authority differ",
            ));
        }
        let Some(provider_binding) = binding else {
            return Ok(());
        };
        InheritedEquationAuthority {
            provider_binding,
            saved_rule_assistance: original.saved_rule_assistance,
            circuit_symmetry_assistance: original.circuit_symmetry_assistance,
            normalization_profile: source_profile.name().to_owned(),
            source_manifest_blake3: blake3::hash(&serde_json::to_vec(source).map_err(io)?)
                .to_hex()
                .to_string(),
        }
    };
    destination["inherited_equation_authority"] = serde_json::to_value(inherited).map_err(io)?;
    Ok(())
}

/// Check the persisted frozen binding without querying its old finite provider.
/// Saved-rule authority stays tied to the currently packaged exact program;
/// circuit authority retains its original inventory hash, not the enlarged one.
pub(super) fn validate(session: &TerminalRelationSession, report: &Value) -> Result<(), AppError> {
    let inherited = record(report)?;
    if session.assistance_binding() != Some(inherited.provider_binding.as_str())
        || inherited.normalization_profile != profile::from_report(report)?.name()
        || !digest(&inherited.source_manifest_blake3)
        || (!inherited.saved_rule_assistance && !inherited.circuit_symmetry_assistance)
    {
        return Err(AppError::input(
            "inherited equation authority differs from native state or normalization profile",
        ));
    }
    let mut parts = inherited.provider_binding.split(';');
    if inherited.saved_rule_assistance {
        let program = report["inputs"]["program_binding"]
            .as_str()
            .ok_or_else(|| AppError::input("inherited saved equations have no program binding"))?;
        let expected = format!("saved-rule-assistance-v1:{program}");
        if parts.next() != Some(expected.as_str()) {
            return Err(AppError::input(
                "inherited saved equation program binding mismatch",
            ));
        }
    }
    if inherited.circuit_symmetry_assistance {
        if !parts
            .next()
            .and_then(|part| part.strip_prefix("circuit-symmetry-assistance-v1:"))
            .is_some_and(digest)
        {
            return Err(AppError::input(
                "invalid inherited circuit inventory binding",
            ));
        }
    }
    if parts.next().is_some() {
        return Err(AppError::input("unsupported inherited equation authority"));
    }
    Ok(())
}

pub(super) fn authority(report: &Value) -> Result<&'static str, AppError> {
    let inherited = record(report)?;
    Ok(assistance::authority(
        inherited.saved_rule_assistance,
        inherited.circuit_symmetry_assistance,
    ))
}
