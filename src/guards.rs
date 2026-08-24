//! Flat, deterministic provenance for exceptional nonzero loci.
//!
//! Guard origins are deliberately non-recursive.  Derivations accumulate a
//! set of atomic facts instead of nesting one provenance tree inside another;
//! equal polynomial conditions can therefore merge without making their
//! representation depend on the order in which algebraic operations ran.

use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

fn write_joined<T: fmt::Display>(
    writer: &mut impl fmt::Write,
    values: &[T],
    separator: &str,
) -> fmt::Result {
    for (ordinal, value) in values.iter().enumerate() {
        if ordinal != 0 {
            writer.write_str(separator)?;
        }
        write!(writer, "{value}")?;
    }
    Ok(())
}

/// One coefficient-valued datum supplied when constructing an integral
/// family.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CoefficientLocation {
    Dimension,
    DenominatorConstant {
        denominator: usize,
    },
    DenominatorCoefficient {
        denominator: usize,
        coordinate: usize,
    },
    ExternalGram {
        row: usize,
        column: usize,
    },
    PowerShift {
        denominator: usize,
    },
    BasisDeterminantNumerator,
}

impl CoefficientLocation {
    /// Version-stable identity used inside persisted proof manifests.
    pub fn stable_string(&self) -> String {
        let mut output = String::new();
        self.write_stable(&mut output)
            .expect("writing coefficient-location provenance to String cannot fail");
        output
    }

    pub(crate) fn write_stable(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Dimension => writer.write_str("dimension"),
            Self::DenominatorConstant { denominator } => {
                write!(writer, "denominator-constant:{denominator}")
            }
            Self::DenominatorCoefficient {
                denominator,
                coordinate,
            } => write!(writer, "denominator-coefficient:{denominator}:{coordinate}"),
            Self::ExternalGram { row, column } => {
                write!(writer, "external-gram:{row}:{column}")
            }
            Self::PowerShift { denominator } => write!(writer, "power-shift:{denominator}"),
            Self::BasisDeterminantNumerator => writer.write_str("basis-determinant-numerator"),
        }
    }
}

/// Stable, module-independent identity of a source relation.
///
/// This mirrors the public row taxonomy without depending on the relation
/// module, avoiding a type dependency cycle in coefficient-level guards.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GuardRowId {
    OrdinaryIbp {
        contraction_momentum: usize,
        differentiated_loop: usize,
    },
    LorentzInvariance {
        first_external: usize,
        second_external: usize,
    },
    Derived {
        label: Arc<str>,
    },
}

impl GuardRowId {
    /// Version-stable identity used inside persisted proof manifests.
    pub fn stable_string(&self) -> String {
        let mut output = String::new();
        self.write_stable(&mut output)
            .expect("writing guard-row provenance to String cannot fail");
        output
    }

    pub(crate) fn write_stable(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::OrdinaryIbp {
                contraction_momentum,
                differentiated_loop,
            } => write!(
                writer,
                "ordinary-ibp:{contraction_momentum}:{differentiated_loop}"
            ),
            Self::LorentzInvariance {
                first_external,
                second_external,
            } => write!(
                writer,
                "lorentz-invariance:{first_external}:{second_external}"
            ),
            Self::Derived { label } => write!(writer, "derived:{}:{label}", label.len()),
        }
    }

    pub(crate) fn shared_payload_bytes(&self) -> usize {
        match self {
            Self::Derived { label } => label.len(),
            Self::OrdinaryIbp { .. } | Self::LorentzInvariance { .. } => 0,
        }
    }
}

