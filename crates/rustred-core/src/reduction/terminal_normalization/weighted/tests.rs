use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;
use crate::persistence::{
    BinaryProgramKind, BinarySection, SectionTag, encode_program, inspect_program,
};
use symbolica::domains::SelfRing;

fn test_family(momenta: &[Vec<i64>], mass: i64, shift: bool) -> IntegralFamily {
    let loops = momenta[0].len();
    let context = CoefficientContext::new(["d"]);
    let denominators = momenta
        .iter()
        .map(|q| {
            AffineDenominator::new(
                context.integer(-mass),
                (0..loops)
                    .flat_map(|i| (i..loops).map(move |j| q[i] * q[j] * if i == j { 1 } else { 2 }))
                    .map(|n| context.integer(n))
                    .collect(),
            )
        })
        .collect();
    IntegralFamily::new(
        "generic_terminal_projection",
        (0..loops).map(|i| format!("k{i}")).collect(),
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        vec![],
        vec![context.integer(i64::from(shift)); momenta.len()],
    )
    .unwrap()
}

fn circuit_momenta(loops: usize) -> Vec<Vec<i64>> {
    let mut rows: Vec<_> = (0..loops)
        .map(|i| (0..loops).map(|j| i64::from(i == j)).collect())
        .collect();
    rows.push(vec![1; loops]);
    for i in 0..loops {
        for j in i + 1..loops {
            if (i, j) != (0, 1) {
                let mut q = vec![0; loops];
                q[i] = 1;
                q[j] = 1;
                rows.push(q);
            }
        }
    }
    rows
}
fn keys(
    family: &IntegralFamily,
    active: &[usize],
    numerator: usize,
    omit_pinch: Option<usize>,
) -> (BTreeSet<IntegralKey>, IntegralKey) {
    let mut scalar = vec![0; family.denominator_count()];
    for &i in active {
        scalar[i] = 1;
    }
    let mut target = scalar.clone();
    target[numerator] = -1;
    let target = IntegralKey::try_new(target).unwrap();
    let mut raw = BTreeSet::from([
        target.clone(),
        IntegralKey::try_new(scalar.clone()).unwrap(),
    ]);
    for &i in active {
        if omit_pinch == Some(i) {
            continue;
        }
        let mut p = scalar.clone();
        p[i] = 0;
        raw.insert(IntegralKey::try_new(p).unwrap());
    }
    (raw, target)
}
fn prepare(family: &IntegralFamily, raw: &BTreeSet<IntegralKey>) -> TerminalNormalizationPlan {
    TerminalNormalizationPlan::vacuum_quadratic_numerators(
        family,
        raw,
        OrderingPolicy::SpiredUncutV1,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn generic_three_and_five_loop_projections_are_flat_and_have_inspectable_native_witnesses() {
    for loops in [3usize, 5] {
        let family = test_family(&circuit_momenta(loops), 1, false);
        let (raw, target) = keys(&family, &(0..=loops).collect::<Vec<_>>(), loops + 1, None);
        // The complete L=5, arity-15 fixture exceeds the U factory's default
        // 20,000 prospective-product-term cap (its first rejected product has
        // 25,200 terms). Give this positive genericity fixture explicit larger
        // finite Symanzik budgets; production defaults remain untouched.
        let limits = TerminalNormalizationLimits {
            parametric: VacuumParametricLimits {
                symanzik: Default::default(),
                ..Default::default()
            },
            ..Default::default()
        };
        let plan = TerminalNormalizationPlan::vacuum_quadratic_numerators(
            &family,
            &raw,
            OrderingPolicy::SpiredUncutV1,
            limits,
        )
        .unwrap();
        assert_eq!(plan.raw_terminals(), &raw);
        assert_eq!(plan.statistics.projected_numerators, 1);
        assert_eq!(plan.canonical_terminals().len(), 2);
        let row = &plan.terms[&target];
        assert_eq!(row.len(), 2);
        let ctx = family.coefficient_context();
        for (key, value) in row {
            let expected = if key.powers().iter().sum::<i64>() == loops as i64 {
                ctx.try_div(
                    &ctx.integer(2 * (loops as i64 - 1)),
                    &ctx.integer(loops as i64),
                    Default::default(),
                )
                .unwrap()
            } else {
                ctx.try_div(
                    &ctx.integer(loops as i64 - 2),
                    &ctx.integer(loops as i64),
                    Default::default(),
                )
                .unwrap()
            };
            assert!(crate::persistence::same_native_coefficient(
                value, &expected
            ));
            assert!(raw.contains(key));
            assert_eq!(plan.terms[key], BTreeMap::from([(key.clone(), ctx.one())]));
        }
        let witness = &plan.projection_witnesses()[&target];
        assert_eq!(witness.support().generators().len(), loops);
        assert_eq!(
            witness.support().affine_columns().len(),
            witness.native_combination().len()
        );
        for map in witness.support().generators() {
            assert!(matches!(
                map.jacobian(),
                crate::sector::symmetry::Jacobian::Unit { .. }
            ));
        }
        let bytes = plan.encode_native(Default::default()).unwrap();
        let cold = TerminalNormalizationPlan::decode_generated(
            &bytes,
            &family,
            &raw,
            plan.ordering(),
            limits,
            Default::default(),
        )
        .unwrap();
        assert_eq!(cold.terms(), plan.terms());
        assert_eq!(cold.statistics(), plan.statistics());
    }
}

#[test]
fn multiple_numerators_remain_distinct_until_their_verified_weighted_projection() {
    let family = test_family(&circuit_momenta(3), 1, false);
    let (mut raw, first) = keys(&family, &[0, 1, 2, 3], 4, None);
    let second = IntegralKey::try_new(vec![1, 1, 1, 1, 0, -1]).unwrap();
    raw.insert(second.clone());
    let aliases = TerminalAliasPlan::vacuum_parametric_equivalences(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
        Default::default(),
    )
    .unwrap();
    assert_ne!(
        aliases.representative(&family, &first).unwrap(),
        aliases.representative(&family, &second).unwrap()
    );
    let plan = prepare(&family, &raw);
    assert_eq!(plan.statistics.projected_numerators, 2);
    assert_eq!(plan.terms[&first], plan.terms[&second]);
    assert_eq!(plan.terms[&first].len(), 2);
    for row in plan.terms.values() {
        for output in row.keys() {
            assert_eq!(
                plan.terms[output],
                BTreeMap::from([(output.clone(), family.coefficient_context().one())])
            );
        }
    }
}

#[test]
fn coloop_flip_removes_an_undeclared_scaleless_pinch_with_an_exact_zero_witness() {
    let family = test_family(
        &[
            [1, 0, 0],
            [0, 1, 0],
            [1, 1, 0],
            [0, 0, 1],
            [1, 0, 1],
            [0, 1, 1],
        ]
        .map(Vec::from),
        1,
        false,
    );
    let (raw, target) = keys(&family, &[0, 1, 2, 3], 4, Some(3));
    let plan = prepare(&family, &raw);
    assert_eq!(plan.statistics.projected_numerators, 1);
    assert_eq!(plan.terms[&target].len(), 2);
    assert!(plan.terms[&target].values().all(|c| c.is_one()));
    assert!(
        plan.zero_certificates()
            .keys()
            .any(|k| k.powers() == [1, 1, 1, 0, 0, 0])
    );
    assert!(plan.zero_certificates().keys().all(|k| !raw.contains(k)));
    assert_eq!(
        plan.projection_witnesses()[&target]
            .support()
            .generators()
            .len(),
        3
    );
}

#[test]
fn unsupported_masses_shifts_powers_and_circuits_are_retained_not_assumed_zero() {
    let momenta = circuit_momenta(3);
    for family in [
        test_family(&momenta, 2, false),
        test_family(&momenta, 1, true),
    ] {
        let (raw, target) = keys(&family, &[0, 1, 2, 3], 4, None);
        let plan = prepare(&family, &raw);
        assert_eq!(plan.statistics.projected_numerators, 0);
        assert!(plan.canonical_terminals().contains(&target));
        assert!(!plan.statistics.skipped.is_empty());
    }
    let mut nonunit = momenta.clone();
    nonunit[3][2] = 2;
    let family = test_family(&nonunit, 1, false);
    let (raw, target) = keys(&family, &[0, 1, 2, 3], 4, None);
    let plan = prepare(&family, &raw);
    assert_eq!(
        plan.statistics.skipped[&TerminalNormalizationSkipReason::NonUnitCircuit],
        1
    );
    assert!(plan.canonical_terminals().contains(&target));
    let family = test_family(&momenta, 1, false);
    let (mut raw, target) = keys(&family, &[0, 1, 2, 3], 4, None);
    raw.remove(&target);
    let mut powers = target.powers().to_vec();
    powers[4] = -2;
    let target = IntegralKey::try_new(powers).unwrap();
    raw.insert(target.clone());
    let plan = prepare(&family, &raw);
    assert_eq!(
        plan.statistics.skipped[&TerminalNormalizationSkipReason::NumeratorShape],
        1
    );
    assert!(plan.canonical_terminals().contains(&target));
}

#[test]
fn undeclared_positive_outputs_and_resource_exhaustion_fail_closed() {
    let family = test_family(&circuit_momenta(3), 1, false);
    let (_, target) = keys(&family, &[0, 1, 2, 3], 4, None);
    let raw = BTreeSet::from([target.clone()]);
    let plan = prepare(&family, &raw);
    assert_eq!(
        plan.statistics.skipped[&TerminalNormalizationSkipReason::UnboundPositiveOutput],
        1
    );
    assert_eq!(plan.canonical_terminals(), &raw);
    assert!(matches!(
        TerminalNormalizationPlan::vacuum_quadratic_numerators(
            &family,
            &raw,
            plan.ordering(),
            TerminalNormalizationLimits {
                max_matrix_cells: 1,
                ..Default::default()
            }
        ),
        Err(TerminalNormalizationError::Limit { .. })
    ));
}

#[test]
fn valid_positive_binding_into_a_harder_sector_is_retained_with_a_typed_skip() {
    // Two unit four-line circuits in the same complete three-loop family.
    // The scalar of the target's support is deliberately not declared; its
    // equivalent declared scalar has a harder sector in the saved ordering.
    let family = test_family(
        &[
            [1, 0, 0],
            [0, 1, 0],
            [1, 0, 1],
            [1, 1, 1],
            [0, 0, 1],
            [0, 1, 1],
        ]
        .map(Vec::from),
        1,
        false,
    );
    let (mut raw, target) = keys(&family, &[0, 1, 3, 4], 2, None);
    let missing = IntegralKey::try_new(vec![1, 1, 0, 1, 1, 0]).unwrap();
    assert!(raw.remove(&missing));
    let harder = IntegralKey::try_new(vec![1, 1, 1, 0, 0, 1]).unwrap();
    raw.insert(harder.clone());
    assert!(
        OrderingPolicy::SpiredUncutV1
            .compare(harder.powers(), target.powers())
            .unwrap()
            .is_gt()
    );
    let plan = prepare(&family, &raw);
    assert_eq!(plan.statistics.projected_numerators, 0);
    assert_eq!(
        plan.statistics.skipped[&TerminalNormalizationSkipReason::NonDescendingOutput],
        1
    );
    assert_eq!(
        plan.terms[&target],
        BTreeMap::from([(target.clone(), family.coefficient_context().one())])
    );
    // The skip is availability, not failure to prove the positive equality.
    assert_eq!(
        plan.positive_aliases()
            .representative(&family, &missing)
            .unwrap(),
        plan.positive_aliases()
            .representative(&family, &harder)
            .unwrap()
    );
}

#[test]
fn external_momenta_two_loop_no_numerator_and_empty_sets_remain_explicit() {
    let context = CoefficientContext::new(["d"]);
    let external = IntegralFamily::new(
        "external_retained",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(context.integer(-1), vec![context.one(), context.zero()]),
            AffineDenominator::new(context.integer(-1), vec![context.one(), context.integer(2)]),
        ],
        vec![vec![context.zero()]],
        vec![context.zero(); 2],
    )
    .unwrap();
    let raw = BTreeSet::from([IntegralKey::try_new([-1, 1]).unwrap()]);
    let plan = prepare(&external, &raw);
    assert_eq!(plan.canonical_terminals(), &raw);
    assert_eq!(
        plan.statistics.skipped[&TerminalNormalizationSkipReason::UnsupportedGeometry(
            ProductSkipReason::ExternalMomenta
        )],
        1
    );
    let family = test_family(&circuit_momenta(2), 1, false);
    let raw = BTreeSet::from([IntegralKey::try_new([1, 1, 1]).unwrap()]);
    let plan = prepare(&family, &raw);
    assert_eq!(plan.statistics.projected_numerators, 0);
    assert_eq!(plan.canonical_terminals(), &raw);
    let raw = BTreeSet::new();
    let plan = prepare(&family, &raw);
    let encoded = plan.encode_native(Default::default()).unwrap();
    let restored = TerminalNormalizationPlan::decode_generated(
        &encoded,
        &family,
        &raw,
        plan.ordering(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert!(restored.terms().is_empty());
}

#[test]
fn native_sidecar_rejects_wrong_owners_kinds_bytes_values_and_constant_contexts() {
    let family = test_family(&circuit_momenta(3), 1, false);
    let (raw, target) = keys(&family, &[0, 1, 2, 3], 4, None);
    let plan = prepare(&family, &raw);
    let bytes = plan.encode_native(Default::default()).unwrap();
    let load = |bytes: &[u8]| {
        TerminalNormalizationPlan::decode_generated(
            bytes,
            &family,
            &raw,
            plan.ordering(),
            Default::default(),
            Default::default(),
        )
    };
    assert!(load(&bytes[..bytes.len() - 1]).is_err());
    assert!(
        TerminalNormalizationPlan::decode_generated(
            &bytes,
            &family,
            &raw,
            OrderingPolicy::RustRedUnshiftedV1,
            Default::default(),
            Default::default()
        )
        .is_err()
    );
    let mut subset = raw.clone();
    subset.pop_first();
    assert!(
        TerminalNormalizationPlan::decode_generated(
            &bytes,
            &family,
            &subset,
            plan.ordering(),
            Default::default(),
            Default::default()
        )
        .is_err()
    );
    let other = test_family(&circuit_momenta(3), 2, false);
    assert!(
        TerminalNormalizationPlan::decode_generated(
            &bytes,
            &other,
            &raw,
            plan.ordering(),
            Default::default(),
            Default::default()
        )
        .is_err()
    );
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    for kind in [
        BinaryProgramKind::Candidates,
        BinaryProgramKind::Certified,
        BinaryProgramKind::TerminalValues,
    ] {
        let wrong = encode_program(kind, envelope.sections(), Default::default()).unwrap();
        assert!(load(&wrong).is_err());
    }
    let mut corrupt = plan.clone();
    let first = corrupt
        .terms
        .get_mut(&target)
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    *first = family.coefficient_context().integer(7);
    assert!(load(&corrupt.encode_native(Default::default()).unwrap()).is_err());
    let mut foreign = plan.clone();
    let other_context = CoefficientContext::new(["foreign_d"]);
    *foreign
        .terms
        .get_mut(&target)
        .unwrap()
        .values_mut()
        .next()
        .unwrap() = other_context.one();
    assert!(load(&foreign.encode_native(Default::default()).unwrap()).is_err());
    let mut payload = envelope.section(SectionTag::PROGRAM).unwrap().to_vec();
    payload[0] = 99;
    let sections: Vec<_> = envelope
        .sections()
        .iter()
        .map(|s| BinarySection {
            tag: s.tag,
            bytes: if s.tag == SectionTag::PROGRAM {
                &payload
            } else {
                s.bytes
            },
        })
        .collect();
    assert!(
        load(
            &encode_program(
                BinaryProgramKind::TerminalNormalization,
                &sections,
                Default::default()
            )
            .unwrap()
        )
        .is_err()
    );
}

fn candidate(
    family: &IntegralFamily,
    raw: &BTreeSet<IntegralKey>,
) -> crate::solver::CandidateReducer<6> {
    use crate::solver::{Integral, SectorSolution};
    let mut sectors: BTreeMap<[bool; 6], Vec<Integral<6>>> = BTreeMap::new();
    for key in raw {
        let mask = std::array::from_fn(|i| key.powers()[i] > 0);
        let powers = std::array::from_fn(|i| i16::try_from(key.powers()[i]).unwrap());
        sectors
            .entry(mask)
            .or_default()
            .push(Integral::numeric(powers).unwrap());
    }
    crate::solver::CandidateReducer::try_new(
        family,
        [true; 6],
        OrderingPolicy::SpiredUncutV1,
        sectors.into_iter().map(|(mask, finite_residuals)| {
            (
                mask,
                SectorSolution {
                    rules: vec![],
                    finite_residuals,
                    stats: Default::default(),
                },
            )
        }),
        vec![],
        Default::default(),
    )
    .unwrap()
}

#[test]
fn weighted_application_keeps_original_mass_target_and_memoizes_both_cache_modes() {
    let family = test_family(&circuit_momenta(3), 1, false);
    let (raw, target) = keys(&family, &[0, 1, 2, 3], 4, None);
    let plan = prepare(&family, &raw);
    for representation in [
        crate::solver::CandidateCacheRepresentation::Sparse,
        crate::solver::CandidateCacheRepresentation::Factorized,
    ] {
        let mut reducer = candidate(&family, &raw);
        assert!(reducer.terminal_normalization().is_none());
        reducer.set_cache_representation(representation).unwrap();
        reducer
            .install_terminal_normalization(plan.clone())
            .unwrap();
        let result = reducer.reduce_unit_mass(&target).unwrap();
        assert_eq!(result.target(), &target);
        assert_eq!(result.terms(), &plan.terms[&target]);
        assert_eq!(reducer.terminals(), &raw);
        for output in result.terms().keys() {
            assert!(
                plan.ordering()
                    .compare(output.powers(), target.powers())
                    .unwrap()
                    .is_lt()
            );
            assert_eq!(
                result.common_mass_squared_power(output).unwrap(),
                i128::from(
                    output.powers().iter().sum::<i64>() - target.powers().iter().sum::<i64>()
                )
            );
        }
        let statistics = reducer.statistics();
        assert_eq!(reducer.reduce_unit_mass(&target).unwrap(), result);
        assert_eq!(
            reducer.statistics().cache_hits(),
            statistics.cache_hits() + 1
        );
        assert_eq!(reducer.statistics().rule_applications(), 0);
        assert!(
            reducer
                .install_terminal_normalization(plan.clone())
                .is_err()
        );
    }
}

#[test]
fn replacing_unit_and_weighted_plans_is_explicit_empty_cache_only_and_atomic() {
    let family = test_family(&circuit_momenta(3), 1, false);
    let (raw, target) = keys(&family, &[0, 1, 2, 3], 4, None);
    let plan = prepare(&family, &raw);
    let aliases = TerminalAliasPlan::vacuum_parametric_equivalences(
        &family,
        &raw,
        plan.ordering(),
        Default::default(),
    )
    .unwrap();
    let mut reducer = candidate(&family, &raw);
    reducer.install_terminal_aliases(aliases.clone()).unwrap();
    reducer
        .install_terminal_normalization(plan.clone())
        .unwrap();
    assert!(reducer.terminal_aliases().is_none());
    assert!(reducer.terminal_normalization().is_some());
    let mut wrong_order = plan.clone();
    wrong_order.ordering = OrderingPolicy::RustRedUnshiftedV1;
    assert!(reducer.install_terminal_normalization(wrong_order).is_err());
    assert_eq!(reducer.canonical_terminals(), plan.canonical_terminals());
    reducer.reduce_unit_mass(&target).unwrap();
    assert!(reducer.install_terminal_aliases(aliases.clone()).is_err());
    assert!(reducer.terminal_normalization().is_some());
    reducer.clear_cache().unwrap();
    reducer.install_terminal_aliases(aliases).unwrap();
    assert!(reducer.terminal_normalization().is_none());
    assert!(reducer.terminal_aliases().is_some());
    let plain = reducer.reduce_unit_mass(&target).unwrap();
    assert_eq!(
        plain.terms(),
        &BTreeMap::from([(target.clone(), family.coefficient_context().one())])
    );
    reducer.clear_cache().unwrap();
    reducer.install_terminal_normalization(plan).unwrap();
    assert_eq!(reducer.reduce_unit_mass(&target).unwrap().terms().len(), 2);
}
