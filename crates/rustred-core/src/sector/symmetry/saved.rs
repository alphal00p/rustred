//! Versioned, process-independent saved form of verified family maps.
//!
//! [`verify()`](super::verify()) is the proof boundary for momentum maps. A
//! [`SavedVerifiedMap`] records the complete output of one successful
//! verification (momentum, scalar-product and denominator maps, row actions,
//! determinants, Jacobian, merged nonzero conditions and statistics) so that a
//! caller can rebuild the identical [`VerifiedMap`] later without repeating
//! the exact algebra. [`SavedVerifiedMap::restore`] is a *trusted* restore: it
//! checks shapes, family dimensions and value-table membership, never the
//! algebra. Use it only for records whose identity and version the caller has
//! bound to the immutable data they were derived from (for example a route
//! preparation add-on of one fixed saved application); otherwise call
//! [`verify()`](super::verify()).
//!
//! Only constant (rational) coefficients are representable. Every map that
//! [`integral_transport::compile`](super::integral_transport::compile)
//! accepts for a vacuum family with numeric masses has only constant
//! coefficients; [`SavedVerifiedMap::capture`] fails explicitly otherwise.
//! Values are exact decimal integers, so the bytes do not depend on the
//! Symbolica symbol registration order of the producing process.
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::str::FromStr;

use symbolica::domains::integer::{Integer, Z};
use symbolica::domains::{EuclideanDomain, Ring};

use crate::algebra::{Coefficient, CoefficientContext, CoefficientPolynomial};
use crate::family::{CoefficientLocation, IntegralFamily};

use super::{
    CoefficientMatrix, ConditionSource, DenominatorAction, DenominatorMap, Jacobian, MomentumMap,
    NonZeroCondition, ScalarProductMap, Stats, VerifiedMap,
};

/// Version of the saved verified-map content, bumped by hand (to the date of
/// the change). Bump it whenever [`verify()`](super::verify()) can return a
/// different map for the same families and momentum map (other derived maps,
/// row actions, condition order or sources, statistics), when a coefficient
/// normal form changes, or when the record layout below changes. The bump
/// invalidates every saved record through the caller's version check.
pub const SAVED_VERIFIED_MAP_SEMANTICS: &str = "rustred-verified-map-2026-10-31";

/// A saved record could not be captured or restored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedMapError(String);

impl SavedMapError {
    fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }
}

impl fmt::Display for SavedMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SavedMapError {}

/// One exact rational value, `numerator / denominator` in lowest terms with a
/// positive denominator, as decimal strings.
#[derive(Clone, Debug, PartialEq, Eq, Hash, bincode::Encode, bincode::Decode)]
struct SavedRational {
    numerator: String,
    denominator: String,
}

/// Deduplicated values in first-occurrence order (deterministic for a
/// deterministic capture order).
#[derive(Default)]
pub struct SavedValueTableBuilder {
    values: Vec<SavedRational>,
    index: HashMap<SavedRational, u32>,
}

