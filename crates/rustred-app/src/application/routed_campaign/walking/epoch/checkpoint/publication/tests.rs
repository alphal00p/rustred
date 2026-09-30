use super::super::super::dispatch::Dispatch;
use super::super::metadata::{Admission, Identity, Inputs};
use super::super::tests::{Directory, state};
use super::*;
use crate::application::routed_campaign::matching::input::Query;
use crate::{OwnerDomainMatchRequest, OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest};
use rustred::solver::DomainPowerBounds;
use serde_json::{Value, json};

fn queries() -> Vec<Query> {
    (0..2)
        .map(|id| Query {
            id: format!("query-{id}"),
            auxiliary: id == 0,
            role_declared: true,
            owner: vec![true, false],
            lower: vec![id, 0],
            upper: vec![Some(id), Some(0)],
            rank: Some(100),
            powers: DomainPowerBounds::default(),
        })
        .collect()
}

fn request(queries: &[Query]) -> OwnerDomainWalkRequest {
    let rows: Vec<_> = queries
        .iter()
        .map(|q| {
            json!({
                "id":q.id,"owner":"10","lower":q.lower,"upper":q.upper,"max_numerator_rank":q.rank
            })
        })
        .collect();
    let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
        "{}".into(),
        json!({"schema":"rustred.owner-domain-queries.json.v2","queries":rows,
            "query_roles":{"required":["query-1"],"auxiliary":["query-0"]}})
        .to_string(),
    ));
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request
}

fn rows() -> Vec<Value> {
    vec![
        json!({"id":"query-0","domain":0,"role":"auxiliary","role_declared":true}),
        json!({"id":"query-1","domain":1,"role":"required","role_declared":true}),
    ]
}

#[test]
fn metadata_records_full_inventory_even_before_initial_admission_finishes() {
    let queries = queries();
    let request = request(&queries);
    let owners = vec!["0".repeat(64)];
    let identity = Identity::new(&request, &owners, &queries).unwrap();
    let state = state(1);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 8).unwrap();
    let rows = rows();
    let mut inputs = Inputs {
        identity: &identity,
        admission: Admission::InProgress,
        rows: &rows[..1],
        frontiers: &[],
        stop: None,
        operational_stop: None,
        admission_failure: None,
    };
    let (bytes, digest) = inputs.write_scalars(&boundary, Vec::new()).unwrap();
    let meta: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(digest.blake3, *blake3::hash(&bytes).as_bytes());
    assert_eq!(meta["total_queries"], 2);
    assert_eq!(meta["processed_queries"], 1);
    assert_eq!(meta["initial_admission"], "in_progress");
    assert_eq!(meta["lockstep_b"], 8);
    assert_eq!(meta["walk_semantics_version"], 4);
    assert_eq!(meta["g2"], "off");
    assert_eq!(meta["quarantined"], json!(0));
    inputs.admission = Admission::Complete;
    assert!(
        inputs.write_scalars(&boundary, Vec::new()).is_err(),
        "an admitted prefix cannot claim to be the complete input set"
    );
}

#[test]
fn metadata_refuses_role_reordering_and_hidden_initial_ids() {
    let queries = queries();
    let request = request(&queries);
    let owners = vec!["0".repeat(64)];
    let identity = Identity::new(&request, &owners, &queries).unwrap();
    let state = state(2);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    for mutation in 0..4 {
        let mut rows = rows();
        match mutation {
            0 => rows[0]["role"] = "required".into(),
            1 => rows.swap(0, 1),
            2 => rows[0]["role_declared"] = false.into(),
            3 => {
                rows.pop();
            }
            _ => unreachable!(),
        }
        let inputs = Inputs {
            identity: &identity,
            admission: Admission::InProgress,
            rows: &rows,
            frontiers: &[],
            stop: None,
            operational_stop: None,
            admission_failure: None,
        };
        assert!(inputs.validate(&boundary).is_err(), "mutation {mutation}");
    }
}

