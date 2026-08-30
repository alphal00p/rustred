//! Exact rule-cell slices on three-line K6 graph sectors.
//!
//! Negative powers on inactive denominators are numerator obligations, even
//! when the corresponding scalar graph corner factorizes.  This module keeps
//! those decorated lanes distinct from authenticated factorization terminals.

mod bridge_descendant_dot_numerator;
#[cfg(test)]
mod bridge_descendant_dot_numerator_tests;
mod decorated_path_numerator;
#[cfg(test)]
mod decorated_path_numerator_tests;
mod incident_path_dot_numerator_endpoint;
#[cfg(test)]
mod incident_path_dot_numerator_endpoint_tests;
mod undotted_path_numerator;
#[cfg(test)]
mod undotted_path_numerator_tests;

use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::RuleCell;

use bridge_descendant_dot_numerator::derive_bridge_descendant_dot_numerator_endpoint;
use decorated_path_numerator::derive_decorated_path_numerator_cells;
use incident_path_dot_numerator_endpoint::derive_incident_path_dot_numerator_endpoint;
use undotted_path_numerator::derive_undotted_path_numerator_cells;

/// Complete ordered owner of the presently certified three-line discovery
/// slices.  The bridge descendant, incident path, decorated path, and
/// undotted numerator lanes stay separate because their fixed restrictions
/// and `S4` orbits are distinct.
pub(super) struct ThreeLineCellSet {
    pub(super) bridge_descendant_dot_numerator_endpoint: RuleCell,
    pub(super) incident_path_dot_numerator_endpoint: RuleCell,
    pub(super) decorated_path_numerator_endpoint: RuleCell,
    pub(super) decorated_path_numerator_bulk: RuleCell,
    pub(super) undotted_path_numerator_endpoint: RuleCell,
    pub(super) undotted_path_numerator_bulk: RuleCell,
}

pub(super) fn derive_three_line_cells() -> Result<ThreeLineCellSet, ArtifactError> {
    let (_context, bridge_descendant_dot_numerator_endpoint) =
        derive_bridge_descendant_dot_numerator_endpoint()?;
    let (_context, incident_path_dot_numerator_endpoint) =
        derive_incident_path_dot_numerator_endpoint()?;
    let (_context, decorated_path_numerator_endpoint, decorated_path_numerator_bulk) =
        derive_decorated_path_numerator_cells()?;
    let (_context, undotted_path_numerator_endpoint, undotted_path_numerator_bulk) =
        derive_undotted_path_numerator_cells()?;
    Ok(ThreeLineCellSet {
        bridge_descendant_dot_numerator_endpoint,
        incident_path_dot_numerator_endpoint,
        decorated_path_numerator_endpoint,
        decorated_path_numerator_bulk,
        undotted_path_numerator_endpoint,
        undotted_path_numerator_bulk,
    })
}
