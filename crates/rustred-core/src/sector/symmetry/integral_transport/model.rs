use std::sync::Arc;

use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::Mask;
use crate::sector::symmetry::VerifiedMap;

use super::Endpoint;

/// Shared immutable owner for a restricted, exactly verified family map.
///
/// Construction is only through [`super::compile`]. A bare [`IntegralKey`]
/// carries no family identity; the caller must supply keys of this source
/// family. No sector search, recursive rule application or rank cutoff lives
/// in this object.
#[derive(Debug)]
pub struct Prepared {
    pub(super) source_fingerprint: Arc<String>,
    pub(super) target: Arc<IntegralFamily>,
    pub(super) map: Arc<VerifiedMap>,
    pub(super) source_root: Mask,
    pub(super) target_root: Mask,
    pub(super) active_target: Box<[Option<usize>]>,
}

impl Prepared {
    pub fn source_family_fingerprint(&self) -> &str {
        &self.source_fingerprint
    }
    pub fn target_family(&self) -> &IntegralFamily {
        &self.target
    }
    pub fn verified_map(&self) -> &VerifiedMap {
        &self.map
    }
    pub fn source_root(&self) -> &Mask {
        &self.source_root
    }
    pub fn target_root(&self) -> &Mask {
        &self.target_root
    }
    /// The already admitted unit active-row bijection, indexed by source axis.
    /// Inactive numerator rows are affine and deliberately have no such target.
    pub(crate) fn active_target_axes(&self) -> &[Option<usize>] {
        &self.active_target
    }
}

/// Exact finite combination in the prepared target family's coefficient context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportedIntegral {
    pub(super) source: IntegralKey,
    pub(super) source_fingerprint: Arc<String>,
    pub(super) target_fingerprint: Arc<String>,
    pub(super) terms: Box<[Endpoint]>,
}

impl TransportedIntegral {
    pub fn source(&self) -> &IntegralKey {
        &self.source
    }
    pub fn source_family_fingerprint(&self) -> &str {
        &self.source_fingerprint
    }
    pub fn target_family_fingerprint(&self) -> &str {
        &self.target_fingerprint
    }
    pub fn terms(&self) -> &[Endpoint] {
        &self.terms
    }
}
