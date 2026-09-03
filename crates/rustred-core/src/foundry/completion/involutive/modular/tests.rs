use std::sync::Arc;

use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring};
use symbolica::prelude::Integer;

use crate::algebra::{CoefficientContext, IndexedCoefficient, IndexedCoefficientContext};
use crate::sector::{Mask, OrderingPolicy};

use super::super::{ForwardShift, InvolutiveLimits, OreOrderingAdapter};
use super::{
    ModularCoefficientDag, ModularGuideError, ModularGuideLimits, ModularProbe, ModularZeroEvidence,
};

const PRIME_A: u64 = 998_244_353;
const PRIME_B: u64 = 1_000_000_007;

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

#[test]
fn arena_hash_conses_nodes_and_composes_physical_translations() {
    let (_, context) = context([], "modular-dag-hash-cons", 2);
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    assert_eq!(
        dag.try_exact_leaf(&context, Arc::new(context.zero()))
            .unwrap(),
        dag.zero()
    );
    assert_eq!(
        dag.try_exact_leaf(&context, Arc::new(context.one()))
            .unwrap(),
        dag.one()
    );
    assert_eq!(dag.exact_leaf_count(), 0);
    let shared = Arc::new(context.index(0).unwrap());
    let x = dag.try_exact_leaf(&context, Arc::clone(&shared)).unwrap();
    assert_eq!(x, dag.try_exact_leaf(&context, shared).unwrap());
    assert_eq!(
        x,
        dag.try_exact_leaf(&context, Arc::new(context.index(0).unwrap()))
            .unwrap()
    );
    assert_eq!(dag.exact_leaf_count(), 1);

    let y = leaf(&mut dag, &context, context.index(1).unwrap());
    let xy = dag.try_add(&x, &y).unwrap();
    let yx = dag.try_add(&y, &x).unwrap();
    assert_eq!(xy, yx);
    let nodes_after_sum = dag.node_count();
    assert_eq!(xy, dag.try_add(&x, &y).unwrap());
    assert_eq!(dag.node_count(), nodes_after_sum);

    let negative = dag.try_neg(&xy).unwrap();
    assert_eq!(xy, dag.try_neg(&negative).unwrap());
    let cancellation = dag.try_add(&xy, &negative).unwrap();
    assert!(dag.is_known_zero(&cancellation).unwrap());

    let first = dag.try_translate_physical(&xy, &[1, -2]).unwrap();
    let nested = dag.try_translate_physical(&first, &[3, 4]).unwrap();
    let summed = dag.try_translate_physical(&xy, &[4, 2]).unwrap();
    assert_eq!(nested, summed);
    assert_eq!(dag.physical_delta_count(), 3);

    let other = ModularCoefficientDag::try_new(&context, limits).unwrap();
    assert_eq!(
        other.is_known_zero(&xy),
        Err(ModularGuideError::WrongDagOwner)
    );
    let batch = ModularProbe::try_new(&dag, &context, 0, PRIME_A, &[2, 3], limits)
        .unwrap()
        .try_evaluate_batch(&dag, std::slice::from_ref(&xy))
        .unwrap();
    assert!(batch.owns_dag(&dag));
    assert!(!batch.owns_dag(&other));
    assert_eq!(batch.queries(), std::slice::from_ref(&xy));
    let wrong_owner_probe =
        ModularProbe::try_new(&dag, &context, 1, PRIME_A, &[2, 3], limits).unwrap();
    assert_eq!(
        wrong_owner_probe.try_evaluate_batch(&other, &[other.one()]),
        Err(ModularGuideError::WrongDagOwner)
    );
    assert_eq!(
        ModularProbe::try_new(&dag, &context, 2, PRIME_A, &[2, 3], limits)
            .unwrap()
            .try_evaluate_batch(&dag, &[xy, other.one()]),
        Err(ModularGuideError::WrongDagOwner)
    );
}

