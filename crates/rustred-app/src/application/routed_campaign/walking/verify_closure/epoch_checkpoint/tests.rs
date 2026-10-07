//! Synthetic framing/selection tests, not a native closure certificate.
use super::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn cp6_amendment_shape_accepts_declared_required_scope_and_auxiliary_rescue() {
    for role in ["required", "auxiliary"] {
        let row = json!({"id":"appended","domain":17,"role":role,
            "role_declared":true,"amendment":1});
        // Shape only. The independent oracle subsequently authenticates the
        // exact role/geometry against the immutable supplied amendment chain.
        assert!(input_inventory(&[row.clone()], &[]).is_ok());
        for (key, value) in [
            ("role_declared", json!(false)),
            ("amendment", json!(0)),
            ("role", json!("unexpected")),
        ] {
            let mut bad = row.clone();
            bad[key] = value;
            assert!(input_inventory(&[bad], &[]).is_err());
        }
    }
}

struct Fixture {
    directory: PathBuf,
    manifest: Manifest,
    permissions: Vec<(PathBuf, fs::Permissions)>,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "rustred-cp6-cold-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let mut value = Self {
            directory,
            permissions: Vec::new(),
            manifest: Manifest {
                format: FORMAT.into(),
                schema: 3,
                generation: 1,
                arity: 1,
                walk_semantics_version: 4,
                resumable: true,
                files: Vec::new(),
            },
        };
        let mut image = vec![0];
        image.extend(1u32.to_le_bytes());
        image.push(1);
        image.extend(0u32.to_le_bytes());
        image.extend(1u16.to_le_bytes());
        image.extend(1u16.to_le_bytes());
        image.extend([0; 27]);
        value.section(1, 1, image);
        value.section(2, 1, vec![0]);
        value.section(3, 1, 1u64.to_le_bytes().to_vec());
        value.section(4, 1, 0u64.to_le_bytes().to_vec());
        value.section(5, 0, vec![]);
        let mut anchors = 2u16.to_le_bytes().to_vec();
        anchors.extend(0u64.to_le_bytes());
        value.section(6, 0, anchors);
        let dispatch: Vec<_> = [0u64, 1, 16, 1, 0, 0, 0, 0, 0]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect();
        value.section(7, 0, dispatch);
        value.section(8, 1, vec![0]);
        value.section(9, 0, vec![]);
        let empty_digest = blake3::hash(b"").to_hex().to_string();
        let scalar = json!({"schema":5,"record_schema":1,
            "epoch_base_window":16,"epoch_result_escrow_jobs":0,"epoch_result_escrow_bytes":null,
            "preparation":{"helpers":0,"obligations":4294967295u64,"retirements":4294967295u64},
            "request":"bound-request","owner_count":0,
            "owners_digest":blake3::hash(b"[]").as_bytes(),"walk_semantics_version":4,
            "lockstep_b":16,"max_domains":100,"k":0,"watermark":1,"p0":1,
            "ledger_counts":[1,0,0,0,0,0,0,0],
            "walk":{"events":0,"successors":0,"conditional":0,"known_reuse":0,"job_duplicates":0,"frontiers":0,
                "completed":0,"natives":0,"routed":0,"route_masks":0,"initial_inspected":0,"dispatched":0,"g2_records":0},
            "closure":{"initial_closed":0,"unavailable":null},"edge_runs":0,"edges":0,
            "records_digest":empty_digest,"edge_digest":empty_digest,
            "initial_admission":"in_progress","total_queries":2,"processed_queries":1,"input_frontiers":0,
            "amendments":[],"quarantined":0,"abandoned_obligations":0,"g2":"off",
            "imported_prefix":0,"engine_certification_void":false});
        value.add("meta", 1, serde_json::to_vec(&scalar).unwrap());
        value.add("owners", 0, vec![]);
        value.add(
            "inputs",
            1,
            b"{\"id\":\"root\",\"domain\":0,\"role\":\"required\",\"role_declared\":true}\n"
                .to_vec(),
        );
        value.add("input-frontiers", 0, vec![]);
        value.add("record-segments", 0, b"[]".to_vec());
        let mut orthants = b"EPORTH01".to_vec();
        orthants.extend(1u64.to_le_bytes());
        orthants.extend(u32::MAX.to_le_bytes());
        value.add("orthants", 1, orthants);
        value.publish();
        fs::copy(
            value.directory.join("latest.json"),
            value.directory.join("previous.json"),
        )
        .unwrap();
        fs::write(value.directory.join("checkpoint.lock"), b"").unwrap();
        fs::write(
            value.directory.join("epoch-session.bin"),
            b"session-not-adopted",
        )
        .unwrap();
        value
    }
    fn add(&mut self, key: &str, count: u64, bytes: Vec<u8>) {
        let suffix = key.strip_prefix("state-").unwrap_or(key);
        let file = format!("epoch-00000000000000000001-{suffix}.part");
        fs::write(self.directory.join(&file), &bytes).unwrap();
        self.manifest.files.push(FileRef {
            key: key.into(),
            file,
            count,
            bytes: bytes.len() as u64,
            blake3: *blake3::hash(&bytes).as_bytes(),
        });
    }
    fn section(&mut self, number: u32, count: u64, body: Vec<u8>) {
        let mut bytes = b"EPC6PART".to_vec();
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(number.to_le_bytes());
        bytes.extend(count.to_le_bytes());
        bytes.extend(body);
        self.add(&format!("state-{number}"), count, bytes);
    }
    fn replace(&mut self, key: &str, count: u64, bytes: Vec<u8>) {
        let reference = self
            .manifest
            .files
            .iter_mut()
            .find(|f| f.key == key)
            .unwrap();
        fs::write(self.directory.join(&reference.file), &bytes).unwrap();
        reference.count = count;
        reference.bytes = bytes.len() as u64;
        reference.blake3 = *blake3::hash(&bytes).as_bytes();
    }
    fn scalar(&self) -> Value {
        let reference = self
            .manifest
            .files
            .iter()
            .find(|f| f.key == "meta")
            .unwrap();
        serde_json::from_slice(&fs::read(self.directory.join(&reference.file)).unwrap()).unwrap()
    }
    fn record(&mut self) -> PathBuf {
        use crate::application::routed_campaign::walking::epoch::records::{typed::*, wire};
        let record = Record {
            authority: Authority {
                id: 0,
                merge_epoch: 1,
                image: Image {
                    route: false,
                    owner: vec![true],
                    lower: vec![1],
                    upper: vec![1],
                    rank: Some(0),
                    powers: Default::default(),
                },
                body: Body::Native(Native {
                    class: 0,
                    kind: 0,
                    v0: 0,
                    distinct_edges: 0,
                    self_edge: false,
                    break_reason: 0,
                    error_kind: 0,
                    err_class: None,
                    panic: false,
                    emitted: 0,
                    accepted: 0,
                    stats_events: 0,
                    has_error: false,
                    frontiers: 0,
                    job_duplicates: 0,
                    known_reuse: 0,
                    resolver: Default::default(),
                    scope: Scope::Whole,
                }),
            },
            diagnostics: Diagnostics {
                stats_json: br#"{"events":0}"#.to_vec(),
                ..Default::default()
            },
        };
        let mut bytes = Vec::new();
        wire::append(&record, &mut bytes).unwrap();
        let name = "records-00000000000000000001.bin";
        let path = self.directory.join(name);
        fs::write(&path, &bytes).unwrap();
        self.replace(
            "record-segments",
            1,
            serde_json::to_vec(&json!([{
            "file":name,"generation":1,"first":0,"count":1,"bytes":bytes.len(),
            "blake3":blake3::hash(&bytes).to_hex().to_string()}]))
            .unwrap(),
        );
        let mut scalar = self.scalar();
        scalar["ledger_counts"] = json!([0, 0, 1, 0, 0, 0, 0, 0]);
        scalar["initial_admission"] = json!("complete");
        scalar["total_queries"] = json!(1);
        self.replace("meta", 1, serde_json::to_vec(&scalar).unwrap());
        self.publish();
        path
    }
    fn publish(&self) {
        let digest = blake3::hash(&serde_json::to_vec(&self.manifest).unwrap());
        fs::write(
            self.directory.join("latest.json"),
            serde_json::to_vec(&json!({"manifest":self.manifest,"blake3":digest.as_bytes()}))
                .unwrap(),
        )
        .unwrap();
    }
    fn inventory(&self) -> BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
        fs::read_dir(&self.directory)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                (
                    path.file_name().unwrap().to_str().unwrap().to_owned(),
                    (
                        fs::read(&path).unwrap(),
                        fs::metadata(path).unwrap().modified().unwrap(),
                    ),
                )
            })
            .collect()
    }
    fn read_only(&mut self) {
        let mut paths: Vec<_> = fs::read_dir(&self.directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        paths.push(self.directory.clone());
        for path in paths {
            let original = fs::metadata(&path).unwrap().permissions();
            let mut readonly = original.clone();
            readonly.set_readonly(true);
            fs::set_permissions(&path, readonly).unwrap();
            self.permissions.push((path, original));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for (path, permissions) in self.permissions.drain(..).rev() {
            fs::set_permissions(path, permissions).unwrap();
        }
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

#[test]
fn authenticated_valid_but_wrong_schedule_scalars_do_not_match_the_command() {
    use crate::{
        OwnerDomainMatchRequest, OwnerDomainWalkEpochPublicationOrder, OwnerDomainWalkRequest,
    };
    let mut fixture = Fixture::new();
    let mut scalar = fixture.scalar();
    scalar["epoch_rolling"] = json!(true);
    scalar["epoch_cut_size"] = json!(8);
    scalar["epoch_publication_order"] = json!("oldest-ready");
    fixture.replace("meta", 1, serde_json::to_vec(&scalar).unwrap());
    fixture.publish();
    let mut request =
        OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("s".into(), "q".into()));
    request.epoch_rolling = true;
    request.epoch_publication_order = OwnerDomainWalkEpochPublicationOrder::OldestReady;
    request.epoch_cut_size = Some(8);
    request.epoch_window = Some(16);
    let (_, sections, _) = read_raw::<1>(&fixture.directory).unwrap();
    assert!(schedule_matches_request(&sections.manifest, &request));
    for (key, wrong) in [
        ("epoch_publication_order", json!("oldest-prefix")),
        ("epoch_cut_size", json!(16)),
        ("lockstep_b", json!(32)),
    ] {
        let mut changed = scalar.clone();
        changed[key] = wrong;
        if key == "lockstep_b" {
            changed["epoch_base_window"] = changed[key].clone();
        }
        fixture.replace("meta", 1, serde_json::to_vec(&changed).unwrap());
        fixture.publish();
        let before = fixture.inventory();
        let (_, sections, _) = read_raw::<1>(&fixture.directory).unwrap();
        assert!(
            !schedule_matches_request(&sections.manifest, &request),
            "{key}"
        );
        assert_eq!(fixture.inventory(), before);
    }
    request.epoch_window = None;
    let (_, sections, _) = read_raw::<1>(&fixture.directory).unwrap();
    assert!(
        schedule_matches_request(&sections.manifest, &request),
        "omitted window inherits saved 32"
    );
}

#[test]
fn historical_diagnostic_cut_is_not_inferred_from_the_cold_process_environment() {
    use crate::{OwnerDomainMatchRequest, OwnerDomainWalkRequest};
    for rolling in [false, true] {
        let mut fixture = Fixture::new();
        let mut scalar = fixture.scalar();
        scalar["lockstep_b"] = json!(4);
        scalar["epoch_base_window"] = json!(4);
        if rolling {
            scalar["epoch_rolling"] = json!(true);
            scalar["epoch_cut_size"] = json!(4);
        }
        fixture.replace("meta", 1, serde_json::to_vec(&scalar).unwrap());
        fixture.publish();
        let (_, sections, _) = read_raw::<1>(&fixture.directory).unwrap();
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("s".into(), "q".into()));
        request.epoch_rolling = rolling;
        assert!(schedule_matches_request(&sections.manifest, &request));
        request.epoch_cut_size = Some(16);
        assert!(!schedule_matches_request(&sections.manifest, &request));
        request.epoch_cut_size = Some(4);
        assert!(schedule_matches_request(&sections.manifest, &request));
    }
}

#[test]
fn escrow_scalar_bounds_and_exact_command_are_checked() {
    use crate::{OwnerDomainMatchRequest, OwnerDomainWalkRequest};
    let mut fixture = Fixture::new();
    let mut scalar = fixture.scalar();
    scalar["epoch_rolling"] = json!(true);
    scalar["epoch_cut_size"] = json!(8);
    scalar["epoch_result_escrow_jobs"] = json!(4);
    scalar["epoch_result_escrow_bytes"] = json!(1024);
    scalar["lockstep_b"] = json!(20);
    fixture.replace("meta", 1, serde_json::to_vec(&scalar).unwrap());
    fixture.publish();
    let (_, sections, _) = read_raw::<1>(&fixture.directory).unwrap();
    let mut request =
        OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("s".into(), "q".into()));
    request.epoch_rolling = true;
    request.epoch_window = Some(16);
    request.epoch_result_escrow_jobs = 4;
    request.epoch_result_escrow_bytes = Some(1024);
    assert!(schedule_matches_request(&sections.manifest, &request));
    request.epoch_window = Some(20);
    assert!(!schedule_matches_request(&sections.manifest, &request));
    request.epoch_window = None;
    assert!(schedule_matches_request(&sections.manifest, &request));
    request.epoch_result_escrow_bytes = Some(2048);
    assert!(!schedule_matches_request(&sections.manifest, &request));
    for (key, value) in [
        ("schema", json!(4)),
        ("epoch_base_window", json!(17)),
        ("epoch_result_escrow_bytes", json!(null)),
        ("epoch_result_escrow_bytes", json!(0)),
        ("epoch_publication_order", json!("oldest-ready")),
        ("epoch_rolling", json!(false)),
    ] {
        let mut changed = scalar.clone();
        changed[key] = value;
        fixture.replace("meta", 1, serde_json::to_vec(&changed).unwrap());
        fixture.publish();
        assert!(read_raw::<1>(&fixture.directory).is_err(), "{key}");
    }
}

