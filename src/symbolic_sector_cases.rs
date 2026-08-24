//! Replayable symbolic case partitions inside one integer sector orthant.
//!
//! LiteRed's `SolvejSector` maintains uncovered integer cases and its
//! `WhenBad` condition separates a rule's exceptional locus from the locus on
//! which the pivot is usable.  This module implements the first independent
//! proof layer needed for that workflow:
//!
//! - the unshifted sector convention is recorded exactly as `n_i >= 1` or
//!   `n_i <= 0` over the integer lattice;
//! - a case is a conjunction of authenticated `K(n)` polynomial predicates;
//! - every split replaces one live case by the complementary branches
//!   `p = 0` and `p != 0`, in that deterministic order; and
//! - a certificate replays the complete split transcript and reconstructs the
//!   exact finite leaf set.
//!
//! Disjointness and coverage are structural: at the first split where two
//! leaves diverge, one contains `p = 0` and the other `p != 0`; replacing a
//! parent by those two children preserves its union.  No Gröbner basis,
//! Presburger reduction, nonlinear Diophantine solver, contradiction pruning,
//! or identification of scalar-multiple polynomial loci is claimed here.
//! Consequently, a certified leaf may be empty as an integer set, and two
//! syntactically different polynomials may describe the same locus.

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;
use std::sync::Arc;

use crate::{
    ExactAlgebraLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricPolynomial, SectorMask,
};

/// Stable schema for the first structural symbolic sector-case proof.
pub const SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA: &str =
    "rustred-symbolic-sector-case-partition-v1";

/// Checked work and retained-proof budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicSectorCaseLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_indices: usize,
    pub max_context_fingerprint_bytes: usize,
    pub max_splits: usize,
    pub max_live_cases: usize,
    pub max_predicates_per_case: usize,
    pub max_total_leaf_predicates: usize,
    /// Sparse terms in immutable polynomial payloads retained by the split
    /// transcript. Leaf predicates share those payloads through `Arc`; their
    /// references are bounded separately by `max_total_leaf_predicates`.
    pub max_retained_polynomial_terms: usize,
    /// Aggregate canonical-display bytes of the immutable polynomial payloads
    /// retained by the split transcript. Leaf references do not duplicate this
    /// payload.
    pub max_retained_polynomial_bytes: usize,
    /// Bytes in the canonical, source-owned identity of the orthant and split
    /// transcript.  The identity is constructed once when the partition is
    /// frozen and then shared by `Arc` with downstream certificates.
    pub max_source_identity_bytes: usize,
}

impl Default for SymbolicSectorCaseLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_indices: 4_096,
            max_context_fingerprint_bytes: 1024 * 1024,
            max_splits: 65_536,
            max_live_cases: 65_537,
            max_predicates_per_case: 256,
            max_total_leaf_predicates: 4_000_000,
            max_retained_polynomial_terms: 16_000_000,
            max_retained_polynomial_bytes: 2 * 1024 * 1024 * 1024,
            max_source_identity_bytes: 1024 * 1024 * 1024,
        }
    }
}

/// Stable identifier allocated in split-transcript order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolicSectorCaseId(u64);

impl SymbolicSectorCaseId {
    pub const ROOT: Self = Self(0);

    pub fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SymbolicSectorCaseId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One exact integer inequality in denominator-index order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectorOrthantSide {
    AtLeastOne,
    AtMostZero,
}

/// Typed form of `n_i >= 1` or `n_i <= 0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectorOrthantConstraint {
    index: usize,
    side: SectorOrthantSide,
}

impl SectorOrthantConstraint {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn side(&self) -> SectorOrthantSide {
        self.side
    }

    pub fn accepts(&self, value: i64) -> bool {
        match self.side {
            SectorOrthantSide::AtLeastOne => value >= 1,
            SectorOrthantSide::AtMostZero => value <= 0,
        }
    }
}

/// The exact unshifted integer orthant attached to every leaf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicSectorOrthant {
    sector: SectorMask,
    constraints: Box<[SectorOrthantConstraint]>,
}

impl SymbolicSectorOrthant {
    fn from_sector(sector: SectorMask) -> Self {
        let constraints = sector
            .active_bits()
            .iter()
            .enumerate()
            .map(|(index, &active)| SectorOrthantConstraint {
                index,
                side: if active {
                    SectorOrthantSide::AtLeastOne
                } else {
                    SectorOrthantSide::AtMostZero
                },
            })
            .collect();
        Self {
            sector,
            constraints,
        }
    }

