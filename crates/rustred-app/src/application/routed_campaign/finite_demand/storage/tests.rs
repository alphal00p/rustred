//! Storage fixtures are synthetic integer records, NOT native closure proofs.
//! Core export/import and publication correctness have separate required gates.
use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
mod atomic;
mod framing;
mod lookup;
mod provenance;
mod raw;
mod validation;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let path = std::env::temp_dir().join(format!(
                "rustred-finite-store-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create owned test directory: {error}"),
            }
        }
        panic!("cannot acquire owned test directory");
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn limits() -> StorageLimits {
    StorageLimits {
        diagnostics_enabled: true,
        raw: RawCensusLimits {
            max_page_records: 2,
            ..Default::default()
        },
        max_arity: 32,
        max_delta_records: 128,
        max_delta_payload_bytes: 1024 * 1024,
        max_page_records: 2,
        max_page_bytes: 64 * 1024,
        max_cached_pages: 1,
        max_cached_page_bytes: 64 * 1024,
        max_directory_bytes: 1024 * 1024,
        max_manifest_bytes: 1024 * 1024,
        max_state_bytes: 1024 * 1024,
        max_scope_rows: 16,
        max_census_records: 128,
        max_runs: 16,
        max_pages: 128,
        max_physical_files: 1024,
        max_disk_bytes: 16 * 1024 * 1024,
        max_lookup_pages: 16,
        max_lookup_bytes: 1024 * 1024,
        max_transient_bytes: 128 * 1024 * 1024,
    }
}
fn binding() -> RawExecutionBinding {
    RawExecutionBinding {
        physical_arity: 2,
        program_identity: [7; 32],
    }
}
fn key(n: i64) -> StoredWork {
    StoredWork::Route(vec![n, 1])
}
fn state(work: u64, windows: u64) -> GenerationState {
    GenerationState {
        scope: ScopeBinding {
            original_source: [4; 32],
            rows: vec![ScopeRow {
                origin: "original:0".into(),
                role: ScopeRole::Required,
                exact_domain: [5; 32],
            }],
        },
        cursors: vec![CursorRecord {
            source_policy: [6; 32],
            completed_windows: windows,
            input_occurrences: windows,
            encoded_cursor: format!("synthetic-wire-cursor-{windows}").into_bytes(),
        }],
        completed_work: work,
        input_occurrences: windows,
        census: RawCensus::default(),
    }
}
fn commit(
    writer: &mut StoreWriter,
    keys: Vec<StoredWork>,
    work: u64,
    windows: u64,
) -> ValidatedGeneration {
    writer
        .commit_generated(keys, state(work, windows), &AtomicBool::new(false))
        .unwrap()
        .generation
}
fn page_path(generation: &ValidatedGeneration, index: usize) -> PathBuf {
    let page = generation
        .manifest
        .runs
        .iter()
        .flatten()
        .nth(index)
        .unwrap();
    generation
        .directory
        .join("pages")
        .join(wire::name(&page.object.digest, "rrwork"))
}
fn physical_bytes(path: &Path) -> u64 {
    [
        path.to_path_buf(),
        path.join("pages"),
        path.join("raw"),
        path.join("states"),
        path.join("generations"),
    ]
    .iter()
    .flat_map(|directory| fs::read_dir(directory).unwrap())
    .map(|entry| entry.unwrap())
    .filter(|entry| entry.file_type().unwrap().is_file())
    .map(|entry| entry.metadata().unwrap().len())
    .sum()
}
