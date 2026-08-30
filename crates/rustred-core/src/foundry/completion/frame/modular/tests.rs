use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::domains::{Ring, RingOps};
use symbolica::prelude::Integer;
use symbolica::tensors::matrix::Matrix;
use symbolica::tensors::sparse::SparseMatrix;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::canonical_three_loop_family;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::Mask;

use super::rank::rank_projection_for_test;
use super::sample::evaluate_coefficient_for_test;
use super::{ModularKernelError, ModularKernelLimits, ModularPhysicalFrame, ModularTargetQuery};
use crate::foundry::completion::frame::{PhysicalFrameLimits, PhysicalFramePlan};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn s4a_degree_one() -> (IndexedCoefficientContext, PhysicalFramePlan) {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let plan = PhysicalFramePlan::try_new(
        &generator,
        &completed,
        Mask::try_new([false, true, true, true, true, false]).unwrap(),
        1,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    (context, plan)
}

fn sampled_s4a<'plan>(
    context: &IndexedCoefficientContext,
    plan: &'plan PhysicalFramePlan,
) -> ModularPhysicalFrame<'plan> {
    plan.try_modular_sample(
        context,
        PRIME,
        &[37],
        &[1, 2, 3, 4, 5, 6],
        ModularKernelLimits::default(),
    )
    .unwrap()
}

#[test]
fn k6_s4a_degree_one_samples_physical_csr_and_preserves_exact_provenance() {
    let (context, plan) = s4a_degree_one();
    let sampled = sampled_s4a(&context, &plan);
    assert!(std::ptr::eq(sampled.plan(), &plan));
    assert_eq!(sampled.matrix().nrows(), 63);
    assert_eq!(sampled.matrix().ncols(), 157);
    assert!(sampled.matrix().nvalues() <= 630);
    assert_eq!(sampled.source_instances(), plan.source_instances());
    assert_eq!(
        sampled
            .source_instances()
            .iter()
            .map(|source| source.stable_string())
            .collect::<Vec<_>>(),
        plan.source_instances()
            .iter()
            .map(|source| source.stable_string())
            .collect::<Vec<_>>()
    );

    let target = sampled.matrix().col_idcs()[0] as usize;
    let query = sampled
        .query_target(target, &[], ModularKernelLimits::default())
        .unwrap();
    assert!(matches!(query, ModularTargetQuery::Hit(_)));
    assert_eq!(query.diagnostics().forbidden_rank, 0);
    assert_eq!(query.diagnostics().augmented_rank, 1);
}

#[test]
fn chart_coordinates_map_to_sector_indices_with_exact_signs() {
    let (context, plan) = s4a_degree_one();
    let sampled = sampled_s4a(&context, &plan);
    let residues = sampled
        .point()
        .iter()
        .map(|value| sampled.field().from_element(value))
        .collect::<Vec<_>>();
    assert_eq!(residues[0], 37);
    assert_eq!(&residues[1..], &[PRIME - 1, 3, 4, 5, 6, PRIME - 6]);
}

#[test]
fn repeated_samples_and_target_queries_are_deterministic() {
    let (context, plan) = s4a_degree_one();
    let first = sampled_s4a(&context, &plan);
    let second = sampled_s4a(&context, &plan);
    assert_eq!(first.matrix(), second.matrix());
    assert_eq!(first.point(), second.point());

    let target = first.matrix().col_idcs()[0] as usize;
    let first_query = first
        .query_target(target, &[], ModularKernelLimits::default())
        .unwrap();
    let second_query = first
        .query_target(target, &[], ModularKernelLimits::default())
        .unwrap();
    assert_eq!(first_query, second_query);
}

#[test]
fn sparse_and_symbolica_dense_rank_agree_on_the_s4a_sample() {
    let (context, plan) = s4a_degree_one();
    let sampled = sampled_s4a(&context, &plan);
    let selected = (0..sampled.matrix().ncols() as usize).collect::<Vec<_>>();
    let (sparse_rank, _) =
        rank_projection_for_test(sampled.matrix(), &selected, ModularKernelLimits::default())
            .unwrap();
    let dense_rank = sampled.matrix().to_dense().rank();
    assert_eq!(sparse_rank, dense_rank);
}

