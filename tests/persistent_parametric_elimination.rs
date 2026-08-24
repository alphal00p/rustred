use rustred::{
    AffineDenominator, CoefficientContext, IndexSpace, IntegralFamily, IntegralOrderingPolicy,
    PARAMETRIC_RELATION_MANIFEST_V2_SCHEMA, ParametricElimination, ParametricEliminationOrdering,
    ParametricIbpGenerator, ParametricRelation, ParametricRelationError, ParametricRowId,
    PersistentParametricEliminationDatabase, PersistentParametricEliminationError,
    PersistentParametricEliminationLimits, PersistentParametricEliminationRowOutcome,
};

fn massive_tadpole() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "persistent-elimination-tadpole",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parameter("m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn pivots_agree(left: &ParametricElimination, right: &ParametricElimination) -> bool {
    left.pivots().len() == right.pivots().len()
        && left
            .pivots()
            .iter()
            .zip(right.pivots())
            .all(|(left, right)| {
                left.ordinal() == right.ordinal()
                    && left.pivot() == right.pivot()
                    && left.trace() == right.trace()
                    && left
                        .unit_relation()
                        .has_identical_guard_provenance(right.unit_relation())
            })
}

#[test]
fn generated_tadpole_rows_obey_submit_cursor_replay_and_clean_boundaries() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let canonical = generated.ibp_li().next().unwrap().clone();
    let ordering =
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [2])
            .unwrap();
    let limits = PersistentParametricEliminationLimits::default();
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ordering.clone(),
        limits,
    )
    .unwrap();

    // A duplicate generated identity is deliberately retained. It is a real
    // source event and must reduce to a dependent row rather than disappear by
    // implicit set-union semantics.
    database
        .submit_prevalidated_rows("depth=0", vec![canonical.clone(), canonical.clone()])
        .unwrap();
    let first = database.solve_until(|_| true).unwrap().unwrap();
    assert!(matches!(
        first.outcome(),
        PersistentParametricEliminationRowOutcome::Pivot { pivot_ordinal: 0 }
    ));
    assert_eq!(database.pending_rows(), 1);

    let translated = canonical
        .translated(
            generated.context(),
            &IndexSpace::try_new(1).unwrap().shift([1]).unwrap(),
            ParametricRowId::Derived {
                label: "tadpole-depth-one".into(),
            },
            limits.elimination.arithmetic,
        )
        .unwrap();
    let error = database
        .submit_prevalidated_rows("depth=1-too-early", vec![translated.clone()])
        .unwrap_err();
    assert!(error.to_string().contains("unconsumed pending rows"));

    let dependent = database.consume_next().unwrap().unwrap();
    assert_eq!(
        dependent.outcome(),
        PersistentParametricEliminationRowOutcome::Dependent
    );
    assert_eq!(database.pending_rows(), 0);

    database
        .submit_prevalidated_rows("depth=1", vec![translated.clone()])
        .unwrap();
    assert!(database.solve_until(|_| false).unwrap().is_none());
    assert_eq!(database.pending_rows(), 0);
    assert_eq!(database.stats().submitted_batches(), 2);
    assert_eq!(database.stats().consumed_rows(), 3);
    assert_eq!(database.events().len(), 3);

    let all_rows = vec![canonical.clone(), canonical, translated];
    let full = ParametricElimination::build(
        generated.context(),
        &all_rows,
        ordering.clone(),
        limits.elimination,
    )
    .unwrap();
    assert!(pivots_agree(database.elimination().unwrap(), &full));

    let certificate = database.finish();
    assert_eq!(certificate.pending_rows(), 0);
    certificate.replay(generated.context()).unwrap();

    // `clean` is represented by a new case-group database: no pivot from the
    // finalized group may leak into it.
    let clean = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ordering,
        limits,
    )
    .unwrap();
    assert!(clean.elimination().is_none());
    assert!(clean.events().is_empty());
    assert_eq!(clean.stats().consumed_rows(), 0);
}