    pub fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub fn constraints(&self) -> &[SectorOrthantConstraint] {
        &self.constraints
    }

    pub fn contains_integer_point(&self, indices: &[i64]) -> Result<bool, SymbolicSectorCaseError> {
        if indices.len() != self.constraints.len() {
            return Err(SymbolicSectorCaseError::WrongIndexArity {
                expected: self.constraints.len(),
                actual: indices.len(),
            });
        }
        Ok(self
            .constraints
            .iter()
            .zip(indices)
            .all(|(constraint, &value)| constraint.accepts(value)))
    }
}

/// One side of an exact polynomial split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolicPolynomialPredicateKind {
    EqualZero,
    NonZero,
}

/// One authenticated predicate in a leaf conjunction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicPolynomialPredicate {
    kind: SymbolicPolynomialPredicateKind,
    polynomial: Arc<ParametricPolynomial>,
}

impl SymbolicPolynomialPredicate {
    pub fn kind(&self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub fn polynomial(&self) -> &ParametricPolynomial {
        self.polynomial.as_ref()
    }
}

/// One final conjunction, implicitly intersected with the certificate orthant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicSectorCase {
    id: SymbolicSectorCaseId,
    predicates: Box<[SymbolicPolynomialPredicate]>,
}

impl SymbolicSectorCase {
    pub fn id(&self) -> SymbolicSectorCaseId {
        self.id
    }

    pub fn predicates(&self) -> &[SymbolicPolynomialPredicate] {
        &self.predicates
    }
}

/// Deterministic children of one neutral equality/nonzero split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicSectorCaseSplitChildren {
    bad_case: SymbolicSectorCaseId,
    good_case: SymbolicSectorCaseId,
}

impl SymbolicSectorCaseSplitChildren {
    /// Child with the appended predicate `p = 0`.
    pub fn equal_zero_case(&self) -> SymbolicSectorCaseId {
        self.bad_case
    }

    /// Child with the appended predicate `p != 0`.
    pub fn nonzero_case(&self) -> SymbolicSectorCaseId {
        self.good_case
    }

    /// Child with the appended predicate `p = 0`.
    #[deprecated(note = "use equal_zero_case; semantic safety depends on the caller")]
    pub fn bad_case(&self) -> SymbolicSectorCaseId {
        self.bad_case
    }

    /// Child with the appended predicate `p != 0`.
    #[deprecated(note = "use nonzero_case; semantic safety depends on the caller")]
    pub fn good_case(&self) -> SymbolicSectorCaseId {
        self.good_case
    }
}

/// One retained binary refinement step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicSectorCaseSplit {
    ordinal: usize,
    parent: SymbolicSectorCaseId,
    bad_polynomial: Arc<ParametricPolynomial>,
    children: SymbolicSectorCaseSplitChildren,
}

impl SymbolicSectorCaseSplit {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn parent(&self) -> SymbolicSectorCaseId {
        self.parent
    }

    pub fn bad_polynomial(&self) -> &ParametricPolynomial {
        self.bad_polynomial.as_ref()
    }

    pub fn children(&self) -> SymbolicSectorCaseSplitChildren {
        self.children
    }
}

/// Exact retained-size summary, checked again during replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicSectorCaseStats {
    split_count: usize,
    leaf_count: usize,
    max_depth: usize,
    total_leaf_predicates: usize,
    retained_polynomial_terms: usize,
    retained_polynomial_bytes: usize,
}

impl SymbolicSectorCaseStats {
    pub fn split_count(&self) -> usize {
        self.split_count
    }

    pub fn leaf_count(&self) -> usize {
        self.leaf_count
    }

    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    pub fn total_leaf_predicates(&self) -> usize {
        self.total_leaf_predicates
    }

    pub fn retained_polynomial_terms(&self) -> usize {
        self.retained_polynomial_terms
    }

    pub fn retained_polynomial_bytes(&self) -> usize {
        self.retained_polynomial_bytes
    }
}

/// Immutable proof that the listed leaves are a finite, pairwise-disjoint
/// cover of the recorded integer orthant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicSectorCasePartitionCertificate {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    orthant: SymbolicSectorOrthant,
    splits: Box<[SymbolicSectorCaseSplit]>,
    cases: Box<[SymbolicSectorCase]>,
    source_identity: Arc<str>,
    stats: SymbolicSectorCaseStats,
}

