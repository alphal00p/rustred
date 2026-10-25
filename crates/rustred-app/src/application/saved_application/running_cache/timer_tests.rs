use super::*;
use rustred::family::IntegralKey;
use std::collections::BTreeMap;

#[test]
fn overdue_timer_commits_before_consuming_ready_ingress() {
    let scratch = super::super::tests::Scratch::new();
    let directory = scratch.0.join("timer-ready-ingress");
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let binding = ApplicationCacheBinding {
        artifact_digest: [43; 32],
        family_fingerprint: "running-cache-timer-ready-ingress".into(),
        physical_arity: 1,
        layer: "raw-declared-unit-mass-v1".into(),
    };
    let options = SavedApplicationRunningCacheOptions {
        checkpoint_interval: Duration::from_secs(1),
        batch_records: 64,
        batch_bytes: usize::MAX,
        ..Default::default()
    };
    let writer =
        SavedApplicationCacheWriter::create(&directory, binding.clone(), options.storage).unwrap();
    let initial_generation =
        SavedApplicationCachePack::open_generated(&directory, &binding, options.storage)
            .unwrap()
            .generation();
    let expected: Vec<_> = [2, 3, 4]
        .into_iter()
        .map(|power| ApplicationCacheRecord {
            key: ApplicationCacheKey::route(&IntegralKey::try_new([power]).unwrap()),
            terms: BTreeMap::from([(IntegralKey::try_new([1]).unwrap(), context.one())]),
            nonzero_conditions: Vec::new(),
        })
        .collect();
    let bytes: Vec<_> = expected
        .iter()
        .map(|record| record_bytes(record).unwrap())
        .collect();
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            accepted: expected.len() as u64,
            committed: 0,
            pending: expected.len(),
            bytes: bytes.iter().sum(),
            reserved: 0,
            reserved_bytes: 0,
            keys: expected.iter().map(|record| record.key.clone()).collect(),
            generation: initial_generation,
            checkpoint: Instant::now().checked_sub(Duration::from_secs(10)).unwrap(),
            last_error: None,
            failure: None,
            capture_skipped_records: 0,
            capture_metadata_exhausted: false,
            closing: false,
            closed: false,
        }),
        changed: Condvar::new(),
    });
    let (sender, receiver) = mpsc::sync_channel(expected.len());
    for (record, &bytes) in expected.iter().zip(&bytes).skip(1) {
        sender
            .send(Message::Record(Queued {
                record: record.clone(),
                bytes,
                budget: ResourceBudget::capture_current(),
            }))
            .unwrap();
    }
    let (reply, response) = mpsc::channel();
    sender
        .send(Message::Close(ResourceBudget::capture_current(), reply))
        .unwrap();

    // Ingress is already ready when the overdue worker starts. A zero receive
    // timeout alone still receives those records, postponing the timer commit
    // until Close and incorrectly combining everything into one chunk.
    Worker {
        writer,
        directory: directory.clone(),
        binding: binding.clone(),
        context: context.clone(),
        options,
        receiver,
        shared,
        resources: ResourceBudget::capture_current(),
        records: vec![expected[0].clone()],
        budgets: vec![ResourceBudget::capture_current()],
        bytes: bytes[0],
    }
    .run();
    // Keep the channel connected throughout run: Close, rather than a sender
    // disconnect, must provide the final publication barrier.
    drop(sender);

    let report = response.recv().unwrap().unwrap();
    assert!(report.closed);
    assert!(report.last_error.is_none());
    assert!(report.writer_error.is_none());
    assert_eq!(report.accepted_records, expected.len() as u64);
    assert_eq!(report.committed_records, report.accepted_records);
    assert_eq!(report.pending_records, 0);
    assert_eq!(report.pending_bytes, 0);
    assert_eq!(report.generation, initial_generation + 2);

    let mut reader =
        SavedApplicationCachePack::open_generated(&directory, &binding, options.storage).unwrap();
    assert_eq!(reader.generation(), initial_generation + 2);
    assert_eq!(reader.statistics().chunks, 2);
    assert_eq!(reader.statistics().records, expected.len());
    for record in expected {
        assert_eq!(reader.lookup(&record.key, &context).unwrap(), Some(record),);
    }
}
