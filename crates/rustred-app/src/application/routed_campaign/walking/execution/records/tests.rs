use super::super::super::{
    checkpoint::{
        self, Store,
        manifest::Section,
        test_support::{Fixture, OWNER, test_directory},
    },
    delegation::{Ledger, SchedulingPolicy},
    queue::{Domain, Phase, Queue},
};
use super::super::{Effect, Event, Finished, NativeStats, OwnerDomainWalkRequest, State, Ticket};
use super::*;
use std::num::NonZeroUsize;
use std::sync::atomic::AtomicBool;

fn record(id: usize) -> Value {
    json!({"id":id,"escaped":"a\nb","frontiers":[]})
}
fn jsonl(records: &[Value]) -> Vec<u8> {
    let mut out = Vec::new();
    for record in records {
        serde_json::to_writer(&mut out, record).unwrap();
        out.push(b'\n');
    }
    out
}
fn read_all(files: &SidecarFiles) -> Vec<Value> {
    let mut out = Vec::new();
    files
        .for_each_record(|record| {
            out.push(record);
            Ok(())
        })
        .unwrap();
    out
}
fn request() -> OwnerDomainWalkRequest {
    OwnerDomainWalkRequest::new(crate::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ))
}
fn domain(lower: u64, upper: Option<u64>) -> Domain<1> {
    Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![lower],
        upper: vec![upper],
        rank: Some(11),
        powers: Default::default(),
    }
}
fn finished() -> Finished {
    Finished {
        stats: NativeStats::Apply(Default::default()),
        error: None,
        error_kind: "native",
        seconds: 0.0,
    }
}
/// Native 0, alias 1 -> 2 (the containing ray), representative 2 pending.
fn aliased() -> State<1> {
    let mut queue = Queue::with_policy(
        100,
        None,
        SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(1).unwrap(),
        },
    )
    .unwrap();
    queue.admit(domain(2, Some(2))).unwrap();
    queue.admit(domain(3, Some(3))).unwrap();
    queue.admit(domain(0, None)).unwrap();
    let mut state = State::new(queue, 0, None);
    state.note_native_started(0).unwrap();
    state.commit(0, finished());
    state.commit_delegated().unwrap();
    assert_eq!(state.queue.next, 2);
    state
}
fn sidecar_state(dir: &Path, generation: u64) -> State<1> {
    let state = aliased();
    let rows = state.records.borrow_mut().take_memory();
    let mut sidecar = Sidecar::new(dir.to_path_buf(), generation);
    for row in &rows {
        sidecar.push(row).unwrap();
    }
    *state.records.borrow_mut() = RecordSink::Sidecar(sidecar);
    state
}

#[test]
fn records_sidecar_seals_per_generation_and_ignores_unreferenced_tail() {
    let dir = test_directory();
    let mut sidecar = Sidecar::new(dir.clone(), 2);
    for id in 0..5 {
        sidecar.push(&record(id)).unwrap();
    }
    assert_eq!(sidecar.total(), 5);
    assert!(
        sidecar.seal(3, 4).is_err(),
        "sealed under the wrong generation"
    );
    let segment = sidecar.seal(2, 3).unwrap().unwrap();
    assert_eq!(
        (segment.generation, segment.first, segment.count),
        (2, 0, 5)
    );
    assert_eq!(segment.file, Section::Records.file_name(2));
    // Exactly the bytes and digest of the CP5 records codec.
    let expected = jsonl(&(0..5).map(record).collect::<Vec<_>>());
    assert_eq!(fs::read(dir.join(&segment.file)).unwrap(), expected);
    assert_eq!(segment.bytes, expected.len() as u64);
    assert_eq!(segment.blake3, blake3::hash(&expected).to_hex().to_string());
    // Nothing committed: no segment, the reservation still moves on.
    let mut idle = Sidecar::new(dir.clone(), 7);
    assert_eq!(idle.seal(7, 8).unwrap(), None);
    assert_eq!(idle.generation(), 8);
    assert!(!dir.join(Section::Records.file_name(7)).exists());
    for id in 5..8 {
        sidecar.push(&record(id)).unwrap();
    }
    assert_eq!(sidecar.total(), 8);
    assert_eq!(read_all(&sidecar.files()).len(), 8); // The tail is readable.
    drop(sidecar); // Crash before the save of generation 3.
    assert!(dir.join(Section::Records.file_name(3)).exists());
    // Resume from a manifest listing only segment 2: the orphan keeps its
    // generation to itself and is never read.
    let reserved = checkpoint::next_free_generation(&dir, 2).unwrap();
    assert_eq!(reserved, 4);
    let reopened = Sidecar::restored(dir.clone(), vec![segment.clone()], reserved);
    assert_eq!(reopened.total(), 5);
    assert_eq!(
        read_all(&reopened.files()),
        (0..5).map(record).collect::<Vec<_>>()
    );
    // A sealed segment is re-verified while it is read back.
    let mut corrupt = segment;
    corrupt.blake3 = "0".repeat(64);
    let error = Sidecar::restored(dir.clone(), vec![corrupt], reserved)
        .files()
        .for_each_line(|_| Ok(()))
        .unwrap_err();
    assert!(error.contains("checksum or length"), "{error}");
    fs::remove_dir_all(dir).unwrap();
}

