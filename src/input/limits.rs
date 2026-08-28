//! Resource policies and admitted-work statistics for input compilation.

use crate::family::IntegralFamilyLimits;
use crate::symbolica_affine_denominator::SymbolicaAffineDenominatorLimits;

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
    pub max_pattern_attempts: usize,
    pub max_pattern_matches: usize,
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
            max_pattern_attempts: 150_000,
            max_pattern_matches: 16_384,
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
pub struct Stats {
    pub(super) input_bytes: usize,
    pub(super) atom_nodes: usize,
    pub(super) maximum_depth: usize,
    pub(super) clauses: usize,
    pub(super) clause_arguments: usize,
    pub(super) pattern_attempts: usize,
    pub(super) pattern_matches: usize,
    pub(super) symbol_inspections: usize,
    pub(super) inferred_parameters: usize,
    pub(super) canonical_nodes: usize,
    pub(super) preconversion_integer_bits: usize,
    pub(super) retained_atom_integer_bits: usize,
    pub(super) retained_atom_bytes: usize,
}

impl Stats {
    pub const fn input_bytes(self) -> usize {
        self.input_bytes
    }

    pub const fn atom_nodes(self) -> usize {
        self.atom_nodes
    }

    pub const fn maximum_depth(self) -> usize {
        self.maximum_depth
    }

    pub const fn clauses(self) -> usize {
        self.clauses
    }

    pub const fn clause_arguments(self) -> usize {
        self.clause_arguments
    }

    pub const fn pattern_attempts(self) -> usize {
        self.pattern_attempts
    }

    pub const fn pattern_matches(self) -> usize {
        self.pattern_matches
    }

    pub const fn symbol_inspections(self) -> usize {
        self.symbol_inspections
    }

    pub const fn inferred_parameters(self) -> usize {
        self.inferred_parameters
    }

    pub const fn canonical_nodes(self) -> usize {
        self.canonical_nodes
    }

    /// Conservative exact-arithmetic work charged before Token-to-Atom
    /// conversion, aggregated across all explicit text fields.
    pub const fn preconversion_integer_bits(self) -> usize {
        self.preconversion_integer_bits
    }

    /// Conservative integer-bit envelope of all packed Atom copies retained by
    /// the normalized project.
    pub const fn retained_atom_integer_bits(self) -> usize {
        self.retained_atom_integer_bits
    }

    /// Conservative packed-byte envelope of all Atom copies retained by the
    /// normalized project.
    pub const fn retained_atom_bytes(self) -> usize {
        self.retained_atom_bytes
    }
}
