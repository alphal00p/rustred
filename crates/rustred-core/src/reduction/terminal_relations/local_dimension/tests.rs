use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn tadpole(constant: i64, analytic: bool) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d", "nu"]);
    Arc::new(
        IntegralFamily::new(
            "local-pair-tadpole",
            vec!["k".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(constant),
                vec![context.one()],
            )],
            vec![],
            vec![if analytic {
                context.parameter("nu").unwrap()
            } else {
                context.zero()
            }],
        )
        .unwrap(),
    )
}

fn embedded_bubble(reverse: bool) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "local-pair-embedded-bubble",
            vec!["k".into(), "q".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    context.integer(-2),
                    vec![context.one(), context.zero(), context.zero()],
                ),
                AffineDenominator::new(
                    context.integer(-3),
                    vec![
                        context.one(),
                        context.integer(if reverse { 2 } else { -2 }),
                        context.one(),
                    ],
                ),
                AffineDenominator::new(
                    context.integer(-5),
                    vec![context.zero(), context.zero(), context.one()],
                ),
            ],
            vec![],
            vec![context.zero(); 3],
        )
        .unwrap(),
    )
}

fn plan(family: Arc<IntegralFamily>, keys: &[IntegralKey]) -> LocalDimensionRecurrencePlan {
    LocalDimensionRecurrencePlan::prepare(
        family,
        &keys.iter().cloned().collect(),
        Default::default(),
        &Default::default(),
    )
    .unwrap()
}

#[test]
fn local_tadpole_pair_has_exact_mass_sign_power_and_symbolic_dimension() {
    for constant in [-3, 3] {
        let family = tadpole(constant, false);
        let context = family.coefficient_context();
        let source = key(&[2]);
        let prepared = plan(family.clone(), &[source.clone()]);
        assert_eq!(prepared.equations_for(&source).len(), 1);
        let row = &prepared.equations_for(&source)[0];
        let expected = context
            .try_sub(
                &context.integer(4),
                family.dimension(),
                ExactAlgebraLimits::unbounded_resources(),
            )
            .unwrap();
        assert_eq!(
            row.terms,
            BTreeMap::from([
                (source.clone(), expected),
                (key(&[3]), context.integer(-4 * constant))
            ])
        );
        assert!(
            row.nonzero_conditions
                .iter()
                .all(|condition| !condition.is_zero())
        );
        assert_eq!(prepared.witnesses[0].contributions.len(), 1);
        assert_eq!(prepared.witnesses[0].contributions[0].seed, source);
    }
}

#[test]
fn analytic_shifts_and_zero_signed_slots_use_effective_exponents() {
    let family = tadpole(-2, true);
    let source = key(&[0]);
    let prepared = plan(family.clone(), &[source.clone()]);
    let context = family.coefficient_context();
    let limits = ExactAlgebraLimits::unbounded_resources();
    let nu = context.parameter("nu").unwrap();
    let leading = context
        .try_sub(
            &context.try_mul(&context.integer(2), &nu, limits).unwrap(),
            family.dimension(),
            limits,
        )
        .unwrap();
    let raised = context.try_mul(&context.integer(4), &nu, limits).unwrap();
    assert_eq!(
        prepared.equations_for(&source)[0].terms,
        BTreeMap::from([(source, leading), (key(&[1]), raised)])
    );
}

#[test]
fn embedded_unequal_mass_dotted_bubble_keeps_spectator_polynomial_shifts() {
    let family = embedded_bubble(false);
    let source = key(&[2, 3, 1]);
    let prepared = plan(family.clone(), &[source.clone()]);
    let direction = vec![
        family.coefficient_context().one(),
        family.coefficient_context().zero(),
    ];
    let witness = prepared
        .witnesses
        .iter()
        .find(|witness| witness.direction == direction)
        .unwrap();
    assert_eq!(witness.spectator_rank, 1);
    // q² = D3+5 is a parent integration variable, not a coefficient to pull
    // through an index-raising operator. Its monomials shift source slot 3.
    assert!(
        witness
            .contributions
            .iter()
            .any(|request| request.seed.powers()[2] != source.powers()[2])
    );
    assert!(
        witness
            .equation
            .terms
            .keys()
            .any(|output| output.powers()[2] != source.powers()[2])
    );
    assert!(!witness.equation.terms.is_empty());
    assert!(prepared.input_keys().is_subset(prepared.inventory()));
}

#[test]
fn reversing_embedded_bubble_spectator_preserves_fixed_d_equations() {
    let source = key(&[1, 2, 1]);
    let forward = plan(embedded_bubble(false), &[source.clone()]);
    let reversed = plan(embedded_bubble(true), &[source]);
    assert_eq!(
        forward.equations.keys().collect::<Vec<_>>(),
        reversed.equations.keys().collect::<Vec<_>>()
    );
    for (source, rows) in &forward.equations {
        let other = &reversed.equations[source];
        assert_eq!(rows.len(), other.len());
        // Direction sorting is deterministic for one family, not invariant
        // under a reflected loop basis. Compare the exact row multiset with
        // a bijection, retaining multiplicities and every coefficient/guard.
        let mut matched = vec![false; other.len()];
        for row in rows {
            let index = other
                .iter()
                .enumerate()
                .find(|(index, candidate)| !matched[*index] && *candidate == row)
                .map(|(index, _)| index)
                .expect("reflected bubble lost an exact fixed-d equation");
            matched[index] = true;
        }
        assert!(matched.into_iter().all(|value| value));
    }
}

