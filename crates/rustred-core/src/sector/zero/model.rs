use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::sector::{Exclusion, Mask};

use super::domain::Domain;

/// Exact sufficient proof that one raw sector is zero.
///
/// The primitive witness has already been replayed through Symbolica's native
/// integer matrix product before a certificate can be constructed.
#[derive(Debug, PartialEq, Eq)]
pub struct Certificate {
    pub(super) family_fingerprint: Arc<str>,
    pub(super) raw_sector: Mask,
    pub(super) effective_sector: Mask,
    pub(super) active_parameter_order: Box<[usize]>,
    pub(super) primitive_kernel: Box<[Integer]>,
    pub(super) rank: usize,
    pub(super) exponent_row_count: usize,
    pub(super) domain: Arc<Domain>,
}

impl Certificate {
    /// Fingerprint of the authenticated family used for this decision.
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Raw unshifted sector requested by the caller.
    pub fn raw_sector(&self) -> &Mask {
        &self.raw_sector
    }

    /// Effective support after formal nonintegral power shifts are included.
    pub fn effective_sector(&self) -> &Mask {
        &self.effective_sector
    }

    /// Family-parameter indices corresponding to kernel coordinates before
    /// the final homogenizing coordinate.
    pub fn active_parameter_order(&self) -> &[usize] {
        &self.active_parameter_order
    }

    /// Sign-oriented primitive integer right-kernel witness.
    pub fn primitive_kernel(&self) -> &[Integer] {
        &self.primitive_kernel
    }

    /// Exact rational rank of the exponent matrix.
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// Number of retained exponent rows.
    pub fn exponent_row_count(&self) -> usize {
        self.exponent_row_count
    }

    /// Generic coefficient locus required by the analysis.
    pub fn domain(&self) -> &Domain {
        &self.domain
    }
}

/// Diagnostic full-column-rank result. It means only that this sufficient
/// zero test was inconclusive; it is not a nonzero proof.
#[derive(Debug, PartialEq, Eq)]
pub struct FullColumnRank {
    pub(super) raw_sector: Mask,
    pub(super) effective_sector: Mask,
    pub(super) active_parameter_order: Box<[usize]>,
    pub(super) rank: usize,
    pub(super) exponent_row_count: usize,
    pub(super) column_count: usize,
}

impl FullColumnRank {
    /// Raw unshifted sector requested by the caller.
    pub fn raw_sector(&self) -> &Mask {
        &self.raw_sector
    }

    /// Effective support after formal nonintegral power shifts are included.
    pub fn effective_sector(&self) -> &Mask {
        &self.effective_sector
    }

    /// Family-parameter indices represented by non-homogenizing columns.
    pub fn active_parameter_order(&self) -> &[usize] {
        &self.active_parameter_order
    }

    /// Exact rational rank of the exponent matrix.
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// Number of retained exponent rows.
    pub fn exponent_row_count(&self) -> usize {
        self.exponent_row_count
    }

    /// Number of exponent-matrix columns, including homogenization.
    pub fn column_count(&self) -> usize {
        self.column_count
    }
}

/// Complete successful classification of one raw sector.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// The sector violates authenticated cuts or a sector pattern.
    Excluded(Exclusion),
    /// The exact rank criterion supplied a zero proof.
    ProvedZero(Certificate),
    /// The sufficient rank criterion did not prove the sector zero.
    Inconclusive(FullColumnRank),
}
