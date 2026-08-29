use serde::Serialize;

use crate::application::producer::ProducerOutputV1;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct LifecycleOutputV1 {
    pub(super) ownership: &'static str,
    pub(super) durable: bool,
    pub(super) persistence: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ArtifactPayloadOutputV1 {
    pub(super) encoding: &'static str,
    pub(super) bytes: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ArtifactSummaryOutputV1 {
    pub(super) schema: &'static str,
    pub(super) schema_version: u32,
    pub(super) algorithm_id: &'static str,
    pub(super) arity: usize,
    pub(super) family_fingerprint: String,
    pub(super) coefficient_context_fingerprint: String,
    pub(super) common_mass_homogeneity: Option<&'static str>,
    pub(super) masters: Vec<IntegralKeyOutputV1>,
    pub(super) zero_terminals: Vec<ZeroTerminalOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ValidationOutputV1 {
    pub(super) source_rows: usize,
    pub(super) replayed_source_rows: usize,
    pub(super) replayed_shift_columns: usize,
    pub(super) guarded_rules: usize,
    pub(super) universally_applicable_guards: usize,
    pub(super) master_terminals: usize,
    pub(super) zero_sector_terminals: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct IntegralKeyOutputV1 {
    pub(super) powers: Vec<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ZeroTerminalOutputV1 {
    pub(super) sector: String,
    pub(super) proof: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct SourceRelationOutputV1 {
    pub(super) ordinal: usize,
    pub(super) stable_id: String,
    pub(super) terms: Vec<RelationTermOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct RelationTermOutputV1 {
    pub(super) shift: Vec<i64>,
    pub(super) coefficient: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ClosingRuleOutputV1 {
    pub(super) ordinal: usize,
    pub(super) sector: String,
    pub(super) domain_lower: Vec<i64>,
    pub(super) domain_upper: Vec<i64>,
    pub(super) pivot: Vec<i64>,
    pub(super) nonzero_guards: Vec<String>,
    pub(super) right_hand_side: Vec<RuleTermOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct RuleTermOutputV1 {
    pub(super) shift: Vec<i64>,
    pub(super) coefficient: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct GenerateOutputV1 {
    pub(super) schema: &'static str,
    pub(super) status: &'static str,
    pub(super) producer: ProducerOutputV1,
    pub(super) family_selector: &'static str,
    pub(super) lifecycle: LifecycleOutputV1,
    pub(super) payload: ArtifactPayloadOutputV1,
    pub(super) artifact: ArtifactSummaryOutputV1,
    pub(super) validation: ValidationOutputV1,
    pub(super) source_relations: Vec<SourceRelationOutputV1>,
    pub(super) rules: Vec<ClosingRuleOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct InspectOutputV1 {
    pub(super) schema: &'static str,
    pub(super) status: &'static str,
    pub(super) producer: ProducerOutputV1,
    pub(super) artifact_source: &'static str,
    pub(super) materialization: &'static str,
    pub(super) lifecycle: LifecycleOutputV1,
    pub(super) artifact: ArtifactSummaryOutputV1,
    pub(super) validation: ValidationOutputV1,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ReduceOutputV1 {
    pub(super) schema: &'static str,
    pub(super) status: &'static str,
    pub(super) producer: ProducerOutputV1,
    pub(super) artifact_source: &'static str,
    pub(super) materialization: &'static str,
    pub(super) family_fingerprint: String,
    pub(super) target: IntegralKeyOutputV1,
    pub(super) common_mass_squared_symbol: &'static str,
    pub(super) statistics: ReductionStatisticsOutputV1,
    pub(super) terms: Vec<ReductionTermOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ReductionStatisticsOutputV1 {
    pub(super) cache_hits: usize,
    pub(super) rule_applications: usize,
    pub(super) cached_integrals: usize,
    pub(super) cached_coefficient_terms: usize,
    pub(super) cached_coefficient_bytes: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ReductionTermOutputV1 {
    pub(super) master: IntegralKeyOutputV1,
    pub(super) unit_mass_coefficient: String,
    /// Canonical decimal string because TOML integers are signed 64-bit while
    /// the generic homogeneity proof uses an exact signed 128-bit exponent.
    pub(super) common_mass_squared_power: String,
    pub(super) common_mass_squared_factor: String,
}
