use super::super::queue::Queue;
use super::*;

#[test]
fn constrained_domains_do_not_seed_unconstrained_orthant_shortcuts() {
    let mut source = domain(Phase::Apply, [true, false], Some(4));
    let initial =
        InitialOrthants::from_initial(&[Arc::new(source.clone())], &AtomicBool::new(false));
    assert!(initial.contains(Phase::Apply, &[true, false], Some(4)));
    for powers in [
        rustred::solver::DomainPowerBounds {
            max_positive_power: Some(10),
            ..Default::default()
        },
        rustred::solver::DomainPowerBounds {
            min_power_difference: Some(0),
            ..Default::default()
        },
        rustred::solver::DomainPowerBounds {
            max_power_difference: Some(3),
            ..Default::default()
        },
    ] {
        source.powers = powers;
        let initial =
            InitialOrthants::from_initial(&[Arc::new(source.clone())], &AtomicBool::new(false));
        assert!(!initial.contains(Phase::Apply, &[true, false], Some(0)));
    }
}

fn domain(phase: Phase, owner: [bool; 2], rank: Option<u32>) -> Domain<2> {
    Domain {
        powers: Default::default(),
        phase,
        owner,
        rank,
        lower: vec![0; 2],
        upper: vec![None; 2],
    }
}

#[test]
fn initial_orthants_authenticate_full_geometry_phase_owner_and_actual_rank() {
    let source = domain(Phase::Apply, [true, false], Some(11));
    let snapshot =
        InitialOrthants::from_initial(&[Arc::new(source.clone())], &AtomicBool::new(false));
    assert!(snapshot.contains(Phase::Apply, &[true, false], Some(11)));
    assert!(snapshot.contains(Phase::Apply, &[true, false], Some(0)));
    assert!(!snapshot.contains(Phase::Apply, &[true, false], Some(12)));
    assert!(!snapshot.contains(Phase::Apply, &[true, false], None));
    assert!(!snapshot.contains(Phase::Route, &[true, false], Some(11)));
    assert!(!snapshot.contains(Phase::Apply, &[false, true], Some(11)));
    for mutation in 0..4 {
        let mut invalid = source.clone();
        match mutation {
            0 => invalid.lower.pop().map(|_| ()).unwrap(),
            1 => invalid.upper.push(None),
            2 => invalid.lower[0] = 1,
            _ => invalid.upper[0] = Some(u64::MAX),
        }
        let snapshot = InitialOrthants::from_initial(&[Arc::new(invalid)], &AtomicBool::new(false));
        assert!(!snapshot.contains(Phase::Apply, &[true, false], Some(0)));
        assert!(snapshot.ranks.is_empty());
    }
}

#[test]
fn initial_orthants_none_dominates_but_finite_max_is_not_none() {
    let mut queue = Queue::new(10, None);
    queue
        .admit(domain(Phase::Apply, [true, false], Some(11)))
        .unwrap();
    queue
        .admit(domain(Phase::Apply, [true, false], Some(u32::MAX)))
        .unwrap();
    let finite = InitialOrthants::from_initial(&queue.domains, &AtomicBool::new(false));
    assert!(finite.contains(Phase::Apply, &[true, false], Some(u32::MAX)));
    assert!(!finite.contains(Phase::Apply, &[true, false], None));
    queue
        .admit(domain(Phase::Apply, [true, false], None))
        .unwrap();
    queue
        .admit(domain(Phase::Route, [true, false], Some(3)))
        .unwrap();
    let all = InitialOrthants::from_initial(&queue.domains, &AtomicBool::new(false));
    assert!(all.contains(Phase::Apply, &[true, false], None));
    assert!(all.contains(Phase::Route, &[true, false], Some(3)));
    assert!(!all.contains(Phase::Route, &[true, false], Some(4)));
    assert_eq!(all.ranks.len(), 2);
    assert!(!finite.contains(Phase::Apply, &[true, false], None)); // immutable
    assert_eq!(queue.next, 0); // every reused initial domain is still pending
}

#[test]
fn initial_orthants_optional_caps_cancel_and_later_admission_fall_back() {
    let mut queue = Queue::new(10, None);
    queue
        .admit(domain(Phase::Apply, [true, false], Some(11)))
        .unwrap();
    queue
        .admit(domain(Phase::Route, [true, false], Some(11)))
        .unwrap();
    let payload = size_of::<((Phase, [bool; 2]), Option<u32>)>();
    for (max_buckets, bytes, expected) in [
        (0, usize::MAX, 0),
        (10, 0, 0),
        (1, usize::MAX, 1),
        (10, payload, 1),
    ] {
        let snapshot = InitialOrthants::with_limits(
            &queue.domains,
            &AtomicBool::new(false),
            max_buckets,
            bytes,
        );
        assert_eq!(snapshot.ranks.len(), expected);
        assert_eq!(
            snapshot.contains(Phase::Apply, &[true, false], Some(11)),
            expected == 1
        );
        assert!(!snapshot.contains(Phase::Route, &[true, false], Some(11)));
        // A missed optional proof still succeeds through ordinary admission.
        assert!(
            !queue
                .admit(domain(Phase::Route, [true, false], Some(10)))
                .unwrap()
                .1
        );
    }
    let snapshot = InitialOrthants::from_initial(&queue.domains, &AtomicBool::new(false));
    queue
        .admit(domain(Phase::Apply, [false, true], None))
        .unwrap();
    assert!(!snapshot.contains(Phase::Apply, &[false, true], Some(0)));
    let cancelled = InitialOrthants::from_initial(&queue.domains, &AtomicBool::new(true));
    assert!(cancelled.ranks.is_empty());
    let fresh = InitialOrthants::<2>::empty();
    assert!(!fresh.contains(Phase::Apply, &[true, false], Some(0)));
    assert_eq!(queue.next, 0);
}
