//! CLASSIFY-ONLY LOAD of one table file (lane import5, interfaces/import.md 18.6): the table is loaded by the import
//! exactly as a one-source spec would load it (`engine_table` source, the table its own frame, an empty start set),
//! with NO closure and the rule bodies parsed but not kept (bounded memory): every refusal of the importer is reported typed, and a loaded table reports what it carries (line
//! kinds, rule and `sym` records, witness status, markers, table-declared terminals). It is the honest answer to "can
//! the campaign load this table": the same code path as a campaign load, minus the check.
//!
//! Nothing here knows a node: the path is the only input.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;
use serde_json::json;

use super::error::ImportError;
use super::load::{LoadOptions, load_inner};
use super::spec::TableSetSpec;

/// Outcome of [`classify_table`].
#[derive(Clone, Debug, Default, Serialize)]
pub struct TableClass {
    pub path: String,
    pub bytes: u64,
    /// `loaded` | `refused`.
    pub outcome: String,
    /// Refused: the stable reason word (`ImportError` kind, or `TableRefusal::word` for a refused table).
    pub reason: Option<String>,
    /// Refused: the full message.
    pub detail: Option<String>,
    /// Non-rule line kinds of the file (loaded tables).
    pub kinds: BTreeMap<String, usize>,
    /// Rule + sym records of the file, records dispatched, records the engine does not dispatch.
    pub records: usize,
    pub dispatched: usize,
    pub not_dispatched: usize,
    pub witness: BTreeMap<String, usize>,
    pub sym_records: usize,
    pub sym_verified: usize,
    pub table_terminals: usize,
    pub assembly: bool,
    pub marker_overrides: Vec<String>,
    pub seconds: f64,
}

/// Options of [`classify_table`].
#[derive(Clone, Debug, Default)]
pub struct ClassifyOptions {
    /// A scratch directory (the empty start file and, without `index_dir`, nothing else); the default is the system
    /// temporary directory.
    pub scratch: Option<PathBuf>,
    /// Persistent index directory (`LoadOptions::index_dir`); None = a temporary index removed after the load.
    pub index_dir: Option<PathBuf>,
    /// `marker_rules.accept_fewer` for this load (None = strict: interfaces/import.md 18.3).
    pub accept_fewer_rules: Option<String>,
    /// The spec's `reader` (per-record memory limits; None = the defaults: interfaces/import.md 19).
    pub reader: Option<super::spec::ReaderSpec>,
}

static SEQ: AtomicUsize = AtomicUsize::new(0);

/// The stable word of an import error (the refusal's own word for a refused table).
pub fn error_word(e: &ImportError) -> String {
    match e {
        ImportError::Io { .. } => "io".into(),
        ImportError::Spec(_) => "spec".into(),
        ImportError::Digest { .. } => "digest".into(),
        ImportError::Line { .. } => "line".into(),
        ImportError::Source { .. } => "source".into(),
        ImportError::Label { .. } => "label".into(),
        ImportError::Evidence { .. } => "evidence".into(),
        ImportError::Core(_) => "core".into(),
        ImportError::Unsupported(_) => "unsupported".into(),
        ImportError::Refused { reason, .. } => reason.word(),
    }
}

/// Load `path` as a one-table spec without a closure (interfaces/import.md 18.6).
pub fn classify_table(path: &Path, opts: &ClassifyOptions) -> TableClass {
    let t0 = std::time::Instant::now();
    let mut out = TableClass {
        path: path.display().to_string(),
        bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        ..Default::default()
    };
    match classify_inner(path, opts, &mut out) {
        Ok(()) => out.outcome = "loaded".into(),
        Err(e) => {
            out.outcome = "refused".into();
            out.reason = Some(error_word(&e));
            out.detail = Some(e.to_string());
        }
    }
    out.seconds = t0.elapsed().as_secs_f64();
    out
}

fn classify_inner(
    path: &Path,
    opts: &ClassifyOptions,
    out: &mut TableClass,
) -> Result<(), ImportError> {
    let scratch = opts
        .scratch
        .clone()
        .unwrap_or_else(std::env::temp_dir)
        .join(format!(
            "rr-import-classify-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
    std::fs::create_dir_all(&scratch).map_err(|e| super::error::io_err(&scratch, e))?;
    let start = scratch.join("start.jsonl");
    let res = (|| {
        std::fs::write(&start, b"").map_err(|e| super::error::io_err(&start, e))?;
        let mut src =
            json!({"id": "table", "kind": "engine_table", "path": path.display().to_string()});
        if let Some(r) = &opts.accept_fewer_rules {
            src["marker_rules"] = json!({ "accept_fewer": r });
        }
        let mut spec = json!({
            "schema": super::spec::SPEC_SCHEMA,
            "frame_from": "table",
            "start": {"path": start.display().to_string(), "format": "keys_jsonl"},
            "sources": [src],
        });
        if let Some(r) = &opts.reader {
            spec["reader"] = serde_json::to_value(r).map_err(|e| ImportError::Spec(e.to_string()))?;
        }
        let spec = TableSetSpec::from_json(&spec.to_string())?;
        // bodies validated (every rhs key parsed) but not kept: bounded memory on tables of several GB
        let set = load_inner(
            &spec,
            &scratch,
            &LoadOptions {
                index_dir: opts.index_dir.clone(),
            },
            true,
        )?;
        let rep = &set.report().sources[0];
        out.kinds = rep.line_kinds.clone();
        out.records = rep.records_in_files;
        out.dispatched = rep.records_dispatched;
        out.not_dispatched = rep.not_dispatched.len();
        out.witness = rep.witness.clone();
        if let Some(s) = &rep.sym {
            out.sym_records = s.records;
            out.sym_verified = s.verified;
        }
        out.table_terminals = rep.table_terminals;
        out.assembly = !rep.assembly_files.is_empty();
        out.marker_overrides = rep.marker_overrides.clone();
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    res
}
