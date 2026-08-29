use super::super::field::{abort_checked_field, call_native};
use super::super::*;
use crate::algebra::{CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
use std::panic::resume_unwind;

#[test]
fn checked_field_abort_recovers_the_exact_error_without_formatting_payloads() {
    let expected = ExactAlgebraError::ResourceLimit {
        resource: "sentinel-test",
        requested: 2,
        limit: 1,
    };
    let error = call_native("sentinel transport", || {
        abort_checked_field(expected.clone())
    })
    .unwrap_err();
    assert_eq!(
        error,
        SymbolicaCoefficientMatrixError::ExactAlgebra(expected)
    );
    assert!(!error.to_string().contains("matrix payload"));
}

#[test]
fn unexpected_native_panic_is_redacted() {
    struct UnexpectedPanic;
    let error = call_native("panic test", || {
        resume_unwind(Box::new(UnexpectedPanic));
    })
    .unwrap_err();
    assert_eq!(
        error,
        SymbolicaCoefficientMatrixError::NativePanic {
            operation: "panic test"
        }
    );
    assert!(!error.to_string().contains("UnexpectedPanic"));
}

#[test]
fn exact_algebra_failure_crosses_native_boundary_as_typed_error() {
    let context = CoefficientContext::new(["x"]);
    let matrix = vec![
        vec![context.parameter("x").unwrap(), context.zero()],
        vec![context.zero(), context.one()],
    ];
    let error = invert_and_verify_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits {
            exact_algebra: ExactAlgebraLimits {
                max_term_operations: 0,
                ..ExactAlgebraLimits::default()
            },
            ..SymbolicaCoefficientMatrixLimits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SymbolicaCoefficientMatrixError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
            limit: 0,
            ..
        })
    ));
}

#[test]
fn checked_field_abort_is_contained_across_parallel_native_sessions() {
    let workers = (0..8)
        .map(|worker| {
            std::thread::spawn(move || {
                let parameter = format!("x{worker}");
                let context = CoefficientContext::new([parameter.as_str()]);
                let matrix = vec![
                    vec![context.parameter(&parameter).unwrap(), context.zero()],
                    vec![context.zero(), context.one()],
                ];
                let error = invert_and_verify_coefficient_matrix(
                    &context,
                    &matrix,
                    SymbolicaCoefficientMatrixLimits {
                        exact_algebra: ExactAlgebraLimits {
                            max_term_operations: 0,
                            ..ExactAlgebraLimits::default()
                        },
                        ..SymbolicaCoefficientMatrixLimits::default()
                    },
                )
                .unwrap_err();
                assert!(matches!(
                    error,
                    SymbolicaCoefficientMatrixError::ExactAlgebra(
                        ExactAlgebraError::ResourceLimit { limit: 0, .. }
                    )
                ));

                let expected = ExactAlgebraError::ResourceLimit {
                    resource: "parallel-sentinel-test",
                    requested: worker + 1,
                    limit: worker,
                };
                assert_eq!(
                    call_native("parallel sentinel transport", || {
                        abort_checked_field(expected.clone())
                    })
                    .unwrap_err(),
                    SymbolicaCoefficientMatrixError::ExactAlgebra(expected)
                );
            })
        })
        .collect::<Vec<_>>();

    for worker in workers {
        worker.join().expect("parallel native session panicked");
    }
}
