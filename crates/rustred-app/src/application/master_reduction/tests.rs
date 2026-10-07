use super::*;
use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use std::collections::BTreeSet;
use std::sync::Arc;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-master-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn session() -> TerminalRelationSession {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let family = Arc::new(
        IntegralFamily::new(
            "master-checkpoint",
            vec!["k".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![context.integer(1)],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    );
    TerminalRelationSession::new(
        family,
        BTreeSet::from([
            IntegralKey::try_new([1]).unwrap(),
            IntegralKey::try_new([2]).unwrap(),
        ]),
        0,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn master_checkpoint_resume_preserves_rows_and_exact_application() {
    let scratch = Scratch::new();
    let options = MasterReductionOptions::new("unused", &scratch.0);
    let mut original = session();
    let cancel = AtomicBool::new(false);
    original.step(&cancel).unwrap();
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    save(
        &options,
        &original,
        &mut report,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let mut restored = load_master_reduction(&scratch.0).unwrap();
    assert_eq!(restored.statistics(), original.statistics());
    for candidate in [&mut original, &mut restored] {
        while !candidate.is_complete() {
            candidate.step(&cancel).unwrap();
        }
    }
    assert_eq!(restored.statistics(), original.statistics());
    for key in original.raw_terminals() {
        assert_eq!(
            restored.apply_terminal(key).unwrap(),
            original.apply_terminal(key).unwrap()
        );
    }
}

#[test]
fn master_final_publication_is_idempotent_and_portable() {
    let scratch = Scratch::new();
    let mut native = session();
    let cancel = AtomicBool::new(false);
    while !native.is_complete() {
        native.step(&cancel).unwrap();
    }
    let options = MasterReductionOptions::new("unused", &scratch.0);
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    save(
        &options,
        &native,
        &mut report,
        "completed_nonminimal",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    publish_final(&options, &mut report).unwrap();
    publish_final(&options, &mut report).unwrap();
    let moved = Scratch::new();
    for entry in std::fs::read_dir(&scratch.0).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), moved.0.join(entry.file_name())).unwrap();
    }
    assert_eq!(
        master_reduction_inspect(&moved.0).unwrap()["status"],
        "completed_nonminimal"
    );
    assert_eq!(
        load_master_reduction(&moved.0).unwrap().statistics(),
        native.statistics()
    );
    let state = moved
        .0
        .join(report["native_state"]["file"].as_str().unwrap());
    let mut bytes = std::fs::read(&state).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(state, bytes).unwrap();
    assert!(load_master_reduction(&moved.0).is_err());
}

#[test]
fn master_writer_lock_releases_on_drop() {
    let scratch = Scratch::new();
    let first = storage::WriterLock::acquire(&scratch.0).unwrap();
    assert!(storage::WriterLock::acquire(&scratch.0).is_err());
    drop(first);
    assert!(storage::WriterLock::acquire(&scratch.0).is_ok());
}

#[test]
fn master_scope_table_distinguishes_caps_from_full_family() {
    let matching = crate::OwnerDomainMatchRequest::new(
        "{}".into(),
        json!({"queries":[{"max_numerator_rank":0,"power_bounds":{"max_power_difference":9}}]})
            .to_string(),
    );
    let scope = storage::scope_summary(&OwnerDomainWalkRequest::new(matching)).unwrap();
    assert_eq!(scope["max_starting_rank"], 0);
    assert_eq!(scope["max_starting_d"], 9);
    assert!(
        scope["meaning"]
            .as_str()
            .unwrap()
            .contains("not every integral")
    );
}
