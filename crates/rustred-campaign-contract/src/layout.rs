//! The run directory of one attempt (child-job.md 3.1.3) and every path inside it: the SINGLE source of these names
//! for all campaign crates (coordinator addendum 2). Attempt directories follow plan 3.3.0:
//! `WDIR/jobs/<job content id>.<rung>.a<attempt>/`, i.e. `jobs/<job_id>.a<attempt>` with `job_id = <content id>.<rung>`
//! ([`job_id`], [`attempt_dir_name`], [`attempt_rel`], [`attempt_dir`]).

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
/// `generate`: the job's open list at its end (sub-cases returned, open cases / numeric endpoints / regions), sealed
/// by an `end` record (lane eng2; `--descend open`).
pub const SUBCASES: &str = "F.subcases.jsonl";

/// The directory of all attempt directories under the work directory.
pub const JOBS_DIR: &str = "jobs";

/// The job id of a job of the campaign: `<job content id>.<rung>` (plan 3.3.0).
pub fn job_id(content_id: &str, rung: &str) -> String {
    format!("{content_id}.{rung}")
}

/// The name of an attempt directory: `<job_id>.a<attempt>`.
pub fn attempt_dir_name(job_id: &str, attempt: u32) -> String {
    format!("{job_id}.a{attempt}")
}

/// The attempt directory relative to the work directory: `jobs/<job_id>.a<attempt>`.
pub fn attempt_rel(job_id: &str, attempt: u32) -> String {
    format!("{JOBS_DIR}/{}", attempt_dir_name(job_id, attempt))
}

/// `WDIR/jobs/<job_id>.a<attempt>`.
pub fn attempt_dir(wdir: &Path, job_id: &str, attempt: u32) -> PathBuf {
    wdir.join(JOBS_DIR).join(attempt_dir_name(job_id, attempt))
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
    pub fn family(&self) -> PathBuf {
        self.out_dir().join(FAMILY)
    }
    pub fn refused(&self) -> PathBuf {
        self.out_dir().join(REFUSED)
    }
    pub fn spill(&self) -> PathBuf {
        self.out_dir().join(SPILL)
    }
    pub fn obligations(&self) -> PathBuf {
        self.out_dir().join(OBLIGATIONS)
    }
    pub fn witness(&self) -> PathBuf {
        self.out_dir().join(WITNESS)
    }
    pub fn loci(&self) -> PathBuf {
        self.out_dir().join(LOCI)
    }
    pub fn subcases(&self) -> PathBuf {
        self.out_dir().join(SUBCASES)
    }
    pub fn results(&self) -> PathBuf {
        self.out_dir().join(RESULTS)
    }
    /// Relative path (as recorded in OUTCOME) of a file in `out/`.
    pub fn out_rel(name: &str) -> String {
        format!("{OUT_DIR}/{name}")
    }
    /// A layout path relative to RUN (`out/<name>`, `OUTCOME.json`, ...) joined to the root. Paths recorded in
    /// OUTCOME are never resolved: they must EQUAL the layout path (audit A C2; [`crate::seal::open_in_run`]).
    pub fn at(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}