#[test]
fn numerator_factors_enlarge_spectator_span_without_parent_loop_count_dispatch() {
    let context = CoefficientContext::new(["d"]);
    let loops = 3;
    let rows = [
        [1, 0, 0, 0, 0, 0],
        [1, -2, 0, 1, 0, 0],
        [1, 0, -2, 0, 0, 1],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 1, 0],
        [0, 0, 0, 0, 0, 1],
    ];
    let family = Arc::new(
        IntegralFamily::new(
            "local-numerator-spectators",
            (0..loops).map(|i| format!("k{i}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            rows.into_iter()
                .enumerate()
                .map(|(i, row)| {
                    AffineDenominator::new(
                        context.integer(-(i as i64 + 1)),
                        row.into_iter().map(|n| context.integer(n)).collect(),
                    )
                })
                .collect(),
            vec![],
            vec![context.zero(); 6],
        )
        .unwrap(),
    );
    let source = key(&[1, 1, -1, 1, 0, 1]);
    let prepared = plan(family, &[source]);
    let witness = prepared
        .witnesses
        .iter()
        .find(|witness| witness.direction == vec![context.one(), context.zero(), context.zero()])
        .unwrap();
    assert_eq!(witness.spectator_rank, 2);
}

#[test]
fn spectator_rank_four_keeps_d_minus_four_exactly() {
    let context = CoefficientContext::new(["d"]);
    let loops = 5;
    let coordinates: Vec<_> = (0..loops)
        .flat_map(|left| (left..loops).map(move |right| (left, right)))
        .collect();
    let arity = coordinates.len();
    let denominators = coordinates
        .iter()
        .enumerate()
        .map(|(slot, &(left, right))| {
            let mut row = vec![context.zero(); arity];
            row[slot] = context.one();
            AffineDenominator::new(
                if left == right {
                    context.integer(-1)
                } else {
                    context.zero()
                },
                row,
            )
        })
        .collect();
    let family = Arc::new(
        IntegralFamily::new(
            "local-four-spectators",
            (0..loops).map(|i| format!("k{i}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); arity],
        )
        .unwrap(),
    );
    let source = key(&coordinates
        .iter()
        .map(|&(left, right)| if left == 0 && right > 0 { -1 } else { 0 })
        .collect::<Vec<_>>());
    let prepared = plan(family.clone(), &[source.clone()]);
    let mut direction = vec![context.zero(); loops];
    direction[0] = context.one();
    let witness = prepared
        .witnesses
        .iter()
        .find(|witness| witness.direction == direction)
        .unwrap();
    assert_eq!(witness.spectator_rank, 4);
    let d_factor = context
        .try_sub(
            &context.integer(4),
            family.dimension(),
            ExactAlgebraLimits::unbounded_resources(),
        )
        .unwrap();
    // These ISP denominators are k0.kr, so their symmetric quadratic H
    // entries are 1/2. The independent ORIGINAL residuals are qr=kr/2;
    // Gram(q) therefore has determinant Gram(kr)/4^4, not Gram(kr).
    // Its constant term is 1/256 for this diagonal unit-mass fixture.
    let expected = context
        .try_div(
            &d_factor,
            &context.integer(256),
            ExactAlgebraLimits::unbounded_resources(),
        )
        .unwrap();
    assert_eq!(witness.equation.terms.get(&source), Some(&expected));
}

#[test]
fn degenerate_external_gram_is_reported_and_not_divided() {
    let context = CoefficientContext::new(["d"]);
    let family = Arc::new(
        IntegralFamily::new(
            "local-null-external",
            vec!["k".into()],
            vec!["p".into()],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(context.integer(-1), vec![context.one(), context.zero()]),
                AffineDenominator::new(
                    context.integer(-2),
                    vec![context.one(), context.integer(-2)],
                ),
            ],
            vec![vec![context.zero()]],
            vec![context.zero(); 2],
        )
        .unwrap(),
    );
    let source = key(&[1, 1]);
    let prepared = plan(family, &[source.clone()]);
    assert!(prepared.equations_for(&source).is_empty());
    assert!(prepared.statistics().degenerate_grams > 0);
    assert!(
        prepared
            .statistics()
            .skipped_reasons
            .keys()
            .any(|reason| reason.contains("Gram"))
    );
}

#[test]
fn preparation_cancellation_wrong_arity_and_defaults_are_explicit() {
    let family = tadpole(-1, false);
    let always_cancelled = || true;
    assert!(matches!(
        LocalDimensionRecurrencePlan::prepare(
            family.clone(),
            &BTreeSet::from([key(&[1])]),
            Default::default(),
            &LocalDimensionRecurrenceControl {
                cancelled: Some(&always_cancelled),
                progress: None
            }
        ),
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        LocalDimensionRecurrencePlan::prepare(
            family,
            &BTreeSet::from([key(&[1, 1])]),
            Default::default(),
            &Default::default()
        ),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(
        LocalDimensionRecurrenceOptions::default().exact_algebra,
        ExactAlgebraLimits::unbounded_resources()
    );
}
