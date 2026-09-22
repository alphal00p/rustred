use super::*;

fn positive_sum_fixture(include_inactive: bool) -> Arc<CandidateOwnerPrograms<3>> {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut sum = c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap();
    if include_inactive {
        sum = c.add(&sum, &c.index(2).unwrap()).unwrap();
    }
    let mut first = rule(0);
    first.equalities.push(poly(&c, sum));
    batch(&mut p).rules = vec![first, rule(1)];
    p
}

#[test]
fn affine_guard_box_original_positive_sum_skips_impossible_equality_without_refinement() {
    let p = positive_sum_fixture(false);
    let (stats, pieces) = refined(
        &p,
        [0; 3],
        [Some(2); 3],
        Some(2),
        OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 100,
            ..Default::default()
        },
    );
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (0, 0));
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].lower(), &[0; 3]);
    assert_eq!(pieces[0].upper(), &[Some(2); 3]);
    assert_eq!(pieces[0].max_numerator_rank(), Some(2));
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
    );
    for x in 0..=2 {
        for y in 0..=2 {
            for z in 0..=2 {
                assert_eq!(
                    at(&pieces, [x, y, z]),
                    concrete(
                        &p,
                        IntegralKey::try_new([x as i64 + 1, y as i64 + 1, -(z as i64)]).unwrap()
                    )
                );
            }
        }
    }
}

#[test]
fn affine_guard_box_original_rank_zero_sum_needs_no_singleton_and_keeps_positive_tails() {
    let p = positive_sum_fixture(true);
    let (stats, pieces) = refined(
        &p,
        [0; 3],
        [None; 3],
        Some(0),
        OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 1,
            ..Default::default()
        },
    );
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (0, 0));
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].lower(), &[0; 3]);
    assert_eq!(pieces[0].upper(), &[None; 3]);
    assert_eq!(pieces[0].max_numerator_rank(), Some(0));
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
    );
    for x in 0..=5 {
        for y in 0..=5 {
            assert_eq!(
                at(&pieces, [x, y, 0]),
                concrete(
                    &p,
                    IntegralKey::try_new([x as i64 + 1, y as i64 + 1, 0]).unwrap()
                )
            );
        }
    }
}

#[test]
fn affine_guard_box_keeps_first_rule_and_matches_concrete_application() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let positive = poly(
        &c,
        c.sub(
            &c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.integer(2),
        )
        .unwrap(),
    );
    let mut first = rule(0);
    first.exceptions.push(vec![positive]);
    batch(&mut p).rules = vec![first, rule(1)];
    let pieces = collect(&p, &[3, 1, 0], &[None, None, Some(0)], Some(0));
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].lower(), &[3, 1, 0]);
    assert_eq!(pieces[0].upper(), &[None, None, Some(0)]);
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    for x in 3..7 {
        for y in 1..5 {
            assert_eq!(
                at(&pieces, [x, y, 0]),
                concrete(
                    &p,
                    IntegralKey::try_new([x as i64 + 1, y as i64 + 1, 0]).unwrap()
                )
            );
        }
    }
    // The larger box includes the exceptional diagonal; a later stored rule
    // must not be selected merely because the earlier guard is unresolved.
    let unresolved = collect(&p, &[0; 3], &[None, None, Some(0)], Some(0));
    assert!(unresolved.iter().all(|p| matches!(
        p.disposition(),
        OwnerDomainMatchDisposition::Unresolved { .. }
    )));
}
