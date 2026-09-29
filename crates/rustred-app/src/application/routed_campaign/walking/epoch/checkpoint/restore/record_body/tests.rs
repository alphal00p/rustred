use super::*;
use crate::application::routed_campaign::walking::epoch::{
    checkpoint::tests::{Directory, state},
    dispatch::{Dispatch, Refill},
    job::{Job, Miss, Scope},
    ledger6::Counters,
    merge::{MergeConfig, RecordBuilder, RecordOut},
    records::Builder,
    state::EpochState,
};
use crate::application::routed_campaign::walking::execution::records::Sidecar;
use std::fs;

struct Rows(Vec<Value>);
impl RecordOut for Rows {
    fn reserve(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn push(&mut self, row: Value) -> Result<(), String> {
        self.0.push(row);
        Ok(())
    }
}

fn native(job: &Job<2>) -> JobResult<2> {
    JobResult {
        seq: job.seq,
        parent: job.parent,
        v0: job.v0,
        kind: NativeKind::Apply,
        error_kind: ErrorKind::None,
        break_reason: BreakReason::None,
        panic: false,
        emitted: 0,
        accepted: 0,
        stats_events: 0,
        successors: 0,
        conditional: 0,
        known_reuse: 0,
        job_duplicates: 0,
        optional: [0; 3],
        route_masks: 0,
        route_joint_pruned: 0,
        seconds: 0.0,
        stats_json: br#"{"events":0}"#.to_vec(),
        error: None,
        frontiers: Vec::new(),
        refusals: Vec::new(),
        refusals_truncated: false,
        scope: None,
        g2: None,
        misses: Vec::new(),
    }
}

fn merge_rows(
    state: &mut EpochState<2>,
    dispatch: &mut Dispatch,
    results: Vec<JobResult<2>>,
) -> Vec<Value> {
    let config = MergeConfig {
        frontier_stop: true,
        lockstep: true,
        g2: false,
    };
    let checked = merge::p1_check(
        state,
        results.iter().map(JobResult::encode).collect(),
        config,
    )
    .unwrap();
    assert!(checked.stop.is_none());
    let plan = merge::p2(state, &checked).unwrap();
    let mut rows = Rows(Vec::new());
    merge::p3_preflight(state, &checked, &plan, &mut rows).unwrap();
    merge::p3_apply(
        state,
        checked,
        plan,
        config,
        &Builder,
        &mut rows,
        &mut |id, attempts| dispatch.requeue(id, attempts),
    )
    .unwrap();
    rows.0
}

fn fixture() -> (EpochState<2>, Vec<Value>) {
    let mut state = state(3);
    state.counters.frontiers = 1; // A distinct initial source-refusal obligation.
    let mut dispatch = Dispatch::new();
    let Refill::Jobs(jobs) = dispatch.refill(&mut state, 3) else {
        panic!("jobs")
    };
    let mut results: Vec<_> = jobs.iter().map(native).collect();
    let frontier = &mut results[1];
    frontier.emitted = 1;
    frontier.accepted = 1;
    frontier.stats_events = 1;
    frontier.stats_json = br#"{"events":1}"#.to_vec();
    frontier
        .frontiers
        .push(br#"{"reason":"frontier"}"#.to_vec());
    let error = &mut results[2];
    error.emitted = 3;
    error.accepted = 2;
    error.stats_events = 3;
    error.successors = 1;
    error.stats_json = br#"{"events":3,"successors":2}"#.to_vec();
    error.error_kind = ErrorKind::ConsumerStop;
    error.break_reason = BreakReason::ResolverRange;
    error.error = Some("charged second successor refused".into());
    error
        .frontiers
        .push(br#"{"reason":"accepted prefix frontier"}"#.to_vec());
    let image = state.store.domains[0];
    error.misses.push(Miss {
        ordinal: 0,
        digest: image.digest().0,
        image,
    });
    let rows = merge_rows(&mut state, &mut dispatch, results);
    (state, rows)
}

fn view(state: &EpochState<2>, input_frontiers: usize) -> View<'_, 2> {
    View {
        store: &state.store,
        ledger: &state.ledger,
        edges: &state.edges,
        anchors: &state.anchors,
        frontier_counts: &state.frontier_counts,
        counters: &state.counters,
        k: state.k,
        input_frontiers,
    }
}

fn files(rows: &[Value]) -> (Directory, FileRef, Vec<Segment>) {
    let directory = Directory::new();
    let mut sidecar = Sidecar::new(directory.0.clone(), 1);
    for row in rows {
        sidecar.push(row).unwrap();
    }
    sidecar.seal(1, 2).unwrap();
    let segments = sidecar.closed().to_vec();
    let bytes = serde_json::to_vec(&segments).unwrap();
    fs::write(directory.0.join("registry"), &bytes).unwrap();
    let file = FileRef {
        key: "record-segments".into(),
        file: "registry".into(),
        count: segments.len() as u64,
        bytes: bytes.len() as u64,
        blake3: *blake3::hash(&bytes).as_bytes(),
    };
    (directory, file, segments)
}

fn check(state: &EpochState<2>, rows: &[Value], initial: usize) -> io::Result<Vec<Segment>> {
    let (directory, file, _) = files(rows);
    read(&directory.0, &file, 1, view(state, initial))
}

#[test]
fn actual_merge_and_sidecar_preserve_c2_prefix_and_accepted_counters() {
    let (state, rows) = fixture();
    assert_eq!(state.counters.frontiers, 3);
    assert_eq!(state.frontier_counts.values().copied().sum::<u32>(), 1);
    assert_eq!(state.counters.successors, 1);
    assert_eq!(rows[2]["stats"]["successors"], 2);
    let (directory, file, segments) = files(&rows);
    assert_eq!(
        read(&directory.0, &file, 1, view(&state, 1)).unwrap(),
        segments
    );
    assert!(check(&state, &rows, 0).is_err());
    assert!(check(&state, &rows, 2).is_err());
}

#[test]
fn rehashed_record_mutations_cannot_change_restore_authority() {
    let (state, original) = fixture();
    for mutation in 0..20 {
        let mut rows = original.clone();
        match mutation {
            0 => rows[0]["id"] = 2.into(),
            1 => rows[0]["lower"][0] = 9.into(),
            2 => rows[0]["phase"] = "Route".into(),
            3 => rows[0]["local_inspection_finished"] = false.into(),
            4 => rows[0]["local_classification_discharged"] = false.into(),
            5 => rows[0]["epoch"]["v0"] = 1.into(),
            6 => rows[0]["epoch"]["merge_epoch"] = 0.into(),
            7 => rows[0]["epoch"]["distinct_edge_count"] = 1.into(),
            8 => rows[0]["epoch"]["self_edge"] = true.into(),
            9 => rows[0]["epoch"]["class"] = "C2".into(),
            10 => rows[1]["frontiers"] = json!([]),
            11 => rows[2]["frontiers"] = json!([]),
            12 => rows[2]["error"] = Value::Null,
            13 => rows[2]["accepted_events"] = 3.into(),
            14 => rows[2]["epoch"]["err_class"] = "conversion".into(),
            15 => rows[2]["epoch"]["resolver_counters"]["successors"] = 2.into(),
            16 => rows[2]["epoch"]["resolver_counters"]["version"] = 2.into(),
            17 => rows[2]["epoch"]["resolver_counters"]["route_masks"] = 1.into(),
            18 => rows[0]["epoch"]["break_reason"] = "protocol".into(),
            19 => rows[0]["initial_overlap"] = json!({}),
            _ => unreachable!(),
        }
        assert!(check(&state, &rows, 1).is_err(), "mutation {mutation}");
    }
}

#[test]
fn diagnostics_are_not_subject_to_a_record_line_cap_but_authority_is_bounded() {
    let (state, mut rows) = fixture();
    rows[2]["error"] = "diagnostic".repeat(128 * 1024).into();
    rows[2]["optional_refusals"] = json!([{"detail":"x".repeat(1024 * 1024)}]);
    check(&state, &rows, 1).unwrap();
    rows[2]["epoch"]["error_kind"] = "x".repeat(8192).into();
    assert!(check(&state, &rows, 1).is_err());
}

#[test]
fn semantic_success_still_requires_the_same_readers_digest() {
    let (state, rows) = fixture();
    let (directory, file, segments) = files(&rows);
    let path = directory.0.join(&segments[0].file);
    let mut bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.last(), Some(&b'\n'));
    *bytes.last_mut().unwrap() = b' '; // Semantically identical, different hash.
    fs::write(path, bytes).unwrap();
    assert!(read(&directory.0, &file, 1, view(&state, 1)).is_err());
}

#[test]
fn recurring_panic_record_remains_terminal_without_native_stats() {
    let mut state = state(1);
    state.ledger = Ledger6::default();
    state
        .ledger
        .restore_word(
            Entry6::Pending(Counters {
                last_err: 4,
                attempts: 1,
                ..Default::default()
            })
            .encode(),
        )
        .unwrap();
    let mut dispatch = Dispatch::new();
    let Refill::Jobs(jobs) = dispatch.refill(&mut state, 1) else {
        panic!("jobs")
    };
    let mut result = native(&jobs[0]);
    result.error_kind = ErrorKind::Other;
    result.panic = true;
    result.stats_events = u64::MAX;
    result.stats_json.clear();
    result.error = Some("caught panic".into());
    let rows = merge_rows(&mut state, &mut dispatch, vec![result]);
    check(&state, &rows, 0).unwrap();
    assert!(matches!(
        state.ledger.get(0).unwrap(),
        Entry6::NativeError {
            err: err_class::RECURRING_PANIC,
            ..
        }
    ));
    let mut changed = rows;
    changed[0]["stats"] = json!({"events":0});
    assert!(check(&state, &changed, 0).is_err());
}

#[test]
fn alias_body_uses_canonical_geometry_and_no_native_aggregate() {
    let mut state = state(2);
    state.k = 1;
    // This isolates record authority; the cross-state pass separately checks
    // transfer containment, protected P0 and the forward-only target rule.
    state.ledger = Ledger6::default();
    state
        .ledger
        .restore_word(Entry6::Alias { to: 1 }.encode())
        .unwrap();
    state
        .ledger
        .restore_word(Entry6::Pending(Counters::default()).encode())
        .unwrap();
    state.edges.append_run(0, &[1], false).unwrap();
    let row = Builder.alias(0, &state.store.domains[0], 1, 1, false);
    check(&state, &[row.clone()], 0).unwrap();
    for key in [
        "representative_id",
        "containment_authority",
        "local_inspection_finished",
    ] {
        let mut changed = row.clone();
        changed[key] = Value::Null;
        assert!(check(&state, &[changed], 0).is_err(), "{key}");
    }
}

#[test]
fn route_records_require_route_scope_and_zero_apply_counters() {
    use crate::application::routed_campaign::walking::{
        descendant_closure::Tracker, queue::Domain,
    };
    let mut state = EpochState::new(100, usize::MAX, usize::MAX);
    let domain = Domain {
        phase: Phase::Route,
        owner: [true, false],
        lower: vec![0, 0],
        upper: vec![Some(0), Some(0)],
        rank: Some(100),
        powers: Default::default(),
    };
    crate::application::routed_campaign::walking::epoch::admit_initial(&mut state, &domain)
        .unwrap();
    state.p0 = 1;
    state.tracker = Tracker::new(1);
    let mut dispatch = Dispatch::new();
    let Refill::Jobs(jobs) = dispatch.refill(&mut state, 1) else {
        panic!("jobs")
    };
    let mut result = native(&jobs[0]);
    result.kind = NativeKind::Route;
    result.route_masks = 3;
    result.route_joint_pruned = 2;
    let rows = merge_rows(&mut state, &mut dispatch, vec![result]);
    check(&state, &rows, 0).unwrap();
    let mut changed = rows.clone();
    changed[0]["epoch"]["resolver_counters"]["optional"][0] = 1.into();
    assert!(check(&state, &changed, 0).is_err());
    let mut changed = rows;
    changed[0]["conservative_route_overcover"] = false.into();
    assert!(check(&state, &changed, 0).is_err());
}

#[test]
fn aggregate_overflow_and_record_inventory_mismatches_refuse() {
    let (state, rows) = fixture();
    let mut changed = rows.clone();
    changed[0]["epoch"]["known_reuse"] = u64::MAX.into();
    changed[1]["epoch"]["known_reuse"] = 1.into();
    assert!(check(&state, &changed, 1).is_err());
    assert!(check(&state, &rows[..2], 1).is_err());
    let mut changed = rows.clone();
    changed.push(rows[2].clone());
    assert!(check(&state, &changed, 1).is_err());
    let mut changed = rows;
    changed.swap(0, 1);
    assert!(check(&state, &changed, 1).is_err());
}

#[test]
fn real_initial_d_band_c0_and_c2_records_bind_anchor_and_residual_scope() {
    use crate::application::routed_campaign::walking::{
        descendant_closure::Tracker,
        epoch::admit_initial,
        queue::{CompactDomain, Domain},
    };
    use rustred::solver::DomainPowerBounds;
    for failed in [false, true] {
        let mut state = EpochState::new(100, usize::MAX, usize::MAX);
        let domain = |lower: [u64; 2], upper: [u64; 2], min_power_difference| Domain {
            phase: Phase::Apply,
            owner: [true, false],
            lower: lower.to_vec(),
            upper: upper.into_iter().map(Some).collect(),
            rank: Some(100),
            powers: DomainPowerBounds {
                min_power_difference,
                ..Default::default()
            },
        };
        admit_initial(&mut state, &domain([0, 0], [20, 20], Some(6))).unwrap();
        admit_initial(&mut state, &domain([30, 30], [30, 30], None)).unwrap();
        state.p0 = 2;
        state.tracker = Tracker::new(2);
        let mut dispatch = Dispatch::new();
        let Refill::Jobs(jobs) = dispatch.refill(&mut state, 2) else {
            panic!("jobs")
        };
        let mut results: Vec<_> = jobs.iter().map(native).collect();
        let image = CompactDomain::try_from_domain(&domain([0, 0], [12, 3], None)).unwrap();
        results[1].misses.push(Miss {
            ordinal: 0,
            digest: image.digest().0,
            image,
        });
        results[1].emitted = 1;
        results[1].accepted = 1;
        results[1].stats_events = 1;
        results[1].successors = 1;
        results[1].stats_json = br#"{"events":1,"successors":1}"#.to_vec();
        let mut rows = merge_rows(&mut state, &mut dispatch, results);
        assert_eq!(state.watermark(), 3);
        let Refill::Jobs(jobs) = dispatch.refill(&mut state, 1) else {
            panic!("partial job")
        };
        assert_eq!((jobs[0].parent, jobs[0].v0), (2, 1));
        let mut result = native(&jobs[0]);
        result.kind = NativeKind::ApplyPartial;
        result.scope = Some(Scope {
            anchor: 0,
            cut: 6,
            residual: DomainPowerBounds {
                max_power_difference: Some(5),
                ..Default::default()
            },
        });
        if failed {
            result.error_kind = ErrorKind::NativeFailure;
            result.error = Some("residual native failure".into());
        }
        rows.extend(merge_rows(&mut state, &mut dispatch, vec![result]));
        assert_eq!(rows[2]["record_kind"], "partial_initial_overlap_inspection");
        assert_eq!(rows[2]["epoch"]["class"], if failed { "C2" } else { "C0" });
        check(&state, &rows, 0).unwrap();
        for mutation in 0..7 {
            let mut changed = rows.clone();
            let row = &mut changed[2];
            match mutation {
                0 => row["initial_overlap"]["anchor_id"] = 1.into(),
                1 => row["initial_overlap"]["cut"] = 5.into(),
                2 => row["initial_overlap"]["residual_power_bounds"] = json!({}),
                3 => row["local_inspection_finished"] = true.into(),
                4 => row["residual_inspection_finished"] = failed.into(),
                5 => row["local_classification_discharged"] = true.into(),
                6 => row["native_inspection_scope"] = "full".into(),
                _ => unreachable!(),
            }
            assert!(
                check(&state, &changed, 0).is_err(),
                "failed={failed} mutation={mutation}"
            );
        }
        let mut anchor = state.anchors.get(2).unwrap().clone();
        assert_eq!(anchor.dispatch_version, 1);
        anchor.dispatch_version = 0; // Still < merge2, but not body v0=1.
        state.anchors = AnchorMap::default();
        state.anchors.push(anchor).unwrap();
        assert!(
            check(&state, &rows, 0).is_err(),
            "anchor version differs from authenticated body"
        );
    }
}