#[test]
fn cp6_cold_reads_twice_without_adopting_session_or_mutating_any_file() {
    let mut fixture = Fixture::new();
    fixture.read_only();
    let before = fixture.inventory();
    for _ in 0..2 {
        let (raw, epoch, records) = read_raw::<1>(&fixture.directory).unwrap();
        assert!(records.is_empty());
        assert_eq!(raw.domains.len(), 1);
        assert_eq!(raw.inputs.len(), 1);
        assert_eq!(epoch.manifest["total_queries"], 2);
        assert_eq!(epoch.manifest["initial_admission"], "in_progress");
        assert!(raw.records.is_empty());
        assert!(raw.executable.is_empty()); // Launch receipt, not native hash binding.
    }
    assert_eq!(fixture.inventory(), before);
    assert!(
        fixture
            .permissions
            .iter()
            .all(|(path, _)| fs::metadata(path).unwrap().permissions().readonly())
    );
    assert!(!fixture.directory.join("epoch-poison").exists());
}

#[test]
fn cp6_cold_refuses_corrupt_latest_without_falling_back_and_rejects_private_format() {
    let mut fixture = Fixture::new();
    fs::write(fixture.directory.join("latest.json"), b"{").unwrap();
    assert!(read_raw::<1>(&fixture.directory).is_err());
    assert!(fixture.directory.join("previous.json").is_file());
    fixture.manifest.format = "RUSTRED-EPOCH-INTERNAL-WRITER".into();
    fixture.publish();
    assert!(
        read_raw::<1>(&fixture.directory)
            .err()
            .unwrap()
            .contains("identity")
    );
}

