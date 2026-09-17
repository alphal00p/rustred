use super::conjunction::expression;
use super::*;

#[test]
fn captured_joint_guard_consequences_miss_the_actual_box() {
    let context = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "joint-bounds-capture",
        10,
    )
    .unwrap();
    let captured = "6*n9+2*n9^2-2*n7+n7*n9-n7^2+4*n0*n9-n0*n7-4*d*n9+d*n7";
    for order in [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        [0, 1, 2, 3, 4, 5, 6, 9, 8, 7],
    ] {
        let mut text = captured.to_owned();
        for axis in 0..10 {
            text = text.replace(&format!("n{axis}"), &format!("{{{axis}}}"));
        }
        for (axis, replacement) in order.into_iter().enumerate() {
            text = text.replace(&format!("{{{axis}}}"), &format!("n{replacement}"));
        }
        let guard = polynomial(&context, &expression(&context, &text));
        let mut sector = [false; 10];
        for axis in [1, 3, 4, 8] {
            sector[order[axis]] = true;
        }
        let mut lower = [0; 10];
        for axis in [0, 2, 7, 9] {
            lower[order[axis]] = 1;
        }
        let mut upper = lower.map(Some);
        upper[order[7]] = None;
        upper[order[9]] = None;
        let check = |lower| {
            validate_guard_on_domain_with_limits(
                &context,
                &guard,
                &LatticeBox::try_new(lower, upper).unwrap(),
                &sector,
                None,
                &[],
                Default::default(),
            )
        };
        check(lower).unwrap();
        // The common coefficient-zero solution n7=n9=0 is real; admitting
        // it must not retain the smaller-domain proof.
        lower[order[7]] = 0;
        lower[order[9]] = 0;
        assert!(check(lower).is_err());
        assert!(
            context
                .specialize_fixed_polynomial(
                    &guard,
                    &[(order[0], -1), (order[7], 0), (order[9], 0)],
                    Default::default(),
                )
                .unwrap()
                .is_zero()
        );
    }
}

#[test]
fn joint_fixed_values_check_sign_and_both_finite_endpoints() {
    let context = context();
    for (expression_text, sector, lower, upper, widened_lower, widened_upper) in [
        // Each equality separately intersects the positive orthant, but their
        // simultaneous solution is zero, which is not a positive power.
        (
            "n0-n1+d*(n0-2*n1)",
            [true; 3],
            [0; 3],
            [None; 3],
            [0; 3],
            [None; 3],
        ),
        (
            "n0-n1+d*(n0-2*n1-1)",
            [false; 3],
            [2, 2, 0],
            [None; 3],
            [0; 3],
            [None; 3],
        ),
        (
            "n0-n1+d*(n0-2*n1-4)",
            [false; 3],
            [0; 3],
            [Some(3), Some(3), None],
            [0; 3],
            [None; 3],
        ),
        (
            "n0-n1+d*(n0-2*n1+4)",
            [true; 3],
            [4, 4, 0],
            [None; 3],
            [0; 3],
            [None; 3],
        ),
    ] {
        let guard = polynomial(&context, &expression(&context, expression_text));
        validate_guard_on_domain_with_limits(
            &context,
            &guard,
            &LatticeBox::try_new(lower, upper).unwrap(),
            &sector,
            None,
            &[],
            Default::default(),
        )
        .unwrap();
        if expression_text == "n0-n1+d*(n0-2*n1)" {
            assert!(
                validate_guard_on_domain_with_limits(
                    &context,
                    &guard,
                    &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
                    &[false; 3],
                    None,
                    &[],
                    Default::default(),
                )
                .is_err()
            );
        } else {
            assert!(
                validate_guard_on_domain_with_limits(
                    &context,
                    &guard,
                    &LatticeBox::try_new(widened_lower, widened_upper).unwrap(),
                    &sector,
                    None,
                    &[],
                    Default::default(),
                )
                .is_err()
            );
        }
    }
}
