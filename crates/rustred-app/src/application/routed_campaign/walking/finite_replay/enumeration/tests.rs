use super::*;
use crate::application::routed_campaign::walking::queue::Phase;
use rustred::solver::DomainPowerBounds;
use std::collections::BTreeSet;

fn limits() -> OwnerDomainWalkFiniteReplayLimits {
    OwnerDomainWalkFiniteReplayLimits {
        max_nodes: 100_000,
        max_rule_applications: 100_000,
        max_transport_calls: 100_000,
        max_transport_operations: 1_000_000,
        max_transport_endpoints: 100_000,
        max_coalescing_additions: 100_000,
        max_positive_layers: 64,
        max_seed_points: 1024,
        max_seed_bytes: 1024 * 1024,
    }
}
fn domain<const N: usize>(owner: [bool; N], a: u64, r: u32, d: Option<i64>) -> Domain<N> {
    Domain {
        phase: Phase::Apply,
        owner,
        lower: vec![0; N],
        upper: vec![None; N],
        rank: Some(r),
        powers: DomainPowerBounds {
            max_positive_power: Some(a),
            min_power_difference: d,
            max_power_difference: d,
        },
    }
}
fn prepare_ok<const N: usize>(domain: &Domain<N>) -> Prepared<N> {
    prepare(
        domain,
        limits(),
        limits().max_nodes,
        &AtomicBool::new(false),
    )
    .unwrap_or_else(|e| panic!("{e:?}"))
}
fn failure<const N: usize>(domain: &Domain<N>, caps: OwnerDomainWalkFiniteReplayLimits) -> Failure {
    match prepare(domain, caps, caps.max_nodes, &AtomicBool::new(false)) {
        Ok(_) => panic!("unexpected complete seeds"),
        Err(e) => e,
    }
}
fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

#[test]
fn finite_replay_envelope_exact_original_260_points_and_independent_membership() {
    let owner = "111000100111001"
        .bytes()
        .map(|x| x == b'1')
        .collect::<Vec<_>>();
    let mut q = domain::<15>(owner.try_into().unwrap(), 10, 1, Some(9));
    q.upper = q
        .owner
        .iter()
        .map(|&active| Some(if active { 2 } else { 1 }))
        .collect();
    let prepared = prepare_ok(&q);
    assert_eq!(prepared.targets.len(), 260);
    assert_eq!(prepared.targets.iter().collect::<BTreeSet<_>>().len(), 260);
    assert!(prepared.work.exhausted);
    assert_eq!(prepared.work.count_calls, 1);
    assert_eq!(prepared.work.seed_results, 260);
    let envelope = FiniteEntryDomain::new(
        q.owner.to_vec(),
        EntryPowerBudget {
            max_positive_power: 10,
            max_numerator_rank: 1,
            min_power_difference: Some(9),
            max_power_difference: Some(9),
        },
    )
    .unwrap();
    assert!(prepared.targets.iter().all(|key| envelope.contains(key)));
    let mut ranks = [0; 2];
    for key in prepared.targets {
        let r: i64 = key.powers().iter().filter(|&&n| n <= 0).map(|&n| -n).sum();
        ranks[r as usize] += 1;
    }
    assert_eq!(ranks, [8, 252]);
}

#[test]
fn finite_replay_envelope_strict_band_zero_and_all_support() {
    let q = domain([true, false], 5, 4, Some(2));
    assert_eq!(
        prepare_ok(&q).targets,
        vec![key(&[2, 0]), key(&[3, -1]), key(&[4, -2]), key(&[5, -3])]
    );
    assert_eq!(
        prepare_ok(&domain([false, false], 0, 0, None)).targets,
        vec![key(&[0, 0])]
    );
    assert_eq!(
        prepare_ok(&domain([true, true], 2, 0, None)).targets,
        vec![key(&[1, 1])]
    );
    let empty = domain([true, true], 1, 0, None);
    assert!(matches!(
        failure(&empty, limits()).reason,
        Reason::Declined("empty_capped_domain")
    ));
}

