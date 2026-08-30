mod error_mapping;
mod model;
mod render;

use rustred::family::IntegralKey;
use rustred::foundry::artifact::{
    ArtifactLoadLimits, ClosedArtifact, derive_one_loop_unit_mass_tadpole,
    derive_two_loop_unit_mass_sunset,
};
use rustred::reduction::{Reducer, ReductionLimits};
use symbolica::prelude::AtomCore;

use super::error::AppError;
use super::options::ClosingFamilySelector;
use super::producer::ProducerOutputV1;
use super::{
    ClosingArtifactGenerateRequest, ClosingArtifactGenerateResult, ClosingArtifactInspectRequest,
    ClosingArtifactInspectResult, ClosingArtifactReduceRequest, ClosingArtifactReduceResult,
    ExactMasterCoefficient,
};
use error_mapping::{map_artifact_encoding_error, map_artifact_load_error, map_reduction_error};
use model::{
    ArtifactPayloadOutputV1, ArtifactSummaryOutputV1, ClosingRuleOutputV1, GenerateOutputV1,
    InspectOutputV1, IntegralKeyOutputV1, LifecycleOutputV1, ReduceOutputV1,
    ReductionStatisticsOutputV1, ReductionTermOutputV1, RelationTermOutputV1, RuleTermOutputV1,
    SourceRelationOutputV1, ValidationOutputV1, ZeroTerminalOutputV1,
};

pub(super) const GENERATE_SCHEMA: &str = "rustred.closing-artifact-generate-output.toml.v1";
pub(super) const INSPECT_SCHEMA: &str = "rustred.closing-artifact-inspect-output.toml.v1";
pub(super) const REDUCE_SCHEMA: &str = "rustred.closing-artifact-reduce-output.toml.v1";

const GENERATED_STATUS: &str = "generated-durable";
const INSPECTED_STATUS: &str = "inspected";
const REDUCED_STATUS: &str = "reduced";
const LOADED_MATERIALIZATION: &str = "decoded-authenticated-durable-bytes";
const ARTIFACT_ENCODING: &str = "rustred.closing-artifact.binary.v1";

pub(super) fn generate_request(
    request: ClosingArtifactGenerateRequest,
) -> Result<ClosingArtifactGenerateResult, AppError> {
    let artifact = generate_family(request.family)?;
    let selector = request.family.as_str();
    let encoded = artifact
        .encode_durable()
        .map_err(map_artifact_encoding_error)?;
    let output = GenerateOutputV1 {
        schema: GENERATE_SCHEMA,
        status: GENERATED_STATUS,
        producer: ProducerOutputV1::current(),
        family_selector: selector,
        lifecycle: lifecycle(),
        payload: ArtifactPayloadOutputV1 {
            encoding: ARTIFACT_ENCODING,
            bytes: encoded.len(),
        },
        artifact: artifact_summary(&artifact),
        validation: validation_output(&artifact),
        source_relations: artifact
            .source_relations()
            .iter()
            .enumerate()
            .map(|(ordinal, relation)| SourceRelationOutputV1 {
                ordinal,
                stable_id: relation.row_id().stable_string(),
                terms: relation
                    .terms()
                    .iter()
                    .map(|(shift, coefficient)| RelationTermOutputV1 {
                        shift: shift.values().to_vec(),
                        coefficient: coefficient.to_expression().to_canonical_string(),
                    })
                    .collect(),
            })
            .collect(),
        rules: artifact
            .rules()
            .iter()
            .enumerate()
            .map(|(ordinal, rule)| ClosingRuleOutputV1 {
                ordinal,
                sector: rule.sector().to_string(),
                domain_lower: rule
                    .domain()
                    .bounds()
                    .iter()
                    .map(|bounds| bounds.lower())
                    .collect(),
                domain_upper: rule
                    .domain()
                    .bounds()
                    .iter()
                    .map(|bounds| bounds.upper())
                    .collect(),
                pivot: rule.pivot().values().to_vec(),
                nonzero_guards: rule
                    .nonzero_guards()
                    .iter()
                    .map(|guard| guard.polynomial().to_expression().to_canonical_string())
                    .collect(),
                right_hand_side: rule
                    .right_hand_side()
                    .iter()
                    .map(|term| RuleTermOutputV1 {
                        shift: term.shift().values().to_vec(),
                        coefficient: term.coefficient().to_expression().to_canonical_string(),
                    })
                    .collect(),
            })
            .chain(
                artifact
                    .rule_cells()
                    .iter()
                    .enumerate()
                    .map(|(index, cell)| {
                        let rule = cell.rule();
                        ClosingRuleOutputV1 {
                            ordinal: artifact.rules().len() + index,
                            sector: cell.application_domain().sector().to_string(),
                            domain_lower: cell
                                .application_domain()
                                .bounds()
                                .iter()
                                .map(|bounds| bounds.lower())
                                .collect(),
                            domain_upper: cell
                                .application_domain()
                                .bounds()
                                .iter()
                                .map(|bounds| bounds.upper())
                                .collect(),
                            pivot: rule.pivot().values().to_vec(),
                            nonzero_guards: cell
                                .guards()
                                .iter()
                                .map(|guard| {
                                    guard.polynomial().to_expression().to_canonical_string()
                                })
                                .collect(),
                            right_hand_side: cell
                                .terms()
                                .iter()
                                .map(|retained| {
                                    let term =
                                        &rule.right_hand_side()[retained.source_rhs_ordinal()];
                                    RuleTermOutputV1 {
                                        shift: term.shift().values().to_vec(),
                                        coefficient: term
                                            .coefficient()
                                            .to_expression()
                                            .to_canonical_string(),
                                    }
                                })
                                .collect(),
                        }
                    }),
            )
            .collect(),
    };
    let canonical_toml = render::serialize(&output)?;
    Ok(ClosingArtifactGenerateResult::new(
        GENERATE_SCHEMA,
        GENERATED_STATUS,
        canonical_toml,
        encoded,
    ))
}