impl SymbolicSectorCasePartitionCertificate {
    pub fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub fn orthant(&self) -> &SymbolicSectorOrthant {
        &self.orthant
    }

    pub fn splits(&self) -> &[SymbolicSectorCaseSplit] {
        &self.splits
    }

    pub fn cases(&self) -> &[SymbolicSectorCase] {
        &self.cases
    }

    /// Canonical exact identity of the context, orthant, and ordered split
    /// transcript.
    ///
    /// Final cases are intentionally not serialized here: authenticated
    /// replay deterministically reconstructs them from the transcript.  The
    /// returned `Arc` is therefore the compact source identity that dependent
    /// certificates should clone instead of serializing the complete
    /// partition again.
    pub fn source_identity(&self) -> &Arc<str> {
        &self.source_identity
    }

    pub fn stats(&self) -> SymbolicSectorCaseStats {
        self.stats
    }

    pub fn case(&self, id: SymbolicSectorCaseId) -> Option<&SymbolicSectorCase> {
        self.cases.iter().find(|case| case.id == id)
    }

    /// Reconstruct the complete split tree under default replay limits.
    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
        expected_sector: &SectorMask,
    ) -> Result<(), SymbolicSectorCaseError> {
        self.replay_with_limits(
            context,
            expected_sector,
            SymbolicSectorCaseLimits::default(),
        )
    }

    /// Reconstruct the complete split tree under caller-owned replay limits.
    pub fn replay_with_limits(
        &self,
        context: &ParametricCoefficientContext,
        expected_sector: &SectorMask,
        limits: SymbolicSectorCaseLimits,
    ) -> Result<(), SymbolicSectorCaseError> {
        if self.schema != SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA {
            return Err(SymbolicSectorCaseError::SchemaMismatch);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(SymbolicSectorCaseError::ContextMismatch);
        }
        check_limit(
            "symbolic sector indices",
            expected_sector.arity(),
            limits.max_indices,
        )?;
        check_limit(
            "symbolic sector context fingerprint bytes",
            context.fingerprint().len(),
            limits.max_context_fingerprint_bytes,
        )?;
        if self.orthant.sector() != expected_sector {
            return Err(SymbolicSectorCaseError::SectorMismatch);
        }
        let expected_orthant = SymbolicSectorOrthant::from_sector(expected_sector.clone());
        if self.orthant != expected_orthant {
            return Err(SymbolicSectorCaseError::OrthantMismatch);
        }

        let mut rebuilt =
            SymbolicSectorCasePartitionBuilder::try_new(context, expected_sector.clone(), limits)?;
        for (expected_ordinal, split) in self.splits.iter().enumerate() {
            if split.ordinal != expected_ordinal {
                return Err(SymbolicSectorCaseError::SplitTranscriptMismatch {
                    ordinal: expected_ordinal,
                });
            }
            let children = rebuilt.split_on_bad_polynomial_arc(
                context,
                split.parent,
                split.bad_polynomial.clone(),
            )?;
            if children != split.children
                || rebuilt.splits.last().is_none_or(|actual| actual != split)
            {
                return Err(SymbolicSectorCaseError::SplitTranscriptMismatch {
                    ordinal: expected_ordinal,
                });
            }
        }
        let reconstructed = rebuilt.try_into_certificate()?;
        if &reconstructed != self {
            return Err(SymbolicSectorCaseError::CertificateStateMismatch);
        }
        Ok(())
    }
}

/// Incremental producer for a structurally certified case partition.
#[derive(Clone, Debug)]
pub struct SymbolicSectorCasePartitionBuilder {
    context_fingerprint: Arc<str>,
    orthant: SymbolicSectorOrthant,
    live_cases: BTreeMap<SymbolicSectorCaseId, SymbolicSectorCase>,
    splits: Vec<SymbolicSectorCaseSplit>,
    next_case_id: u64,
    stats: SymbolicSectorCaseStats,
    limits: SymbolicSectorCaseLimits,
}

