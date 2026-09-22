use super::*;
use crate::solver::candidate_reduction::power_domain::DomainPowerBounds;

fn admitted<const N: usize>(
    owner: &[bool; N],
    lower: &[u64],
    upper: &[Option<u64>],
    rank: Option<u32>,
    bounds: DomainPowerBounds,
    n: [i64; N],
) -> bool {
    let mut a = 0i128;
    let mut r = 0i128;
    for i in 0..N {
        if (n[i] > 0) != owner[i] {
            return false;
        }
        let x = if owner[i] {
            n[i] as u64 - 1
        } else {
            n[i].unsigned_abs()
        };
        if x < lower[i] || upper[i].is_some_and(|u| x > u) {
            return false;
        }
        a += i128::from(n[i].max(0));
        r += -i128::from(n[i].min(0));
    }
    rank.is_none_or(|m| r <= i128::from(m))
        && bounds.max_positive_power.is_none_or(|m| a <= i128::from(m))
        && bounds
            .min_power_difference
            .is_none_or(|m| a - r >= i128::from(m))
        && bounds
            .max_power_difference
            .is_none_or(|m| a - r <= i128::from(m))
}

#[test]
fn sign_refined_power_images_match_exhaustive_integer_translation() {
    let cancel = AtomicBool::new(false);
    for mask in 0..4 {
        let owner = [mask & 1 != 0, mask & 2 != 0];
        for shift0 in -2..=2 {
            for shift1 in -2..=2 {
                let shift = [shift0, shift1];
                for rank in [None, Some(0), Some(2)] {
                    for powers in [
                        DomainPowerBounds::default(),
                        DomainPowerBounds {
                            max_positive_power: Some(4),
                            min_power_difference: Some(-1),
                            max_power_difference: Some(2),
                        },
                        DomainPowerBounds {
                            max_positive_power: Some(3),
                            min_power_difference: Some(1),
                            max_power_difference: None,
                        },
                    ] {
                        let mut budget = engine::Budget {
                            limits: Default::default(),
                            stats: Default::default(),
                            cancel: &cancel,
                        };
                        let source = geometry::copy_box(&[0; 2], &[Some(2); 2]).unwrap();
                        let mut images = Vec::new();
                        for sign in
                            geometry::sign_cells(&source, &owner, &shift, &mut budget).unwrap()
                        {
                            let Some((sign, sr)) =
                                geometry::normalize(sign, &owner, rank, powers, &mut budget)
                                    .unwrap()
                            else {
                                continue;
                            };
                            let mut boundaries =
                                geometry::Boundaries::new(&sign, &owner, &shift, &budget).unwrap();
                            while let Some(face) = boundaries.next(&mut budget).unwrap() {
                                let Some((face, fr)) =
                                    geometry::normalize(face, &owner, sr, powers, &mut budget)
                                        .unwrap()
                                else {
                                    continue;
                                };
                                let image = geometry::image(&face, &owner, &shift, fr).unwrap();
                                let dd = i128::from(shift0) + i128::from(shift1);
                                let translated =
                                    powers.shifted(image.delta_rank + dd, dd).unwrap().unwrap();
                                images.push((image, translated));
                            }
                        }
                        for n0 in -3..=4 {
                            for n1 in -3..=4 {
                                let n = [n0, n1];
                                let child = [n0 + shift0, n1 + shift1];
                                let hits = images
                                    .iter()
                                    .filter(|(image, p)| {
                                        admitted(
                                            &image.sector,
                                            &image.lower,
                                            &image.upper,
                                            image.rank,
                                            *p,
                                            child,
                                        )
                                    })
                                    .count();
                                assert_eq!(
                                    hits,
                                    usize::from(admitted(
                                        &owner,
                                        &[0; 2],
                                        &[Some(2); 2],
                                        rank,
                                        powers,
                                        n
                                    )),
                                    "mask={mask} n={n:?} shift={shift:?} powers={powers:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn actual_applied_pinch_retains_exact_constraints_and_allows_descendant_rank_growth() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-2, 0, 0], c.integer(1))])];
    let powers = DomainPowerBounds {
        max_positive_power: Some(5),
        min_power_difference: Some(1),
        max_power_difference: Some(4),
    };
    let mut count = 0;
    let mut rank_grew = false;
    let mut coverage = [0usize; 27];
    let stats = p
        .visit_power_bounded_owner_applied_successors(
            OWNER,
            &[0; 3],
            &[Some(2); 3],
            Some(1),
            powers,
            Default::default(),
            &AtomicBool::new(false),
            |event| {
                match event {
                    OwnerAppliedEvent::Successor(edge) => {
                        count += 1;
                        rank_grew |= edge.target_rank_limit.is_some_and(|r| r > 1);
                        assert_eq!(edge.source.power_bounds(), powers);
                        for x in 0..=2 {
                            for y in 0..=2 {
                                for z in 0..=2 {
                                    let n = [x + 1, y + 1, -z];
                                    let child = [x - 1, y + 1, -z];
                                    if admitted(
                                        edge.target_sector,
                                        edge.target_lower,
                                        edge.target_upper,
                                        edge.target_rank_limit,
                                        edge.target_power_bounds,
                                        child,
                                    ) {
                                        assert!(admitted(
                                            &OWNER,
                                            &[0; 3],
                                            &[Some(2); 3],
                                            Some(1),
                                            powers,
                                            n
                                        ));
                                        coverage[(x * 9 + y * 3 + z) as usize] += 1;
                                    }
                                }
                            }
                        }
                    }
                    OwnerAppliedEvent::Problem(problem) => panic!("unexpected {problem:?}"),
                    _ => {}
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert!(count > 0 && rank_grew);
    assert_eq!(stats.problems, 0);
    for x in 0..=2 {
        for y in 0..=2 {
            for z in 0..=2 {
                assert_eq!(
                    coverage[(x * 9 + y * 3 + z) as usize],
                    usize::from(admitted(
                        &OWNER,
                        &[0; 3],
                        &[Some(2); 3],
                        Some(1),
                        powers,
                        [x + 1, y + 1, -z]
                    ))
                );
            }
        }
    }
}

#[test]
fn default_power_api_preserves_unbounded_descriptors_and_counters() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-2, 0, -1], c.integer(1))])];
    let (old, old_stats) = collect(&p, [2, 0, 0], [None; 3], Some(10));
    let mut edges = Vec::new();
    let stats = p
        .visit_power_bounded_owner_applied_successors(
            OWNER,
            &[2, 0, 0],
            &[None; 3],
            Some(10),
            DomainPowerBounds::default(),
            Default::default(),
            &AtomicBool::new(false),
            |event| {
                if let OwnerAppliedEvent::Successor(e) = event {
                    assert_eq!(e.target_power_bounds, DomainPowerBounds::default());
                    edges.push((
                        e.target_lower.to_vec(),
                        e.target_upper.to_vec(),
                        e.target_rank_limit,
                    ));
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!(stats, old_stats);
    assert_eq!(
        edges,
        old.edges
            .iter()
            .map(|e| (e.lower.clone(), e.upper.clone(), e.rank))
            .collect::<Vec<_>>()
    );
}
