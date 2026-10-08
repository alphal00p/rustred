//! Capacity axes are storage only, including after affine numerator pinches.
use super::*;

fn padded_fixture() -> RoutedCandidateReducer<4> {
    let family = Arc::new(crate::solver::tests::sunset());
    let programs = programs(
        family.clone(),
        Some(10),
        vec![input([true, false, true, false], Some(10), vec![], &[])],
        Default::default(),
    );
    let route = CandidateOwnerRoute {
        owner_sector: Mask::try_new(TARGET).unwrap(),
        transport: prepared(&family, SOURCE, TARGET, false),
    };
    RoutedCandidateReducer::try_new(programs, [route], Default::default()).unwrap()
}

fn narrow(event: CandidateDomainRouteEvent<4>) -> CandidateDomainRouteEvent<3> {
    fn cover(value: CandidateDomainRouteCover<4>) -> CandidateDomainRouteCover<3> {
        assert_eq!(value.lower[3], 0);
        assert_eq!(value.upper[3], Some(0));
        assert!(!value.source_sector[3]);
        assert!(!value.target_root[3]);
        CandidateDomainRouteCover {
            source_sector: value.source_sector[..3].try_into().unwrap(),
            target_root: value.target_root[..3].try_into().unwrap(),
            lower: value.lower[..3].try_into().unwrap(),
            upper: value.upper[..3].try_into().unwrap(),
            actual_rank: value.actual_rank,
            power_bounds: value.power_bounds,
            conservative: value.conservative,
        }
    }
    match event {
        CandidateDomainRouteEvent::Apply {
            owner_sector,
            cover: value,
        } => {
            assert!(!owner_sector[3]);
            CandidateDomainRouteEvent::Apply {
                owner_sector: owner_sector[..3].try_into().unwrap(),
                cover: cover(value),
            }
        }
        CandidateDomainRouteEvent::Route {
            sector,
            cover: value,
        } => {
            assert!(!sector[3]);
            CandidateDomainRouteEvent::Route {
                sector: sector[..3].try_into().unwrap(),
                cover: cover(value),
            }
        }
        event => panic!("unexpected routing event: {event:?}"),
    }
}

#[test]
fn route_capacity_padding_stays_fixed_for_literal_and_pinched_covers() {
    let exact = fixture();
    let padded = padded_fixture();
    for source in [SOURCE, TARGET] {
        let padded_source = [source[0], source[1], source[2], false];
        for rank in [Some(0), Some(2), None] {
            let (expected, _) = collect(&exact, source, rank);
            let (actual, _) = collect(&padded, padded_source, rank);
            assert_eq!(actual.into_iter().map(narrow).collect::<Vec<_>>(), expected);
        }
    }
}

#[test]
fn route_capacity_padding_stays_fixed_under_power_projection_and_joint_pruning() {
    let exact = fixture();
    let padded = padded_fixture();
    for source in [SOURCE, TARGET] {
        for joint_source_support_pruning in [false, true] {
            for power_bounds in [
                crate::solver::DomainPowerBounds::default(),
                crate::solver::DomainPowerBounds {
                    max_positive_power: Some(8),
                    min_power_difference: Some(-2),
                    max_power_difference: Some(7),
                },
            ] {
                let mut expected = Vec::new();
                let mut actual = Vec::new();
                let options = CandidateDomainRouteOptions {
                    joint_source_support_pruning,
                };
                exact
                    .visit_power_bounded_domain_route_overcover_with_options(
                        source,
                        &[0; 3],
                        &[None; 3],
                        Some(3),
                        power_bounds,
                        Default::default(),
                        options,
                        &AtomicBool::new(false),
                        |event| {
                            expected.push(event);
                            ControlFlow::Continue(())
                        },
                    )
                    .unwrap();
                padded
                    .visit_power_bounded_domain_route_overcover_with_options(
                        [source[0], source[1], source[2], false],
                        &[0; 4],
                        &[None, None, None, Some(0)],
                        Some(3),
                        power_bounds,
                        Default::default(),
                        options,
                        &AtomicBool::new(false),
                        |event| {
                            actual.push(narrow(event));
                            ControlFlow::Continue(())
                        },
                    )
                    .unwrap();
                assert_eq!(actual, expected);
            }
        }
    }
}

#[test]
fn route_capacity_padding_rejects_active_nonzero_or_unbounded_axes() {
    let padded = padded_fixture();
    for mutation in 0..4 {
        let mut source = [SOURCE[0], SOURCE[1], SOURCE[2], false];
        let mut lower = [0; 4];
        let mut upper = [None, None, None, Some(0)];
        match mutation {
            0 => source[3] = true,
            1 => lower[3] = 1,
            2 => upper[3] = Some(1),
            _ => upper[3] = None,
        }
        let mut emitted = false;
        let error = padded
            .visit_bounded_domain_route_overcover(
                source,
                &lower,
                &upper,
                Some(3),
                Default::default(),
                &AtomicBool::new(false),
                |_| {
                    emitted = true;
                    ControlFlow::Continue(())
                },
            )
            .unwrap_err();
        assert!(!emitted);
        assert!(matches!(
            error.failure,
            CandidateDomainRouteFailure::InvalidDomain(_)
        ));
    }
}
