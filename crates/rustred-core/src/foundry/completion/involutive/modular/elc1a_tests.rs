use std::sync::Arc;

use crate::algebra::{CoefficientContext, IndexedCoefficient, IndexedCoefficientContext};

use super::{
    ExactMaterializationBudget, ExactMaterializerLimits, ModularCoefficientDag, ModularGuideError,
    ModularGuideLimits, ModularProbe, NonzeroCertification, try_certify_batch,
    try_issue_support_certificates, try_materialize_exact, try_materialize_exact_batch,
};

const PRIME: u64 = 998_244_353;

fn make_context(scope: &str) -> (CoefficientContext, IndexedCoefficientContext) {
    let base = CoefficientContext::new(std::iter::empty::<&'static str>());
    let indexed = IndexedCoefficientContext::try_new(&base, scope, 1).unwrap();
    (base, indexed)
}

fn leaf(
    dag: &mut ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    coefficient: IndexedCoefficient,
) -> super::CoeffRef {
    dag.try_exact_leaf(context, Arc::new(coefficient)).unwrap()
}

#[test]
fn consumed_guarded_batch_binds_order_positions_and_sampled_zeros() {
    let (_, context) = make_context("elc1a-ordered-certificates");
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let one = dag.one();
    let n_plus_one = dag.try_add(&n, &one).unwrap();

    let batch = try_certify_batch(
        &dag,
        &context,
        std::slice::from_ref(&one),
        &[n_plus_one.clone(), n.clone()],
        17,
        PRIME,
        &[0],
        limits,
    )
    .unwrap();
    assert!(batch.owns(
        &dag,
        &context,
        std::slice::from_ref(&one),
        &[n_plus_one.clone(), n.clone()]
    ));
    assert_eq!(batch.probe().ordinal(), 17);
    assert_eq!(batch.census().queries(), 3);
    let outcomes = batch.into_outcomes().into_vec();
    let NonzeroCertification::Certified(nonzero) = &outcomes[0] else {
        panic!("the first coefficient should have a one-sided proof")
    };
    assert!(nonzero.owns(&dag, &context, &n_plus_one));
    assert!(!nonzero.owns(&dag, &context, &n));
    assert_eq!(nonzero.query_position(), 1);
    assert_eq!(nonzero.residue(), 1);
    let NonzeroCertification::Unresolved(sampled_zero) = &outcomes[1] else {
        panic!("a zero image must remain unresolved")
    };
    assert!(sampled_zero.owns(&dag, &context, &n));
    assert_eq!(sampled_zero.query_position(), 2);
    assert_eq!(sampled_zero.probe().ordinal(), 17);
}

#[test]
fn issuer_rejects_reordered_truncated_foreign_and_stale_layouts() {
    let (_, context) = make_context("elc1a-layout-rejection");
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let one = dag.one();
    let n_plus_one = dag.try_add(&n, &one).unwrap();

    let reordered = ModularProbe::try_new(&dag, &context, 0, PRIME, &[2], limits)
        .unwrap()
        .try_evaluate_guarded_batch(
            &dag,
            std::slice::from_ref(&one),
            &[n.clone(), n_plus_one.clone()],
        )
        .unwrap();
    assert_eq!(
        try_issue_support_certificates(
            reordered,
            &dag,
            &context,
            std::slice::from_ref(&one),
            &[n_plus_one.clone(), n.clone()],
        )
        .unwrap_err(),
        ModularGuideError::InconsistentBatchQueryLayout
    );

    let truncated = ModularProbe::try_new(&dag, &context, 1, PRIME, &[2], limits)
        .unwrap()
        .try_evaluate_guarded_batch(
            &dag,
            std::slice::from_ref(&one),
            &[n.clone(), n_plus_one.clone()],
        )
        .unwrap();
    assert_eq!(
        try_issue_support_certificates(
            truncated,
            &dag,
            &context,
            std::slice::from_ref(&one),
            std::slice::from_ref(&n),
        )
        .unwrap_err(),
        ModularGuideError::InconsistentBatchQueryLayout
    );

    let duplicate = ModularProbe::try_new(&dag, &context, 2, PRIME, &[2], limits)
        .unwrap()
        .try_evaluate_guarded_batch(&dag, &[], &[n.clone(), n_plus_one.clone()])
        .unwrap();
    assert_eq!(
        try_issue_support_certificates(duplicate, &dag, &context, &[], &[n.clone(), n.clone()],)
            .unwrap_err(),
        ModularGuideError::InconsistentBatchQueryLayout
    );

    let foreign_batch = ModularProbe::try_new(&dag, &context, 3, PRIME, &[2], limits)
        .unwrap()
        .try_evaluate_guarded_batch(&dag, &[], std::slice::from_ref(&n))
        .unwrap();
    let (_, other_context) = make_context("elc1a-layout-foreign");
    let other_dag = ModularCoefficientDag::try_new(&other_context, limits).unwrap();
    assert_eq!(
        try_issue_support_certificates(
            foreign_batch,
            &other_dag,
            &other_context,
            &[],
            std::slice::from_ref(&n),
        )
        .unwrap_err(),
        ModularGuideError::WrongDagOwner
    );

    let wrong_context_batch = ModularProbe::try_new(&dag, &context, 4, PRIME, &[2], limits)
        .unwrap()
        .try_evaluate_guarded_batch(&dag, &[], std::slice::from_ref(&n))
        .unwrap();
    assert_eq!(
        try_issue_support_certificates(
            wrong_context_batch,
            &dag,
            &other_context,
            &[],
            std::slice::from_ref(&n),
        )
        .unwrap_err(),
        ModularGuideError::WrongIndexedContext
    );

    let checkpoint = dag.checkpoint();
    let transient = dag.try_add(&n_plus_one, &n).unwrap();
    let stale_batch = ModularProbe::try_new(&dag, &context, 5, PRIME, &[2], limits)
        .unwrap()
        .try_evaluate_guarded_batch(&dag, &[], std::slice::from_ref(&transient))
        .unwrap();
    dag.try_rollback(checkpoint).unwrap();
    assert!(matches!(
        try_issue_support_certificates(
            stale_batch,
            &dag,
            &context,
            &[],
            std::slice::from_ref(&transient),
        ),
        Err(ModularGuideError::StaleDagReference { .. })
    ));

    let checkpoint = dag.checkpoint();
    let transient = dag.try_add(&n_plus_one, &n).unwrap();
    let certified = try_certify_batch(
        &dag,
        &context,
        &[],
        &[n_plus_one.clone(), transient],
        6,
        PRIME,
        &[2],
        limits,
    )
    .unwrap()
    .into_outcomes()
    .into_vec();
    dag.try_rollback(checkpoint).unwrap();
    let NonzeroCertification::Certified(prefix_certificate) = &certified[0] else {
        panic!("the persistent prefix root should initially be certified")
    };
    assert!(!prefix_certificate.owns(&dag, &context, &n_plus_one));
}

#[test]
fn guard_zero_and_later_singularity_reject_whole_batch_with_census_only() {
    let (_, context) = make_context("elc1a-atomic-probe-rejection");
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let one = dag.one();
    let exact_n = context.index(0).unwrap();
    let exact_neg_n = context.mul(&context.integer(-1), &exact_n).unwrap();
    let neg_n_leaf = leaf(&mut dag, &context, exact_neg_n);
    let nonsyntactic_zero = dag.try_add(&n, &neg_n_leaf).unwrap();
    assert!(!dag.is_known_zero(&nonsyntactic_zero).unwrap());
    let inverse = dag.try_inv(&nonsyntactic_zero).unwrap();

    let guard_rejection = try_certify_batch(
        &dag,
        &context,
        std::slice::from_ref(&n),
        std::slice::from_ref(&inverse),
        0,
        PRIME,
        &[0],
        limits,
    )
    .unwrap_err();
    assert_eq!(
        guard_rejection.error(),
        &ModularGuideError::SampledZeroLocalizationGuard
    );
    assert_eq!(guard_rejection.census().queries(), 1);
    assert!(guard_rejection.census().evaluation_steps() > 0);

    let singular_rejection =
        try_certify_batch(&dag, &context, &[], &[one, inverse], 1, PRIME, &[0], limits)
            .unwrap_err();
    assert!(matches!(
        singular_rejection.error(),
        ModularGuideError::SingularInverse { .. }
    ));
    assert_eq!(singular_rejection.census().queries(), 2);
    assert!(singular_rejection.census().evaluation_steps() >= 2);
}

#[test]
fn guarded_batch_preflights_one_below_resource_limits() {
    let (_, context) = make_context("elc1a-probe-one-below");
    let default_limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, default_limits).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let one = dag.one();

    let too_few_queries = ModularGuideLimits {
        max_probe_queries: 1,
        ..default_limits
    };
    let report = ModularProbe::try_new(&dag, &context, 0, PRIME, &[2], too_few_queries)
        .unwrap()
        .try_evaluate_guarded_batch(&dag, std::slice::from_ref(&one), std::slice::from_ref(&n))
        .unwrap_err();
    assert_eq!(
        report.error(),
        &ModularGuideError::ResourceLimit {
            resource: "modular guide probe queries",
            requested: 2,
            limit: 1,
        }
    );
    assert_eq!(report.census().queries(), 0);

    let too_few_images = ModularGuideLimits {
        max_probe_batch_images: 1,
        ..default_limits
    };
    let report = ModularProbe::try_new(&dag, &context, 1, PRIME, &[2], too_few_images)
        .unwrap()
        .try_evaluate_guarded_batch(&dag, std::slice::from_ref(&one), std::slice::from_ref(&n))
        .unwrap_err();
    assert_eq!(
        report.error(),
        &ModularGuideError::ResourceLimit {
            resource: "modular guide completed batch images",
            requested: 2,
            limit: 1,
        }
    );
    assert_eq!(report.census().queries(), 0);

    let exact_queries = ModularGuideLimits {
        max_probe_queries: 2,
        max_probe_batch_images: 2,
        ..default_limits
    };
    let batch = ModularProbe::try_new(&dag, &context, 2, PRIME, &[2], exact_queries)
        .unwrap()
        .try_evaluate_guarded_batch(&dag, std::slice::from_ref(&one), std::slice::from_ref(&n))
        .unwrap();
    assert_eq!(batch.census().queries(), 2);

    let no_cache = ModularGuideLimits {
        max_probe_cached_values: 0,
        ..default_limits
    };
    let report = ModularProbe::try_new(&dag, &context, 3, PRIME, &[2], no_cache)
        .unwrap()
        .try_evaluate_guarded_batch(&dag, &[], std::slice::from_ref(&n))
        .unwrap_err();
    assert!(matches!(
        report.error(),
        ModularGuideError::ResourceLimit {
            resource: "modular guide cached values",
            requested: 1,
            limit: 0,
        }
    ));
    assert_eq!(report.census().queries(), 1);
    assert_eq!(report.census().evaluation_steps(), 1);
}

