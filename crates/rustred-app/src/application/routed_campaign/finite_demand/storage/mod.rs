//! Integer-only storage for explicitly trusted-generated finite checkpoints.
//! Integrity/shape validation is NOT independent closure or IBP verification.
//! This layer does not yet mint a native completed-baseline capability.
mod reader;
#[cfg(test)]
mod tests;
mod wire;
mod writer;

pub(crate) use reader::{Lookup, ReaderStatistics, WorkReader};
use std::{fmt, path::PathBuf, sync::Arc};
pub(crate) use writer::{CommitOutcome, StoreWriter};

type Digest = [u8; 32];
type Result<T> = std::result::Result<T, StorageError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ErrorKind {
    Input,
    Corrupt,
    Io,
    Resource,
    Locked,
    Stale,
    Cancelled,
    Indeterminate,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StorageError {
    pub kind: ErrorKind,
    detail: String,
}
impl StorageError {
    fn new(kind: ErrorKind, detail: impl ToString) -> Self {
        Self {
            kind,
            detail: detail.to_string(),
        }
    }
}
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}
impl std::error::Error for StorageError {}
fn corrupt(detail: impl ToString) -> StorageError {
    StorageError::new(ErrorKind::Corrupt, detail)
}
fn resource(detail: impl ToString) -> StorageError {
    StorageError::new(ErrorKind::Resource, detail)
}
fn io(detail: impl ToString) -> StorageError {
    StorageError::new(ErrorKind::Io, detail)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StorageLimits {
    pub max_arity: usize,
    pub max_delta_records: usize,
    pub max_delta_payload_bytes: usize,
    pub max_page_records: usize,
    pub max_page_bytes: usize,
    pub max_cached_pages: usize,
    pub max_cached_page_bytes: usize,
    pub max_directory_bytes: usize,
    pub max_manifest_bytes: usize,
    pub max_state_bytes: usize,
    pub max_scope_rows: usize,
    pub max_census_records: usize,
    pub max_runs: usize,
    pub max_pages: usize,
    /// Retained and temporary owned pathnames, including atomic staging links.
    pub max_physical_files: usize,
    pub max_disk_bytes: u64,
    pub max_lookup_pages: usize,
    pub max_lookup_bytes: u64,
    /// Encoded input plus the bounded native bincode decoder allowance.
    /// This is separate from a possibly zero resident-page-cache grant.
    pub max_transient_bytes: usize,
}
impl StorageLimits {
    fn validate(self) -> Result<()> {
        if self.max_arity == 0
            || self.max_page_records == 0
            || self.max_page_bytes == 0
            || self.max_manifest_bytes == 0
            || self.max_state_bytes == 0
            || self.max_page_bytes > wire::MAX_FRAME
            || self.max_manifest_bytes > wire::MAX_FRAME
            || self.max_state_bytes > wire::MAX_FRAME
        {
            return Err(resource("unsupported finite-store framing policy"));
        }
        let required = wire::MAX_FRAME
            .checked_add(
                self.max_page_bytes
                    .max(self.max_manifest_bytes)
                    .max(self.max_state_bytes),
            )
            .ok_or_else(|| resource("transient decode allowance overflow"))?;
        if self.max_transient_bytes < required {
            return Err(resource("finite-store transient decode allowance"));
        }
        Ok(())
    }
}

/// Caller computes this from the immutable family/source conditions, ordered
/// owner/overlay/route programs and native dispatch/raw-terminal convention.
/// Downstream refinement settings are deliberately not part of this identity.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct RawExecutionBinding {
    pub physical_arity: u32,
    pub program_identity: Digest,
}

