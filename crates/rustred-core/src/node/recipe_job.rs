//! The child-job contract (`rustred.child-job.v1`) pieces the verb `rr_node solve-recipe --job RUN/job.json` writes
//! itself when it runs stand-alone (without the campaign's `rr_node job` entry): the domain-separated digests
//! (spec, canonical stream), the canonical record form, `job.effective` with its digest, the strict job-spec reader
//! with the contract's pre-flight order (unknown fields 40, own digest / features / gated environment 42, input
//! digests 41), the no-clobber OUTCOME write and `version --json`.
//!
//! Mirrors the contract crate `rustred-campaign-contract` (campaign branch 995d41a1: canon.rs, spec.rs,
//! preflight.rs, outcome.rs) without depending on it: same contexts, same canonical form for the records this verb
//! writes, same refusal codes. The test vectors below are values computed by that crate.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

/// Contract name (`version --json` `contract`, spec `schema`).
pub const CONTRACT: &str = "rustred.child-job.v1";
/// OUTCOME `job.schema`.
pub const OUTCOME_SCHEMA: &str = "rustred.child-job.outcome.v1";
/// BLAKE3 key-derivation context of the job-spec digest (`spec_blake3`).
pub const CONTEXT_SPEC: &str = "rustred.child-job.v1 2026-11-14 job spec digest";
/// BLAKE3 key-derivation context of a canonical-stream digest (`canonical_blake3`).
pub const CONTEXT_CANONICAL_STREAM: &str =
    "rustred.child-job.v1 2026-11-14 canonical stream digest";
/// BLAKE3 key-derivation context of the per-record digest.
pub const CONTEXT_RECORD: &str = "rustred.child-job.v1 2026-11-14 record digest";
/// BLAKE3 key-derivation context of `job.effective.blake3` (contract outcome.rs `CONTEXT_EFFECTIVE`; contract owner's
/// ruling 2026-11-14, `EFFECTIVE_DIGEST_VERSION` 2).
pub const CONTEXT_EFFECTIVE: &str = "rustred.child-job.v1 2026-11-14 effective digest";
/// The program name of the spec (`program.name`).
pub const PROGRAM: &str = "rr_node";
/// Features of THIS build in job mode (kind solve-recipe only; `version --json`, `features_required`).
pub const JOB_FEATURES: [&str; 3] = ["solve_recipe", "signals", "heartbeat"];
/// Job kinds this build runs.
pub const JOB_KINDS: [&str; 1] = ["solve-recipe"];
/// Volatile top-level keys by record type, for the record types of `out/results.jsonl` (canonical form v4).
const VOLATILE: [(&str, &[&str]); 3] = [
    ("relation", &["seconds", "vmhwm_kb"]),
    ("end", &["seconds", "vmhwm_kb"]),
    ("partial", &["seconds", "vmhwm_kb"]),
];
/// Environment variables that change behaviour: refused unless in `env_allow` (contract 3.2.2).
pub const GATED_ENV: [&str; 6] = [
    "RUSTRED_PROBE_POINT",
    "RUSTRED_TRACE_DUMP",
    "RUSTRED_TRACE_DUMP_MIN_ROWS",
    "RUSTRED_TRACE_DUMP_EXIT",
    "RR_REPLAY_RECENTER_SEARCH",
    "RR_REPLAY_RECENTER_LIMIT",
];
const TYPED_FIELD_FLAGS: [&str; 8] = [
    "--family",
    "--out",
    "--table",
    "--initial",
    "--initial-presented",
    "--cover",
    "--case-budget",
    "--max-cases",
];
const REFUSED_FLAGS: [&str; 4] = [
    "--registry",
    "--pool",
    "--unsafe-shared-registry",
    "--lease-wait",
];

/// `spec_blake3`: BLAKE3 of the job.json bytes in the [`CONTEXT_SPEC`] domain.
pub fn spec_blake3(bytes: &[u8]) -> String {
    let mut h = blake3::Hasher::new_derive_key(CONTEXT_SPEC);
    h.update(bytes);
    h.finalize().to_hex().to_string()
}

