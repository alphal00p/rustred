//! Exact rule-cell slices on three-line K6 graph sectors.
//!
//! Negative powers on inactive denominators are numerator obligations, even
//! when the corresponding scalar graph corner factorizes.  This module keeps
//! those decorated lanes distinct from authenticated factorization terminals.

mod decorated_path_numerator;

#[cfg(test)]
mod decorated_path_numerator_tests;
mod undotted_path_numerator;
#[cfg(test)]
mod undotted_path_numerator_tests;

use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::RuleCell;

use decorated_path_numerator::derive_decorated_path_numerator_cells;
use undotted_path_numerator::derive_undotted_path_numerator_cells;

/// Complete ordered owner of the presently certified three-line discovery
/// slices.  Decorated and undotted numerator lanes stay separate because
/// their fixed restrictions and `S4` orbits are distinct.
pub(super) struct ThreeLineCellSet {
    pub(super) decorated_path_numerator_endpoint: RuleCell,
    pub(super) decorated_path_numerator_bulk: RuleCell,
    pub(super) undotted_path_numerator_endpoint: RuleCell,
    pub(super) undotted_path_numerator_bulk: RuleCell,
}

pub(super) fn derive_three_line_cells() -> Result<ThreeLineCellSet, ArtifactError> {
    let (_context, decorated_path_numerator_endpoint, decorated_path_numerator_bulk) =
        derive_decorated_path_numerator_cells()?;
    let (_context, undotted_path_numerator_endpoint, undotted_path_numerator_bulk) =
        derive_undotted_path_numerator_cells()?;
    Ok(ThreeLineCellSet {
        decorated_path_numerator_endpoint,
        decorated_path_numerator_bulk,
        undotted_path_numerator_endpoint,
        undotted_path_numerator_bulk,
    })
}
