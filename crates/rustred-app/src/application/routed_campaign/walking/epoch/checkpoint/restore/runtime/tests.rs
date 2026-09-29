use super::super::super::{metadata::Inputs, tests::Directory};
use super::*;
use crate::application::routed_campaign::{
    matching::input::Query,
    walking::{
        epoch::{
            admit_initial,
            dispatch::Refill,
            job::{BreakReason, ErrorKind, JobResult, NativeKind},
            ledger6::Entry6,
            merge::{self, MergeConfig, RecordOut},
            records::Builder,
        },
        queue::{Domain, Phase},
    },
};
use crate::{
    CandidateOwnerBundle, FamilyCandidatesRequest, OwnerDomainMatchRequest,
    OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest, family_candidates,
    load_generated_candidate_owners,
};
use serde_json::{Value, json};
use std::fs;
use std::sync::Arc;

struct Fixture {
    directory: Directory,
    request: OwnerDomainWalkRequest,
    queries: Vec<Query>,
    owners: Vec<String>,
    reducer: RoutedCandidateReducer<1>,
}
impl Fixture {
    fn new() -> Self {
        let source = r#"
schema="rustred.project.toml.v1"
[family]
name="epoch_private_restore"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
        let mut request = FamilyCandidatesRequest::new(source);
        request.numerical_depth = 0;
        request.max_numerator_rank = Some(2);
        let generated = family_candidates(request).unwrap();
        let owner = rustred::sector::Mask::try_new([true]).unwrap();
        let owners = vec![blake3::hash(generated.bundle()).to_hex().to_string()];
        let (_, programs) = load_generated_candidate_owners::<1>(
            &[CandidateOwnerBundle {
                bytes: generated.bundle(),
                owner_sector: &owner,
            }],
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let reducer =
            RoutedCandidateReducer::try_new(Arc::new(programs), [], Default::default()).unwrap();
        let queries: Vec<_> = (0..3)
            .map(|id| Query {
                id: format!("q-{id}"),
                auxiliary: id == 1,
                role_declared: true,
                owner: vec![true],
                lower: vec![id],
                upper: vec![Some(id)],
                rank: Some(2),
                powers: Default::default(),
            })
            .collect();
        let document = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":queries.iter().map(|q| json!({
            "id":q.id,"owner":"1","lower":q.lower,"upper":q.upper,"max_numerator_rank":2})).collect::<Vec<_>>(),
            "query_roles":{"required":["q-0","q-2"],"auxiliary":["q-1"]}});
        let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
            "{}".into(),
            document.to_string(),
        ));
        request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
        Self {
            directory: Directory::new(),
            request,
            queries,
            owners,
            reducer,
        }
    }
    fn identity(&self) -> Identity<'_> {
        Identity::new(&self.request, &self.owners, &self.queries).unwrap()
    }
    fn save(&self, admitted: usize, reserved: usize) {
        self.save_order(admitted, reserved, false);
    }
    fn save_order(&self, admitted: usize, reserved: usize, reverse: bool) {
        let mut state = EpochState::<1>::new(
            self.request.max_domains,
            self.request.max_events,
            self.request.max_frontiers,
        );
        let mut rows = Vec::new();
        for query in &self.queries[..admitted] {
            let domain = Domain {
                phase: Phase::Apply,
                owner: [true],
                lower: query.lower.clone(),
                upper: query.upper.clone(),
                rank: query.rank,
                powers: query.powers,
            };
            let id = admit_initial(&mut state, &domain).unwrap();
            rows.push(json!({"id":query.id,"domain":id,"role":if query.auxiliary { "auxiliary" } else { "required" },"role_declared":true}));
        }
        state.p0 = state.watermark();
        state.tracker = Tracker::new(admitted);
        let mut dispatch = Dispatch::new();
        if reserved > 0 {
            assert!(matches!(
                dispatch.refill(&mut state, reserved),
                Refill::Jobs(_)
            ));
        }
        if reverse {
            // Valid saved ordering differing from the parent-ID index. Actual
            // deferred/requeue-first dispatch can have this same order.
            let first = state.in_flight[&0];
            let second = state.in_flight[&1];
            state.in_flight.insert(0, second);
            state.in_flight.insert(1, first);
        }
        let identity = self.identity();
        let inputs = Inputs {
            identity: &identity,
            admission: if admitted == 3 {
                Admission::Complete
            } else {
                Admission::InProgress
            },
            rows: &rows,
            frontiers: &[],
            stop: None,
        };
        let mut publisher = publication::Store::fresh(self.directory.0.clone()).unwrap();
        let mut sidecar = Sidecar::new(self.directory.0.clone(), 1);
        publisher
            .save(
                &MergeBoundary::borrow(&state, &dispatch, 16).unwrap(),
                &inputs,
                &mut sidecar,
            )
            .unwrap();
    }
    fn open(&self) -> io::Result<Restored<1>> {
        open(
            self.directory.0.clone(),
            &self.identity(),
            &self.reducer,
            16,
        )
    }
}

