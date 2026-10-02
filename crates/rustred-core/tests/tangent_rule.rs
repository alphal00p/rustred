//! Public-API integration of generated tangent sources and checked local rules.
//!
//! The common four-loop basis is input data, matching the portable example in
//! `examples/input/four_loop_combined/four_loop_common_basis.toml`. No owner
//! artifact, external program, campaign directory or guessed reduction is used.

use rustred::algebra::IndexedCoefficientContext;
use rustred::family::{ContractionMomentum, IntegralFamily, IntegralKey};
use rustred::foundry::artifact::{
    OriginalSourceCombinationRequest, OriginalSourceContribution, check_original_source_combination,
};
use rustred::foundry::cell::FixedIndexRestriction;
use rustred::identity::{
    IntegralShift, ParametricIbpGenerator, TangentSourceCombination, TangentSourcePlan,
    TangentSourceSpec,
};
use rustred::input::{Compiler, TextProject, TextPropagator};
use rustred::sector::{Mask, OrderingPolicy};

fn family() -> IntegralFamily {
    let denominators = [
        "k1^2-1",
        "k2^2-1",
        "k3^2-1",
        "k4^2-1",
        "(k1-k4)^2-1",
        "(k2-k4)^2-1",
        "(k3-k4)^2-1",
        "(k1-k2)^2-1",
        "(k1-k3)^2-1",
        "(k1-k2-k3)^2-1",
    ];
    Compiler::new(Default::default())
        .unwrap()
        .compile_text(TextProject {
            name: Some("tangent_rule_integration".into()),
            parameters: None,
            loop_momenta: (1..=4).map(|i| format!("k{i}")).collect(),
            external_momenta: vec![],
            dimension: "d".into(),
            propagators: denominators
                .into_iter()
                .enumerate()
                .map(|(i, expression)| TextPropagator {
                    id: format!("D{}", i + 1),
                    expression: expression.into(),
                    target_power: i64::from(i < 8),
                    power_shift: None,
                })
                .collect(),
            external_gram: vec![],
            numerator: None,
        })
        .unwrap()
        .into_lowered(Default::default())
        .unwrap()
        .into_family()
}

fn materialize(family: &IntegralFamily) -> (IndexedCoefficientContext, TangentSourceCombination) {
    let generator = ParametricIbpGenerator::try_new(family).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
    let completed = prepared.complete(rows).unwrap();
    let mut recenter = vec![0; 10];
    recenter[8] = 1;
    let plan = TangentSourcePlan::try_new(
        family,
        &TangentSourceSpec {
            differentiated_loop: 2,
            protected_denominators: [2, 6],
            contractions: [
                ContractionMomentum::Loop(2),
                ContractionMomentum::Loop(0),
                ContractionMomentum::Loop(3),
            ],
            recenter: IntegralShift::try_new(recenter).unwrap(),
            multiplier: None,
        },
        Default::default(),
    )
    .unwrap();
    let combination = plan
        .materialize(&generator, &completed, Default::default())
        .unwrap();
    (generator.context().clone(), combination)
}

#[test]
fn symbolic_bubble_source_preserves_rank_one_boundary() {
    let family = family();
    let (context, combination) = materialize(&family);
    assert!(!combination.conditions().is_empty());
    assert_eq!(combination.sources().len(), combination.weights().len());
    let target = combination
        .product()
        .keys()
        .find(|shift| shift.values().iter().all(|&n| n == 0))
        .unwrap();
    let pivot = context
        .specialize_fixed_indices(
            &combination.product()[target],
            &[(9, 0)],
            Default::default(),
        )
        .unwrap()
        .0;
    let expected = context
        .mul(
            &context.integer(-2),
            &context
                .add(
                    &context
                        .sub(
                            &context.index(8).unwrap(),
                            &context.lift(family.dimension()).unwrap(),
                        )
                        .unwrap(),
                    &context.integer(3),
                )
                .unwrap(),
        )
        .unwrap();
    assert_eq!(pivot, expected);

    // These are independent boundary checks of the full row, not a sampled
    // replacement for the checked rule's later unbounded domain proof.
    for rank in [1, 2, 5] {
        let mut powers = vec![1i64; 10];
        powers[0] = 2;
        powers[8] = -rank;
        powers[9] = 0;
        let parent_degree: i64 = powers.iter().map(|n| n.abs()).sum();
        let mut nonzero_rhs = 0;
        for (shift, coefficient) in combination.product() {
            let value = context
                .specialize(coefficient, &powers, Default::default())
                .unwrap()
                .0;
            if shift == target || value.is_zero() {
                continue;
            }
            let child: Vec<_> = powers
                .iter()
                .zip(shift.values())
                .map(|(n, s)| n + s)
                .collect();
            assert!(child[8] <= 0 && child[9] == 0);
            assert!(child.iter().map(|n| n.abs()).sum::<i64>() < parent_degree);
            nonzero_rhs += 1;
        }
        assert!(nonzero_rhs > 0);
    }
}

