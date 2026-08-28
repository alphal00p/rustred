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
    /// Version-stable identity used in user-facing output and proof payloads.
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
    /// Version-stable identity used in user-facing output and proof payloads.
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

    /// Affine index translation `n -> n + offset` applied to a guard.
    IndexTranslation { offset: Box<[i64]> },
    /// Exact integer specialization applied to a guard.
    IndexSpecialization { assignment: Box<[i64]> },
    /// Exact sparse integer specialization applied while the remaining index
    /// variables stay symbolic.  Entries are canonicalized by increasing
    /// index position before this provenance atom can be constructed.
    PartialIndexSpecialization { assignments: Box<[(usize, i64)]> },
    /// Original denominator of a coefficient specialized separately from a
    /// relation.
    CoefficientSpecializationDenominator,
    /// Original denominator of a coefficient after sparse index
    /// specialization and before fraction normalization can cancel it.
    CoefficientPartialSpecializationDenominator,
    /// Mapped pre-normalization denominator of one concrete term in a source
    /// row partially specialized on an equality locus.
    RelationPartialSpecializationTermDenominator { row: GuardRowId, shift: Box<[i64]> },
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

    pub(crate) fn relation_attached_retained_byte_bound(row: &GuardRowId) -> Option<usize> {
        Self::retained_byte_base_bound()?.checked_add(row.shared_payload_bytes())
    }

    /// Version-stable identity used in user-facing output and proof payloads.
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
            | Self::RelationPartialSpecializationTermDenominator { row, shift } => {
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
            Self::IndexTranslation { offset }
            | Self::IndexSpecialization { assignment: offset } => {
                add(slice_bytes(offset.len(), size_of::<i64>())?)?;
            }
            Self::PartialIndexSpecialization { assignments } => {
                add(slice_bytes(assignments.len(), size_of::<(usize, i64)>())?)?;
            }
            Self::FamilyInputCoefficientDenominator { .. }
            | Self::FamilyBasisDeterminantNumerator
            | Self::PowerShiftSupport { .. }
            | Self::GuardedDivisionDividendDenominator
            | Self::GuardedDivisionDivisorDenominator
            | Self::GuardedDivisionDivisorNumerator
            | Self::ExplicitRelationCondition
            | Self::CoefficientSpecializationDenominator
            | Self::CoefficientPartialSpecializationDenominator
            | Self::ExplicitShiftOperatorCondition => {}
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
            Self::IndexTranslation { offset } => {
                writer.write_str("index-translation:[")?;
                write_joined(writer, offset, ",")?;
                writer.write_str("]")
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
        }
    }
}