#[test]
fn exact_batch_is_ordered_root_bound_and_reuses_one_iterative_cache() {
    let (_, context) = make_context("elc1a-exact-batch-cache");
    let guide_limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, guide_limits).unwrap();
    let exact_n = context.index(0).unwrap();
    let n = leaf(&mut dag, &context, exact_n.clone());
    let one = dag.one();
    let shared = dag.try_add(&n, &one).unwrap();
    let square = dag.try_mul(&shared, &shared).unwrap();
    let shifted = dag.try_add(&shared, &n).unwrap();
    let roots = [square.clone(), shifted.clone()];
    let mut budget = ExactMaterializationBudget::new(ExactMaterializerLimits::default());
    let batch = try_materialize_exact_batch(&dag, &context, &roots, &mut budget).unwrap();
    assert!(batch.owns(&dag, &context, &roots));
    assert_eq!(batch.roots(), roots.as_slice());
    assert_eq!(batch.materializations().len(), 2);
    assert!(batch.census().cache_hits() > 0);
    assert_eq!(batch.census().output_values(), 2);
    assert_eq!(budget.attempts(), 1);

    let exact_shared = context.add(&exact_n, &context.one()).unwrap();
    let exact_square = context.mul(&exact_shared, &exact_shared).unwrap();
    let exact_shifted = context.add(&exact_shared, &exact_n).unwrap();
    assert_eq!(batch.materializations()[0].value(), &exact_square);
    assert_eq!(batch.materializations()[1].value(), &exact_shifted);
    assert!(batch.materializations()[0].owns(&dag, &context, &square));
    assert!(!batch.materializations()[0].owns(&dag, &context, &shifted));

    let scalar = try_materialize_exact(
        &dag,
        &context,
        &square,
        &mut ExactMaterializationBudget::new(ExactMaterializerLimits::default()),
    )
    .unwrap();
    assert_eq!(scalar.value(), batch.materializations()[0].value());
}

