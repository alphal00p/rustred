//! Generated rule-cell slices on the canonical five-line K6 face.
//!
//! These cells are test-only discovery evidence. In particular, the scalar
//! five-line corner remains an uncovered obligation rather than a master.

mod numerator;
#[cfg(test)]
mod numerator_tests;
mod positive;
#[cfg(test)]
mod positive_tests;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::RuleCell;
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};

use self::numerator::{NegativeNumeratorCells, derive_negative_numerator_cells};
use self::positive::{PositiveDotCells, derive_positive_dot_cells};
use super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::support::complete_ordinary_sources;

pub(super) const FIVE_LINE_SECTOR: [i64; 6] = [0, 1, 1, 1, 1, 1];

pub(super) struct FiveLineCellSet {
    pub(super) adjacent_dot: RuleCell,
    pub(super) opposite_dot: RuleCell,
    pub(super) scalar_numerator_endpoint: RuleCell,
    pub(super) scalar_numerator_bulk: RuleCell,
    pub(super) adjacent_numerator_endpoint: RuleCell,
    pub(super) adjacent_numerator_bulk: RuleCell,
    pub(super) opposite_numerator_endpoint: RuleCell,
    pub(super) opposite_numerator_bulk: RuleCell,
}

/// Derive every presently certified five-line discovery slice from the nine
/// ordinary generated K6 rows and their exact translated source views.
pub(super) fn derive_five_line_cells()
-> Result<(IndexedCoefficientContext, FiveLineCellSet), ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, source_count) = complete_ordinary_sources(&generator)?;

    let PositiveDotCells { adjacent, opposite } = derive_positive_dot_cells(
        &generator,
        &completed,
        source_count,
        &canonicalizer,
        &zero_sectors,
    )?;
    let NegativeNumeratorCells {
        scalar_endpoint,
        scalar_bulk,
        adjacent_endpoint,
        adjacent_bulk,
        opposite_endpoint,
        opposite_bulk,
    } = derive_negative_numerator_cells(
        &generator,
        &completed,
        source_count,
        &canonicalizer,
        &zero_sectors,
    )?;

    let context = generator.context().clone();
    drop(generator);
    Ok((
        context,
        FiveLineCellSet {
            adjacent_dot: adjacent,
            opposite_dot: opposite,
            scalar_numerator_endpoint: scalar_endpoint,
            scalar_numerator_bulk: scalar_bulk,
            adjacent_numerator_endpoint: adjacent_endpoint,
            adjacent_numerator_bulk: adjacent_bulk,
            opposite_numerator_endpoint: opposite_endpoint,
            opposite_numerator_bulk: opposite_bulk,
        },
    ))
}
