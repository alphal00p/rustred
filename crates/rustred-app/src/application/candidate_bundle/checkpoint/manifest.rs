use serde::{Deserialize, Serialize};

use crate::application::{AppError, MAX_INPUT_BYTES};

use super::super::{codec, model::*, policy, preparation};

/// Structural/provenance metadata only; ordinary shard ProgramRecords do not
/// independently carry the exact backend. This manifest supplies that campaign
/// binding, not authentication or additional candidate authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::candidate_bundle) struct CheckpointManifest {
    version: u32,
    recipe: String,
    family_source: String,
    input_format: String,
    family_fingerprint: String,
    root_sector: Vec<bool>,
    permutation: Option<Vec<usize>>,
    integral_order: String,
    solver_policy: String,
    exact_backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    discovery_strategy: Option<super::super::CandidateDiscoveryStrategy>,
    /// None requests the full root downset; Some binds an explicit exact set.
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_sectors: Option<Vec<Vec<bool>>>,
    /// Original prepared order, independent of worker/completion order.
    sectors: Vec<Vec<bool>>,
}

const VERSION: u32 = 4;
// Bump when source construction, preconditioning, random seed or backend
// defaults change in a way incompatible with resuming this generation recipe.
const RECIPE: &str = "ordinary-family-candidates-checkpoint-selection-v4";

impl CheckpointManifest {
    pub(in crate::application::candidate_bundle) fn for_request(
        request: &FamilyCandidatesRequest,
        family_fingerprint: &str,
        root_sector: &[bool],
        sectors: Vec<Vec<bool>>,
    ) -> Result<Self, AppError> {
        if preparation::root(root_sector.len(), &request.nonpositive_indices)? != root_sector {
            return Err(AppError::input("checkpoint root differs from the request"));
        }
        let selected_sectors = super::super::selection::canonical_masks(
            root_sector,
            request.selected_sectors.as_deref(),
            request.bundle_limits.max_collection_entries,
        )?;
        let manifest = Self {
            version: VERSION,
            recipe: RECIPE.into(),
            family_source: request.source.clone(),
            input_format: request.input_format.as_str().into(),
            family_fingerprint: family_fingerprint.into(),
            root_sector: root_sector.to_vec(),
            permutation: request.permutation.clone(),
            integral_order: super::super::order::request_policy(request, root_sector.len())?
                .stable_id()
                .to_string(),
            solver_policy: policy::encode_request(request),
            exact_backend: request.exact_backend.as_str().into(),
            discovery_strategy: request.discovery_strategy.clone(),
            selected_sectors,
            sectors,
        };
        manifest.validate(request.bundle_limits)?;
        Ok(manifest)
    }

    pub(in crate::application::candidate_bundle) fn sectors(&self) -> &[Vec<bool>] {
        &self.sectors
    }

    pub(super) fn validate(&self, limits: CandidateBundleLimits) -> Result<(), AppError> {
        if self.version != VERSION || self.recipe != RECIPE {
            return Err(AppError::schema(
                "unsupported candidate checkpoint recipe/version",
            ));
        }
        if self.family_source.len() > MAX_INPUT_BYTES {
            return Err(AppError::limit(
                "checkpoint source exceeds its input byte limit",
            ));
        }
        policy::numerical_depth(&self.solver_policy)?;
        self.exact_backend.parse::<CandidateExactBackend>()?;
        let arity = self.root_sector.len();
        if arity == 0 || self.family_fingerprint.is_empty() {
            return Err(AppError::input(
                "checkpoint requires a nonempty root and family fingerprint",
            ));
        }
        preparation::validate_permutation(arity, self.permutation.as_deref())?;
        super::super::order::bound_policy(
            &self.integral_order,
            arity,
            self.permutation.as_deref(),
        )?;
        if let Some(strategy) = &self.discovery_strategy {
            strategy.validate(arity, &self.sectors, limits.max_collection_entries)?;
        }
        let entries = self
            .sectors
            .len()
            .checked_mul(arity)
            .and_then(|n| n.checked_add(self.sectors.len()))
            .and_then(|n| n.checked_add(arity))
            .and_then(|n| {
                self.selected_sectors.as_ref().map_or(Some(n), |selected| {
                    selected
                        .len()
                        .checked_mul(arity + 1)
                        .and_then(|extra| n.checked_add(extra))
                })
            })
            .ok_or_else(|| AppError::limit("checkpoint manifest collection count overflow"))?;
        if entries > limits.max_collection_entries {
            return Err(AppError::limit(
                "checkpoint manifest exceeds its collection limit",
            ));
        }
        if self.sectors.windows(2).any(|pair| pair[0] >= pair[1])
            || self.sectors.iter().any(|sector| {
                sector.len() != arity
                    || sector
                        .iter()
                        .zip(&self.root_sector)
                        .any(|(&active, &root)| active && !root)
            })
        {
            return Err(AppError::input(
                "checkpoint sectors must be strictly ordered, distinct, correct-arity subsets of the root",
            ));
        }
        if let Some(selected) = &self.selected_sectors {
            if selected.is_empty() || selected != &self.sectors {
                return Err(AppError::input(
                    "checkpoint selected_sectors must equal its nonempty canonical sector list",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn encode(&self, limit: usize) -> Result<Vec<u8>, AppError> {
        let text =
            toml::to_string(self).map_err(|error| AppError::serialization(error.to_string()))?;
        if text.len() > limit {
            return Err(AppError::limit(
                "checkpoint manifest exceeds its byte limit",
            ));
        }
        Ok(text.into_bytes())
    }

    pub(super) fn admit_manifest(
        &self,
        bytes: &[u8],
        limits: CandidateBundleLimits,
    ) -> Result<(), AppError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| AppError::schema("checkpoint manifest is not UTF-8"))?;
        let found: Self = toml::from_str(text)
            .map_err(|error| AppError::schema(format!("invalid checkpoint manifest: {error}")))?;
        found.validate(limits)?;
        if &found != self {
            return Err(AppError::input(
                "checkpoint manifest differs from the prepared request/sector list",
            ));
        }
        Ok(())
    }

    /// No native State import: codec preflight checks framing and structural
    /// shapes, then every persisted request binding and the sole sector match.
    /// Native family, coefficient IDs and indexed contexts are checked once by
    /// assembly; this admission is not an algebra-validation receipt.
    pub(super) fn admit_shard(
        &self,
        ordinal: usize,
        bytes: &[u8],
        limits: CandidateBundleLimits,
    ) -> Result<(usize, usize), AppError> {
        let expected = self.sectors.get(ordinal).ok_or_else(|| {
            AppError::input("checkpoint sector ordinal lies outside its manifest")
        })?;
        let (_, record, _) = codec::read_structure(bytes, limits)?;
        if record.family_source != self.family_source
            || record.input_format != self.input_format
            || record.family_fingerprint != self.family_fingerprint
            || record.root_sector != self.root_sector
            || record.permutation != self.permutation
            || record.integral_order != self.integral_order
            || record.solver_policy != self.solver_policy
            || record.sectors.len() != 1
            || record.sectors[0].sector != *expected
        {
            return Err(AppError::input(
                "checkpoint shard differs from its request/sector binding",
            ));
        }
        let sector = &record.sectors[0];
        Ok((sector.rules.len(), sector.finite_residuals.len()))
    }
}
