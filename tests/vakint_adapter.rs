#![cfg(feature = "legacy-authored-oracles")]

// Keep every Symbolica-backed adapter check on one test worker.  The restricted
// Symbolica license used in CI permits one process and requires first-thread use.

use rustred::legacy_oracle_support::symbolica_atom::{Atom, AtomCore, try_parse};
use rustred::{
    Integral, TwoLoopReductionConfig, VakintAdapterError, VakintAdapterLimits, VakintTwoLoopAdapter,
};

fn small_adapter() -> VakintTwoLoopAdapter {
    VakintTwoLoopAdapter::build(
        TwoLoopReductionConfig {
            // The acceptance cases need only the two masters and analytic
            // pair-sector boundary formulas, not a dotted top-sector table.
            max_dots: 0,
            max_numerator_degree: 2,
            max_seed_candidates: 100,
            max_boundary_terms: 100_000,
        },
        VakintAdapterLimits::default(),
    )
    .unwrap()
}

fn assert_canonical_expansions_equal(actual: &Atom, expected: &Atom) {
    assert_eq!(
        actual.expand(),
        expected.expand(),
        "expressions differ after exact Symbolica expansion\nactual: {actual}\nexpected: {expected}"
    );
}

#[test]
fn form_free_vakint_two_loop_vertical_slice() {
    let mut adapter = small_adapter();

    // Arbitrarily decorated index Atoms are interned structurally and retained
    // exactly; non-Vakint spectator functions (including an inverse) stay inert.
    let input = adapter
        .parse(
            "spectator::weight(spectator::tag(9))^-1 \
             * k(1,spectator::mink(spectator::dim,spectator::slot(7))) \
             * k(1,spectator::mink(spectator::dim,spectator::slot(8))) \
             * topo(I2L(mu2,1,1,1))",
        )
        .unwrap();
    let decoded = adapter.decode(input.as_view()).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].integral(), &Integral::from([1, 1, 1]));
    assert_eq!(decoded[0].index_atoms().len(), 2);
    for expected in [
        try_parse!("spectator::mink(spectator::dim,spectator::slot(7))").unwrap(),
        try_parse!("spectator::mink(spectator::dim,spectator::slot(8))").unwrap(),
    ] {
        assert!(decoded[0].index_atoms().contains(&expected));
    }
    assert_eq!(
        decoded[0].spectator(),
        &try_parse!("spectator::weight(spectator::tag(9))^-1").unwrap()
    );

    // k1(mu) k1(nu) projects and lowers to
    // g(mu,nu)/d * [P - mu2*S], with explicit stable I2L powers.
    let output = adapter.reduce_atom(input.as_view()).unwrap();
    let expected = adapter
        .parse(
            "spectator::weight(spectator::tag(9))^-1 \
             * g(spectator::mink(spectator::dim,spectator::slot(7)), \
                 spectator::mink(spectator::dim,spectator::slot(8))) \
             / rustred::d \
             * (topo(I2L(mu2,0,1,1))-mu2*topo(I2L(mu2,1,1,1)))",
        )
        .unwrap();
    assert_canonical_expansions_equal(&output, &expected);

    // Dot notation is lowered without a tensor projector or FORM.
    let dot = adapter
        .reduce_str("dot(k(1),k(1))*topo(I2L(mu2,1,1,1))")
        .unwrap();
    let expected_dot = adapter
        .parse("topo(I2L(mu2,0,1,1))-mu2*topo(I2L(mu2,1,1,1))")
        .unwrap();
    assert_canonical_expansions_equal(&dot, &expected_dot);

    // An input metric can contract arbitrary decorated dummy indices.  Here
    // g(mu,nu) k1(mu) k2(nu) becomes k1.k2 before scalar reduction.
    let contracted = adapter
        .reduce_str(
            "g(spectator::mu,spectator::nu) \
             * k(1,spectator::mu)*k(2,spectator::nu) \
             * topo(I2L(mu2,1,1,1))",
        )
        .unwrap();
    let expected_contracted = adapter
        .parse(
            "-1/2*topo(I2L(mu2,0,1,1)) \
             +mu2/2*topo(I2L(mu2,1,1,1))",
        )
        .unwrap();
    assert_canonical_expansions_equal(&contracted, &expected_contracted);

    // A reserved head nested in an otherwise opaque spectator is rejected,
    // because silently preserving loop-momentum dependence would be wrong.
    let nested = adapter
        .parse("spectator::opaque(k(1,spectator::mu))*topo(I2L(mu2,1,1,1))")
        .unwrap();
    assert!(matches!(
        adapter.decode(nested.as_view()),
        Err(VakintAdapterError::UnsupportedReservedFactor(_))
    ));

    let external_dot = adapter
        .parse("dot(k(1),spectator::p(1))*topo(I2L(mu2,1,1,1))")
        .unwrap();
    assert!(matches!(
        adapter.decode(external_dot.as_view()),
        Err(VakintAdapterError::UnsupportedScalarProduct(_))
    ));

    let negative_tensor_power = adapter
        .parse("k(1,spectator::mu)^-1*topo(I2L(mu2,1,1,1))")
        .unwrap();
    assert!(matches!(
        adapter.decode(negative_tensor_power.as_view()),
        Err(VakintAdapterError::UnsupportedPower(_))
    ));

    // Controlled distribution enforces its term cap before allocating the
    // Cartesian product.
    let mut tight_limits = VakintAdapterLimits::default();
    tight_limits.max_expanded_terms = 1;
    let tight =
        VakintTwoLoopAdapter::from_pipeline(adapter.pipeline().clone(), tight_limits).unwrap();
    let sum = tight
        .parse(
            "(k(1,spectator::mu)+k(2,spectator::mu)) \
             * topo(I2L(mu2,1,1,1))",
        )
        .unwrap();
    assert!(matches!(
        tight.decode(sum.as_view()),
        Err(VakintAdapterError::ResourceLimit {
            resource: "expanded terms",
            requested: 2,
            limit: 1,
        })
    ));
}
