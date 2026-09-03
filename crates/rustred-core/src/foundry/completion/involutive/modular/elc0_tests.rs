use std::{mem::size_of, sync::Arc};

use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::prelude::Integer;

use crate::algebra::{CoefficientContext, IndexedCoefficient, IndexedCoefficientContext};
use crate::sector::{Mask, OrderingPolicy};

use super::super::{ForwardShift, InvolutiveLimits, OreOrderingAdapter};
use super::{
    ExactMaterialization, ExactMaterializationBudget, ExactMaterializerLimits,
    ModularCoefficientDag, ModularGuideError, ModularGuideLimits, ModularProbe,
    NonzeroCertification, try_certify_batch, try_materialize_exact,
};

const PRIME: u64 = 998_244_353;

fn context(
    base_names: impl IntoIterator<Item = &'static str>,
    scope: &str,
    arity: usize,
) -> (CoefficientContext, IndexedCoefficientContext) {
    let base = CoefficientContext::new(base_names);
    let indexed = IndexedCoefficientContext::try_new(&base, scope, arity).unwrap();
    (base, indexed)
}

fn leaf(
    dag: &mut ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    coefficient: IndexedCoefficient,
) -> super::CoeffRef {
    dag.try_exact_leaf(context, Arc::new(coefficient)).unwrap()
}

fn materialize_once(
    dag: &ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    root: &super::CoeffRef,
    limits: ExactMaterializerLimits,
) -> Result<ExactMaterialization, ModularGuideError> {
    let mut budget = ExactMaterializationBudget::new(limits);
    try_materialize_exact(dag, context, root, &mut budget)
}

fn certify_once(
    dag: &ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    root: &super::CoeffRef,
    ordinal: usize,
    point: &[i64],
    limits: ModularGuideLimits,
) -> Result<NonzeroCertification, super::RejectedProbeReport> {
    let mut outcomes = try_certify_batch(
        dag,
        context,
        &[],
        std::slice::from_ref(root),
        ordinal,
        PRIME,
        point,
        limits,
    )?
    .into_outcomes()
    .into_vec();
    Ok(outcomes
        .pop()
        .expect("one-root support batch has one outcome"))
}

#[test]
fn iterative_probe_evaluates_a_more_than_4096_deep_active_inactive_ore_chain() {
    const NESTED_ADDITIONS: usize = 4_097;
    let (_, context) = context([], "elc0-deep-active-inactive", 2);
    let exact_limits = InvolutiveLimits::default();
    let ordering = OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new([true, false]).unwrap(),
        exact_limits,
    )
    .unwrap();
    let operator = ForwardShift::try_new([1, 1], exact_limits).unwrap();
    assert_eq!(
        ordering.try_physical_translation(&operator).unwrap(),
        [1, -1]
    );

    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let exact_term = context.add(&n0, &n1).unwrap();
    let exact_translated = context
        .translate(&exact_term, &[1, -1], exact_limits.indexed_algebra)
        .unwrap();
    let expected = context
        .mul(
            &context.integer(i64::try_from(NESTED_ADDITIONS + 1).unwrap()),
            &exact_translated,
        )
        .unwrap();

    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let n0_ref = leaf(&mut dag, &context, n0);
    let n1_ref = leaf(&mut dag, &context, n1);
    let term = dag.try_add(&n0_ref, &n1_ref).unwrap();
    let translated = dag
        .try_translate_by_operator(&term, &operator, &ordering)
        .unwrap();
    let mut root = translated.clone();
    for _ in 0..NESTED_ADDITIONS {
        root = dag.try_add(&root, &translated).unwrap();
    }
    let expected = leaf(&mut dag, &context, expected);

    let batch = ModularProbe::try_new(&dag, &context, 0, PRIME, &[3, 7], limits)
        .unwrap()
        .try_evaluate_batch(&dag, &[root, expected])
        .unwrap();
    assert_eq!(batch.images()[0], batch.images()[1]);
    assert!(batch.census().evaluation_frame_pushes() > 4_096);
    assert!(
        batch.census().peak_live_evaluation_frames() > 4_096
            || batch.census().peak_live_evaluation_values() > 4_096
    );

    // At (n0,n1)=(3,7), the sector-aware image is (3+1)+(7-1)=10.
    let field = Zp64::new(PRIME);
    let expected =
        Integer::from(10 * i64::try_from(NESTED_ADDITIONS + 1).unwrap()).to_finite_field(&field);
    assert_eq!(*batch.images()[0].value(), expected);
}