#[test]
fn aggregate_batch_limit_is_preflighted_without_mutation() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let canonical = generated.ibp_li().next().unwrap().clone();
    let mut limits = PersistentParametricEliminationLimits::default();
    limits.max_submitted_batches = 0;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        limits,
    )
    .unwrap();
    let error = database
        .submit_prevalidated_rows("rejected", vec![canonical])
        .unwrap_err();
    assert!(error.to_string().contains("submitted batches"));
    assert!(database.batches().is_empty());
    assert_eq!(database.pending_rows(), 0);
}

#[test]
fn a_selector_stop_retains_and_replays_the_exact_pending_suffix() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let canonical = generated.ibp_li().next().unwrap().clone();
    let translated = canonical
        .translated(
            generated.context(),
            &IndexSpace::try_new(1).unwrap().shift([1]).unwrap(),
            ParametricRowId::Derived {
                label: "pending-depth-one".into(),
            },
            PersistentParametricEliminationLimits::default()
                .elimination
                .arithmetic,
        )
        .unwrap();
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [2])
            .unwrap(),
        PersistentParametricEliminationLimits::default(),
    )
    .unwrap();
    database
        .submit_prevalidated_rows("one-layer", vec![canonical, translated])
        .unwrap();
    let selected = database.solve_until(|_| true).unwrap().unwrap();
    assert_eq!(selected.source_ordinal(), 0);
    assert_eq!(database.pending_rows(), 1);

    let certificate = database.finish();
    assert_eq!(certificate.events().len(), 1);
    assert_eq!(certificate.pending_rows(), 1);
    certificate.replay(generated.context()).unwrap();
}

#[test]
fn exhausted_cumulative_work_poisoning_is_typed_and_fail_closed() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let canonical = generated.ibp_li().next().unwrap().clone();
    let mut limits = PersistentParametricEliminationLimits::default();
    limits.max_cumulative_construction_reductions = 0;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        limits,
    )
    .unwrap();
    database
        .submit_prevalidated_rows("duplicate", vec![canonical.clone(), canonical])
        .unwrap();
    database.consume_next().unwrap().unwrap();
    let error = database.consume_next().unwrap_err();
    assert!(error.to_string().contains("parametric reductions"));
    assert_eq!(database.stats().consumed_rows(), 1);
    assert!(
        database
            .consume_next()
            .unwrap_err()
            .to_string()
            .contains("interrupted")
    );
    let certificate = database.finish();
    assert!(certificate.interrupted());
    assert!(certificate.replay(generated.context()).is_err());
}

#[test]
fn bounded_relation_manifest_is_exact_and_rejects_one_byte_below() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let source = generated.ibp_li().next().unwrap().clone();
    let elimination = ParametricElimination::build(
        generated.context(),
        &[source],
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        PersistentParametricEliminationLimits::default().elimination,
    )
    .unwrap();
    // A normalized pivot carries divisor guard provenance, so this exercises
    // every field in the typed sparse encoder rather than only a raw source row.
    let relation = elimination.pivots()[0].unit_relation();
    let manifest = relation.stable_manifest();
    assert!(manifest.starts_with(PARAMETRIC_RELATION_MANIFEST_V2_SCHEMA));
    assert_eq!(
        relation.stable_manifest_with_limit(manifest.len()).unwrap(),
        manifest
    );
    let one_below = manifest.len() - 1;
    assert!(matches!(
        relation.stable_manifest_with_limit(one_below),
        Err(ParametricRelationError::ResourceLimit {
            resource: "parametric relation manifest bytes",
            requested,
            limit,
        }) if requested > limit && limit == one_below
    ));

    // Exercise the opposite representation extreme: a constant sparse
    // coefficient must still accept its exact V2 size.
    let mut constant = ParametricRelation::new(
        family.fingerprint(),
        ParametricRowId::Derived {
            label: "constant-manifest".into(),
        },
        generated.context(),
    );
    constant
        .add_term(
            generated.context(),
            IndexSpace::try_new(1).unwrap().zero(),
            generated.context().one(),
        )
        .unwrap();
    let constant_manifest = constant.stable_manifest();
    assert_eq!(
        constant
            .stable_manifest_with_limit(constant_manifest.len())
            .unwrap(),
        constant_manifest
    );
}

