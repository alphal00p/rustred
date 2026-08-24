//! Backend-neutral normalized bad-domain formulas for sector coverage.
//!
//! The outer Symbolica-backed normalization layer authenticates every
//! polynomial against one immutable base structural-locus table, then records
//! only ordinals in this IR.  Backends may lower this representation in
//! different ways, but they may not add or reinterpret base loci here.

use crate::direct_bad_formula::DirectBadFormulaClause;
use std::fmt;

pub(crate) const PARAMETRIC_SECTOR_FORMULA_IR_V1_SCHEMA: &str =
    "rustred-parametric-sector-formula-ir-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum NormalizedBadLiteralPolarity {
    EqualZero,
    NonZero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NormalizedBadLiteral {
    structural_locus_ordinal: usize,
    polarity: NormalizedBadLiteralPolarity,
}

impl NormalizedBadLiteral {
    pub(crate) const fn new(
        structural_locus_ordinal: usize,
        polarity: NormalizedBadLiteralPolarity,
    ) -> Self {
        Self {
            structural_locus_ordinal,
            polarity,
        }
    }

    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.structural_locus_ordinal
    }

    pub(crate) const fn polarity(self) -> NormalizedBadLiteralPolarity {
        self.polarity
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum NormalizedBadClauseSource {
    IndexDomainGuard { condition_ordinal: usize },
    LeakEvent { event_ordinal: usize },
}

/// Backend routing role authenticated by the normalization layer.
///
/// An ordinary atomic equality remains an ordinary DNF clause.  Only a clause
/// explicitly marked `AtomicEqualZeroFactor` may be compiled through the
/// product-factor OR route used to avoid constructing a product polynomial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum NormalizedBadClauseRole {
    Ordinary,
    AtomicEqualZeroFactor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedBadClause {
    body: DirectBadFormulaClause<NormalizedBadLiteral>,
    sources: Box<[NormalizedBadClauseSource]>,
    role: NormalizedBadClauseRole,
}

impl NormalizedBadClause {
    pub(crate) fn new(
        body: DirectBadFormulaClause<NormalizedBadLiteral>,
        sources: Box<[NormalizedBadClauseSource]>,
        role: NormalizedBadClauseRole,
    ) -> Self {
        Self {
            body,
            sources,
            role,
        }
    }

    pub(crate) const fn body(&self) -> DirectBadFormulaClause<NormalizedBadLiteral> {
        self.body
    }

    pub(crate) fn sources(&self) -> &[NormalizedBadClauseSource] {
        &self.sources
    }

    pub(crate) const fn role(&self) -> NormalizedBadClauseRole {
        self.role
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NormalizedFactorZeroSource {
    structural_locus_ordinal: usize,
    source_clause_ordinal: usize,
}

impl NormalizedFactorZeroSource {
    pub(crate) const fn new(structural_locus_ordinal: usize, source_clause_ordinal: usize) -> Self {
        Self {
            structural_locus_ordinal,
            source_clause_ordinal,
        }
    }

    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.structural_locus_ordinal
    }

    pub(crate) const fn source_clause_ordinal(self) -> usize {
        self.source_clause_ordinal
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedBadFormulaBody {
    False,
    True {
        sources: Box<[NormalizedBadClauseSource]>,
    },
    Dnf {
        clauses: Box<[NormalizedBadClause]>,
        atomic_equal_zero_factors: Box<[NormalizedFactorZeroSource]>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedCandidateBadFormula {
    source_attempt_ordinal: usize,
    body: NormalizedBadFormulaBody,
}

impl NormalizedCandidateBadFormula {
    pub(crate) const fn new(source_attempt_ordinal: usize, body: NormalizedBadFormulaBody) -> Self {
        Self {
            source_attempt_ordinal,
            body,
        }
    }

    pub(crate) const fn source_attempt_ordinal(&self) -> usize {
        self.source_attempt_ordinal
    }

    pub(crate) const fn body(&self) -> &NormalizedBadFormulaBody {
        &self.body
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedCoverageAttempt {
    Certified(NormalizedCandidateBadFormula),
    Unsupported { source_attempt_ordinal: usize },
}

impl NormalizedCoverageAttempt {
    pub(crate) const fn source_attempt_ordinal(&self) -> usize {
        match self {
            Self::Certified(formula) => formula.source_attempt_ordinal,
            Self::Unsupported {
                source_attempt_ordinal,
            } => *source_attempt_ordinal,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedCoverageIr {
    schema: &'static str,
    base_structural_locus_count: usize,
    // Retain a fallibly reserved vector. A compiler assembling a caller-sized
    // attempt batch must not need an infallible proportional shrink merely to
    // cross this backend-neutral boundary.
    attempts: Vec<NormalizedCoverageAttempt>,
}

impl NormalizedCoverageIr {
    pub(crate) fn try_new(
        base_structural_locus_count: usize,
        attempts: Box<[NormalizedCoverageAttempt]>,
    ) -> Result<Self, ParametricSectorFormulaIrError> {
        Self::try_new_preallocated(base_structural_locus_count, attempts.into_vec())
    }

    /// Retain a caller's already fallibly reserved attempt allocation without
    /// requesting an infallible shrink or proportional copy.
    pub(crate) fn try_new_preallocated(
        base_structural_locus_count: usize,
        attempts: Vec<NormalizedCoverageAttempt>,
    ) -> Result<Self, ParametricSectorFormulaIrError> {
        let value = Self {
            schema: PARAMETRIC_SECTOR_FORMULA_IR_V1_SCHEMA,
            base_structural_locus_count,
            attempts,
        };
        validate_ir(&value)?;
        Ok(value)
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn base_structural_locus_count(&self) -> usize {
        self.base_structural_locus_count
    }

    pub(crate) fn attempts(&self) -> &[NormalizedCoverageAttempt] {
        &self.attempts
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorFormulaIrError {
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AttemptOrdinalMismatch {
        position: usize,
        ordinal: usize,
    },
    StructuralLocusOutOfRange {
        locus: usize,
        locus_count: usize,
    },
    EmptyClauseSources {
        clause_ordinal: usize,
    },
    NonCanonicalClauseSources {
        clause_ordinal: usize,
    },
    EmptyTrueSources,
    EmptyDnf,
    NonCanonicalClauseOrder {
        clause_ordinal: usize,
    },
    DuplicateClauseBody {
        first: usize,
        duplicate: usize,
    },
    FactorRoleBodyMismatch {
        clause_ordinal: usize,
    },
    NonCanonicalFactorOrder {
        factor_ordinal: usize,
    },
    FactorClauseOutOfRange {
        factor_ordinal: usize,
        clause_ordinal: usize,
        clause_count: usize,
    },
    FactorClauseMismatch {
        factor_ordinal: usize,
        clause_ordinal: usize,
    },
    FactorSourceCountMismatch {
        marked_clauses: usize,
        factor_references: usize,
    },
}

impl fmt::Display for ParametricSectorFormulaIrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "normalized sector formula IR error: {self:?}")
    }
}

impl std::error::Error for ParametricSectorFormulaIrError {}

/// Canonical DNF body order.
///
/// Derived ordering places every atom before every conjunction. Literals are
/// ordered by structural-locus ordinal and then polarity; conjunctions retain
/// their source-significant operand order and compare left operand before right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ClauseBodyKey {
    Atom(NormalizedBadLiteral),
    Conjunction(NormalizedBadLiteral, NormalizedBadLiteral),
}

impl From<DirectBadFormulaClause<NormalizedBadLiteral>> for ClauseBodyKey {
    fn from(value: DirectBadFormulaClause<NormalizedBadLiteral>) -> Self {
        match value {
            DirectBadFormulaClause::Atom(literal) => Self::Atom(literal),
            DirectBadFormulaClause::Conjunction(left, right) => Self::Conjunction(left, right),
        }
    }
}

fn validate_ir(ir: &NormalizedCoverageIr) -> Result<(), ParametricSectorFormulaIrError> {
    for (position, attempt) in ir.attempts.iter().enumerate() {
        let ordinal = attempt.source_attempt_ordinal();
        if ordinal != position {
            return Err(ParametricSectorFormulaIrError::AttemptOrdinalMismatch {
                position,
                ordinal,
            });
        }
        if let NormalizedCoverageAttempt::Certified(formula) = attempt {
            validate_formula_body(&formula.body, ir.base_structural_locus_count)?;
        }
    }
    Ok(())
}

fn validate_formula_body(
    body: &NormalizedBadFormulaBody,
    locus_count: usize,
) -> Result<(), ParametricSectorFormulaIrError> {
    match body {
        NormalizedBadFormulaBody::False => Ok(()),
        NormalizedBadFormulaBody::True { sources } => {
            if sources.is_empty() {
                return Err(ParametricSectorFormulaIrError::EmptyTrueSources);
            }
            if !strictly_increasing(sources) {
                return Err(ParametricSectorFormulaIrError::NonCanonicalClauseSources {
                    clause_ordinal: 0,
                });
            }
            Ok(())
        }
        NormalizedBadFormulaBody::Dnf {
            clauses,
            atomic_equal_zero_factors,
        } => {
            if clauses.is_empty() {
                return Err(ParametricSectorFormulaIrError::EmptyDnf);
            }

            let mut previous_body = None;
            let mut marked_factor_clauses = 0usize;
            for (ordinal, clause) in clauses.iter().enumerate() {
                if clause.sources.is_empty() {
                    return Err(ParametricSectorFormulaIrError::EmptyClauseSources {
                        clause_ordinal: ordinal,
                    });
                }
                if !strictly_increasing(&clause.sources) {
                    return Err(ParametricSectorFormulaIrError::NonCanonicalClauseSources {
                        clause_ordinal: ordinal,
                    });
                }
                match clause.body {
                    DirectBadFormulaClause::Atom(literal) => {
                        validate_literal(literal, locus_count)?;
                    }
                    DirectBadFormulaClause::Conjunction(left, right) => {
                        // Operand order is source-significant.  In particular,
                        // boundary/gate leak provenance must survive even when
                        // the reduced Boolean function is commutative.
                        validate_literal(left, locus_count)?;
                        validate_literal(right, locus_count)?;
                    }
                }
                if clause.role == NormalizedBadClauseRole::AtomicEqualZeroFactor {
                    marked_factor_clauses = marked_factor_clauses.checked_add(1).ok_or(
                        ParametricSectorFormulaIrError::ResourceCountOverflow {
                            resource: "normalized factor-role count",
                        },
                    )?;
                    if !matches!(
                        clause.body,
                        DirectBadFormulaClause::Atom(NormalizedBadLiteral {
                            polarity: NormalizedBadLiteralPolarity::EqualZero,
                            ..
                        })
                    ) {
                        return Err(ParametricSectorFormulaIrError::FactorRoleBodyMismatch {
                            clause_ordinal: ordinal,
                        });
                    }
                }

                let body_key = ClauseBodyKey::from(clause.body);
                if let Some((previous_key, previous_ordinal)) = previous_body {
                    if previous_key == body_key {
                        return Err(ParametricSectorFormulaIrError::DuplicateClauseBody {
                            first: previous_ordinal,
                            duplicate: ordinal,
                        });
                    }
                    if previous_key > body_key {
                        return Err(ParametricSectorFormulaIrError::NonCanonicalClauseOrder {
                            clause_ordinal: ordinal,
                        });
                    }
                }
                previous_body = Some((body_key, ordinal));
            }

            for (factor_ordinal, factor) in atomic_equal_zero_factors.iter().enumerate() {
                if factor.structural_locus_ordinal >= locus_count {
                    return Err(ParametricSectorFormulaIrError::StructuralLocusOutOfRange {
                        locus: factor.structural_locus_ordinal,
                        locus_count,
                    });
                }
                if factor_ordinal > 0
                    && atomic_equal_zero_factors[factor_ordinal - 1].structural_locus_ordinal
                        >= factor.structural_locus_ordinal
                {
                    return Err(ParametricSectorFormulaIrError::NonCanonicalFactorOrder {
                        factor_ordinal,
                    });
                }
                let clause = clauses.get(factor.source_clause_ordinal).ok_or(
                    ParametricSectorFormulaIrError::FactorClauseOutOfRange {
                        factor_ordinal,
                        clause_ordinal: factor.source_clause_ordinal,
                        clause_count: clauses.len(),
                    },
                )?;
                if clause.role != NormalizedBadClauseRole::AtomicEqualZeroFactor
                    || clause.body
                        != DirectBadFormulaClause::Atom(NormalizedBadLiteral::new(
                            factor.structural_locus_ordinal,
                            NormalizedBadLiteralPolarity::EqualZero,
                        ))
                {
                    return Err(ParametricSectorFormulaIrError::FactorClauseMismatch {
                        factor_ordinal,
                        clause_ordinal: factor.source_clause_ordinal,
                    });
                }
            }
            if marked_factor_clauses != atomic_equal_zero_factors.len() {
                return Err(ParametricSectorFormulaIrError::FactorSourceCountMismatch {
                    marked_clauses: marked_factor_clauses,
                    factor_references: atomic_equal_zero_factors.len(),
                });
            }
            Ok(())
        }
    }
}

fn validate_literal(
    literal: NormalizedBadLiteral,
    locus_count: usize,
) -> Result<(), ParametricSectorFormulaIrError> {
    if literal.structural_locus_ordinal >= locus_count {
        Err(ParametricSectorFormulaIrError::StructuralLocusOutOfRange {
            locus: literal.structural_locus_ordinal,
            locus_count,
        })
    } else {
        Ok(())
    }
}

fn strictly_increasing<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(ordinal: usize) -> Box<[NormalizedBadClauseSource]> {
        vec![NormalizedBadClauseSource::LeakEvent {
            event_ordinal: ordinal,
        }]
        .into_boxed_slice()
    }

    fn literal(locus: usize, polarity: NormalizedBadLiteralPolarity) -> NormalizedBadLiteral {
        NormalizedBadLiteral::new(locus, polarity)
    }

    fn clause(
        body: DirectBadFormulaClause<NormalizedBadLiteral>,
        source_ordinal: usize,
        role: NormalizedBadClauseRole,
    ) -> NormalizedBadClause {
        NormalizedBadClause::new(body, source(source_ordinal), role)
    }

    fn certified(
        ordinal: usize,
        clauses: Vec<NormalizedBadClause>,
        factors: Vec<NormalizedFactorZeroSource>,
    ) -> NormalizedCoverageAttempt {
        NormalizedCoverageAttempt::Certified(NormalizedCandidateBadFormula::new(
            ordinal,
            NormalizedBadFormulaBody::Dnf {
                clauses: clauses.into_boxed_slice(),
                atomic_equal_zero_factors: factors.into_boxed_slice(),
            },
        ))
    }

    #[test]
    fn ordinary_equal_zero_and_source_ordered_conjunctions_are_valid() {
        let equal = literal(2, NormalizedBadLiteralPolarity::EqualZero);
        let nonzero = literal(2, NormalizedBadLiteralPolarity::NonZero);
        let ir = NormalizedCoverageIr::try_new(
            3,
            vec![certified(
                0,
                vec![
                    clause(
                        DirectBadFormulaClause::Atom(equal),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    ),
                    clause(
                        DirectBadFormulaClause::Conjunction(equal, nonzero),
                        1,
                        NormalizedBadClauseRole::Ordinary,
                    ),
                    clause(
                        DirectBadFormulaClause::Conjunction(nonzero, equal),
                        2,
                        NormalizedBadClauseRole::Ordinary,
                    ),
                ],
                Vec::new(),
            )]
            .into_boxed_slice(),
        )
        .unwrap();
        assert_eq!(ir.base_structural_locus_count(), 3);
        assert_eq!(ir.attempts().len(), 1);
    }

    #[test]
    fn reversed_clause_body_order_is_rejected_at_the_second_ordinal() {
        let result = NormalizedCoverageIr::try_new(
            2,
            vec![certified(
                0,
                vec![
                    clause(
                        DirectBadFormulaClause::Atom(literal(
                            1,
                            NormalizedBadLiteralPolarity::EqualZero,
                        )),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    ),
                    clause(
                        DirectBadFormulaClause::Atom(literal(
                            0,
                            NormalizedBadLiteralPolarity::EqualZero,
                        )),
                        1,
                        NormalizedBadClauseRole::Ordinary,
                    ),
                ],
                Vec::new(),
            )]
            .into_boxed_slice(),
        );
        assert!(matches!(
            result,
            Err(ParametricSectorFormulaIrError::NonCanonicalClauseOrder { clause_ordinal: 1 })
        ));
    }

    #[test]
    fn adjacent_duplicate_clause_body_is_rejected_with_exact_ordinals() {
        let equal = literal(1, NormalizedBadLiteralPolarity::EqualZero);
        let result = NormalizedCoverageIr::try_new(
            2,
            vec![certified(
                0,
                vec![
                    clause(
                        DirectBadFormulaClause::Atom(literal(
                            0,
                            NormalizedBadLiteralPolarity::EqualZero,
                        )),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    ),
                    clause(
                        DirectBadFormulaClause::Atom(equal),
                        1,
                        NormalizedBadClauseRole::Ordinary,
                    ),
                    clause(
                        DirectBadFormulaClause::Atom(equal),
                        2,
                        NormalizedBadClauseRole::Ordinary,
                    ),
                ],
                Vec::new(),
            )]
            .into_boxed_slice(),
        );
        assert!(matches!(
            result,
            Err(ParametricSectorFormulaIrError::DuplicateClauseBody {
                first: 1,
                duplicate: 2
            })
        ));
    }

    #[test]
    fn factor_sources_are_exact_and_ordinary_equalities_remain_ordinary() {
        let clauses = vec![
            clause(
                DirectBadFormulaClause::Atom(literal(1, NormalizedBadLiteralPolarity::EqualZero)),
                0,
                NormalizedBadClauseRole::Ordinary,
            ),
            clause(
                DirectBadFormulaClause::Atom(literal(3, NormalizedBadLiteralPolarity::EqualZero)),
                1,
                NormalizedBadClauseRole::AtomicEqualZeroFactor,
            ),
            clause(
                DirectBadFormulaClause::Atom(literal(5, NormalizedBadLiteralPolarity::EqualZero)),
                2,
                NormalizedBadClauseRole::AtomicEqualZeroFactor,
            ),
        ];
        let ir = NormalizedCoverageIr::try_new(
            6,
            vec![certified(
                0,
                clauses,
                vec![
                    NormalizedFactorZeroSource::new(3, 1),
                    NormalizedFactorZeroSource::new(5, 2),
                ],
            )]
            .into_boxed_slice(),
        )
        .unwrap();
        let NormalizedCoverageAttempt::Certified(formula) = &ir.attempts()[0] else {
            panic!("expected certified formula")
        };
        let NormalizedBadFormulaBody::Dnf { clauses, .. } = formula.body() else {
            panic!("expected DNF")
        };
        assert_eq!(clauses[0].role(), NormalizedBadClauseRole::Ordinary);
    }

    #[test]
    fn malformed_factor_provenance_is_rejected() {
        let marked_nonzero = clause(
            DirectBadFormulaClause::Atom(literal(1, NormalizedBadLiteralPolarity::NonZero)),
            0,
            NormalizedBadClauseRole::AtomicEqualZeroFactor,
        );
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                2,
                vec![certified(0, vec![marked_nonzero], Vec::new())].into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::FactorRoleBodyMismatch { clause_ordinal: 0 })
        ));

        let marked = clause(
            DirectBadFormulaClause::Atom(literal(1, NormalizedBadLiteralPolarity::EqualZero)),
            0,
            NormalizedBadClauseRole::AtomicEqualZeroFactor,
        );
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                3,
                vec![certified(
                    0,
                    vec![marked.clone()],
                    vec![NormalizedFactorZeroSource::new(2, 0)],
                )]
                .into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::FactorClauseMismatch {
                factor_ordinal: 0,
                clause_ordinal: 0
            })
        ));
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                2,
                vec![certified(0, vec![marked], Vec::new())].into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::FactorSourceCountMismatch {
                marked_clauses: 1,
                factor_references: 0
            })
        ));
    }

    #[test]
    fn factor_order_and_clause_reference_are_validated() {
        let marked = |locus, source_ordinal| {
            clause(
                DirectBadFormulaClause::Atom(literal(
                    locus,
                    NormalizedBadLiteralPolarity::EqualZero,
                )),
                source_ordinal,
                NormalizedBadClauseRole::AtomicEqualZeroFactor,
            )
        };
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                4,
                vec![certified(
                    0,
                    vec![marked(1, 0), marked(3, 1)],
                    vec![
                        NormalizedFactorZeroSource::new(3, 1),
                        NormalizedFactorZeroSource::new(1, 0),
                    ],
                )]
                .into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::NonCanonicalFactorOrder { factor_ordinal: 1 })
        ));
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                2,
                vec![certified(
                    0,
                    vec![marked(1, 0)],
                    vec![NormalizedFactorZeroSource::new(1, 7)],
                )]
                .into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::FactorClauseOutOfRange {
                factor_ordinal: 0,
                clause_ordinal: 7,
                clause_count: 1
            })
        ));
    }

    #[test]
    fn attempts_sources_constants_and_locus_range_are_validated() {
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                0,
                vec![NormalizedCoverageAttempt::Unsupported {
                    source_attempt_ordinal: 1,
                }]
                .into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::AttemptOrdinalMismatch {
                position: 0,
                ordinal: 1
            })
        ));
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                0,
                vec![NormalizedCoverageAttempt::Certified(
                    NormalizedCandidateBadFormula::new(
                        0,
                        NormalizedBadFormulaBody::True {
                            sources: Vec::new().into_boxed_slice(),
                        },
                    ),
                )]
                .into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::EmptyTrueSources)
        ));
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                1,
                vec![certified(
                    0,
                    vec![clause(
                        DirectBadFormulaClause::Atom(literal(
                            1,
                            NormalizedBadLiteralPolarity::EqualZero,
                        )),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                )]
                .into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::StructuralLocusOutOfRange {
                locus: 1,
                locus_count: 1
            })
        ));
    }

    #[test]
    fn empty_and_noncanonical_formula_payloads_are_rejected() {
        let empty_dnf = NormalizedCoverageAttempt::Certified(NormalizedCandidateBadFormula::new(
            0,
            NormalizedBadFormulaBody::Dnf {
                clauses: Vec::new().into_boxed_slice(),
                atomic_equal_zero_factors: Vec::new().into_boxed_slice(),
            },
        ));
        assert!(matches!(
            NormalizedCoverageIr::try_new(0, vec![empty_dnf].into_boxed_slice()),
            Err(ParametricSectorFormulaIrError::EmptyDnf)
        ));

        let body =
            DirectBadFormulaClause::Atom(literal(0, NormalizedBadLiteralPolarity::EqualZero));
        let empty_sources = NormalizedBadClause::new(
            body,
            Vec::new().into_boxed_slice(),
            NormalizedBadClauseRole::Ordinary,
        );
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                1,
                vec![certified(0, vec![empty_sources], Vec::new())].into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::EmptyClauseSources { clause_ordinal: 0 })
        ));

        let duplicate_sources = NormalizedBadClause::new(
            body,
            vec![
                NormalizedBadClauseSource::LeakEvent { event_ordinal: 1 },
                NormalizedBadClauseSource::LeakEvent { event_ordinal: 1 },
            ]
            .into_boxed_slice(),
            NormalizedBadClauseRole::Ordinary,
        );
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                1,
                vec![certified(0, vec![duplicate_sources], Vec::new())].into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::NonCanonicalClauseSources { clause_ordinal: 0 })
        ));

        let true_with_reversed_sources =
            NormalizedCoverageAttempt::Certified(NormalizedCandidateBadFormula::new(
                0,
                NormalizedBadFormulaBody::True {
                    sources: vec![
                        NormalizedBadClauseSource::LeakEvent { event_ordinal: 1 },
                        NormalizedBadClauseSource::LeakEvent { event_ordinal: 0 },
                    ]
                    .into_boxed_slice(),
                },
            ));
        assert!(matches!(
            NormalizedCoverageIr::try_new(0, vec![true_with_reversed_sources].into_boxed_slice(),),
            Err(ParametricSectorFormulaIrError::NonCanonicalClauseSources { clause_ordinal: 0 })
        ));
    }

    #[test]
    fn factor_role_and_factor_index_bijection_rejects_all_malformed_shapes() {
        let equal = literal(0, NormalizedBadLiteralPolarity::EqualZero);
        let nonzero = literal(0, NormalizedBadLiteralPolarity::NonZero);
        let marked_conjunction = clause(
            DirectBadFormulaClause::Conjunction(equal, nonzero),
            0,
            NormalizedBadClauseRole::AtomicEqualZeroFactor,
        );
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                1,
                vec![certified(0, vec![marked_conjunction], Vec::new())].into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::FactorRoleBodyMismatch { clause_ordinal: 0 })
        ));

        let marked = clause(
            DirectBadFormulaClause::Atom(equal),
            0,
            NormalizedBadClauseRole::AtomicEqualZeroFactor,
        );
        assert!(matches!(
            NormalizedCoverageIr::try_new(
                1,
                vec![certified(
                    0,
                    vec![marked.clone()],
                    vec![
                        NormalizedFactorZeroSource::new(0, 0),
                        NormalizedFactorZeroSource::new(0, 0),
                    ],
                )]
                .into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::NonCanonicalFactorOrder { factor_ordinal: 1 })
        ));

        assert!(matches!(
            NormalizedCoverageIr::try_new(
                1,
                vec![certified(
                    0,
                    vec![marked],
                    vec![NormalizedFactorZeroSource::new(1, 0)],
                )]
                .into_boxed_slice(),
            ),
            Err(ParametricSectorFormulaIrError::StructuralLocusOutOfRange {
                locus: 1,
                locus_count: 1
            })
        ));
    }
}