#[test]
fn s4a_nonempty_target_partition_matches_independent_dense_evidence() {
    let (context, plan) = s4a_degree_one();
    let sampled = sampled_s4a(&context, &plan);
    let forbidden = (0..32).collect::<Vec<_>>();
    let target = sampled
        .matrix()
        .col_idcs()
        .iter()
        .map(|&column| column as usize)
        .find(|&column| column >= 32)
        .unwrap();
    let query = sampled
        .query_target(target, &forbidden, ModularKernelLimits::default())
        .unwrap();
    let diagnostics = query.diagnostics();

    let mut augmented = forbidden.clone();
    augmented.insert(augmented.binary_search(&target).unwrap_err(), target);
    let dense = sampled.matrix().to_dense();
    let all_rows = (0..dense.nrows()).collect::<Vec<_>>();

    assert_eq!(
        diagnostics.forbidden_rank,
        dense_projection(&dense, &all_rows, &forbidden).rank()
    );
    assert_eq!(
        diagnostics.augmented_rank,
        dense_projection(&dense, &all_rows, &augmented).rank()
    );
    assert_eq!(
        diagnostics.forbidden_pivot_columns.as_ref(),
        dense_pivot_columns(&dense, &forbidden).as_slice()
    );
    assert_eq!(
        diagnostics.augmented_pivot_columns.as_ref(),
        dense_pivot_columns(&dense, &augmented).as_slice()
    );
    assert_eq!(
        diagnostics.forbidden_independent_source_rows.as_ref(),
        dense_independent_rows(&dense, &forbidden).as_slice()
    );
    assert_eq!(
        diagnostics.augmented_independent_source_rows.as_ref(),
        dense_independent_rows(&dense, &augmented).as_slice()
    );
    assert_eq!(
        diagnostics.forbidden_total_fill_nonzeros,
        diagnostics.forbidden_lower_pattern_nonzeros + diagnostics.forbidden_upper_nonzeros
    );
    assert_eq!(
        diagnostics.augmented_total_fill_nonzeros,
        diagnostics.augmented_lower_pattern_nonzeros + diagnostics.augmented_upper_nonzeros
    );
}

#[test]
fn target_queries_use_their_own_forbidden_column_sets() {
    let field = Zp64::new(PRIME);
    // c0=(1,0), c1=(0,1), c2=c0+c1.  The target c2 is a hit over
    // {c0}, but a no-hit over {c0,c1}; repeating the first query must not
    // inherit the second query's projected reducer.
    let matrix = SparseMatrix::from_csr(
        2,
        3,
        vec![field.one(), field.one(), field.one(), field.one()],
        vec![0, 2, 4],
        vec![0, 2, 1, 2],
        field,
    );
    let hit = query_matrix_for_test(&matrix, 2, &[0]).unwrap();
    let no_hit = query_matrix_for_test(&matrix, 2, &[0, 1]).unwrap();
    let repeated_hit = query_matrix_for_test(&matrix, 2, &[0]).unwrap();
    assert_eq!(hit, repeated_hit);
    assert_eq!(hit, (1, 2));
    assert_eq!(no_hit, (2, 2));
}

#[test]
fn malformed_moduli_and_sample_arities_are_rejected_before_native_evaluation() {
    let (context, plan) = s4a_degree_one();
    let limits = ModularKernelLimits::default();
    assert_eq!(
        plan.try_modular_sample(&context, 10, &[37], &[0; 6], limits)
            .unwrap_err(),
        ModularKernelError::UnsupportedEvenModulus { modulus: 10 }
    );
    assert_eq!(
        plan.try_modular_sample(&context, 9, &[37], &[0; 6], limits)
            .unwrap_err(),
        ModularKernelError::NonPrimeModulus { modulus: 9 }
    );
    assert_eq!(
        plan.try_modular_sample(&context, PRIME, &[], &[0; 6], limits)
            .unwrap_err(),
        ModularKernelError::WrongBaseParameterArity {
            expected: 1,
            actual: 0,
        }
    );
    assert_eq!(
        plan.try_modular_sample(&context, PRIME, &[37], &[0; 5], limits)
            .unwrap_err(),
        ModularKernelError::WrongChartCoordinateArity {
            expected: 6,
            actual: 5,
        }
    );
}

#[test]
fn rational_coefficient_denominator_zero_is_rejected_before_division() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "modular-denominator-zero", 1).unwrap();
    let rational = base.coefficient_fixture("1/(d-2)");
    let coefficient = context.lift(&rational).unwrap();
    let field = Zp64::new(PRIME);
    let point = [
        Integer::from(2).to_finite_field(&field),
        Integer::from(1).to_finite_field(&field),
    ];
    assert!(evaluate_coefficient_for_test(&coefficient, &point, &field));
}

#[test]
fn vanishing_exact_source_condition_rejects_the_whole_sample() {
    let (family, base) = guarded_tadpole();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let plan = PhysicalFramePlan::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let error = plan
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[4, 0],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        ModularKernelError::SourceConditionZero { .. }
    ));
    drop(base);
}

#[test]
fn finite_field_rank_does_not_use_normal_equations() {
    let field = Zp64::new(5);
    // A=(1,2)^T has rank one, while A^T A = 1+4 = 0 in F_5.
    let matrix = SparseMatrix::from_csr(
        2,
        1,
        vec![field.one(), field.to_element(2)],
        vec![0, 1, 2],
        vec![0, 0],
        field.clone(),
    );
    let (rank, _) =
        rank_projection_for_test(&matrix, &[0], ModularKernelLimits::default()).unwrap();
    let gram = field.add(
        &field.one(),
        &field.mul(&field.to_element(2), &field.to_element(2)),
    );
    assert_eq!(rank, 1);
    assert!(field.is_zero(&gram));
}