#[test]
fn one_loop_recurrence_image_matches_exact_translation_and_manual_value() {
    let (base, context) = context(["d"], "modular-one-loop-differential", 1);
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let n = context.index(0).unwrap();
    let two = context.integer(2);
    let twice_n = context.mul(&two, &n).unwrap();
    let numerator = context.sub(&d, &twice_n).unwrap();
    let exact = context.div(&numerator, &twice_n).unwrap();

    let mut dag = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    let d_ref = leaf(&mut dag, &context, d);
    let n_ref = leaf(&mut dag, &context, n);
    let two_ref = leaf(&mut dag, &context, two);
    let twice_n_ref = dag.try_mul(&two_ref, &n_ref).unwrap();
    let numerator_ref = dag.try_sub(&d_ref, &twice_n_ref).unwrap();
    let recurrence_ref = dag.try_div(&numerator_ref, &twice_n_ref).unwrap();
    let translated_ref = dag.try_translate_physical(&recurrence_ref, &[2]).unwrap();

    let exact_translated = context
        .translate(&exact, &[2], InvolutiveLimits::default().indexed_algebra)
        .unwrap();
    let expected_ref = leaf(&mut dag, &context, exact_translated);
    let batch = ModularProbe::try_new(&dag, &context, 7, PRIME_A, &[11, 3], Default::default())
        .unwrap()
        .try_evaluate_batch(
            &dag,
            &[translated_ref.clone(), expected_ref, translated_ref.clone()],
        )
        .unwrap();
    let actual = batch.images()[0];
    let expected = batch.images()[1];
    assert_eq!(actual.value(), expected.value());
    assert_eq!(actual.zero_evidence(), ModularZeroEvidence::Nonzero);
    assert_eq!(batch.images()[2], actual);

    // d=11 and translated n=5 gives (11-10)/10 = 1/10.
    let field = Zp64::new(PRIME_A);
    let manual = field.div(&field.one(), &Integer::from(10).to_finite_field(&field));
    assert_eq!(*actual.value(), manual);
    assert_eq!(batch.census().queries(), 3);
    assert!(batch.census().cache_hits() > 0);
}

#[test]
fn two_loop_shaped_left_axpy_matches_exact_ore_action_in_independent_probes() {
    let (base, context) = context(["d"], "modular-two-loop-differential", 3);
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let n2 = context.index(2).unwrap();
    let one = context.one();
    let two = context.integer(2);

    // A representative K=3 rational coefficient. The derived expression
    // deliberately gives its source child an Ore translation while retaining
    // the multiplier and accumulator at the unshifted point.
    let source_numerator = context
        .sub(
            &context.sub(&d, &context.mul(&two, &n0).unwrap()).unwrap(),
            &n1,
        )
        .unwrap();
    let source_denominator = context.add(&n2, &one).unwrap();
    let source = context.div(&source_numerator, &source_denominator).unwrap();
    let multiplier = context.add(&n0, &one).unwrap();
    let accumulator = context.div(&n1, &context.sub(&d, &n2).unwrap()).unwrap();

    let exact_limits = InvolutiveLimits::default();
    let ordering = OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new([true, false, true]).unwrap(),
        exact_limits,
    )
    .unwrap();
    let forward = ForwardShift::try_new([1, 2, 3], exact_limits).unwrap();
    let physical = ordering.try_physical_translation(&forward).unwrap();
    assert_eq!(physical, [1, -2, 3]);
    let translated_source = context
        .translate(&source, &physical, exact_limits.indexed_algebra)
        .unwrap();
    let exact_axpy = context
        .add(
            &accumulator,
            &context.mul(&multiplier, &translated_source).unwrap(),
        )
        .unwrap();

    let mut dag = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    let d_ref = leaf(&mut dag, &context, d);
    let n0_ref = leaf(&mut dag, &context, n0);
    let n1_ref = leaf(&mut dag, &context, n1);
    let n2_ref = leaf(&mut dag, &context, n2);
    let one_ref = dag.one();
    let two_ref = leaf(&mut dag, &context, two);
    let two_n0_ref = dag.try_mul(&two_ref, &n0_ref).unwrap();
    let d_minus_two_n0 = dag.try_sub(&d_ref, &two_n0_ref).unwrap();
    let source_numerator_ref = dag.try_sub(&d_minus_two_n0, &n1_ref).unwrap();
    let source_denominator_ref = dag.try_add(&n2_ref, &one_ref).unwrap();
    let source_ref = dag
        .try_div(&source_numerator_ref, &source_denominator_ref)
        .unwrap();
    let translated_source_ref = dag
        .try_translate_by_operator(&source_ref, &forward, &ordering)
        .unwrap();
    let multiplier_ref = dag.try_add(&n0_ref, &one_ref).unwrap();
    let d_minus_n2 = dag.try_sub(&d_ref, &n2_ref).unwrap();
    let accumulator_ref = dag.try_div(&n1_ref, &d_minus_n2).unwrap();
    let scaled_source_ref = dag
        .try_mul(&multiplier_ref, &translated_source_ref)
        .unwrap();
    let axpy_ref = dag.try_add(&accumulator_ref, &scaled_source_ref).unwrap();
    let exact_ref = leaf(&mut dag, &context, exact_axpy);

    for (ordinal, modulus, point) in [(0, PRIME_A, [17, 2, 5, 4]), (1, PRIME_B, [19, 3, 7, 6])] {
        let batch =
            ModularProbe::try_new(&dag, &context, ordinal, modulus, &point, Default::default())
                .unwrap()
                .try_evaluate_batch(&dag, &[axpy_ref.clone(), exact_ref.clone()])
                .unwrap();
        let actual = batch.images()[0];
        let expected = batch.images()[1];
        assert_eq!(actual.value(), expected.value());
        assert_eq!(actual.zero_evidence(), expected.zero_evidence());
        assert_eq!(batch.identity().ordinal(), ordinal);
        assert_eq!(batch.identity().modulus(), modulus);
        assert_eq!(batch.identity().point(), point);
        assert!(batch.census().delta_compositions() > 0);
        assert!(batch.census().delta_coordinate_operations() > 0);
        assert!(batch.census().evaluation_steps() > 0);
        assert!(batch.census().exact_leaf_terms_evaluated() > 0);
        assert!(batch.census().exact_leaf_exponent_cells_evaluated() > 0);
    }
}