/// One atomic reason why a polynomial must be nonzero.
///
/// The enum contains no `GuardOrigin` child.  Transformations append another
/// origin to a condition's ordered set, which keeps provenance finite, flat,
/// and deterministic even after repeated translation or adapter round trips.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GuardOrigin {
    /// The denominator of a coefficient supplied as family input.
    FamilyInputCoefficientDenominator { location: CoefficientLocation },
    /// The numerator of the complete denominator-basis determinant.
    FamilyBasisDeterminantNumerator,
    /// The numerator whose generic nonvanishing makes a power shift contribute
    /// to LiteRed's effective sector support.
    PowerShiftSupport { denominator: usize },

    /// Denominator of the dividend operand before guarded division.
    GuardedDivisionDividendDenominator,
    /// Denominator of the divisor operand before guarded division.
    GuardedDivisionDivisorDenominator,
    /// Numerator of the divisor before guarded division; this is the actual
    /// nonzero requirement for division, including a normalized `0 / n`.
    GuardedDivisionDivisorNumerator,

    /// A condition inserted through the polynomial-only relation API.
    ExplicitRelationCondition,
    /// Public-safe provenance for one concrete nonzero condition emitted by
    /// the sealed generated-affine rule path.
    ///
    /// The retained owner certificate carries the complete replayable
    /// provenance.  Concrete reductions expose only this flat marker so
    /// private affine translations, recentering vectors, row identities, and
    /// certificate locators cannot escape through the public guard API.
    GeneratedAffineSealedCondition,
    /// The condition was attached to this relation.  Source-row atoms remain
    /// in the set when the condition later flows into a derived row.
    RelationConditionAttached { row: GuardRowId },
    /// Denominator of a term before it is inserted into a relation.
    RelationInputTermDenominator { row: GuardRowId, shift: Box<[i64]> },
    /// Denominator of the collected coefficient after equal relation keys
    /// have been added.
    RelationCollectedTermDenominator { row: GuardRowId, shift: Box<[i64]> },
    /// Denominator of a scalar used to add a scaled relation.
    RelationScaleFactorDenominator {
        target_row: GuardRowId,
        source_row: GuardRowId,
    },

    /// A whole relation was translated into another stable row identity.
    RelationTranslation {
        source_row: GuardRowId,
        target_row: GuardRowId,
        offset: Box<[i64]>,
    },

    /// A residual-affine pivot equation was recentered using two distinct
    /// lattice operations. `coefficient_offset` is the free-coordinate
    /// substitution applied to coefficients and guards, while `key_center`
    /// is subtracted from every ambient integral key.  Keeping both vectors
    /// in one flat provenance atom prevents this operation from being
    /// mistaken for an ordinary whole-relation translation.
    RelationAffineFreeRecentering {
        source_row: GuardRowId,
        target_row: GuardRowId,
        // Retain the fallibly reserved vectors. Converting a user-sized Vec
        // into Box<[T]> can request a second proportional shrink allocation.
        coefficient_offset: Vec<i64>,
        key_center: Vec<i64>,
    },

    /// A whole relation was transported through a verified denominator
    /// permutation and reparameterized back onto the canonical `n` variables.
    RelationIndexPermutation {
        source_row: GuardRowId,
        target_row: GuardRowId,
        source_to_target: Box<[usize]>,
    },

    /// Affine index translation `n -> n + offset` applied to a guard.
    IndexTranslation { offset: Box<[i64]> },
    /// Simultaneous index substitution
    /// `n_source[i] -> n_target[source_to_target[i]]`.
    IndexPermutation { source_to_target: Box<[usize]> },
    /// One exact base-field domain condition retained by the verified affine
    /// family map underlying a denominator permutation.  The symmetry proof
    /// itself remains in the higher-level transport certificate; this flat
    /// atom identifies the replayed condition without introducing recursive
    /// provenance types.
    VerifiedSymmetryMapDomain {
        source_to_target: Box<[usize]>,
        condition_ordinal: usize,
    },
    /// Exact integer specialization applied to a guard.
    IndexSpecialization { assignment: Box<[i64]> },
    /// Exact sparse integer specialization applied while the remaining index
    /// variables stay symbolic.  Entries are canonicalized by increasing
    /// index position before this provenance atom can be constructed.
    PartialIndexSpecialization { assignments: Box<[(usize, i64)]> },
    /// A certified simultaneous unit-affine index substitution was applied.
    ///
    /// The enclosing certificate owns the complete replayable map.  These
    /// three fields are a compact, typed locator into that certificate rather
    /// than an unauthenticated copy of its serialized payload.
    ResidualUnitAffineIndexSubstitution {
        source_case: u64,
        predicate_ordinal: usize,
        bound_position: usize,
    },
    /// One Coverage V4 branch guard was composed through the complete
    /// source-neutral residual affine integer-system map.
    ResidualAffineBranchNonzeroGuardSubstitution {
        source_case: u64,
        source_work_item_ordinal: usize,
        ready_terminal_ordinal: usize,
        structural_locus_ordinal: usize,
    },
    /// Original denominator of a coefficient specialized separately from a
    /// relation.
    CoefficientSpecializationDenominator,
    /// Original denominator of a coefficient after sparse index
    /// specialization and before fraction normalization can cancel it.
    CoefficientPartialSpecializationDenominator,
    /// Mapped pre-normalization denominator of one concrete term in a source
    /// row partially specialized on an equality locus.
    RelationPartialSpecializationTermDenominator { row: GuardRowId, shift: Box<[i64]> },
    /// Mapped pre-normalization denominator of one coefficient restricted to
    /// a certified unit-affine residual locus.
    CoefficientResidualUnitAffineSubstitutionDenominator {
        source_case: u64,
        predicate_ordinal: usize,
        bound_position: usize,
    },
    /// Mapped pre-normalization denominator of one relation term restricted
    /// to a certified unit-affine residual locus.
    RelationResidualUnitAffineSubstitutionTermDenominator {
        row: GuardRowId,
        shift: Box<[i64]>,
        source_case: u64,
        predicate_ordinal: usize,
        bound_position: usize,
    },
    /// Mapped pre-normalization denominator of one relation term restricted
    /// to a certified residual affine branch.
    ///
    /// The enclosing certificate owns the complete replayable map.  The
    /// three branch fields form a compact locator into that certificate.
    RelationResidualAffineBranchSubstitutionTermDenominator {
        row: GuardRowId,
        shift: Box<[i64]>,
        source_case: u64,
        source_work_item_ordinal: usize,
        ready_terminal_ordinal: usize,
    },
    /// A complete relation was restricted to a certified unit-affine
    /// residual locus.
    RelationResidualUnitAffineSubstitution {
        source_row: GuardRowId,
        target_row: GuardRowId,
        source_case: u64,
        predicate_ordinal: usize,
        bound_position: usize,
    },
    /// A complete relation was restricted to a certified residual affine
    /// branch.
    ///
    /// The enclosing certificate owns the complete replayable map.  The
    /// three branch fields form a compact locator into that certificate.
    RelationResidualAffineBranchSubstitution {
        source_row: GuardRowId,
        target_row: GuardRowId,
        source_case: u64,
        source_work_item_ordinal: usize,
        ready_terminal_ordinal: usize,
    },
    /// Numerator of the collected LHS coefficient inverted after quotienting
    /// a concrete parametric equation by zero sectors and symmetries.
    QuotientPivotNumerator,
    /// Numerator of one exact concrete-elimination pivot after generated rows
    /// were specialized and quotient-collected.
    ConcreteQuotientEliminationPivotNumerator { pivot: usize },

    /// A condition inserted through the polynomial-only shift-operator API.
    ExplicitShiftOperatorCondition,
    /// The condition was attached to this operator expression.
    ShiftOperatorConditionAttached { row: GuardRowId },
    /// Denominator of a scalar coefficient before operator-term insertion.
    ShiftOperatorInputTermDenominator { row: GuardRowId },
    /// Denominator after collecting equal operator monomials.
    ShiftOperatorCollectedTermDenominator { row: GuardRowId },
    /// The relation-to-operator adapter carried this condition.
    ShiftOperatorFromRelationAdapter { row: GuardRowId },
    /// The operator-to-relation adapter carried this condition.
    ShiftOperatorToRelationAdapter { row: GuardRowId },

    /// Compact locator for one exact generated-affine group recentering event.
    ///
    /// The future database owner retains the replayable coefficient offset,
    /// physical key center, and row provenance.  This public flat atom names
    /// only that owner-local event: it deliberately retains no vectors,
    /// hashes, row labels, coefficients, or physical geometry.
    GeneratedAffineGroupRecentering {
        solve_group_ordinal: usize,
        database_epoch: usize,
        event_ordinal: usize,
    },
    /// Compact locator for the denominator guard of one coefficient emitted
    /// by an exact generated-affine group top-reduction event.
    ///
    /// The database owner retains the physical key, affine translation,
    /// coefficient payload, and replay certificate. This flat public atom
    /// deliberately identifies only the owner-local coefficient operation;
    /// `pivot_normalization` distinguishes the normalized pivot coefficient
    /// from ordinary emitted terms without retaining either coefficient.
    GeneratedAffineGroupTopReductionCoefficientDenominator {
        solve_group_ordinal: usize,
        database_epoch: usize,
        event_ordinal: usize,
        operation_ordinal: usize,
        term_ordinal: usize,
        pivot_normalization: bool,
    },
}

