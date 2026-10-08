use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;
use crate::reduction::terminal_relations::TerminalRelationSession;

fn family(
    name: &str,
    context: CoefficientContext,
    dimension_offset: i64,
    scale: i64,
    mass: i64,
    shift: i64,
    reverse: bool,
) -> Arc<IntegralFamily> {
    let mut rows = [[scale, 0], [0, scale], [scale, scale]];
    if reverse {
        rows.reverse();
    }
    let dimension = context
        .try_add(
            &context
                .parameter(context.parameter_names().first().unwrap())
                .unwrap(),
            &context.integer(dimension_offset),
            Default::default(),
        )
        .unwrap();
    Arc::new(
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            vec![],
            context.clone(),
            dimension,
            rows.into_iter()
                .map(|q| {
                    AffineDenominator::new(
                        context.integer(-mass),
                        [q[0] * q[0], 2 * q[0] * q[1], q[1] * q[1]]
                            .into_iter()
                            .map(|n| context.integer(n))
                            .collect(),
                    )
                })
                .collect(),
            vec![],
            vec![context.integer(shift), context.zero(), context.zero()],
        )
        .unwrap(),
    )
}
fn standard(name: &str, reverse: bool) -> Arc<IntegralFamily> {
    family(name, CoefficientContext::new(["d"]), 0, 1, 1, 0, reverse)
}
fn key(powers: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}
fn inventory(
    family: Arc<IntegralFamily>,
    powers: &[[i64; 3]],
) -> (Arc<IntegralFamily>, BTreeSet<IntegralKey>) {
    (family, powers.iter().copied().map(key).collect())
}

#[test]
fn cross_family_dotted_aliases_are_direct_deterministic_and_replayed_after_cold_load() {
    let a = inventory(standard("collection-a", false), &[[1, 1, 1], [2, 1, 1]]);
    let b = inventory(standard("collection-b", true), &[[1, 1, 1], [1, 1, 2]]);
    let plan = VacuumFamilyAliasPlan::prepare(&[a.clone(), b.clone()], Default::default()).unwrap();
    assert_eq!(plan.raw_terminals().len(), 4);
    assert_eq!(plan.canonical_terminals().len(), 2);
    assert_eq!(plan.aliases().len(), 2);
    for (source, alias) in plan.aliases() {
        assert!(alias.representative() < source);
        assert!(plan.canonical_terminals().contains(alias.representative()));
        assert_ne!(
            source.family_fingerprint(),
            alias.representative().family_fingerprint()
        );
        let witness = alias.witness();
        for (from, &to) in witness.parameter_permutation().iter().enumerate() {
            assert_eq!(
                source.integral().powers()[witness.source_slots()[from]],
                alias.representative().integral().powers()[witness.representative_slots()[to]]
            );
        }
    }
    let reversed =
        VacuumFamilyAliasPlan::prepare(&[b.clone(), a.clone()], Default::default()).unwrap();
    assert_eq!(plan.statistics(), reversed.statistics());
    assert_eq!(plan.canonical_terminals(), reversed.canonical_terminals());
    for (source, alias) in plan.aliases() {
        assert_eq!(
            alias.representative(),
            reversed.aliases()[source].representative()
        );
        assert_eq!(
            alias.witness().parameter_permutation(),
            reversed.aliases()[source].witness().parameter_permutation()
        );
    }
    let cold: Vec<_> = [a, b]
        .into_iter()
        .map(|(family, raw)| {
            let session =
                TerminalRelationSession::new(family, raw.clone(), 0, Default::default()).unwrap();
            let loaded = TerminalRelationSession::from_native_bytes(
                &session.to_native_bytes(Default::default()).unwrap(),
                Default::default(),
                Default::default(),
            )
            .unwrap();
            (loaded.family_owner().clone(), raw)
        })
        .collect();
    let replayed = VacuumFamilyAliasPlan::prepare(&cold, Default::default()).unwrap();
    assert_eq!(plan.canonical_terminals(), replayed.canonical_terminals());
    for (source, alias) in plan.aliases() {
        assert_eq!(
            alias.representative(),
            replayed.aliases()[source].representative()
        );
    }
}