#[test]
fn iterative_probe_stack_is_explicitly_bounded_and_poisoned_on_exhaustion() {
    let (_, context) = context([], "elc0-probe-stack-bound", 1);
    let mut dag = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let one = dag.one();
    let sum = dag.try_add(&n, &one).unwrap();
    let limits = ModularGuideLimits {
        max_probe_live_evaluation_frames: 1,
        ..Default::default()
    };
    assert_eq!(
        ModularProbe::try_new(&dag, &context, 0, PRIME, &[2], limits)
            .unwrap()
            .try_evaluate_batch(&dag, &[sum]),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular guide live evaluation frames",
            requested: 2,
            limit: 1,
        })
    );
}

#[test]
fn consumed_probe_preflights_the_complete_query_batch_before_retention() {
    let (_, context) = context([], "elc0-consumed-batch-preflight", 1);
    let base_limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, base_limits).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let limits = ModularGuideLimits {
        max_probe_batch_images: 0,
        ..base_limits
    };
    assert_eq!(
        ModularProbe::try_new(&dag, &context, 0, PRIME, &[2], limits)
            .unwrap()
            .try_evaluate_batch(&dag, &[n]),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular guide completed batch images",
            requested: 1,
            limit: 0,
        })
    );
}

#[test]
fn exact_materializer_matches_symbolica_for_sector_aware_rational_translation() {
    let (base, context) = context(["d"], "elc0-exact-differential", 2);
    let exact_limits = InvolutiveLimits::default();
    let ordering = OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new([true, false]).unwrap(),
        exact_limits,
    )
    .unwrap();
    let operator = ForwardShift::try_new([2, 3], exact_limits).unwrap();
    let physical = ordering.try_physical_translation(&operator).unwrap();
    assert_eq!(physical, [2, -3]);

    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let product = context.mul(&n0, &n1).unwrap();
    let translated_product = context
        .translate(&product, &physical, exact_limits.indexed_algebra)
        .unwrap();
    let translated_n0 = context
        .translate(&n0, &physical, exact_limits.indexed_algebra)
        .unwrap();
    let exact = context
        .div(
            &context.add(&d, &translated_product).unwrap(),
            &context.add(&translated_n0, &context.integer(2)).unwrap(),
        )
        .unwrap();

    let mut dag = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    let d_ref = leaf(&mut dag, &context, d);
    let n0_ref = leaf(&mut dag, &context, n0);
    let n1_ref = leaf(&mut dag, &context, n1);
    let product_ref = dag.try_mul(&n0_ref, &n1_ref).unwrap();
    let translated_product_ref = dag
        .try_translate_by_operator(&product_ref, &operator, &ordering)
        .unwrap();
    let translated_n0_ref = dag
        .try_translate_by_operator(&n0_ref, &operator, &ordering)
        .unwrap();
    let numerator = dag.try_add(&d_ref, &translated_product_ref).unwrap();
    let two = leaf(&mut dag, &context, context.integer(2));
    let denominator = dag.try_add(&translated_n0_ref, &two).unwrap();
    let root = dag.try_div(&numerator, &denominator).unwrap();

    let materialized =
        materialize_once(&dag, &context, &root, ExactMaterializerLimits::default()).unwrap();
    assert_eq!(materialized.value(), &exact);
    assert!(materialized.owns(&dag, &context, &root));
    assert!(!materialized.owns(&dag, &context, &numerator));
    let census = materialized.census();
    assert!(census.traversal_steps() > 0);
    assert!(census.frame_pushes() > census.traversal_steps());
    assert!(census.peak_live_frames() > 0);
    assert!(census.peak_live_values() > 0);
    assert!(census.exact_operations() > 0);
    assert!(census.delta_compositions() > 0);
    assert!(census.delta_coordinate_operations() > 0);
    assert!(census.cached_values() > 0);
    assert!(census.retained_terms() >= census.output_terms());
    assert!(census.retained_exponent_cells() >= census.output_exponent_cells());
    assert!(census.retained_bytes() >= census.output_bytes());
}

