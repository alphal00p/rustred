use super::super::super::metadata::{Admission, Inputs};
use super::super::super::tests::{Directory, state};
use super::super::super::{MergeBoundary, publication};
use super::*;
use crate::application::routed_campaign::matching::input::Query;
use crate::application::routed_campaign::walking::epoch::dispatch::Dispatch;
use crate::application::routed_campaign::walking::epoch::record_store::Sidecar;
use crate::{OwnerDomainMatchRequest, OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest};
use rustred::solver::DomainPowerBounds;
use serde_json::{Value, json};
use std::fs;

struct Fixture {
    directory: Directory,
    request: OwnerDomainWalkRequest,
    queries: Vec<Query>,
    owners: Vec<String>,
}

impl Fixture {
    fn new(partial: bool) -> Self {
        let queries: Vec<_> = (0..2)
            .map(|id| Query {
                id: format!("q-{id}"),
                auxiliary: id == 0,
                role_declared: true,
                owner: vec![true, false],
                lower: vec![id, 0],
                upper: vec![Some(id), Some(0)],
                rank: Some(100),
                powers: DomainPowerBounds::default(),
            })
            .collect();
        let query_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"q-0","owner":"10","lower":[0,0],"upper":[0,0],"max_numerator_rank":100},
            {"id":"q-1","owner":"10","lower":[1,0],"upper":[1,0],"max_numerator_rank":100}
        ],"query_roles":{"required":["q-1"],"auxiliary":["q-0"]}});
        let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
            "{}".into(),
            query_json.to_string(),
        ));
        request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
        let fixture = Self {
            directory: Directory::new(),
            request,
            queries,
            owners: vec!["0".repeat(64)],
        };
        let identity = fixture.identity();
        let count = if partial { 1 } else { 2 };
        let state = state(count);
        let dispatch = Dispatch::new();
        let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
        let rows: Vec<_> = fixture.queries[..count].iter().enumerate().map(|(id, query)| json!({
            "id":query.id,"domain":id,"role":if query.auxiliary { "auxiliary" } else { "required" },"role_declared":true
        })).collect();
        let inputs = Inputs {
            identity: &identity,
            admission: if partial {
                Admission::InProgress
            } else {
                Admission::Complete
            },
            rows: &rows,
            frontiers: &[],
            stop: None,
            operational_stop: None,
            admission_failure: None,
        };
        let mut publisher = publication::Store::fresh(fixture.directory.0.clone()).unwrap();
        let mut records = Sidecar::new(fixture.directory.0.clone(), 1);
        publisher.save(&boundary, &inputs, &mut records).unwrap();
        fixture
    }
    fn identity(&self) -> Identity<'_> {
        Identity::new(&self.request, &self.owners, &self.queries).unwrap()
    }
    fn read(&self) -> io::Result<Provisional<2>> {
        read(&self.directory.0, &self.identity(), 16)
    }

    fn write_manifest(&self, manifest: &Manifest) {
        let digest = *blake3::hash(&serde_json::to_vec(manifest).unwrap()).as_bytes();
        fs::write(
            self.directory.0.join(publication::LATEST),
            serde_json::to_vec(&json!({"manifest":manifest,"blake3":digest})).unwrap(),
        )
        .unwrap();
    }
    fn mutate_manifest(&self, change: impl FnOnce(&mut Manifest)) {
        let mut manifest =
            publication::read_manifest(&self.directory.0.join(publication::LATEST)).unwrap();
        change(&mut manifest);
        self.write_manifest(&manifest);
    }
    fn replace_file(&self, key: &str, change: impl FnOnce(Vec<u8>) -> Vec<u8>) {
        let mut manifest =
            publication::read_manifest(&self.directory.0.join(publication::LATEST)).unwrap();
        let file = manifest
            .files
            .iter_mut()
            .find(|file| file.key == key)
            .unwrap();
        let path = self.directory.0.join(&file.file);
        let bytes = change(fs::read(&path).unwrap());
        fs::write(path, &bytes).unwrap();
        file.bytes = bytes.len() as u64;
        file.blake3 = *blake3::hash(&bytes).as_bytes();
        self.write_manifest(&manifest);
    }
    fn mutate_meta(&self, change: impl FnOnce(&mut Value)) {
        self.replace_file("meta", |bytes| {
            let mut meta: Value = serde_json::from_slice(&bytes).unwrap();
            change(&mut meta);
            serde_json::to_vec(&meta).unwrap()
        });
    }
}

