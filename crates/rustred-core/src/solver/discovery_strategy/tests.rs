use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{Integral, Term};

fn row(context: &CoefficientContext, shifts: &[[i16; 2]]) -> PolynomialRow<2> {
    shifts
        .iter()
        .map(|&shift| Term {
            integral: Integral::symbolic(shift).unwrap(),
            coefficient: context.one().numerator,
        })
        .collect()
}

#[test]
fn default_has_no_plan_and_callbacks_materialize_once_with_ordinal_ties() {
    let context = CoefficientContext::new(["n", "m"]);
    let basis = vec![
        row(&context, &[[1, 0]]),
        row(&context, &[[-1, 0]]),
        row(&context, &[[2, 0]]),
    ];
    assert!(
        SourceDiscoveryStrategy::InputOrder
            .materialize(&basis)
            .unwrap()
            .is_none()
    );
    let mut calls = Vec::new();
    let plan = SourceVisitOrder::by_key(&basis, |ordinal, features| {
        calls.push(ordinal);
        features.terms
    })
    .unwrap();
    assert_eq!(calls, [0, 1, 2]);
    assert_eq!(plan.ordinals(), [0, 1, 2]);
    let rematerialized = SourceDiscoveryStrategy::Materialized(plan.clone())
        .materialize(&basis)
        .unwrap()
        .unwrap();
    assert_eq!(rematerialized, plan);
    assert_eq!(calls.len(), 3);
}

#[test]
fn weighted_features_read_native_counts_and_checked_shift_sums() {
    let context = CoefficientContext::new(["n", "m"]);
    let basis = vec![
        row(&context, &[[2, -3]]),
        row(&context, &[[-1, 4]]),
        row(&context, &[[1, 0], [0, 0]]),
    ];
    let f = SourceRowFeatures::read(&basis[0]).unwrap();
    assert_eq!((f.terms, f.coefficient_monomials), (1, 1));
    assert_eq!(f.positive_shifts, [2, 0]);
    assert_eq!(f.negative_shifts, [0, 3]);
    let strategy = SourceDiscoveryStrategy::Features(vec![SourceRowPriority {
        feature: SourceRowFeature::AbsoluteShifts(vec![1, 2]),
        descending: false,
    }]);
    assert_eq!(
        strategy.materialize(&basis).unwrap().unwrap().ordinals(),
        [2, 0, 1]
    );
    let strategy = SourceDiscoveryStrategy::Features(vec![SourceRowPriority {
        feature: SourceRowFeature::PositiveShifts(vec![1, 0]),
        descending: true,
    }]);
    assert_eq!(
        strategy.materialize(&basis).unwrap().unwrap().ordinals(),
        [0, 2, 1]
    );
}

#[test]
fn fixed_numeric_cut_powers_are_not_shift_features() {
    let context = CoefficientContext::new(["n", "m"]);
    let row = vec![Term {
        integral: Integral::new([
            crate::solver::Power::new(false, 1).unwrap(),
            crate::solver::Power::new(true, -3).unwrap(),
        ]),
        coefficient: context.one().numerator,
    }];
    let features = SourceRowFeatures::read(&row).unwrap();
    assert_eq!(features.positive_shifts, [0, 0]);
    assert_eq!(features.negative_shifts, [0, 3]);
}

#[test]
fn invalid_plans_and_feature_recipes_fail_closed() {
    for plan in [vec![], vec![0], vec![0, 0], vec![0, 2], vec![usize::MAX, 1]] {
        assert!(SourceVisitOrder::new(plan, 2).is_err());
    }
    for weights in [vec![], vec![1], vec![0, 0], vec![1_000_001, 1]] {
        let strategy = SourceDiscoveryStrategy::Features(vec![SourceRowPriority {
            feature: SourceRowFeature::NegativeShifts(weights),
            descending: false,
        }]);
        assert!(strategy.validate(2).is_err());
    }
    assert!(
        SourceDiscoveryStrategy::Features(vec![])
            .validate(2)
            .is_err()
    );
    assert!(
        SourceDiscoveryStrategy::Features(vec![
            SourceRowPriority {
                feature: SourceRowFeature::Terms,
                descending: false
            };
            9
        ])
        .validate(2)
        .is_err()
    );
    assert!(SourceVisitOrder::new(vec![], 0).is_ok());
}

#[test]
fn sector_callbacks_have_deterministic_ties_and_complete_original_ordinals() {
    let sectors = [[true, false], [true, true], [false, true], [true, false]];
    let mut calls = 0;
    let plan = SectorVisitOrder::by_key(&sectors, |_, sector| {
        calls += 1;
        std::cmp::Reverse(sector.iter().filter(|&&v| v).count())
    });
    assert_eq!(calls, 4);
    assert_eq!(plan.ordinals(), [1, 0, 2, 3]);
    assert!(SectorVisitOrder::new(vec![0, 1, 1, 3], 4).is_err());
}
