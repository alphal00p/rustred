use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustred::IntegralFamily;
use rustred::sector::{Mask, OrderingPolicy};
use serde::{Deserialize, Serialize};

use super::super::error::AppError;
use super::super::input::{
    PreparedProject, ProjectDocumentV1, looks_like_symbolica, prepare_project_document,
    prepare_symbolica_root,
};
use super::super::lowering::lower_project;
use super::super::model::MetadataValue;
use super::super::options::InputFormat;
use super::super::producer::ProducerOutputV1;
use super::super::{CampaignPlanRequest, CampaignPlanResult, MAX_OUTPUT_BYTES};

const CAMPAIGN_INPUT_SCHEMA: &str = "rustred.campaign-input.toml.v1";
pub(crate) const CAMPAIGN_OUTPUT_SCHEMA: &str = "rustred.campaign-plan-output.toml.v1";
const MAX_CAMPAIGN_INPUT_ROOTS: usize = 100_000;
const MAX_CAMPAIGN_ROOT_ID_BYTES: usize = 4 * 1024;
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
    project: PreparedProject,
}

struct RootDraft {
    detected_input_form: &'static str,
    input_schema: String,
    canonical_integral: String,
    metadata: BTreeMap<String, MetadataValue>,
}

#[derive(Clone)]
struct FamilyKey(Arc<IntegralFamily>);

impl FamilyKey {
    fn family(&self) -> &IntegralFamily {
        &self.0
    }

    fn fingerprint(&self) -> &str {
        self.0.fingerprint_ref()
    }
}

impl PartialEq for FamilyKey {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint() == other.fingerprint()
    }
}

impl Eq for FamilyKey {}

impl PartialOrd for FamilyKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FamilyKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fingerprint().cmp(other.fingerprint())
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DeclaredPowerJobKey {
    family: FamilyKey,
    sector: Mask,
}

struct RootRecord {
    job: DeclaredPowerJobKey,
    draft: RootDraft,
}

#[derive(Debug, Serialize)]
struct CampaignPlanOutputV1 {
    schema: &'static str,
    status: &'static str,
    scope: &'static str,
    ordering: &'static str,
    producer: ProducerOutputV1,
    counts: CampaignCountsOutputV1,
    roots: Vec<CampaignRootOutputV1>,
    families: Vec<CampaignFamilyOutputV1>,
    declared_power_jobs: Vec<CampaignDeclaredPowerJobOutputV1>,
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
                AppError::input("raw Symbolica campaign input requires root_id".to_owned())
            })?;
            validate_root_id(&id).map_err(|error| {
                AppError::input(format!("invalid raw campaign root identifier: {error}"))
            })?;
            Ok(vec![PreparedCampaignRoot {
                id,
                project: prepare_symbolica_root(source, None, BTreeMap::new(), "raw_symbolica")?,
            }])
        }
        InputFormat::Toml => {
            if raw_root_id.is_some() {
                return Err(AppError::input(
                    "root_id is only valid for one raw Symbolica campaign input; TOML roots carry their own ids"
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
        .map_err(|error| AppError::input(format!("invalid RustRed campaign TOML: {error}")))?;
    if document.schema != CAMPAIGN_INPUT_SCHEMA {
        return Err(AppError::schema(format!(
            "unsupported campaign schema {:?}; expected {:?}",
            document.schema, CAMPAIGN_INPUT_SCHEMA
        )));
    }
    if document.roots.is_empty() {
        return Err(AppError::input(
            "campaign TOML needs at least one [[roots]] entry".to_owned(),
        ));
    }
    if document.roots.len() > MAX_CAMPAIGN_INPUT_ROOTS {
        return Err(AppError::limit(format!(
            "campaign TOML has {} roots, exceeding the limit {MAX_CAMPAIGN_INPUT_ROOTS}",
            document.roots.len()
        )));
    }

    // Reject duplicate ingress labels before initializing Symbolica or
    // lowering any family. Distinct labels may still intern to one job.
    let mut seen = BTreeSet::new();
    for root in &document.roots {
        validate_root_id(&root.id).map_err(|error| {
            AppError::input(format!(
                "invalid campaign root identifier {:?}: {error}",
                root.id
            ))
        })?;
        if !seen.insert(root.id.as_str()) {
            return Err(AppError::input(format!(
                "campaign root identifier {:?} occurs more than once",
                root.id
            )));
        }
    }

    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(document.roots.len())
        .map_err(|_| AppError::limit("cannot reserve bounded campaign root records".to_owned()))?;
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
                Err(AppError::input(
                    "a nested project must keep parameters and metadata inside that rustred.project.toml.v1 payload"
                        .to_owned(),
                ))
            }
            (None, Some(project)) => prepare_project_document(project),
            (Some(_), Some(_)) => Err(AppError::input(
                "must choose exactly one of integral and project".to_owned(),
            )),
            (None, None) => Err(AppError::input(
                "needs either an integral Symbolica expression or a nested project".to_owned(),
            )),
        }
        .map_err(|error| prefix_root_error(&id, error))?;
        prepared.push(PreparedCampaignRoot { id, project });
    }
    Ok(prepared)
}

