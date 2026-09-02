use std::sync::{Arc, OnceLock};

mod carrier;

use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::{
    FULL_RANK_ORBITS, canonical_three_loop_family, derive_k6_terminal_authority,
    derive_k6_terminal_authority_with_ordering,
};
#[cfg(test)]
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::source_discovery::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaLimits, OrdinarySourceIncidenceIndex,
    ProbeCampaignLimits,
};
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator,
    TranslatedSourceBatch,
};
use crate::sector::{Mask, OrderingPolicy};

use super::k6_resource::K6CampaignResourceProfile;
use super::{FoundryCampaignError, FoundryCampaignSetupStage};
use carrier::source_safe_closure_carrier;

static K6_FAMILY: OnceLock<Result<IntegralFamily, CachedSetupFailure>> = OnceLock::new();
static K6_ALGEBRA_INPUTS: OnceLock<Result<K6AlgebraInputs, CachedSetupFailure>> = OnceLock::new();
static K6_ROOT_PREDECESSOR: OnceLock<Result<ImmutableOwnerSnapshot, CachedSetupFailure>> =
    OnceLock::new();

#[derive(Clone, Debug)]
struct CachedSetupFailure {
    stage: FoundryCampaignSetupStage,
    message: String,
}

impl CachedSetupFailure {
    fn new(stage: FoundryCampaignSetupStage, error: impl std::fmt::Display) -> Self {
        Self {
            stage,
            message: error.to_string(),
        }
    }

    fn invariant(stage: FoundryCampaignSetupStage, message: &'static str) -> Self {
        Self {
            stage,
            message: message.to_owned(),
        }
    }

    fn public(&self) -> FoundryCampaignError {
        FoundryCampaignError::Setup {
            stage: self.stage,
            message: self.message.clone(),
        }
    }
}

/// Immutable algebra shared by every full-rank K6 orbit campaign.
///
/// No sector, predecessor authority, ledger nonce, or discovery revision is
/// cached here. Each sector invocation constructs a fresh ledger against the
/// caller's exact immutable predecessor.
pub(crate) struct K6AlgebraInputs {
    generator: ParametricIbpGenerator<'static>,
    completed: CompletedIbpSourceRows,
    zero_sources: TranslatedSourceBatch,
}

impl K6AlgebraInputs {
    pub(crate) const fn generator(&self) -> &ParametricIbpGenerator<'static> {
        &self.generator
    }

    pub(crate) const fn completed(&self) -> &CompletedIbpSourceRows {
        &self.completed
    }

    pub(crate) const fn zero_sources(&self) -> &TranslatedSourceBatch {
        &self.zero_sources
    }
}

pub(crate) fn shared_k6_algebra_inputs() -> Result<&'static K6AlgebraInputs, FoundryCampaignError> {
    K6_ALGEBRA_INPUTS
        .get_or_init(build_k6_algebra_inputs)
        .as_ref()
        .map_err(CachedSetupFailure::public)
}

/// Shared terminal/zero/factorization predecessor used to start the first K6
/// wave. Returning an owned snapshot preserves its immutable authority while
/// keeping every fresh ledger independently nonce-bearing.
pub(crate) fn shared_k6_root_predecessor() -> Result<ImmutableOwnerSnapshot, FoundryCampaignError> {
    K6_ROOT_PREDECESSOR
        .get_or_init(build_k6_root_predecessor)
        .as_ref()
        .map(Clone::clone)
        .map_err(CachedSetupFailure::public)
}

/// Reuse the production K6 source-stencil carrier in crate-internal
/// diagnostics. Keeping this test seam beside the production construction
/// prevents campaign and closure-sweep fixtures from silently reverting to
/// the unsafe full `i64` carrier at representability boundaries.
#[cfg(test)]
pub(crate) fn source_safe_k6_closure_carrier_for_test(
    zero_sources: &TranslatedSourceBatch,
    sector: &Mask,
) -> Result<LatticeBox, FoundryCampaignError> {
    let iterations = ProbeCampaignLimits::default()
        .replay
        .scheduler
        .max_iterations_per_probe;
    source_safe_closure_carrier(zero_sources, sector, iterations).map_err(|error| error.public())
}

