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

use super::obstruction::verify_obstruction_for_test;
use super::rank::{rank_projection_for_test, right_obstruction_for_test};
use super::sample::evaluate_coefficient_for_test;
use super::{
    ModularKernelError, ModularKernelLimits, ModularObstructionEntry, ModularPhysicalFrame,
    ModularSourceEvaluationError, ModularTargetQuery,
};
use crate::foundry::completion::frame::{
    OneSidedChartFrame, PhysicalFrameLimits, PhysicalFramePlan,
};

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
    let plan = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([false, true, true, true, true, false]).unwrap(),
        1,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
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
fn exact_source_evaluator_matches_sampled_rows_and_retains_modular_zeros() {
    let (context, plan) = s4a_degree_one();
    let sampled = sampled_s4a(&context, &plan);
    let mut evaluated = Vec::new();
    let mut zero_specializations = 0usize;

    for row in 0..plan.row_count() {
        let source = plan.source_for_row(row).unwrap();
        sampled
            .try_evaluate_translated_source(&context, source, &mut evaluated)
            .unwrap();
        assert_eq!(evaluated.len(), source.terms().len());

        let structural = plan.column_indices_for_row(row).unwrap();
        assert_eq!(structural.len(), evaluated.len());
        let mut expected_columns = Vec::new();
        let mut expected_values = Vec::new();
        for (&column, value) in structural.iter().zip(&evaluated) {
            if sampled.field().is_zero(value) {
                zero_specializations += 1;
            } else {
                expected_columns.push(column);
                expected_values.push(value.clone());
            }
        }

        let bounds = &sampled.matrix().row_ptrs()[row..=row + 1];
        assert_eq!(
            &sampled.matrix().col_idcs()[bounds[0]..bounds[1]],
            expected_columns.as_slice()
        );
        assert_eq!(
            &sampled.matrix().values()[bounds[0]..bounds[1]],
            expected_values.as_slice()
        );
    }

    assert!(
        zero_specializations > 0,
        "the retained exact source images must expose at least one value which the sampled CSR drops"
    );
}

#[test]
fn source_evaluation_is_bound_to_an_admitted_sample_and_fails_transactionally() {
    let (family, base) = guarded_tadpole();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let plan = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
    let source = plan.source_for_row(0).unwrap();
    assert!(!source.nonzero_conditions().is_empty());

    let sampled = plan
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[4, 1],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let mut output = vec![sampled.field().one()];
    sampled
        .try_evaluate_translated_source(generator.context(), source, &mut output)
        .unwrap();
    assert_eq!(output.len(), source.terms().len());

    let foreign = IndexedCoefficientContext::try_new(&base, "foreign-source-evaluator", 1).unwrap();
    assert_eq!(
        sampled.try_evaluate_translated_source(&foreign, source, &mut output),
        Err(ModularSourceEvaluationError::FrameContextMismatch)
    );
    assert!(output.is_empty());
}

#[test]
fn source_evaluator_api_cannot_accept_an_independent_field_or_point() {
    let (context, plan) = s4a_degree_one();
    let sampled = sampled_s4a(&context, &plan);
    let source = plan.source_for_row(0).unwrap();

    // Keeping the method item and invoking it with only the admitted owner,
    // exact context/source, and output makes a future raw field/point argument
    // an API-shape compile failure in this regression.
    let evaluator = ModularPhysicalFrame::try_evaluate_translated_source;

    let foreign_field = Zp64::new(1_000_000_009);
    let foreign_point = vec![foreign_field.one(); sampled.point().len()];
    assert_ne!(
        foreign_field.get_prime(),
        sampled.sample_fingerprint().modulus()
    );
    assert_eq!(foreign_point.len(), sampled.point().len());
    assert_eq!(
        sampled.field().get_prime(),
        sampled.sample_fingerprint().modulus()
    );

    let mut output = Vec::new();
    evaluator(&sampled, &context, source, &mut output).unwrap();
    assert_eq!(output.len(), source.terms().len());
}