/// The canonical form of a JSON value: members sorted bytewise by key at every depth, no whitespace, serde_json's
/// string escaping and number spelling (integers exact, floats as ryu writes them).
pub fn canonical(v: &Value) -> String {
    let mut s = String::new();
    write_canonical(v, &mut s);
    s
}

fn write_canonical(v: &Value, s: &mut String) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            s.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&serde_json::to_string(k).unwrap_or_default());
                s.push(':');
                write_canonical(&m[k], s);
            }
            s.push('}');
        }
        Value::Array(a) => {
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                write_canonical(x, s);
            }
            s.push(']');
        }
        other => s.push_str(&serde_json::to_string(other).unwrap_or_default()),
    }
}

/// The canonical line of one record of `out/results.jsonl`: [`canonical`] without the volatile top-level keys of
/// its type.
pub fn canonical_record(v: &Value) -> String {
    let t = v.get("type").and_then(Value::as_str);
    let vol: &[&str] = VOLATILE
        .iter()
        .find(|(n, _)| Some(*n) == t)
        .map(|(_, k)| *k)
        .unwrap_or(&[]);
    match v {
        Value::Object(m) if !vol.is_empty() => {
            let mut m = m.clone();
            for k in vol {
                m.remove(*k);
            }
            canonical(&Value::Object(m))
        }
        _ => canonical(v),
    }
}

/// `canonical_blake3` of a record stream: BLAKE3 in the [`CONTEXT_CANONICAL_STREAM`] domain over the canonical lines,
/// each followed by "\n", in file order.
pub fn canonical_stream_blake3<'a>(records: impl IntoIterator<Item = &'a Value>) -> String {
    let mut h = blake3::Hasher::new_derive_key(CONTEXT_CANONICAL_STREAM);
    for r in records {
        h.update(canonical_record(r).as_bytes());
        h.update(b"\n");
    }
    h.finalize().to_hex().to_string()
}

/// The behaviour-changing environment variables that are set (licence: presence only), as `job.effective.env`.
pub fn effective_env() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for k in GATED_ENV.iter().chain(&[
        "RUSTRED_NODE_PROGRESS",
        "RAYON_NUM_THREADS",
        "OMP_NUM_THREADS",
    ]) {
        if let Ok(v) = std::env::var(k) {
            m.insert(k.to_string(), v);
        }
    }
    if std::env::var_os("SYMBOLICA_LICENSE").is_some() {
        m.insert("SYMBOLICA_LICENSE".into(), "<present>".into());
    }
    m
}

/// `job.effective` with its digest (contract outcome.rs `Effective::digest`, version 2): `blake3` = BLAKE3 in the
/// [`CONTEXT_EFFECTIVE`] domain over the canonical JSON line of exactly `{subcommand,args,node_options,order,env}`
/// (`blake3` itself is not hashed).
pub fn effective(args: &Value, node_options: Value, order: Value) -> Value {
    let mut e = json!({"subcommand": "solve-recipe", "args": args, "node_options": node_options,
        "order": order, "env": effective_env()});
    let d = effective_digest(&e);
    e["blake3"] = json!(d);
    e
}

/// [`CONTEXT_EFFECTIVE`] digest of the five members of an effective object (any other member is ignored).
pub fn effective_digest(e: &Value) -> String {
    let five = json!({"subcommand": e["subcommand"], "args": e["args"], "node_options": e["node_options"],
        "order": e["order"], "env": e["env"]});
    let mut h = blake3::Hasher::new_derive_key(CONTEXT_EFFECTIVE);
    h.update(canonical(&five).as_bytes());
    h.finalize().to_hex().to_string()
}

/// The one line of `rr_node version --json` (contract 3.2.1).
pub fn version_json() -> Value {
    json!({"program": PROGRAM, "package_version": env!("CARGO_PKG_VERSION"),
        "git_commit": git_commit(), "contract": [CONTRACT], "features": JOB_FEATURES,
        "job_kinds": JOB_KINDS, "arities": [5, 9, 12, 14, 20]})
}