fn prefix_root_error(id: &str, error: AppError) -> AppError {
    let kind = error.kind();
    let message = error.into_message();
    AppError::new(kind, format!("campaign root {id:?} is invalid: {message}"))
}

fn validate_root_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("a campaign root identifier cannot be empty".to_owned());
    }
    if id.len() > MAX_CAMPAIGN_ROOT_ID_BYTES {
        return Err(format!(
            "campaign root identifier bytes needs {} units, exceeding limit {MAX_CAMPAIGN_ROOT_ID_BYTES}",
            id.len()
        ));
    }
    Ok(())
}

fn compile_roots_only_output(
    roots: Vec<PreparedCampaignRoot>,
) -> Result<CampaignPlanOutputV1, AppError> {
    let mut bound = RenderBound::new();
    let mut root_records = BTreeMap::new();
    let mut families: BTreeSet<FamilyKey> = BTreeSet::new();
    let mut jobs: BTreeSet<DeclaredPowerJobKey> = BTreeSet::new();
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
        let sector = Mask::try_from_indices(normalized.target().powers()).map_err(|error| {
            AppError::input(format!(
                "campaign root {:?} has an invalid target sector: {error}",
                root.id
            ))
        })?;
        let family = FamilyKey(Arc::new(lowered.into_family()));
        if sector.arity() != family.family().denominator_count() {
            return Err(AppError::input(format!(
                "cannot compile roots-only campaign: campaign root {} has sector arity {}, expected {}",
                root.id,
                sector.arity(),
                family.family().denominator_count()
            )));
        }
        let family = if let Some(existing) = families.get(&family) {
            if existing.family().limits() != family.family().limits() {
                return Err(AppError::input(format!(
                    "cannot compile roots-only campaign: campaign family with {} identity bytes was repeated with a different retained resource policy",
                    family.fingerprint().len()
                )));
            }
            existing.clone()
        } else {
            families.insert(family.clone());
            family
        };
        let job = DeclaredPowerJobKey { family, sector };
        let job = if let Some(existing) = jobs.get(&job) {
            existing.clone()
        } else {
            jobs.insert(job.clone());
            job
        };
        let previous = root_records.insert(
            root.id,
            RootRecord {
                job,
                draft: RootDraft {
                    detected_input_form: input_form,
                    input_schema,
                    canonical_integral,
                    metadata,
                },
            },
        );
        debug_assert!(
            previous.is_none(),
            "duplicate roots were rejected at ingress"
        );
    }

    let ordering = OrderingPolicy::RustRedUnshiftedV1;
    let family_ordinals: BTreeMap<FamilyKey, usize> = families
        .iter()
        .cloned()
        .enumerate()
        .map(|(ordinal, id)| (id, ordinal))
        .collect();
    let job_ordinals: BTreeMap<DeclaredPowerJobKey, usize> = jobs
        .iter()
        .cloned()
        .enumerate()
        .map(|(ordinal, job)| (job, ordinal))
        .collect();

    let mut root_outputs = Vec::new();
    root_outputs
        .try_reserve_exact(root_records.len())
        .map_err(|_| AppError::output_limit("cannot reserve campaign root output".to_owned()))?;
    for (ordinal, (id, record)) in root_records.into_iter().enumerate() {
        let family = *family_ordinals.get(&record.job.family).ok_or_else(|| {
            AppError::internal_invariant("root family ordinal is missing".to_owned())
        })?;
        let declared_power_job = *job_ordinals.get(&record.job).ok_or_else(|| {
            AppError::internal_invariant("root job ordinal is missing".to_owned())
        })?;
        root_outputs.push(CampaignRootOutputV1 {
            ordinal,
            id,
            family,
            declared_power_job,
            declared_power_sector: record.job.sector.to_bit_string(),
            detected_input_form: record.draft.detected_input_form,
            input_schema: record.draft.input_schema,
            canonical_integral: record.draft.canonical_integral,
            metadata: record.draft.metadata,
        });
    }

    let mut family_outputs = Vec::new();
    family_outputs
        .try_reserve_exact(families.len())
        .map_err(|_| AppError::output_limit("cannot reserve campaign family output".to_owned()))?;
    for (ordinal, key) in families.iter().enumerate() {
        bound.add(FAMILY_RENDER_OVERHEAD)?;
        bound.add_string(key.fingerprint())?;
        bound.add_string(key.family().name())?;
        family_outputs.push(CampaignFamilyOutputV1 {
            ordinal,
            name: key.family().name().to_owned(),
            fingerprint: key.fingerprint().to_owned(),
            loop_count: key.family().loop_count(),
            external_count: key.family().external_count(),
            denominator_count: key.family().denominator_count(),
        });
    }

    let mut declared_power_jobs = Vec::new();
    declared_power_jobs
        .try_reserve_exact(jobs.len())
        .map_err(|_| AppError::output_limit("cannot reserve campaign job output".to_owned()))?;
    for (ordinal, key) in jobs.iter().enumerate() {
        bound.add(JOB_RENDER_OVERHEAD)?;
        let family = *family_ordinals.get(&key.family).ok_or_else(|| {
            AppError::internal_invariant("job family ordinal is missing".to_owned())
        })?;
        declared_power_jobs.push(CampaignDeclaredPowerJobOutputV1 {
            ordinal,
            family,
            declared_power_sector: key.sector.to_bit_string(),
            ordering: ordering.stable_id(),
        });
    }
    bound.finish()?;

    Ok(CampaignPlanOutputV1 {
        schema: CAMPAIGN_OUTPUT_SCHEMA,
        status: "ok",
        scope: "roots_only",
        ordering: ordering.stable_id(),
        producer: ProducerOutputV1::current(),
        counts: CampaignCountsOutputV1 {
            roots: root_outputs.len(),
            unique_families: family_outputs.len(),
            declared_power_jobs: declared_power_jobs.len(),
        },
        roots: root_outputs,
        families: family_outputs,
        declared_power_jobs,
    })
}