#[test]
fn internal_publication_is_bound_and_latest_precedes_previous() {
    let directory = Directory::new();
    let mut store = Store::fresh(directory.0.clone()).unwrap();
    let mut records = Sidecar::new(directory.0.clone(), 1);
    let queries = queries();
    let request = request(&queries);
    let owners = vec!["0".repeat(64)];
    let identity = Identity::new(&request, &owners, &queries).unwrap();
    let state = state(2);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    let rows = rows();
    let inputs = Inputs {
        identity: &identity,
        admission: Admission::Complete,
        rows: &rows,
        frontiers: &[],
        stop: None,
        operational_stop: None,
        admission_failure: None,
    };
    let first = store.save(&boundary, &inputs, &mut records).unwrap();
    assert_eq!(first.generation, 1);
    assert!(first.warnings.is_empty());
    let manifest = read_manifest(&first.manifest).unwrap();
    assert!(manifest.resumable);
    assert_eq!(manifest.format, FORMAT);
    assert_eq!(manifest.schema, 3);
    assert_eq!(first.manifest_blake3, manifest_digest(&manifest).unwrap());
    assert!(
        serde_json::to_value(&manifest)
            .unwrap()
            .get("restore_validated")
            .is_none()
    );
    // Observing streamed writes cannot alter even one section/manifest byte.
    let observed_directory = Directory::new();
    let mut observed_store = Store::fresh(observed_directory.0.clone()).unwrap();
    let mut observed_records = Sidecar::new(observed_directory.0.clone(), 1);
    let mut notifications = 0;
    let observed = observed_store
        .save_observed(&boundary, &inputs, &mut observed_records, &mut || {
            notifications += 1
        })
        .unwrap();
    assert!(notifications > 0);
    assert_eq!(
        fs::read(&observed.manifest).unwrap(),
        fs::read(&first.manifest).unwrap()
    );
    for reference in &manifest.files {
        let bytes = fs::read(directory.0.join(&reference.file)).unwrap();
        assert_eq!(bytes.len() as u64, reference.bytes);
        assert_eq!(*blake3::hash(&bytes).as_bytes(), reference.blake3);
    }
    store.fail = Some(FailPoint::BeforePrevious);
    let second = store.save(&boundary, &inputs, &mut records).unwrap();
    assert_eq!(second.generation, 2);
    assert_eq!(second.warnings.len(), 1);
    assert_eq!(read_manifest(&second.manifest).unwrap().generation, 2);
    assert!(!directory.0.join(PREVIOUS).exists());
    assert!(!store.failed);
    store.fail = None;
    store.save(&boundary, &inputs, &mut records).unwrap();
    assert_eq!(
        read_manifest(&directory.0.join(LATEST)).unwrap().generation,
        3
    );
    assert_eq!(
        read_manifest(&directory.0.join(PREVIOUS))
            .unwrap()
            .generation,
        2
    );
    assert!(!directory.0.join("epoch-internal-latest.json").exists());
    let path = directory.0.join(LATEST);
    let mut changed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    changed["manifest"]["generation"] = 4.into();
    fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(
        read_manifest(&path).is_err(),
        "changed authority is not self-authenticated"
    );
}

#[test]
fn public_store_refuses_old_private_names_and_foreign_identity() {
    let directory = Directory::new();
    fs::write(directory.0.join("epoch-internal-latest.json"), b"{}").unwrap();
    assert_eq!(
        Store::open(directory.0.clone()).err().unwrap().kind(),
        io::ErrorKind::InvalidInput
    );
    assert!(!directory.0.join("checkpoint.lock").exists());
    let other = Directory::new();
    let path = other.0.join(LATEST);
    for format in [
        "RUSTRED-EPOCH-INTERNAL-WRITER",
        "RUSTRED-WALK-CP5",
        "RUSTRED-EPOCH-EXPORT",
    ] {
        let manifest = Manifest {
            format: format.into(),
            schema: 3,
            generation: 1,
            arity: 1,
            walk_semantics_version: 4,
            resumable: true,
            files: Vec::new(),
        };
        write_manifest(&path, &manifest).unwrap();
        assert_eq!(
            read_manifest(&path).err().unwrap().kind(),
            io::ErrorKind::InvalidInput
        );
    }
}