/// This wire order is identical to CandidateRoutedWork: Route before Apply,
/// then the exact literal owner (Apply), then lexicographic native powers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, bincode::Encode, bincode::Decode)]
pub(crate) enum StoredWork {
    Route(Vec<i64>),
    Apply { owner: Vec<bool>, powers: Vec<i64> },
}
impl StoredWork {
    pub fn from_native<const N: usize>(
        work: &rustred::solver::CandidateRoutedWork<N>,
    ) -> Result<Self> {
        use rustred::solver::CandidateRoutedWork;
        let arity = work.target().powers().len();
        if !rustred::fits_storage(arity, N) {
            return Err(corrupt("native Work has invalid storage arity"));
        }
        let value = match work {
            CandidateRoutedWork::Route(key) => Self::Route(key.powers().to_vec()),
            CandidateRoutedWork::Apply {
                owner_sector,
                target,
            } => {
                if !rustred::fits_storage(arity, N)
                    || owner_sector[arity..].iter().any(|&active| active)
                {
                    return Err(corrupt("nonzero or invalid stored owner padding"));
                }
                Self::Apply {
                    owner: owner_sector[..arity].to_vec(),
                    powers: target.powers().to_vec(),
                }
            }
        };
        value.validate(arity)?;
        Ok(value)
    }
    pub fn to_native<const N: usize>(&self) -> Result<rustred::solver::CandidateRoutedWork<N>> {
        use rustred::{family::IntegralKey, solver::CandidateRoutedWork};
        let arity = self.powers().len();
        self.validate(arity)?;
        if !rustred::fits_storage(arity, N) {
            return Err(corrupt("stored Work exceeds native storage arity"));
        }
        let key = IntegralKey::try_new(self.powers().iter().copied()).map_err(corrupt)?;
        Ok(match self {
            Self::Route(_) => CandidateRoutedWork::Route(key),
            Self::Apply { owner, .. } => CandidateRoutedWork::Apply {
                owner_sector: std::array::from_fn(|i| owner.get(i).copied().unwrap_or(false)),
                target: key,
            },
        })
    }
    pub fn powers(&self) -> &[i64] {
        match self {
            Self::Route(powers) | Self::Apply { powers, .. } => powers,
        }
    }
    fn validate(&self, arity: usize) -> Result<()> {
        if arity == 0 || self.powers().len() != arity {
            return Err(corrupt("stored Work arity"));
        }
        if let Self::Apply { owner, powers } = self {
            if owner.len() != arity
                || owner
                    .iter()
                    .zip(powers)
                    .any(|(&active, &power)| active != (power > 0))
            {
                return Err(corrupt("stored Apply owner differs from physical support"));
            }
        }
        Ok(())
    }
    fn heap_bytes(&self) -> Result<usize> {
        let (powers, owner) = match self {
            Self::Route(powers) => (powers, 0),
            Self::Apply { owner, powers } => (powers, owner.capacity()),
        };
        powers
            .capacity()
            .checked_mul(std::mem::size_of::<i64>())
            .and_then(|n| n.checked_add(owner))
            .ok_or_else(|| resource("stored key payload overflow"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) enum ScopeRole {
    Required,
    Auxiliary,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct ScopeRow {
    pub origin: String,
    pub role: ScopeRole,
    pub exact_domain: Digest,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct ScopeBinding {
    pub original_source: Digest,
    pub rows: Vec<ScopeRow>,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct CursorRecord {
    pub source_policy: Digest,
    pub completed_windows: u64,
    pub input_occurrences: u64,
    pub encoded_cursor: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct SelectionRecord {
    pub owner: Vec<bool>,
    pub batch: u64,
    pub baseline: u64,
    pub selected: u64,
    pub multiplicity: u64,
}
impl SelectionRecord {
    fn identity(&self) -> (&[bool], u64, u64, u64) {
        (&self.owner, self.batch, self.baseline, self.selected)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct RawCensus {
    pub terminals: Vec<Vec<i64>>,
    pub zeros: Vec<Vec<i64>>,
    pub selections: Vec<SelectionRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct GenerationState {
    pub scope: ScopeBinding,
    /// Same order and length as scope.rows; opaque bytes require native prefix
    /// validation before restoration. These counters do not certify closure.
    pub cursors: Vec<CursorRecord>,
    pub completed_work: u64,
    pub input_occurrences: u64,
    pub census: RawCensus,
}

/// Captured immutable generation, validated as TRUSTED-GENERATED state.
/// Private fields prevent arbitrary byte/hash constructors at consumer seams.
/// It is not a proof against someone rewriting a complete valid checkpoint.
#[derive(Clone, Debug)]
pub(crate) struct ValidatedGeneration {
    directory: PathBuf,
    digest: Digest,
    manifest: Arc<wire::Manifest>,
    state: Arc<GenerationState>,
}
impl ValidatedGeneration {
    pub fn generation(&self) -> u64 {
        self.manifest.generation
    }
    pub fn digest(&self) -> Digest {
        self.digest
    }
    pub fn execution(&self) -> &RawExecutionBinding {
        &self.manifest.execution
    }
    pub fn state(&self) -> &GenerationState {
        &self.state
    }
    pub fn cursor(&self, origin: usize) -> Result<ValidatedCursorRecord<'_>> {
        let row = self
            .state
            .scope
            .rows
            .get(origin)
            .ok_or_else(|| corrupt("cursor origin out of scope"))?;
        let cursor = self
            .state
            .cursors
            .get(origin)
            .ok_or_else(|| corrupt("cursor origin missing"))?;
        Ok(ValidatedCursorRecord {
            generation: self.digest,
            row,
            cursor,
        })
    }
    pub fn reader(&self, limits: StorageLimits) -> Result<WorkReader> {
        WorkReader::new(self.clone(), limits)
    }
}

/// Integrity/structural provenance from a captured trusted-generated commit.
/// Fast prefix restore must still validate current shape/source/policy. It may
/// trust prior committed transitions; deep audit proves them by replay instead.
pub(crate) struct ValidatedCursorRecord<'a> {
    generation: Digest,
    row: &'a ScopeRow,
    cursor: &'a CursorRecord,
}
impl ValidatedCursorRecord<'_> {
    pub fn generation(&self) -> Digest {
        self.generation
    }
    pub fn row(&self) -> &ScopeRow {
        self.row
    }
    pub fn source_policy(&self) -> Digest {
        self.cursor.source_policy
    }
    pub fn encoded_cursor(&self) -> &[u8] {
        &self.cursor.encoded_cursor
    }
    pub fn completed_windows(&self) -> u64 {
        self.cursor.completed_windows
    }
    pub fn input_occurrences(&self) -> u64 {
        self.cursor.input_occurrences
    }
}