impl GuardOrigin {
    fn retained_byte_base_bound() -> Option<usize> {
        // One insertion can allocate a complete B-tree node with capacity
        // for several keys and child edges. Use a deliberately loose
        // complete-node allowance per atom without depending on libstd's
        // private branching factor.
        size_of::<Self>()
            .checked_mul(16)?
            .checked_add(32usize.checked_mul(size_of::<usize>())?)
    }

    /// Preflight the provenance atom used when a mapped relation-term
    /// denominator is attached by the affine-locus wrapper.  This variant
    /// owns a boxed shift, so callers need its byte bound before allocating
    /// that payload.
    pub(crate) fn residual_unit_affine_term_denominator_retained_byte_bound(
        row: &GuardRowId,
        shift_len: usize,
    ) -> Option<usize> {
        Self::retained_byte_base_bound()?
            .checked_add(row.shared_payload_bytes())?
            .checked_add(shift_len.checked_mul(size_of::<i64>())?)
    }

    /// Preflight the provenance atom used when a mapped relation-term
    /// denominator is attached by the residual-affine branch wrapper.  The
    /// atom owns a boxed shift, so this must run before that payload is
    /// allocated.
    pub(crate) fn residual_affine_branch_term_denominator_retained_byte_bound(
        row: &GuardRowId,
        shift_len: usize,
    ) -> Option<usize> {
        Self::retained_byte_base_bound()?
            .checked_add(row.shared_payload_bytes())?
            .checked_add(shift_len.checked_mul(size_of::<i64>())?)
    }

