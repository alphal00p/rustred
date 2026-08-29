use std::thread;

use symbolica::prelude::Integer;

use super::{
    RightKernelDecision, RightKernelError, RightKernelLimits, first_primitive_right_kernel,
    verify_product_fixture,
};

fn limits() -> RightKernelLimits {
    RightKernelLimits {
        max_rows: 100,
        max_columns: 100,
        max_entries: 10_000,
        max_rank_operations: 1_000_000,
        max_rref_integer_bits: 10_000,
        max_kernel_entries: 100,
        max_kernel_integer_bits: 10_000,
    }
}

fn deficient(entries: &[u16], rows: usize, columns: usize) -> (usize, Box<[Integer]>) {
    match first_primitive_right_kernel(entries, rows, columns, limits()).unwrap() {
        RightKernelDecision::Deficient {
            rank,
            primitive_kernel,
        } => (rank, primitive_kernel),
        RightKernelDecision::FullColumnRank { .. } => panic!("expected a right-kernel witness"),
    }
}

#[test]
fn rational_primitive_part_clears_mixed_denominators_and_orients_sign() {
    let (rank, kernel) = deficient(&[2, 0, 1, 0, 3, 1], 2, 3);
    assert_eq!(rank, 2);
    assert_eq!(
        &*kernel,
        &[Integer::from(3), Integer::from(2), Integer::from(-6)]
    );
}

#[test]
fn first_free_column_is_the_stable_kernel_choice() {
    let (_, kernel) = deficient(&[1, 0, 1], 1, 3);
    assert_eq!(
        &*kernel,
        &[Integer::from(0), Integer::from(1), Integer::from(0)]
    );

    thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..4 {
            workers.push(scope.spawn(|| deficient(&[1, 0, 1], 1, 3).1));
        }
        for worker in workers {
            assert_eq!(worker.join().unwrap(), kernel);
        }
    });
}

#[test]
fn native_integer_product_accepts_the_exact_witness() {
    let (_, kernel) = deficient(&[2, 3, 4, 6], 2, 2);
    assert_eq!(&*kernel, &[Integer::from(3), Integer::from(-2)]);
}

#[test]
fn native_integer_product_rejects_a_false_witness() {
    assert_eq!(
        verify_product_fixture(&[2, 3], 1, 2, vec![Integer::from(1), Integer::from(0)],),
        Err(RightKernelError::ReplayFailure)
    );
}

#[test]
fn full_column_rank_has_no_witness() {
    assert_eq!(
        first_primitive_right_kernel(&[1, 0, 0, 1], 2, 2, limits()).unwrap(),
        RightKernelDecision::FullColumnRank { rank: 2 }
    );
}

#[test]
fn structurally_empty_matrix_uses_the_first_basis_vector() {
    let (rank, kernel) = deficient(&[], 0, 3);
    assert_eq!(rank, 0);
    assert_eq!(
        &*kernel,
        &[Integer::from(1), Integer::from(0), Integer::from(0)]
    );
}

#[test]
fn malformed_shape_and_zero_columns_are_rejected_before_symbolica() {
    assert_eq!(
        first_primitive_right_kernel(&[1], 1, 2, limits()),
        Err(RightKernelError::Shape {
            rows: 1,
            columns: 2,
            entries: 1,
        })
    );
    assert_eq!(
        first_primitive_right_kernel(&[], 0, 0, limits()),
        Err(RightKernelError::ZeroColumns)
    );
}

#[test]
fn rank_and_kernel_resources_remain_hard_limits() {
    let mut bounded = limits();
    bounded.max_rank_operations = 1;
    assert_eq!(
        first_primitive_right_kernel(&[2, 3], 1, 2, bounded),
        Err(RightKernelError::ResourceLimit {
            resource: "rank operations",
            requested: 2,
            limit: 1,
        })
    );

    bounded = limits();
    bounded.max_kernel_entries = 1;
    assert_eq!(
        first_primitive_right_kernel(&[2, 3], 1, 2, bounded),
        Err(RightKernelError::ResourceLimit {
            resource: "certificate kernel entries",
            requested: 2,
            limit: 1,
        })
    );

    bounded = limits();
    bounded.max_rref_integer_bits = 7;
    assert_eq!(
        first_primitive_right_kernel(&[2, 3], 1, 2, bounded),
        Err(RightKernelError::ResourceLimit {
            resource: "RREF integer bits",
            requested: 8,
            limit: 7,
        })
    );

    bounded = limits();
    bounded.max_kernel_integer_bits = 5;
    assert_eq!(
        first_primitive_right_kernel(&[2, 3], 1, 2, bounded),
        Err(RightKernelError::ResourceLimit {
            resource: "certificate kernel integer bits",
            requested: 6,
            limit: 5,
        })
    );
}