#[test]
fn exact_materializer_resolves_nonsyntactic_zero_and_rejects_its_inverse() {
    let (_, context) = context([], "elc0-nonsyntactic-zero", 1);
    let mut dag = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let one = dag.one();
    let n_plus_one = dag.try_add(&n, &one).unwrap();
    let translated_n = dag.try_translate_physical(&n, &[1]).unwrap();
    let zero = dag.try_sub(&n_plus_one, &translated_n).unwrap();
    assert!(!dag.is_known_zero(&zero).unwrap());
    let materialized =
        materialize_once(&dag, &context, &zero, ExactMaterializerLimits::default()).unwrap();
    assert!(materialized.value().is_zero());

    let inverse = dag.try_inv(&zero).unwrap();
    assert!(matches!(
        materialize_once(&dag, &context, &inverse, ExactMaterializerLimits::default(),),
        Err(ModularGuideError::ExactZeroInverse { .. })
    ));
}

#[test]
fn exact_materializer_enforces_cumulative_retention_work_and_output_limits() {
    let (_, context) = context([], "elc0-materializer-limits", 1);
    let mut dag = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let one = dag.one();
    let root = dag.try_add(&n, &one).unwrap();
    let baseline =
        materialize_once(&dag, &context, &root, ExactMaterializerLimits::default()).unwrap();
    let census = baseline.census();

    assert!(matches!(
        materialize_once(
            &dag,
            &context,
            &root,
            ExactMaterializerLimits {
                max_exact_operations: 0,
                ..Default::default()
            },
        ),
        Err(ModularGuideError::ResourceLimit {
            resource: "exact materializer Symbolica operations",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(
        materialize_once(
            &dag,
            &context,
            &root,
            ExactMaterializerLimits {
                max_retained_terms: census.retained_terms() - 1,
                ..Default::default()
            },
        )
        .unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "exact materializer retained coefficient terms",
            requested: census.retained_terms(),
            limit: census.retained_terms() - 1,
        }
    );
    assert_eq!(
        materialize_once(
            &dag,
            &context,
            &root,
            ExactMaterializerLimits {
                max_output_terms: census.output_terms() - 1,
                ..Default::default()
            },
        )
        .unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "exact materializer output coefficient terms",
            requested: census.output_terms(),
            limit: census.output_terms() - 1,
        }
    );

    let cumulative_limits = ExactMaterializerLimits {
        max_exact_operations: census.exact_operations(),
        ..Default::default()
    };
    let mut cumulative = ExactMaterializationBudget::new(cumulative_limits);
    try_materialize_exact(&dag, &context, &root, &mut cumulative).unwrap();
    assert_eq!(
        try_materialize_exact(&dag, &context, &root, &mut cumulative).unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "exact materializer Symbolica operations",
            requested: census.exact_operations() + 1,
            limit: census.exact_operations(),
        }
    );
    assert_eq!(cumulative.attempts(), 2);
    assert_eq!(
        cumulative.census().exact_operations(),
        census.exact_operations()
    );
}

#[test]
fn certified_nonzero_is_replayed_and_sampled_zero_remains_unresolved() {
    let (_, context) = context([], "elc0-nonzero-certificate", 1);
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let one = dag.one();
    let nonzero = dag.try_add(&n, &one).unwrap();
    let certified = match certify_once(&dag, &context, &nonzero, 7, &[2], limits).unwrap() {
        NonzeroCertification::Certified(certificate) => certificate,
        NonzeroCertification::Unresolved(_) => panic!("nonzero image remained unresolved"),
    };
    assert!(certified.owns(&dag, &context, &nonzero));
    assert!(!certified.owns(&dag, &context, &n));
    assert_eq!(certified.probe().ordinal(), 7);
    assert_eq!(certified.residue(), 3);

    let unresolved = match certify_once(&dag, &context, &n, 8, &[0], limits).unwrap() {
        NonzeroCertification::Certified(_) => panic!("sampled zero became a certificate"),
        NonzeroCertification::Unresolved(unresolved) => unresolved,
    };
    assert!(unresolved.owns(&dag, &context, &n));
    assert_eq!(unresolved.probe().ordinal(), 8);

    let structural_zero = dag.try_sub(&n, &n).unwrap();
    assert_eq!(
        certify_once(&dag, &context, &structural_zero, 9, &[0], limits)
            .unwrap_err()
            .into_error(),
        ModularGuideError::KnownZeroCannotBeCertified
    );

    let checkpoint = dag.checkpoint();
    let transient = dag.try_add(&nonzero, &n).unwrap();
    let transient_certificate =
        match certify_once(&dag, &context, &transient, 10, &[2], limits).unwrap() {
            NonzeroCertification::Certified(certificate) => certificate,
            NonzeroCertification::Unresolved(_) => panic!("nonzero transient image was unresolved"),
        };
    assert!(transient_certificate.owns(&dag, &context, &transient));
    dag.try_rollback(checkpoint).unwrap();
    assert!(!transient_certificate.owns(&dag, &context, &transient));

    // A different node immediately reuses the rolled-back ordinal.  Its
    // monotone incarnation must keep the stale proof from resurrecting.
    let replacement = dag.try_mul(&nonzero, &n).unwrap();
    assert_eq!(replacement.node_ordinal(), transient.node_ordinal());
    assert_ne!(replacement, transient);
    assert!(!transient_certificate.owns(&dag, &context, &replacement));
    assert!(matches!(
        dag.raw(&transient),
        Err(ModularGuideError::StaleDagReference {
            resource: "coefficient node",
            ..
        })
    ));
}

#[test]
fn rollback_slot_reuse_cannot_resurrect_stale_batches_or_translations() {
    let (_, context) = context([], "elc0-slot-incarnations", 1);
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let prefix_batch = ModularProbe::try_new(&dag, &context, 0, PRIME, &[3], limits)
        .unwrap()
        .try_evaluate_batch(&dag, std::slice::from_ref(&n))
        .unwrap();

    let checkpoint = dag.checkpoint();
    let shifted_once = dag.try_translate_physical(&n, &[1]).unwrap();
    let shifted_materialization = materialize_once(
        &dag,
        &context,
        &shifted_once,
        ExactMaterializerLimits::default(),
    )
    .unwrap();
    assert!(shifted_materialization.owns(&dag, &context, &shifted_once));
    let stale_batch = ModularProbe::try_new(&dag, &context, 1, PRIME, &[3], limits)
        .unwrap()
        .try_evaluate_batch(&dag, std::slice::from_ref(&shifted_once))
        .unwrap();
    assert!(stale_batch.owns_dag(&dag));

    dag.try_rollback(checkpoint).unwrap();
    assert!(prefix_batch.owns_dag(&dag));
    assert!(!stale_batch.owns_dag(&dag));
    assert!(!shifted_materialization.owns(&dag, &context, &shifted_once));

    // Reuse the same physical-delta ordinal with a different translation.
    // Equality and DAG admission both include the new incarnation.
    let shifted_twice = dag.try_translate_physical(&n, &[2]).unwrap();
    assert_eq!(
        shifted_once.raw.translation.ordinal(),
        shifted_twice.raw.translation.ordinal()
    );
    assert_ne!(shifted_once, shifted_twice);
    assert!(!stale_batch.owns_dag(&dag));
    assert!(!shifted_materialization.owns(&dag, &context, &shifted_twice));
    assert!(matches!(
        dag.raw(&shifted_once),
        Err(ModularGuideError::StaleDagReference {
            resource: "physical translation",
            ..
        })
    ));

    // Incarnation safety stays compact enough for million-node guide DAGs.
    assert_eq!(size_of::<super::model::CoeffNodeId>(), size_of::<u64>());
    assert_eq!(size_of::<super::model::PhysicalDeltaId>(), size_of::<u64>());
    assert_eq!(size_of::<super::model::RawCoeffRef>(), 2 * size_of::<u64>());
}

#[test]
fn exact_leaf_payload_and_dag_churn_are_bounded_across_rollback() {
    let (_, context) = context([], "elc0-arena-payload-bounds", 1);
    let coefficient = context.index(0).unwrap();

    let mut measured = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    leaf(&mut measured, &context, coefficient.clone());
    let (terms, exponent_cells, bytes) = measured.exact_leaf_payload_census();
    assert!(terms > 0 && exponent_cells > 0 && bytes > 0);

    let base = ModularGuideLimits::default();
    let one_below = [
        (
            ModularGuideLimits {
                max_exact_leaf_terms: terms - 1,
                ..base
            },
            "modular coefficient exact-leaf terms",
            terms,
            terms - 1,
        ),
        (
            ModularGuideLimits {
                max_exact_leaf_exponent_cells: exponent_cells - 1,
                ..base
            },
            "modular coefficient exact-leaf exponent cells",
            exponent_cells,
            exponent_cells - 1,
        ),
        (
            ModularGuideLimits {
                max_exact_leaf_retained_bytes: bytes - 1,
                ..base
            },
            "modular coefficient exact-leaf retained bytes",
            bytes,
            bytes - 1,
        ),
    ];
    for (limits, resource, requested, limit) in one_below {
        let mut capped = ModularCoefficientDag::try_new(&context, limits).unwrap();
        assert_eq!(
            capped
                .try_exact_leaf(&context, Arc::new(coefficient.clone()))
                .unwrap_err(),
            ModularGuideError::ResourceLimit {
                resource,
                requested,
                limit,
            }
        );
        assert_eq!(capped.exact_leaf_payload_census(), (0, 0, 0));
    }

    let cumulative_limits = ModularGuideLimits {
        max_exact_leaf_terms: terms,
        max_exact_leaf_exponent_cells: exponent_cells,
        max_exact_leaf_retained_bytes: bytes,
        max_total_exact_leaf_terms_ingressed: terms,
        max_total_exact_leaf_exponent_cells_ingressed: exponent_cells,
        max_total_exact_leaf_bytes_ingressed: bytes,
        ..base
    };
    let mut cumulative = ModularCoefficientDag::try_new(&context, cumulative_limits).unwrap();
    let checkpoint = cumulative.checkpoint();
    leaf(&mut cumulative, &context, coefficient.clone());
    assert_eq!(
        cumulative.exact_leaf_payload_census(),
        (terms, exponent_cells, bytes)
    );
    cumulative.try_rollback(checkpoint).unwrap();
    assert_eq!(cumulative.exact_leaf_payload_census(), (0, 0, 0));
    assert_eq!(
        cumulative
            .try_exact_leaf(&context, Arc::new(coefficient.clone()))
            .unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "total modular coefficient exact-leaf terms ingressed",
            requested: 2 * terms,
            limit: terms,
        }
    );

    let repeat_limits = ModularGuideLimits {
        max_total_exact_leaf_terms_ingressed: 2 * terms,
        max_total_exact_leaf_exponent_cells_ingressed: 2 * exponent_cells,
        max_total_exact_leaf_bytes_ingressed: 2 * bytes,
        ..base
    };
    let mut repeated = ModularCoefficientDag::try_new(&context, repeat_limits).unwrap();
    leaf(&mut repeated, &context, coefficient.clone());
    leaf(&mut repeated, &context, coefficient.clone());
    assert_eq!(repeated.exact_leaf_count(), 1);
    assert_eq!(
        repeated.cumulative_creation_census().3,
        2 * terms,
        "already-interned ingress still consumes bounded hashing/equality work"
    );
    assert_eq!(
        repeated
            .try_exact_leaf(&context, Arc::new(coefficient.clone()))
            .unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "total modular coefficient exact-leaf terms ingressed",
            requested: 3 * terms,
            limit: 2 * terms,
        }
    );

    let node_limits = ModularGuideLimits {
        max_total_nodes_created: 3,
        ..base
    };
    let mut node_capped = ModularCoefficientDag::try_new(&context, node_limits).unwrap();
    let checkpoint = node_capped.checkpoint();
    leaf(&mut node_capped, &context, coefficient.clone());
    node_capped.try_rollback(checkpoint).unwrap();
    assert_eq!(
        node_capped
            .try_exact_leaf(&context, Arc::new(coefficient.clone()))
            .unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "total modular coefficient DAG nodes created",
            requested: 4,
            limit: 3,
        }
    );

    let delta_limits = ModularGuideLimits {
        max_total_physical_delta_coordinate_operations: 1,
        ..base
    };
    let mut delta_capped = ModularCoefficientDag::try_new(&context, delta_limits).unwrap();
    let n = leaf(&mut delta_capped, &context, coefficient);
    let checkpoint = delta_capped.checkpoint();
    delta_capped.try_translate_physical(&n, &[1]).unwrap();
    delta_capped.try_rollback(checkpoint).unwrap();
    assert_eq!(
        delta_capped.try_translate_physical(&n, &[2]).unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "total modular physical-delta coordinate operations",
            requested: 2,
            limit: 1,
        }
    );
    let (nodes_created, deltas_created, coordinate_operations, _, _, _) =
        delta_capped.cumulative_creation_census();
    assert_eq!(nodes_created, 3);
    assert_eq!(deltas_created, 2);
    assert_eq!(coordinate_operations, 1);

    let delta_creation_limits = ModularGuideLimits {
        max_total_physical_deltas_created: 2,
        ..base
    };
    let mut delta_creation_capped =
        ModularCoefficientDag::try_new(&context, delta_creation_limits).unwrap();
    let n = leaf(
        &mut delta_creation_capped,
        &context,
        context.index(0).unwrap(),
    );
    let checkpoint = delta_creation_capped.checkpoint();
    delta_creation_capped
        .try_translate_physical(&n, &[1])
        .unwrap();
    delta_creation_capped.try_rollback(checkpoint).unwrap();
    assert_eq!(
        delta_creation_capped
            .try_translate_physical(&n, &[2])
            .unwrap_err(),
        ModularGuideError::ResourceLimit {
            resource: "total modular coefficient physical deltas created",
            requested: 3,
            limit: 2,
        }
    );
}

