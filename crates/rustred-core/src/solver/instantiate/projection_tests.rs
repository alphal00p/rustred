use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::Power;

fn mixed(offset: i16, fixed: i16) -> Integral<2> {
    Integral::new([
        Power::new(true, offset).unwrap(),
        Power::new(false, fixed).unwrap(),
    ])
}

#[test]
fn symbolic_zero_projection_keeps_potentially_activating_support() {
    let order = IntegralOrder::new([false, true], [false; 2]);
    for offset in [-2, -1, 0, 1, 2] {
        assert!(!vanishes_in_subsector(
            &mixed(offset, 0),
            &order,
            &[[false, false]],
        ));
    }
    // Duplicate evidence cannot replace the missing active-symbolic support.
    assert!(!vanishes_in_subsector(
        &mixed(1, 0),
        &order,
        &[[false, false], [false, false]],
    ));
    assert!(vanishes_in_subsector(
        &mixed(1, 0),
        &order,
        &[[false, false], [true, false]],
    ));
}

#[test]
fn zero_current_offset_is_not_safe_after_target_recentering() {
    let context = CoefficientContext::new(["a", "b"]);
    let source = vec![
        Term {
            integral: Integral::symbolic([-1, 0]).unwrap(),
            coefficient: context.one().numerator,
        },
        Term {
            integral: Integral::symbolic([0, -1]).unwrap(),
            coefficient: context.one().numerator,
        },
    ];
    let order = IntegralOrder::new([false, true], [false; 2]);
    let row = instantiate(
        &source,
        &Seed {
            integral: mixed(0, 1),
            shifts: [0; 2],
        },
        &[0, 1],
        &[None; 2],
        &order,
        &[[false, false]],
        None,
    )
    .unwrap();
    assert_eq!(row.len(), 2, "the zero-offset column must survive");
    let (target, rhs) = canonicalize(row, &[0, 1]).unwrap();
    assert_eq!(target, mixed(0, 1));
    assert_eq!(rhs.len(), 1);
    assert_eq!(rhs[0].integral, mixed(1, 0));
    assert_eq!(rhs[0].coefficient, context.integer(-1));
}

#[test]
fn active_symbolic_pinches_also_require_both_supports() {
    let order = IntegralOrder::new([true, true], [false; 2]);
    assert!(!vanishes_in_subsector(
        &mixed(-1, 0),
        &order,
        &[[true, false]],
    ));
    assert!(vanishes_in_subsector(
        &mixed(-1, 0),
        &order,
        &[[true, false], [false, false]],
    ));
}

#[test]
fn numeric_masks_and_fixed_missing_cuts_remain_cheap_and_exact() {
    let order = IntegralOrder::new([true, false], [false; 2]);
    let numeric = Integral::numeric([0, 1]).unwrap();
    assert!(!vanishes_in_subsector(&numeric, &order, &[[false; 2]]));
    assert!(vanishes_in_subsector(&numeric, &order, &[[false, true]]));

    let cut = IntegralOrder::new([false, true], [false, true]);
    assert!(vanishes_in_subsector(&mixed(4, 0), &cut, &[]));
    let symbolic_cut = IntegralOrder::new([true, true], [true, false]);
    assert!(!vanishes_in_subsector(&mixed(-1, 0), &symbolic_cut, &[]));
}

#[test]
fn projection_budget_exhaustion_retains_the_source_column() {
    let integral = Integral::<4>::symbolic([0; 4]).unwrap();
    let mut support = [false; 4];
    let mut small_budget = 3;
    assert!(!every_symbolic_support_is_zero(
        &integral,
        &mut support,
        0,
        &mut small_budget,
        &|_| true,
    ));
    let mut sufficient_budget = 32;
    assert!(every_symbolic_support_is_zero(
        &integral,
        &mut support,
        0,
        &mut sufficient_budget,
        &|_| true,
    ));
}
