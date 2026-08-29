//! Explicit resource policy for family-presentation authentication.

/// Aggregate admission limits for one authenticated family presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FamilyPresentationLimits {
    /// Sum of UTF-8 bytes across denominator IDs and source routing labels.
    pub max_role_and_routing_label_bytes: usize,
    /// Total caller-owned exact coefficients inspected in roles and routing.
    pub max_coefficient_inputs: usize,
    /// Total possible coefficient-denominator, determinant, and scale guards
    /// inspected while constructing the presentation domain.
    pub max_condition_inputs: usize,
    /// Number of distinct retained presentation-domain polynomials.
    pub max_nonzero_conditions: usize,
    /// Number of distinct provenances retained across all conditions.
    pub max_condition_sources: usize,
}

impl Default for FamilyPresentationLimits {
    fn default() -> Self {
        Self {
            max_role_and_routing_label_bytes: 1024 * 1024 * 1024,
            max_coefficient_inputs: 16_000_000,
            max_condition_inputs: 16_000_000,
            max_nonzero_conditions: 16_000_000,
            max_condition_sources: 16_000_000,
        }
    }
}
