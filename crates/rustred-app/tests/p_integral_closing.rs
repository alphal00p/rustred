//! Closing artifacts (`family-close`) of massless propagator-type families:
//! the source-port audit replays the `L(L+1)` ordinary IBP rows of one
//! external momentum, and the artifact carries the uniform external-scale
//! homogeneity proof (powers of `q^2/q.q`, as `m^2` for a unit-mass vacuum).
use rustred_app::{
    ClosingArtifactReduceRequest, FamilyCloseRequest, MAX_CLOSING_RULE_APPLICATIONS,
    closing_artifact_reduce, family_close,
};

const BUBBLE: &str = "I(name(p1l_bubble),loops(k1),externals(q),dimension(d),\
    prop(D1,k1^2,1),prop(D2,(k1-q)^2,1),gram(q,q,-1))";

const KITE: &str = "I(name(p2l_kite),loops(k1,k2),externals(q),dimension(d),\
    prop(D1,k1^2,1),prop(D2,k2^2,1),prop(D3,(k1-q)^2,1),prop(D4,(k2-q)^2,1),\
    prop(D5,(k1-k2)^2,1),gram(q,q,-1))";

/// `(master powers, scale power)` of every term of one reduction.
fn reduce(artifact: &[u8], target: &[i64]) -> Vec<(Vec<i64>, i128)> {
    let reduction = closing_artifact_reduce(ClosingArtifactReduceRequest {
        diagnostics_enabled: false,
        load_limits: Default::default(),
        artifact: artifact.to_vec(),
        target_powers: target.to_vec(),
        max_rule_applications: MAX_CLOSING_RULE_APPLICATIONS,
    })
    .unwrap();
    reduction
        .terms()
        .iter()
        .map(|term| {
            (
                term.master_powers().to_vec(),
                term.common_mass_squared_power(),
            )
        })
        .collect()
}

#[test]
fn bubble_closes_onto_one_master_with_its_external_scale() {
    let closed = family_close(FamilyCloseRequest::new(BUBBLE)).unwrap();
    assert_eq!(reduce(closed.artifact(), &[3, 2]), [(vec![1, 1], -3)]);
    // (k-q)^2 / (k^2)^2 is scaleless.
    assert!(reduce(closed.artifact(), &[2, -1]).is_empty());
}

#[test]
fn kite_closes_onto_its_three_raw_terminals() {
    let closed = family_close(FamilyCloseRequest::new(KITE)).unwrap();
    let terms = reduce(closed.artifact(), &[2, 1, 1, 1, -1]);
    assert!(!terms.is_empty());
    for (master, scale) in terms {
        assert!(
            [[1, 1, 1, 1, 0], [1, 0, 0, 1, 1], [0, 1, 1, 0, 1]]
                .contains(&<[i64; 5]>::try_from(master.as_slice()).unwrap()),
            "{master:?}"
        );
        assert_eq!(scale, i128::from(master.iter().sum::<i64>() - 4));
    }
}
