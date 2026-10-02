use serde::{Deserialize, Serialize};

/// Read-only selection and output resources, not solver/coverage constraints.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct CandidateProgramInspectionOptions {
    /// Original-coordinate sector masks. None selects every stored sector.
    pub sectors: Option<Vec<Vec<bool>>>,
    /// Zero-based saved ordinals within each selected sector, preserving order.
    pub rule_ordinals: Option<Vec<usize>>,
    /// Include RHS numerator/denominator expressions, rather than just IDs.
    /// Affine equations and exclusion expressions are always included.
    pub include_rhs_coefficients: bool,
    /// Bounds diagnostic text and final JSON, not native-import peak RAM.
    pub max_output_bytes: usize,
}

impl Default for CandidateProgramInspectionOptions {
    fn default() -> Self {
        Self {
            sectors: None,
            rule_ordinals: None,
            include_rhs_coefficients: false,
            max_output_bytes: 16 * 1024 * 1024,
        }
    }
}

impl CandidateProgramInspectionOptions {
    pub fn from_json(text: &str) -> Result<Self, crate::AppError> {
        if text.len() > crate::MAX_INPUT_BYTES {
            return Err(crate::AppError::limit(
                "inspection options exceed input budget",
            ));
        }
        serde_json::from_str(text).map_err(|e| crate::AppError::input(e.to_string()))
    }
}

/// Diagnostic view, never a saved artifact or algebraic equality certificate.
#[derive(Clone, Debug, Serialize)]
pub struct CandidateProgramInspection {
    pub schema: &'static str,
    pub candidate_schema: String,
    pub family_fingerprint: String,
    pub arity: usize,
    pub root_sector: Vec<bool>,
    pub integral_order: String,
    pub priority_slots: Option<Vec<usize>>,
    pub coordinates: &'static str,
    pub index_symbols: Vec<String>,
    pub exclusion_semantics: &'static str,
    pub applicability_warning: &'static str,
    pub coefficient_text_authority: &'static str,
    pub source_replay_claim: bool,
    pub closure_claim: bool,
    pub total_sectors: usize,
    pub omitted_sectors: usize,
    pub total_rules: usize,
    pub omitted_rules: usize,
    pub total_terminals: usize,
    pub omitted_terminals: usize,
    pub rhs_coefficient_details_included: bool,
    pub sectors: Vec<CandidateSectorInspection>,
    /// IDs are local to this payload. Do not compare IDs between bundles.
    pub coefficients: Vec<CandidateCoefficientInspection>,
    #[serde(skip)]
    pub(super) max_output_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateSectorInspection {
    pub ordinal: usize,
    pub sector: Vec<bool>,
    pub total_rules: usize,
    pub omitted_rules: usize,
    pub rules: Vec<CandidateRuleInspection>,
    /// Exact original-coordinate integer keys, in saved order, not minimized.
    pub terminals: Vec<Vec<i16>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateRuleInspection {
    pub ordinal: usize,
    /// Absence is ordinary Partition; old diagnostic JSON stays unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch_policy: Option<&'static str>,
    pub case: CandidateCaseInspection,
    pub target: CandidateIntegralInspection,
    pub rhs: Vec<CandidateTermInspection>,
    /// Each inner list is an AND of zero equations; outer list is OR.
    pub excluded_all_zero_conjunctions: Vec<Vec<u32>>,
    pub retained_source_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateCaseInspection {
    pub kind: String,
    pub fixed: Vec<CandidateFixedAxisInspection>,
    /// Coefficient IDs for equations == 0; AND, not alternative branches.
    pub affine_zero_equations: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CandidateFixedAxisInspection {
    pub axis: usize,
    pub value: i16,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CandidateIntegralInspection {
    /// Coordinate i means n_i + values[i] when true; values[i] otherwise.
    pub symbolic: Vec<bool>,
    pub values: Vec<i16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateTermInspection {
    pub ordinal: usize,
    pub integral: CandidateIntegralInspection,
    /// Symbolic-axis displacement. None for a fixed-coordinate replacement.
    pub symbolic_shifts: Vec<Option<i32>>,
    /// New fixed values. None for symbolic axes; no uniform shift is implied.
    pub fixed_replacements: Vec<Option<i16>>,
    pub coefficient_id: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateCoefficientInspection {
    pub id: u32,
    pub variables: Vec<String>,
    pub numerator: String,
    pub denominator: String,
    /// Native polynomial term counts, not expression complexity predictions.
    pub numerator_terms: usize,
    pub denominator_terms: usize,
}