#[test]
fn generated_tangent_identity_passes_unbounded_original_source_admission() {
    let family = family();
    let (context, combination) = materialize(&family);
    let fixed = [(9, 0)];
    let target = combination
        .product()
        .keys()
        .find(|shift| shift.values().iter().all(|&n| n == 0))
        .unwrap();
    let pivot = context
        .specialize_fixed_indices(&combination.product()[target], &fixed, Default::default())
        .unwrap()
        .0;
    let contributions = combination
        .sources()
        .sources()
        .iter()
        .zip(combination.weights())
        .map(|(source, weight)| OriginalSourceContribution {
            source_row: source.provenance().source_row().clone(),
            offset: source.provenance().offset().clone(),
            weight: context.div(weight, &pivot).unwrap(),
        })
        .collect();
    let mut conditions: Vec<_> = combination
        .conditions()
        .iter()
        .map(|condition| condition.polynomial().clone())
        .collect();
    let mut rhs = vec![];
    for (shift, coefficient) in combination.product() {
        let (coefficient, denominator) = context
            .specialize_fixed_indices(coefficient, &fixed, Default::default())
            .unwrap();
        conditions.push(denominator);
        if shift == target || coefficient.is_zero() {
            continue;
        }
        rhs.push((
            shift.clone(),
            context
                .div(
                    &context
                        .neg_with_limits(&coefficient, Default::default())
                        .unwrap(),
                    &pivot,
                )
                .unwrap(),
        ));
    }
    let mut lower = vec![0; 10];
    lower[8] = 1; // n8 <= -1, with no numerator-rank upper bound.
    let mut upper = vec![None; 10];
    upper[9] = Some(0);
    let admitted_started = std::time::Instant::now();
    let checked = check_original_source_combination(
        &family,
        OriginalSourceCombinationRequest {
            root_sector: Mask::try_new((0..10).map(|i| i < 9)).unwrap(),
            sector: Mask::try_new((0..10).map(|i| i < 8)).unwrap(),
            ordering: OrderingPolicy::SpiredUncutV1,
            lower,
            upper,
            fixed: vec![FixedIndexRestriction::new(9, 0)],
            contributions,
            rhs,
            retained_conditions: conditions,
        },
        Default::default(),
    )
    .unwrap();
    eprintln!(
        "tangent original-source admission: {} exact sign cells in {:.3}s (test diagnostic, not a campaign benchmark)",
        checked.cells().len(),
        admitted_started.elapsed().as_secs_f64(),
    );
    assert_eq!(checked.family_fingerprint(), family.fingerprint());
    assert!(checked.cells().len() > 0);
    // Native admission above proves each whole (possibly unbounded) sign cell.
    // These checks additionally catch accidental loss of boundary/interior
    // membership in the executable carrier.
    for rank in [1, 2, 5, 30] {
        for bubble_power in [1, 2, 7] {
            let mut powers = vec![1; 10];
            powers[2] = bubble_power;
            powers[6] = bubble_power;
            powers[8] = -rank;
            powers[9] = 0;
            let target = IntegralKey::try_new(powers).unwrap();
            assert_eq!(
                checked
                    .cells()
                    .filter(|cell| cell.assignment_for_target(&target).unwrap().is_some())
                    .count(),
                1,
            );
        }
    }
    for outside in [vec![1; 10], vec![1, 1, 1, 1, 1, 1, 1, 1, 0, 0]] {
        let target = IntegralKey::try_new(outside).unwrap();
        assert!(
            checked
                .cells()
                .all(|cell| cell.assignment_for_target(&target).unwrap().is_none())
        );
    }
}
