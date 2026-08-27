use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustred::{
    CampaignFamilyId, CampaignJobKey, CampaignPlan, CampaignPlanLimits, CampaignRootId,
    CampaignRootSpec, IntegralOrderingPolicy, SectorMask,
};
use serde::{Deserialize, Serialize};
use symbolica::LicenseManager;

use crate::cli::args::InputFormat;
use crate::cli::backend::lower_project;
use crate::cli::error::AppError;
use crate::cli::input::{
    PreparedCliProject, ProjectDocumentV1, looks_like_symbolica, prepare_project_document,
    prepare_symbolica_root,
};
use crate::cli::model::MetadataValue;
use crate::{CampaignPlanRequest, CampaignPlanResult, MAX_OUTPUT_BYTES};

const CAMPAIGN_INPUT_SCHEMA: &str = "rustred.campaign-input.toml.v1";
pub(crate) const CAMPAIGN_OUTPUT_SCHEMA: &str = "rustred.campaign-plan-output.toml.v1";
const EXPRESSION_FORMAT: &str = "rustred.symbolica-canonical-string.v1";
const MAX_CAMPAIGN_INPUT_ROOTS: usize = 100_000;
const ROOT_RENDER_OVERHEAD: usize = 4_096;
const FAMILY_RENDER_OVERHEAD: usize = 2_048;
const JOB_RENDER_OVERHEAD: usize = 1_024;
const ATOM_RENDER_FACTOR: usize = 320;
const STRING_ESCAPE_FACTOR: usize = 6;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampaignDocumentV1 {
    schema: String,
    roots: Vec<CampaignRootDocumentV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampaignRootDocumentV1 {
    id: String,
    integral: Option<String>,
    parameters: Option<Vec<String>>,
    #[serde(default)]
    metadata: BTreeMap<String, MetadataValue>,
    project: Option<ProjectDocumentV1>,
}

struct PreparedCampaignRoot {
    id: String,
    project: PreparedCliProject,
}

struct RootDraft {
    detected_input_form: &'static str,
    input_schema: String,
    canonical_integral: String,
    metadata: BTreeMap<String, MetadataValue>,
}

#[derive(Debug, Serialize)]
struct CampaignPlanOutputV1 {
    schema: &'static str,
    status: &'static str,
    scope: &'static str,
    ordering: &'static str,
    producer: CampaignProducerOutputV1,
    phases: CampaignPhaseStatusOutputV1,
    counts: CampaignCountsOutputV1,
    roots: Vec<CampaignRootOutputV1>,
    families: Vec<CampaignFamilyOutputV1>,
    declared_power_jobs: Vec<CampaignDeclaredPowerJobOutputV1>,
}

#[derive(Debug, Serialize)]
struct CampaignProducerOutputV1 {
    name: &'static str,
    rustred_version: &'static str,
    symbolica_version: &'static str,
    expression_format: &'static str,
}

#[derive(Debug, Serialize)]
struct CampaignPhaseStatusOutputV1 {
    root_ingress: &'static str,
    target_normalization: &'static str,
    dependency_discovery: &'static str,
    derivation: &'static str,
    closure: &'static str,
    publication: &'static str,
}

#[derive(Debug, Serialize)]
struct CampaignCountsOutputV1 {
    roots: usize,
    unique_families: usize,
    declared_power_jobs: usize,
}

#[derive(Debug, Serialize)]
struct CampaignRootOutputV1 {
    ordinal: usize,
    id: String,
    family: usize,
    declared_power_job: usize,
    declared_power_sector: String,
    detected_input_form: &'static str,
    input_schema: String,
    canonical_integral: String,
    metadata: BTreeMap<String, MetadataValue>,
}

#[derive(Debug, Serialize)]
struct CampaignFamilyOutputV1 {
    ordinal: usize,
    name: String,
    fingerprint: String,
    loop_count: usize,
    external_count: usize,
    denominator_count: usize,
}

#[derive(Debug, Serialize)]
struct CampaignDeclaredPowerJobOutputV1 {
    ordinal: usize,
    family: usize,
    declared_power_sector: String,
    ordering: &'static str,
}

pub(crate) fn plan_request(request: CampaignPlanRequest) -> Result<CampaignPlanResult, AppError> {
    let roots = prepare_campaign_roots(&request.source, request.input_format, request.root_id)?;
    let output = compile_roots_only_output(roots)?;
    let serialized = serialize_campaign_output(&output)?;
    Ok(CampaignPlanResult::new(
        CAMPAIGN_OUTPUT_SCHEMA,
        "ok",
        serialized,
    ))
}

fn prepare_campaign_roots(
    source: &str,
    requested_format: InputFormat,
    raw_root_id: Option<String>,
) -> Result<Vec<PreparedCampaignRoot>, AppError> {
    let detected = match requested_format {
        InputFormat::Auto if looks_like_symbolica(source) => InputFormat::Symbolica,
        InputFormat::Auto => InputFormat::Toml,
        explicit => explicit,
    };
    match detected {
        InputFormat::Symbolica => {
            let id = raw_root_id.ok_or_else(|| {
                AppError::Input("raw Symbolica campaign input requires --root-id <ID>".to_owned())
            })?;
            CampaignRootId::try_new(&id).map_err(|error| {
                AppError::Input(format!("invalid raw campaign root identifier: {error}"))
            })?;
            Ok(vec![PreparedCampaignRoot {
                id,
                project: prepare_symbolica_root(source, None, BTreeMap::new(), "raw_symbolica")?,
            }])
        }
        InputFormat::Toml => {
            if raw_root_id.is_some() {
                return Err(AppError::Input(
                    "--root-id is only valid for one raw Symbolica campaign input; TOML roots carry their own ids"
                        .to_owned(),
                ));
            }
            prepare_campaign_document(source)
        }
        InputFormat::Auto => unreachable!("auto input is resolved above"),
    }
}

fn prepare_campaign_document(source: &str) -> Result<Vec<PreparedCampaignRoot>, AppError> {
    let document: CampaignDocumentV1 = toml::from_str(source)
        .map_err(|error| AppError::Input(format!("invalid RustRed campaign TOML: {error}")))?;
    if document.schema != CAMPAIGN_INPUT_SCHEMA {
        return Err(AppError::Input(format!(
            "unsupported campaign schema {:?}; expected {:?}",
            document.schema, CAMPAIGN_INPUT_SCHEMA
        )));
    }
    if document.roots.is_empty() {
        return Err(AppError::Input(
            "campaign TOML needs at least one [[roots]] entry".to_owned(),
        ));
    }
    if document.roots.len() > MAX_CAMPAIGN_INPUT_ROOTS {
        return Err(AppError::Input(format!(
            "campaign TOML has {} roots, exceeding the limit {MAX_CAMPAIGN_INPUT_ROOTS}",
            document.roots.len()
        )));
    }

    // Reject duplicate ingress labels before initializing Symbolica or
    // lowering any family. Distinct labels may still intern to one job.
    let mut seen = BTreeSet::new();
    for root in &document.roots {
        CampaignRootId::try_new(&root.id).map_err(|error| {
            AppError::Input(format!(
                "invalid campaign root identifier {:?}: {error}",
                root.id
            ))
        })?;
        if !seen.insert(root.id.as_str()) {
            return Err(AppError::Input(format!(
                "campaign root identifier {:?} occurs more than once",
                root.id
            )));
        }
    }

    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(document.roots.len())
        .map_err(|_| AppError::Input("cannot reserve bounded campaign root records".to_owned()))?;
    for root in document.roots {
        let CampaignRootDocumentV1 {
            id,
            integral,
            parameters,
            metadata,
            project,
        } = root;
        let project = match (integral, project) {
            (Some(integral), None) => prepare_symbolica_root(
                &integral,
                parameters,
                metadata,
                "campaign_symbolica",
            ),
            (None, Some(_)) if parameters.is_some() || !metadata.is_empty() => {
                Err(AppError::Input(
                    "a nested project must keep parameters and metadata inside that rustred.project.toml.v1 payload"
                        .to_owned(),
                ))
            }
            (None, Some(project)) => prepare_project_document(project),
            (Some(_), Some(_)) => Err(AppError::Input(
                "must choose exactly one of integral and project".to_owned(),
            )),
            (None, None) => Err(AppError::Input(
                "needs either an integral Symbolica expression or a nested project".to_owned(),
            )),
        }
        .map_err(|error| prefix_root_error(&id, error))?;
        prepared.push(PreparedCampaignRoot { id, project });
    }
    Ok(prepared)
}

fn prefix_root_error(id: &str, error: AppError) -> AppError {
    match error {
        AppError::Input(message) => {
            AppError::Input(format!("campaign root {id:?} is invalid: {message}"))
        }
        other => other,
    }
}

fn compile_roots_only_output(
    roots: Vec<PreparedCampaignRoot>,
) -> Result<CampaignPlanOutputV1, AppError> {
    let root_count = roots.len();
    let mut bound = RenderBound::new();
    let mut specs = Vec::new();
    specs
        .try_reserve_exact(root_count)
        .map_err(|_| AppError::Input("cannot reserve campaign root specifications".to_owned()))?;
    let mut drafts = BTreeMap::new();
    for root in roots {
        let lowered =
            lower_project(root.project).map_err(|error| prefix_root_error(&root.id, error))?;
        let (input_form, input_schema, metadata, lowered) = lowered.into_parts();
        let normalized = lowered.normalized();
        bound.add(try_mul(
            "campaign canonical-expression render bound",
            normalized.canonical_atom().as_view().get_byte_size(),
            ATOM_RENDER_FACTOR,
        )?)?;
        bound.add(ROOT_RENDER_OVERHEAD)?;
        bound.add_string(&root.id)?;
        bound.add_string(&input_schema)?;
        bound.add_metadata(&metadata)?;
        let canonical_integral = normalized.canonical_string();
        bound.add_string(&canonical_integral)?;
        let sector =
            SectorMask::try_from_indices(normalized.target().powers()).map_err(|error| {
                AppError::Input(format!(
                    "campaign root {:?} has an invalid target sector: {error}",
                    root.id
                ))
            })?;
        let family = Arc::new(lowered.into_family());
        specs.push(
            CampaignRootSpec::try_new(&root.id, family, sector).map_err(|error| {
                AppError::Input(format!(
                    "cannot authenticate campaign root {:?}: {error}",
                    root.id
                ))
            })?,
        );
        let previous = drafts.insert(
            root.id,
            RootDraft {
                detected_input_form: input_form,
                input_schema,
                canonical_integral,
                metadata,
            },
        );
        debug_assert!(
            previous.is_none(),
            "duplicate roots were rejected at ingress"
        );
    }

    let ordering = IntegralOrderingPolicy::RustRedUnshiftedV1;
    let plan = CampaignPlan::compile(specs, ordering, CampaignPlanLimits::default())
        .map_err(|error| AppError::Input(format!("cannot compile roots-only campaign: {error}")))?;
    plan.verify()
        .map_err(|error| AppError::Input(format!("roots-only campaign replay failed: {error}")))?;
    if plan.stats().dependency_edges() != 0 {
        return Err(AppError::Serialization(
            "roots-only campaign unexpectedly contains dependency edges".to_owned(),
        ));
    }
    let family_ordinals: BTreeMap<CampaignFamilyId, usize> = plan
        .families()
        .keys()
        .cloned()
        .enumerate()
        .map(|(ordinal, id)| (id, ordinal))
        .collect();
    let job_ordinals: BTreeMap<CampaignJobKey, usize> = plan
        .jobs()
        .keys()
        .cloned()
        .enumerate()
        .map(|(ordinal, job)| (job, ordinal))
        .collect();

    let mut root_outputs = Vec::new();
    root_outputs
        .try_reserve_exact(plan.roots().len())
        .map_err(|_| AppError::Serialization("cannot reserve campaign root output".to_owned()))?;
    for (ordinal, (id, record)) in plan.roots().iter().enumerate() {
        let draft = drafts.remove(id.as_str()).ok_or_else(|| {
            AppError::Serialization(format!(
                "compiled campaign root {id} has no retained ingress record"
            ))
        })?;
        let family = *family_ordinals
            .get(record.job().family_id())
            .ok_or_else(|| AppError::Serialization("root family ordinal is missing".to_owned()))?;
        let declared_power_job = *job_ordinals
            .get(record.job())
            .ok_or_else(|| AppError::Serialization("root job ordinal is missing".to_owned()))?;
        root_outputs.push(CampaignRootOutputV1 {
            ordinal,
            id: id.as_str().to_owned(),
            family,
            declared_power_job,
            declared_power_sector: record.job().sector().to_bit_string(),
            detected_input_form: draft.detected_input_form,
            input_schema: draft.input_schema,
            canonical_integral: draft.canonical_integral,
            metadata: draft.metadata,
        });
    }
    if !drafts.is_empty() {
        return Err(AppError::Serialization(
            "retained ingress roots were not represented in the compiled campaign".to_owned(),
        ));
    }

    let mut families = Vec::new();
    families
        .try_reserve_exact(plan.families().len())
        .map_err(|_| AppError::Serialization("cannot reserve campaign family output".to_owned()))?;
    for (ordinal, (id, record)) in plan.families().iter().enumerate() {
        bound.add(FAMILY_RENDER_OVERHEAD)?;
        bound.add_string(id.as_str())?;
        bound.add_string(record.family().name())?;
        families.push(CampaignFamilyOutputV1 {
            ordinal,
            name: record.family().name().to_owned(),
            fingerprint: id.as_str().to_owned(),
            loop_count: record.family().loop_count(),
            external_count: record.family().external_count(),
            denominator_count: record.family().denominator_count(),
        });
    }

    let mut declared_power_jobs = Vec::new();
    declared_power_jobs
        .try_reserve_exact(plan.jobs().len())
        .map_err(|_| AppError::Serialization("cannot reserve campaign job output".to_owned()))?;
    for (ordinal, key) in plan.jobs().keys().enumerate() {
        bound.add(JOB_RENDER_OVERHEAD)?;
        let family = *family_ordinals
            .get(key.family_id())
            .ok_or_else(|| AppError::Serialization("job family ordinal is missing".to_owned()))?;
        declared_power_jobs.push(CampaignDeclaredPowerJobOutputV1 {
            ordinal,
            family,
            declared_power_sector: key.sector().to_bit_string(),
            ordering: key.ordering().stable_id(),
        });
    }
    bound.finish()?;

    Ok(CampaignPlanOutputV1 {
        schema: CAMPAIGN_OUTPUT_SCHEMA,
        status: "ok",
        scope: "roots_only",
        ordering: ordering.stable_id(),
        producer: CampaignProducerOutputV1 {
            name: "RustRed",
            rustred_version: env!("CARGO_PKG_VERSION"),
            symbolica_version: LicenseManager::get_version(),
            expression_format: EXPRESSION_FORMAT,
        },
        phases: CampaignPhaseStatusOutputV1 {
            root_ingress: "complete",
            target_normalization: "not_started",
            dependency_discovery: "not_started",
            derivation: "not_started",
            closure: "not_started",
            publication: "not_started",
        },
        counts: CampaignCountsOutputV1 {
            roots: plan.stats().roots(),
            unique_families: plan.stats().families(),
            declared_power_jobs: plan.stats().jobs(),
        },
        roots: root_outputs,
        families,
        declared_power_jobs,
    })
}

fn serialize_campaign_output(output: &CampaignPlanOutputV1) -> Result<String, AppError> {
    let mut serialized = toml::to_string_pretty(output).map_err(|error| {
        AppError::Serialization(format!(
            "cannot serialize deterministic campaign TOML output: {error}"
        ))
    })?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    if serialized.len() > MAX_OUTPUT_BYTES {
        return Err(AppError::Serialization(format!(
            "campaign TOML output needs {} bytes, exceeding the {MAX_OUTPUT_BYTES}-byte CLI limit",
            serialized.len()
        )));
    }
    Ok(serialized)
}

#[derive(Clone, Copy, Debug, Default)]
struct RenderBound {
    bytes: usize,
}

impl RenderBound {
    const fn new() -> Self {
        Self { bytes: 0 }
    }

    fn add(&mut self, bytes: usize) -> Result<(), AppError> {
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(|| {
            AppError::Serialization("campaign TOML render bound overflowed".to_owned())
        })?;
        if self.bytes > MAX_OUTPUT_BYTES {
            return Err(AppError::Serialization(format!(
                "campaign TOML has a conservative {}-byte render bound, exceeding the {MAX_OUTPUT_BYTES}-byte CLI limit",
                self.bytes
            )));
        }
        Ok(())
    }

    fn add_string(&mut self, value: &str) -> Result<(), AppError> {
        let escaped = try_mul(
            "campaign TOML string render bound",
            value.len(),
            STRING_ESCAPE_FACTOR,
        )?;
        self.add(escaped.checked_add(64).ok_or_else(|| {
            AppError::Serialization("campaign TOML string bound overflowed".to_owned())
        })?)
    }

    fn add_metadata(&mut self, metadata: &BTreeMap<String, MetadataValue>) -> Result<(), AppError> {
        for (key, value) in metadata {
            self.add_string(key)?;
            match value {
                MetadataValue::String(value) => self.add_string(value)?,
                MetadataValue::StringArray(values) => {
                    for value in values {
                        self.add_string(value)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<(), AppError> {
        if self.bytes <= MAX_OUTPUT_BYTES {
            Ok(())
        } else {
            Err(AppError::Serialization(format!(
                "campaign TOML render bound {} exceeds the {MAX_OUTPUT_BYTES}-byte CLI limit",
                self.bytes
            )))
        }
    }
}

fn try_mul(resource: &'static str, left: usize, right: usize) -> Result<usize, AppError> {
    left.checked_mul(right)
        .ok_or_else(|| AppError::Serialization(format!("{resource} overflowed")))
}
