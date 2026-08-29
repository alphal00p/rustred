use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::application::error::AppError;
use crate::application::input::{
    PreparedProject, ProjectDocumentV1, looks_like_symbolica, prepare_project_document,
    prepare_symbolica_root,
};
use crate::application::model::MetadataValue;
use crate::application::options::InputFormat;

const CAMPAIGN_INPUT_SCHEMA: &str = "rustred.campaign-input.toml.v1";
const MAX_CAMPAIGN_INPUT_ROOTS: usize = 100_000;
const MAX_CAMPAIGN_ROOT_ID_BYTES: usize = 4 * 1024;

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

pub(super) struct PreparedCampaignRoot {
    pub(super) id: String,
    pub(super) project: PreparedProject,
}

pub(super) fn prepare_campaign_roots(
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

pub(super) fn prefix_root_error(id: &str, error: AppError) -> AppError {
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