impl SavedValueTableBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn intern(&mut self, value: SavedRational) -> Result<u32, SavedMapError> {
        if let Some(id) = self.index.get(&value) {
            return Ok(*id);
        }
        let id = u32::try_from(self.values.len())
            .map_err(|_| SavedMapError::new("saved value table exceeds u32 entries"))?;
        self.values.push(value.clone());
        self.index.insert(value, id);
        Ok(id)
    }

    fn coefficient(&mut self, coefficient: &Coefficient) -> Result<u32, SavedMapError> {
        if !coefficient.numerator.is_constant() || !coefficient.denominator.is_constant() {
            return Err(SavedMapError::new(
                "saved verified maps support constant coefficients only",
            ));
        }
        self.intern(SavedRational {
            numerator: coefficient.numerator.get_constant().to_string(),
            denominator: coefficient.denominator.get_constant().to_string(),
        })
    }

    fn polynomial(&mut self, polynomial: &CoefficientPolynomial) -> Result<u32, SavedMapError> {
        if !polynomial.is_constant() {
            return Err(SavedMapError::new(
                "saved verified maps support constant nonzero conditions only",
            ));
        }
        self.intern(SavedRational {
            numerator: polynomial.get_constant().to_string(),
            denominator: "1".into(),
        })
    }

    fn matrix(&mut self, matrix: &CoefficientMatrix) -> Result<SavedMatrix, SavedMapError> {
        Ok(SavedMatrix {
            rows: index(matrix.rows)?,
            columns: index(matrix.columns)?,
            entries: matrix
                .entries()
                .iter()
                .map(|entry| self.coefficient(entry))
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn finish(self) -> SavedValueTable {
        SavedValueTable {
            values: self.values,
        }
    }
}

/// The exact values referenced by saved maps.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct SavedValueTable {
    values: Vec<SavedRational>,
}

impl SavedValueTable {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Build every value once in `context` (checked lowest terms, positive
    /// denominator). Restores then only clone these coefficients.
    pub fn materialize(&self, context: &CoefficientContext) -> Result<SavedValues, SavedMapError> {
        let template = &context.template().numerator;
        let mut coefficients = Vec::with_capacity(self.values.len());
        for value in &self.values {
            let numerator = Integer::from_str(&value.numerator)
                .map_err(|_| SavedMapError::new("invalid saved value numerator"))?;
            let denominator = Integer::from_str(&value.denominator)
                .map_err(|_| SavedMapError::new("invalid saved value denominator"))?;
            if denominator <= Integer::zero() {
                return Err(SavedMapError::new("saved value denominator is not positive"));
            }
            let coprime = if Z.is_zero(&numerator) {
                Z.is_one(&denominator)
            } else {
                Z.is_one(&Z.gcd(&numerator, &denominator))
            };
            if !coprime {
                return Err(SavedMapError::new("saved value is not in lowest terms"));
            }
            coefficients.push(Coefficient {
                numerator: template.constant(numerator),
                denominator: template.constant(denominator),
            });
        }
        Ok(SavedValues { coefficients })
    }
}

/// Materialized values of one [`SavedValueTable`] in one coefficient context.
pub struct SavedValues {
    coefficients: Vec<Coefficient>,
}

impl SavedValues {
    fn coefficient(&self, id: u32) -> Result<Coefficient, SavedMapError> {
        self.coefficients
            .get(id as usize)
            .cloned()
            .ok_or_else(|| SavedMapError::new("saved value index is outside its table"))
    }

    fn polynomial(&self, id: u32) -> Result<CoefficientPolynomial, SavedMapError> {
        let value = self
            .coefficients
            .get(id as usize)
            .ok_or_else(|| SavedMapError::new("saved value index is outside its table"))?;
        if !value.denominator.is_one() {
            return Err(SavedMapError::new("saved condition polynomial is not integral"));
        }
        Ok(value.numerator.clone())
    }

    fn matrix(
        &self,
        matrix: &SavedMatrix,
        name: &str,
        rows: usize,
        columns: usize,
    ) -> Result<CoefficientMatrix, SavedMapError> {
        if matrix.rows as usize != rows
            || matrix.columns as usize != columns
            || matrix.entries.len() != rows * columns
        {
            return Err(SavedMapError::new(format!(
                "saved matrix {name} differs from the family dimensions"
            )));
        }
        let entries = matrix
            .entries
            .iter()
            .map(|id| self.coefficient(*id))
            .collect::<Result<Vec<_>, _>>()?;
        CoefficientMatrix::try_new(rows, columns, entries)
            .map_err(|error| SavedMapError::new(format!("saved matrix {name}: {error:?}")))
    }

    fn vector(&self, ids: &[u32], name: &str, len: usize) -> Result<Box<[Coefficient]>, SavedMapError> {
        if ids.len() != len {
            return Err(SavedMapError::new(format!(
                "saved vector {name} differs from the family dimensions"
            )));
        }
        ids.iter().map(|id| self.coefficient(*id)).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct SavedMatrix {
    rows: u32,
    columns: u32,
    entries: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum SavedRowAction {
    Monomial { target: u32, scale: u32 },
    Affine,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum SavedJacobian {
    Unit(i8),
    FormalDeterminantPower(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum SavedLocation {
    Dimension,
    DenominatorConstant(u32),
    DenominatorCoefficient(u32, u32),
    ExternalGram(u32, u32),
    PowerShift(u32),
    BasisDeterminantNumerator,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum SavedConditionSource {
    SourceFamily(SavedLocation),
    TargetFamily(SavedLocation),
    MomentumMapDenominator { matrix: u8, row: u32, column: u32 },
    LoopMapDeterminantNumerator,
    ExternalMapDeterminantNumerator,
    DenominatorScaleNumerator { source: u32, target: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct SavedCondition {
    polynomial: u32,
    sources: Vec<SavedConditionSource>,
}

/// The complete output of one successful [`verify()`](super::verify()).
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct SavedVerifiedMap {
    momentum: [SavedMatrix; 3],
    scalar_constant: Vec<u32>,
    scalar_linear: SavedMatrix,
    denominator_constant: Vec<u32>,
    denominator_linear: SavedMatrix,
    row_actions: Vec<SavedRowAction>,
    loop_determinant: u32,
    external_determinant: u32,
    jacobian: SavedJacobian,
    conditions: Vec<SavedCondition>,
    stats: [u64; 13],
}

const MOMENTUM_MATRICES: [&str; 3] = ["A", "B", "C"];

fn index(value: usize) -> Result<u32, SavedMapError> {
    u32::try_from(value).map_err(|_| SavedMapError::new("saved map index exceeds u32"))
}

fn location(location: &CoefficientLocation) -> Result<SavedLocation, SavedMapError> {
    Ok(match location {
        CoefficientLocation::Dimension => SavedLocation::Dimension,
        CoefficientLocation::DenominatorConstant { denominator } => {
            SavedLocation::DenominatorConstant(index(*denominator)?)
        }
        CoefficientLocation::DenominatorCoefficient {
            denominator,
            coordinate,
        } => SavedLocation::DenominatorCoefficient(index(*denominator)?, index(*coordinate)?),
        CoefficientLocation::ExternalGram { row, column } => {
            SavedLocation::ExternalGram(index(*row)?, index(*column)?)
        }
        CoefficientLocation::PowerShift { denominator } => {
            SavedLocation::PowerShift(index(*denominator)?)
        }
        CoefficientLocation::BasisDeterminantNumerator => SavedLocation::BasisDeterminantNumerator,
    })
}

fn restore_location(location: &SavedLocation) -> CoefficientLocation {
    match *location {
        SavedLocation::Dimension => CoefficientLocation::Dimension,
        SavedLocation::DenominatorConstant(denominator) => {
            CoefficientLocation::DenominatorConstant {
                denominator: denominator as usize,
            }
        }
        SavedLocation::DenominatorCoefficient(denominator, coordinate) => {
            CoefficientLocation::DenominatorCoefficient {
                denominator: denominator as usize,
                coordinate: coordinate as usize,
            }
        }
        SavedLocation::ExternalGram(row, column) => CoefficientLocation::ExternalGram {
            row: row as usize,
            column: column as usize,
        },
        SavedLocation::PowerShift(denominator) => CoefficientLocation::PowerShift {
            denominator: denominator as usize,
        },
        SavedLocation::BasisDeterminantNumerator => CoefficientLocation::BasisDeterminantNumerator,
    }
}

fn source(source: &ConditionSource) -> Result<SavedConditionSource, SavedMapError> {
    Ok(match source {
        ConditionSource::SourceFamily(at) => SavedConditionSource::SourceFamily(location(at)?),
        ConditionSource::TargetFamily(at) => SavedConditionSource::TargetFamily(location(at)?),
        ConditionSource::MomentumMapDenominator {
            matrix,
            row,
            column,
        } => SavedConditionSource::MomentumMapDenominator {
            matrix: MOMENTUM_MATRICES
                .iter()
                .position(|name| name == matrix)
                .ok_or_else(|| SavedMapError::new("unknown momentum matrix condition source"))?
                as u8,
            row: index(*row)?,
            column: index(*column)?,
        },
        ConditionSource::LoopMapDeterminantNumerator => {
            SavedConditionSource::LoopMapDeterminantNumerator
        }
        ConditionSource::ExternalMapDeterminantNumerator => {
            SavedConditionSource::ExternalMapDeterminantNumerator
        }
        ConditionSource::DenominatorScaleNumerator {
            source_denominator,
            target_denominator,
        } => SavedConditionSource::DenominatorScaleNumerator {
            source: index(*source_denominator)?,
            target: index(*target_denominator)?,
        },
    })
}

fn restore_source(source: &SavedConditionSource) -> Result<ConditionSource, SavedMapError> {
    Ok(match source {
        SavedConditionSource::SourceFamily(at) => {
            ConditionSource::SourceFamily(restore_location(at))
        }
        SavedConditionSource::TargetFamily(at) => {
            ConditionSource::TargetFamily(restore_location(at))
        }
        SavedConditionSource::MomentumMapDenominator {
            matrix,
            row,
            column,
        } => ConditionSource::MomentumMapDenominator {
            matrix: MOMENTUM_MATRICES
                .get(*matrix as usize)
                .ok_or_else(|| SavedMapError::new("unknown saved momentum matrix"))?,
            row: *row as usize,
            column: *column as usize,
        },
        SavedConditionSource::LoopMapDeterminantNumerator => {
            ConditionSource::LoopMapDeterminantNumerator
        }
        SavedConditionSource::ExternalMapDeterminantNumerator => {
            ConditionSource::ExternalMapDeterminantNumerator
        }
        SavedConditionSource::DenominatorScaleNumerator { source, target } => {
            ConditionSource::DenominatorScaleNumerator {
                source_denominator: *source as usize,
                target_denominator: *target as usize,
            }
        }
    })
}

fn stats_array(stats: &Stats) -> [u64; 13] {
    [
        stats.matrix_entries,
        stats.exact_operations,
        stats.symbolica_exact_operations,
        stats.symbolica_admitted_exact_operations,
        stats.symbolica_largest_matrix_entries,
        stats.symbolica_peak_live_matrix_entries,
        stats.symbolica_input_retained_bytes,
        stats.symbolica_output_retained_bytes,
        stats.symbolica_determinant_calls,
        stats.symbolica_product_calls,
        stats.symbolica_transpose_calls,
        stats.nonzero_conditions,
        stats.condition_sources,
    ]
    .map(|value| value as u64)
}

fn restore_stats(values: &[u64; 13]) -> Result<Stats, SavedMapError> {
    let mut fields = [0usize; 13];
    for (field, value) in fields.iter_mut().zip(values) {
        *field = usize::try_from(*value)
            .map_err(|_| SavedMapError::new("saved statistic exceeds usize"))?;
    }
    let [
        matrix_entries,
        exact_operations,
        symbolica_exact_operations,
        symbolica_admitted_exact_operations,
        symbolica_largest_matrix_entries,
        symbolica_peak_live_matrix_entries,
        symbolica_input_retained_bytes,
        symbolica_output_retained_bytes,
        symbolica_determinant_calls,
        symbolica_product_calls,
        symbolica_transpose_calls,
        nonzero_conditions,
        condition_sources,
    ] = fields;
    Ok(Stats {
        matrix_entries,
        exact_operations,
        symbolica_exact_operations,
        symbolica_admitted_exact_operations,
        symbolica_largest_matrix_entries,
        symbolica_peak_live_matrix_entries,
        symbolica_input_retained_bytes,
        symbolica_output_retained_bytes,
        symbolica_determinant_calls,
        symbolica_product_calls,
        symbolica_transpose_calls,
        nonzero_conditions,
        condition_sources,
    })
}

impl SavedVerifiedMap {
    /// Record a verified map; values are interned into `table`.
    pub fn capture(
        map: &VerifiedMap,
        table: &mut SavedValueTableBuilder,
    ) -> Result<Self, SavedMapError> {
        let momentum = [
            table.matrix(&map.momentum.loop_linear)?,
            table.matrix(&map.momentum.loop_external)?,
            table.matrix(&map.momentum.external_linear)?,
        ];
        let scalar_constant = map
            .scalar_products
            .constant
            .iter()
            .map(|value| table.coefficient(value))
            .collect::<Result<_, _>>()?;
        let scalar_linear = table.matrix(&map.scalar_products.linear)?;
        let denominator_constant = map
            .denominators
            .constant
            .iter()
            .map(|value| table.coefficient(value))
            .collect::<Result<_, _>>()?;
        let denominator_linear = table.matrix(&map.denominators.linear)?;
        let row_actions = map
            .row_actions
            .iter()
            .map(|action| {
                Ok(match action {
                    DenominatorAction::Monomial { target, scale } => SavedRowAction::Monomial {
                        target: index(*target)?,
                        scale: table.coefficient(scale)?,
                    },
                    DenominatorAction::Affine => SavedRowAction::Affine,
                })
            })
            .collect::<Result<_, SavedMapError>>()?;
        let loop_determinant = table.coefficient(&map.loop_determinant)?;
        let external_determinant = table.coefficient(&map.external_determinant)?;
        let jacobian = match &map.jacobian {
            Jacobian::Unit { determinant_sign } => SavedJacobian::Unit(*determinant_sign),
            Jacobian::FormalDeterminantPower { determinant } => {
                SavedJacobian::FormalDeterminantPower(table.coefficient(determinant)?)
            }
        };
        let conditions = map
            .nonzero_conditions
            .iter()
            .map(|condition| {
                Ok(SavedCondition {
                    polynomial: table.polynomial(condition.polynomial())?,
                    sources: condition
                        .sources()
                        .iter()
                        .map(source)
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<_, SavedMapError>>()?;
        Ok(Self {
            momentum,
            scalar_constant,
            scalar_linear,
            denominator_constant,
            denominator_linear,
            row_actions,
            loop_determinant,
            external_determinant,
            jacobian,
            conditions,
            stats: stats_array(&map.stats),
        })
    }

    /// Rebuild the recorded map between `source` and `target` without exact
    /// replay. `values` must be materialized in the source family's context.
    pub fn restore(
        &self,
        source_family: &IntegralFamily,
        target_family: &IntegralFamily,
        values: &SavedValues,
    ) -> Result<VerifiedMap, SavedMapError> {
        if source_family.loop_count() != target_family.loop_count()
            || source_family.external_count() != target_family.external_count()
            || source_family.denominator_count() != target_family.denominator_count()
            || !source_family
                .coefficient_context()
                .has_same_variable_map(target_family.coefficient_context())
        {
            return Err(SavedMapError::new(
                "saved map families have different dimensions or coefficient contexts",
            ));
        }
        let loops = source_family.loop_count();
        let externals = source_family.external_count();
        let source_count = source_family.denominator_count();
        let target_count = target_family.denominator_count();
        let momentum = MomentumMap::new(
            values.matrix(&self.momentum[0], "A", loops, loops)?,
            values.matrix(&self.momentum[1], "B", loops, externals)?,
            values.matrix(&self.momentum[2], "C", externals, externals)?,
        );
        let scalar_products = ScalarProductMap {
            constant: values.vector(&self.scalar_constant, "scalar constant", source_count)?,
            linear: values.matrix(
                &self.scalar_linear,
                "scalar linear",
                source_count,
                target_count,
            )?,
        };
        let denominators = DenominatorMap {
            constant: values.vector(
                &self.denominator_constant,
                "denominator constant",
                source_count,
            )?,
            linear: values.matrix(
                &self.denominator_linear,
                "denominator linear",
                source_count,
                target_count,
            )?,
        };
        if self.row_actions.len() != source_count {
            return Err(SavedMapError::new(
                "saved row actions differ from the family dimensions",
            ));
        }
        let row_actions = self
            .row_actions
            .iter()
            .map(|action| {
                Ok(match action {
                    SavedRowAction::Monomial { target, scale } => {
                        if *target as usize >= target_count {
                            return Err(SavedMapError::new(
                                "saved row action target is outside the family",
                            ));
                        }
                        DenominatorAction::Monomial {
                            target: *target as usize,
                            scale: values.coefficient(*scale)?,
                        }
                    }
                    SavedRowAction::Affine => DenominatorAction::Affine,
                })
            })
            .collect::<Result<Box<[_]>, SavedMapError>>()?;
        let jacobian = match &self.jacobian {
            SavedJacobian::Unit(sign) if *sign == 1 || *sign == -1 => Jacobian::Unit {
                determinant_sign: *sign,
            },
            SavedJacobian::Unit(_) => {
                return Err(SavedMapError::new("invalid saved Jacobian sign"));
            }
            SavedJacobian::FormalDeterminantPower(id) => Jacobian::FormalDeterminantPower {
                determinant: values.coefficient(*id)?,
            },
        };
        let nonzero_conditions = self
            .conditions
            .iter()
            .map(|condition| {
                let sources = condition
                    .sources
                    .iter()
                    .map(restore_source)
                    .collect::<Result<BTreeSet<_>, _>>()?;
                if sources.is_empty() || sources.len() != condition.sources.len() {
                    return Err(SavedMapError::new(
                        "saved condition sources are empty or repeated",
                    ));
                }
                Ok(NonZeroCondition::from_parts(
                    values.polynomial(condition.polynomial)?,
                    sources,
                ))
            })
            .collect::<Result<Box<[_]>, SavedMapError>>()?;
        Ok(VerifiedMap {
            source_family_fingerprint: source_family.fingerprint_owner(),
            target_family_fingerprint: target_family.fingerprint_owner(),
            momentum,
            scalar_products,
            denominators,
            row_actions,
            loop_determinant: values.coefficient(self.loop_determinant)?,
            external_determinant: values.coefficient(self.external_determinant)?,
            jacobian,
            nonzero_conditions,
            stats: restore_stats(&self.stats)?,
        })
    }
}

#[cfg(test)]
mod tests;