#[test]
fn reducer_dense_fill_limit_is_checked_before_symbolica() {
    let field = Zp64::new(PRIME);
    let matrix = SparseMatrix::identity(2, field);
    let mut limits = ModularKernelLimits::default();
    limits.max_reducer_dense_cells = 3;
    assert_eq!(
        rank_projection_for_test(&matrix, &[0, 1], limits).unwrap_err(),
        ModularKernelError::ResourceLimit {
            resource: "modular reducer dense-fill cells",
            requested: 4,
            limit: 3,
        }
    );
}

#[test]
fn projected_row_offsets_and_total_fill_have_query_owned_limits() {
    let field = Zp64::new(PRIME);
    let matrix = SparseMatrix::identity(2, field);

    let mut row_limits = ModularKernelLimits::default();
    row_limits.max_csr_row_offsets = 2;
    assert_eq!(
        rank_projection_for_test(&matrix, &[0, 1], row_limits).unwrap_err(),
        ModularKernelError::ResourceLimit {
            resource: "modular projected CSR row offsets",
            requested: 3,
            limit: 2,
        }
    );

    let mut envelope_limits = ModularKernelLimits::default();
    envelope_limits.max_reducer_total_fill_entries = 7;
    assert_eq!(
        rank_projection_for_test(&matrix, &[0, 1], envelope_limits).unwrap_err(),
        ModularKernelError::ResourceLimit {
            resource: "modular reducer total L-pattern plus U fill entries",
            requested: 8,
            limit: 7,
        }
    );

    let mut ratio_limits = ModularKernelLimits::default();
    ratio_limits.max_reducer_fill_multiple = 1;
    assert_eq!(
        rank_projection_for_test(&matrix, &[0, 1], ratio_limits).unwrap_err(),
        ModularKernelError::ResourceLimit {
            resource: "modular reducer fill-multiple entries",
            requested: 4,
            limit: 2,
        }
    );

    let (rank, total_fill) =
        rank_projection_for_test(&matrix, &[0, 1], ModularKernelLimits::default()).unwrap();
    assert_eq!((rank, total_fill), (2, 4));
}

fn query_matrix_for_test(
    matrix: &SparseMatrix<Zp64>,
    target: usize,
    forbidden: &[usize],
) -> Result<(usize, usize), ModularKernelError> {
    let (forbidden_rank, _) =
        rank_projection_for_test(matrix, forbidden, ModularKernelLimits::default())?;
    let mut augmented = forbidden.to_vec();
    augmented.push(target);
    augmented.sort_unstable();
    let (augmented_rank, _) =
        rank_projection_for_test(matrix, &augmented, ModularKernelLimits::default())?;
    Ok((forbidden_rank, augmented_rank))
}

fn dense_projection(matrix: &Matrix<Zp64>, rows: &[usize], columns: &[usize]) -> Matrix<Zp64> {
    let mut values = Vec::with_capacity(rows.len() * columns.len());
    for &row in rows {
        for &column in columns {
            values.push(matrix[(row as u32, column as u32)].clone());
        }
    }
    Matrix::from_linear(
        values,
        rows.len() as u32,
        columns.len() as u32,
        matrix.field().clone(),
    )
    .unwrap()
}

fn dense_pivot_columns(matrix: &Matrix<Zp64>, columns: &[usize]) -> Vec<usize> {
    let rows = (0..matrix.nrows()).collect::<Vec<_>>();
    let mut projected = dense_projection(matrix, &rows, columns);
    let rank = projected.partial_row_reduce(columns.len() as u32) as usize;
    projected
        .row_iter()
        .take(rank)
        .map(|row| {
            let projected_column = row
                .iter()
                .position(|value| !projected.field().is_zero(value))
                .unwrap();
            columns[projected_column]
        })
        .collect()
}

fn dense_independent_rows(matrix: &Matrix<Zp64>, columns: &[usize]) -> Vec<usize> {
    let mut independent = Vec::new();
    let mut rank = 0usize;
    for row in 0..matrix.nrows() {
        independent.push(row);
        let candidate_rank = dense_projection(matrix, &independent, columns).rank();
        if candidate_rank > rank {
            rank = candidate_rank;
        } else {
            independent.pop();
        }
    }
    independent
}

fn guarded_tadpole() -> (IntegralFamily, CoefficientContext) {
    let base = CoefficientContext::new(["d", "x"]);
    let reciprocal = base
        .try_div(
            &base.one(),
            &base.parameter("x").unwrap(),
            Default::default(),
        )
        .unwrap();
    let family = IntegralFamily::new(
        "modular-source-condition-tadpole",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![reciprocal],
    )
    .unwrap();
    (family, base)
}

#[test]
fn symbolica_dense_rank_zero_row_edge_is_guarded_by_the_sparse_kernel() {
    let field = Zp64::new(PRIME);
    let empty = SparseMatrix::new(0, 2, field);
    assert_eq!(
        rank_projection_for_test(&empty, &[0, 1], ModularKernelLimits::default()).unwrap(),
        (0, 0)
    );
    // Do not call Matrix::rank on a 0xN matrix: Symbolica 2.2.0 indexes row
    // zero in `partial_row_reduce`.  The production probe above returns the
    // mathematically correct typed rank before entering that native edge.
    let _dense_type_guard: Option<Matrix<Zp64>> = None;
}
