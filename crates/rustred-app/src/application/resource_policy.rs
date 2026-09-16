//! Reproducible caller resource settings, separate from artifact authority.

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ResourcePolicyOutput {
    // Decimal strings preserve every usize, including values beyond TOML's
    // signed 64-bit integer range. These fields never enter durable bytes.
    max_domain_bound_endpoint_cells: String,
    max_predicate_consistency_work: String,
    max_predicate_atoms: String,
}

impl ResourcePolicyOutput {
    pub(super) fn new(
        endpoint_cells: usize,
        consistency_work: usize,
        predicate_atoms: usize,
    ) -> Self {
        Self {
            max_domain_bound_endpoint_cells: endpoint_cells.to_string(),
            max_predicate_consistency_work: consistency_work.to_string(),
            max_predicate_atoms: predicate_atoms.to_string(),
        }
    }
}