#[test]
fn writer_manifest_scalar_owner_and_fixed_arrays_roundtrip_provisionally() {
    let fixture = Fixture::new(false);
    let decoded = fixture.read().unwrap();
    assert_eq!(decoded.manifest.generation, 1);
    assert_eq!(decoded.scalars.lockstep_b, 16);
    assert_eq!(decoded.scalars.watermark, 2);
    assert_eq!(decoded.store.len(), 2);
    assert_eq!(decoded.ledger.words(), [0, 0]);
    assert_eq!(decoded.nodes, [0, 0]);
    assert_eq!(decoded.live, [3]);
    assert_eq!(decoded.closure_flags, [0, 0]);
    assert_eq!(decoded.edges.runs(), 0);
    assert_eq!(decoded.anchors.len(), 0);
    assert!(decoded.frontier_counts.is_empty());
    assert_eq!(decoded.dispatch.session, 1);
    assert_eq!(decoded.dispatch.counter, 0);
    assert!(decoded.dispatch.in_flight.is_empty());
    assert!(decoded.record_segments.is_empty());
    assert_eq!(
        decoded
            .store
            .buckets
            .iter()
            .map(|b| b.index.storage().live)
            .sum::<usize>(),
        2
    );
    assert!(
        read::<2>(&fixture.directory.0, &fixture.identity(), 8).is_err(),
        "actual diagnostic B is bound"
    );
    assert!(read::<3>(&fixture.directory.0, &fixture.identity(), 16).is_err());
    let mut changed = fixture.request.clone();
    changed.max_domains = 1;
    let identity = Identity::new(&changed, &fixture.owners, &fixture.queries).unwrap();
    assert!(
        read::<2>(&fixture.directory.0, &identity, 16).is_err(),
        "requested domain limit bounds allocation"
    );
    changed.max_domains = 200_000;
    changed.workers = 3;
    let identity = Identity::new(&changed, &fixture.owners, &fixture.queries).unwrap();
    assert!(
        read::<2>(&fixture.directory.0, &identity, 16).is_ok(),
        "larger aggregate allowance and worker width do not change semantic binding"
    );
}

#[test]
fn bound_prefix_is_not_reclassified_as_complete_admission() {
    let fixture = Fixture::new(true);
    let decoded = fixture.read().unwrap();
    assert_eq!(
        (
            decoded.scalars.total_queries,
            decoded.scalars.processed_queries
        ),
        (2, 1)
    );
    assert_eq!(decoded.store.len(), 1);
    fixture.mutate_meta(|meta| meta["initial_admission"] = "complete".into());
    assert!(fixture.read().is_err());
}

#[test]
fn authenticated_sections_still_need_cross_state_seals_and_record_digest() {
    let fixture = Fixture::new(false);
    fixture.replace_file("state-2", |mut bytes| {
        bytes[28] = 1; // authenticated but Pending cannot be sealed
        bytes
    });
    assert!(fixture.read().is_err());
    let fixture = Fixture::new(false);
    fixture.replace_file("state-8", |mut bytes| {
        bytes[28] = 4; // authenticated Pending cannot be closed either
        bytes
    });
    assert!(fixture.read().is_err());
    let fixture = Fixture::new(false);
    fixture.mutate_meta(|meta| meta["records_digest"] = "0".repeat(64).into());
    assert!(fixture.read().is_err());
}

#[test]
fn scalar_identity_counts_reserved_features_and_overflows_fail_before_arrays() {
    for (key, value) in [
        ("walk_semantics_version", json!(2)),
        ("lockstep_b", json!(8)),
        ("request", json!("wrong")),
        ("owner_count", json!(2)),
        ("k", json!(1u64 << 48)),
        ("watermark", json!(u32::MAX)),
        ("max_domains", json!(1)),
        ("p0", json!(3)),
        ("total_queries", json!(1)),
        ("processed_queries", json!(3)),
        ("initial_admission", json!("unknown")),
        ("g2", json!("union")),
        ("amendments", json!([{}])),
        ("quarantined", json!([0])),
        ("imported_prefix", json!(1)),
        ("engine_certification_void", json!(true)),
        ("ledger_counts", json!([u64::MAX, 1, 0, 0, 0, 0, 0])),
        ("edge_runs", json!(1)),
        ("edges", json!(u64::MAX)),
        ("stop_reason", json!("unknown")),
        ("unexpected_field", json!(true)),
    ] {
        let fixture = Fixture::new(false);
        fixture.mutate_meta(|meta| meta[key] = value);
        assert!(fixture.read().is_err(), "{key}");
    }
}

#[test]
fn owner_inventory_and_cross_file_counts_are_not_trusted_from_manifest() {
    let fixture = Fixture::new(false);
    fixture.replace_file("owners", |mut bytes| {
        bytes[1] = b'1';
        bytes
    });
    assert!(fixture.read().is_err());
    for key in [
        "state-1",
        "state-2",
        "state-3",
        "state-4",
        "state-5",
        "state-6",
        "state-7",
        "state-8",
        "state-9",
        "orthants",
        "inputs",
        "input-frontiers",
        "record-segments",
    ] {
        let fixture = Fixture::new(false);
        fixture.mutate_manifest(|manifest| {
            manifest
                .files
                .iter_mut()
                .find(|file| file.key == key)
                .unwrap()
                .count = u64::MAX
        });
        assert!(fixture.read().is_err(), "{key}");
    }
    let fixture = Fixture::new(false);
    fixture.mutate_manifest(|manifest| {
        manifest
            .files
            .iter_mut()
            .find(|file| file.key == "meta")
            .unwrap()
            .bytes = publication::MAX_META_BYTES + 1
    });
    assert!(fixture.read().is_err());
}