#[test]
fn retained_manifest_limit_is_preflighted_without_database_mutation() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let relation = generated.ibp_li().next().unwrap().clone();
    let mut limits = PersistentParametricEliminationLimits::default();
    limits.max_retained_source_manifest_bytes = relation.stable_manifest().len() - 1;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        limits,
    )
    .unwrap();

    assert!(matches!(
        database.submit_prevalidated_rows("bounded", vec![relation]),
        Err(PersistentParametricEliminationError::ResourceLimit {
            resource: "persistent retained source manifest bytes",
            requested,
            limit,
        }) if requested > limit && limit == limits.max_retained_source_manifest_bytes
    ));
    assert!(database.batches().is_empty());
    assert!(database.events().is_empty());
    assert_eq!(database.pending_rows(), 0);
    assert_eq!(database.stats().submitted_rows(), 0);
    assert_eq!(database.stats().retained_source_manifest_bytes(), 0);
    assert_eq!(database.stats().retained_batch_label_bytes(), 0);
}

#[test]
fn cumulative_label_bytes_are_preflighted_without_mutation() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let relation = generated.ibp_li().next().unwrap().clone();
    let mut limits = PersistentParametricEliminationLimits::default();
    limits.max_retained_batch_label_bytes = 3;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        limits,
    )
    .unwrap();
    database
        .submit_prevalidated_rows("ab", vec![relation.clone()])
        .unwrap();
    database.consume_next().unwrap().unwrap();
    let before = database.stats();

    assert_eq!(
        database
            .submit_prevalidated_rows("cd", vec![relation])
            .unwrap_err(),
        PersistentParametricEliminationError::ResourceLimit {
            resource: "persistent retained batch label bytes",
            requested: 4,
            limit: 3,
        }
    );
    assert_eq!(database.stats(), before);
    assert_eq!(database.batches().len(), 1);
    assert_eq!(database.events().len(), 1);
    assert_eq!(database.pending_rows(), 0);
}

#[test]
fn replay_clone_bound_is_enforced_at_submit_without_mutation() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let relation = generated.ibp_li().next().unwrap().clone();
    let manifest_bytes = relation.stable_manifest().len();
    let mut limits = PersistentParametricEliminationLimits::default();
    limits.max_replay_batch_clone_manifest_bytes = manifest_bytes - 1;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        limits,
    )
    .unwrap();

    assert_eq!(
        database
            .submit_prevalidated_rows("clone-bounded", vec![relation])
            .unwrap_err(),
        PersistentParametricEliminationError::ResourceLimit {
            resource: "persistent replay batch clone manifest bytes",
            requested: manifest_bytes,
            limit: manifest_bytes - 1,
        }
    );
    assert!(database.batches().is_empty());
    assert_eq!(database.pending_rows(), 0);
    assert_eq!(database.stats().submitted_rows(), 0);
}

#[test]
fn replay_transient_limit_is_preflighted_one_below_without_mutation() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let relation = generated.ibp_li().next().unwrap().clone();
    let transient_manifest_bytes = 2 * relation.stable_manifest().len();
    let mut limits = PersistentParametricEliminationLimits::default();
    limits.max_replay_source_clone_manifest_bytes = transient_manifest_bytes - 1;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        limits,
    )
    .unwrap();

    assert_eq!(
        database
            .submit_prevalidated_rows("transient-bounded", vec![relation])
            .unwrap_err(),
        PersistentParametricEliminationError::ResourceLimit {
            resource: "persistent replay coexisting source clone manifest bytes",
            requested: transient_manifest_bytes,
            limit: transient_manifest_bytes - 1,
        }
    );
    assert!(database.batches().is_empty());
    assert_eq!(database.pending_rows(), 0);
    assert_eq!(database.stats(), Default::default());
}