#[test]
fn cp6_cold_checks_ignored_auxiliary_digest_and_exact_path_inventory() {
    let mut fixture = Fixture::new();
    let path = fixture.directory.join("epoch-00000000000000000001-7.part");
    let original = fs::read(&path).unwrap();
    let mut corrupted = original.clone();
    *corrupted.last_mut().unwrap() ^= 1;
    fs::write(&path, corrupted).unwrap();
    assert!(
        read_raw::<1>(&fixture.directory)
            .err()
            .unwrap()
            .contains("digest")
    );
    fs::write(&path, original).unwrap();
    fixture.manifest.files[0].file = "../foreign.part".into();
    fixture.publish();
    assert!(
        read_raw::<1>(&fixture.directory)
            .err()
            .unwrap()
            .contains("path")
    );
}

#[test]
fn cp6_manifest_cannot_relabel_prior_generation_sections() {
    let mut fixture = Fixture::new();
    fixture.manifest.generation = 2;
    fixture.publish();
    assert!(
        read_raw::<1>(&fixture.directory)
            .err()
            .unwrap()
            .contains("path")
    );
}

#[test]
fn cp6_input_reads_selected_immutable_reference_even_if_latest_changes() {
    let fixture = Fixture::new();
    let reference = fixture
        .manifest
        .files
        .iter()
        .find(|f| f.key == "owners")
        .unwrap();
    let input = Input::open(&fixture.directory, reference).unwrap();
    fs::write(
        fixture.directory.join("latest.json"),
        b"replaced-after-selection",
    )
    .unwrap();
    input.drain().unwrap();
    assert!(read_raw::<1>(&fixture.directory).is_err());
}

