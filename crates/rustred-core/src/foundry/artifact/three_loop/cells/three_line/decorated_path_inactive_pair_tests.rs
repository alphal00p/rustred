use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, SourceViewBatch};
use crate::foundry::parametric::{ParametricRule, ParametricRuleError};
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::sector::InteriorBounds;

use super::super::super::{canonical_family, canonical_s4};
use super::decorated_path_inactive_pair::{
    ADJACENT_INACTIVE_PAIR_PIVOT, DecoratedPathInactivePairBuild, FREE_POSITION,
    OPPOSITE_INACTIVE_PAIR_PIVOT, PAIR_REPLAY_ANCHOR, PATH_SECTOR, SHIFTED_DOT_PIVOT,
    derive_decorated_path_inactive_pair_build, derive_decorated_path_inactive_pair_cells,
    derive_opposite_candidate, derive_pair_candidate, fixed_opposite_source_face,
    fixed_pair_source_face, fixed_shifted_dot_source_face, pair_search_depth,
    shifted_dot_search_depth,
};

const ORDINARY_ROWS: [&str; 9] = [
    "ordinary-ibp:0:0",
    "ordinary-ibp:0:1",
    "ordinary-ibp:0:2",
    "ordinary-ibp:1:0",
    "ordinary-ibp:1:1",
    "ordinary-ibp:1:2",
    "ordinary-ibp:2:0",
    "ordinary-ibp:2:1",
    "ordinary-ibp:2:2",
];
const SAFE: [usize; 30] = [
    0, 3, 4, 5, 8, 9, 12, 13, 14, 17, 27, 30, 31, 32, 35, 36, 39, 40, 41, 44, 45, 48, 49, 50, 53,
    54, 57, 58, 59, 62,
];
const E_COMPLETE: [usize; 35] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 33, 34,
    35, 36, 39, 42, 45, 46, 48, 49, 51, 52,
];
const E_SELECTED: [usize; 7] = [12, 13, 17, 30, 31, 32, 35];
const U_COMPLETE: [usize; 24] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 18, 21, 22, 23, 26, 27, 28, 29, 30, 31, 33, 35,
];
const U_SELECTED: [usize; 11] = [0, 3, 9, 12, 13, 17, 27, 30, 31, 32, 35];
const V_SELECTED: [usize; 6] = [0, 1, 3, 4, 6, 7];
const E_RHS: [[i64; 6]; 6] = [
    [-1, 0, 0, 1, 0, 0],
    [0, -1, 0, 1, 0, 0],
    [0; 6],
    [0, 0, 0, 1, 0, 0],
    [0, -1, 0, 1, -1, 0],
    [0, 0, 0, 1, -1, 0],
];
const U_RHS: [[i64; 6]; 6] = [
    [-1, 0, 0, 1, 0, 0],
    [0, -1, 0, 1, 0, 0],
    [0; 6],
    [0, 0, -1, 0, 0, 1],
    [0, 0, 0, 1, 0, 0],
    [0, 0, -1, 1, 0, 0],
];

