//! Test seams around the real on-disk store. `rewrite_section` re-encodes a
//! section from its decoded JSON image AND recomputes the manifest digest, so
//! semantic validators, not checksums, must catch the corruption; `flip_byte`
//! leaves the digest stale to exercise the checksum path.
use super::super::{
    OwnerDomainMatchRequest, OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest,
    OwnerDomainWalkSchedulingPolicy,
    delegation::StoredLedger,
    execution::State,
    queue::{Domain, StoredBuckets},
};
use super::manifest::{Manifest, Section, SectionRef, Segment, Segmented};
use super::sections::{self, HEADER_BYTES, Header, Identity, Tag};
use super::{OwnerDomainWalkCheckpointOptions, Restored, Store, manifest, restore};
use crate::application::atomic_file::write_file_atomically;
use serde_json::{Value, json};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Explicitly a test binding, not a claim about a production owner bundle.
pub(in super::super) const OWNER: &str = "test-only immutable in-memory native fixture";

pub(in super::super) fn test_directory() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../TMP")
        .join(format!(
            "checkpoint-store-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
    fs::create_dir(&path).unwrap();
    path
}

/// One test-owned checkpoint directory; removed on drop.
pub(in super::super) struct Fixture {
    pub dir: PathBuf,
    pub request: OwnerDomainWalkRequest,
}
impl Fixture {
    /// Minimal request whose publication and scheduling policies match the
    /// state's ledger; the store binds both on resume.
    pub fn request_for<const N: usize>(state: &State<N>) -> OwnerDomainWalkRequest {
        let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
            "selection".into(),
            "queries".into(),
        ));
        if state.ready() {
            request.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
        }
        if let Some(ledger) = state.queue.delegation.as_ref() {
            request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
                lookahead: ledger.lookahead(),
            };
        }
        request
    }
    pub fn save<const N: usize>(state: &State<N>) -> Self {
        Self::save_with(state, Self::request_for(state), &[], &[])
    }
    /// Bootstrap, bind a synthetic owner and write generation 2.
    pub fn save_with<const N: usize>(
        state: &State<N>,
        request: OwnerDomainWalkRequest,
        inputs: &[Value],
        frontiers: &[Value],
    ) -> Self {
        let dir = test_directory();
        let mut request = request;
        request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&dir));
        let fixture = Self { dir, request };
        let mut store = fixture.open(false).unwrap();
        store.bootstrap().unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        store.save(state, inputs, frontiers, true, &|_| {}).unwrap();
        fixture
    }
    pub fn open(&self, resume: bool) -> Result<Store, String> {
        let mut request = self.request.clone();
        request.checkpoint.as_mut().unwrap().resume = resume;
        Store::open(&request)?.ok_or_else(|| "test checkpoint store missing".to_owned())
    }
    /// Another generation from a resumed-open store, without restoring.
    pub fn save_again<const N: usize>(&self, state: &State<N>) -> Result<Option<Value>, String> {
        let mut store = self.open(true)?;
        store.bind_owners(vec![OWNER.into()])?;
        store.save(state, &[], &[], true, &|_| {})
    }
    pub fn resume_full<const N: usize>(&self) -> Result<Restored<N>, String> {
        let mut store = self.open(true)?;
        store.bind_owners(vec![OWNER.into()])?;
        store
            .resume::<N>(&|_| {})?
            .ok_or_else(|| "test stateful checkpoint missing".to_owned())
    }
    pub fn resume<const N: usize>(&self) -> Result<State<N>, String> {
        Ok(self.resume_full::<N>()?.state)
    }
    pub fn manifest(&self) -> Value {
        serde_json::from_reader(File::open(self.dir.join("latest.json")).unwrap()).unwrap()
    }
    pub fn write_manifest(&self, manifest: &Value) {
        write_file_atomically(
            &self.dir.join("latest.json"),
            &serde_json::to_vec(manifest).unwrap(),
            true,
        )
        .unwrap();
    }
    fn typed_manifest(&self) -> Manifest {
        serde_json::from_value(self.manifest()).unwrap()
    }
    fn identity<const N: usize>(manifest: &Manifest) -> Identity {
        Identity {
            arity: N,
            ready: manifest.publication_policy == "ready",
            semantics: manifest.walk_semantics_version,
        }
    }
    /// Files of one section in manifest order (segments for segmented ones).
    fn files(&self, section: Section) -> Vec<Segment> {
        let manifest = self.typed_manifest();
        if let Some(plain) = manifest.sections.plain(section) {
            return vec![Segment {
                generation: manifest.generation,
                file: plain.file.clone(),
                first: 0,
                count: 0,
                bytes: plain.bytes,
                blake3: plain.blake3.clone(),
            }];
        }
        manifest
            .sections
            .segmented(section)
            .map(|s| s.segments.clone())
            .unwrap_or_default()
    }
    fn read(&self, file: &str) -> Vec<u8> {
        fs::read(self.dir.join(file)).unwrap()
    }
    /// Flip one byte of the section's first file; the manifest digest stays stale.
    pub fn flip_byte(&self, section: Section, offset: usize) {
        let file = self.files(section).remove(0).file;
        let mut bytes = self.read(&file);
        bytes[offset] ^= 0x55;
        fs::write(self.dir.join(&file), &bytes).unwrap();
    }
    fn install(&self, section: Section, file: String, bytes: Vec<u8>, count: usize) {
        let mut manifest = self.typed_manifest();
        let digest = blake3::hash(&bytes).to_hex().to_string();
        let len = bytes.len() as u64;
        fs::write(self.dir.join(&file), &bytes).unwrap();
        if section.segmented() {
            *manifest.sections.segmented_mut(section) = Some(Segmented {
                total: count as u64,
                segments: (count > 0)
                    .then(|| {
                        vec![Segment {
                            generation: manifest.generation,
                            file,
                            first: 0,
                            count: count as u64,
                            bytes: len,
                            blake3: digest,
                        }]
                    })
                    .unwrap_or_default(),
            });
        } else {
            *manifest.sections.plain_mut(section) = Some(SectionRef {
                file,
                bytes: len,
                blake3: digest,
            });
        }
        self.write_manifest(&serde_json::to_value(&manifest).unwrap());
    }
    /// Raw rewrite of the section's first file with the digest recomputed
    /// (for header-level corruption).
    pub fn rewrite_bytes(&self, section: Section, edit: impl FnOnce(&mut Vec<u8>)) {
        let segment = self.files(section).remove(0);
        let mut bytes = self.read(&segment.file);
        edit(&mut bytes);
        let digest = blake3::hash(&bytes).to_hex().to_string();
        let len = bytes.len() as u64;
        fs::write(self.dir.join(&segment.file), &bytes).unwrap();
        let mut manifest = self.typed_manifest();
        if section.segmented() {
            let s = manifest.sections.segmented_mut(section).as_mut().unwrap();
            let entry = s
                .segments
                .iter_mut()
                .find(|s| s.file == segment.file)
                .unwrap();
            entry.bytes = len;
            entry.blake3 = digest;
        } else {
            let plain = manifest.sections.plain_mut(section).as_mut().unwrap();
            plain.bytes = len;
            plain.blake3 = digest;
        }
        self.write_manifest(&serde_json::to_value(&manifest).unwrap());
    }
    /// Decode a section to JSON, edit it, re-encode it as one file under the
    /// latest generation and recompute its manifest entry (segmented
    /// sections collapse to a single segment tiling the new total).
    pub fn rewrite_section<const N: usize>(&self, section: Section, edit: impl FnOnce(&mut Value)) {
        let manifest = self.typed_manifest();
        let identity = Self::identity::<N>(&manifest);
        let generation = manifest.generation;
        let file = section.file_name(generation);
        let files = self.files(section);
        let header = |tag: Tag, count: usize| {
            Header::new(tag, &identity, count, 0)
                .unwrap()
                .encode()
                .to_vec()
        };
        match section {
            Section::Meta => {
                let mut value: Value = serde_json::from_slice(&self.read(&files[0].file)).unwrap();
                edit(&mut value);
                self.install(section, file, serde_json::to_vec(&value).unwrap(), 0);
            }
            Section::Nodes => {
                let flags = sections::read_nodes(&self.read(&files[0].file), &identity).unwrap();
                let mut value = json!(flags);
                edit(&mut value);
                let flags: Vec<u8> = serde_json::from_value(value).unwrap();
                let mut bytes = header(Tag::Nodes, flags.len());
                bytes.extend_from_slice(&flags);
                self.install(section, file, bytes, flags.len());
            }
            Section::Ledger => {
                let ledger = sections::read_ledger(&self.read(&files[0].file), &identity).unwrap();
                let mut value = serde_json::to_value(&ledger).unwrap();
                edit(&mut value);
                let ledger: StoredLedger = serde_json::from_value(value).unwrap();
                let mut bytes = header(Tag::Ledger, ledger.len());
                bincode::serde::encode_into_std_write(
                    &ledger,
                    &mut bytes,
                    bincode::config::standard(),
                )
                .unwrap();
                self.install(section, file, bytes, ledger.len());
            }
            Section::Index => {
                let buckets = sections::read_index(&self.read(&files[0].file), &identity).unwrap();
                let mut value = serde_json::to_value(&buckets).unwrap();
                edit(&mut value);
                let buckets: StoredBuckets = serde_json::from_value(value).unwrap();
                let mut bytes = header(Tag::Index, buckets.len());
                bincode::serde::encode_into_std_write(
                    &buckets,
                    &mut bytes,
                    bincode::config::standard(),
                )
                .unwrap();
                self.install(section, file, bytes, buckets.len());
            }
            Section::Domains => {
                let mut domains: Vec<Domain<N>> = Vec::new();
                for segment in &files {
                    sections::read_domains(
                        &self.read(&segment.file),
                        &identity,
                        segment.first as usize,
                        segment.count as usize,
                        &mut domains,
                    )
                    .unwrap();
                }
                let mut value = serde_json::to_value(&domains).unwrap();
                edit(&mut value);
                let domains: Vec<Arc<Domain<N>>> = serde_json::from_value::<Vec<Domain<N>>>(value)
                    .unwrap()
                    .into_iter()
                    .map(Arc::new)
                    .collect();
                let mut bytes = Vec::new();
                sections::write_domains(&mut bytes, &identity, &domains, 0).unwrap();
                self.install(section, file, bytes, domains.len());
            }
            Section::Edges => {
                let mut edges = Vec::new();
                for segment in &files {
                    sections::read_edges(
                        &self.read(&segment.file),
                        &identity,
                        segment.first as usize,
                        segment.count as usize,
                        &mut edges,
                    )
                    .unwrap();
                }
                let mut value = json!(edges);
                edit(&mut value);
                let edges: Vec<(u32, u32)> = serde_json::from_value(value).unwrap();
                let mut bytes = header(Tag::Edges, edges.len());
                for (source, target) in &edges {
                    bytes.extend_from_slice(&source.to_le_bytes());
                    bytes.extend_from_slice(&target.to_le_bytes());
                }
                self.install(section, file, bytes, edges.len());
            }
            Section::Records => {
                let mut records = Vec::new();
                for segment in &files {
                    sections::read_records(
                        &self.read(&segment.file),
                        segment.count as usize,
                        &mut records,
                    )
                    .unwrap();
                }
                let mut value = Value::Array(records);
                edit(&mut value);
                let records = value.as_array().unwrap();
                let mut bytes = Vec::new();
                sections::write_records(&mut bytes, records).unwrap();
                self.install(section, file, bytes, records.len());
            }
        }
        assert!(HEADER_BYTES == 32);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Small-fixture round trip through the real store (bootstrap, save, resume).
pub(in super::super) fn round_trip_state<const N: usize>(
    state: &State<N>,
) -> Result<State<N>, String> {
    Fixture::save(state).resume()
}

/// Round trip bound to the caller's request (publication, subdivision and
/// limit policy); the checkpoint location is replaced by a test directory.
pub(in super::super) fn round_trip_state_on_disk<const N: usize>(
    state: &State<N>,
    request: &OwnerDomainWalkRequest,
) -> Result<State<N>, String> {
    Fixture::save_with(state, request.clone(), &[], &[]).resume()
}

/// Restore a CP5 directory without a request binding (ignored replay tests).
pub(in super::super) fn restore_directory<const N: usize>(
    dir: &Path,
) -> Result<Restored<N>, String> {
    let manifest = manifest::read(&dir.join("latest.json"))?;
    let verify_seconds = restore::verify_files(dir, &manifest)?;
    restore::restore::<N>(dir, &manifest, verify_seconds)
}