#[test]
fn cp6_consumed_record_bytes_are_authenticated_after_reference_capture() {
    use super::super::{Violations, load_records};
    let mut fixture = Fixture::new();
    let path = fixture.record();
    let original = fs::read(&path).unwrap();
    let (raw, epoch, records) = read_raw::<1>(&fixture.directory).unwrap();
    let mut violations = Violations::new(10);
    let loaded = load_records(raw, Some(epoch), Some(records), false, &mut violations).unwrap();
    assert_eq!(loaded.positions, [0]);

    let (raw, epoch, records) = read_raw::<1>(&fixture.directory).unwrap();
    // Same length, same id/tag/outdegree; only the authenticated full sidecar
    // digest can reject this otherwise benign record-field mutation.
    use crate::application::routed_campaign::walking::epoch::records::wire;
    let mut record = wire::read(&mut original.as_slice()).unwrap();
    record.diagnostics.seconds = 1.0;
    let mut changed = Vec::new();
    wire::append(&record, &mut changed).unwrap();
    assert_ne!(changed, original);
    assert_eq!(changed.len(), original.len());
    let replacement = fixture.directory.join("replacement.bin");
    fs::write(&replacement, &changed).unwrap();
    fs::rename(&replacement, &path).unwrap();
    assert!(
        load_records(raw, Some(epoch), Some(records), false, &mut violations)
            .err()
            .unwrap()
            .contains("digest")
    );

    fs::write(&path, &original).unwrap();
    let (_, _, records) = read_raw::<1>(&fixture.directory).unwrap();
    let input = records[0].open(&path, 1).unwrap();
    // A path replacement after opening preserves the selected original inode.
    fs::write(&replacement, &changed).unwrap();
    fs::rename(&replacement, &path).unwrap();
    let mut selected = Vec::new();
    BufReader::new(input).read_to_end(&mut selected).unwrap();
    assert_eq!(selected, original);

    fs::write(&path, &original).unwrap();
    let input = records[0].open(&path, 1).unwrap();
    fs::write(&path, &changed).unwrap();
    assert!(BufReader::new(input).read_to_end(&mut Vec::new()).is_err());
    // An empty stream must also reach an authenticated EOF.
    fs::write(&path, b"").unwrap();
    let reference = RecordRef(FileRef {
        key: "records".into(),
        file: path.file_name().unwrap().to_str().unwrap().into(),
        count: 0,
        bytes: 0,
        blake3: *blake3::hash(b"wrong").as_bytes(),
    });
    assert!(
        BufReader::new(reference.open(&path, 0).unwrap())
            .lines()
            .collect::<io::Result<Vec<_>>>()
            .is_err()
    );
}

