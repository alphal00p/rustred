use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustred::family::IntegralFamily;
use rustred::sector::{Mask, OrderingPolicy};
use serde::Serialize;

use crate::application::error::AppError;
use crate::application::lowering::lower_project;
use crate::application::model::MetadataValue;
use crate::application::producer::ProducerOutputV1;

use super::CAMPAIGN_OUTPUT_SCHEMA;
use super::decode::{PreparedCampaignRoot, prefix_root_error};
use super::render::{RenderBound, render_mask};

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
        self.0.fingerprint()
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
pub(super) struct CampaignPlanOutputV1 {
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

pub(super) fn compile_roots_only_output(
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
        bound.add_atom_payload(normalized.canonical_atom().as_view().get_byte_size())?;
        bound.add_root_overhead()?;
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
            declared_power_sector: render_mask(&record.job.sector)?,
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
        bound.add_family_overhead()?;
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
        bound.add_job_overhead()?;
        let family = *family_ordinals.get(&key.family).ok_or_else(|| {
            AppError::internal_invariant("job family ordinal is missing".to_owned())
        })?;
        declared_power_jobs.push(CampaignDeclaredPowerJobOutputV1 {
            ordinal,
            family,
            declared_power_sector: render_mask(&key.sector)?,
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
