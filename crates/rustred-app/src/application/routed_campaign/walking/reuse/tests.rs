use super::super::queue::Queue;
use super::*;

fn domain(x: u64) -> Domain<2> {
    Domain {
        phase: Phase::Apply,
        owner: [true, false],
        lower: vec![x, 0],
        upper: vec![Some(x + 2), None],
        rank: Some(11),
    }
}
fn event(d: Domain<2>, conditional: bool) -> Event<2> {
    Event::one(Effect::Admit {
        domain: d,
        successor: true,
        conditional,
    })
}
#[test]
fn job_local_reuse_seeds_only_accepted_full_keys_and_current_flags() {
    let mut cache = Cache::new(true);
    assert!(
        cache
            .forward(event(domain(1), false), &mut |_| ControlFlow::Break(()))
            .is_break()
    );
    assert!(cache.keys.is_empty());
    let mut kinds = Vec::new();
    let mut emit = |e: Event<2>| {
        kinds.push(match e.effect {
            Effect::Admit { conditional, .. } => (false, conditional),
            Effect::KnownReuse { conditional, .. } => (true, conditional),
            _ => panic!("unexpected"),
        });
        ControlFlow::Continue(())
    };
    assert!(
        cache
            .forward(event(domain(1), false), &mut emit)
            .is_continue()
    );
    assert!(
        cache
            .forward(event(domain(1), true), &mut emit)
            .is_continue()
    );
    for changed in 0..5 {
        let mut d = domain(1);
        match changed {
            0 => d.phase = Phase::Route,
            1 => d.owner = [false, true],
            2 => d.rank = None,
            3 => d.upper[0] = None,
            _ => d.upper[0] = Some(u64::MAX),
        }
        assert!(cache.forward(event(d, false), &mut emit).is_continue());
    }
    assert_eq!(kinds[0..2], [(false, false), (true, true)]);
    assert!(kinds[2..].iter().all(|(hit, _)| !hit));
    assert_eq!(cache.keys.len(), 6);
}

#[test]
fn job_local_reuse_capacity_bytes_disable_and_fresh_job_fall_back() {
    for (enabled, max_keys, max_bytes) in [
        (false, 0, MAX_KEY_BYTES),
        (true, 1, MAX_KEY_BYTES),
        (true, MAX_KEYS, 0),
    ] {
        let mut cache = Cache::<2>::new(enabled);
        cache.max_keys = max_keys;
        cache.max_bytes = max_bytes;
        let mut hits = 0;
        for x in [0, 1, 1, 0] {
            let accepted = cache.forward(event(domain(x), false), &mut |e| {
                hits += usize::from(matches!(e.effect, Effect::KnownReuse { .. }));
                ControlFlow::Continue(())
            });
            assert!(accepted.is_continue());
        }
        assert_eq!(
            hits,
            usize::from(enabled && max_keys == 1 && max_bytes != 0)
        );
        assert!(cache.keys.len() <= max_keys);
        assert!(cache.keys.len() * size_of::<Key<2>>() <= max_bytes);
    }
    let mut fresh = Cache::new(true);
    let accepted = fresh.forward(event(domain(0), false), &mut |e| {
        assert!(matches!(e.effect, Effect::Admit { .. }));
        ControlFlow::Continue(())
    });
    assert!(accepted.is_continue());
}

#[test]
fn job_local_reuse_fifo_graph_matches_cache_off_after_general_containment() {
    let run = |enabled| {
        let mut queue = Queue::<2>::new(100, usize::MAX);
        let mut cache = Cache::new(enabled);
        let mut hits = 0;
        let mut callbacks = 0;
        for x in [0, 1, 1, 8, 9, 9, 2, 2, 0, 11, 12, 12] {
            let mut d = domain(x);
            d.upper[0] = Some((x / 8) * 8 + 7);
            let accepted = cache.forward(event(d, callbacks % 2 == 0), &mut |e| {
                callbacks += 1;
                match e.effect {
                    Effect::Admit { domain, .. } => {
                        queue.admit(domain).unwrap();
                    }
                    Effect::KnownReuse { .. } => {
                        queue.count_known_reuse(1).unwrap();
                        hits += 1;
                    }
                    _ => panic!("unexpected"),
                }
                ControlFlow::Continue(())
            });
            assert!(accepted.is_continue());
        }
        (queue, hits, callbacks)
    };
    let (plain, no_hits, plain_callbacks) = run(false);
    let (cached, hits, cached_callbacks) = run(true);
    assert_eq!(plain.domains, cached.domains);
    assert_eq!(plain.deduplicated, cached.deduplicated);
    assert_eq!(plain_callbacks, cached_callbacks);
    assert_eq!(no_hits, 0);
    assert!(hits > 0 && cached.containment_checks < plain.containment_checks);
    assert_eq!(cached.next, 0); // every containing domain is still pending
}