    /// Preflight the input-denominator atom constructed by ordinary
    /// `ParametricRelation` term insertion. This has the same owned shape as
    /// the affine wrapper's term-denominator atom, but naming the actual
    /// variant keeps complete-row translation audits source-exact.
    pub(crate) fn relation_input_term_denominator_retained_byte_bound(
        row: &GuardRowId,
        shift_len: usize,
    ) -> Option<usize> {
        Self::retained_byte_base_bound()?
            .checked_add(row.shared_payload_bytes())?
            .checked_add(shift_len.checked_mul(size_of::<i64>())?)
    }

    /// Preflight the provenance atom used when a complete relation is mapped
    /// through a residual-affine branch.  Callers can budget both shared row
    /// labels before cloning either row identity.
    pub(crate) fn residual_affine_branch_relation_retained_byte_bound(
        source_row: &GuardRowId,
        target_row: &GuardRowId,
    ) -> Option<usize> {
        Self::retained_byte_base_bound()?
            .checked_add(source_row.shared_payload_bytes())?
            .checked_add(target_row.shared_payload_bytes())
    }

    pub(crate) fn index_translation_retained_byte_bound(shift_len: usize) -> Option<usize> {
        Self::retained_byte_base_bound()?.checked_add(shift_len.checked_mul(size_of::<i64>())?)
    }

    pub(crate) fn index_specialization_retained_byte_bound(assignment_len: usize) -> Option<usize> {
        Self::retained_byte_base_bound()?.checked_add(assignment_len.checked_mul(size_of::<i64>())?)
    }

    pub(crate) fn relation_translation_retained_byte_bound(
        source_row: &GuardRowId,
        target_row: &GuardRowId,
        shift_len: usize,
    ) -> Option<usize> {
        Self::retained_byte_base_bound()?
            .checked_add(source_row.shared_payload_bytes())?
            .checked_add(target_row.shared_payload_bytes())?
            .checked_add(shift_len.checked_mul(size_of::<i64>())?)
    }

    pub(crate) fn relation_affine_free_recentering_retained_byte_bound(
        source_row: &GuardRowId,
        target_row: &GuardRowId,
        coefficient_offset_len: usize,
        key_center_len: usize,
    ) -> Option<usize> {
        Self::retained_byte_base_bound()?
            .checked_add(source_row.shared_payload_bytes())?
            .checked_add(target_row.shared_payload_bytes())?
            .checked_add(coefficient_offset_len.checked_mul(size_of::<i64>())?)?
            .checked_add(key_center_len.checked_mul(size_of::<i64>())?)
    }

    pub(crate) fn relation_attached_retained_byte_bound(row: &GuardRowId) -> Option<usize> {
        Self::retained_byte_base_bound()?.checked_add(row.shared_payload_bytes())
    }

    /// Version-stable identity used inside persisted proof manifests.
    pub fn stable_string(&self) -> String {
        let mut output = String::new();
        self.write_stable(&mut output)
            .expect("writing guard-origin provenance to String cannot fail");
        output
    }

