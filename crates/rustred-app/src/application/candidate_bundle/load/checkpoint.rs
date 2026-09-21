//! Complete trusted-local checkpoint consumption without monolithic encoding.

use rustred::family::IntegralFamily;
use rustred::identity::ParametricIbpGenerator;
use rustred::persistence::SectionTag;
use rustred::reduction::ReductionLimits;
use rustred::solver::CandidateReducer;

use crate::application::AppError;

use super::super::{
    CandidateBundleLimits, FamilyCandidatesRequest,
    checkpoint::{CheckpointManifest, CheckpointStore},
    codec, preparation,
};

/// Load every expected sector of an existing trusted-local generation campaign
/// into one experimental concrete reducer, without solving or re-encoding.
///
/// Requires `request.checkpoint` with explicit `resume = true`, an installed
/// manifest and cooperative lock file, and the complete expected set of stored
/// sector records.
/// Holds that lock while reading; neither shards, manifest nor lock are created
/// or modified. Missing/invalid sectors fail rather than being regenerated.
/// Request source, root, ordering, rank, finite-case policy, backend and recipe
/// must match. Worker count is unused: no sector executor is constructed.
///
/// `request.bundle_limits` bounds each shard (at most 1 GiB), cumulative program
/// collection entries, cumulative coefficient-table entries and cumulative
/// coefficient-table bytes, counting repeated local dictionaries conservatively.
/// All those cumulative checks precede any native shard import. Native state
/// bytes remain individually bounded by the existing binary codec and globally
/// by `checkpoint.max_total_bytes`, which also counts all directory payloads.
/// These bounds are not peak-RSS guarantees: the existing reducer retains all
/// rules and explicit terminals in memory. There is no large monolithic buffer.
///
/// As for [`super::load_generated_candidate_bundle`], native Symbolica payloads
/// require trusted matching-stack provenance. This is storage/structure
/// admission, not source replay, recursive closure or master independence.
/// Above-entry-rank successors remain internal to the same reducer and are
/// never clipped or re-admitted as independent public entry targets.
pub fn load_generated_candidate_checkpoint<const N: usize>(
    request: &FamilyCandidatesRequest,
    reduction_limits: ReductionLimits,
) -> Result<(IntegralFamily, CandidateReducer<N>), AppError> {
    let options = request.checkpoint.as_ref().ok_or_else(|| {
        AppError::input("checkpoint loading requires an existing resume checkpoint")
    })?;
    if !options.resume {
        return Err(AppError::input(
            "checkpoint loading requires explicit resume",
        ));
    }
    if !(1..=16).contains(&N) {
        return Err(AppError::input("checkpoint/reducer arity mismatch"));
    }
    let family = preparation::family(&request.source, request.input_format)?;
    if family.denominator_count() != N {
        return Err(AppError::input("checkpoint/reducer arity mismatch"));
    }
    let root = preparation::root(N, &request.nonpositive_indices)?;
    let prepared = preparation::prepare::<N>(family, &root, request.permutation.as_deref())?;
    let manifest = CheckpointManifest::for_request(
        request,
        prepared.family.fingerprint(),
        &root,
        prepared
            .sectors
            .iter()
            .map(|sector| sector.to_vec())
            .collect(),
    )?;
    let limits = request.bundle_limits;
    let store = CheckpointStore::open_existing(options, manifest, limits)?;
    let missing = store.pending()?;
    if !missing.is_empty() {
        return Err(AppError::input(format!(
            "complete checkpoint loading requires every sector; {} missing, first ordinal {}",
            missing.len(),
            missing[0].0,
        )));
    }
    let receipts = store.receipts()?;
    if receipts.len() != prepared.sectors.len()
        || receipts
            .iter()
            .enumerate()
            .any(|(ordinal, receipt)| receipt.ordinal != ordinal)
    {
        return Err(AppError::internal_invariant(
            "complete checkpoint ordinal gap",
        ));
    }

    // Preflight the WHOLE campaign before importing its first native shard.
    // Only one structural shard exists at a time, and no coefficient table or
    // final program is constructed merely to enforce aggregate ingress limits.
    let mut budget = IngressBudget::new(limits, receipts.len())?;
    for receipt in &receipts {
        let bytes = store.read(receipt.ordinal)?;
        budget.admit(&bytes)?;
    }

    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .map_err(|error| AppError::execution(error.to_string()))?
        .context()
        .clone();
    let mut solutions = Vec::with_capacity(receipts.len());
    // Repeat cumulative admission on the actual imported bytes under the held
    // cooperative lock. Native frames and IDs retain the existing codec checks.
    let mut budget = IngressBudget::new(limits, receipts.len())?;
    for receipt in receipts {
        let bytes = store.read(receipt.ordinal)?;
        budget.admit(&bytes)?;
        let bundle = codec::read(&bytes, limits)?;
        let family = super::reconstruct_family::<N>(&bundle, limits)?;
        if family.fingerprint() != prepared.family.fingerprint() {
            return Err(AppError::input(
                "checkpoint native family differs from the prepared family",
            ));
        }
        let mut decoded = codec::solutions::<N>(
            &bundle,
            &context,
            prepared.sources.index_variables(),
            limits,
        )?;
        if decoded.len() != 1 || decoded[0].0 != prepared.sectors[receipt.ordinal] {
            return Err(AppError::input(
                "checkpoint reconstructed sector differs from manifest",
            ));
        }
        solutions.push(decoded.pop().expect("checked single sector"));
    }
    let ordering = super::candidate_ordering(N, request.permutation.as_deref())?;
    super::finish_reducer(
        prepared,
        solutions,
        ordering,
        request.max_numerator_rank,
        reduction_limits,
    )
}

struct IngressBudget {
    limits: CandidateBundleLimits,
    collections: codec::CollectionBudget,
    coefficient_entries: usize,
    coefficient_bytes: usize,
}

impl IngressBudget {
    fn new(limits: CandidateBundleLimits, sectors: usize) -> Result<Self, AppError> {
        Ok(Self {
            limits,
            collections: codec::CollectionBudget::new(limits, sectors)?,
            coefficient_entries: 0,
            coefficient_bytes: 0,
        })
    }

    fn admit(&mut self, bytes: &[u8]) -> Result<(), AppError> {
        let (envelope, record, _) = codec::read_structure(bytes, self.limits)?;
        for sector in &record.sectors {
            self.collections.admit_sector(sector)?;
        }
        let table = envelope
            .section(SectionTag::COEFFICIENTS)
            .expect("checked section");
        let count_bytes = table
            .get(..8)
            .ok_or_else(|| AppError::schema("truncated coefficient count"))?;
        let count = usize::try_from(u64::from_le_bytes(count_bytes.try_into().unwrap()))
            .map_err(|_| AppError::limit("coefficient count exceeds host width"))?;
        charge(
            &mut self.coefficient_entries,
            count,
            self.limits.max_collection_entries,
            "checkpoint aggregate coefficient-entry budget exceeded",
        )?;
        charge(
            &mut self.coefficient_bytes,
            table.len(),
            self.limits.max_total_coefficient_bytes,
            "checkpoint aggregate coefficient-table byte budget exceeded",
        )
    }
}

fn charge(
    total: &mut usize,
    amount: usize,
    limit: usize,
    message: &'static str,
) -> Result<(), AppError> {
    let next = total
        .checked_add(amount)
        .ok_or_else(|| AppError::limit(message))?;
    if next > limit {
        return Err(AppError::limit(message));
    }
    *total = next;
    Ok(())
}