#[test]
fn structural_and_sampled_zero_are_distinct_and_singularity_rejects_only_its_lane() {
    let (_, context) = context([], "modular-zero-authority", 1);
    let mut dag = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let structural_zero = dag.try_sub(&n, &n).unwrap();
    let inverse = dag.try_inv(&n).unwrap();
    assert_eq!(
        dag.try_inv(&structural_zero),
        Err(ModularGuideError::StructurallyZeroInverse)
    );

    let known_zero_batch =
        ModularProbe::try_new(&dag, &context, 0, PRIME_A, &[0], Default::default())
            .unwrap()
            .try_evaluate_batch(&dag, std::slice::from_ref(&structural_zero))
            .unwrap();
    assert_eq!(
        known_zero_batch.images()[0].zero_evidence(),
        ModularZeroEvidence::KnownZero
    );
    let sampled_zero_batch =
        ModularProbe::try_new(&dag, &context, 1, PRIME_A, &[0], Default::default())
            .unwrap()
            .try_evaluate_batch(&dag, std::slice::from_ref(&n))
            .unwrap();
    assert_eq!(
        sampled_zero_batch.images()[0].zero_evidence(),
        ModularZeroEvidence::SampledZero
    );
    // A successful prefix never escapes if a later entry is singular.
    assert!(matches!(
        ModularProbe::try_new(&dag, &context, 2, PRIME_A, &[0], Default::default())
            .unwrap()
            .try_evaluate_batch(&dag, &[n.clone(), inverse.clone()]),
        Err(ModularGuideError::SingularInverse { .. })
    ));

    let independent = ModularProbe::try_new(&dag, &context, 3, PRIME_B, &[1], Default::default())
        .unwrap()
        .try_evaluate_batch(&dag, std::slice::from_ref(&inverse))
        .unwrap();
    assert_eq!(
        independent.images()[0].zero_evidence(),
        ModularZeroEvidence::Nonzero
    );

    let rational = context
        .div(&context.one(), &context.index(0).unwrap())
        .unwrap();
    let rational = leaf(&mut dag, &context, rational);
    assert!(matches!(
        ModularProbe::try_new(&dag, &context, 4, PRIME_A, &[0], Default::default())
            .unwrap()
            .try_evaluate_batch(&dag, &[rational]),
        Err(ModularGuideError::SingularExactLeaf { .. })
    ));
}

#[test]
fn translated_denominator_singularity_and_large_signed_offsets_are_probe_local() {
    let (_, context) = context([], "modular-shifted-singularity", 1);
    let mut dag = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    let rational = context
        .div(&context.one(), &context.index(0).unwrap())
        .unwrap();
    let rational = leaf(&mut dag, &context, rational);
    let shifted_to_pole = dag.try_translate_physical(&rational, &[-1]).unwrap();
    let regular = ModularProbe::try_new(&dag, &context, 0, PRIME_A, &[1], Default::default())
        .unwrap()
        .try_evaluate_batch(&dag, std::slice::from_ref(&rational))
        .unwrap();
    assert_eq!(
        regular.images()[0].zero_evidence(),
        ModularZeroEvidence::Nonzero
    );
    assert!(matches!(
        ModularProbe::try_new(&dag, &context, 1, PRIME_A, &[1], Default::default())
            .unwrap()
            .try_evaluate_batch(&dag, &[shifted_to_pole]),
        Err(ModularGuideError::SingularExactLeaf { .. })
    ));

    let n = leaf(&mut dag, &context, context.index(0).unwrap());
    let large = dag.try_translate_physical(&n, &[i64::MAX]).unwrap();
    let large_batch = ModularProbe::try_new(&dag, &context, 2, PRIME_B, &[-1], Default::default())
        .unwrap()
        .try_evaluate_batch(&dag, &[large])
        .unwrap();
    let field = Zp64::new(PRIME_B);
    let expected = Integer::from(i64::MAX - 1).to_finite_field(&field);
    assert_eq!(*large_batch.images()[0].value(), expected);
}