#[test]
fn exact_complete_search_safe_filter_and_compact_reprojection_are_pinned() {
    assert_eq!(
        derive_opposite_candidate(0),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    assert_eq!(
        derive_pair_candidate(0, &ADJACENT_INACTIVE_PAIR_PIVOT),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    let DecoratedPathInactivePairBuild {
        context: _,
        opposite,
        adjacent,
        shifted_dot,
        opposite_machine_safe_complete_source_ordinals,
        pair_machine_safe_complete_source_ordinals,
        opposite_selected_complete_source_ordinals,
        adjacent_selected_complete_source_ordinals,
        shifted_dot_machine_safe_complete_source_ordinals,
        shifted_dot_selected_complete_source_ordinals,
        selection_witness,
    } = derive_decorated_path_inactive_pair_build(true).unwrap();
    let witness = selection_witness.unwrap();
    let depth_one = search(pair_search_depth());
    let depth_zero = search(shifted_dot_search_depth());
    assert_eq!((pair_search_depth(), depth_one.offset_count()), (1, 7));
    assert_eq!(
        (shifted_dot_search_depth(), depth_zero.offset_count()),
        (0, 1)
    );
    for sources in [
        &witness.opposite_complete_sources,
        &witness.pair_complete_sources,
    ] {
        assert_eq!(sources.len(), 63);
        assert_provenance(sources, &(0..63).collect::<Vec<_>>(), &depth_one);
    }
    assert_eq!(ordinals(&witness.opposite_complete_rule), E_COMPLETE);
    assert_eq!(ordinals(&witness.adjacent_complete_rule), U_COMPLETE);
    assert_eq!(
        opposite_machine_safe_complete_source_ordinals.as_ref(),
        SAFE
    );
    assert_eq!(pair_machine_safe_complete_source_ordinals.as_ref(), SAFE);
    assert_eq!(witness.opposite_machine_safe_sources.len(), SAFE.len());
    assert_eq!(witness.pair_machine_safe_sources.len(), SAFE.len());
    assert_provenance(&witness.opposite_machine_safe_sources, &SAFE, &depth_one);
    assert_provenance(&witness.pair_machine_safe_sources, &SAFE, &depth_one);
    assert_eq!(
        opposite_selected_complete_source_ordinals.as_ref(),
        E_SELECTED
    );
    assert_eq!(
        adjacent_selected_complete_source_ordinals.as_ref(),
        U_SELECTED
    );
    assert_provenance(opposite.sources(), &E_SELECTED, &depth_one);
    assert_provenance(adjacent.sources(), &U_SELECTED, &depth_one);
    assert_eq!(ordinals(opposite.rule()), (0..7).collect::<Vec<_>>());
    assert_eq!(ordinals(adjacent.rule()), (0..11).collect::<Vec<_>>());

    assert_eq!(
        shifted_dot_machine_safe_complete_source_ordinals.as_ref(),
        (0..9).collect::<Vec<_>>()
    );
    assert_eq!(witness.shifted_dot_complete_sources.len(), 9);
    assert_eq!(witness.shifted_dot_machine_safe_sources.len(), 9);
    assert_provenance(
        &witness.shifted_dot_complete_sources,
        &(0..9).collect::<Vec<_>>(),
        &depth_zero,
    );
    assert_provenance(
        &witness.shifted_dot_machine_safe_sources,
        &(0..9).collect::<Vec<_>>(),
        &depth_zero,
    );
    assert_eq!(
        shifted_dot_selected_complete_source_ordinals.as_ref(),
        V_SELECTED
    );
    assert_eq!(ordinals(&witness.shifted_dot_complete_rule), V_SELECTED);
    assert_provenance(shifted_dot.sources(), &V_SELECTED, &depth_zero);
}

#[test]
fn exact_coefficients_guards_replay_domains_and_machine_bounds_are_pinned() {
    let build = derive_decorated_path_inactive_pair_build(true).unwrap();
    let witness = build.selection_witness.as_ref().unwrap();
    assert_e_coefficients(&build.context, build.opposite.rule());
    assert_coefficients_equal(&witness.opposite_machine_safe_rule, build.opposite.rule());
    assert_u_coefficients(&build.context, build.adjacent.rule());
    assert_coefficients_equal(&witness.adjacent_machine_safe_rule, build.adjacent.rule());
    for rule in [
        &witness.shifted_dot_machine_safe_rule,
        build.shifted_dot.rule(),
    ] {
        assert_v_coefficients(&build.context, rule);
    }
    assert_shape(
        build.opposite.rule(),
        &OPPOSITE_INACTIVE_PAIR_PIVOT,
        &E_RHS,
        (7, 18, 86),
        (7, 43, 6, 50, 1, 137, 22),
    );
    assert_shape(
        build.adjacent.rule(),
        &ADJACENT_INACTIVE_PAIR_PIVOT,
        &U_RHS,
        (11, 24, 162),
        (11, 81, 6, 88, 4, 251, 37),
    );
    assert_shape(
        build.shifted_dot.rule(),
        &SHIFTED_DOT_PIVOT,
        &[[0; 6], [0, 0, 0, 1, 0, 0]],
        (6, 10, 70),
        (6, 35, 2, 38, 0, 103, 17),
    );

    for (cell, bounds, fixed) in [
        (
            &build.opposite,
            e_bounds(i64::MIN),
            fixed_opposite_source_face().to_vec(),
        ),
        (
            &build.adjacent,
            u_bounds(i64::MIN),
            fixed_pair_source_face().to_vec(),
        ),
        (
            &build.shifted_dot,
            v_bounds(i64::MIN + 1),
            fixed_shifted_dot_source_face().to_vec(),
        ),
    ] {
        assert_eq!(
            cell.domain_proof(),
            RuleCellDomainProof::ReprovedSectorMonotone
        );
        assert_eq!(cell.application_domain().bounds(), bounds);
        assert_eq!(cell.fixed_restrictions(), fixed);
        assert!(cell.terms().iter().all(|term| term.descent().verify()));
    }
    assert_target_range(
        &build.opposite,
        [0, -1, 1, -1, 2, 1],
        [0, -1, 1, i64::MIN, 2, 1],
        [0, -1, 1, 0, 2, 1],
    );
    assert_target_range(
        &build.adjacent,
        [-1, 0, 2, -1, 1, 1],
        [-1, 0, 2, i64::MIN, 1, 1],
        [-1, 0, 2, 0, 1, 1],
    );
    assert_eq!(
        build
            .shifted_dot
            .assignment_for_target(&key([0, 0, 1, -2, 1, 2]))
            .unwrap(),
        Some(vec![0, 0, 1, -1, 1, 1])
    );
    assert_eq!(
        build
            .shifted_dot
            .assignment_for_target(&key([0, 0, 1, i64::MIN, 1, 2]))
            .unwrap(),
        Some(vec![0, 0, 1, i64::MIN + 1, 1, 1])
    );
    assert!(
        build
            .shifted_dot
            .assignment_for_target(&key([0, 0, 1, -1, 1, 2]))
            .unwrap()
            .is_none()
    );

    let (_, e, u, v) = derive_decorated_path_inactive_pair_cells().unwrap();
    for (left, right) in [
        (&e, &build.opposite),
        (&u, &build.adjacent),
        (&v, &build.shifted_dot),
    ] {
        assert_eq!(left.rule(), right.rule());
        assert_eq!(left.sources().relations(), right.sources().relations());
        assert_eq!(left.application_domain(), right.application_domain());
    }
}

#[test]
fn exact_s4_ownership_nonownership_and_canonical_children_are_pinned() {
    let (_, e, u, v) = derive_decorated_path_inactive_pair_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    for free in [-1, -2, i64::MIN] {
        for (cell, powers) in [(&e, [0, -1, 1, free, 2, 1]), (&u, [-1, 0, 2, free, 1, 1])] {
            let target = key(powers);
            let orbit = canonicalizer.orbit(&target).unwrap();
            assert_eq!((orbit.group_order(), orbit.orbit_size()), (24, 24));
            assert_eq!(orbit.canonical().integral(), &target);
            assert!(cell.assignment_for_target(&target).unwrap().is_some());
        }
        let nonowners = if free == -1 {
            vec![
                [-1, 0, 1, -1, 1, 2],
                [-1, 0, 1, -1, 2, 1],
                [0, -1, 1, -1, 1, 2],
            ]
        } else {
            vec![
                [free, 0, 1, -1, 1, 2],
                [free, 0, 1, -1, 2, 1],
                [free, 0, 2, -1, 1, 1],
                [-1, 0, 1, free, 1, 2],
                [-1, 0, 1, free, 2, 1],
                [0, -1, 1, free, 1, 2],
                [0, -1, 2, free, 1, 1],
            ]
        };
        for powers in nonowners {
            let target = key(powers);
            assert_eq!(
                canonicalizer.canonicalize(&target).unwrap().canonical(),
                &target
            );
            assert!(e.assignment_for_target(&target).unwrap().is_none());
            assert!(u.assignment_for_target(&target).unwrap().is_none());
        }
    }
    for free in [-2, -7, i64::MIN] {
        let target = key([0, 0, 1, free, 1, 2]);
        assert_eq!(canonicalizer.orbit(&target).unwrap().orbit_size(), 24);
        assert!(v.assignment_for_target(&target).unwrap().is_some());
        for powers in [
            [free, 0, 1, 0, 1, 2],
            [free, 0, 1, 0, 2, 1],
            [0, 0, 1, free, 2, 1],
            [0, 0, 2, free, 1, 1],
        ] {
            let canonical = canonicalizer.canonicalize(&key(powers)).unwrap();
            assert!(
                v.assignment_for_target(canonical.canonical())
                    .unwrap()
                    .is_none()
            );
        }
    }
    assert_eq!(
        children(&canonicalizer, &e, &[0, 0, 1, -1, 2, 1]),
        [
            vec![-1, 0, 1, 0, 2, 1],
            vec![0, 0, 2, -1, 1, 1],
            vec![0, 0, 1, -1, 2, 1],
            vec![0, 0, 1, 0, 2, 1],
            vec![0, 0, 1, -1, 1, 1],
            vec![0, 0, 1, 0, 1, 1],
        ]
    );
    assert_eq!(
        children(&canonicalizer, &u, &[0, 0, 2, -1, 1, 1]),
        [
            vec![-1, 0, 1, 0, 2, 1],
            vec![0, 0, 1, -1, 2, 1],
            vec![0, 0, 2, -1, 1, 1],
            vec![0, 0, 1, -1, 1, 2],
            vec![0, 0, 1, 0, 2, 1],
            vec![0, 0, 1, 0, 1, 1],
        ]
    );
    assert_eq!(
        children(&canonicalizer, &v, &[0, 0, 1, -1, 1, 1]),
        [vec![0, 0, 1, -1, 1, 1], vec![0, 0, 1, 0, 1, 1]]
    );
}

fn assert_e_coefficients(c: &crate::algebra::IndexedCoefficientContext, rule: &ParametricRule) {
    let d = dimension(c);
    let n = c.index(FREE_POSITION).unwrap();
    let r = sub(c, &sub(c, &scale(c, 2, &d), &n), &c.integer(2));
    let reciprocal = c.div(&c.one(), &r).unwrap();
    let sources = scaled(c, &reciprocal, &[1, 1, 1, -3, -3, -2, 1]);
    assert_eq!(coefficients(rule), sources.iter().collect::<Vec<_>>());
    assert_eq!(rule.pivot_guard().coefficient(), &r);
    let nr = c.mul(&n, &reciprocal).unwrap();
    let scalar = c
        .mul(
            &c.add(&sub(c, &scale(c, 2, &d), &c.integer(2)), &n).unwrap(),
            &reciprocal,
        )
        .unwrap();
    let rhs = [
        scale(c, -2, &nr),
        nr.clone(),
        scalar,
        nr.clone(),
        scale(c, -1, &nr),
        nr,
    ];
    assert_eq!(rhs_coefficients(rule), rhs.iter().collect::<Vec<_>>());
    let guard = c
        .numerator_condition_with_limits(&r, Default::default())
        .unwrap();
    assert_eq!(
        rule.nonzero_guards()
            .iter()
            .map(|g| g.polynomial())
            .collect::<Vec<_>>(),
        [&guard]
    );
}

fn assert_u_coefficients(c: &crate::algebra::IndexedCoefficientContext, rule: &ParametricRule) {
    let d = dimension(c);
    let n = c.index(FREE_POSITION).unwrap();
    let a = sub(c, &sub(c, &d, &n), &c.integer(1));
    let b = sub(c, &sub(c, &scale(c, 2, &d), &n), &c.integer(4));
    let n2 = c.mul(&n, &n).unwrap();
    let d2 = c.mul(&d, &d).unwrap();
    let cpoly = c
        .add(
            &c.add(
                &n2,
                &c.mul(&sub(c, &c.integer(5), &scale(c, 3, &d)), &n).unwrap(),
            )
            .unwrap(),
            &c.add(&sub(c, &scale(c, 2, &d2), &scale(c, 6, &d)), &c.integer(4))
                .unwrap(),
        )
        .unwrap();
    let ra = c.div(&c.one(), &a).unwrap();
    let rb = c.div(&c.one(), &b).unwrap();
    let rc = c.div(&c.one(), &cpoly).unwrap();
    let sources = vec![
        ra.clone(),
        scale(c, -1, &ra),
        scale(c, -1, &ra),
        c.mul(
            &sub(
                c,
                &sub(c, &scale(c, 3, &d), &scale(c, 2, &n)),
                &c.integer(5),
            ),
            &rc,
        )
        .unwrap(),
        rb.clone(),
        rb.clone(),
        scale(c, -1, &ra),
        c.mul(
            &c.add(&sub(c, &scale(c, 4, &n), &scale(c, 5, &d)), &c.integer(7))
                .unwrap(),
            &rc,
        )
        .unwrap(),
        scale(c, -3, &rb),
        scale(c, -2, &rb),
        rb.clone(),
    ];
    assert_eq!(coefficients(rule), sources.iter().collect::<Vec<_>>());
    assert_eq!(rule.pivot_guard().coefficient(), &b);
    let rhs = vec![
        c.mul(
            &c.mul(
                &n,
                &c.add(&sub(c, &scale(c, 3, &n), &scale(c, 4, &d)), &c.integer(6))
                    .unwrap(),
            )
            .unwrap(),
            &rc,
        )
        .unwrap(),
        c.mul(&c.mul(&n, &sub(c, &c.integer(3), &d)).unwrap(), &rc)
            .unwrap(),
        c.mul(&sub(c, &scale(c, 4, &d), &c.integer(6)), &rb)
            .unwrap(),
        scale(c, 2, &rb),
        c.mul(&c.mul(&n, &sub(c, &c.integer(3), &d)).unwrap(), &rc)
            .unwrap(),
        c.mul(
            &c.mul(
                &n,
                &sub(
                    c,
                    &sub(c, &scale(c, 6, &d), &scale(c, 4, &n)),
                    &c.integer(10),
                ),
            )
            .unwrap(),
            &rc,
        )
        .unwrap(),
    ];
    assert_eq!(rhs_coefficients(rule), rhs.iter().collect::<Vec<_>>());
    let minus_a = scale(c, -1, &a);
    let guards = [&minus_a, &b, &cpoly, &a].map(|value| {
        c.numerator_condition_with_limits(value, Default::default())
            .unwrap()
    });
    assert_eq!(
        rule.nonzero_guards()
            .iter()
            .map(|g| g.polynomial())
            .collect::<Vec<_>>(),
        guards.iter().collect::<Vec<_>>()
    );
}

fn assert_v_coefficients(c: &crate::algebra::IndexedCoefficientContext, rule: &ParametricRule) {
    let d = dimension(c);
    let n = c.index(FREE_POSITION).unwrap();
    let half = c.div(&c.one(), &c.integer(2)).unwrap();
    let sources = [
        c.integer(-1),
        c.integer(-1),
        half.clone(),
        half.clone(),
        half.clone(),
        half.clone(),
    ];
    assert_eq!(coefficients(rule), sources.iter().collect::<Vec<_>>());
    assert_eq!(rule.pivot_guard().coefficient(), &c.integer(2));
    let rhs = [
        c.mul(&sub(c, &d, &scale(c, 3, &n)), &half).unwrap(),
        c.mul(&scale(c, -3, &n), &half).unwrap(),
    ];
    assert_eq!(rhs_coefficients(rule), rhs.iter().collect::<Vec<_>>());
    assert!(rule.nonzero_guards().is_empty());
}

fn assert_shape(
    rule: &ParametricRule,
    pivot: &[i64; 6],
    rhs: &[[i64; 6]],
    p: (usize, usize, usize),
    c: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(rule.anchor().powers(), PAIR_REPLAY_ANCHOR);
    assert_eq!(rule.pivot().values(), pivot);
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|t| t.shift().values())
            .collect::<Vec<_>>(),
        rhs.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>()
    );
    assert_eq!(
        (
            rule.replay().source_rows_used(),
            rule.replay().shift_columns_checked(),
            rule.replay().exact_operations()
        ),
        p
    );
    let r = rule.concrete_replay();
    assert_eq!(
        (
            r.source_contributions_checked(),
            r.source_terms_checked(),
            r.right_hand_side_terms_checked(),
            r.integral_keys_checked(),
            r.nonzero_guards_checked(),
            r.exact_operations(),
            r.peak_retained_coefficient_terms()
        ),
        c
    );
}