pub(super) fn inspect_request(
    request: ClosingArtifactInspectRequest,
) -> Result<ClosingArtifactInspectResult, AppError> {
    let artifact = decode_artifact(&request.artifact)?;
    let output = InspectOutputV1 {
        schema: INSPECT_SCHEMA,
        status: INSPECTED_STATUS,
        producer: ProducerOutputV1::current(),
        artifact_source: "request-bytes",
        materialization: LOADED_MATERIALIZATION,
        lifecycle: lifecycle(),
        artifact: artifact_summary(&artifact),
        validation: validation_output(&artifact),
    };
    let canonical_toml = render::serialize(&output)?;
    Ok(ClosingArtifactInspectResult::new(
        INSPECT_SCHEMA,
        INSPECTED_STATUS,
        canonical_toml,
    ))
}

pub(super) fn reduce_request(
    request: ClosingArtifactReduceRequest,
) -> Result<ClosingArtifactReduceResult, AppError> {
    if request.max_rule_applications > super::MAX_CLOSING_RULE_APPLICATIONS {
        return Err(AppError::limit(format!(
            "closing-artifact reduction max_rule_applications is {}, exceeding the application ceiling {}",
            request.max_rule_applications,
            super::MAX_CLOSING_RULE_APPLICATIONS
        )));
    }
    let artifact = decode_artifact(&request.artifact)?;
    let target = IntegralKey::try_new(request.target_powers.iter().copied())
        .map_err(|error| AppError::input(format!("invalid reduction target: {error}")))?;
    if target.powers().len() != artifact.arity() {
        return Err(AppError::input(format!(
            "reduction target has {} powers, but the loaded artifact has arity {}",
            target.powers().len(),
            artifact.arity()
        )));
    }
    let mut limits = ReductionLimits::default();
    limits.max_rule_applications = request.max_rule_applications;
    let mut reducer = Reducer::with_limits(&artifact, limits).map_err(|error| {
        map_reduction_error("cannot initialize closing-artifact reducer", error)
    })?;
    let decomposition = reducer
        .reduce_with_common_mass_homogeneity(&target)
        .map_err(|error| map_reduction_error("cannot reduce target", error))?;
    let statistics = reducer.statistics();
    let mut public_terms = Vec::new();
    let mut output_terms = Vec::new();
    public_terms
        .try_reserve_exact(decomposition.terms().len())
        .map_err(|_| AppError::output_limit("cannot reserve exact master output"))?;
    output_terms
        .try_reserve_exact(decomposition.terms().len())
        .map_err(|_| AppError::output_limit("cannot reserve exact master TOML output"))?;
    for (master, coefficient) in decomposition.terms() {
        let rendered = coefficient
            .unit_mass_coefficient()
            .to_expression()
            .to_canonical_string();
        let term = ExactMasterCoefficient::new(
            master.powers().to_vec(),
            rendered.clone(),
            coefficient.common_mass_squared_power(),
        );
        output_terms.push(ReductionTermOutputV1 {
            master: IntegralKeyOutputV1 {
                powers: master.powers().to_vec(),
            },
            unit_mass_coefficient: rendered,
            common_mass_squared_power: coefficient.common_mass_squared_power().to_string(),
            common_mass_squared_factor: format!(
                "mass_squared^({})",
                coefficient.common_mass_squared_power()
            ),
        });
        public_terms.push(term);
    }
    let family_fingerprint = decomposition.family_fingerprint().to_owned();
    let output = ReduceOutputV1 {
        schema: REDUCE_SCHEMA,
        status: REDUCED_STATUS,
        producer: ProducerOutputV1::current(),
        artifact_source: "request-bytes",
        materialization: LOADED_MATERIALIZATION,
        family_fingerprint: family_fingerprint.clone(),
        target: IntegralKeyOutputV1 {
            powers: target.powers().to_vec(),
        },
        common_mass_squared_symbol: "mass_squared",
        statistics: ReductionStatisticsOutputV1 {
            cache_hits: statistics.cache_hits(),
            rule_applications: statistics.rule_applications(),
            cached_integrals: statistics.cached_integrals(),
            cached_coefficient_terms: statistics.cached_coefficient_terms(),
            cached_coefficient_bytes: statistics.cached_coefficient_bytes(),
        },
        terms: output_terms,
    };
    let canonical_toml = render::serialize(&output)?;
    Ok(ClosingArtifactReduceResult::new(
        REDUCE_SCHEMA,
        REDUCED_STATUS,
        canonical_toml,
        family_fingerprint,
        target.powers().to_vec(),
        public_terms,
    ))
}