fn resave(fixture: &Fixture, restored: &mut Restored<1>) {
    let identity = fixture.identity();
    let inputs = Inputs {
        identity: &identity,
        admission: restored.admission,
        rows: &restored.roots.rows,
        frontiers: &restored.roots.frontiers,
        stop: (restored.state.counters.native_errors != 0).then_some(merge::StopReason::ErrorStop),
    };
    restored
        .publisher
        .save(
            &MergeBoundary::borrow(&restored.state, &restored.dispatch, 16).unwrap(),
            &inputs,
            &mut restored.records,
        )
        .unwrap();
}

#[test]
fn prepared_roots_restore_replay_resave_and_crash_session_roundtrip() {
    let fixture = Fixture::new();
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    assert!(matches!(restored.admission, Admission::Complete));
    assert_eq!(restored.roots.rows[1]["role"], "auxiliary");
    assert_eq!(
        restored
            .replay
            .iter()
            .map(|job| job.parent)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        restored
            .replay
            .iter()
            .map(|job| job.seq)
            .collect::<Vec<_>>(),
        [(2 << 40) | 1, (2 << 40) | 2]
    );
    assert!(
        matches!(
            restored.dispatch.refill(&mut restored.state, 16),
            Refill::Stalled
        ),
        "Pending cannot overtake replay"
    );
    assert!(fixture.open().is_err(), "store remains exclusively locked");
    resave(&fixture, &mut restored);
    drop(restored);
    let restored = fixture.open().unwrap();
    assert_eq!(restored.dispatch.checkpoint_snapshot().session, 3);
    assert_eq!(
        restored.state.ledger.get(0).unwrap(),
        Entry6::Reserved(Default::default())
    );
    drop(restored); // Crash before another checkpoint generation.
    let restored = fixture.open().unwrap();
    assert_eq!(restored.dispatch.checkpoint_snapshot().session, 4);
    assert_eq!(restored.state.k, 0);
    assert_eq!(
        restored.state.tracker.node_flags().collect::<Vec<_>>(),
        [0, 0, 0]
    );
}

#[test]
fn interrupted_input_admission_stays_a_prefix() {
    let fixture = Fixture::new();
    fixture.save(1, 0);
    let mut restored = fixture.open().unwrap();
    assert!(matches!(restored.admission, Admission::InProgress));
    assert_eq!(restored.roots.rows.len(), 1);
    assert_eq!(restored.state.p0, 1);
    assert!(restored.replay.is_empty());
    assert!(
        matches!(
            restored.dispatch.refill(&mut restored.state, 16),
            Refill::Stalled
        ),
        "a saved prefix cannot dispatch as a complete query set"
    );
}

#[test]
fn replay_preserves_saved_sequence_order_not_parent_order() {
    let fixture = Fixture::new();
    fixture.save_order(3, 2, true);
    let restored = fixture.open().unwrap();
    assert_eq!(
        restored
            .replay
            .iter()
            .map(|job| job.parent)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(restored.replay[0].seq, (2 << 40) | 1);
    assert_eq!(restored.replay[1].seq, (2 << 40) | 2);
}

#[test]
fn orphan_generations_are_skipped_and_corruption_falls_back_explicitly() {
    let fixture = Fixture::new();
    fixture.save(3, 0);
    let mut restored = fixture.open().unwrap();
    resave(&fixture, &mut restored); // latest2, previous1
    drop(restored);
    fs::write(
        fixture
            .directory
            .0
            .join("records-00000000000000000007.jsonl"),
        b"orphan",
    )
    .unwrap();
    fs::write(
        fixture
            .directory
            .0
            .join("epoch-internal-00000000000000000002-1.part"),
        b"broken",
    )
    .unwrap();
    let mut restored = fixture.open().unwrap();
    assert_eq!(restored.warnings.len(), 1);
    assert_eq!(restored.publisher.next_generation(), 8);
    resave(&fixture, &mut restored);
    let latest =
        publication::read_manifest(&fixture.directory.0.join(publication::LATEST)).unwrap();
    assert_eq!(latest.generation, 8);
}

#[test]
fn sticky_poison_refuses_even_valid_previous() {
    let fixture = Fixture::new();
    fixture.save(3, 0);
    fs::write(fixture.directory.0.join("epoch-internal-poison"), b"fatal").unwrap();
    assert!(fixture.open().is_err());
}

struct Output<'a>(&'a mut Sidecar);
impl RecordOut for Output<'_> {
    fn reserve(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn push(&mut self, row: Value) -> Result<(), String> {
        self.0.push(&row)
    }
}

