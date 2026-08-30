use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::sector::SectorInteriorDomain;

/// One lower-family integral entering an exact factorization identity.
///
/// Parent powers are projected in the declared order into the immutable
/// dependency artifact.  The dependency master is typed explicitly so a
/// multi-master lower family can never be collapsed accidentally.
#[derive(Debug)]
pub struct FactorizationFactor {
    pub(super) dependency_ordinal: usize,
    pub(super) parent_positions: Box<[usize]>,
    pub(super) dependency_master: IntegralKey,
    pub(super) transformed_loop_positions: Box<[usize]>,
}

impl FactorizationFactor {
    pub fn dependency_ordinal(&self) -> usize {
        self.dependency_ordinal
    }

    pub fn parent_positions(&self) -> &[usize] {
        &self.parent_positions
    }

    pub fn dependency_master(&self) -> &IntegralKey {
        &self.dependency_master
    }

    /// Loop coordinates of the certified transformed parent basis owned by
    /// this independent lower-family factor.
    pub fn transformed_loop_positions(&self) -> &[usize] {
        &self.transformed_loop_positions
    }

    pub(super) fn new(
        dependency_ordinal: usize,
        parent_positions: impl IntoIterator<Item = usize>,
        dependency_master: IntegralKey,
        transformed_loop_positions: impl IntoIterator<Item = usize>,
    ) -> Self {
        Self {
            dependency_ordinal,
            parent_positions: parent_positions.into_iter().collect(),
            dependency_master,
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
/// explicit parent master is that sector corner.  Consequently every
/// nonterminal application descends strictly to the master, while the master
/// itself is intercepted by the artifact terminal before rule selection.
#[derive(Debug)]
pub struct FactorizationRule {
    pub(super) application_domain: SectorInteriorDomain,
    pub(super) factors: Box<[FactorizationFactor]>,
    pub(super) parent_master: IntegralKey,
    pub(super) normalization: Coefficient,
    pub(super) loop_basis: UnimodularLoopBasis,
}

impl FactorizationRule {
    pub fn application_domain(&self) -> &SectorInteriorDomain {
        &self.application_domain
    }

    pub fn factors(&self) -> &[FactorizationFactor] {
        &self.factors
    }

    pub fn parent_master(&self) -> &IntegralKey {
        &self.parent_master
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
        parent_master: IntegralKey,
        normalization: Coefficient,
        loop_basis: UnimodularLoopBasis,
    ) -> Self {
        Self {
            application_domain,
            factors: factors.into_iter().collect(),
            parent_master,
            normalization,
            loop_basis,
        }
    }
}