fn serialize_campaign_output(output: &CampaignPlanOutputV1) -> Result<String, AppError> {
    let mut serialized = toml::to_string_pretty(output).map_err(|error| {
        AppError::serialization(format!(
            "cannot serialize deterministic campaign TOML output: {error}"
        ))
    })?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    if serialized.len() > MAX_OUTPUT_BYTES {
        return Err(AppError::output_limit(format!(
            "campaign TOML output needs {} bytes, exceeding the {MAX_OUTPUT_BYTES}-byte application limit",
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
            AppError::output_limit("campaign TOML render bound overflowed".to_owned())
        })?;
        if self.bytes > MAX_OUTPUT_BYTES {
            return Err(AppError::output_limit(format!(
                "campaign TOML has a conservative {}-byte render bound, exceeding the {MAX_OUTPUT_BYTES}-byte application limit",
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
            AppError::output_limit("campaign TOML string bound overflowed".to_owned())
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
            Err(AppError::output_limit(format!(
                "campaign TOML render bound {} exceeds the {MAX_OUTPUT_BYTES}-byte application limit",
                self.bytes
            )))
        }
    }
}

fn try_mul(resource: &'static str, left: usize, right: usize) -> Result<usize, AppError> {
    left.checked_mul(right)
        .ok_or_else(|| AppError::output_limit(format!("{resource} overflowed")))
}