#[test]
fn elimination_source_manifest_limit_is_preflighted_before_retention() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let relation = generated.ibp_li().next().unwrap().clone();
    let ordering =
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap();
    let source_manifest_bytes = ParametricElimination::build(
        generated.context(),
        std::slice::from_ref(&relation),
        ordering.clone(),
        PersistentParametricEliminationLimits::default().elimination,
    )
    .unwrap()
    .source_manifest()
    .len();
    let mut limits = PersistentParametricEliminationLimits::default();
    limits.elimination.max_source_manifest_bytes = source_manifest_bytes - 1;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ordering,
        limits,
    )
    .unwrap();

    assert_eq!(
        database
            .submit_prevalidated_rows("bounded-source", vec![relation])
            .unwrap_err(),
        PersistentParametricEliminationError::ResourceLimit {
            resource: "persistent retained elimination source manifest bytes",
            requested: source_manifest_bytes,
            limit: source_manifest_bytes - 1,
        }
    );
    assert!(database.batches().is_empty());
    assert_eq!(database.pending_rows(), 0);
    assert_eq!(database.stats(), Default::default());
}

#[test]
fn fixed_elimination_input_limits_are_preflighted_without_mutation() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let relation = generated.ibp_li().next().unwrap().clone();
    for (max_input_terms, max_columns) in
        [(relation.terms().len() - 1, usize::MAX), (usize::MAX, 0)]
    {
        let mut limits = PersistentParametricEliminationLimits::default();
        limits.elimination.max_input_terms = max_input_terms;
        limits.elimination.max_columns = max_columns;
        let mut database = PersistentParametricEliminationDatabase::try_new(
            generated.context(),
            family.fingerprint(),
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap(),
            limits,
        )
        .unwrap();
        assert!(matches!(
            database.submit_prevalidated_rows("fixed-input", vec![relation.clone()]),
            Err(PersistentParametricEliminationError::ResourceLimit { .. })
        ));
        assert!(database.batches().is_empty());
        assert_eq!(database.pending_rows(), 0);
        assert_eq!(database.stats(), Default::default());
    }
}

#[test]
fn zero_rows_cannot_escape_the_cumulative_prefix_rebuild_budget() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let zero = ParametricRelation::new(
        family.fingerprint(),
        ParametricRowId::Derived {
            label: "zero-prefix-work".into(),
        },
        generated.context(),
    );
    let mut limits = PersistentParametricEliminationLimits::default();
    // Prefixes of lengths one and two request three cumulative row-units.
    limits.max_cumulative_prefix_rebuild_rows = 2;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        limits,
    )
    .unwrap();
    database
        .submit_prevalidated_rows("zeros", vec![zero.clone(), zero])
        .unwrap();
    let first = database.consume_next().unwrap().unwrap();
    assert_eq!(
        first.outcome(),
        PersistentParametricEliminationRowOutcome::Dependent
    );
    let before = database.stats();

    assert_eq!(
        database.consume_next().unwrap_err(),
        PersistentParametricEliminationError::ResourceLimit {
            resource: "persistent cumulative prefix rebuild rows",
            requested: 3,
            limit: 2,
        }
    );
    assert_eq!(database.stats(), before);
    assert_eq!(database.events().len(), 1);
    assert_eq!(database.pending_rows(), 1);
    assert_eq!(
        database.consume_next().unwrap_err(),
        PersistentParametricEliminationError::DatabaseInterrupted
    );
}