#[test]
fn exact_batch_rejects_foreign_stale_singular_and_one_below_without_partial_output() {
    let (_, context) = make_context("elc1a-exact-batch-rejection");
    let guide_limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, guide_limits).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let one = dag.one();

    let root_cap = ExactMaterializerLimits {
        max_batch_roots: 1,
        ..ExactMaterializerLimits::default()
    };
    let mut root_budget = ExactMaterializationBudget::new(root_cap);
    assert_eq!(
        try_materialize_exact_batch(&dag, &context, &[one.clone(), n.clone()], &mut root_budget)
            .unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "exact materializer batch roots",
            requested: 2,
            limit: 1,
        }
    );
    assert_eq!(root_budget.attempts(), 1);
    assert_eq!(root_budget.census().output_values(), 0);

    let output_cap = ExactMaterializerLimits {
        max_output_values: 1,
        ..ExactMaterializerLimits::default()
    };
    let mut output_budget = ExactMaterializationBudget::new(output_cap);
    assert_eq!(
        try_materialize_exact_batch(
            &dag,
            &context,
            &[one.clone(), n.clone()],
            &mut output_budget,
        )
        .unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "exact materializer output values",
            requested: 2,
            limit: 1,
        }
    );
    assert_eq!(output_budget.attempts(), 1);
    assert_eq!(output_budget.census().traversal_steps(), 0);
    assert_eq!(output_budget.census().output_values(), 0);

    let exact_boundary = ExactMaterializerLimits {
        max_batch_roots: 2,
        max_output_values: 2,
        ..ExactMaterializerLimits::default()
    };
    let mut exact_boundary_budget = ExactMaterializationBudget::new(exact_boundary);
    let exact_boundary_batch = try_materialize_exact_batch(
        &dag,
        &context,
        &[one.clone(), n.clone()],
        &mut exact_boundary_budget,
    )
    .unwrap();
    assert_eq!(exact_boundary_batch.materializations().len(), 2);
    assert_eq!(exact_boundary_budget.census().output_values(), 2);

    let exact_n = context.index(0).unwrap();
    let exact_neg_n = context.mul(&context.integer(-1), &exact_n).unwrap();
    let neg_n_leaf = leaf(&mut dag, &context, exact_neg_n);
    let nonsyntactic_zero = dag.try_add(&n, &neg_n_leaf).unwrap();
    assert!(!dag.is_known_zero(&nonsyntactic_zero).unwrap());
    let inverse = dag.try_inv(&nonsyntactic_zero).unwrap();
    let mut singular_budget = ExactMaterializationBudget::new(ExactMaterializerLimits::default());
    assert!(matches!(
        try_materialize_exact_batch(
            &dag,
            &context,
            &[one.clone(), inverse],
            &mut singular_budget,
        ),
        Err(ModularGuideError::ExactZeroInverse { .. })
    ));
    assert!(singular_budget.census().traversal_steps() > 0);
    assert_eq!(singular_budget.census().output_values(), 0);

    let (_, foreign_context) = make_context("elc1a-exact-batch-foreign");
    let foreign_dag = ModularCoefficientDag::try_new(&foreign_context, guide_limits).unwrap();
    let mut foreign_budget = ExactMaterializationBudget::new(ExactMaterializerLimits::default());
    assert_eq!(
        try_materialize_exact_batch(
            &foreign_dag,
            &foreign_context,
            std::slice::from_ref(&n),
            &mut foreign_budget,
        )
        .unwrap_err(),
        ModularGuideError::WrongDagOwner
    );

    let checkpoint = dag.checkpoint();
    let transient = dag.try_add(&n, &one).unwrap();
    dag.try_rollback(checkpoint).unwrap();
    let mut stale_budget = ExactMaterializationBudget::new(ExactMaterializerLimits::default());
    assert!(matches!(
        try_materialize_exact_batch(
            &dag,
            &context,
            std::slice::from_ref(&transient),
            &mut stale_budget,
        ),
        Err(ModularGuideError::StaleDagReference { .. })
    ));
}