impl SymbolicSectorCasePartitionBuilder {
    pub fn try_new(
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        limits: SymbolicSectorCaseLimits,
    ) -> Result<Self, SymbolicSectorCaseError> {
        if sector.arity() != context.index_count() {
            return Err(SymbolicSectorCaseError::WrongIndexArity {
                expected: context.index_count(),
                actual: sector.arity(),
            });
        }
        check_limit(
            "symbolic sector indices",
            sector.arity(),
            limits.max_indices,
        )?;
        check_limit(
            "symbolic sector context fingerprint bytes",
            context.fingerprint().len(),
            limits.max_context_fingerprint_bytes,
        )?;
        check_limit("live symbolic sector cases", 1, limits.max_live_cases)?;
        let root = SymbolicSectorCase {
            id: SymbolicSectorCaseId::ROOT,
            predicates: Box::new([]),
        };
        Ok(Self {
            context_fingerprint: context.fingerprint().into(),
            orthant: SymbolicSectorOrthant::from_sector(sector),
            live_cases: BTreeMap::from([(SymbolicSectorCaseId::ROOT, root)]),
            splits: Vec::new(),
            next_case_id: 1,
            stats: SymbolicSectorCaseStats {
                split_count: 0,
                leaf_count: 1,
                max_depth: 0,
                total_leaf_predicates: 0,
                retained_polynomial_terms: 0,
                retained_polynomial_bytes: 0,
            },
            limits,
        })
    }

    pub fn root_case(&self) -> SymbolicSectorCaseId {
        SymbolicSectorCaseId::ROOT
    }

    pub fn orthant(&self) -> &SymbolicSectorOrthant {
        &self.orthant
    }

    pub fn live_cases(&self) -> impl Iterator<Item = &SymbolicSectorCase> {
        self.live_cases.values()
    }

    pub fn case(&self, id: SymbolicSectorCaseId) -> Option<&SymbolicSectorCase> {
        self.live_cases.get(&id)
    }

    pub fn stats(&self) -> SymbolicSectorCaseStats {
        self.stats
    }

    /// Exact budgets governing construction of this partition.  Higher proof
    /// layers use the same algebra bound when deciding whether a requested
    /// split polynomial is already represented by an equivalent locus on the
    /// current lineage.
    pub const fn limits(&self) -> SymbolicSectorCaseLimits {
        self.limits
    }

    /// Split one live case into the neutral branches `p = 0` and `p != 0`.
    ///
    /// The two child identifiers and branch order are deterministic.  Exact
    /// repetition of a polynomial on one lineage is rejected; no equivalence
    /// test for scalar multiples or polynomial ideals is attempted.
    pub fn split_on_bad_polynomial(
        &mut self,
        context: &ParametricCoefficientContext,
        case_id: SymbolicSectorCaseId,
        bad_polynomial: ParametricPolynomial,
    ) -> Result<SymbolicSectorCaseSplitChildren, SymbolicSectorCaseError> {
        self.split_on_bad_polynomial_arc(context, case_id, Arc::new(bad_polynomial))
    }