    /// Conservative retained-byte census for one flat provenance atom.
    ///
    /// This includes a conservative complete B-tree-node allowance, boxed
    /// slice payloads, and shared row-label bytes. It performs only
    /// checked arithmetic and allocates nothing, so callers can reject a
    /// provenance copy before cloning boxed payloads or allocating tree
    /// nodes.
    pub(crate) fn retained_byte_bound(&self) -> Option<usize> {
        let mut bytes = Self::retained_byte_base_bound()?;
        let row_bytes = |row: &GuardRowId| row.shared_payload_bytes();
        let slice_bytes = |length: usize, element_size: usize| length.checked_mul(element_size);
        let mut add = |payload: usize| -> Option<()> {
            bytes = bytes.checked_add(payload)?;
            Some(())
        };

        match self {
            Self::RelationConditionAttached { row }
            | Self::ShiftOperatorConditionAttached { row }
            | Self::ShiftOperatorInputTermDenominator { row }
            | Self::ShiftOperatorCollectedTermDenominator { row }
            | Self::ShiftOperatorFromRelationAdapter { row }
            | Self::ShiftOperatorToRelationAdapter { row } => add(row_bytes(row))?,
            Self::RelationInputTermDenominator { row, shift }
            | Self::RelationCollectedTermDenominator { row, shift }
            | Self::RelationPartialSpecializationTermDenominator { row, shift }
            | Self::RelationResidualUnitAffineSubstitutionTermDenominator { row, shift, .. }
            | Self::RelationResidualAffineBranchSubstitutionTermDenominator {
                row, shift, ..
            } => {
                add(row_bytes(row))?;
                add(slice_bytes(shift.len(), size_of::<i64>())?)?;
            }
            Self::RelationScaleFactorDenominator {
                target_row,
                source_row,
            } => {
                add(row_bytes(target_row))?;
                add(row_bytes(source_row))?;
            }
            Self::RelationTranslation {
                source_row,
                target_row,
                offset,
            } => {
                add(row_bytes(source_row))?;
                add(row_bytes(target_row))?;
                add(slice_bytes(offset.len(), size_of::<i64>())?)?;
            }
            Self::RelationAffineFreeRecentering {
                source_row,
                target_row,
                coefficient_offset,
                key_center,
            } => {
                add(row_bytes(source_row))?;
                add(row_bytes(target_row))?;
                add(slice_bytes(coefficient_offset.len(), size_of::<i64>())?)?;
                add(slice_bytes(key_center.len(), size_of::<i64>())?)?;
            }
            Self::RelationIndexPermutation {
                source_row,
                target_row,
                source_to_target,
            } => {
                add(row_bytes(source_row))?;
                add(row_bytes(target_row))?;
                add(slice_bytes(source_to_target.len(), size_of::<usize>())?)?;
            }
            Self::IndexTranslation { offset }
            | Self::IndexSpecialization { assignment: offset } => {
                add(slice_bytes(offset.len(), size_of::<i64>())?)?;
            }
            Self::IndexPermutation { source_to_target }
            | Self::VerifiedSymmetryMapDomain {
                source_to_target, ..
            } => add(slice_bytes(source_to_target.len(), size_of::<usize>())?)?,
            Self::PartialIndexSpecialization { assignments } => {
                add(slice_bytes(assignments.len(), size_of::<(usize, i64)>())?)?;
            }
            Self::RelationResidualUnitAffineSubstitution {
                source_row,
                target_row,
                ..
            }
            | Self::RelationResidualAffineBranchSubstitution {
                source_row,
                target_row,
                ..
            } => {
                add(row_bytes(source_row))?;
                add(row_bytes(target_row))?;
            }
            Self::FamilyInputCoefficientDenominator { .. }
            | Self::FamilyBasisDeterminantNumerator
            | Self::PowerShiftSupport { .. }
            | Self::GuardedDivisionDividendDenominator
            | Self::GuardedDivisionDivisorDenominator
            | Self::GuardedDivisionDivisorNumerator
            | Self::ExplicitRelationCondition
            | Self::GeneratedAffineSealedCondition
            | Self::ResidualUnitAffineIndexSubstitution { .. }
            | Self::ResidualAffineBranchNonzeroGuardSubstitution { .. }
            | Self::CoefficientSpecializationDenominator
            | Self::CoefficientPartialSpecializationDenominator
            | Self::CoefficientResidualUnitAffineSubstitutionDenominator { .. }
            | Self::QuotientPivotNumerator
            | Self::ConcreteQuotientEliminationPivotNumerator { .. }
            | Self::ExplicitShiftOperatorCondition
            | Self::GeneratedAffineGroupRecentering { .. }
            | Self::GeneratedAffineGroupTopReductionCoefficientDenominator { .. } => {}
        }
        Some(bytes)
    }