#[test]
fn probe_identity_uses_canonical_residues_and_keeps_integer_provenance() {
    let (_, context) = context([], "modular-probe-identity", 1);
    let dag = ModularCoefficientDag::try_new(&context, Default::default()).unwrap();
    let first =
        ModularProbe::try_new(&dag, &context, 10, PRIME_A, &[2], Default::default()).unwrap();
    let second = ModularProbe::try_new(
        &dag,
        &context,
        11,
        PRIME_A,
        &[i64::try_from(PRIME_A).unwrap() + 2],
        Default::default(),
    )
    .unwrap();
    assert_ne!(first.identity().point(), second.identity().point());
    assert_eq!(first.identity().residues(), second.identity().residues());
    assert!(first.identity().residue_equivalent(second.identity()));
    assert_ne!(first.identity().ordinal(), second.identity().ordinal());
}

#[test]
fn tight_limits_fail_transactionally_and_poison_only_the_exhausted_probe() {
    let (_, context) = context([], "modular-guide-limits", 1);
    let coefficient = Arc::new(context.index(0).unwrap());
    let defaults = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(
        &context,
        ModularGuideLimits {
            max_nodes: 2,
            ..defaults
        },
    )
    .unwrap();
    assert_eq!(
        dag.try_exact_leaf(&context, Arc::clone(&coefficient)),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular coefficient DAG nodes",
            requested: 3,
            limit: 2,
        })
    );
    assert_eq!(dag.node_count(), 2);
    assert_eq!(dag.exact_leaf_count(), 0);

    let mut dag = ModularCoefficientDag::try_new(
        &context,
        ModularGuideLimits {
            max_physical_deltas: 1,
            ..defaults
        },
    )
    .unwrap();
    let n = dag.try_exact_leaf(&context, coefficient).unwrap();
    assert_eq!(
        dag.try_translate_physical(&n, &[1]),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular coefficient physical deltas",
            requested: 2,
            limit: 1,
        })
    );
    assert_eq!(dag.physical_delta_count(), 1);

    let mut compound = ModularCoefficientDag::try_new(
        &context,
        ModularGuideLimits {
            max_nodes: 5,
            ..defaults
        },
    )
    .unwrap();
    let x = leaf(&mut compound, &context, context.index(0).unwrap());
    let y = leaf(&mut compound, &context, context.integer(2));
    assert_eq!(compound.node_count(), 4);
    assert_eq!(
        compound.try_sub(&x, &y),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular coefficient DAG nodes",
            requested: 6,
            limit: 5,
        })
    );
    assert_eq!(compound.node_count(), 4);
    assert_eq!(
        compound.try_div(&x, &y),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular coefficient DAG nodes",
            requested: 6,
            limit: 5,
        })
    );
    assert_eq!(compound.node_count(), 4);

    let probe = ModularProbe::try_new(
        &dag,
        &context,
        0,
        PRIME_A,
        &[2],
        ModularGuideLimits {
            max_probe_cached_values: 0,
            ..defaults
        },
    )
    .unwrap();
    assert_eq!(
        probe.try_evaluate_batch(&dag, std::slice::from_ref(&n)),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular guide cached values",
            requested: 1,
            limit: 0,
        })
    );

    let term_capped = ModularProbe::try_new(
        &dag,
        &context,
        1,
        PRIME_A,
        &[2],
        ModularGuideLimits {
            max_probe_exact_leaf_terms_evaluated: 1,
            ..defaults
        },
    )
    .unwrap();
    assert_eq!(
        term_capped.try_evaluate_batch(&dag, std::slice::from_ref(&n)),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular guide exact-leaf terms evaluated",
            requested: 2,
            limit: 1,
        })
    );

    let query_capped = ModularProbe::try_new(
        &dag,
        &context,
        2,
        PRIME_A,
        &[2],
        ModularGuideLimits {
            max_probe_queries: 1,
            ..defaults
        },
    )
    .unwrap();
    assert_eq!(
        query_capped.try_evaluate_batch(&dag, &[n.clone(), n]),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular guide probe queries",
            requested: 2,
            limit: 1,
        })
    );

    assert_eq!(
        ModularProbe::try_new(&dag, &context, 3, 21, &[2], defaults).unwrap_err(),
        ModularGuideError::UnsupportedModulus { modulus: 21 }
    );

    let no_batch = ModularProbe::try_new(
        &dag,
        &context,
        4,
        PRIME_A,
        &[2],
        ModularGuideLimits {
            max_probe_batch_images: 0,
            ..defaults
        },
    )
    .unwrap();
    assert_eq!(
        no_batch.try_evaluate_batch(&dag, &[dag.one()]),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular guide completed batch images",
            requested: 1,
            limit: 0,
        })
    );
}
