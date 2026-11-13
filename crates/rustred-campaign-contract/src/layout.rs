//! The run directory of one attempt (child-job.md 3.1.3): `CDIR/jobs/<job_id>/a<attempt>/`.

use std::path::{Path, PathBuf};

/// `RUN/job.json`: supervisor, before spawn, read-only.
pub const JOB_SPEC: &str = "job.json";
/// `RUN/in/`: supervisor, small generated inputs.
pub const IN_DIR: &str = "in";
/// `RUN/out/`: child, the legacy output dir.
pub const OUT_DIR: &str = "out";
/// `RUN/EVENTS.jsonl`: child (3.5).
pub const EVENTS: &str = "EVENTS.jsonl";
/// `RUN/OUTCOME.json`: child, atomic (3.7).
pub const OUTCOME: &str = "OUTCOME.json";
/// `RUN/RESUME.json`: child, atomic (3.8).
pub const RESUME: &str = "RESUME.json";
/// `RUN/tmp/`: child scratch (TMPDIR).
pub const TMP_DIR: &str = "tmp";
/// `RUN/.reserve`: child disk reserve (3.10).
pub const RESERVE: &str = ".reserve";
/// Size of the disk reserve.
pub const RESERVE_BYTES: u64 = 1 << 20;
/// `RUN/STOP`: supervisor or operator stop request (3.4.5).
pub const STOP: &str = "STOP";
/// `RUN/stdout.log`, `RUN/stderr.log`: supervisor redirects.
pub const STDOUT_LOG: &str = "stdout.log";
pub const STDERR_LOG: &str = "stderr.log";
/// `RUN/sup/`: supervisor only.
pub const SUP_DIR: &str = "sup";

/// Files inside `out/`.
pub const TABLE: &str = "F.table.jsonl";
pub const QUEUE: &str = "F.queue.jsonl";
pub const CERT: &str = "F.cert.jsonl";
pub const FAMILY: &str = "F.family.sym";
pub const REFUSED: &str = "F.refused.jsonl";
pub const SPILL: &str = "F.spill.json";
pub const OBLIGATIONS: &str = "F.obligations.json";
pub const WITNESS: &str = "WITNESS.jsonl";
pub const LOCI: &str = "LOCI.jsonl";
pub const RESULTS: &str = "results.jsonl";

/// `CDIR/jobs/<job_id>/a<attempt>`.
pub fn attempt_dir(cdir: &Path, job_id: &str, attempt: u32) -> PathBuf {
    cdir.join("jobs").join(job_id).join(format!("a{attempt}"))
}

/// Paths of one run directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLayout {
    pub root: PathBuf,
}

impl RunLayout {
    pub fn new(root: impl Into<PathBuf>) -> RunLayout {
        RunLayout { root: root.into() }
    }
    pub fn job_spec(&self) -> PathBuf {
        self.root.join(JOB_SPEC)
    }
    pub fn in_dir(&self) -> PathBuf {
        self.root.join(IN_DIR)
    }
    pub fn out_dir(&self) -> PathBuf {
        self.root.join(OUT_DIR)
    }
    pub fn events(&self) -> PathBuf {
        self.root.join(EVENTS)
    }
    pub fn outcome(&self) -> PathBuf {
        self.root.join(OUTCOME)
    }
    pub fn resume(&self) -> PathBuf {
        self.root.join(RESUME)
    }
    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join(TMP_DIR)
    }
    pub fn reserve(&self) -> PathBuf {
        self.root.join(RESERVE)
    }
    pub fn stop_file(&self) -> PathBuf {
        self.root.join(STOP)
    }
    pub fn stdout_log(&self) -> PathBuf {
        self.root.join(STDOUT_LOG)
    }
    pub fn stderr_log(&self) -> PathBuf {
        self.root.join(STDERR_LOG)
    }
    pub fn sup_dir(&self) -> PathBuf {
        self.root.join(SUP_DIR)
    }
    pub fn table(&self) -> PathBuf {
        self.out_dir().join(TABLE)
    }
    pub fn queue(&self) -> PathBuf {
        self.out_dir().join(QUEUE)
    }
    pub fn cert(&self) -> PathBuf {
        self.out_dir().join(CERT)
    }
    /// Relative path (as recorded in OUTCOME) of a file in `out/`.
    pub fn out_rel(name: &str) -> String {
        format!("{OUT_DIR}/{name}")
    }
    /// Resolve a path recorded in OUTCOME (relative to RUN unless absolute).
    pub fn resolve(&self, p: &str) -> PathBuf {
        let p = Path::new(p);
        if p.is_absolute() { p.to_path_buf() } else { self.root.join(p) }
    }
}