    /// Shared-payload seam used by authenticated replay. The public builder
    /// API deliberately remains owned, while replay can retain the exact
    /// immutable polynomial payload already carried by its source transcript.
    fn split_on_bad_polynomial_arc(
        &mut self,
        context: &ParametricCoefficientContext,
        case_id: SymbolicSectorCaseId,
        bad_polynomial: Arc<ParametricPolynomial>,
    ) -> Result<SymbolicSectorCaseSplitChildren, SymbolicSectorCaseError> {
        self.validate_context(context)?;
        context
            .validate_polynomial_with_limits(bad_polynomial.as_ref(), self.limits.exact_algebra)?;
        if bad_polynomial.is_zero() {
            return Err(SymbolicSectorCaseError::IdenticallyZeroSplitPolynomial);
        }
        // Constants are relative to the index polynomial ring K[n], not just
        // integer constants in Symbolica's ambient Z[theta,n] storage.  A
        // nonzero base-only polynomial is already invertible in K=Q(theta).
        if !context.polynomial_depends_on_indices_with_limits(
            bad_polynomial.as_ref(),
            self.limits.exact_algebra,
        )? {
            return Err(SymbolicSectorCaseError::NonzeroConstantSplitPolynomial);
        }

        let parent = self
            .live_cases
            .get(&case_id)
            .ok_or(SymbolicSectorCaseError::CaseNotLive { case: case_id })?;
        if let Some(predicate) = parent
            .predicates
            .iter()
            .find(|predicate| predicate.polynomial.as_ref() == bad_polynomial.as_ref())
        {
            return Err(SymbolicSectorCaseError::PredicateAlreadyDecided {
                case: case_id,
                kind: predicate.kind,
            });
        }

        let split_count = checked_add("symbolic sector case splits", self.splits.len(), 1)?;
        check_limit(
            "symbolic sector case splits",
            split_count,
            self.limits.max_splits,
        )?;
        let live_count = checked_add("live symbolic sector cases", self.live_cases.len(), 1)?;
        check_limit(
            "live symbolic sector cases",
            live_count,
            self.limits.max_live_cases,
        )?;
        let child_predicates = checked_add(
            "predicates per symbolic sector case",
            parent.predicates.len(),
            1,
        )?;
        check_limit(
            "predicates per symbolic sector case",
            child_predicates,
            self.limits.max_predicates_per_case,
        )?;
        // Removing the parent and inserting two children adds one copy of
        // every parent predicate plus the two new complementary predicates.
        let leaf_predicate_delta =
            checked_add("total symbolic leaf predicates", parent.predicates.len(), 2)?;
        let total_leaf_predicates = checked_add(
            "total symbolic leaf predicates",
            self.stats.total_leaf_predicates,
            leaf_predicate_delta,
        )?;
        check_limit(
            "total symbolic leaf predicates",
            total_leaf_predicates,
            self.limits.max_total_leaf_predicates,
        )?;

        // The split transcript and every descendant leaf share one immutable
        // payload for this split through `Arc`. Parent-path cloning therefore
        // adds predicate references, charged above, but no polynomial terms.
        let retained_polynomial_terms = checked_add(
            "retained symbolic predicate terms",
            self.stats.retained_polynomial_terms,
            bad_polynomial.term_count(),
        )?;
        check_limit(
            "retained symbolic predicate terms",
            retained_polynomial_terms,
            self.limits.max_retained_polynomial_terms,
        )?;

        let bad_polynomial_bytes = polynomial_display_bytes(
            bad_polynomial.as_ref(),
            self.limits.max_retained_polynomial_bytes,
        )?;
        let retained_polynomial_bytes = checked_add(
            "retained symbolic predicate bytes",
            self.stats.retained_polynomial_bytes,
            bad_polynomial_bytes,
        )?;
        check_limit(
            "retained symbolic predicate bytes",
            retained_polynomial_bytes,
            self.limits.max_retained_polynomial_bytes,
        )?;

        let bad_id = SymbolicSectorCaseId(self.next_case_id);
        let good_raw = self
            .next_case_id
            .checked_add(1)
            .ok_or(SymbolicSectorCaseError::CaseIdOverflow)?;
        let next_case_id = good_raw
            .checked_add(1)
            .ok_or(SymbolicSectorCaseError::CaseIdOverflow)?;
        let good_id = SymbolicSectorCaseId(good_raw);

        // All fallible preflights are complete. Store one immutable polynomial
        // payload for the transcript and both branches. Descendant path and
        // authenticated-replay clones copy only `Arc` references.
        let parent = self
            .live_cases
            .remove(&case_id)
            .expect("the live parent was authenticated above");
        let mut bad_predicates = parent.predicates.to_vec();
        bad_predicates.push(SymbolicPolynomialPredicate {
            kind: SymbolicPolynomialPredicateKind::EqualZero,
            polynomial: bad_polynomial.clone(),
        });
        let mut good_predicates = parent.predicates.into_vec();
        good_predicates.push(SymbolicPolynomialPredicate {
            kind: SymbolicPolynomialPredicateKind::NonZero,
            polynomial: bad_polynomial.clone(),
        });
        let children = SymbolicSectorCaseSplitChildren {
            bad_case: bad_id,
            good_case: good_id,
        };
        let split = SymbolicSectorCaseSplit {
            ordinal: self.splits.len(),
            parent: case_id,
            bad_polynomial,
            children,
        };

        self.live_cases.insert(
            bad_id,
            SymbolicSectorCase {
                id: bad_id,
                predicates: bad_predicates.into_boxed_slice(),
            },
        );
        self.live_cases.insert(
            good_id,
            SymbolicSectorCase {
                id: good_id,
                predicates: good_predicates.into_boxed_slice(),
            },
        );
        self.splits.push(split);
        self.next_case_id = next_case_id;
        self.stats = SymbolicSectorCaseStats {
            split_count,
            leaf_count: live_count,
            max_depth: self.stats.max_depth.max(child_predicates),
            total_leaf_predicates,
            retained_polynomial_terms,
            retained_polynomial_bytes,
        };
        Ok(children)
    }