#[test]
fn cp6_input_frontiers_correspond_exactly_to_unresolved_rows() {
    let mut fixture = Fixture::new();
    let mapped = json!({"id":"root","domain":0,"role":"required","role_declared":true});
    for marker in [json!(true), json!(false), Value::Null] {
        let mut row = mapped.clone();
        row["source_validity_unresolved"] = marker;
        let mut bytes = serde_json::to_vec(&row).unwrap();
        bytes.push(b'\n');
        fixture.replace("inputs", 1, bytes);
        fixture.publish();
        assert!(
            read_raw::<1>(&fixture.directory)
                .err()
                .unwrap()
                .contains("mapped input")
        );
    }
    let mut bytes = serde_json::to_vec(&mapped).unwrap();
    bytes.push(b'\n');
    fixture.replace("inputs", 1, bytes);
    fixture.replace("input-frontiers", 1, b"{\"id\":\"root\"}\n".to_vec());
    let mut scalar = fixture.scalar();
    scalar["input_frontiers"] = json!(1);
    fixture.replace("meta", 1, serde_json::to_vec(&scalar).unwrap());
    fixture.publish();
    assert!(
        read_raw::<1>(&fixture.directory)
            .err()
            .unwrap()
            .contains("no unresolved row")
    );

    let unresolved = json!({"id":"root","domain":null,"role":"required","role_declared":true,
        "source_validity_unresolved":true});
    let frontier = json!({"id":"root"});
    assert!(input_inventory(&[unresolved.clone()], &[frontier.clone()]).is_ok());
    assert!(input_inventory(&[unresolved.clone()], &[]).is_err());
    assert!(input_inventory(&[unresolved.clone()], &[json!({"id":"other"})]).is_err());
    assert!(input_inventory(&[unresolved.clone()], &[frontier.clone(), frontier.clone()]).is_err());
    let mut missing = unresolved.clone();
    missing.as_object_mut().unwrap().remove("domain");
    assert!(input_inventory(&[missing], &[frontier.clone()]).is_err());
    let mut extra = unresolved;
    extra["unknown"] = json!(false);
    assert!(input_inventory(&[extra], &[frontier]).is_err());
}