/// Install the terminal/factorization root under the exact proof ordering of
/// the surrounding campaign. The natural authority remains process-cached;
/// payload-bearing research orders are cold-boundary values and are installed
/// once per top-level run, then cheaply cloned across all sibling ledgers.
pub(crate) fn k6_root_predecessor_for_ordering(
    ordering: OrderingPolicy,
) -> Result<ImmutableOwnerSnapshot, FoundryCampaignError> {
    if ordering == OrderingPolicy::default() {
        return shared_k6_root_predecessor();
    }
    let authority = derive_k6_terminal_authority_with_ordering(ordering)
        .map(Arc::new)
        .map_err(|error| {
            FoundryCampaignError::setup(FoundryCampaignSetupStage::TerminalAuthority, error)
        })?;
    let predecessor =
        ImmutableOwnerSnapshot::try_from_terminal_authority(authority, Default::default())
            .map_err(|error| {
                FoundryCampaignError::setup(FoundryCampaignSetupStage::PredecessorSnapshot, error)
            })?;
    if predecessor.closed_layer_count() != 0 {
        return Err(FoundryCampaignError::Invariant {
            detail: "K6 ordered root unexpectedly owns an ordinary-rule layer",
        });
    }
    Ok(predecessor)
}

/// Construct a fresh source-safe exact ledger for one authenticated full-rank
/// K6 orbit representative and the caller's immutable predecessor.
#[cfg(test)]
pub(crate) fn try_new_k6_full_rank_ledger(
    inputs: &K6AlgebraInputs,
    representative: [i64; 6],
    predecessor: ImmutableOwnerSnapshot,
) -> Result<CanonicalExactOwnerLedger, FoundryCampaignError> {
    try_new_k6_full_rank_ledger_with_limits(
        inputs,
        representative,
        predecessor,
        OrderingPolicy::default(),
        ExactOwnerCoverDeltaLimits::default(),
        ProbeCampaignLimits::default(),
    )
}

/// Construct a fresh K6 ledger under the exact proof order already used to
/// install `predecessor`.
pub(crate) fn try_new_k6_full_rank_ledger_with_profile_and_ordering(
    inputs: &K6AlgebraInputs,
    representative: [i64; 6],
    predecessor: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    profile: K6CampaignResourceProfile,
    campaign_limits: ProbeCampaignLimits,
) -> Result<CanonicalExactOwnerLedger, FoundryCampaignError> {
    try_new_k6_full_rank_ledger_with_limits(
        inputs,
        representative,
        predecessor,
        ordering,
        profile.exact_limits(),
        campaign_limits,
    )
}

fn try_new_k6_full_rank_ledger_with_limits(
    inputs: &K6AlgebraInputs,
    representative: [i64; 6],
    predecessor: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    limits: ExactOwnerCoverDeltaLimits,
    campaign_limits: ProbeCampaignLimits,
) -> Result<CanonicalExactOwnerLedger, FoundryCampaignError> {
    if predecessor.canonicalizer_ordering() != Some(ordering) {
        return Err(FoundryCampaignError::Setup {
            stage: FoundryCampaignSetupStage::Ledger,
            message: format!(
                "K6 predecessor canonicalizer uses {:?}, but the ledger requested {}",
                predecessor
                    .canonicalizer_ordering()
                    .map(|policy| policy.stable_id()),
                ordering.stable_id(),
            ),
        });
    }
    if !FULL_RANK_ORBITS
        .iter()
        .any(|orbit| orbit.representative == representative)
    {
        return Err(FoundryCampaignError::Setup {
            stage: FoundryCampaignSetupStage::Sector,
            message: "requested K6 sector is not a full-rank orbit representative".to_owned(),
        });
    }
    let sector = Mask::try_from_indices(&representative)
        .map_err(|error| FoundryCampaignError::setup(FoundryCampaignSetupStage::Sector, error))?;
    let closure_carrier = source_safe_closure_carrier(
        inputs.zero_sources(),
        &sector,
        campaign_limits.replay.scheduler.max_iterations_per_probe,
    )
    .map_err(|error| error.public())?;
    let terminal = IntegralKey::try_new(representative)
        .map_err(|error| FoundryCampaignError::setup(FoundryCampaignSetupStage::Ledger, error))?;
    CanonicalExactOwnerLedger::try_new_with_closure_carrier(
        inputs.generator().context(),
        predecessor,
        sector,
        ordering,
        [terminal],
        closure_carrier,
        limits,
    )
    .map_err(|error| FoundryCampaignError::setup(FoundryCampaignSetupStage::Ledger, error))
}

