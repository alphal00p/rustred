mod admission;
mod retained;

pub(super) use admission::{
    exact_operation_allocation_envelope, planned_operation_polynomial_census,
    verify_operation_result_envelope,
};
pub(super) use retained::{
    coefficient_census, compiled_retained_byte_bound, integer_magnitude_bits, multiply_census,
    planned_coefficient_clone_census, planned_polynomial_clone_census,
    planned_unit_coefficient_census, polynomial_census, retained_variable_map_arc_bytes,
    signed_i64_magnitude_bits,
};