#[test]
fn arena_checkpoints_are_owned_consuming_and_prefix_incarnation_safe() {
    let (_, context) = context([], "elc0-checkpoint-authority", 1);
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let foreign = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let foreign_checkpoint = foreign.checkpoint();
    assert_eq!(
        dag.try_rollback(foreign_checkpoint).unwrap_err(),
        ModularGuideError::WrongDagOwner
    );
    assert_eq!(dag.node_count(), 2);

    let base = dag.checkpoint();
    let old = leaf(&mut dag, &context, context.index(0).unwrap());
    let stale_same_count = dag.checkpoint();
    dag.try_neg(&old).unwrap();
    let future = dag.checkpoint();
    dag.try_rollback(base).unwrap();
    assert_eq!(dag.node_count(), 2);
    assert_eq!(
        dag.try_rollback(future).unwrap_err(),
        ModularGuideError::InvalidArenaCheckpoint {
            detail: "retained counts are outside the current arena prefix",
        }
    );

    let replacement_coefficient = context
        .add(&context.index(0).unwrap(), &context.integer(1))
        .unwrap();
    let replacement = leaf(&mut dag, &context, replacement_coefficient);
    assert_eq!(old.node_ordinal(), replacement.node_ordinal());
    assert_ne!(old, replacement);
    assert_eq!(
        dag.try_rollback(stale_same_count).unwrap_err(),
        ModularGuideError::InvalidArenaCheckpoint {
            detail: "coefficient-node prefix incarnation has changed",
        }
    );
    assert!(dag.raw(&replacement).is_ok());
    assert!(matches!(
        dag.raw(&old),
        Err(ModularGuideError::StaleDagReference {
            resource: "coefficient node",
            ..
        })
    ));
}