#[test]
fn finite_replay_envelope_never_truncates_coordinate_caps_or_nonzero_lowers() {
    let mut q = domain([true, false], 5, 4, Some(2));
    q.upper[0] = Some(2);
    assert!(matches!(
        failure(&q, limits()).reason,
        Reason::Declined("unsupported_envelope_geometry")
    ));
    q.upper[0] = None;
    q.lower[0] = 1;
    assert!(matches!(
        failure(&q, limits()).reason,
        Reason::Declined("unsupported_envelope_geometry")
    ));
    // Retain the existing nonzero-lower singleton path even when it cannot
    // be described by the zero-lower envelope service.
    q.lower = vec![2, 1];
    q.upper = vec![Some(2), Some(1)];
    let one = prepare_ok(&q);
    assert_eq!(one.targets, vec![key(&[3, -1])]);
    assert_eq!(one.work.mode, "singleton");
    assert_eq!(one.work.count_calls, 0);
}

#[test]
fn finite_replay_envelope_unbounded_or_unrepresentable_declines_malformed_is_hard() {
    let mut q = domain([true, false], 5, 4, None);
    q.powers.max_positive_power = None;
    assert!(matches!(
        failure(&q, limits()).reason,
        Reason::Declined("unsupported_envelope_budget")
    ));
    q.powers.max_positive_power = Some(u64::from(u32::MAX) + 1);
    assert!(matches!(
        failure(&q, limits()).reason,
        Reason::Declined("unsupported_envelope_budget")
    ));
    q.powers.max_positive_power = Some(5);
    q.powers.min_power_difference = Some(i64::from(i32::MIN) - 1);
    assert!(matches!(
        failure(&q, limits()).reason,
        Reason::Declined("unsupported_envelope_budget")
    ));
    q.powers.min_power_difference = Some(4);
    q.powers.max_power_difference = Some(3);
    assert!(matches!(failure(&q, limits()).reason, Reason::Hard(_)));
    q.powers.min_power_difference = None;
    q.powers.max_power_difference = None;
    q.lower.pop();
    assert!(matches!(failure(&q, limits()).reason, Reason::Hard(_)));
}

#[test]
fn finite_replay_envelope_layer_seed_storage_and_native_caps_are_explicit() {
    let q = domain([true, false], 5, 4, Some(2));
    for (which, reason) in [
        (0, "enumeration_layer_budget"),
        (1, "seed_point_budget"),
        (2, "seed_byte_budget"),
        (3, "aggregate_budget"),
    ] {
        let mut caps = limits();
        match which {
            0 => caps.max_positive_layers = 0,
            1 => caps.max_seed_points = 0,
            2 => caps.max_seed_bytes = 0,
            _ => caps.max_nodes = 0,
        }
        let failed = failure(&q, caps);
        assert!(matches!(failed.reason, Reason::Declined(r) if r == reason));
        assert_eq!(failed.work.seed_results, 0);
        assert_eq!(failed.work.seed_buffer_bytes, 0);
        assert_eq!(failed.work.count_calls, 1);
    }
    let mut caps = limits();
    caps.max_seed_points = 4;
    caps.max_seed_bytes = buffer_bytes::<2>(4).unwrap();
    assert!(prepare(&q, caps, 4, &AtomicBool::new(false)).is_ok());
    caps.max_seed_bytes -= 1;
    assert!(matches!(
        failure(&q, caps).reason,
        Reason::Declined("seed_byte_budget")
    ));
    assert!(buffer_bytes::<2>(usize::MAX).is_none());
    assert!(buffer_bytes::<{ usize::MAX }>(1).is_none());
    let mut work = Work::default();
    assert!(matches!(
        admit_buffer::<2>(
            usize::MAX,
            OwnerDomainWalkFiniteReplayLimits {
                max_seed_points: usize::MAX,
                max_seed_bytes: usize::MAX,
                ..limits()
            },
            usize::MAX,
            &mut work
        )
        .unwrap_err()
        .reason,
        Reason::Hard(_)
    ));
}

