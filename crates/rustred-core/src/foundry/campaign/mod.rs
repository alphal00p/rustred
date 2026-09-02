//! Bounded, diagnostic foundry campaigns.
//!
//! This module is the stable value boundary around RustRed's private exact
//! completion engine. A campaign report is detached telemetry: it contains no
//! live ledger identity, rule owner, circuit, or publication authority. In
//! particular, an incomplete diagnostic run is not a [`super::artifact::ClosedArtifact`].

mod autonomous;
mod config;
mod error;
mod k6_resource;
mod k6_wave;
mod model;
mod preset_k6;
mod provenance;
mod rejection;
mod requested;
mod run;

pub use autonomous::{FoundryAutonomousSelectionRound, FoundryAutonomousSelectionTelemetry};
pub use config::{
    FOUNDRY_CAMPAIGN_CONFIG_SCHEMA, FOUNDRY_CAMPAIGN_REPORT_SCHEMA, FoundryCampaignConfig,
    FoundryCampaignDomainHint, FoundryCampaignExternalHints, FoundryCampaignItinerary,
    FoundryCampaignPreset, FoundryCampaignProbe, MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINT_ARITY,
    MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINTS, MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATE_CELLS,
    MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATES, MAX_FOUNDRY_CAMPAIGN_PROBES,
};
pub use error::{FoundryCampaignConfigError, FoundryCampaignError, FoundryCampaignSetupStage};
pub use k6_wave::{
    K6_FULL_RANK_WAVE_WIDTHS, K6IncompleteOrbitReport, K6IncompleteSectorWave,
    K6OrbitCampaignProgress, K6OrbitCampaignState, K6PublishedSectorWaves, K6WaveCampaignErrorKind,
    K6WaveCampaignOutcome, K6WaveCampaignProgress, K6WaveCampaignRunError, K6WaveCampaignState,
    run_k6_full_rank_wave_campaign, run_k6_full_rank_wave_campaign_with_progress,
};
pub use model::{
    FoundryCampaignCensus, FoundryCampaignCoverageObstruction, FoundryCampaignCoverageStatus,
    FoundryCampaignNeedsRefinementReason, FoundryCampaignOperationalLimit, FoundryCampaignProgress,
    FoundryCampaignReport, FoundryCampaignRun, FoundryCampaignSnapshot, FoundryCampaignStop,
    FoundryCampaignTaskLocation, FoundryCampaignTaskLocationKind, FoundryCampaignUncoveredBox,
};
pub use provenance::FoundrySearchProvenance;
pub use rejection::{
    FoundryCampaignProbeStage, FoundryCampaignSchedulerRejection,
    FoundryCampaignSchedulerRejectionCategory,
};
pub use run::{run_foundry_campaign, run_foundry_campaign_with_progress};

#[cfg(test)]
pub(crate) use preset_k6::source_safe_k6_closure_carrier_for_test;

#[cfg(test)]
mod tests;
