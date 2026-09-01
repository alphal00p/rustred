use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::sector::SectorInteriorDomain;

/// One installer-compiled product embedding.
///
/// `raw_parent_master` is obtained by injecting one typed master from every
/// dependency into its disjoint parent slots. `parent_terminal` is its exact
/// canonical representative in the parent artifact. The finite table is
/// generated and authenticated once when the artifact is sealed.
#[derive(Debug)]
pub struct FactorizationMasterEmbedding {
    raw_parent_master: IntegralKey,
    parent_terminal: IntegralKey,
}

impl FactorizationMasterEmbedding {
    pub fn raw_parent_master(&self) -> &IntegralKey {
        &self.raw_parent_master
    }

    pub fn parent_terminal(&self) -> &IntegralKey {
        &self.parent_terminal
    }

    pub(super) fn new(raw_parent_master: IntegralKey, parent_terminal: IntegralKey) -> Self {
        Self {
            raw_parent_master,
            parent_terminal,
        }
    }
}

/// One lower-family integral entering an exact factorization identity.
///
/// Parent powers are projected in the declared order into the immutable
/// dependency artifact. Every typed master returned by that dependency is
/// embedded back into the same parent positions; product expansion is not
/// restricted to a distinguished lower-family master.
#[derive(Debug)]
pub struct FactorizationFactor {
    pub(super) dependency_ordinal: usize,
    pub(super) parent_positions: Box<[usize]>,
    pub(super) transformed_loop_positions: Box<[usize]>,
}

impl FactorizationFactor {
    pub fn dependency_ordinal(&self) -> usize {
        self.dependency_ordinal
    }

    pub fn parent_positions(&self) -> &[usize] {
        &self.parent_positions
    }

    /// Loop coordinates of the certified transformed parent basis owned by
    /// this independent lower-family factor.
    pub fn transformed_loop_positions(&self) -> &[usize] {
        &self.transformed_loop_positions
    }

    pub(super) fn new(
        dependency_ordinal: usize,
        parent_positions: impl IntoIterator<Item = usize>,
        transformed_loop_positions: impl IntoIterator<Item = usize>,
    ) -> Self {
        Self {
            dependency_ordinal,
            parent_positions: parent_positions.into_iter().collect(),
            transformed_loop_positions: transformed_loop_positions.into_iter().collect(),
        }
    }
}

/// Integer loop-basis change `q = U k` whose determinant is replayed as
/// `+1` or `-1` through Symbolica before a factorization can be sealed.
#[derive(Debug)]
pub struct UnimodularLoopBasis {
    dimension: usize,
    row_major: Box<[i64]>,
}

impl UnimodularLoopBasis {
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    pub fn row_major(&self) -> &[i64] {
        &self.row_major
    }

    pub(super) fn new(dimension: usize, row_major: impl IntoIterator<Item = i64>) -> Self {
        Self {
            dimension,
            row_major: row_major.into_iter().collect(),
        }
    }
}

/// Exact product factorization on one rectangular parent-family cell.
///
/// The application domain is restricted to a sector-corner product: inactive
/// coordinates are fixed at zero and active coordinates start at one.  The
/// returned lower-family masters are embedded into the parent coordinates and
/// canonicalized to explicit parent terminals. Consequently a factorized
/// lower family may itself have multiple masters within the explicit product
/// cardinality limits.
#[derive(Debug)]
pub struct FactorizationRule {
    pub(super) application_domain: SectorInteriorDomain,
    pub(super) factors: Box<[FactorizationFactor]>,
    pub(super) master_embeddings: Box<[FactorizationMasterEmbedding]>,
    pub(super) normalization: Coefficient,
    pub(super) loop_basis: UnimodularLoopBasis,
    /// Cold-installation binding. This is derived state, not durable schema.
    pub(super) installed_family_fingerprint: Option<Arc<String>>,
}

impl FactorizationRule {
    pub fn application_domain(&self) -> &SectorInteriorDomain {
        &self.application_domain
    }

    pub fn factors(&self) -> &[FactorizationFactor] {
        &self.factors
    }

    /// Complete, raw-key-sorted Cartesian dependency-master embedding.
    pub fn master_embeddings(&self) -> &[FactorizationMasterEmbedding] {
        &self.master_embeddings
    }

    pub fn normalization(&self) -> &Coefficient {
        &self.normalization
    }

    pub fn loop_basis(&self) -> &UnimodularLoopBasis {
        &self.loop_basis
    }

    pub(super) fn new(
        application_domain: SectorInteriorDomain,
        factors: impl IntoIterator<Item = FactorizationFactor>,
        normalization: Coefficient,
        loop_basis: UnimodularLoopBasis,
    ) -> Self {
        Self {
            application_domain,
            factors: factors.into_iter().collect(),
            master_embeddings: Box::new([]),
            normalization,
            loop_basis,
            installed_family_fingerprint: None,
        }
    }

    pub(super) fn install_master_embeddings(
        &mut self,
        embeddings: impl IntoIterator<Item = FactorizationMasterEmbedding>,
        family_fingerprint: Arc<String>,
    ) {
        self.master_embeddings = embeddings.into_iter().collect();
        self.installed_family_fingerprint = Some(family_fingerprint);
    }

    /// Family identity stamped only by the exact artifact installer.
    pub(crate) fn installed_family_fingerprint(&self) -> Option<&str> {
        self.installed_family_fingerprint
            .as_ref()
            .map(|fingerprint| fingerprint.as_str())
    }

    pub(crate) fn parent_terminal_for(&self, raw: &IntegralKey) -> Option<&IntegralKey> {
        self.master_embeddings
            .binary_search_by(|embedding| embedding.raw_parent_master().cmp(raw))
            .ok()
            .map(|ordinal| self.master_embeddings[ordinal].parent_terminal())
    }
}