#[test]
fn replayed_error_is_terminal_across_actual_merge_save_restore() {
    let fixture = Fixture::new();
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    let target = restored.state.store.domains[1];
    let results: Vec<_> = std::mem::take(&mut restored.replay)
        .into_iter()
        .map(|job| JobResult::<1> {
            seq: job.seq,
            parent: job.parent,
            v0: job.v0,
            kind: NativeKind::Apply,
            error_kind: if job.parent == 0 {
                ErrorKind::NativeFailure
            } else {
                ErrorKind::None
            },
            break_reason: BreakReason::None,
            panic: false,
            emitted: u64::from(job.parent == 0),
            accepted: u64::from(job.parent == 0),
            stats_events: u64::from(job.parent == 0),
            successors: u64::from(job.parent == 0),
            conditional: 0,
            known_reuse: 0,
            job_duplicates: 0,
            optional: [0; 3],
            route_masks: 0,
            route_joint_pruned: 0,
            seconds: 0.0,
            stats_json: format!("{{\"events\":{}}}", u64::from(job.parent == 0)).into_bytes(),
            error: (job.parent == 0).then(|| "deterministic failure".into()),
            frontiers: Vec::new(),
            refusals: Vec::new(),
            refusals_truncated: false,
            scope: None,
            g2: None,
            misses: if job.parent == 0 {
                vec![
                    crate::application::routed_campaign::walking::epoch::job::Miss {
                        ordinal: 0,
                        digest: target.digest().0,
                        image: target,
                    },
                ]
            } else {
                Vec::new()
            },
        })
        .collect();
    let config = MergeConfig {
        frontier_stop: true,
        lockstep: true,
        g2: false,
    };
    let checked = merge::p1_check(
        &mut restored.state,
        results.iter().map(JobResult::encode).collect(),
        config,
    )
    .unwrap();
    let plan = merge::p2(&mut restored.state, &checked).unwrap();
    let mut output = Output(&mut restored.records);
    merge::p3_preflight(&mut restored.state, &checked, &plan, &mut output).unwrap();
    let applied = merge::p3_apply(
        &mut restored.state,
        checked,
        plan,
        config,
        &Builder,
        &mut output,
        &mut |id, attempts| restored.dispatch.requeue(id, attempts),
    )
    .unwrap();
    assert_eq!(applied.stop, Some(merge::StopReason::ErrorStop));
    assert_eq!(restored.state.k, 1);
    assert_eq!(restored.state.edges.edges(), 1);
    restored
        .state
        .tracker
        .disable("test memory pressure released optional monitoring");
    resave(&fixture, &mut restored);
    drop(restored);
    let mut restored = fixture.open().unwrap();
    assert_eq!(restored.stop_reason.as_deref(), Some("error_stop"));
    assert!(restored.replay.is_empty());
    assert!(matches!(
        restored.state.ledger.get(0).unwrap(),
        Entry6::NativeError { .. }
    ));
    assert!(!restored.state.is_sealed(0));
    assert_eq!(
        restored.state.edges.edges(),
        1,
        "mathematical edges survive unavailable monitoring"
    );
    assert!(restored.state.tracker.node_flags().next().is_none());
    let Refill::Jobs(next) = restored.dispatch.refill(&mut restored.state, 16) else {
        panic!("remaining Pending work")
    };
    assert_eq!(next.iter().map(|job| job.parent).collect::<Vec<_>>(), [2]);
    resave(&fixture, &mut restored);
    drop(restored);
    assert_eq!(fixture.open().unwrap().replay[0].parent, 2);
}

#[test]
fn smaller_requested_limit_cannot_choose_an_older_valid_prefix() {
    let mut fixture = Fixture::new();
    fixture.save(1, 0);
    let mut restored = fixture.open().unwrap();
    for query in &fixture.queries[1..] {
        let domain = Domain {
            phase: Phase::Apply,
            owner: [true],
            lower: query.lower.clone(),
            upper: query.upper.clone(),
            rank: query.rank,
            powers: query.powers,
        };
        let id = admit_initial(&mut restored.state, &domain).unwrap();
        restored.roots.rows.push(json!({"id":query.id,"domain":id,"role":if query.auxiliary { "auxiliary" } else { "required" },"role_declared":true}));
    }
    restored.state.p0 = 3;
    restored.state.tracker = Tracker::new(3);
    restored.admission = Admission::Complete;
    resave(&fixture, &mut restored);
    drop(restored);
    fixture.request.max_domains = 1;
    let error = match fixture.open() {
        Ok(_) => panic!("must refuse changed limit"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn closure_counter_corruption_refuses_before_reserving_a_session() {
    let fixture = Fixture::new();
    fixture.save(3, 0);
    let path = fixture.directory.0.join(publication::LATEST);
    let mut manifest = publication::read_manifest(&path).unwrap();
    let meta = manifest
        .files
        .iter_mut()
        .find(|file| file.key == "meta")
        .unwrap();
    let meta_path = fixture.directory.0.join(&meta.file);
    let mut value: Value = serde_json::from_slice(&fs::read(&meta_path).unwrap()).unwrap();
    value["closure"]["total_closed"] = 1.into(); // Flags are still [0,0,0].
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(meta_path, &bytes).unwrap();
    meta.bytes = bytes.len() as u64;
    meta.blake3 = *blake3::hash(&bytes).as_bytes();
    let digest = *blake3::hash(&serde_json::to_vec(&manifest).unwrap()).as_bytes();
    fs::write(
        path,
        serde_json::to_vec(&json!({"manifest":manifest,"blake3":digest})).unwrap(),
    )
    .unwrap();
    assert!(fixture.open().is_err());
    assert_eq!(
        super::super::super::session::read(&fixture.directory.0).unwrap(),
        1
    );
}
