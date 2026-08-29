use super::construction::check_limit;
use super::error::SymbolicaAffineDenominatorError;
use super::limits::SymbolicaAffineDenominatorLimits;

#[derive(Clone, Copy)]
pub(super) enum BinaryOperation {
    Add,
    Multiply,
    Divide,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ExactWorkBudget {
    pub(super) dense_degree_box_terms: usize,
    pub(super) dense_degree_box_exponent_entries: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ProjectionStats {
    pub(super) projected_retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ProjectionAllocationBudget {
    polynomial_terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
    retained_bytes: usize,
    pub(super) groups: usize,
    denominator_replication_terms: usize,
    gram_operations: usize,
}

impl ProjectionAllocationBudget {
    pub(super) fn charge_structure(
        &mut self,
        groups: usize,
        denominator_replication_terms: usize,
        gram_operations: usize,
        limits: SymbolicaAffineDenominatorLimits,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        self.groups = self.groups.checked_add(groups).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "aggregate projection groups",
            },
        )?;
        self.denominator_replication_terms = self
            .denominator_replication_terms
            .checked_add(denominator_replication_terms)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "aggregate projection denominator replication terms",
            })?;
        self.gram_operations = self.gram_operations.checked_add(gram_operations).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "aggregate projection Gram operations",
            },
        )?;
        check_limit(
            "aggregate projection groups",
            self.groups,
            limits.max_projection_groups,
        )?;
        check_limit(
            "aggregate projection denominator replication terms",
            self.denominator_replication_terms,
            limits.max_projection_denominator_replication_terms,
        )?;
        check_limit(
            "aggregate projection Gram operations",
            self.gram_operations,
            limits.max_projection_gram_operations,
        )
    }

    pub(super) fn charge(
        &mut self,
        census: CoefficientCensus,
        limits: SymbolicaAffineDenominatorLimits,
        resource: &'static str,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        self.polynomial_terms = self
            .polynomial_terms
            .checked_add(census.polynomial_terms)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.exponent_entries = self
            .exponent_entries
            .checked_add(census.exponent_entries)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.integer_bits = self
            .integer_bits
            .checked_add(census.integer_bits)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.retained_bytes = self
            .retained_bytes
            .checked_add(census.retained_bytes)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        check_limit(
            resource,
            self.polynomial_terms,
            limits.max_projected_polynomial_terms,
        )?;
        check_limit(
            "aggregate projected exponent entries",
            self.exponent_entries,
            limits.max_projected_exponent_entries,
        )?;
        check_limit(
            "aggregate projected integer bits",
            self.integer_bits,
            limits.max_projected_integer_bits,
        )?;
        check_limit(
            "aggregate projected retained bytes",
            self.retained_bytes,
            limits.max_projected_retained_bytes,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CoefficientCensus {
    pub(super) polynomial_terms: usize,
    pub(super) exponent_entries: usize,
    pub(super) integer_bits: usize,
    pub(super) retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct NormalizedExpressionCensus {
    pub(super) nodes: usize,
    pub(super) integer_bits: usize,
}

impl CoefficientCensus {
    pub(super) fn checked_add_assign(
        &mut self,
        other: Self,
        resource: &'static str,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        self.polynomial_terms = self
            .polynomial_terms
            .checked_add(other.polynomial_terms)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.exponent_entries = self
            .exponent_entries
            .checked_add(other.exponent_entries)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.integer_bits = self
            .integer_bits
            .checked_add(other.integer_bits)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.retained_bytes = self
            .retained_bytes
            .checked_add(other.retained_bytes)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExactOperationAllocationEnvelope {
    pub(super) census: CoefficientCensus,
    pub(super) numerator_terms: usize,
    pub(super) denominator_terms: usize,
}