#[test]
fn finite_replay_envelope_huge_exact_count_declines_before_machine_conversion() {
    let mut owner = [false; 15];
    owner[0] = true;
    let q = domain(owner, 1, u32::MAX, None);
    let caps = OwnerDomainWalkFiniteReplayLimits {
        max_nodes: usize::MAX,
        max_seed_points: usize::MAX,
        max_seed_bytes: usize::MAX,
        ..limits()
    };
    let failed = failure(&q, caps);
    assert!(matches!(
        failed.reason,
        Reason::Declined("aggregate_budget")
    ));
    assert_eq!(failed.work.count_calls, 1);
    assert_eq!(failed.work.seed_results, 0);
    assert_eq!(failed.work.seed_buffer_bytes, 0);
}

#[test]
fn finite_replay_envelope_cancel_before_after_count_and_during_enumeration() {
    let q = domain([true, false], 5, 4, Some(2));
    let cancel = AtomicBool::new(true);
    assert!(matches!(
        prepare(&q, limits(), 100, &cancel),
        Err(Failure {
            reason: Reason::Cancelled,
            ..
        })
    ));
    cancel.store(false, Ordering::Release);
    let envelope = FiniteEntryDomain::new(
        vec![true, false],
        EntryPowerBudget {
            max_positive_power: 5,
            max_numerator_rank: 4,
            min_power_difference: Some(2),
            max_power_difference: Some(2),
        },
    )
    .unwrap();
    let mut work = Work::default();
    assert!(matches!(
        count(&envelope, limits(), 100, &cancel, &mut work, || cancel
            .store(true, Ordering::Release)),
        Err(Failure {
            reason: Reason::Cancelled,
            ..
        })
    ));
    assert_eq!(work.count_calls, 1);
    cancel.store(false, Ordering::Release);
    let prepared = prepare_ok(&q);
    let mut seen = 0;
    let iterator = envelope.targets().inspect(|_| {
        seen += 1;
        if seen == 2 {
            cancel.store(true, Ordering::Release);
        }
    });
    let mut work = Work::default();
    assert!(matches!(
        collect(
            iterator,
            Vec::with_capacity(4),
            4,
            &prepared.admission,
            &cancel,
            &mut work
        ),
        Err(Failure {
            reason: Reason::Cancelled,
            ..
        })
    ));
    assert_eq!(work.seed_results, 2);
    assert_eq!(work.retained_seed_points, 1);
    assert!(!work.exhausted);
    // Cancellation also overrides an already computed resource decline.
    let failed = Failure {
        reason: Reason::Declined("seed_point_budget"),
        work,
    };
    assert!(matches!(
        failed.outcome(&cancel),
        Outcome::Failed {
            cancelled: true,
            ..
        }
    ));
}

#[test]
fn finite_replay_envelope_iterator_errors_prefixes_and_duplicates_never_certify() {
    let q = domain([true, false], 5, 4, Some(2));
    let prepared = prepare_ok(&q);
    let cancel = AtomicBool::new(false);
    for values in [
        vec![
            Ok(key(&[2, 0])),
            Err(AppError::limit("injected iterator failure")),
        ],
        vec![Ok(key(&[2, 0]))],
        vec![Ok(key(&[2, 0])); 5],
        vec![Ok(key(&[1, 0])); 4],
    ] {
        let mut work = Work::default();
        assert!(matches!(
            collect(
                values.into_iter(),
                Vec::with_capacity(4),
                4,
                &prepared.admission,
                &cancel,
                &mut work
            ),
            Err(Failure {
                reason: Reason::Hard(_),
                ..
            })
        ));
    }
    let duplicates = vec![Ok(key(&[2, 0])); 4];
    let mut work = Work::default();
    let targets = collect(
        duplicates.into_iter(),
        Vec::with_capacity(4),
        4,
        &prepared.admission,
        &cancel,
        &mut work,
    )
    .unwrap();
    let unique = targets.iter().collect::<BTreeSet<_>>().len();
    assert!(!complete_cardinality(targets.len(), unique, 4));
    assert!(!complete_cardinality(3, 3, 4));
    assert!(!complete_cardinality(5, 4, 4));
    assert!(complete_cardinality(4, 4, 4));
    assert!(!complete_cardinality(0, 0, 0));
}