#[test]
fn job_local_reuse_uncommitted_first_admission_cannot_leave_marker_only_prefix() {
    let mut cache = Cache::new(true);
    let mut stream = Vec::new();
    for _ in 0..3 {
        let accepted = cache.forward(event(domain(1), false), &mut |e| {
            stream.push(e);
            ControlFlow::Continue(())
        });
        assert!(accepted.is_continue());
    }
    assert!(matches!(stream[0].effect, Effect::Admit { .. }));
    assert!(matches!(stream[1].effect, Effect::KnownReuse { .. }));
    let mut queue = Queue::<2>::new(0, 0);
    let mut committed_markers = 0;
    for e in stream {
        match e.effect {
            Effect::Admit { domain, .. } => {
                if queue.admit(domain).is_err() {
                    break;
                }
            }
            Effect::KnownReuse { .. } => committed_markers += 1,
            _ => unreachable!(),
        }
    }
    assert_eq!(committed_markers, 0);
    assert!(queue.domains.is_empty());
}

#[test]
fn job_local_reuse_after_general_comparison_cap_is_pending_not_completed() {
    let mut queue = Queue::<2>::new(3, 1);
    let mut container = domain(0);
    container.upper[0] = Some(100);
    queue.admit(container).unwrap();
    let mut cache = Cache::new(true);
    for _ in 0..3 {
        assert!(
            cache
                .forward(event(domain(1), false), &mut |e| {
                    match e.effect {
                        Effect::Admit { domain, .. } => {
                            queue.admit(domain).unwrap();
                        }
                        Effect::KnownReuse { .. } => queue.count_known_reuse(1).unwrap(),
                        _ => panic!("unexpected"),
                    }
                    ControlFlow::Continue(())
                })
                .is_continue()
        );
    }
    assert_eq!(queue.containment_checks, 1);
    assert_eq!(queue.deduplicated, 3);
    assert_eq!(queue.next, 0);
    assert!(queue.admit(domain(101)).is_err());
}

#[test]
fn job_local_reuse_native_inspection_cache_off_matches_saved_k1() {
    use super::super::{OwnerDomainMatchRequest, OwnerDomainWalkRequest, inspection, stats_json};
    use crate::{
        CandidateOwnerBundle, FamilyCandidatesRequest, family_candidates,
        load_generated_candidate_owners,
    };
    use std::sync::{Arc, atomic::AtomicBool};
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="job_local_reuse_native_fixture"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
    let mut generation = FamilyCandidatesRequest::new(source);
    generation.numerical_depth = 0;
    generation.max_numerator_rank = Some(2);
    let saved = family_candidates(generation).unwrap();
    let owner_sector = rustred::sector::Mask::try_new([true]).unwrap();
    let (_, programs) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes: saved.bundle(),
            owner_sector: &owner_sector,
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let reducer = rustred::solver::RoutedCandidateReducer::try_new(
        Arc::new(programs),
        [],
        Default::default(),
    )
    .unwrap();
    let request =
        OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
    let source = Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![0],
        upper: vec![None],
        rank: Some(11),
    };
    let run = |enabled| {
        let mut queue = Queue::new(100, usize::MAX);
        queue.admit(source.clone()).unwrap();
        let mut logical = 0;
        let mut successors = 0;
        let mut conditional = 0;
        let mut frontiers = Vec::new();
        let finished = inspection::inspect_with_reuse(
            &reducer,
            &source,
            &request,
            &AtomicBool::new(false),
            enabled,
            &mut |e| {
                logical += e.count;
                match e.effect {
                    Effect::Admit {
                        domain,
                        successor,
                        conditional: condition,
                    } => {
                        queue.admit(domain).unwrap();
                        successors += usize::from(successor);
                        conditional += usize::from(condition);
                    }
                    Effect::KnownReuse {
                        successor,
                        conditional: condition,
                    } => {
                        queue.count_known_reuse(e.count).unwrap();
                        successors += e.count * usize::from(successor);
                        conditional += e.count * usize::from(condition);
                    }
                    Effect::Frontier { value, .. } => frontiers.push(value),
                    Effect::Count => {}
                    Effect::Optional(_) => {
                        panic!("tiny native fixture must not need optional fallback")
                    }
                }
                ControlFlow::Continue(())
            },
        );
        assert!(finished.error.is_none(), "{:?}", finished.error);
        let inspection::NativeStats::Apply(stats) = finished.stats else {
            panic!("Apply fixture");
        };
        (
            queue.domains,
            queue.deduplicated,
            logical,
            successors,
            conditional,
            frontiers,
            stats_json(stats),
        )
    };
    let plain = run(false);
    let cached = run(true);
    assert_eq!(plain, cached);
    assert!(cached.3 > 0);
    assert!(cached.5.is_empty());
}
