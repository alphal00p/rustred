//! One child process of the emitter (the producer's lift, the independent checker): own process group, stdout / stderr
//! to files, a WALL BUDGET (on overrun: SIGTERM then SIGKILL to the process group this module created, never anything
//! else), the child's own peak RSS from `wait4` (ru_maxrss of that child). Runs inside the caller's namespace and memory
//! scope (L10: the emitter is started in a private pid namespace under a systemd unit with its MemoryMax).
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// What one run gave.
#[derive(Clone, Debug)]
pub struct ChildRun {
    /// exit code, None when killed by a signal
    pub code: Option<i32>,
    pub seconds: f64,
    pub peak_mb: f64,
    pub over_budget: bool,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

impl ChildRun {
    pub fn ok(&self) -> bool {
        self.code == Some(0) && !self.over_budget
    }
    pub fn stdout_text(&self) -> String {
        std::fs::read_to_string(&self.stdout).unwrap_or_default()
    }
    pub fn stderr_tail(&self, n: usize) -> String {
        let t = std::fs::read_to_string(&self.stderr).unwrap_or_default();
        let mut k = t.len().saturating_sub(n);
        while !t.is_char_boundary(k) {
            k += 1;
        }
        t[k..].to_string()
    }
}

/// Environment variable naming a LICENCE-SLOT WRAPPER (e.g. implementer_ops/sym_slot_wait.sh): when set, every child
/// runs as `<wrapper> bash -c 'touch "$RR_SLOT_MARK"; exec "$@"' _ <program> <args>`, so each Symbolica-linked step
/// takes a slot of its own (SYMBOLICA_SLOTS.md, addition 2026-11-18T21:55Z) and the wall budget starts when the slot is
/// held (the mark file appears), not while the step waits for it.
pub const SLOT_WRAPPER_ENV: &str = "RR_EMISSION_SLOT_WRAPPER";

/// Run `program args` in `dir` with extra environment `env`; stdout / stderr to `<dir>/<name>.out|.err`.
pub fn run(
    program: &Path,
    args: &[String],
    env: &[(String, String)],
    dir: &Path,
    name: &str,
    budget_s: f64,
) -> Result<ChildRun, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let out = dir.join(format!("{name}.out"));
    let err = dir.join(format!("{name}.err"));
    let fo = std::fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let fe = std::fs::File::create(&err).map_err(|e| format!("{}: {e}", err.display()))?;
    let wrapper = std::env::var(SLOT_WRAPPER_ENV)
        .ok()
        .filter(|w| !w.is_empty());
    let mark = dir.join(format!("{name}.slot"));
    let _ = std::fs::remove_file(&mark);
    let mut cmd = match &wrapper {
        Some(w) => {
            let mut c = Command::new(w);
            c.arg("bash")
                .arg("-c")
                .arg("touch \"$RR_SLOT_MARK\"; exec \"$@\"")
                .arg("_")
                .arg(program)
                .args(args)
                .env("RR_SLOT_MARK", &mark);
            c
        }
        None => {
            let mut c = Command::new(program);
            c.args(args);
            c
        }
    };
    cmd.current_dir(dir)
        .stdin(Stdio::null())
        .stdout(fo)
        .stderr(fe)
        .process_group(0);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut t0 = Instant::now();
    let mut started = wrapper.is_none();
    let child = cmd
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", program.display()))?;
    let pid = child.id() as libc::pid_t;
    let mut over = false;
    let mut term_at: Option<Instant> = None;
    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: rusage is plain data; wait4 on our own child pid
        let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
        let r = unsafe { libc::wait4(pid, &mut status, libc::WNOHANG, &mut ru) };
        if r == pid {
            let code = if libc::WIFEXITED(status) {
                Some(libc::WEXITSTATUS(status))
            } else {
                None
            };
            return Ok(ChildRun {
                code,
                seconds: t0.elapsed().as_secs_f64(),
                peak_mb: ru.ru_maxrss.max(0) as f64 / 1024.0,
                over_budget: over,
                stdout: out,
                stderr: err,
            });
        }
        if r < 0 {
            return Err(format!("wait4 {pid}: {}", std::io::Error::last_os_error()));
        }
        if !started && mark.exists() {
            // the slot is held: the budget starts now
            t0 = Instant::now();
            started = true;
        }
        if started && !over && t0.elapsed().as_secs_f64() > budget_s {
            over = true;
            term_at = Some(Instant::now());
            // the process group THIS module created (process_group(0) above): never a foreign process
            unsafe { libc::kill(-pid, libc::SIGTERM) };
        }
        if let Some(t) = term_at
            && t.elapsed() > Duration::from_secs(10)
        {
            unsafe { libc::kill(-pid, libc::SIGKILL) };
            term_at = None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// sha256 of a file (hex). Files of 1 MiB or more (binaries) are memoised per process on (canonical path, size, mtime): an
/// emitter hashes its checker and its descent binary (hundreds of MB for a debug build) at every construction; a
/// rewritten file (other size or mtime) is hashed again.
pub fn file_sha256(p: &Path) -> Result<String, String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    type Key = (std::path::PathBuf, u64, Option<std::time::SystemTime>);
    static MEMO: OnceLock<Mutex<HashMap<Key, String>>> = OnceLock::new();
    let meta = std::fs::metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let key: Key = (
        std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()),
        meta.len(),
        meta.modified().ok(),
    );
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    // only large files (binaries) are memoised; certificates and other small files are always read
    let big = meta.len() >= 1 << 20;
    if big
        && key.2.is_some()
        && let Some(s) = memo.lock().unwrap_or_else(|e| e.into_inner()).get(&key)
    {
        return Ok(s.clone());
    }
    let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let s = rustred_campaign_core::emission::body::sha256_hex(&b);
    // the bytes read must still be the file the key names (a rewrite during the read is not memoised)
    if big && key.2.is_some() && b.len() as u64 == key.1 {
        memo.lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, s.clone());
    }
    Ok(s)
}