#[cfg(unix)]
#[test]
fn public_manifest_pointer_refuses_symlinks() {
    let directory = Directory::new();
    fs::write(directory.0.join("target.json"), b"{}").unwrap();
    std::os::unix::fs::symlink("target.json", directory.0.join(LATEST)).unwrap();
    assert!(read_manifest(&directory.0.join(LATEST)).is_err());
}

#[test]
fn preinstallation_failure_preserves_old_authority_and_only_leaves_orphans() {
    for point in [FailPoint::AfterSections, FailPoint::BeforeLatest] {
        let directory = Directory::new();
        let mut store = Store::fresh(directory.0.clone()).unwrap();
        let mut records = Sidecar::new(directory.0.clone(), 1);
        let queries = queries();
        let request = request(&queries);
        let owners = vec!["0".repeat(64)];
        let identity = Identity::new(&request, &owners, &queries).unwrap();
        let state = state(2);
        let dispatch = Dispatch::new();
        let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
        let rows = rows();
        let inputs = Inputs {
            identity: &identity,
            admission: Admission::Complete,
            rows: &rows,
            frontiers: &[],
            stop: None,
            operational_stop: None,
            admission_failure: None,
        };
        store.save(&boundary, &inputs, &mut records).unwrap();
        let old = fs::read(directory.0.join(LATEST)).unwrap();
        store.fail = Some(point);
        assert!(store.save(&boundary, &inputs, &mut records).is_err());
        assert_eq!(fs::read(directory.0.join(LATEST)).unwrap(), old);
        assert_eq!(
            read_manifest(&directory.0.join(LATEST)).unwrap().generation,
            1
        );
        assert!(directory.0.join(Section::Domains.filename(2)).exists());
        assert!(store.failed);
        assert!(store.save(&boundary, &inputs, &mut records).is_err());
    }
}

#[test]
fn manifest_reader_is_bounded_and_refuses_changed_authority() {
    let directory = Directory::new();
    let path = directory.0.join(LATEST);
    fs::write(&path, vec![b' '; MAX_MANIFEST_BYTES + 1]).unwrap();
    assert!(read_manifest(&path).is_err());
    let mut limited = Limit::new(Vec::new(), 3);
    limited.write_all(b"abc").unwrap();
    assert!(limited.write_all(b"d").is_err());
    assert_eq!(limited.output, b"abc");
}

#[test]
fn orthant_slots_are_serialized_even_when_their_ids_are_not_live() {
    let mut state = state(2);
    // This test isolates persistence of the historical slot from later
    // mathematical restore validation of its full-orthant geometry.
    state.store.unique_mut().unwrap().buckets[0].orthant = Some(0);
    state.set_live(0, false);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    let (bytes, digest) = write_orthants(&boundary, Vec::new()).unwrap();
    assert_eq!(&bytes[16..], &0u32.to_le_bytes());
    assert_eq!(digest.blake3, *blake3::hash(&bytes).as_bytes());
}

#[test]
fn large_owner_inventory_does_not_make_scalar_metadata_unsavable() {
    let queries = queries();
    let request = request(&queries);
    // More than the scalar metadata cap, but stored as a streamed section.
    let owners = vec!["0".repeat(64); 20_000];
    let identity = Identity::new(&request, &owners, &queries).unwrap();
    let state = state(2);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    let rows = rows();
    let inputs = Inputs {
        identity: &identity,
        admission: Admission::Complete,
        rows: &rows,
        frontiers: &[],
        stop: None,
        operational_stop: None,
        admission_failure: None,
    };
    let (_, inventory) = inputs.write_owners(io::sink()).unwrap();
    assert!(inventory.bytes > MAX_META_BYTES);
    let (scalar, _) = inputs
        .write_scalars(&boundary, Limit::new(Vec::new(), MAX_META_BYTES))
        .unwrap();
    assert!(scalar.output.len() < 8192);
    let meta: Value = serde_json::from_slice(&scalar.output).unwrap();
    assert_eq!(meta["owner_count"], 20_000);
    assert!(meta.get("owners").is_none());
}
