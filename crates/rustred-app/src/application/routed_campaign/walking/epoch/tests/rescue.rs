use super::*;

#[test]
fn quarantine_exact_groups_continue_to_oldest_admissible_equal_image() {
    let source = boxed([1, 1], [2, 2]);
    let mut state = state_with(&[source.clone()]);
    state.store.enable_rescue_duplicates().unwrap();
    state.store.install_quarantine(vec![1]).unwrap();
    let new = admit_initial(&mut state, &source).unwrap();
    assert_eq!(new, 1);
    assert_eq!(state.store.domains[0], state.store.domains[1]);
    assert_eq!(admit_initial(&mut state, &source).unwrap(), 1);
    state.store.install_quarantine(vec![0]).unwrap();
    assert_eq!(admit_initial(&mut state, &source).unwrap(), 0);
    assert_eq!(
        state.p0, 1,
        "rescue never extends the protected original prefix"
    );
}

#[test]
fn abandoned_result_is_explicit_uninspected_unsealed_and_does_not_frontier_stop() {
    let mut state = state_with(&[boxed([1, 1], [2, 2])]);
    state.rescue = Some(super::super::rescue::State {
        amendments: Vec::new(),
        abandoned: vec![1],
    });
    state.store.enable_rescue_duplicates().unwrap();
    state.store.install_quarantine(vec![1]).unwrap();
    let mut dispatch = Dispatch::new();
    let Refill::Jobs(jobs) = dispatch.refill(&mut state, 1) else {
        panic!("job")
    };
    assert_eq!(jobs[0].flags, super::super::job::JOB_RESCUE_ABANDONED);
    let mut r = result(&jobs[0], &[]);
    r.kind = NativeKind::Abandoned;
    r.emitted = 1;
    r.accepted = 1;
    r.stats_events = 1;
    r.stats_json = br#"{"events":1,"successors":0}"#.to_vec();
    r.frontiers.push(
        serde_json::to_vec(
            &serde_json::json!({"kind":super::super::super::inspection::RESCUE_ABANDONED_KIND}),
        )
        .unwrap(),
    );
    let mut rows = Rows(Vec::new());
    let config = MergeConfig {
        frontier_stop: true,
        lockstep: true,
        g2: false,
        finite_replay: None,
    };
    let applied = merge_cut_with(&mut state, &mut dispatch, &mut rows, vec![r], config).unwrap();
    assert_eq!(state.ledger.tag(0), Some(Tag::Abandoned));
    assert_eq!(state.nodes[0], 0);
    assert_eq!(state.counters.natives, 0);
    assert_eq!(state.counters.completed, 0);
    assert_eq!(state.counters.frontiers, 0);
    assert_eq!(
        state.edges.run_iter().collect::<Vec<_>>(),
        vec![(0, &[][..])]
    );
    assert_eq!(rows.0[0]["rescue_abandoned"], true);
    assert_eq!(rows.0[0]["local_inspection_finished"], false);
    assert!(applied.stop.is_none());
}

#[test]
fn rescue_bitset_codec_rejects_padding_and_abandonment_outside_quarantine() {
    let (_, receipt) = super::super::rescue::write(2, &[1], &[1], Vec::new()).unwrap();
    assert_eq!(receipt.bytes, 32);
    let (bytes, _) = super::super::rescue::write(2, &[1], &[1], Vec::new()).unwrap();
    assert_eq!(
        super::super::rescue::read(bytes.as_slice(), 2).unwrap(),
        (vec![1], vec![1])
    );
    let mut bad = bytes.clone();
    bad[24] = 2;
    assert!(super::super::rescue::read(bad.as_slice(), 2).is_err());
    let mut bad = bytes;
    bad[16] = 4;
    assert!(super::super::rescue::read(bad.as_slice(), 2).is_err());
}

#[test]
fn rescue_rebuild_excludes_quarantined_g2_but_keeps_historic_dependency_and_new_equal_lender() {
    let (mut state, mut dispatch, mut rows, job) = d_band_fixture(G2);
    merge_cut_with(
        &mut state,
        &mut dispatch,
        &mut rows,
        vec![g2_result(
            &job,
            2,
            &[(0, 1, 0)],
            vec![Piece::band(2, None, Some(5))],
        )],
        G2,
    )
    .unwrap();
    assert_eq!(run_of(&state, 3), [0]);
    let historic = state.edges.log().to_vec();
    let quarantine = state.tracker.tainted_with(&[1]).unwrap();
    assert!(
        super::super::rescue::contains(&quarantine, 3),
        "old borrower remains dependent on superseded lender"
    );
    state.store.enable_rescue_duplicates().unwrap();
    state.store.install_quarantine(quarantine).unwrap();
    super::super::g2::enable(&mut state);
    assert!(!state.merged_view.contains(0, state.k));
    assert!(!state.merged_view.contains(3, state.k));
    assert_eq!(state.edges.log(), historic);
    let old = state.store.domains[0].expand();
    assert_eq!(admit_initial(&mut state, &old).unwrap(), 5);
    state.tracker.discovered(state.store.len());
    assert_eq!(
        state.p0, 3,
        "existing descendants must not become protected initial inputs"
    );
    let batch = jobs(&mut state, &mut dispatch, 2);
    let replacement = batch.iter().find(|job| job.parent == 5).unwrap();
    let forged = g2_result(replacement, 2, &[(0, 1, 0)], Vec::new());
    assert!(merge::p1_check(&mut state, vec![forged.encode()], G2).is_err());
    let results = batch
        .iter()
        .map(|job| {
            let mut r = result(job, &[]);
            if job.parent == 4 {
                r.emitted = 1;
                r.accepted = 1;
                r.stats_events = 1;
                r.stats_json = br#"{"events":1,"successors":0}"#.to_vec();
                r.frontiers
                    .push(br#"{"kind":"test_unresolved_child"}"#.to_vec());
            }
            r
        })
        .collect();
    merge_cut_with(&mut state, &mut dispatch, &mut rows, results, G2).unwrap();
    assert!(
        !state.merged_view.contains(4, state.k),
        "own frontier cannot lend"
    );
    assert!(
        state.merged_view.contains(5, state.k),
        "new equal nonquarantined image can lend"
    );
    assert_eq!(
        run_of(&state, 3),
        [0],
        "rescue never rewrites an accepted G2 dependency"
    );
    let outcome = state.g2_store.as_ref().unwrap().plan_at(
        state.k + 1,
        &job.image.expand(),
        &AtomicBool::new(false),
    );
    let super::super::super::g2::Outcome::Planned(plan) = outcome else {
        panic!("replacement supplies residual cover")
    };
    assert!(plan.anchors.iter().any(|anchor| anchor.id == 5));
    assert!(
        plan.anchors
            .iter()
            .all(|anchor| anchor.id != 0 && anchor.id != 3 && anchor.id != 4)
    );
}