    /// Extract the normalized pivot numerator and split on its zero locus.
    ///
    /// Denominators of the pivot coefficient remain separate domain
    /// conditions; callers must split or otherwise discharge them explicitly.
    pub fn split_on_pivot_coefficient(
        &mut self,
        context: &ParametricCoefficientContext,
        case_id: SymbolicSectorCaseId,
        pivot: &ParametricCoefficient,
    ) -> Result<SymbolicSectorCaseSplitChildren, SymbolicSectorCaseError> {
        self.validate_context(context)?;
        let bad_polynomial =
            context.numerator_condition_with_limits(pivot, self.limits.exact_algebra)?;
        self.split_on_bad_polynomial(context, case_id, bad_polynomial)
    }

    /// Freeze and immediately replay the complete proof.
    pub fn finish(
        self,
        context: &ParametricCoefficientContext,
    ) -> Result<SymbolicSectorCasePartitionCertificate, SymbolicSectorCaseError> {
        let expected_sector = self.orthant.sector.clone();
        let limits = self.limits;
        let certificate = self.try_into_certificate()?;
        certificate.replay_with_limits(context, &expected_sector, limits)?;
        Ok(certificate)
    }

    fn validate_context(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), SymbolicSectorCaseError> {
        if self.context_fingerprint.as_ref() == context.fingerprint() {
            Ok(())
        } else {
            Err(SymbolicSectorCaseError::ContextMismatch)
        }
    }

