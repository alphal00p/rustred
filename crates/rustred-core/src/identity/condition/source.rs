use std::fmt;

use crate::family::CoefficientLocation;

use super::super::row::RowId;

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

/// One atomic reason why an identity polynomial must remain nonzero.
///
/// Sources are flat and refer directly to the real identity row type. Equal
/// polynomials therefore merge deterministically without an adapter row ID or
/// recursive provenance tree.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IdentityConditionSource {
    FamilyInputCoefficientDenominator {
        location: CoefficientLocation,
    },
    FamilyBasisDeterminantNumerator,
    RelationConditionAttached {
        row: RowId,
    },
    RelationInputTermDenominator {
        row: RowId,
        shift: Box<[i64]>,
    },
    RelationCollectedTermDenominator {
        row: RowId,
        shift: Box<[i64]>,
    },
    RelationScaleFactorDenominator {
        target_row: RowId,
        source_row: RowId,
    },
    RelationTranslation {
        source_row: RowId,
        target_row: RowId,
        offset: Box<[i64]>,
    },
    IndexTranslation {
        offset: Box<[i64]>,
    },
}

impl IdentityConditionSource {
    /// Version-stable identity used in user-facing output and proof payloads.
    pub fn stable_string(&self) -> String {
        let mut output = String::new();
        self.write_stable(&mut output)
            .expect("writing identity-condition source to String cannot fail");
        output
    }

    fn write_stable(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::FamilyInputCoefficientDenominator { location } => {
                writer.write_str("family-input-coefficient-denominator:")?;
                location.write_stable(writer)
            }
            Self::FamilyBasisDeterminantNumerator => {
                writer.write_str("family-basis-determinant-numerator")
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
            Self::IndexTranslation { offset } => {
                writer.write_str("index-translation:[")?;
                write_joined(writer, offset, ",")?;
                writer.write_str("]")
            }
        }
    }
}
