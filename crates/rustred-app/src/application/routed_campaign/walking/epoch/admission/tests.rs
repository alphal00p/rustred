use super::super::admit_initial;
use super::*;

fn query(id: usize, lower: u64, upper: u64) -> matching::input::Query {
    matching::input::Query {
        id: format!("row-{id}"),
        auxiliary: id == 1,
        role_declared: true,
        owner: vec![true, false],
        lower: vec![lower, 0],
        upper: vec![Some(upper), Some(0)],
        rank: Some(100),
        powers: Default::default(),
    }
}

#[test]
fn shared_rows_match_legacy_success_and_all_phase_decisions() {
    for installed in [false, true] {
        for overcover in [false, true] {
            for conditions in [false, true] {
                assert_eq!(
                    phase(installed, overcover, conditions),
                    if installed || !overcover {
                        Some(Phase::Apply)
                    } else if conditions {
                        None
                    } else {
                        Some(Phase::Route)
                    }
                );
            }
        }
    }
    let phases = [
        Some(Phase::Apply),
        Some(Phase::Apply),
        Some(Phase::Apply),
        Some(Phase::Apply),
        Some(Phase::Route),
        None,
    ];
    let queries = [
        query(0, 2, 2),
        query(1, 2, 2),
        query(2, 1, 5),
        query(3, 3, 3),
        query(4, 3, 3),
        query(5, 7, 9),
    ];
    let mut current = EpochState::<2>::new(100, 100, 100);
    let mut old = EpochState::<2>::new(100, 100, 100);
    let (mut rows, mut frontiers, mut old_rows, mut old_frontiers) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (query, phase) in queries.iter().zip(phases) {
        one(&mut current, query, phase, &mut rows, &mut frontiers).unwrap();
        if let Some(phase) = phase {
            let domain = Domain {
                phase,
                owner: [true, false],
                lower: query.lower.clone(),
                upper: query.upper.clone(),
                rank: query.rank,
                powers: query.powers,
            };
            let id = admit_initial(&mut old, &domain).unwrap();
            old_rows.push(input_row(query, Some(id)));
        } else {
            old_frontiers.push(
                json!({"id":query.id,"kind":"initial_route_source_validity_obligation",
                "owner":"10","lower":query.lower,"upper":query.upper,"rank":query.rank,
                "power_bounds":power_bounds_json(query.powers),"reached_missing_rule_claim":false}),
            );
            let mut row = input_row(query, None);
            row["source_validity_unresolved"] = true.into();
            old_rows.push(row);
        }
        assert_eq!(rows, old_rows);
        assert_eq!(frontiers, old_frontiers);
        assert!(current.store.domains == old.store.domains);
        assert_eq!(current.ledger.words(), old.ledger.words());
        assert_eq!(current.live, old.live);
        assert_eq!(current.verify, old.verify);
        assert_eq!(
            serde_json::to_value(current.lookup).unwrap(),
            serde_json::to_value(old.lookup).unwrap()
        );
    }
    assert_eq!(rows.len(), 6);
    assert_eq!(current.store.len(), 3);
    assert_eq!(frontiers.len(), 1);
}

#[test]
fn row_reservation_failures_never_leave_an_unmapped_id_or_half_frontier() {
    for phase in [Some(Phase::Apply), None] {
        let query = query(0, 2, 2);
        let mut count = 0;
        one_with(
            &mut EpochState::<2>::new(100, 100, 100),
            &query,
            phase,
            &mut Vec::new(),
            &mut Vec::new(),
            || {
                count += 1;
                Ok(())
            },
        )
        .unwrap();
        for fail_at in 0..count {
            let mut state = EpochState::<2>::new(100, 100, 100);
            let (mut rows, mut frontiers, mut step) = (Vec::new(), Vec::new(), 0);
            let result = one_with(&mut state, &query, phase, &mut rows, &mut frontiers, || {
                let fail = step == fail_at;
                step += 1;
                if fail {
                    Err("injected row allocation")
                } else {
                    Ok(())
                }
            });
            assert!(matches!(result, Err(AdmissionError::Alloc(_))));
            assert!(rows.is_empty() && frontiers.is_empty());
            assert_eq!(state.watermark(), 0);
            assert!(state.ledger.words().is_empty() && state.live.is_empty());
            assert!(!state.poisoned);
        }
    }
    assert!(matches!(
        one(
            &mut EpochState::<2>::new(100, 100, 0),
            &query(0, 2, 2),
            None,
            &mut Vec::new(),
            &mut Vec::new()
        ),
        Err(AdmissionError::FrontierCap)
    ));
}
