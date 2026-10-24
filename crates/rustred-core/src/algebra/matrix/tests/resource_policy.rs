use super::super::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    invert_and_verify_coefficient_matrix, rank_of_coefficient_matrix,
};
use crate::algebra::{CoefficientContext, ExactAlgebraError};
use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

#[test]
fn matrix_count_and_retained_thresholds_require_diagnostic_opt_in() {
    let context = CoefficientContext::new(["d"]);
    let matrix = vec![
        vec![context.integer(2), context.one()],
        vec![context.one(), context.one()],
    ];
    let limits = SymbolicaCoefficientMatrixLimits {
        max_single_matrix_entries: 0,
        max_live_matrix_entries: 0,
        max_exact_operations: 0,
        max_input_retained_bytes: 0,
        max_output_retained_bytes: 0,
        ..SymbolicaCoefficientMatrixLimits::default()
    };
    let inverse = invert_and_verify_coefficient_matrix(&context, &matrix, limits).unwrap();
    assert_eq!(inverse.determinant, context.one());
    assert!(inverse.stats.exact_operations() > 0);

    let enabled = limits.with_diagnostics();
    assert!(enabled.diagnostics_enabled);
    assert!(enabled.exact_algebra.diagnostics_enabled);
    assert!(matches!(
        invert_and_verify_coefficient_matrix(&context, &matrix, enabled),
        Err(SymbolicaCoefficientMatrixError::ResourceLimit { limit: 0, .. })
    ));
}

#[test]
fn ambient_matrix_resource_exhaustion_is_a_typed_error() {
    let context = CoefficientContext::new(["d"]);
    let matrix = vec![vec![context.one()]];
    let budget = ResourceBudget::with_memory_monitor(
        ResourceLimits {
            max_process_rss_bytes: Some(1),
            ..ResourceLimits::default()
        },
        || Ok(2),
    );
    assert!(matches!(
        budget.run(|| rank_of_coefficient_matrix(&context, &matrix, Default::default())),
        Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::Resource(ResourceLimitError::Memory { .. })
        ))
    ));
}
