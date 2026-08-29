use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::family::IntegralFamily;
use crate::sector::{Exclusion, Mask};

use super::analysis::ZeroSectorAnalyzer;
use super::domain::ZeroSectorDomain;
use super::error::ZeroSectorError;
use super::limits::{PowerShiftPolicy, ZeroSectorLimits};

pub const ZERO_SECTOR_CERTIFICATE_SCHEMA: &str = "rustred.zero-sector-certificate.v1";

/// Bounded work that prevented one effective-mask decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZeroSectorResource {
    pub(super) resource: &'static str,
    pub(super) requested: usize,
    pub(super) limit: usize,
}

impl ZeroSectorResource {
    pub fn resource(&self) -> &'static str {
        self.resource
    }

    pub fn requested(&self) -> usize {
        self.requested
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

/// Replayable sufficient proof that one raw sector is zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZeroSectorCertificate {
    pub(super) schema: &'static str,
    pub(super) family_fingerprint: Arc<str>,
    pub(super) g_fingerprint: Arc<str>,
    pub(super) raw_sector: Mask,
    pub(super) effective_sector: Mask,
    pub(super) active_parameter_order: Box<[usize]>,
    pub(super) primitive_kernel: Box<[Integer]>,
    pub(super) rank: usize,
    pub(super) exponent_row_count: usize,
    pub(super) domain: ZeroSectorDomain,
    pub(super) policy: PowerShiftPolicy,
}

impl ZeroSectorCertificate {
    pub fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn g_fingerprint(&self) -> &str {
        &self.g_fingerprint
    }

    pub fn raw_sector(&self) -> &Mask {
        &self.raw_sector
    }

    pub fn effective_sector(&self) -> &Mask {
        &self.effective_sector
    }

    pub fn active_parameter_order(&self) -> &[usize] {
        &self.active_parameter_order
    }

    pub fn primitive_kernel(&self) -> &[Integer] {
        &self.primitive_kernel
    }

    pub fn rank(&self) -> usize {
        self.rank
    }

    pub fn exponent_row_count(&self) -> usize {
        self.exponent_row_count
    }

    pub fn domain(&self) -> &ZeroSectorDomain {
        &self.domain
    }

    pub fn policy(&self) -> PowerShiftPolicy {
        self.policy
    }

    /// Reconstruct `G` and replay this certificate with default limits.
    pub fn replay(&self, family: &IntegralFamily) -> Result<(), ZeroSectorError> {
        self.replay_with_limits(family, ZeroSectorLimits::default())
    }

    /// Reconstruct `G` and replay this certificate with explicit limits.
    pub fn replay_with_limits(
        &self,
        family: &IntegralFamily,
        limits: ZeroSectorLimits,
    ) -> Result<(), ZeroSectorError> {
        catch_unwind(AssertUnwindSafe(|| {
            let analyzer = ZeroSectorAnalyzer::build_unrestricted(family, self.policy, limits)?;
            analyzer.replay_certificate_inner(self)
        }))
        .map_err(|_| ZeroSectorError::SymbolicaPanic)?
    }
}

/// Diagnostic full-column-rank result. It means only that this sufficient
/// zero test did not produce a certificate; it is not a nonzero proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullColumnRankWitness {
    pub(super) raw_sector: Mask,
    pub(super) effective_sector: Mask,
    pub(super) active_parameter_order: Box<[usize]>,
    pub(super) rank: usize,
    pub(super) exponent_row_count: usize,
    pub(super) column_count: usize,
}

impl FullColumnRankWitness {
    pub fn raw_sector(&self) -> &Mask {
        &self.raw_sector
    }

    pub fn effective_sector(&self) -> &Mask {
        &self.effective_sector
    }

    pub fn active_parameter_order(&self) -> &[usize] {
        &self.active_parameter_order
    }

    pub fn rank(&self) -> usize {
        self.rank
    }

    pub fn exponent_row_count(&self) -> usize {
        self.exponent_row_count
    }

    pub fn column_count(&self) -> usize {
        self.column_count
    }
}

/// Complete semantics for one raw sector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZeroSectorDecision {
    Excluded(Exclusion),
    ProvedZero(ZeroSectorCertificate),
    NoZeroCertificate(FullColumnRankWitness),
    ResourceLimited(ZeroSectorResource),
    Failed(ZeroSectorError),
}
