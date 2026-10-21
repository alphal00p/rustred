//! Two private inventory sources; normalization/refinement remains non-generic.
use super::*;
use crate::OwnerDomainWalkInventory;
use crate::application::routed_campaign::finite_demand::backend::publication::CompletedFiniteSource;
use rustred::family::{IntegralFamily, IntegralKey};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Copy)]
pub(super) enum SourceRequest<'a> {
    Walk(&'a OwnerDomainWalkRequest),
    Finite(&'a CompletedFiniteSource),
}
impl<'a> SourceRequest<'a> {
    pub fn request(self) -> &'a OwnerDomainWalkRequest {
        match self {
            Self::Walk(r) => r,
            Self::Finite(s) => s.request(),
        }
    }
    pub fn schema(self) -> &'static str {
        match self {
            Self::Walk(_) => SCHEMA,
            Self::Finite(_) => FINITE_SCHEMA,
        }
    }
    pub fn binding(
        self,
        options: &MasterReductionOptions,
        feedback: bool,
        deep: bool,
        normalization: MasterNormalizationProfile,
    ) -> Result<String, AppError> {
        let binding = match self {
            Self::Walk(request) => storage::scope_binding(
                request,
                &options.checkpoint,
                options.seed_depth,
                options.containing_sector_depth,
                options.circuit_symmetry_assistance,
                feedback,
                deep,
                normalization,
            ),
            Self::Finite(source) => {
                if deep {
                    return Err(AppError::input(
                        "finite-demand publication has no independent graph replay; use trusted generated provenance",
                    ));
                }
                let mut hash = blake3::Hasher::new();
                hash.update(b"rustred-finite-demand-master-scope-v1");
                hash.update(&source.source_identity());
                hash.update(source.request().checkpoint_binding().as_bytes());
                hash.update(&options.seed_depth.to_le_bytes());
                hash.update(&options.containing_sector_depth.to_le_bytes());
                hash.update(&[
                    u8::from(options.circuit_symmetry_assistance),
                    u8::from(feedback),
                ]);
                normalization.hash(&mut hash);
                for amendment in &source.request().amendments {
                    hash.update(amendment.digest().as_bytes());
                }
                Ok(hash.finalize().to_hex().to_string())
            }
        }?;
        if options.numerator_symmetry_assistance {
            let mut hash = blake3::Hasher::new();
            hash.update(b"numerator-symmetry-assistance-v1");
            hash.update(binding.as_bytes());
            Ok(hash.finalize().to_hex().to_string())
        } else {
            // Preserve historical flag-off source identities exactly.
            Ok(binding)
        }
    }
    /// Must finish before exposing any v3 latest/state marker.
    pub fn prepare_provenance(
        self,
        directory: &Path,
        report: &mut Value,
        cancel: &AtomicBool,
        policy: Option<&crate::FiniteDemandPolicy>,
    ) -> Result<(), AppError> {
        match self {
            Self::Walk(_) => Ok(()),
            Self::Finite(source) => {
                finite_provenance::capture(source, directory, report, cancel, policy)
            }
        }
    }
}

pub(super) enum SourceInventory<'a> {
    Walk(OwnerDomainWalkInventory),
    Finite(&'a CompletedFiniteSource),
}
impl SourceInventory<'_> {
    pub fn summary(&self) -> Value {
        match self {
            Self::Walk(v) => v.summary(),
            Self::Finite(v) => v.summary().clone(),
        }
    }
    pub fn family_owner(&self) -> &Arc<IntegralFamily> {
        match self {
            Self::Walk(v) => v.family_owner(),
            Self::Finite(v) => v.family_owner(),
        }
    }
    pub fn terminal_keys(&self) -> &BTreeSet<IntegralKey> {
        match self {
            Self::Walk(v) => v.terminal_keys(),
            Self::Finite(v) => v.terminal_keys(),
        }
    }
    pub fn payload_digests(&self) -> Result<Vec<String>, AppError> {
        match self {
            Self::Walk(v) => serde_json::from_value(
                v.summary()["verification"]["checkpoint"]["prepared_owner_payload_blake3"].clone(),
            )
            .map_err(|_| {
                AppError::input("inventory extraction did not return prepared owner identities")
            }),
            Self::Finite(v) => Ok(v.owner_payload_digests().to_vec()),
        }
    }
    pub fn save(
        &self,
        directory: &Path,
        report: &mut Value,
        policy: Option<&crate::FiniteDemandPolicy>,
    ) -> Result<(), AppError> {
        match self {
            Self::Walk(v) => census_cache::save(v, directory, report),
            Self::Finite(_) => finite_provenance::validate_required(directory, report, policy),
        }
    }
}
