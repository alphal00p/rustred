use super::*;
use rustred::family::IntegralKey;
use rustred::foundry::artifact::{ClosedArtifact, derive_two_loop_unit_mass_sunset};
use rustred::reduction::{ClosedUnitMassProgram, GuardedRootCache, GuardedRootSource};

fn key(power: i64) -> IntegralKey {
    // This factorized tensor numerator contributes the genuine angular
    // nonzero guard d, unlike the normalized scalar tadpole recurrence.
    IntegralKey::try_new([power, 1, -4]).unwrap()
}

fn artifact() -> (ClosedArtifact, [u8; 32]) {
    let bytes = derive_two_loop_unit_mass_sunset()
        .unwrap()
        .encode_durable()
        .unwrap();
    let digest = *blake3::hash(&bytes).as_bytes();
    (ClosedArtifact::decode_durable(&bytes).unwrap(), digest)
}

fn reference(program: &ClosedUnitMassProgram<'_>, power: i64) -> ApplicationCacheRecord {
    program
        .reduce_guarded_root(
            &key(power),
            &mut GuardedRootCache::new(program, Default::default()),
            Default::default(),
            None,
        )
        .unwrap()
        .record
}

#[test]
fn closed_root_backing_store_is_durable_without_descendant_capture() {
    let scratch = super::super::tests::Scratch::new();
    let delta = scratch.0.join("closed-delta");
    let (artifact, digest) = artifact();
    let program = ClosedUnitMassProgram::new(&artifact, digest);
    let expected = reference(&program, 2);
    assert!(!expected.nonzero_conditions.is_empty());
    assert!(
        expected
            .nonzero_conditions
            .contains(artifact.family().dimension())
    );
    let options = SavedApplicationRunningCacheOptions {
        checkpoint_interval: Duration::from_secs(60),
        ..Default::default()
    };
    let mut cold = SavedApplicationRunningCache::open(
        program.binding().clone(),
        program.context().clone(),
        &delta,
        Vec::new(),
        options,
    )
    .unwrap();
    let mut cold_ram = GuardedRootCache::new(&program, Default::default());
    let result = cold
        .with_io(|backing, _capture| {
            program
                .reduce_guarded_root(&key(2), &mut cold_ram, Default::default(), Some(backing))
                .map_err(|error| AppError::from_native(error, AppErrorKind::Execution))
        })
        .unwrap();
    assert_eq!(result.source, GuardedRootSource::Reduced);
    assert_eq!(result.record, expected);
    let report = cold.control().report().unwrap();
    assert_eq!(report.accepted_records, 1);
    assert_eq!(report.committed_records, 1);
    assert_eq!(report.pending_records, 0);
    cold.control().close().unwrap();

    let mut warm = SavedApplicationRunningCache::open(
        program.binding().clone(),
        program.context().clone(),
        &delta,
        Vec::new(),
        options,
    )
    .unwrap();
    let mut fresh_ram = GuardedRootCache::new(&program, Default::default());
    let result = warm
        .with_io(|backing, _capture| {
            program
                .reduce_guarded_root(&key(2), &mut fresh_ram, Default::default(), Some(backing))
                .map_err(|error| AppError::from_native(error, AppErrorKind::Execution))
        })
        .unwrap();
    assert_eq!(result.source, GuardedRootSource::Backing);
    assert_eq!(result.record, expected);
    assert_eq!(result.work.rule_applications(), 0);
    let report = warm.control().report().unwrap();
    assert_eq!(report.accepted_records, 0);
    assert_eq!(report.committed_records, 0);
    warm.control().close().unwrap();
}

#[test]
fn closed_read_only_miss_computes_without_training_its_frozen_pack() {
    let scratch = super::super::tests::Scratch::new();
    let delta = scratch.0.join("closed-base");
    let (artifact, digest) = artifact();
    let program = ClosedUnitMassProgram::new(&artifact, digest);
    let stored = reference(&program, 2);
    let expected = reference(&program, 3);
    assert!(!expected.nonzero_conditions.is_empty());
    let mut writer =
        SavedApplicationCacheWriter::create(&delta, program.binding().clone(), Default::default())
            .unwrap();
    writer
        .append_completed(&[stored], program.context())
        .unwrap();
    writer.checkpoint().unwrap();
    let snapshot = writer
        .freeze_snapshot(&scratch.0.join("closed-base.rrindex"))
        .unwrap();
    drop(writer);
    let frozen = SavedApplicationCachePack::open_snapshot(
        &delta,
        program.binding(),
        &snapshot,
        Default::default(),
    )
    .unwrap();
    let generation = frozen.generation();
    let chunks = frozen.statistics().chunks;
    let original_index = std::fs::read(delta.join("cache.rrindex")).unwrap();
    let mut read_only = SavedApplicationReadOnlyCache::new(
        program.binding().clone(),
        program.context().clone(),
        vec![SavedApplicationRunningCacheBase::Pack(frozen)],
    )
    .unwrap();
    let held_out = ApplicationCacheKey::guarded_root(&key(3));
    assert_eq!(
        read_only.lookup(&held_out, program.context()).unwrap(),
        None
    );
    let mut ram = GuardedRootCache::new(&program, Default::default());
    let result = program
        .reduce_guarded_root(&key(3), &mut ram, Default::default(), Some(&mut read_only))
        .unwrap();
    assert_eq!(result.source, GuardedRootSource::Reduced);
    assert_eq!(result.record, expected);
    assert_eq!(
        read_only.lookup(&held_out, program.context()).unwrap(),
        None
    );

    let foreign = CoefficientContext::try_new(["closed_read_only_foreign_d"]).unwrap();
    assert!(read_only.store_completed(&expected, &foreign).is_err());
    let mut wrong_arity = expected;
    wrong_arity.key = ApplicationCacheKey::guarded_root(&IntegralKey::try_new([5, 1]).unwrap());
    assert!(
        read_only
            .store_completed(&wrong_arity, program.context())
            .is_err()
    );
    let unchanged =
        SavedApplicationCachePack::open_generated(&delta, program.binding(), Default::default())
            .unwrap();
    assert_eq!(unchanged.generation(), generation);
    assert_eq!(unchanged.statistics().chunks, chunks);
    assert_eq!(
        std::fs::read(delta.join("cache.rrindex")).unwrap(),
        original_index
    );
}