#[test]
fn certificate_batch_rejects_invalid_probe_and_singular_root() {
    let (_, context) = context([], "elc0-certificate-singularity", 1);
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    assert_eq!(
        certify_once(&dag, &context, &n, 0, &[2, 3], limits)
            .unwrap_err()
            .into_error(),
        ModularGuideError::WrongPointArity {
            expected: 1,
            actual: 2,
        }
    );

    let inverse = dag.try_inv(&n).unwrap();
    assert!(matches!(
        certify_once(&dag, &context, &inverse, 1, &[0], limits)
            .unwrap_err()
            .into_error(),
        ModularGuideError::SingularInverse { .. }
    ));
}

#[test]
fn iterative_probe_preserves_left_to_right_singularity_chronology() {
    let (_, context) = context([], "elc0-singularity-chronology", 2);
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let n0 = leaf(&mut dag, &context, context.index(0).unwrap());
    let n1 = leaf(&mut dag, &context, context.index(1).unwrap());
    let left = dag.try_inv(&n0).unwrap();
    let right = dag.try_inv(&n1).unwrap();
    let sum = dag.try_add(&left, &right).unwrap();
    let error = ModularProbe::try_new(&dag, &context, 0, PRIME, &[0, 0], limits)
        .unwrap()
        .try_evaluate_batch(&dag, &[sum])
        .unwrap_err();
    match error {
        ModularGuideError::SingularInverse { node } => {
            assert_eq!(node.ordinal(), left.node_ordinal());
        }
        other => panic!("unexpected singularity result: {other}"),
    }
}
