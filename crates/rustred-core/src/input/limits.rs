//! Resource policies and admitted-work statistics for input compilation.

use super::affine::SymbolicaAffineDenominatorLimits;
use crate::family::IntegralFamilyLimits;

use super::error::Error;

/// Resource policy for exact Symbolica-to-family lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoweringLimits {
    pub affine_denominator: SymbolicaAffineDenominatorLimits,
    pub integral_family: IntegralFamilyLimits,
}

impl Default for LoweringLimits {
    fn default() -> Self {
        Self {
            affine_denominator: SymbolicaAffineDenominatorLimits::default(),
            integral_family: IntegralFamilyLimits::default(),
        }
    }
}

/// Aggregate parser and normalization limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_raw_parser_units: usize,
    pub max_raw_integer_digits: usize,
    pub max_abs_power: u32,
    pub max_preconversion_integer_bits: usize,
    /// Conservative aggregate integer-bit envelope of every packed Atom copy
    /// retained by one normalized project.
    pub max_retained_atom_integer_bits: usize,
    /// Conservative aggregate bytes of every packed Atom copy retained by one
    /// normalized project.
    pub max_retained_atom_bytes: usize,
    pub max_unique_identifiers: usize,
    pub max_atom_nodes: usize,
    pub max_nesting_depth: usize,
    pub max_clauses: usize,
    pub max_clause_arguments: usize,
    pub max_label_bytes: usize,
    pub max_parameters: usize,
    pub max_momenta: usize,
    pub max_propagators: usize,
    pub max_gram_entries: usize,
    pub max_symbol_inspections: usize,
    pub max_canonical_nodes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 4 * 1024 * 1024,
            max_raw_parser_units: 1_000_000,
            max_raw_integer_digits: 1_000_000,
            max_abs_power: 256,
            max_preconversion_integer_bits: 64_000_000,
            max_retained_atom_integer_bits: 256_000_000,
            max_retained_atom_bytes: 256 * 1024 * 1024,
            max_unique_identifiers: 16_384,
            max_atom_nodes: 250_000,
            max_nesting_depth: 128,
            max_clauses: 16_384,
            max_clause_arguments: 65_536,
            max_label_bytes: 256,
            max_parameters: 4_096,
            max_momenta: 256,
            max_propagators: 16_384,
            max_gram_entries: 16_384,
            max_symbol_inspections: 1_000_000,
            max_canonical_nodes: 500_000,
        }
    }
}

/// Exact work census for one compact syntax compilation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Stats {
    pub(super) input_bytes: usize,
    pub(super) atom_nodes: usize,
    pub(super) maximum_depth: usize,
    pub(super) clauses: usize,
    pub(super) clause_arguments: usize,
    pub(super) symbol_inspections: usize,
    pub(super) inferred_parameters: usize,
    pub(super) preconversion_integer_bits: usize,
    pub(super) retained_atom_integer_bits: usize,
    pub(super) retained_atom_bytes: usize,
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, Error> {
    left.checked_add(right)
        .ok_or(Error::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, Error> {
    left.checked_mul(right)
        .ok_or(Error::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), Error> {
    if requested > limit {
        Err(Error::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
