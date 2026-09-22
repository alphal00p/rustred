use super::*;
use crate::solver::candidate_reduction::power_domain::{DomainPowerBounds, DomainPowerError};

fn allowed(x: [u64; 3], rank: Option<u32>, bounds: DomainPowerBounds) -> bool {
    let a = i128::from(x[0]) + i128::from(x[1]) + 2;
    let d = a - i128::from(x[2]);
    rank.is_none_or(|r| x[2] <= u64::from(r))
        && bounds.max_positive_power.is_none_or(|m| a <= i128::from(m))
        && bounds
            .min_power_difference
            .is_none_or(|m| d >= i128::from(m))
        && bounds
            .max_power_difference
            .is_none_or(|m| d <= i128::from(m))
}

fn contains(piece: &OwnerDomainMatchPiece<3>, x: [u64; 3]) -> bool {
    (0..3).all(|axis| {
        x[axis] >= piece.lower()[axis] && piece.upper()[axis].is_none_or(|u| x[axis] <= u)
    }) && allowed(x, piece.max_numerator_rank(), piece.power_bounds())
}

#[test]
fn power_bounded_partition_matches_concrete_ordered_dispatch() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(0);
    first.fixed[0] = Some(2);
    first.equalities.push(minus(&c, 1, 2));
    let mut second = rule(1);
    second
        .exceptions
        .push(vec![minus(&c, 0, 1), minus(&c, 1, 1)]);
    second.rhs.push(PreparedTerm {
        shift: [0; 3],
        coefficient: c.zero(),
        denominator: minus(&c, 0, 3),
    });
    batch(&mut p).rules = vec![first, second, rule(2)];
    batch(&mut p)
        .terminals
        .insert(IntegralKey::try_new([1, 1, 0]).unwrap());
    let mut expected = Vec::new();
    for x in 0..=2 {
        for y in 0..=2 {
            for z in 0..=2 {
                let point = [x, y, z];
                let disposition = concrete(
                    &p,
                    IntegralKey::try_new([x as i64 + 1, y as i64 + 1, -(z as i64)]).unwrap(),
                );
                expected.push((point, disposition));
            }
        }
    }
    for cap in [None, Some(2), Some(3), Some(4), Some(5), Some(6)] {
        for rank in [None, Some(0), Some(1), Some(2)] {
            for (min, max) in [
                (None, None),
                (Some(-1), Some(1)),
                (Some(1), Some(3)),
                (Some(3), Some(3)),
                (Some(5), Some(6)),
            ] {
                let bounds = DomainPowerBounds {
                    max_positive_power: cap,
                    min_power_difference: min,
                    max_power_difference: max,
                };
                let mut pieces = Vec::new();
                p.visit_power_bounded_owner_domain_matches(
                    OWNER,
                    &[0; 3],
                    &[Some(2); 3],
                    rank,
                    bounds,
                    Default::default(),
                    &AtomicBool::new(false),
                    |piece| {
                        assert_eq!(piece.power_bounds(), bounds);
                        pieces.push(piece);
                        ControlFlow::Continue(())
                    },
                )
                .unwrap();
                for &(point, expected) in &expected {
                    let hits: Vec<_> = pieces
                        .iter()
                        .filter(|piece| contains(piece, point))
                        .collect();
                    if allowed(point, rank, bounds) {
                        assert_eq!(hits.len(), 1, "{point:?} {bounds:?} {rank:?}");
                        assert_eq!(hits[0].disposition(), expected);
                    } else {
                        assert!(hits.is_empty(), "projection lost correlation at {point:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn sibling_face_effective_rank_does_not_leak() {
    let mut p = fixture();
    let mut first = rule(0);
    first.fixed[0] = Some(2);
    batch(&mut p).rules = vec![first, rule(1)];
    let bounds = DomainPowerBounds {
        max_positive_power: Some(5),
        min_power_difference: Some(3),
        max_power_difference: None,
    };
    let mut pieces = Vec::new();
    p.visit_power_bounded_owner_domain_matches(
        OWNER,
        &[0; 3],
        &[Some(3), Some(0), None],
        None,
        bounds,
        Default::default(),
        &AtomicBool::new(false),
        |piece| {
            pieces.push(piece);
            ControlFlow::Continue(())
        },
    )
    .unwrap();
    let low = pieces.iter().find(|p| contains(p, [1, 0, 0])).unwrap();
    assert_eq!(low.max_numerator_rank(), Some(0));
    let high = pieces.iter().find(|p| contains(p, [3, 0, 2])).unwrap();
    assert_eq!(high.max_numerator_rank(), Some(2));
    assert_eq!(
        high.disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
    );
}

#[test]
fn correlation_empty_fixed_face_spends_no_predicate_budget() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(0);
    first.fixed = [Some(3), Some(3), None];
    first.equalities.push(minus(&c, 0, 3));
    batch(&mut p).rules = vec![first, rule(1)];
    let bounds = DomainPowerBounds {
        max_positive_power: Some(4),
        ..Default::default()
    };
    let stats = p
        .visit_power_bounded_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[Some(2); 3],
            None,
            bounds,
            OwnerDomainMatchLimits {
                max_predicates: 0,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |piece| {
                assert_eq!(
                    piece.disposition(),
                    OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
                );
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert!(stats.correlation_empty_cells > 0);
    assert_eq!(stats.predicates, 0);
}

#[test]
fn malformed_power_domain_has_typed_error_and_retained_input() {
    let bounds = DomainPowerBounds {
        min_power_difference: Some(3),
        max_power_difference: Some(2),
        ..Default::default()
    };
    let error = fixture()
        .visit_power_bounded_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(7),
            bounds,
            Default::default(),
            &AtomicBool::new(false),
            |_| panic!("invalid"),
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        OwnerDomainMatchFailure::PowerDomain(DomainPowerError::InvertedDifferenceBounds)
    );
    assert_eq!(error.power_bounds, bounds);
    assert_eq!(error.max_numerator_rank, Some(7));
}