#[test]
fn same_family_remains_supported_but_duplicate_owners_and_unknown_keys_are_errors() {
    let family = standard("collection-same", false);
    let input = inventory(family.clone(), &[[2, 1, 1], [1, 2, 1], [1, 1, 2]]);
    let plan = VacuumFamilyAliasPlan::prepare(&[input.clone()], Default::default()).unwrap();
    assert_eq!(plan.canonical_terminals().len(), 1);
    for raw in &input.1 {
        assert!(
            plan.canonical_terminals()
                .contains(plan.representative(&family, raw).unwrap())
        );
    }
    assert!(matches!(
        VacuumFamilyAliasPlan::prepare(&[input.clone(), input], Default::default()),
        Err(VacuumFamilyAliasError::DuplicateFamily)
    ));
    assert!(matches!(
        plan.representative(&standard("foreign", false), &key([2, 1, 1])),
        Err(Error::WrongFamily)
    ));
    assert!(matches!(
        plan.representative(&family, &key([9, 1, 1])),
        Err(Error::UndeclaredTerminal)
    ));
    assert!(matches!(
        plan.representative(&family, &IntegralKey::try_new([1, 1]).unwrap()),
        Err(Error::WrongArity { .. })
    ));
}

#[test]
fn full_scale_masses_shifts_and_numerators_cannot_be_erased() {
    let ordinary = inventory(standard("collection-control", false), &[[1, 1, 1]]);
    for (scale, mass, shift, expected_skip) in [
        (2, 1, 0, None),
        (1, 2, 0, Some(Skip::NonUnitMass)),
        (1, 1, 1, Some(Skip::AnalyticPowerShifts)),
    ] {
        let foreign = inventory(
            family(
                "collection-foreign",
                CoefficientContext::new(["d"]),
                0,
                scale,
                mass,
                shift,
                false,
            ),
            &[[1, 1, 1]],
        );
        let plan = VacuumFamilyAliasPlan::prepare(&[ordinary.clone(), foreign], Default::default())
            .unwrap();
        assert!(plan.aliases().is_empty());
        assert_eq!(plan.raw_terminals(), plan.canonical_terminals());
        if let Some(reason) = expected_skip {
            assert_eq!(plan.statistics().skipped[&reason], 1);
        }
    }
    let numerator = inventory(standard("collection-numerator", false), &[[1, 1, -1]]);
    let plan = VacuumFamilyAliasPlan::prepare(&[ordinary, numerator], Default::default()).unwrap();
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.statistics().skipped[&Skip::NumeratorPowers], 1);
}

#[test]
fn dimension_and_exact_context_mismatches_fail_before_alias_admission() {
    let ordinary = inventory(standard("collection-control", false), &[[1, 1, 1]]);
    for (context, offset) in [
        (CoefficientContext::new(["d"]), 1),
        (CoefficientContext::new(["D"]), 0),
        (CoefficientContext::new(["d", "x"]), 0),
    ] {
        let foreign = inventory(
            family("collection-incompatible", context, offset, 1, 1, 0, false),
            &[[1, 1, 1]],
        );
        assert!(matches!(
            VacuumFamilyAliasPlan::prepare(&[ordinary.clone(), foreign], Default::default()),
            Err(VacuumFamilyAliasError::IncompatibleFamilies)
        ));
    }
    let bad = (
        standard("collection-bad-arity", false),
        BTreeSet::from([IntegralKey::try_new([1, 1]).unwrap()]),
    );
    assert!(matches!(
        VacuumFamilyAliasPlan::prepare(&[bad], Default::default()),
        Err(VacuumFamilyAliasError::Alias(Error::WrongArity { .. }))
    ));
}

#[test]
fn resource_limits_are_collection_wide_and_skip_without_false_equalities() {
    let inputs = [
        inventory(standard("collection-a", false), &[[1, 1, 1]]),
        inventory(standard("collection-b", true), &[[1, 1, 1]]),
    ];
    for limits in [
        VacuumFamilyAliasLimits {
            max_families: 1,
            ..Default::default()
        },
        VacuumFamilyAliasLimits {
            max_terminals: 1,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            VacuumFamilyAliasPlan::prepare(&inputs, limits),
            Err(VacuumFamilyAliasError::ResourceLimit {
                requested: 2,
                limit: 1,
                ..
            })
        ));
    }
    for limit_kind in 0..4 {
        let mut limits = VacuumFamilyAliasLimits::default();
        match limit_kind {
            0 => limits.parametric.max_supports = 1,
            1 => limits.parametric.max_canonicalizations = 1,
            2 => limits.parametric.max_graph_edges = 0,
            3 => limits.parametric.symanzik.max_polynomial_terms = 0,
            _ => unreachable!(),
        }
        let plan = VacuumFamilyAliasPlan::prepare(&inputs, limits).unwrap();
        assert!(plan.aliases().is_empty());
        assert_eq!(plan.raw_terminals(), plan.canonical_terminals());
        assert!(plan.statistics().skipped[&Skip::ParametricPreparationLimit] > 0);
    }
    let empty = VacuumFamilyAliasPlan::prepare(&[], Default::default()).unwrap();
    assert!(empty.canonical_terminals().is_empty());
}
