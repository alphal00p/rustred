//! Versioned application resource recipes for reproducible normalization.
//! These are explicit snapshots, never aliases for changing core defaults.
use super::*;
use rustred::algebra::{ExactAlgebraLimits, IndexedAlgebraLimits};
use rustred::family::symanzik::FeynmanPolynomialLimits;
use rustred::reduction::terminal_normalization::{
    TerminalNormalizationLimits, VacuumParametricLimits,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MasterNormalizationProfile {
    #[default]
    ConservativeV1,
    StandardV1,
    /// Trusted local preparation delegates operational counts to its live RAM
    /// guard. Native exponent and matrix representation bounds still apply.
    RamControlledV1,
}

impl MasterNormalizationProfile {
    pub fn name(self) -> &'static str {
        match self {
            Self::ConservativeV1 => "conservative-v1",
            Self::StandardV1 => "standard-v1",
            Self::RamControlledV1 => "ram-controlled-v1",
        }
    }

    pub(super) fn normalization_limits(self) -> TerminalNormalizationLimits {
        let standard = self == Self::StandardV1;
        let mut limits = TerminalNormalizationLimits {
            diagnostics_enabled: false,
            parametric: VacuumParametricLimits {
                diagnostics_enabled: false,
                symanzik: FeynmanPolynomialLimits {
                    diagnostics_enabled: false,
                    exact_algebra: ExactAlgebraLimits {
                        diagnostics_enabled: false,
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
        };
        if self == Self::RamControlledV1 {
            limits.max_terminals = usize::MAX;
            limits.max_supports = usize::MAX;
            limits.max_matrix_cells = usize::MAX;
            limits.max_output_terms = usize::MAX;
            let p = &mut limits.parametric;
            p.max_supports = usize::MAX;
            p.max_canonicalizations = usize::MAX;
            p.max_graph_vertices = usize::MAX;
            p.max_graph_edges = usize::MAX;
            let s = &mut p.symanzik;
            s.exact_algebra = ExactAlgebraLimits::unbounded_resources();
            s.max_parameters = usize::MAX;
            s.max_polynomial_terms = usize::MAX;
            s.max_exponent_entries = usize::MAX;
            s.max_term_operations = usize::MAX;
            s.max_determinant_matrix_entries = usize::MAX;
            s.max_determinant_ring_operations = usize::MAX;
            s.max_adjugate_minors = usize::MAX;
        }
        limits
    }

    pub(super) fn relation_limits(self) -> TerminalRelationLimits {
        TerminalRelationLimits {
            diagnostics_enabled: false,
            // Campaigns have a live RAM guard. A preset column count is not a
            // memory budget and used to stop otherwise healthy refinements.
            // Callers can impose an explicit resumable operational cap.
            max_columns: usize::MAX,
            max_seeds: usize::MAX,
            max_rows: usize::MAX,
            max_nonzeros: usize::MAX,
            normalization: self.normalization_limits(),
            algebra: IndexedAlgebraLimits::unbounded_resources(),
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

/// Native checkpoints are trusted generated state governed by the caller's
/// process RAM policy. No byte/count/arithmetic quota is inferred from saved
/// metadata. An explicit native grant is preserved verbatim, including tighter
/// admission limits; representation bounds remain enforced by native codecs.
pub(super) fn native_io_limits(explicit: Option<BinaryIoLimits>) -> BinaryIoLimits {
    explicit.unwrap_or(BinaryIoLimits {
        diagnostics_enabled: false,
        max_program_bytes: usize::MAX,
        max_sections: usize::MAX,
        max_collection_entries: usize::MAX,
        max_state_bytes: usize::MAX,
        max_atom_bytes: usize::MAX,
        max_total_atom_bytes: usize::MAX,
        exact_algebra: ExactAlgebraLimits::unbounded_resources(),
    })
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
        Some("ram-controlled-v1") => MasterNormalizationProfile::RamControlledV1,
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