    fn try_into_certificate(
        self,
    ) -> Result<SymbolicSectorCasePartitionCertificate, SymbolicSectorCaseError> {
        let source_identity = partition_source_identity(
            &self.context_fingerprint,
            &self.orthant,
            &self.splits,
            self.limits.max_source_identity_bytes,
        )?;
        Ok(SymbolicSectorCasePartitionCertificate {
            schema: SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA,
            context_fingerprint: self.context_fingerprint,
            orthant: self.orthant,
            splits: self.splits.into_boxed_slice(),
            cases: self.live_cases.into_values().collect(),
            source_identity: source_identity.into(),
            stats: self.stats,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicSectorCaseError {
    WrongIndexArity {
        expected: usize,
        actual: usize,
    },
    ContextMismatch,
    SectorMismatch,
    OrthantMismatch,
    SchemaMismatch,
    IdenticallyZeroSplitPolynomial,
    NonzeroConstantSplitPolynomial,
    CaseNotLive {
        case: SymbolicSectorCaseId,
    },
    PredicateAlreadyDecided {
        case: SymbolicSectorCaseId,
        kind: SymbolicPolynomialPredicateKind,
    },
    CaseIdOverflow,
    SplitTranscriptMismatch {
        ordinal: usize,
    },
    CertificateStateMismatch,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
    },
    ParametricCoefficient(ParametricCoefficientError),
}

impl fmt::Display for SymbolicSectorCaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongIndexArity { expected, actual } => {
                write!(formatter, "index arity is {actual}, expected {expected}")
            }
            Self::ContextMismatch => {
                formatter.write_str("symbolic case proof belongs to a different K(n) context")
            }
            Self::SectorMismatch => {
                formatter.write_str("symbolic case proof belongs to a different sector")
            }
            Self::OrthantMismatch => {
                formatter.write_str("stored sector orthant constraints do not replay")
            }
            Self::SchemaMismatch => formatter.write_str("symbolic sector-case schema mismatch"),
            Self::IdenticallyZeroSplitPolynomial => {
                formatter.write_str("cannot refine a case by an identically zero polynomial")
            }
            Self::NonzeroConstantSplitPolynomial => formatter.write_str(
                "cannot refine a case by a nonzero coefficient-field constant polynomial",
            ),
            Self::CaseNotLive { case } => write!(formatter, "symbolic case {case} is not live"),
            Self::PredicateAlreadyDecided { case, kind } => write!(
                formatter,
                "symbolic case {case} already fixes this polynomial as {kind:?}"
            ),
            Self::CaseIdOverflow => formatter.write_str("symbolic case identifier overflow"),
            Self::SplitTranscriptMismatch { ordinal } => {
                write!(formatter, "symbolic case split {ordinal} does not replay")
            }
            Self::CertificateStateMismatch => {
                formatter.write_str("final symbolic case certificate state does not replay")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure { resource } => {
                write!(
                    formatter,
                    "{resource} allocation failed after bounded preflight"
                )
            }
            Self::ParametricCoefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SymbolicSectorCaseError {}

impl From<ParametricCoefficientError> for SymbolicSectorCaseError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SymbolicSectorCaseError> {
    left.checked_add(right)
        .ok_or(SymbolicSectorCaseError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicSectorCaseError> {
    if requested > limit {
        Err(SymbolicSectorCaseError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn polynomial_display_bytes(
    polynomial: &ParametricPolynomial,
    limit: usize,
) -> Result<usize, SymbolicSectorCaseError> {
    let mut writer = BoundedByteCounter { bytes: 0, limit };
    if write!(&mut writer, "{}", polynomial.raw()).is_err() {
        return Err(SymbolicSectorCaseError::ResourceLimit {
            resource: "retained symbolic predicate bytes",
            requested: limit.saturating_add(1),
            limit,
        });
    }
    Ok(writer.bytes)
}

/// Build the exact identity shared by every downstream view of one partition.
/// The encoding is structural and length/count delimited; it never relies on
/// `Debug` or on a probabilistic digest.
fn partition_source_identity(
    context_fingerprint: &str,
    orthant: &SymbolicSectorOrthant,
    splits: &[SymbolicSectorCaseSplit],
    limit: usize,
) -> Result<String, SymbolicSectorCaseError> {
    let mut identity = BoundedSourceIdentityBuilder::new(limit);
    write!(
        &mut identity,
        "{}:{}|context={}:{}|orthant={}",
        SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA.len(),
        SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA,
        context_fingerprint.len(),
        context_fingerprint,
        orthant.sector().arity(),
    )
    .map_err(|_| identity.error())?;
    for active in orthant.sector().active_bits() {
        identity
            .write_char(if *active { '1' } else { '0' })
            .map_err(|_| identity.error())?;
    }
    write!(&mut identity, "|splits={}", splits.len()).map_err(|_| identity.error())?;
    for split in splits {
        write!(
            &mut identity,
            "|split={},{},{},{}|poly=",
            split.ordinal,
            split.parent.value(),
            split.children.equal_zero_case().value(),
            split.children.nonzero_case().value(),
        )
        .map_err(|_| identity.error())?;
        write_source_identity_polynomial(&mut identity, &split.bad_polynomial)?;
    }
    identity.finish()
}

fn write_source_identity_polynomial(
    identity: &mut BoundedSourceIdentityBuilder,
    polynomial: &ParametricPolynomial,
) -> Result<(), SymbolicSectorCaseError> {
    let raw = polynomial.raw();
    write!(identity, "{},{}[", raw.variables.len(), raw.nterms()).map_err(|_| identity.error())?;
    for term in 0..raw.nterms() {
        if term != 0 {
            identity.write_char(';').map_err(|_| identity.error())?;
        }
        write!(identity, "{}:", raw.coefficients[term]).map_err(|_| identity.error())?;
        for (variable, exponent) in raw.exponents(term).iter().enumerate() {
            if variable != 0 {
                identity.write_char(',').map_err(|_| identity.error())?;
            }
            write!(identity, "{exponent}").map_err(|_| identity.error())?;
        }
    }
    identity.write_char(']').map_err(|_| identity.error())?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceIdentityFailure {
    Limit { requested: usize },
    Overflow,
    Allocation,
}

struct BoundedSourceIdentityBuilder {
    value: String,
    limit: usize,
    failure: Option<SourceIdentityFailure>,
}

impl BoundedSourceIdentityBuilder {
    fn new(limit: usize) -> Self {
        Self {
            value: String::new(),
            limit,
            failure: None,
        }
    }

    fn error(&self) -> SymbolicSectorCaseError {
        match self.failure {
            Some(SourceIdentityFailure::Limit { requested }) => {
                SymbolicSectorCaseError::ResourceLimit {
                    resource: "symbolic partition source identity bytes",
                    requested,
                    limit: self.limit,
                }
            }
            Some(SourceIdentityFailure::Overflow) => {
                SymbolicSectorCaseError::ResourceCountOverflow {
                    resource: "symbolic partition source identity bytes",
                }
            }
            Some(SourceIdentityFailure::Allocation) | None => {
                SymbolicSectorCaseError::AllocationFailure {
                    resource: "symbolic partition source identity",
                }
            }
        }
    }

    fn finish(self) -> Result<String, SymbolicSectorCaseError> {
        if self.failure.is_some() {
            return Err(self.error());
        }
        Ok(self.value)
    }
}

impl fmt::Write for BoundedSourceIdentityBuilder {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.value.len().checked_add(value.len()) else {
            self.failure = Some(SourceIdentityFailure::Overflow);
            return Err(fmt::Error);
        };
        if requested > self.limit {
            self.failure = Some(SourceIdentityFailure::Limit { requested });
            return Err(fmt::Error);
        }
        if self.value.try_reserve(value.len()).is_err() {
            self.failure = Some(SourceIdentityFailure::Allocation);
            return Err(fmt::Error);
        }
        self.value.push_str(value);
        Ok(())
    }
}

struct BoundedByteCounter {
    bytes: usize,
    limit: usize,
}

impl fmt::Write for BoundedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.bytes > self.limit {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoefficientContext;

    fn context() -> ParametricCoefficientContext {
        ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolic-sector-case-unit",
            2,
        )
        .unwrap()
    }

    fn n0_minus_two(context: &ParametricCoefficientContext) -> ParametricPolynomial {
        let coefficient = context
            .sub(&context.index(0).unwrap(), &context.integer(2))
            .unwrap();
        context.numerator_condition(&coefficient).unwrap()
    }

    fn one_split_certificate() -> SymbolicSectorCasePartitionCertificate {
        let context = context();
        let sector = SectorMask::try_new([true, false]).unwrap();
        let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
            &context,
            sector,
            SymbolicSectorCaseLimits::default(),
        )
        .unwrap();
        builder
            .split_on_bad_polynomial(&context, SymbolicSectorCaseId::ROOT, n0_minus_two(&context))
            .unwrap();
        builder.finish(&context).unwrap()
    }

    #[test]
    fn replay_rejects_tampered_schema_transcript_leaf_and_stats() {
        let context = context();
        let sector = SectorMask::try_new([true, false]).unwrap();
        let certificate = one_split_certificate();

        let mut tampered = certificate.clone();
        tampered.schema = "forged";
        assert_eq!(
            tampered.replay(&context, &sector),
            Err(SymbolicSectorCaseError::SchemaMismatch)
        );

        let mut tampered = certificate.clone();
        tampered.splits[0].children.good_case = SymbolicSectorCaseId(99);
        assert!(matches!(
            tampered.replay(&context, &sector),
            Err(SymbolicSectorCaseError::SplitTranscriptMismatch { ordinal: 0 })
        ));

        let mut tampered = certificate.clone();
        tampered.cases[0].predicates[0].kind = SymbolicPolynomialPredicateKind::NonZero;
        assert_eq!(
            tampered.replay(&context, &sector),
            Err(SymbolicSectorCaseError::CertificateStateMismatch)
        );

        let mut tampered = certificate;
        tampered.stats.leaf_count += 1;
        assert_eq!(
            tampered.replay(&context, &sector),
            Err(SymbolicSectorCaseError::CertificateStateMismatch)
        );

        let mut tampered = one_split_certificate();
        tampered.source_identity = Arc::from("forged-source-identity");
        assert_eq!(
            tampered.replay(&context, &sector),
            Err(SymbolicSectorCaseError::CertificateStateMismatch)
        );

        let mut tampered = one_split_certificate();
        tampered.stats.retained_polynomial_bytes += 1;
        assert_eq!(
            tampered.replay(&context, &sector),
            Err(SymbolicSectorCaseError::CertificateStateMismatch)
        );
    }

    #[test]
    fn replay_rejects_tampered_orthant() {
        let context = context();
        let sector = SectorMask::try_new([true, false]).unwrap();
        let mut certificate = one_split_certificate();
        certificate.orthant.constraints[0].side = SectorOrthantSide::AtMostZero;
        assert_eq!(
            certificate.replay(&context, &sector),
            Err(SymbolicSectorCaseError::OrthantMismatch)
        );
    }
}
