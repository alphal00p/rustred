//! Checked resource metadata for campaign preflight and width planning.
//!
//! This module performs no algebra and creates no workers. Its memory values
//! are deterministic planning envelopes, not hard RSS guarantees for
//! Symbolica's opaque allocator.

use std::fmt;
use std::num::{NonZeroU64, NonZeroUsize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CampaignBytes(u64);

impl CampaignBytes {
    pub const ZERO: Self = Self(0);

    pub const fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    fn checked_add(self, other: Self) -> Result<Self, CampaignResourceError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(CampaignResourceError::ByteCountOverflow {
                operation: "memory addition",
            })
    }
}

impl fmt::Display for CampaignBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} bytes", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignMemoryEstimate {
    visible_logical: CampaignBytes,
    opaque_native_reserve: CampaignBytes,
    total: CampaignBytes,
}

impl CampaignMemoryEstimate {
    pub fn try_new(
        visible_logical: CampaignBytes,
        opaque_native_reserve: CampaignBytes,
    ) -> Result<Self, CampaignResourceError> {
        Ok(Self {
            visible_logical,
            opaque_native_reserve,
            total: visible_logical.checked_add(opaque_native_reserve)?,
        })
    }

    pub const fn zero() -> Self {
        Self {
            visible_logical: CampaignBytes::ZERO,
            opaque_native_reserve: CampaignBytes::ZERO,
            total: CampaignBytes::ZERO,
        }
    }

    pub const fn visible_logical(self) -> CampaignBytes {
        self.visible_logical
    }

    pub const fn opaque_native_reserve(self) -> CampaignBytes {
        self.opaque_native_reserve
    }

    pub const fn total(self) -> CampaignBytes {
        self.total
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignTaskMemoryEnvelope {
    retained_output: CampaignMemoryEstimate,
    transient_excluding_output: CampaignMemoryEstimate,
    peak_additional: CampaignBytes,
}

impl CampaignTaskMemoryEnvelope {
    /// Both components are incremental beyond the already-accounted campaign
    /// baseline snapshot. `retained_output` still coexists with the old base
    /// at peak and is therefore fully charged by execution-width preflight.
    pub fn try_new(
        retained_output: CampaignMemoryEstimate,
        transient_excluding_output: CampaignMemoryEstimate,
    ) -> Result<Self, CampaignResourceError> {
        Ok(Self {
            retained_output,
            transient_excluding_output,
            peak_additional: retained_output
                .total()
                .checked_add(transient_excluding_output.total())?,
        })
    }

    pub const fn retained_output(self) -> CampaignMemoryEstimate {
        self.retained_output
    }

    pub const fn transient_excluding_output(self) -> CampaignMemoryEstimate {
        self.transient_excluding_output
    }

    pub const fn peak_additional(self) -> CampaignBytes {
        self.peak_additional
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CampaignEstimatorRevision(NonZeroU64);

impl CampaignEstimatorRevision {
    pub fn try_new(revision: u64) -> Result<Self, CampaignResourceError> {
        Ok(Self(
            NonZeroU64::new(revision).ok_or(CampaignResourceError::ZeroEstimatorRevision)?,
        ))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignTaskResourceEstimate {
    estimator_revision: CampaignEstimatorRevision,
    cores: NonZeroUsize,
    memory: CampaignTaskMemoryEnvelope,
}

impl CampaignTaskResourceEstimate {
    pub fn try_new(
        estimator_revision: CampaignEstimatorRevision,
        cores: usize,
        memory: CampaignTaskMemoryEnvelope,
    ) -> Result<Self, CampaignResourceError> {
        Ok(Self {
            estimator_revision,
            cores: NonZeroUsize::new(cores).ok_or(CampaignResourceError::ZeroTaskCoreRequest)?,
            memory,
        })
    }

    pub const fn estimator_revision(self) -> CampaignEstimatorRevision {
        self.estimator_revision
    }

    pub const fn cores(self) -> usize {
        self.cores.get()
    }

    pub const fn memory(self) -> CampaignTaskMemoryEnvelope {
        self.memory
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignBaselineMemory {
    fixed_and_shared: CampaignBytes,
    hydrated_retained: CampaignBytes,
    staged_results: CampaignBytes,
    total: CampaignBytes,
}

impl CampaignBaselineMemory {
    pub fn try_new(
        fixed_and_shared: CampaignBytes,
        hydrated_retained: CampaignBytes,
        staged_results: CampaignBytes,
    ) -> Result<Self, CampaignResourceError> {
        let total = fixed_and_shared
            .checked_add(hydrated_retained)?
            .checked_add(staged_results)?;
        Ok(Self {
            fixed_and_shared,
            hydrated_retained,
            staged_results,
            total,
        })
    }

    pub const fn fixed_and_shared(self) -> CampaignBytes {
        self.fixed_and_shared
    }

    pub const fn hydrated_retained(self) -> CampaignBytes {
        self.hydrated_retained
    }

    pub const fn staged_results(self) -> CampaignBytes {
        self.staged_results
    }

    pub const fn total(self) -> CampaignBytes {
        self.total
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignResourceError {
    ZeroEstimatorRevision,
    ZeroTaskCoreRequest,
    ByteCountOverflow { operation: &'static str },
}

impl fmt::Display for CampaignResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroEstimatorRevision => {
                formatter.write_str("campaign estimator revision must be positive")
            }
            Self::ZeroTaskCoreRequest => {
                formatter.write_str("campaign task core request must be positive")
            }
            Self::ByteCountOverflow { operation } => {
                write!(formatter, "{operation} overflowed u64")
            }
        }
    }
}

impl std::error::Error for CampaignResourceError {}