    pub(crate) fn write_stable(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::FamilyInputCoefficientDenominator { location } => {
                writer.write_str("family-input-coefficient-denominator:")?;
                location.write_stable(writer)
            }
            Self::FamilyBasisDeterminantNumerator => {
                writer.write_str("family-basis-determinant-numerator")
            }
            Self::PowerShiftSupport { denominator } => {
                write!(writer, "power-shift-support:{denominator}")
            }
            Self::GuardedDivisionDividendDenominator => {
                writer.write_str("guarded-division-dividend-denominator")
            }
            Self::GuardedDivisionDivisorDenominator => {
                writer.write_str("guarded-division-divisor-denominator")
            }
            Self::GuardedDivisionDivisorNumerator => {
                writer.write_str("guarded-division-divisor-numerator")
            }
            Self::ExplicitRelationCondition => writer.write_str("explicit-relation-condition"),
            Self::GeneratedAffineSealedCondition => {
                writer.write_str("generated-affine-sealed-condition")
            }
            Self::RelationConditionAttached { row } => {
                writer.write_str("relation-condition-attached:")?;
                row.write_stable(writer)
            }
            Self::RelationInputTermDenominator { row, shift } => {
                writer.write_str("relation-input-term-denominator:")?;
                row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, shift, ",")?;
                writer.write_str("]")
            }
            Self::RelationCollectedTermDenominator { row, shift } => {
                writer.write_str("relation-collected-term-denominator:")?;
                row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, shift, ",")?;
                writer.write_str("]")
            }
            Self::RelationScaleFactorDenominator {
                target_row,
                source_row,
            } => {
                writer.write_str("relation-scale-factor-denominator:")?;
                target_row.write_stable(writer)?;
                writer.write_str(":")?;
                source_row.write_stable(writer)
            }
            Self::RelationTranslation {
                source_row,
                target_row,
                offset,
            } => {
                writer.write_str("relation-translation:")?;
                source_row.write_stable(writer)?;
                writer.write_str(":")?;
                target_row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, offset, ",")?;
                writer.write_str("]")
            }
            Self::RelationAffineFreeRecentering {
                source_row,
                target_row,
                coefficient_offset,
                key_center,
            } => {
                writer.write_str("relation-affine-free-recentering:")?;
                source_row.write_stable(writer)?;
                writer.write_str(":")?;
                target_row.write_stable(writer)?;
                writer.write_str(":coefficient-offset:[")?;
                write_joined(writer, coefficient_offset, ",")?;
                writer.write_str("]:key-center:[")?;
                write_joined(writer, key_center, ",")?;
                writer.write_str("]")
            }
            Self::RelationIndexPermutation {
                source_row,
                target_row,
                source_to_target,
            } => {
                writer.write_str("relation-index-permutation:")?;
                source_row.write_stable(writer)?;
                writer.write_str(":")?;
                target_row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, source_to_target, ",")?;
                writer.write_str("]")
            }
            Self::IndexTranslation { offset } => {
                writer.write_str("index-translation:[")?;
                write_joined(writer, offset, ",")?;
                writer.write_str("]")
            }
            Self::IndexPermutation { source_to_target } => {
                writer.write_str("index-permutation:[")?;
                write_joined(writer, source_to_target, ",")?;
                writer.write_str("]")
            }
            Self::VerifiedSymmetryMapDomain {
                source_to_target,
                condition_ordinal,
            } => {
                writer.write_str("verified-symmetry-map-domain:[")?;
                write_joined(writer, source_to_target, ",")?;
                write!(writer, "]:{condition_ordinal}")
            }
            Self::IndexSpecialization { assignment } => {
                writer.write_str("index-specialization:[")?;
                write_joined(writer, assignment, ",")?;
                writer.write_str("]")
            }
            Self::PartialIndexSpecialization { assignments } => {
                writer.write_str("partial-index-specialization:[")?;
                for (ordinal, (position, value)) in assignments.iter().enumerate() {
                    if ordinal != 0 {
                        writer.write_str(",")?;
                    }
                    write!(writer, "{position}={value}")?;
                }
                writer.write_str("]")
            }
            Self::ResidualUnitAffineIndexSubstitution {
                source_case,
                predicate_ordinal,
                bound_position,
            } => write!(
                writer,
                "residual-unit-affine-index-substitution:{source_case}:{predicate_ordinal}:{bound_position}"
            ),
            Self::ResidualAffineBranchNonzeroGuardSubstitution {
                source_case,
                source_work_item_ordinal,
                ready_terminal_ordinal,
                structural_locus_ordinal,
            } => write!(
                writer,
                "residual-affine-branch-nonzero-guard-substitution:{source_case}:{source_work_item_ordinal}:{ready_terminal_ordinal}:{structural_locus_ordinal}"
            ),
            Self::CoefficientSpecializationDenominator => {
                writer.write_str("coefficient-specialization-denominator")
            }
            Self::CoefficientPartialSpecializationDenominator => {
                writer.write_str("coefficient-partial-specialization-denominator")
            }
            Self::RelationPartialSpecializationTermDenominator { row, shift } => {
                writer.write_str("relation-partial-specialization-term-denominator:")?;
                row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, shift, ",")?;
                writer.write_str("]")
            }
            Self::CoefficientResidualUnitAffineSubstitutionDenominator {
                source_case,
                predicate_ordinal,
                bound_position,
            } => write!(
                writer,
                "coefficient-residual-unit-affine-substitution-denominator:{source_case}:{predicate_ordinal}:{bound_position}"
            ),
            Self::RelationResidualUnitAffineSubstitutionTermDenominator {
                row,
                shift,
                source_case,
                predicate_ordinal,
                bound_position,
            } => {
                writer.write_str("relation-residual-unit-affine-substitution-term-denominator:")?;
                row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, shift, ",")?;
                write!(
                    writer,
                    "]:{source_case}:{predicate_ordinal}:{bound_position}"
                )
            }
            Self::RelationResidualUnitAffineSubstitution {
                source_row,
                target_row,
                source_case,
                predicate_ordinal,
                bound_position,
            } => {
                writer.write_str("relation-residual-unit-affine-substitution:")?;
                source_row.write_stable(writer)?;
                writer.write_str(":")?;
                target_row.write_stable(writer)?;
                write!(
                    writer,
                    ":{source_case}:{predicate_ordinal}:{bound_position}"
                )
            }
            Self::RelationResidualAffineBranchSubstitutionTermDenominator {
                row,
                shift,
                source_case,
                source_work_item_ordinal,
                ready_terminal_ordinal,
            } => {
                writer
                    .write_str("relation-residual-affine-branch-substitution-term-denominator:")?;
                row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, shift, ",")?;
                write!(
                    writer,
                    "]:{source_case}:{source_work_item_ordinal}:{ready_terminal_ordinal}"
                )
            }
            Self::RelationResidualAffineBranchSubstitution {
                source_row,
                target_row,
                source_case,
                source_work_item_ordinal,
                ready_terminal_ordinal,
            } => {
                writer.write_str("relation-residual-affine-branch-substitution:")?;
                source_row.write_stable(writer)?;
                writer.write_str(":")?;
                target_row.write_stable(writer)?;
                write!(
                    writer,
                    ":{source_case}:{source_work_item_ordinal}:{ready_terminal_ordinal}"
                )
            }
            Self::QuotientPivotNumerator => writer.write_str("quotient-pivot-numerator"),
            Self::ConcreteQuotientEliminationPivotNumerator { pivot } => {
                write!(
                    writer,
                    "concrete-quotient-elimination-pivot-numerator:{pivot}"
                )
            }
            Self::ExplicitShiftOperatorCondition => {
                writer.write_str("explicit-shift-operator-condition")
            }
            Self::ShiftOperatorConditionAttached { row } => {
                writer.write_str("shift-operator-condition-attached:")?;
                row.write_stable(writer)
            }
            Self::ShiftOperatorInputTermDenominator { row } => {
                writer.write_str("shift-operator-input-term-denominator:")?;
                row.write_stable(writer)
            }
            Self::ShiftOperatorCollectedTermDenominator { row } => {
                writer.write_str("shift-operator-collected-term-denominator:")?;
                row.write_stable(writer)
            }
            Self::ShiftOperatorFromRelationAdapter { row } => {
                writer.write_str("shift-operator-from-relation-adapter:")?;
                row.write_stable(writer)
            }
            Self::ShiftOperatorToRelationAdapter { row } => {
                writer.write_str("shift-operator-to-relation-adapter:")?;
                row.write_stable(writer)
            }
            Self::GeneratedAffineGroupRecentering {
                solve_group_ordinal,
                database_epoch,
                event_ordinal,
            } => write!(
                writer,
                "generated-affine-group-recentering:solve-group-ordinal={solve_group_ordinal}:database-epoch={database_epoch}:event-ordinal={event_ordinal}"
            ),
            Self::GeneratedAffineGroupTopReductionCoefficientDenominator {
                solve_group_ordinal,
                database_epoch,
                event_ordinal,
                operation_ordinal,
                term_ordinal,
                pivot_normalization,
            } => write!(
                writer,
                "generated-affine-group-top-reduction-coefficient-denominator:solve-group-ordinal={solve_group_ordinal}:database-epoch={database_epoch}:event-ordinal={event_ordinal}:operation-ordinal={operation_ordinal}:term-ordinal={term_ordinal}:pivot-normalization={pivot_normalization}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GuardOrigin, GuardRowId};
    use std::sync::Arc;

    #[test]
    fn generated_affine_sealed_origin_has_no_private_retained_payload() {
        let origin = GuardOrigin::GeneratedAffineSealedCondition;

        assert_eq!(origin.stable_string(), "generated-affine-sealed-condition");
        assert_eq!(
            origin.retained_byte_bound(),
            GuardOrigin::retained_byte_base_bound()
        );
    }

    #[test]
    fn generated_affine_group_recentering_is_a_locator_only_stable_atom() {
        let origin = GuardOrigin::GeneratedAffineGroupRecentering {
            solve_group_ordinal: 17,
            database_epoch: 23,
            event_ordinal: 31,
        };

        assert_eq!(
            origin.stable_string(),
            "generated-affine-group-recentering:solve-group-ordinal=17:database-epoch=23:event-ordinal=31"
        );
        assert_eq!(
            origin.retained_byte_bound(),
            GuardOrigin::retained_byte_base_bound()
        );
        let debug = format!("{origin:?}");
        for forbidden in [
            "coefficient_offset",
            "key_center",
            "hash",
            "row_label",
            "coefficient:",
            "geometry",
        ] {
            assert!(!debug.contains(forbidden), "leaked {forbidden}: {debug}");
        }
    }

    #[test]
    fn generated_affine_top_reduction_denominator_is_a_locator_only_stable_atom() {
        let origin = GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
            solve_group_ordinal: 17,
            database_epoch: 23,
            event_ordinal: 31,
            operation_ordinal: 37,
            term_ordinal: 41,
            pivot_normalization: true,
        };

        assert_eq!(
            origin.stable_string(),
            "generated-affine-group-top-reduction-coefficient-denominator:solve-group-ordinal=17:database-epoch=23:event-ordinal=31:operation-ordinal=37:term-ordinal=41:pivot-normalization=true"
        );
        assert_eq!(
            origin.retained_byte_bound(),
            GuardOrigin::retained_byte_base_bound()
        );
        let debug = format!("{origin:?}");
        for forbidden in [
            "physical_key",
            "coefficient_offset",
            "key_center",
            "shift",
            "row_label",
            "coefficient:",
            "geometry",
        ] {
            assert!(!debug.contains(forbidden), "leaked {forbidden}: {debug}");
        }
    }

    #[test]
    fn residual_affine_branch_relation_origin_is_stable_and_preflight_exact() {
        let source_row = GuardRowId::Derived {
            label: Arc::from("source-row"),
        };
        let target_row = GuardRowId::Derived {
            label: Arc::from("target-row"),
        };
        let expected_bound = GuardOrigin::residual_affine_branch_relation_retained_byte_bound(
            &source_row,
            &target_row,
        );
        let origin = GuardOrigin::RelationResidualAffineBranchSubstitution {
            source_row,
            target_row,
            source_case: 17,
            source_work_item_ordinal: 3,
            ready_terminal_ordinal: 11,
        };

        assert_eq!(
            origin.stable_string(),
            "relation-residual-affine-branch-substitution:derived:10:source-row:derived:10:target-row:17:3:11"
        );
        assert_eq!(origin.retained_byte_bound(), expected_bound);
    }

    #[test]
    fn residual_affine_branch_term_denominator_origin_is_stable_and_preflight_exact() {
        let row = GuardRowId::Derived {
            label: Arc::from("branch-row"),
        };
        let shift = vec![-2, 0, 5].into_boxed_slice();
        let expected_bound =
            GuardOrigin::residual_affine_branch_term_denominator_retained_byte_bound(
                &row,
                shift.len(),
            );
        let origin = GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
            row,
            shift,
            source_case: 23,
            source_work_item_ordinal: 7,
            ready_terminal_ordinal: 19,
        };

        assert_eq!(
            origin.stable_string(),
            "relation-residual-affine-branch-substitution-term-denominator:derived:10:branch-row:[-2,0,5]:23:7:19"
        );
        assert_eq!(origin.retained_byte_bound(), expected_bound);
    }

    #[test]
    fn residual_affine_branch_term_denominator_preflight_rejects_shift_overflow() {
        let row = GuardRowId::OrdinaryIbp {
            contraction_momentum: 1,
            differentiated_loop: 2,
        };

        assert_eq!(
            GuardOrigin::residual_affine_branch_term_denominator_retained_byte_bound(
                &row,
                usize::MAX,
            ),
            None
        );
    }
}