#[test]
fn exact_prefix_manifest_work_limit_rejects_one_byte_below_without_commit() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let zero = ParametricRelation::new(
        family.fingerprint(),
        ParametricRowId::Derived {
            label: "zero-manifest-work".into(),
        },
        generated.context(),
    );
    let ordering =
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap();
    let elimination_limits = PersistentParametricEliminationLimits::default().elimination;
    let first_manifest_bytes = ParametricElimination::build(
        generated.context(),
        std::slice::from_ref(&zero),
        ordering.clone(),
        elimination_limits,
    )
    .unwrap()
    .source_manifest()
    .len();
    let second_manifest_bytes = ParametricElimination::build(
        generated.context(),
        &[zero.clone(), zero.clone()],
        ordering.clone(),
        elimination_limits,
    )
    .unwrap()
    .source_manifest()
    .len();
    let requested = 2 * (first_manifest_bytes + second_manifest_bytes);
    let mut limits = PersistentParametricEliminationLimits::default();
    limits.max_cumulative_prefix_rebuild_manifest_bytes = requested - 1;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ordering,
        limits,
    )
    .unwrap();
    database
        .submit_prevalidated_rows("zeros", vec![zero.clone(), zero])
        .unwrap();
    database.consume_next().unwrap().unwrap();
    let before = database.stats();

    assert_eq!(
        database.consume_next().unwrap_err(),
        PersistentParametricEliminationError::ResourceLimit {
            resource: "persistent cumulative prefix rebuild manifest bytes",
            requested,
            limit: requested - 1,
        }
    );
    assert_eq!(database.stats(), before);
    assert_eq!(database.events().len(), 1);
    assert_eq!(database.pending_rows(), 1);
}

#[test]
fn coexisting_elimination_manifest_peak_is_checked_before_rebuild() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let relation = generated.ibp_li().next().unwrap().clone();
    let ordering =
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap();
    let source_manifest_bytes = ParametricElimination::build(
        generated.context(),
        std::slice::from_ref(&relation),
        ordering.clone(),
        PersistentParametricEliminationLimits::default().elimination,
    )
    .unwrap()
    .source_manifest()
    .len();
    let row_manifest_bytes = relation.stable_manifest().len();
    let requested = 2 * source_manifest_bytes + row_manifest_bytes;
    let mut limits = PersistentParametricEliminationLimits::default();
    limits.max_coexisting_elimination_source_manifest_bytes = requested - 1;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ordering,
        limits,
    )
    .unwrap();
    database
        .submit_prevalidated_rows("coexisting-manifests", vec![relation])
        .unwrap();
    let before = database.stats();

    assert_eq!(
        database.consume_next().unwrap_err(),
        PersistentParametricEliminationError::ResourceLimit {
            resource: "persistent coexisting elimination source manifest bytes",
            requested,
            limit: requested - 1,
        }
    );
    assert_eq!(database.stats(), before);
    assert!(database.events().is_empty());
    assert_eq!(database.pending_rows(), 1);
    assert_eq!(
        database.consume_next().unwrap_err(),
        PersistentParametricEliminationError::DatabaseInterrupted
    );
}

#[test]
fn certificate_replay_charges_its_outer_elimination_manifest() {
    let family = massive_tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let relation = generated.ibp_li().next().unwrap().clone();
    let ordering =
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap();
    let source_manifest_bytes = ParametricElimination::build(
        generated.context(),
        std::slice::from_ref(&relation),
        ordering.clone(),
        PersistentParametricEliminationLimits::default().elimination,
    )
    .unwrap()
    .source_manifest()
    .len();
    let mut limits = PersistentParametricEliminationLimits::default();
    // Live construction needs two copies (build plus its independent replay),
    // while certificate replay also retains the certified copy.
    let row_manifest_bytes = relation.stable_manifest().len();
    limits.max_coexisting_elimination_source_manifest_bytes =
        2 * source_manifest_bytes + row_manifest_bytes;
    let mut database = PersistentParametricEliminationDatabase::try_new(
        generated.context(),
        family.fingerprint(),
        ordering,
        limits,
    )
    .unwrap();
    database
        .submit_prevalidated_rows("outer-certificate-manifest", vec![relation])
        .unwrap();
    database.consume_next().unwrap().unwrap();
    let certificate = database.finish();

    assert_eq!(
        certificate.replay(generated.context()).unwrap_err(),
        PersistentParametricEliminationError::ResourceLimit {
            resource: "persistent coexisting elimination source manifest bytes",
            requested: 3 * source_manifest_bytes + row_manifest_bytes,
            limit: 2 * source_manifest_bytes + row_manifest_bytes,
        }
    );
}