/// A sidecar whose directory is a regular file: creating its first segment
/// fails with ENOTDIR, which no privilege bypasses.
fn blocked_sidecar(dir: &Path) -> Sidecar {
    let blocker = dir.join("blocker");
    File::create(&blocker).unwrap();
    Sidecar::new(blocker, 2)
}
/// The failed prefix is never checkpointed.
fn assert_save_refused(state: &State<1>) {
    let fixture = Fixture::save(&State::<1>::new(Queue::new(8, None), 0, None));
    let mut store = fixture.open(true).unwrap();
    store.bind_owners(vec![OWNER.into()]).unwrap();
    let error = store.save(state, &[], &[], true, &|_| {}).unwrap_err();
    assert!(error.contains("failed publication prefix"), "{error}");
}

#[test]
fn sidecar_write_failure_sets_publisher_error_not_a_fake_record() {
    let dir = test_directory();
    let mut queue = Queue::<1>::new(8, None);
    queue.admit(domain(0, Some(0))).unwrap();
    let mut state = State::new(queue, 0, None);
    *state.records.get_mut() = RecordSink::Sidecar(blocked_sidecar(&dir));
    state.commit(0, finished());
    let error = state.error.clone().unwrap();
    assert!(
        error.contains("cannot create record sidecar segment"),
        "{error}"
    );
    assert_eq!(state.records.borrow().total(), 0); // No placeholder record.
    // The sidecar stays failed even once its directory is usable: the error
    // is the stored one, and no segment is created.
    fs::remove_file(dir.join("blocker")).unwrap();
    fs::create_dir(dir.join("blocker")).unwrap();
    assert_eq!(state.records.get_mut().push(record(1)).unwrap_err(), error);
    assert!(
        !dir.join("blocker")
            .join(Section::Records.file_name(2))
            .exists()
    );
    assert_save_refused(&state);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn records_reach_the_segment_in_batches_and_stay_readable_before() {
    let dir = test_directory();
    let mut sidecar = Sidecar::new(dir.clone(), 2);
    let file = dir.join(Section::Records.file_name(2));
    sidecar.push(&record(0)).unwrap();
    // Created by the first push, written only once the batch is full.
    assert_eq!(fs::metadata(&file).unwrap().len(), 0);
    assert_eq!(read_all(&sidecar.files()), vec![record(0)]);
    let large = json!({"id":1,"pad":"x".repeat(BATCH_BYTES)});
    sidecar.push(&large).unwrap();
    assert_eq!(fs::read(&file).unwrap(), jsonl(&[record(0), large.clone()]));
    sidecar.push(&record(2)).unwrap();
    assert_eq!(sidecar.total(), 3);
    let all = vec![record(0), large, record(2)];
    assert_eq!(read_all(&sidecar.files()), all);
    // The save writes the rest before sealing.
    let segment = sidecar.seal(2, 3).unwrap().unwrap();
    assert_eq!(segment.count, 3);
    assert_eq!(fs::read(&file).unwrap(), jsonl(&all));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sidecar_write_failure_keeps_the_batch_readable_for_the_failed_report() {
    let dir = test_directory();
    let mut sidecar = Sidecar::new(dir.clone(), 2);
    sidecar.push(&record(0)).unwrap();
    sidecar.write_batch().unwrap();
    // A read-only handle in place of the segment's writer: the next write
    // fails with EBADF, which no privilege bypasses.
    let read_only = File::open(dir.join(Section::Records.file_name(2))).unwrap();
    sidecar.open.as_mut().unwrap().writer = HashingWriter::new(read_only);
    sidecar.push(&record(1)).unwrap(); // Batched, not yet written.
    let error = sidecar.seal(2, 3).unwrap_err();
    assert!(error.contains("record sidecar write failed"), "{error}");
    // Record 0 from the file, record 1 from the unwritten batch: every
    // committed record for the failed run's report.
    assert_eq!(sidecar.total(), 2);
    assert_eq!(read_all(&sidecar.files()), vec![record(0), record(1)]);
    assert_eq!(sidecar.push(&record(2)).unwrap_err(), error);
    assert_eq!(sidecar.seal(2, 3).unwrap_err(), error);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_seal_keeps_the_open_tail_readable_for_the_failed_report() {
    let dir = test_directory();
    let mut sidecar = Sidecar::new(dir.clone(), 2);
    for id in 0..3 {
        sidecar.push(&record(id)).unwrap();
    }
    sidecar.seal(2, 3).unwrap().unwrap();
    for id in 3..5 {
        sidecar.push(&record(id)).unwrap();
    }
    // The segment's fsync succeeds, the directory sync cannot open the moved
    // directory (ENOENT, which no privilege bypasses).
    let moved = dir.with_extension("moved");
    fs::rename(&dir, &moved).unwrap();
    let error = sidecar.seal(3, 4).unwrap_err();
    fs::rename(&moved, &dir).unwrap();
    assert!(
        error.contains("cannot sync checkpoint directory"),
        "{error}"
    );
    // Sealed segment 2 plus the unsealed tail: every committed record, in
    // order, for the failed run's result.json.
    assert_eq!(sidecar.total(), 5);
    assert_eq!(sidecar.closed().len(), 1);
    assert_eq!(sidecar.generation(), 3);
    assert_eq!(sidecar.seal(3, 4).unwrap_err(), error);
    let streamed = Streamed::new(sidecar, Annotations::default());
    assert_eq!(streamed.total(), 5);
    let ids: Vec<_> = streamed
        .collect()
        .unwrap()
        .iter()
        .map(|record| record["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, [0, 1, 2, 3, 4]);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn delegated_record_write_failure_keeps_the_cursor_with_the_ledger() {
    let dir = test_directory();
    let mut queue = Queue::with_policy(
        100,
        None,
        SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(1).unwrap(),
        },
    )
    .unwrap();
    queue.admit(domain(2, Some(2))).unwrap();
    queue.admit(domain(3, Some(3))).unwrap();
    queue.admit(domain(0, None)).unwrap();
    let mut state = State::new(queue, 0, None);
    state.note_native_started(0).unwrap();
    state.commit(0, finished());
    *state.records.get_mut() = RecordSink::Sidecar(blocked_sidecar(&dir));
    let error = state.commit_delegated().unwrap_err();
    assert!(
        error.contains("cannot create record sidecar segment"),
        "{error}"
    );
    // The alias is published in the ledger, its record is not written, and
    // the publisher's cursor follows the ledger.
    assert_eq!(state.records.borrow().total(), 0);
    let ledger = state.queue.delegation.as_ref().unwrap();
    assert_eq!(ledger.published_count(), 2);
    assert_eq!(state.queue.next, ledger.cursor());
    // As the callers do: the write error is the run's error, and the final
    // resolution reports no secondary cursor mismatch.
    state.error = Some(error.clone());
    assert_ne!(
        state.finalize_delegation().0.unwrap()["error"],
        "delegation final cursor mismatch"
    );
    assert_eq!(state.error.as_deref(), Some(error.as_str()));
    assert_save_refused(&state);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn finalization_annotations_from_ledger_and_closure_match_previous_record_mutation() {
    let dir = test_directory();
    for sidecar in [false, true] {
        let mut state = if sidecar {
            sidecar_state(&dir, 2)
        } else {
            aliased()
        };
        state.note_native_started(2).unwrap();
        state.commit(2, finished());
        state.refresh_closure(&AtomicBool::new(false), true);
        let (summary, resolutions) = state.finalize_delegation();
        assert_eq!(summary.unwrap()["delegated_resolved"], 1);
        let annotations = Annotations::new(resolutions, &state.closure.borrow());
        let rows = match state.records.replace(RecordSink::Memory(Vec::new())) {
            RecordSink::Memory(mut rows) => {
                assert!(!sidecar);
                rows.iter_mut().for_each(|row| annotations.apply(row));
                rows
            }
            RecordSink::Sidecar(writer) => {
                assert!(sidecar);
                Streamed::new(writer, annotations).collect().unwrap()
            }
        };
        assert_eq!(rows.len(), 3);
        // The alias: representative and status from the ledger resolution.
        assert_eq!(rows[1]["record_kind"], "delegated_not_inspected");
        assert_eq!(rows[1]["final_representative_id"], 2);
        assert_eq!(
            rows[1]["responsibility_status"],
            "discharged_by_representative"
        );
        // Natives keep their own fields; every row gains the closed flag.
        for row in &rows {
            assert_eq!(row["descendant_closed"], true, "{row}");
        }
        assert!(rows[0].get("final_representative_id").is_none());
        assert!(rows[2].get("responsibility_status").is_none());
    }
    // A partial inspection whose anchor retained a frontier stays blocked.
    let mut queue = Queue::with_policy(
        20,
        None,
        SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(3).unwrap(),
        },
    )
    .unwrap();
    let ledger = queue.delegation.as_mut().unwrap();
    ledger.begin_initial_admission().unwrap();
    queue.admit(domain(0, Some(0))).unwrap();
    queue.admit(domain(10, Some(10))).unwrap();
    queue
        .delegation
        .as_mut()
        .unwrap()
        .finish_initial_admission()
        .unwrap();
    let mut state = State::new(queue, 0, None);
    state.note_native_started(0).unwrap();
    state
        .accept(
            Event::one(Effect::Admit {
                domain: domain(20, Some(20)),
                successor: false,
                conditional: false,
            }),
            &request(),
        )
        .unwrap();
    state
        .accept(
            Event::one(Effect::Frontier {
                value: json!({"kind":"anchor_frontier"}),
                successor: false,
                conditional: false,
            }),
            &request(),
        )
        .unwrap();
    state.commit(0, finished());
    state.note_native_started(1).unwrap();
    state.commit(1, finished());
    state.note_native_started(2).unwrap();
    let scope = super::super::super::initial_overlap::InitialOverlapScope {
        anchor_id: 0,
        cut: 1,
        residual_powers: Default::default(),
    };
    state.commit(
        2,
        Finished {
            stats: NativeStats::ApplyPartial(Default::default(), scope),
            ..finished()
        },
    );
    assert!(state.error.is_none());
    let (_, rows) = state.finalized_records();
    assert_eq!(rows[2]["record_kind"], "partial_initial_overlap_inspection");
    assert_eq!(rows[2]["local_classification_discharged"], false);
    assert_eq!(
        rows[2]["responsibility_status"],
        json!({"blocked_by_residual_or_initial_anchor_frontiers":1})
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn streamed_report_is_byte_identical_to_the_materialized_pretty_document() {
    let dir = test_directory();
    let mut state = sidecar_state(&dir, 2);
    state.refresh_closure(&AtomicBool::new(false), true);
    let (_, resolutions) = state.finalize_delegation();
    let annotations = Annotations::new(resolutions, &state.closure.borrow());
    let RecordSink::Sidecar(writer) = state.records.replace(RecordSink::Memory(Vec::new())) else {
        unreachable!("sidecar state")
    };
    let streamed = Streamed::new(writer, annotations);
    let document = json!({"alpha":{"nested":[1,2.5,null]},"domains":null,"zeta":"z","empty":[]});
    let mut bytes = Vec::new();
    streamed.write_json(&document, &mut bytes).unwrap();
    let mut materialized = document.clone();
    materialized["domains"] = Value::Array(streamed.collect().unwrap());
    assert_eq!(materialized["domains"].as_array().unwrap().len(), 2);
    assert_eq!(bytes, serde_json::to_vec_pretty(&materialized).unwrap());
    // No records: still the materialized empty array.
    let empty = Streamed::new(Sidecar::new(dir.clone(), 9), Annotations::default());
    let mut bytes = Vec::new();
    empty.write_json(&document, &mut bytes).unwrap();
    let mut materialized = document;
    materialized["domains"] = json!([]);
    assert_eq!(bytes, serde_json::to_vec_pretty(&materialized).unwrap());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn restore_reconstructs_the_sidecar_without_loading_records_and_appends_after_it() {
    let fixture = Fixture::save(&aliased());
    assert_eq!(fixture.manifest()["sections"]["records"]["total"], 2);
    let mut store = fixture.open(true).unwrap();
    store.bind_owners(vec![OWNER.into()]).unwrap();
    let mut state = store.resume::<1>(&|_| {}).unwrap().unwrap().state;
    match &*state.records.borrow() {
        RecordSink::Sidecar(sidecar) => {
            assert_eq!(sidecar.directory(), fixture.dir);
            assert_eq!(sidecar.total(), 2);
            assert_eq!(sidecar.closed().len(), 1);
            assert_eq!(sidecar.generation(), 3);
        }
        RecordSink::Memory(_) => panic!("restored records must stay on disk"),
    }
    store.attach_records(&state).unwrap();
    state.note_native_started(2).unwrap();
    state.commit(2, finished());
    assert!(state.error.is_none(), "{:?}", state.error);
    assert!(fixture.dir.join(Section::Records.file_name(3)).exists());
    let saved = store
        .save(&state, &[], &[], true, &|_| {})
        .unwrap()
        .unwrap();
    assert_eq!(saved["checkpoint"]["generation"], 3);
    assert!(saved["checkpoint"]["section_seconds"]["records"].is_number());
    drop(store);
    let manifest = fixture.manifest();
    assert_eq!(manifest["sections"]["records"]["total"], 3);
    let segments = manifest["sections"]["records"]["segments"]
        .as_array()
        .unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[1]["generation"], 3);
    assert_eq!(segments[1]["first"], 2);
    let resumed = fixture.resume::<1>().unwrap();
    let rows = resumed.records.borrow().snapshot();
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    // A Memory prefix cannot be attached over saved walk state.
    let store = fixture.open(true).unwrap();
    assert!(store.attach_records(&aliased()).is_err());
}

#[test]
fn record_inventory_is_checked_even_when_the_closure_is_unavailable() {
    let fixture = Fixture::save(&aliased());
    fixture.rewrite_section::<1>(Section::Nodes, |flags| *flags = json!([]));
    fixture.rewrite_section::<1>(Section::Edges, |edges| *edges = json!([]));
    fixture.rewrite_section::<1>(Section::Meta, |meta| {
        meta["closure"]["unavailable"] = json!("test: monitor released");
    });
    // An unavailable monitor is accepted (it claims nothing) ...
    fixture.resume::<1>().unwrap();
    // ... but the sidecar inventory is not optional.
    fixture.rewrite_section::<1>(Section::Records, |records| {
        records.as_array_mut().unwrap().pop();
    });
    let error = fixture.resume::<1>().err().unwrap();
    assert_eq!(error, "checkpoint records/publications disagree");
}

/// Ready: stream 1 publishes out of order with two accepted events.
fn ready_state() -> State<1> {
    let mut queue = Queue::new(10, None);
    queue.delegation = Some(Ledger::new_ready(NonZeroUsize::new(3).unwrap(), 10).unwrap());
    for id in 0..3 {
        queue.admit(domain(id, Some(id))).unwrap();
    }
    let mut state = State::new(queue, 0, None);
    let ticket = Ticket {
        parent: 1,
        part: None,
    };
    state.activate_stream(ticket, true).unwrap();
    state.note_native_started(1).unwrap();
    for _ in 0..2 {
        state.accept(Event::one(Effect::Count), &request()).unwrap();
    }
    state.commit(1, finished());
    state.complete_stream(ticket);
    assert!(state.error.is_none(), "{:?}", state.error);
    state
}

fn meta_of(fixture: &Fixture) -> Value {
    let manifest = fixture.manifest();
    let file = manifest["sections"]["meta"]["file"].as_str().unwrap();
    serde_json::from_slice(&fs::read(fixture.dir.join(file)).unwrap()).unwrap()
}
fn keys(value: &Value) -> std::collections::BTreeSet<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn ready_accepted_events_aggregate_is_persisted_and_derived_for_older_checkpoints() {
    let state = ready_state();
    assert_eq!(state.records_accepted_events, 2);
    assert_eq!(state.events, 2);
    let fixture = Fixture::save(&state);
    assert_eq!(meta_of(&fixture)["progress"]["records_accepted_events"], 2);
    let restored = fixture.resume_full::<1>().unwrap();
    assert_eq!(restored.state.records_accepted_events, 2);
    assert_eq!(restored.report["records_accepted_events_derived"], false);
    // A present but malformed aggregate is refused, never silently derived.
    fixture.rewrite_section::<1>(Section::Meta, |meta| {
        meta["progress"]["records_accepted_events"] = json!("2");
    });
    let error = fixture.resume::<1>().err().unwrap();
    assert!(
        error.contains("invalid ready accepted-events aggregate"),
        "{error}"
    );
    // A checkpoint written without the aggregate (a fable_5_1 binary, also
    // after an executable-history rollback saved again): derived once.
    fixture.rewrite_section::<1>(Section::Meta, |meta| {
        meta["progress"]
            .as_object_mut()
            .unwrap()
            .remove("records_accepted_events");
    });
    let restored = fixture.resume_full::<1>().unwrap();
    assert_eq!(restored.state.records_accepted_events, 2);
    assert_eq!(restored.report["records_accepted_events_derived"], true);
    // The derivation reads the records: a corrupt count is refused.
    fixture.rewrite_section::<1>(Section::Records, |records| {
        records[0]["accepted_events"] = json!(3);
    });
    let error = fixture.resume::<1>().err().unwrap();
    assert!(
        error.contains("accepted-prefix accounting mismatch"),
        "{error}"
    );
    fixture.rewrite_section::<1>(Section::Records, |records| {
        records[0]
            .as_object_mut()
            .unwrap()
            .remove("accepted_events");
    });
    let error = fixture.resume::<1>().err().unwrap();
    assert!(error.contains("no accepted-events count"), "{error}");
    // Ordered checkpoints never carry the key.
    let ordered = Fixture::save(&aliased());
    assert!(
        meta_of(&ordered)["progress"]
            .get("records_accepted_events")
            .is_none()
    );
    ordered.rewrite_section::<1>(Section::Meta, |meta| {
        meta["progress"]["records_accepted_events"] = json!(1);
    });
    assert!(
        ordered
            .resume::<1>()
            .err()
            .unwrap()
            .contains("ordered checkpoint carries ready accepted-event accounting")
    );
}

#[test]
fn meta_section_keeps_the_key_set_the_fable_5_1_binaries_accept() {
    // Top-level meta keys of rustred-102adcc3 (fable_5_1 at 343a86a7), whose
    // meta section denies unknown fields; its progress reader reads only the
    // keys below and ignores any other. A campaign may roll back to it
    // through the executable history, so neither policy may add a meta key.
    const META: [&str; 13] = [
        "closure",
        "counters",
        "details",
        "input_frontiers",
        "inputs",
        "optional",
        "parallel",
        "progress",
        "queue",
        "refusals",
        "route_joint_support_masks_pruned",
        "streams",
        "uncommitted",
    ];
    const PROGRESS: [&str; 5] = [
        "physical_enabled",
        "physical_inspections_published",
        "physical_parent",
        "replay",
        "subdivided_logical_inspections",
    ];
    for state in [ready_state(), aliased()] {
        let meta = meta_of(&Fixture::save(&state));
        assert_eq!(keys(&meta), META.into_iter().collect());
        let mut progress = PROGRESS
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        if state.ready() {
            progress.insert("records_accepted_events");
        }
        assert_eq!(keys(&meta["progress"]), progress);
    }
}

#[test]
fn save_takes_the_sidecar_reservation_and_refuses_a_foreign_sidecar() {
    // The save generation is the sidecar's reservation, never a fresh scan
    // that would skip over the sidecar's own open segment.
    let fixture = Fixture::save(&State::<1>::new(Queue::new(8, None), 0, None));
    let mut store: Store = fixture.open(true).unwrap();
    store.bind_owners(vec![OWNER.into()]).unwrap();
    let mut state = store.resume::<1>(&|_| {}).unwrap().unwrap().state;
    state
        .records
        .get_mut()
        .push(json!({"id":0,"note":"unpublished, never resumed"}))
        .unwrap();
    assert!(fixture.dir.join(Section::Records.file_name(3)).exists());
    state.events += 1;
    let saved = store
        .save(&state, &[], &[], true, &|_| {})
        .unwrap()
        .unwrap();
    assert_eq!(saved["checkpoint"]["generation"], 3);
    assert_eq!(fixture.manifest()["sections"]["records"]["total"], 1);
    // A sidecar from another directory is refused.
    let other = test_directory();
    *state.records.get_mut() = RecordSink::Sidecar(Sidecar::new(other.clone(), 9));
    state.events += 1;
    assert!(
        store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap_err()
            .contains("record sidecar")
    );
    fs::remove_dir_all(other).unwrap();
}