fn build_k6_algebra_inputs() -> Result<K6AlgebraInputs, CachedSetupFailure> {
    let family = K6_FAMILY
        .get_or_init(|| {
            canonical_three_loop_family()
                .map_err(|error| CachedSetupFailure::new(FoundryCampaignSetupStage::Family, error))
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(family, ParametricIbpConfig::default())
            .map_err(|error| {
                CachedSetupFailure::new(FoundryCampaignSetupStage::OrdinarySources, error)
            })?;
    let prepared = generator.prepare_ordinary_ibp().map_err(|error| {
        CachedSetupFailure::new(FoundryCampaignSetupStage::OrdinarySources, error)
    })?;
    let expected_rows = prepared.len();
    let rows = (0..expected_rows)
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows).map_err(|error| {
        CachedSetupFailure::new(FoundryCampaignSetupStage::OrdinarySources, error)
    })?;
    if !completed.is_complete_ordinary() {
        return Err(CachedSetupFailure::invariant(
            FoundryCampaignSetupStage::OrdinarySources,
            "prepared source barrier is not the complete ordinary module",
        ));
    }
    if expected_rows != 9 || completed.source_row_count() != expected_rows {
        return Err(CachedSetupFailure::invariant(
            FoundryCampaignSetupStage::OrdinarySources,
            "three-loop vacuum preset did not produce its nine ordinary source rows",
        ));
    }

    let limits = ProbeCampaignLimits::default();
    let zero_shift =
        IntegralShift::try_new(std::iter::repeat_n(0, generator.context().index_count())).map_err(
            |error| CachedSetupFailure::new(FoundryCampaignSetupStage::ZeroTranslation, error),
        )?;
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [zero_shift],
            limits.replay.scheduler.source_discovery.translation,
        )
        .map_err(|error| {
            CachedSetupFailure::new(FoundryCampaignSetupStage::ZeroTranslation, error)
        })?;
    OrdinarySourceIncidenceIndex::try_new(&zero_sources, limits.replay.scheduler.source_discovery)
        .map_err(|error| {
            CachedSetupFailure::new(FoundryCampaignSetupStage::IncidenceIndex, error)
        })?;
    Ok(K6AlgebraInputs {
        generator,
        completed,
        zero_sources,
    })
}

fn build_k6_root_predecessor() -> Result<ImmutableOwnerSnapshot, CachedSetupFailure> {
    let authority = derive_k6_terminal_authority().map_err(|error| {
        CachedSetupFailure::new(FoundryCampaignSetupStage::TerminalAuthority, error)
    })?;
    let predecessor =
        ImmutableOwnerSnapshot::try_from_terminal_authority(authority, Default::default())
            .map_err(|error| {
                CachedSetupFailure::new(FoundryCampaignSetupStage::PredecessorSnapshot, error)
            })?;
    if predecessor.closed_layer_count() != 0 {
        return Err(CachedSetupFailure::invariant(
            FoundryCampaignSetupStage::PredecessorSnapshot,
            "K6 diagnostic predecessor unexpectedly owns an ordinary-rule layer",
        ));
    }
    Ok(predecessor)
}