fn assert_coefficients_equal(left: &ParametricRule, right: &ParametricRule) {
    assert_eq!(coefficients(left), coefficients(right));
    assert_eq!(
        left.right_hand_side()
            .iter()
            .map(|term| (term.shift(), term.coefficient()))
            .collect::<Vec<_>>(),
        right
            .right_hand_side()
            .iter()
            .map(|term| (term.shift(), term.coefficient()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        left.pivot_guard().coefficient(),
        right.pivot_guard().coefficient()
    );
}

fn search(depth: usize) -> SectorSearchDiamond {
    SectorSearchDiamond::try_new(
        IntegralKey::try_new(PATH_SECTOR).unwrap(),
        depth,
        SectorSearchLimits::default(),
    )
    .unwrap()
}
fn assert_provenance(sources: &SourceViewBatch, ordinals: &[usize], search: &SectorSearchDiamond) {
    for (source, &ordinal) in sources.provenance().iter().zip(ordinals) {
        assert_eq!(source.translated().offset(), &search.offsets()[ordinal / 9]);
        assert_eq!(
            source.translated().source_row().stable_string(),
            ORDINARY_ROWS[ordinal % 9]
        );
    }
}
fn ordinals(rule: &ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|s| s.source_ordinal())
        .collect()
}
fn coefficients(rule: &ParametricRule) -> Vec<&crate::algebra::IndexedCoefficient> {
    rule.source_combination()
        .iter()
        .map(|s| s.coefficient())
        .collect()
}
fn rhs_coefficients(rule: &ParametricRule) -> Vec<&crate::algebra::IndexedCoefficient> {
    rule.right_hand_side()
        .iter()
        .map(|t| t.coefficient())
        .collect()
}
fn dimension(c: &crate::algebra::IndexedCoefficientContext) -> crate::algebra::IndexedCoefficient {
    c.lift(&c.base().coefficient_fixture("d")).unwrap()
}
fn scale(
    c: &crate::algebra::IndexedCoefficientContext,
    k: i64,
    v: &crate::algebra::IndexedCoefficient,
) -> crate::algebra::IndexedCoefficient {
    c.mul(&c.integer(k), v).unwrap()
}
fn scaled(
    c: &crate::algebra::IndexedCoefficientContext,
    v: &crate::algebra::IndexedCoefficient,
    ks: &[i64],
) -> Vec<crate::algebra::IndexedCoefficient> {
    ks.iter().map(|&k| scale(c, k, v)).collect()
}
fn sub(
    c: &crate::algebra::IndexedCoefficientContext,
    l: &crate::algebra::IndexedCoefficient,
    r: &crate::algebra::IndexedCoefficient,
) -> crate::algebra::IndexedCoefficient {
    c.sub(l, r).unwrap()
}
fn assert_target_range(cell: &RuleCell, endpoint: [i64; 6], minimum: [i64; 6], outside: [i64; 6]) {
    assert!(
        cell.assignment_for_target(&key(endpoint))
            .unwrap()
            .is_some()
    );
    assert!(cell.assignment_for_target(&key(minimum)).unwrap().is_some());
    assert!(cell.assignment_for_target(&key(outside)).unwrap().is_none());
}
fn children(
    c: &crate::sector::symmetry::Canonicalizer,
    cell: &RuleCell,
    a: &[i64; 6],
) -> Vec<Vec<i64>> {
    cell.rule()
        .right_hand_side()
        .iter()
        .map(|t| {
            let raw = IntegralKey::try_new(std::array::from_fn::<_, 6, _>(|i| {
                a[i].checked_add(t.shift().values()[i]).unwrap()
            }))
            .unwrap();
            c.canonicalize(&raw).unwrap().canonical().powers().to_vec()
        })
        .collect()
}
fn key(p: [i64; 6]) -> IntegralKey {
    IntegralKey::try_new(p).unwrap()
}
fn e_bounds(l: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(l, -1),
        InteriorBounds::new(2, 2),
        InteriorBounds::new(1, 1),
    ]
}
fn u_bounds(l: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(2, 2),
        InteriorBounds::new(l, -1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}
fn v_bounds(l: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(l, -1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}