#[test]
fn source_evaluator_reports_exact_foreign_payload_ordinals() {
    let (family, base) = guarded_tadpole();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let plan = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let sampled = plan
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[4, 1],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();

    let reciprocal = base
        .try_div(
            &base.one(),
            &base.parameter("x").unwrap(),
            Default::default(),
        )
        .unwrap();
    let foreign_guarded_family = IntegralFamily::new(
        "foreign-guarded-source-condition-tadpole",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![reciprocal],
    )
    .unwrap();
    let foreign_guarded_generator =
        ParametricIbpGenerator::try_new(&foreign_guarded_family).unwrap();
    let foreign_guarded_completed = complete_ordinary(&foreign_guarded_generator);
    let foreign_guarded_plan = OneSidedChartFrame::try_new(
        &foreign_guarded_generator,
        &foreign_guarded_completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let guarded_source = foreign_guarded_plan.plan().source_for_row(0).unwrap();
    assert!(!guarded_source.nonzero_conditions().is_empty());
    let mut output = vec![sampled.field().one()];
    assert_eq!(
        sampled.try_evaluate_translated_source(generator.context(), guarded_source, &mut output,),
        Err(ModularSourceEvaluationError::ConditionContextMismatch {
            condition_ordinal: 0,
        })
    );
    assert!(output.is_empty());

    let foreign_family = IntegralFamily::new(
        "foreign-unguarded-source-term-tadpole",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap();
    let foreign_generator = ParametricIbpGenerator::try_new(&foreign_family).unwrap();
    let foreign_completed = complete_ordinary(&foreign_generator);
    let foreign_plan = OneSidedChartFrame::try_new(
        &foreign_generator,
        &foreign_completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let source = foreign_plan.plan().source_for_row(0).unwrap();
    assert!(source.nonzero_conditions().is_empty());
    assert!(!source.terms().is_empty());

    output.push(sampled.field().one());
    assert_eq!(
        sampled.try_evaluate_translated_source(generator.context(), source, &mut output),
        Err(ModularSourceEvaluationError::TermContextMismatch { term_ordinal: 0 })
    );
    assert!(output.is_empty());
}

#[test]
fn s4a_no_hit_obstruction_retains_exact_query_identity() {
    let (context, plan) = s4a_degree_one();
    let sampled = sampled_s4a(&context, &plan);
    let target = 0;
    let forbidden = (1..sampled.matrix().ncols() as usize).collect::<Vec<_>>();
    let query = sampled
        .query_target(target, &forbidden, ModularKernelLimits::default())
        .unwrap();
    let obstruction = query.obstruction().expect("all-other-column S4a no-hit");

    assert!(matches!(
        &query,
        ModularTargetQuery::NoHitWithObstruction(_)
    ));
    assert!(std::ptr::eq(obstruction.plan(), &plan));
    assert_eq!(
        obstruction.sample_fingerprint(),
        sampled.sample_fingerprint()
    );
    assert_eq!(obstruction.diagnostics(), query.diagnostics());
    assert_eq!(
        obstruction.logical_forbidden_columns(),
        forbidden.as_slice()
    );
    assert_eq!(obstruction.target_physical_column(), target);
    assert_eq!(
        obstruction.target_logical_column(),
        obstruction.logical_physical_columns().len() - 1
    );
    let target_entry = obstruction.entries().last().unwrap();
    assert_eq!(
        target_entry.logical_column(),
        obstruction.target_logical_column()
    );
    assert!(sampled.field().is_one(target_entry.coefficient()));
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

    // The query-local obstruction projection always numbers the target last,
    // independently of its physical-frame ordinal.
    let mut augmented = forbidden.clone();
    augmented.push(target);
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
        {
            let mut pivots = dense_pivot_columns(&dense, &augmented);
            pivots.sort_unstable();
            pivots
        }
        .as_slice()
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
fn target_first_no_hit_returns_a_checked_deterministic_obstruction() {
    let field = Zp64::new(PRIME);
    // Physical c0 is the target, while logical columns are [c1, c2, c0].
    // c2 is an unrelated free column, so the right kernel is
    // multidimensional.  The canonical target-normalized representative sets
    // every non-target free coordinate to zero and returns (-1, 0, 1).
    let matrix = SparseMatrix::from_csr(
        1,
        3,
        vec![field.one(), field.one()],
        vec![0, 2],
        vec![0, 1],
        field.clone(),
    );
    let limits = ModularKernelLimits::default();
    let first = right_obstruction_for_test(&matrix, 0, &[2, 1], limits).unwrap();
    let repeated = right_obstruction_for_test(&matrix, 0, &[1, 2], limits).unwrap();
    assert_eq!(first, repeated);
    assert_eq!((first.0, first.1), (1, 1));
    assert_eq!(first.2, vec![1, 2, 0]);
    assert_eq!(first.3.ncols(), 3);
    assert_eq!(first.3.col_idcs(), &[0, 2]);
    assert_eq!(
        first.4,
        vec![
            ModularObstructionEntry::new(0, field.neg(&field.one())),
            ModularObstructionEntry::new(2, field.one()),
        ]
    );
    verify_obstruction_for_test(&first.3, 2, &first.4, limits).unwrap();
}

#[test]
fn empty_rows_and_a_zero_target_column_have_canonical_unit_obstructions() {
    let field = Zp64::new(PRIME);
    let limits = ModularKernelLimits::default();
    let empty = SparseMatrix::new(0, 2, field.clone());
    let empty_obstruction = right_obstruction_for_test(&empty, 0, &[1], limits).unwrap();
    assert_eq!((empty_obstruction.0, empty_obstruction.1), (0, 0));
    assert_eq!(empty_obstruction.2, vec![1, 0]);
    assert_eq!(empty_obstruction.3.row_ptrs(), &[0]);
    assert_eq!(
        empty_obstruction.4,
        vec![ModularObstructionEntry::new(1, field.one())]
    );

    // The sole row has support only in forbidden physical c1; physical c0 is
    // an identically zero target column and therefore supplies the unit
    // target direction in the right kernel.
    let zero_target =
        SparseMatrix::from_csr(1, 2, vec![field.one()], vec![0, 1], vec![1], field.clone());
    let zero_obstruction = right_obstruction_for_test(&zero_target, 0, &[1], limits).unwrap();
    assert_eq!((zero_obstruction.0, zero_obstruction.1), (1, 1));
    assert_eq!(zero_obstruction.2, vec![1, 0]);
    assert_eq!(
        zero_obstruction.4,
        vec![ModularObstructionEntry::new(1, field.one())]
    );
    verify_obstruction_for_test(&zero_obstruction.3, 1, &zero_obstruction.4, limits).unwrap();
}

#[test]
fn extending_forbidden_columns_rebuilds_the_logical_projection() {
    let field = Zp64::new(PRIME);
    // Row zero states c0+c1=0.  Row one contains the newly forbidden c2.
    // Adding c2 must put it between c1 and the target in logical order and
    // retain its physical row, without changing the normalized relation.
    let matrix = SparseMatrix::from_csr(
        2,
        3,
        vec![field.one(), field.one(), field.one()],
        vec![0, 2, 3],
        vec![0, 1, 2],
        field.clone(),
    );
    let limits = ModularKernelLimits::default();
    let smaller = right_obstruction_for_test(&matrix, 0, &[1], limits).unwrap();
    assert_eq!(smaller.2, vec![1, 0]);
    assert_eq!(smaller.3.row_ptrs(), &[0, 2, 2]);
    assert_eq!(smaller.3.col_idcs(), &[0, 1]);

    let extended = right_obstruction_for_test(&matrix, 0, &[1, 2], limits).unwrap();
    assert_eq!((extended.0, extended.1), (2, 2));
    assert_eq!(extended.2, vec![1, 2, 0]);
    assert_eq!(extended.3.row_ptrs(), &[0, 2, 3]);
    assert_eq!(extended.3.col_idcs(), &[0, 2, 1]);
    assert_eq!(
        extended.4,
        vec![
            ModularObstructionEntry::new(0, field.neg(&field.one())),
            ModularObstructionEntry::new(2, field.one()),
        ]
    );
}

#[test]
fn tampered_right_obstructions_fail_exact_finite_field_replay() {
    let field = Zp64::new(PRIME);
    let matrix = SparseMatrix::from_csr(
        1,
        2,
        vec![field.one(), field.one()],
        vec![0, 2],
        vec![0, 1],
        field.clone(),
    );
    let limits = ModularKernelLimits::default();
    let (_, _, _, projected, entries) =
        right_obstruction_for_test(&matrix, 0, &[1], limits).unwrap();

    let mut bad_residual = entries.clone();
    bad_residual[0] = ModularObstructionEntry::new(0, field.one());
    assert_eq!(
        verify_obstruction_for_test(&projected, 1, &bad_residual, limits).unwrap_err(),
        ModularKernelError::Invariant {
            detail: "modular right obstruction failed exact finite-field residual replay",
        }
    );

    let mut bad_normalization = entries;
    bad_normalization[1] = ModularObstructionEntry::new(1, field.neg(&field.one()));
    assert_eq!(
        verify_obstruction_for_test(&projected, 1, &bad_normalization, limits).unwrap_err(),
        ModularKernelError::Invariant {
            detail: "modular right obstruction is not normalized to target coefficient one",
        }
    );
}

#[test]
fn obstruction_projection_and_back_substitution_have_owned_caps() {
    let field = Zp64::new(PRIME);
    let matrix = SparseMatrix::from_csr(
        1,
        3,
        vec![field.one(), field.one()],
        vec![0, 2],
        vec![0, 1],
        field,
    );

    let mut projection_limits = ModularKernelLimits::default();
    projection_limits.max_projected_columns = 2;
    assert_eq!(
        right_obstruction_for_test(&matrix, 0, &[1, 2], projection_limits).unwrap_err(),
        ModularKernelError::ResourceLimit {
            resource: "modular projected columns",
            requested: 3,
            limit: 2,
        }
    );

    // The two-column projection has two retained inputs, forward reduction
    // owns three possible L+U entries, and obstruction extraction admits a
    // two-entry RREF output.  A live cap of six must reject the conservative
    // seven-entry envelope before native back substitution.
    let mut live_limits = ModularKernelLimits::default();
    live_limits.max_reducer_total_fill_entries = 6;
    assert_eq!(
        right_obstruction_for_test(&matrix, 0, &[1], live_limits).unwrap_err(),
        ModularKernelError::ResourceLimit {
            resource: "modular obstruction back-substitution live entries",
            requested: 7,
            limit: 6,
        }
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
fn foreign_same_arity_context_is_rejected_at_the_plan_boundary() {
    let (context, plan) = s4a_degree_one();
    let foreign = IndexedCoefficientContext::try_new(
        context.base(),
        "foreign-modular-frame-context",
        context.index_count(),
    )
    .unwrap();

    assert_eq!(
        plan.try_modular_sample(&foreign, 10, &[], &[], ModularKernelLimits::default(),)
            .unwrap_err(),
        ModularKernelError::WrongFrameContext
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
    let plan = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
    let field = Zp64::new(PRIME);
    let point = [
        Integer::from(4).to_finite_field(&field),
        Integer::from(0).to_finite_field(&field),
        Integer::from(1).to_finite_field(&field),
    ];
    assert!(
        plan.source_for_row(0)
            .unwrap()
            .terms()
            .values()
            .any(|coefficient| evaluate_coefficient_for_test(coefficient, &point, &field)),
        "the condition-zero regression must also exercise a later zero denominator"
    );
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