fn generate_family(selector: ClosingFamilySelector) -> Result<ClosedArtifact, AppError> {
    match selector {
        ClosingFamilySelector::UnitMassVacuumK1 => derive_one_loop_unit_mass_tadpole(),
        ClosingFamilySelector::UnitMassVacuumK3 => derive_two_loop_unit_mass_sunset(),
    }
    .map_err(|error| AppError::derivation(format!("cannot generate closing artifact: {error}")))
}

fn decode_artifact(bytes: &[u8]) -> Result<ClosedArtifact, AppError> {
    if bytes.len() > super::MAX_CLOSING_ARTIFACT_BYTES {
        return Err(AppError::limit(format!(
            "closing artifact has {} bytes, exceeding the application ceiling {}",
            bytes.len(),
            super::MAX_CLOSING_ARTIFACT_BYTES
        )));
    }
    debug_assert_eq!(
        ArtifactLoadLimits::default().max_artifact_bytes,
        super::MAX_CLOSING_ARTIFACT_BYTES
    );
    ClosedArtifact::decode_durable(bytes).map_err(map_artifact_load_error)
}

fn lifecycle() -> LifecycleOutputV1 {
    LifecycleOutputV1 {
        ownership: "immutable-durable-bytes",
        durable: true,
        persistence: ARTIFACT_ENCODING,
    }
}

fn artifact_summary(artifact: &ClosedArtifact) -> ArtifactSummaryOutputV1 {
    ArtifactSummaryOutputV1 {
        schema: artifact.schema().stable_id(),
        schema_version: artifact.schema().as_u32(),
        algorithm_id: artifact.algorithm_id(),
        arity: artifact.arity(),
        family_fingerprint: artifact.family_fingerprint().to_owned(),
        coefficient_context_fingerprint: artifact.context_fingerprint().to_owned(),
        common_mass_homogeneity: artifact
            .common_mass_homogeneity()
            .map(|proof| proof.stable_id()),
        masters: artifact
            .masters()
            .iter()
            .map(|master| IntegralKeyOutputV1 {
                powers: master.powers().to_vec(),
            })
            .collect(),
        zero_terminals: artifact
            .zero_sectors()
            .iter()
            .map(|terminal| ZeroTerminalOutputV1 {
                sector: terminal.sector().to_string(),
                proof: terminal.proof().stable_id(),
            })
            .collect(),
    }
}

fn validation_output(artifact: &ClosedArtifact) -> ValidationOutputV1 {
    let validation = artifact.validation();
    ValidationOutputV1 {
        source_rows: validation.source_rows(),
        replayed_source_rows: validation.replayed_source_rows(),
        replayed_shift_columns: validation.replayed_shift_columns(),
        guarded_rules: validation.guarded_rules(),
        universally_applicable_guards: validation.universally_applicable_guards(),
        master_terminals: validation.master_terminals(),
        zero_sector_terminals: validation.zero_sector_terminals(),
    }
}
