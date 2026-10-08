//! Versioned application resource recipes for reproducible normalization.
//! These are explicit snapshots, never aliases for changing core defaults.
use super::*;
use rustred::algebra::ExactAlgebraLimits;
use rustred::family::symanzik::FeynmanPolynomialLimits;
use rustred::reduction::terminal_normalization::{
    TerminalNormalizationLimits, VacuumParametricLimits,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MasterNormalizationProfile {
    #[default]
    ConservativeV1,
    StandardV1,
}

impl MasterNormalizationProfile {
    pub fn name(self) -> &'static str {
        match self {
            Self::ConservativeV1 => "conservative-v1",
            Self::StandardV1 => "standard-v1",
        }
    }

    pub(super) fn normalization_limits(self) -> TerminalNormalizationLimits {
        let standard = self == Self::StandardV1;
        TerminalNormalizationLimits {
            parametric: VacuumParametricLimits {
                symanzik: FeynmanPolynomialLimits {
                    exact_algebra: ExactAlgebraLimits {
                        max_exponent: 65_535,
                        max_polynomial_terms: 4_000_000,
                        max_term_operations: 16_000_000,
                    },
                    max_parameters: 4_096,
                    max_parameter_exponent: 65_535,
                    max_polynomial_terms: if standard { 4_000_000 } else { 20_000 },
                    max_exponent_entries: if standard { 64_000_000 } else { 1_000_000 },
                    max_term_operations: if standard { 16_000_000 } else { 2_000_000 },
                    max_determinant_matrix_entries: 1_048_576,
                    max_determinant_ring_operations: if standard { 16_000_000 } else { 1_000_000 },
                    max_adjugate_minors: 1_048_576,
                },
                max_supports: 4_096,
                max_canonicalizations: 16_384,
                max_graph_vertices: 4_096,
                max_graph_edges: 65_536,
            },
            max_terminals: 1_000_000,
            max_supports: 100_000,
            max_matrix_cells: 1_000_000,
            max_output_terms: 4_000_000,
        }
    }

    pub(super) fn relation_limits(self) -> TerminalRelationLimits {
        TerminalRelationLimits {
            normalization: self.normalization_limits(),
            ..Default::default()
        }
    }

    pub(super) fn hash(self, hash: &mut blake3::Hasher) {
        if self != Self::ConservativeV1 {
            hash.update(b"terminal-normalization-profile-v1");
            hash.update(self.name().as_bytes());
        }
    }

    fn snapshot(self) -> Value {
        let limits = self.normalization_limits();
        let p = limits.parametric;
        let s = p.symanzik;
        json!({
            "max_terminals":limits.max_terminals,"max_supports":limits.max_supports,
            "max_matrix_cells":limits.max_matrix_cells,"max_output_terms":limits.max_output_terms,
            "parametric":{
                "max_supports":p.max_supports,"max_canonicalizations":p.max_canonicalizations,
                "max_graph_vertices":p.max_graph_vertices,"max_graph_edges":p.max_graph_edges,
                "symanzik":{
                    "exact_algebra":{"max_exponent":s.exact_algebra.max_exponent,
                        "max_polynomial_terms":s.exact_algebra.max_polynomial_terms,
                        "max_term_operations":s.exact_algebra.max_term_operations},
                    "max_parameters":s.max_parameters,"max_parameter_exponent":s.max_parameter_exponent,
                    "max_polynomial_terms":s.max_polynomial_terms,"max_exponent_entries":s.max_exponent_entries,
                    "max_term_operations":s.max_term_operations,
                    "max_determinant_matrix_entries":s.max_determinant_matrix_entries,
                    "max_determinant_ring_operations":s.max_determinant_ring_operations,
                    "max_adjugate_minors":s.max_adjugate_minors,
                },
            },
        })
    }
}

pub(super) fn from_report(report: &Value) -> Result<MasterNormalizationProfile, AppError> {
    let Some(name) = report.get("normalization_profile") else {
        if report.get("normalization_limits").is_some() {
            return Err(AppError::input(
                "normalization limits have no versioned profile",
            ));
        }
        return Ok(MasterNormalizationProfile::ConservativeV1);
    };
    let profile = match name.as_str() {
        Some("conservative-v1") => MasterNormalizationProfile::ConservativeV1,
        Some("standard-v1") => MasterNormalizationProfile::StandardV1,
        _ => return Err(AppError::input("unknown normalization profile")),
    };
    if report["normalization_limits"] != profile.snapshot() {
        return Err(AppError::input(
            "normalization limits differ from the versioned profile",
        ));
    }
    Ok(profile)
}

pub(super) fn for_resume(
    requested: Option<MasterNormalizationProfile>,
    report: &Value,
) -> Result<MasterNormalizationProfile, AppError> {
    let profile = from_report(report)?;
    if requested.is_some_and(|requested| requested != profile) {
        return Err(AppError::input(
            "cannot change normalization profile on resume; start a new phase directory",
        ));
    }
    Ok(profile)
}

pub(super) fn for_import(
    requested: Option<MasterNormalizationProfile>,
    inherited: MasterNormalizationProfile,
    operation: MasterReductionOperation,
) -> Result<MasterNormalizationProfile, AppError> {
    let selected = requested.unwrap_or(inherited);
    if operation == MasterReductionOperation::Publish && selected != inherited {
        return Err(AppError::input(
            "publication must inherit its imported normalization profile",
        ));
    }
    Ok(selected)
}

pub(super) fn record(report: &mut Value, profile: MasterNormalizationProfile) {
    report["normalization_profile"] = json!(profile.name());
    report["normalization_limits"] = profile.snapshot();
}