/// Build commit (`RUSTRED_BUILD_GIT_COMMIT` at build time; "unknown" when empty or absent).
pub fn git_commit() -> String {
    match option_env!("RUSTRED_BUILD_GIT_COMMIT").map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => "unknown".into(),
    }
}

/// `job.program`.
pub fn program_info() -> Value {
    json!({"name": PROGRAM, "version": env!("CARGO_PKG_VERSION"), "git_commit": git_commit(),
        "features": JOB_FEATURES, "exe": exe_info()})
}

/// `job.program.exe`.
pub fn exe_info() -> Value {
    let Ok(p) = std::fs::read_link("/proc/self/exe") else {
        return Value::Null;
    };
    match std::fs::metadata(&p) {
        Ok(m) => {
            json!({"path": p.display().to_string(), "dev": m.dev(), "ino": m.ino(), "size": m.size(),
            "mtime": m.mtime()})
        }
        Err(_) => Value::Null,
    }
}

/// Write `bytes` at `target` without ever replacing a file there (temporary file + hard link).
pub fn write_new(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = target.with_extension(format!("part.{}", std::process::id()));
    {
        let mut f = create_excl(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    let r = std::fs::hard_link(&tmp, target);
    let _ = std::fs::remove_file(&tmp);
    r
}

/// Create a new file: O_EXCL | O_NOFOLLOW (never an existing file, never through a symlink).
pub fn create_excl(p: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(p)
}

// ---------------------------------------------------------------------------------------------------------------
// The job spec (strict: every object refuses unknown members, as the contract's `JobSpec`).

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSpec {
    pub schema: String,
    pub job_id: String,
    pub attempt: u32,
    pub kind: String,
    pub run_dir: String,
    pub program: ProgramRef,
    #[serde(default)]
    pub inputs: Vec<InputRef>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub seed: SeedSpec,
    #[serde(default)]
    pub budgets: Budgets,
    #[serde(default = "d_stop")]
    pub on_case_budget: String,
    #[serde(default = "d_recorded")]
    pub determinism: String,
    #[serde(default)]
    pub output: OutputOptions,
    #[serde(default = "d_heartbeat")]
    pub heartbeat_s: u32,
    #[serde(default = "d_grace")]
    pub stop_grace_s: u32,
    #[serde(default = "d_resume_every")]
    pub resume_every_s: u32,
    #[serde(default)]
    pub resume: Option<Value>,
    #[serde(default)]
    pub env_allow: Vec<String>,
    #[serde(default)]
    pub fake: Option<Value>,
}

fn d_stop() -> String {
    "stop".into()
}
fn d_recorded() -> String {
    "recorded".into()
}
fn d_heartbeat() -> u32 {
    10
}
fn d_grace() -> u32 {
    20
}
fn d_resume_every() -> u32 {
    30
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramRef {
    pub name: String,
    pub blake3: String,
    #[serde(default)]
    pub features_required: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputRef {
    pub role: String,
    pub path: String,
    pub blake3: String,
    #[serde(default)]
    pub upto: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedSpec {
    #[serde(default)]
    pub sample_seed: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    #[serde(default)]
    pub work: WorkBudgets,
    #[serde(default)]
    pub wall: WallBudgets,
    #[serde(default)]
    pub mem_soft_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkBudgets {
    pub cases: Option<u64>,
    pub seeds_per_case: Option<u64>,
    pub seeds_total: Option<u64>,
    pub rows_per_case: Option<u64>,
    pub rows_total: Option<u64>,
    pub probe_evals_per_case: Option<u64>,
    pub reducer_rows_per_case: Option<u64>,
    pub trace_rows_max: Option<u64>,
    pub lift_monomials_total: Option<u64>,
    pub rhs_terms_total: Option<u64>,
    pub numeric_points_total: Option<u64>,
}

impl WorkBudgets {
    fn any(&self) -> bool {
        [
            self.cases,
            self.seeds_per_case,
            self.seeds_total,
            self.rows_per_case,
            self.rows_total,
            self.probe_evals_per_case,
            self.reducer_rows_per_case,
            self.trace_rows_max,
            self.lift_monomials_total,
            self.rhs_terms_total,
            self.numeric_points_total,
        ]
        .iter()
        .any(Option::is_some)
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallBudgets {
    #[serde(default)]
    pub case_s: Option<u64>,
    #[serde(default)]
    pub job_s: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputOptions {
    #[serde(default)]
    pub elide_witness_loci: bool,
}

/// A refused start: exit code, stop code, error class, message; `outcome` = the spec and its digest when an error
/// OUTCOME may be written (only once the run directory is verified to be this attempt's).
#[derive(Debug, Clone)]
pub struct Refusal {
    pub code: i32,
    pub message: String,
    pub outcome: Option<(Box<JobSpec>, String)>,
}

impl Refusal {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Refusal {
            code,
            message: message.into(),
            outcome: None,
        }
    }

    /// The stop code and error class of the exit code.
    pub fn stop_and_class(code: i32) -> (&'static str, &'static str) {
        match code {
            40 => ("refused", "usage"),
            41 => ("input_error", "input"),
            42 => ("refused", "refused"),
            _ => ("internal_error", "internal"),
        }
    }

    /// Report on stderr, write the error OUTCOME when allowed (no-clobber) and exit with the code.
    pub fn exit(self) -> ! {
        eprintln!("rr_node solve-recipe: {}", self.message);
        if let Some((spec, digest)) = &self.outcome {
            let _ = error_outcome(spec, digest, self.code, &self.message, Value::Null);
        }
        std::process::exit(self.code)
    }
}

/// Write an error OUTCOME (`RUN/OUTCOME.json`, no-clobber) for `spec`: status refused / error, stop and error class
/// from the exit code, no outputs.
pub fn error_outcome(
    spec: &JobSpec,
    spec_digest: &str,
    code: i32,
    message: &str,
    inputs: Value,
) -> std::io::Result<()> {
    let job = json!({"job_id": spec.job_id, "attempt": spec.attempt, "spec_blake3": spec_digest,
        "inputs": if inputs.is_null() { json!([]) } else { inputs }});
    error_outcome_job(&job, Path::new(&spec.run_dir), code, message)
}

/// [`error_outcome`] from the verb's job block (`job_id`, `attempt`, `spec_blake3`, `inputs`). Exit codes: 40 / 42
/// refused, 41 input, 50 nospace, 51 io, else internal.
pub fn error_outcome_job(
    job: &Value,
    run_dir: &Path,
    code: i32,
    message: &str,
) -> std::io::Result<()> {
    let (stop, class) = match code {
        50 => ("nospace", "nospace"),
        51 => ("io_error", "io"),
        c => Refusal::stop_and_class(c),
    };
    let status = match code {
        40 | 42 => "refused",
        _ => "error",
    };
    let msg: String = message.chars().take(4000).collect();
    let o = json!({"job": {"schema": OUTCOME_SCHEMA, "job_id": job["job_id"], "attempt": job["attempt"],
        "kind": "solve-recipe", "spec_blake3": job["spec_blake3"], "status": status, "exit_code": code,
        "stop": {"code": stop, "detail": Value::Null, "source": Value::Null, "graceful": true},
        "program": program_info(), "effective": Value::Null,
        "seed": {"sample_seed": 0, "point_source": "seed"},
        "inputs": if job["inputs"].is_array() { job["inputs"].clone() } else { json!([]) },
        "error": {"class": class, "message": msg}}});
    write_new(
        &run_dir.join("OUTCOME.json"),
        (serde_json::to_string_pretty(&o).unwrap_or_default() + "\n").as_bytes(),
    )
}

/// Prefix of the verb's errors raised while sealing its outputs (I/O: exit 51 in job mode).
pub const IO_ERROR_PREFIX: &str = "io: ";

/// A spec that passed the pre-flight.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub spec: JobSpec,
    pub spec_blake3: String,
    pub run_dir: PathBuf,
    /// role -> absolute path
    pub inputs: BTreeMap<String, String>,
    /// `job.inputs` echo (paths as in the spec, RUN-relative when relative)
    pub echo: Value,
}

fn own_uid() -> Option<u32> {
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

/// Step 4: the run directory is this attempt's and unused.
fn verify_run_dir(root: &Path, digest: &str) -> Result<(), String> {
    let md = std::fs::symlink_metadata(root).map_err(|e| format!("run_dir: {e}"))?;
    if !md.file_type().is_dir() {
        return Err("run_dir is not a directory".into());
    }
    if own_uid() != Some(md.uid()) {
        return Err("run_dir is not owned by this user".into());
    }
    let jm =
        std::fs::symlink_metadata(root.join("job.json")).map_err(|e| format!("job.json: {e}"))?;
    if !jm.file_type().is_file() {
        return Err("RUN/job.json is not a regular file".into());
    }
    let job = std::fs::read(root.join("job.json")).map_err(|e| format!("job.json: {e}"))?;
    if spec_blake3(&job) != digest {
        return Err("RUN/job.json is not the spec file that was read".into());
    }
    if std::fs::symlink_metadata(root.join("OUTCOME.json")).is_ok() {
        return Err(
            "OUTCOME.json already exists (a finished job is final; nothing written)".into(),
        );
    }
    for f in ["EVENTS.jsonl", "RESUME.json", ".reserve"] {
        if std::fs::symlink_metadata(root.join(f)).is_ok() {
            return Err(format!("{f} exists: the run directory was already started"));
        }
    }
    for sub in ["", "out", "tmp", "in"] {
        let dir = if sub.is_empty() {
            root.to_path_buf()
        } else {
            root.join(sub)
        };
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !sub.is_empty() => continue,
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        };
        if !sub.is_empty() && !std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_dir())
        {
            return Err(format!("{} is not a directory", dir.display()));
        }
        for e in rd {
            let e = e.map_err(|e| e.to_string())?;
            if e.file_type().map_err(|e| e.to_string())?.is_symlink() {
                return Err(format!(
                    "{} is a symlink (refused in a run directory)",
                    e.path().display()
                ));
            }
            if sub == "out" {
                return Err("out/ is not empty".into());
            }
        }
    }
    Ok(())
}

/// Step 5: the spec's own consistency for kind solve-recipe (`Err((code, message))`, 40 or 42).
fn validate(s: &JobSpec) -> Result<(), (i32, String)> {
    let bad = |m: String| Err((40, m));
    if s.schema != CONTRACT {
        return bad(format!("schema {:?} (want {CONTRACT})", s.schema));
    }
    if s.kind != "solve-recipe" {
        return Err((
            42,
            format!("kind {:?}: this verb runs kind solve-recipe only", s.kind),
        ));
    }
    if s.job_id.is_empty() || s.attempt == 0 {
        return bad("job_id must be non-empty and attempt >= 1".into());
    }
    if s.fake.is_some() {
        return bad("a fake script on a non-fake kind".into());
    }
    if s.resume.is_some() {
        return bad("kind solve-recipe has no resume (rerun from scratch)".into());
    }
    if s.budgets.work.any() {
        return bad(
            "budgets.work: kind solve-recipe has no work counters (use --max-trace-rows)".into(),
        );
    }
    if s.budgets.mem_soft_bytes.is_some() {
        return bad("budgets.mem_soft_bytes: not implemented by kind solve-recipe".into());
    }
    if s.budgets.wall.job_s.is_some() {
        return bad(
            "budgets.wall.job_s: kind solve-recipe is bounded by the supervisor's wall cap".into(),
        );
    }
    if s.budgets.wall.case_s == Some(0) {
        return bad("budgets.wall: a zero limit is not allowed".into());
    }
    if s.seed.sample_seed != 0 {
        return bad("seed.sample_seed: kind solve-recipe fixes the sample seed 0".into());
    }
    if s.on_case_budget != "stop" {
        return bad(format!(
            "on_case_budget {:?}: kind solve-recipe: stop only",
            s.on_case_budget
        ));
    }
    if !["recorded", "strict"].contains(&s.determinism.as_str()) {
        return bad(format!("determinism {:?}", s.determinism));
    }
    if s.heartbeat_s == 0 || s.stop_grace_s == 0 {
        return bad("heartbeat_s and stop_grace_s: minimum 1".into());
    }
    for e in &s.env_allow {
        let ok = !e.is_empty()
            && !e.as_bytes()[0].is_ascii_digit()
            && e.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        if !ok {
            return bad(format!(
                "env_allow: {e:?} is not an environment variable name"
            ));
        }
    }
    // args: `--flag VALUE` pairs from the allow-list, each once
    let mut seen: Vec<&str> = Vec::new();
    let mut k = 0;
    while k < s.args.len() {
        let tok = s.args[k].as_str();
        let f = tok.split('=').next().unwrap_or(tok);
        if REFUSED_FLAGS.contains(&f) {
            return Err((
                42,
                format!("{f} writes outside the run directory (refused in job mode)"),
            ));
        }
        if TYPED_FIELD_FLAGS.contains(&f) {
            return bad(format!("{f} is a typed field of the spec, not an arg"));
        }
        if !tok.starts_with("--") {
            return bad(format!("arg not allowed: positional {tok:?}"));
        }
        if tok.contains('=') {
            return bad(format!(
                "arg not allowed: {tok:?}: write `{f} VALUE` as two tokens"
            ));
        }
        let uint = match tok {
            "--order" => false,
            "--threads" | "--max-trace-rows" | "--key-budget-s" => true,
            _ => {
                return bad(format!(
                    "arg not allowed: {tok} for kind solve-recipe (--order, --threads, --max-trace-rows, --key-budget-s)"
                ));
            }
        };
        if seen.contains(&tok) {
            return bad(format!("arg not allowed: {tok} repeated"));
        }
        seen.push(tok);
        let Some(v) = s.args.get(k + 1) else {
            return bad(format!("arg not allowed: {tok} needs a value"));
        };
        if uint && !(v.len() <= 20 && !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit())) {
            return bad(format!("arg {tok} {v:?}: an unsigned integer"));
        }
        if uint && v.parse::<u64>().ok() == Some(0) && tok != "--max-trace-rows" {
            return bad(format!("arg {tok} 0: minimum 1"));
        }
        k += 2;
    }
    if !seen.contains(&"--order") {
        return bad("args: kind solve-recipe needs --order".into());
    }
    let key_budget = seen.contains(&"--key-budget-s");
    if key_budget && s.budgets.wall.case_s.is_some() {
        return bad("budgets.wall.case_s: the per-key wall budget is given twice (--key-budget-s and budgets.wall.case_s)".into());
    }
    if s.determinism == "strict" && (key_budget || s.budgets.wall.case_s.is_some()) {
        return bad("determinism strict: a per-key wall budget is a clock decision".into());
    }
    // input roles: family, recipe, keys required, expect optional, each once
    for i in &s.inputs {
        if !["family", "recipe", "keys", "expect"].contains(&i.role.as_str()) {
            return bad(format!(
                "inputs: role {:?} is not an input of kind solve-recipe",
                i.role
            ));
        }
    }
    for role in ["family", "recipe", "keys", "expect"] {
        let n = s.inputs.iter().filter(|i| i.role == role).count();
        if n > 1 {
            return bad(format!("inputs: role {role:?} listed {n} times"));
        }
        if n == 0 && role != "expect" {
            return bad(format!("inputs: role {role:?} is required"));
        }
    }
    for i in &s.inputs {
        if i.upto.is_some() {
            return bad(format!(
                "inputs: role {:?}: upto is not used by kind solve-recipe",
                i.role
            ));
        }
        if !(i.blake3.len() == 64
            && i.blake3
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        {
            return bad(format!(
                "inputs: role {:?}: blake3 is not 64 lower-case hex",
                i.role
            ));
        }
    }
    Ok(())
}

/// The pre-flight of the contract, in its order: 1 read (40), 2 strict parse (40), 3 `RUN/job.json` with an absolute
/// `run_dir` that is its directory (42), 4 the run directory is ours and unused (42) -- nothing written so far --,
/// then with an error OUTCOME: 5 validate (40 / 42), 6 program name and own BLAKE3 (42), 7 `features_required` (42),
/// 8 gated environment variables not in `env_allow` (42), 10 every input exists with its BLAKE3 (41).
pub fn preflight(
    spec_path: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Prepared, Refusal> {
    let bytes = std::fs::read(spec_path).map_err(|e| {
        Refusal::new(
            40,
            format!("cannot read job spec {}: {e}", spec_path.display()),
        )
    })?;
    let digest = spec_blake3(&bytes);
    let spec: JobSpec =
        serde_json::from_slice(&bytes).map_err(|e| Refusal::new(40, format!("job spec: {e}")))?;
    if spec_path.file_name().and_then(|n| n.to_str()) != Some("job.json") {
        return Err(Refusal::new(42, "the job spec file must be named job.json"));
    }
    let run_dir = PathBuf::from(&spec.run_dir);
    if !run_dir.is_absolute() {
        return Err(Refusal::new(42, "run_dir is not absolute"));
    }
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let spec_dir = canon(spec_path.parent().unwrap_or(Path::new(".")));
    if canon(&run_dir) != spec_dir {
        return Err(Refusal::new(
            42,
            format!(
                "run_dir {} is not the directory of the spec ({})",
                spec.run_dir,
                spec_dir.display()
            ),
        ));
    }
    verify_run_dir(&run_dir, &digest).map_err(|m| Refusal::new(42, m))?;
    let with = |code: i32, m: String| Refusal {
        code,
        message: m,
        outcome: Some((Box::new(spec.clone()), digest.clone())),
    };
    validate(&spec).map_err(|(c, m)| with(c, m))?;
    if spec.program.name != PROGRAM {
        return Err(with(
            42,
            format!(
                "program {PROGRAM} does not run kind solve-recipe of program {}",
                spec.program.name
            ),
        ));
    }
    let own = std::fs::read("/proc/self/exe")
        .map(|b| blake3::hash(&b).to_hex().to_string())
        .map_err(|e| with(70, format!("cannot read the own executable: {e}")))?;
    if own != spec.program.blake3 {
        return Err(with(
            42,
            "the running binary's BLAKE3 is not spec.program.blake3".into(),
        ));
    }
    let missing: Vec<&str> = spec
        .program
        .features_required
        .iter()
        .map(String::as_str)
        .filter(|f| !JOB_FEATURES.contains(f))
        .collect();
    if !missing.is_empty() {
        return Err(with(
            42,
            format!("features not supported: {}", missing.join(", ")),
        ));
    }
    for v in GATED_ENV {
        if env(v).is_some() && !spec.env_allow.iter().any(|a| a == v) {
            return Err(with(
                42,
                format!("environment variable {v} is set but not in env_allow"),
            ));
        }
    }
    let mut inputs = BTreeMap::new();
    let mut echo = Vec::new();
    for i in &spec.inputs {
        let p = if Path::new(&i.path).is_absolute() {
            PathBuf::from(&i.path)
        } else {
            run_dir.join(&i.path)
        };
        let b = std::fs::read(&p)
            .map_err(|e| with(41, format!("input {:?} ({}): {e}", i.role, p.display())))?;
        let d = blake3::hash(&b).to_hex().to_string();
        if d != i.blake3 {
            return Err(with(
                41,
                format!("input {:?} ({}) digest mismatch", i.role, p.display()),
            ));
        }
        echo.push(json!({"role": i.role, "path": i.path, "bytes": b.len(), "blake3": d, "verified": true}));
        inputs.insert(i.role.clone(), p.display().to_string());
    }
    Ok(Prepared {
        spec,
        spec_blake3: digest,
        run_dir,
        inputs,
        echo: Value::Array(echo),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors computed with the contract crate rustred-campaign-contract (campaign 995d41a1):
    // spec::spec_blake3, canon::CanonicalDigest (push_line per line), blake3_hex(canonical_line(..)).
    const SPEC_BYTES: &[u8] = b"{\"schema\":\"rustred.child-job.v1\",\"job_id\":\"j1\"}\n";
    const STREAM: &str = concat!(
        "{\"type\":\"relation\",\"ordinal\":0,\"key\":[1,-2,0],\"verdict\":\"SOLVED\",\"seconds\":1.25,\"vmhwm_kb\":77,\"z\":{\"b\":1,\"a\":[0.5,\"\\u0001\\n\"]}}\n",
        "{\"type\":\"end\",\"records\":1,\"counts\":{\"SOLVED\":1},\"recipe_digest\":\"x\",\"determinism_id\":\"y\"}\n"
    );

    #[test]
    fn contract_test_vectors() {
        assert_eq!(
            spec_blake3(SPEC_BYTES),
            "4386ca36567851b3192ff2f1a798402675031f1b872515e7b6d99873cdef8a96"
        );
        let recs: Vec<Value> = STREAM
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(
            canonical_stream_blake3(&recs),
            "635b662c52e402afedfffc9c2948aa88480f0285668e2bb32ef1ad628876e762"
        );
        assert_eq!(
            canonical_record(&recs[0]),
            r#"{"key":[1,-2,0],"ordinal":0,"type":"relation","verdict":"SOLVED","z":{"a":[0.5,"\u0001\n"],"b":1}}"#
        );
        let e = json!({"subcommand":"solve-recipe","args":["--order","id"],"node_options":{"b":2,"a":1},
            "order":null,"env":{}});
        assert_eq!(
            blake3::hash(canonical(&e).as_bytes()).to_hex().to_string(),
            "0eb6af6ae058bfb993267685545f53b84cdc71c3423e910ae586e2dc42121c3f"
        );
    }

    #[test]
    fn effective_digest_is_the_contract_version_2_vector() {
        // contract crate tests/effective_digest.rs (lane engfix4, outcome.rs Effective::digest, version 2)
        let mut e = json!({"subcommand":"solve-recipe","args":["--order","id"],
            "node_options":{"threads":1,"proof":true,"relation_rule":"m1_first","key_budget_s":null,
                "max_trace_rows":null,"recipe_digest":"abc","determinism_id":"d"},
            "order":{"name":"id"},"env":{"SYMBOLICA_LICENSE":"<present>"}});
        let d = effective_digest(&e);
        assert_eq!(d, "04d5bb45173ff273bc02b20999b0c851f668226c22b5abafc519a2c27bb05012");
        // `blake3` (and any other extra member) is not hashed
        e["blake3"] = json!("f".repeat(64));
        assert_eq!(effective_digest(&e), d);
        // version 1 (plain BLAKE3 of the canonical JSON) differs
        e.as_object_mut().unwrap().remove("blake3");
        assert_ne!(blake3::hash(canonical(&e).as_bytes()).to_hex().to_string(), d);
        // the sealing function writes exactly this digest
        let s = effective(&json!(["--order", "id"]), json!({"threads": 1}), json!({"name": "id"}));
        assert_eq!(s["blake3"], json!(effective_digest(&s)));
    }

    #[test]
    fn strict_spec_refuses_unknown_fields() {
        let ok = r#"{"schema":"rustred.child-job.v1","job_id":"j","attempt":1,"kind":"solve-recipe","run_dir":"/x",
            "program":{"name":"rr_node","blake3":"00"}}"#;
        assert!(serde_json::from_str::<JobSpec>(ok).is_ok());
        let unknown = ok.replace("\"attempt\":1", "\"attempt\":1,\"bogus\":true");
        assert!(serde_json::from_str::<JobSpec>(&unknown).is_err());
        let nested = ok.replace("\"blake3\":\"00\"", "\"blake3\":\"00\",\"x\":1");
        assert!(serde_json::from_str::<JobSpec>(&nested).is_err());
        let float = ok.replace(
            "\"attempt\":1",
            "\"attempt\":1,\"budgets\":{\"wall\":{\"case_s\":1.5}}",
        );
        assert!(serde_json::from_str::<JobSpec>(&float).is_err());
    }
}
