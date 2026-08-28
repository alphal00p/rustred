//! Flat, deterministic provenance for exceptional nonzero loci.
//!
//! Guard origins are deliberately non-recursive.  Derivations accumulate a
//! set of atomic facts instead of nesting one provenance tree inside another;
//! equal polynomial conditions can therefore merge without making their
//! representation depend on the order in which algebraic operations ran.

use std::fmt;
use std::sync::Arc;

use crate::generic_family::CoefficientLocation;

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
    /// Original denominator of a coefficient specialized separately from a
    /// relation.
    CoefficientSpecializationDenominator,
}

impl GuardOrigin {
    /// Version-stable identity used in user-facing output and proof payloads.
    pub fn stable_string(&self) -> String {
        let mut output = String::new();
        self.write_stable(&mut output)
            .expect("writing guard-origin provenance to String cannot fail");
        output
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
            Self::CoefficientSpecializationDenominator => {
                writer.write_str("coefficient-specialization-denominator")
            }
        }
    }
}