#[test]
fn cp6_source_frontier_is_checked_against_full_bound_query_geometry() {
    let query = crate::application::routed_campaign::matching::input::Query {
        id: "root".into(),
        auxiliary: false,
        role_declared: true,
        owner: vec![true],
        lower: vec![1],
        upper: vec![Some(2)],
        rank: Some(3),
        powers: DomainPowerBounds::default(),
    };
    let frontier = json!({"id":"root","kind":"initial_route_source_validity_obligation",
        "owner":"1","lower":[1],"upper":[2],"rank":3,"power_bounds":{
            "max_positive_power":null,"min_power_difference":null,"max_power_difference":null},
        "reached_missing_rule_claim":false});
    assert!(source_frontier_matches::<1>(&query, &frontier));
    for (key, value) in [
        ("lower", json!([0])),
        ("rank", json!(4)),
        ("owner", json!("0")),
        ("kind", json!("other")),
        ("reached_missing_rule_claim", json!(true)),
    ] {
        let mut bad = frontier.clone();
        bad[key] = value;
        assert!(!source_frontier_matches::<1>(&query, &bad));
    }
}

#[test]
fn cp6_summary_binding_is_explicitly_incomplete_and_generation_mismatch_is_not_waived() {
    use super::super::{Loaded, Node, Violations, bind_result};
    let fixture = Fixture::new();
    let (mut raw, epoch, _) = read_raw::<1>(&fixture.directory).unwrap();
    let domains = std::mem::take(&mut raw.domains);
    let loaded = Loaded {
        raw,
        domains,
        nodes: vec![Node::MISSING],
        digests: vec![[0; 16]],
        residuals: BTreeMap::new(),
        g2: BTreeMap::new(),
        positions: vec![u64::MAX],
        epoch: Some(epoch),
        finite_replay: None,
    };
    let path = fixture.directory.join("summary.json");
    for generation in [1, 2] {
        fs::write(
            &path,
            serde_json::to_vec(&json!({
                "checkpoint":{"state":"saved","format":FORMAT,"generation":generation},
                "full_result_in_output_document":false,
                "all_scheduled_domains_resolved":false,"recursive_worklist_exhausted":false
            }))
            .unwrap(),
        )
        .unwrap();
        let mut violations = Violations::new(10);
        let bound = bind_result(&path, &loaded, &[false], &mut violations).unwrap();
        assert_eq!(bound["kind"], "checkpoint_only_summary");
        assert_eq!(bound["complete"], false);
        assert!(bound["note"].as_str().unwrap().contains("--no-result"));
        assert_eq!(violations.is_empty(), generation == 1);
    }
}
