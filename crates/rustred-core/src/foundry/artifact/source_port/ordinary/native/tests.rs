use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::Integral;

fn term(context: &CoefficientContext, shift: i16, value: &str) -> Term<1, Coefficient> {
    Term {
        integral: Integral::symbolic([shift]).unwrap(),
        coefficient: context.coefficient_fixture(value),
    }
}

#[test]
fn compact_weights_expand_back_into_original_source_positions() {
    let context = CoefficientContext::new(["n"]);
    let order = IntegralOrder::new([true], [false]);
    let rows = vec![
        vec![],
        vec![term(&context, 2, "1")],
        vec![term(&context, 1, "1")],
    ];
    let desired = vec![term(&context, 1, "1")];
    let weights = propose_verified(
        &rows,
        &desired,
        &order,
        |_| Ok(false),
        |_| Ok(false),
        |_| true,
        true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(weights, vec![context.zero(), context.zero(), context.one()]);
    verify(&rows, &desired, &weights, &order, |_| Ok(false)).unwrap();
}

#[test]
fn compact_full_replay_failure_uses_same_quotient_complete_exact_proposal() {
    let context = CoefficientContext::new(["n"]);
    let order = IntegralOrder::new([true], [false]);
    let rows = vec![
        vec![term(&context, 2, "n"), term(&context, 1, "n")],
        vec![term(&context, 2, "1")],
    ];
    let desired = vec![term(&context, 2, "1")];
    let mut accepted_proposals = 0;
    let weights = propose_verified(
        &rows,
        &desired,
        &order,
        |term| Ok(term.integral == Integral::symbolic([1]).unwrap()),
        |_| Ok(false),
        |_| {
            accepted_proposals += 1;
            true
        },
        true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        accepted_proposals, 1,
        "compact support must actually be attempted"
    );
    assert_eq!(weights, vec![context.zero(), context.one()]);
}

#[test]
fn compact_pole_rejection_uses_same_quotient_pole_free_fallback() {
    let context = CoefficientContext::new(["n"]);
    let order = IntegralOrder::new([true], [false]);
    let rows = vec![vec![term(&context, 1, "n")], vec![term(&context, 1, "1")]];
    let desired = vec![term(&context, 1, "1")];
    let mut rejected = 0;
    let weights = propose_verified(
        &rows,
        &desired,
        &order,
        |_| Ok(false),
        |_| Ok(false),
        |weights| {
            rejected += 1;
            assert_eq!(weights[0], context.coefficient_fixture("1/n"));
            false
        },
        true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(rejected, 1);
    assert_eq!(weights, vec![context.zero(), context.one()]);
}

#[test]
fn disabled_fast_path_preserves_complete_exact_proposal_without_admission_callback() {
    let context = CoefficientContext::new(["n"]);
    let order = IntegralOrder::new([true], [false]);
    let rows = vec![vec![term(&context, 1, "n")], vec![term(&context, 1, "1")]];
    let desired = vec![term(&context, 1, "1")];
    let expected = propose(&rows, &desired, &order, |_| Ok(false)).unwrap();
    let actual = propose_verified(
        &rows,
        &desired,
        &order,
        |_| Ok(false),
        |_| Ok(false),
        |_| panic!("disabled"),
        false,
    )
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn neither_modular_nor_exact_quotient_membership_bypasses_full_replay() {
    let context = CoefficientContext::new(["n"]);
    let order = IntegralOrder::new([true], [false]);
    let rows = vec![vec![term(&context, 2, "1"), term(&context, 1, "1")], vec![]];
    let desired = vec![term(&context, 2, "1")];
    assert!(
        propose_verified(
            &rows,
            &desired,
            &order,
            |term| Ok(term.integral == Integral::symbolic([1]).unwrap()),
            |_| Ok(false),
            |_| true,
            true,
        )
        .is_err()
    );
}

#[test]
fn unlucky_modular_membership_that_does_not_lift_uses_full_exact_fallback() {
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use symbolica::domains::finite_field::PrimeIteratorU64;

    let context = CoefficientContext::new(["n"]);
    let order = IntegralOrder::new([true], [false]);
    let prime = PrimeIteratorU64::new(1 << 61).next().unwrap();
    let mut rng = StdRng::seed_from_u64(0x736f_7572_6365_7300);
    let sampled_root = rng.random_range(1..prime);
    let rows = vec![
        vec![
            term(&context, 2, "1"),
            term(&context, 1, &format!("n-{sampled_root}")),
        ],
        vec![term(&context, 2, "1")],
    ];
    let desired = vec![term(&context, 2, "1")];
    let mut projected = rows.clone();
    projected.push(desired.clone());
    assert!(support::candidates(&projected, &order).contains(&vec![0]));
    assert!(
        propose(&rows[..1], &desired, &order, |_| Ok(false))
            .unwrap()
            .is_none()
    );
    let weights = propose_verified(
        &rows,
        &desired,
        &order,
        |_| Ok(false),
        |_| Ok(false),
        |_| panic!("false modular support must fail exact lifting before admission"),
        true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(weights, vec![context.zero(), context.one()]);
}
