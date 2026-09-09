use super::super::SpiredCoordinateCaseObligation;

/// Deterministic work census for one exact guard-case materialization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredCoordinateGuardCaseCensus {
    pub(super) required_predicates: usize,
    pub(super) guard_ordinal_references: usize,
    pub(super) resolved_required_predicates: usize,
    pub(super) exact_hyperplanes: usize,
    pub(super) exact_duplicate_cases: usize,
    pub(super) output_cases: usize,
    pub(super) output_coordinate_cells: usize,
    pub(super) output_identity_bytes: usize,
}

impl SpiredCoordinateGuardCaseCensus {
    pub(crate) const fn required_predicates(self) -> usize {
        self.required_predicates
    }

    pub(crate) const fn guard_ordinal_references(self) -> usize {
        self.guard_ordinal_references
    }

    pub(crate) const fn resolved_required_predicates(self) -> usize {
        self.resolved_required_predicates
    }

    pub(crate) const fn exact_hyperplanes(self) -> usize {
        self.exact_hyperplanes
    }

    pub(crate) const fn exact_duplicate_cases(self) -> usize {
        self.exact_duplicate_cases
    }

    pub(crate) const fn output_cases(self) -> usize {
        self.output_cases
    }

    pub(crate) const fn output_coordinate_cells(self) -> usize {
        self.output_coordinate_cells
    }

    pub(crate) const fn output_identity_bytes(self) -> usize {
        self.output_identity_bytes
    }
}

/// Exact proposal-only, equality-only coordinate discovery cases. Cases from
/// different guard-zero equations may overlap. Empty output proves only that
/// this adapter found no in-domain exceptional face; it is not closure
/// authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredCoordinateGuardCases {
    cases: Box<[SpiredCoordinateCaseObligation]>,
    census: SpiredCoordinateGuardCaseCensus,
}

impl SpiredCoordinateGuardCases {
    pub(crate) fn cases(&self) -> &[SpiredCoordinateCaseObligation] {
        &self.cases
    }

    pub(crate) const fn census(&self) -> SpiredCoordinateGuardCaseCensus {
        self.census
    }

    pub(super) fn from_parts(
        cases: Vec<SpiredCoordinateCaseObligation>,
        census: SpiredCoordinateGuardCaseCensus,
    ) -> Self {
        Self {
            cases: cases.into_boxed_slice(),
            census,
        }
    }
}

/// A normal fail-closed stop. No cases produced before this stop survive it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredCoordinateGuardCaseIncomplete {
    ConservativeCoordinateCover {
        required_predicate_ordinal: usize,
        hyperplanes: usize,
    },
    UnsupportedCoupledOrNonAffineGeometry {
        required_predicate_ordinal: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
}

/// Complete exact proposal set or a typed fail-closed stop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredCoordinateGuardCaseOutcome {
    Exact(SpiredCoordinateGuardCases),
    /// The proposed rule requires a nonzero guard which is identically zero
    /// on the parent equality case.  This rejects only that candidate; the
    /// fair translated-source search must continue on the same case.
    CandidateUnusable(SpiredCoordinateGuardCaseRejection),
    Incomplete(SpiredCoordinateGuardCaseIncomplete),
}

/// Exact reason why one replayed rule cannot own any part of its parent case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredCoordinateGuardCaseRejection {
    GuardIdenticallyZero { required_predicate_ordinal: usize },
}
