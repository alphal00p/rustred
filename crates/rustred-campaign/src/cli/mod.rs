//! Library functions of the `rr_campaign` commands (interfaces/supervisor.md sections 9-12). Argument parsing is
//! hand-written like the rest of the engine (no clap). Every function returns the process exit code.

use crate::supervisor::exit;
use crate::supervisor::spec::CampaignSpec;
use crate::supervisor::{self, CONTROL_SCHEMA, ControlRequest, RunOptions};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const USAGE: &str = "usage:
  rr_campaign run --spec SPEC.json [--dir CDIR] [--plain] [--fake-child] [--accept-binary-change] [--adopt-moved-campaign] [--cores N] [--mem-total 30G] [--cpus LIST] [--max-jobs N] [--mem-cap SIZE] [--accept-truncation-to SEQ]
      (the live dashboard when stdout is a terminal and --plain is not given; Ctrl-C = drain, twice = force;
       --cpus / --max-jobs / --mem-cap: this SESSION's resources (launch-time; not part of the campaign's identity: a resume may use other CPUs))
  rr_campaign preflight --spec SPEC.json [--dir CDIR]
  rr_campaign watch CDIR [--plain]          (alias: dashboard; read-only viewer: q / Ctrl-C close it, the campaign is unaffected)
  rr_campaign status CDIR [--once] [--width N] [--height M] [--json]
  rr_campaign control CDIR pause [NODE|ITEM]|resume [NODE|ITEM]|stop|retry ITEM|skip ITEM|resolve JOB [--reason TEXT]
  rr_campaign control CDIR resolve-terminals [--node NODE] (--accept KEYS_FILE | --reject) [--reason TEXT]
      (terminal proposals of the closing loop: the listed keys are accepted as terminals, the other proposals rejected)
  rr_campaign verify CDIR
  rr_campaign result CDIR
  rr_campaign rebuild-manifest CDIR         (recovery: a ledger whose MANIFEST.json is missing; refused while a supervisor holds the lock)
  rr_campaign derivations --run-dir JOBDIR --engine RR_NODE --out OUT.jsonl
      (lane sizegen: the effective derivation of every rule a finished generate job sealed; TRICKS 31 / 35)
  rr_campaign poles-choose --choice CHOICE.json --scores SCORES.jsonl [--policy POLES.json] --out OUT.json
      (lane sizegen: the pole objective on a finished generation choice; pure, no campaign; interfaces/genchoice.md 6)";

/// `println!` without the panic on a closed / hung-up stdout (EPIPE, EIO): output is best effort (gate-B m4).
macro_rules! outln {
    ($($t:tt)*) => {{
        let _ = writeln!(std::io::stdout(), $($t)*);
    }};
}

/// `eprintln!` without the panic on a closed / hung-up stderr (gate-B m4: exit 101 when the terminal was gone).
macro_rules! errln {
    ($($t:tt)*) => {{
        let _ = writeln!(std::io::stderr(), $($t)*);
    }};
}

/// procfix5 (gate4 m1): set when this process's `main` dispatches the spawn-barrier verb (`main_with` does), so that a
/// supervisor running in it may start its children behind the barrier (`LaunchSpec::spawn_barrier`).
static BARRIER_HOSTED: AtomicBool = AtomicBool::new(false);

/// Does this process's binary run the spawn-barrier shim (see [`main_with`])?
pub fn barrier_hosted() -> bool {
    BARRIER_HOSTED.load(Ordering::SeqCst)
}

/// procfix5 (gate4 M2): the refusal of every verb started from an environment that carries the campaign marker
/// (`RR_CAMPAIGN_JOB`): such a process (and everything it starts) is "ours" by the ownership rule of the campaign the
/// marker names, so a supervisor started there would kill the operator's own processes (gate4 E11). `None` = may run.
pub fn marked_environment_refusal(marker: Option<&std::ffi::OsStr>) -> Option<String> {
    let v = marker?;
    Some(format!(
        "refused: this environment carries {}={} (a campaign job's marker; copied from a job's environment?). \
         Every process carrying it counts as that campaign's job and is killed by its supervisor, together with what it \
         started. Start rr_campaign from a clean shell (`env | grep {}` must print nothing), e.g. `env -u {} rr_campaign ...`",
        crate::resources::MARKER_VAR,
        v.to_string_lossy(),
        crate::resources::MARKER_VAR,
        crate::resources::MARKER_VAR
    ))
}

/// Dispatch `argv` (without the program name).
pub fn main_with(args: &[String]) -> i32 {
    BARRIER_HOSTED.store(true, Ordering::SeqCst);
    // The spawn-barrier shim runs INSIDE a job (it carries the job's marker by design) and only execs the job's
    // program after the supervisor released it: dispatched before the marked-environment refusal, nothing else allowed.
    if args.first().map(String::as_str) == Some(crate::launch::BARRIER_VERB) {
        return crate::launch::barrier_shim(&args[1..]);
    }
    if let Some(m) =
        marked_environment_refusal(std::env::var_os(crate::resources::MARKER_VAR).as_deref())
    {
        errln!("rr_campaign: {m}");
        return exit::REFUSED;
    }
    let Some(cmd) = args.first() else {
        errln!("{USAGE}");
        return exit::USAGE;
    };
    let rest = &args[1..];
    match cmd.as_str() {
        "run" => cmd_run(args, rest),
        "preflight" => cmd_preflight(rest),
        "control" => cmd_control(rest),
        "verify" => cmd_verify(rest),
        "status" => cmd_status(rest),
        "watch" | "dashboard" => cmd_watch(rest),
        "result" => cmd_result(rest),
        "rebuild-manifest" => cmd_rebuild_manifest(rest),
        "derivations" => match supervisor::genchoice::derivations_cmd(rest) {
            Ok(t) => {
                outln!("{t}");
                exit::OK
            }
            Err(e) => {
                errln!("derivations: {e}");
                exit::USAGE
            }
        },
        "poles-choose" => match supervisor::genchoice::poles_choose(rest) {
            Ok(t) => {
                outln!("{t}");
                exit::OK
            }
            Err(e) => {
                errln!("poles-choose: {e}");
                exit::USAGE
            }
        },
        "-h" | "--help" | "help" => {
            outln!("{USAGE}");
            exit::OK
        }
        other => {
            errln!("unknown command {other:?}\n{USAGE}");
            exit::USAGE
        }
    }
}

#[derive(Default)]
struct Flags {
    spec: Option<PathBuf>,
    dir: Option<PathBuf>,
    plain: bool,
    fake_child: bool,
    accept_binary_change: bool,
    adopt_moved: bool,
    once: bool,
    json: bool,
    width: Option<u16>,
    height: Option<u16>,
    reason: Option<String>,
    accept_truncation_to: Option<u64>,
    cores: Option<usize>,
    mem_total: Option<u64>,
    /// lane t266run: launch-time CPU list / job count of THIS session (not part of the campaign identity: a campaign
    /// resumes on other CPUs; the session's argv, recorded by `SupervisorStarted`, names them).
    cpus: Option<String>,
    max_jobs: Option<usize>,
    mem_cap: Option<String>,
    positional: Vec<String>,
    // lane closeloop4
    node: Option<String>,
    accept: Option<PathBuf>,
    reject: bool,
}

fn parse(rest: &[String]) -> Result<Flags, String> {
    let mut f = Flags::default();
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match a.as_str() {
            "--spec" => f.spec = Some(val("--spec")?.into()),
            "--dir" => f.dir = Some(val("--dir")?.into()),
            "--plain" => f.plain = true,
            "--fake-child" => f.fake_child = true,
            "--accept-binary-change" => f.accept_binary_change = true,
            "--adopt-moved-campaign" => f.adopt_moved = true,
            "--once" => f.once = true,
            "--json" => f.json = true,
            "--width" => f.width = Some(val("--width")?.parse().map_err(|_| "--width: a number")?),
            "--height" => {
                f.height = Some(val("--height")?.parse().map_err(|_| "--height: a number")?)
            }
            "--reason" => f.reason = Some(val("--reason")?),
            "--node" => f.node = Some(val("--node")?),
            "--accept" => f.accept = Some(val("--accept")?.into()),
            "--reject" => f.reject = true,
            "--accept-truncation-to" => {
                f.accept_truncation_to = Some(
                    val("--accept-truncation-to")?
                        .parse()
                        .map_err(|_| "--accept-truncation-to: a ledger seq")?,
                )
            }
            "--cores" => f.cores = Some(val("--cores")?.parse().map_err(|_| "--cores: a number")?),
            "--cpus" => f.cpus = Some(val("--cpus")?),
            "--mem-cap" => f.mem_cap = Some(val("--mem-cap")?),
            "--max-jobs" => {
                f.max_jobs = Some(
                    val("--max-jobs")?
                        .parse()
                        .map_err(|_| "--max-jobs: a number")?,
                )
            }
            "--mem-total" => {
                f.mem_total = Some(supervisor::spec::ByteSize::parse(&val("--mem-total")?)?.0)
            }
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            s => f.positional.push(s.to_string()),
        }
    }
    Ok(f)
}

fn load_spec(f: &Flags) -> Result<(CampaignSpec, String, PathBuf), (i32, String)> {
    let path = f
        .spec
        .as_ref()
        .ok_or((exit::USAGE, "--spec is required".to_string()))?;
    let (spec, digest) =
        CampaignSpec::read(path).map_err(|e| (exit::PF_SPEC, format!("spec: {e}")))?;
    let cdir = f
        .dir
        .clone()
        .or_else(|| spec.dirs.campaign_dir.clone())
        .ok_or((
            exit::USAGE,
            "no campaign directory: --dir or dirs.campaign_dir".to_string(),
        ))?;
    let cdir = if cdir.is_absolute() {
        cdir
    } else {
        std::env::current_dir()
            .map(|d| d.join(&cdir))
            .unwrap_or(cdir)
    };
    Ok((spec, digest, cdir))
}

fn cmd_run(argv: &[String], rest: &[String]) -> i32 {
    let f = match parse(rest) {
        Ok(f) => f,
        Err(e) => {
            errln!("{e}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let bytes_digest = f
        .spec
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .map(|b| blake3::hash(&b).to_hex().to_string());
    let (mut spec, digest, cdir) = match load_spec(&f) {
        Ok(x) => x,
        Err((c, m)) => {
            errln!("{m}");
            return c;
        }
    };
    // lane t266run: launch-time resources override the spec's for this session only (the digest is the spec FILE's)
    if let Some(c) = &f.cpus {
        if let Err(e) = crate::cpu_policy::CpuSpec::parse(c) {
            errln!("--cpus: {e}");
            return exit::USAGE;
        }
        spec.resources.cpus = Some(c.clone());
    }
    if let Some(m) = f.max_jobs {
        spec.resources.max_jobs = m.max(1);
    }
    // lane t266run (L17): the per-job memory cap is a launch-time resource too (never part of the campaign identity)
    if let Some(m) = &f.mem_cap {
        match crate::supervisor::spec::ByteSize::parse(m) {
            Ok(b) if b.0 > 0 => spec.resources.job_mem_cap = Some(b),
            Ok(_) => {
                errln!("--mem-cap: must be > 0");
                return exit::USAGE;
            }
            Err(e) => {
                errln!("--mem-cap: {e}");
                return exit::USAGE;
            }
        }
    }
    let mut full = vec!["rr_campaign".to_string()];
    full.extend(argv.iter().cloned());
    let dash_opts = crate::tui::DashboardOptions {
        plain: f.plain,
        ..Default::default()
    };
    // The dashboard needs a terminal on stdout; otherwise (or with --plain) the supervisor's plain log lines.
    let quiet = Arc::new(AtomicBool::new(false));
    let with_dashboard = !f.plain
        && std::io::stdout().is_terminal()
        && std::env::var("TERM").ok().as_deref() != Some("dumb");
    let done = Arc::new(AtomicBool::new(false));
    let mut dash: Option<std::sync::mpsc::Receiver<crate::tui::ExitReason>> = None;
    let opts = RunOptions {
        spec,
        spec_blake3: digest,
        spec_bytes_blake3: bytes_digest,
        cpus: f.cpus.clone(),
        cdir: cdir.clone(),
        plain: !with_dashboard,
        fake_child: f.fake_child,
        accept_binary_change: f.accept_binary_change,
        adopt_moved: f.adopt_moved,
        cores: f.cores,
        mem_total: f.mem_total,
        argv: full,
        tick: Duration::from_millis(200),
        accept_truncation_to: f.accept_truncation_to,
        quiet_stderr: Some(Arc::clone(&quiet)),
    };
    let r = supervisor::run_with(opts, &mut |started| {
        if !with_dashboard {
            return;
        }
        // The dashboard runs on its OWN thread (its file reading on its reader thread): the control loop is never
        // blocked by rendering. Ctrl-C in raw mode = `request_stop()` on the supervisor's stop handle.
        quiet.store(true, Ordering::SeqCst);
        let controls = crate::tui::SupervisorControls {
            handle: started.handle.clone(),
            grace_s: started.grace_s,
            done: Arc::clone(&done),
        };
        let source: Arc<dyn crate::tui::ProgressSource> =
            Arc::new(crate::tui::DirSource::new(cdir.clone()));
        let o = dash_opts.clone();
        let q = Arc::clone(&quiet);
        let (tx, rx) = std::sync::mpsc::channel();
        if std::thread::Builder::new()
            .name("dashboard".into())
            .spawn(move || {
                let reason = crate::tui::run_dashboard(source, &controls, &o);
                // Terminal restored: later supervisor log lines go to stderr again (unless it is gone: see below).
                if reason != crate::tui::ExitReason::Hangup {
                    q.store(false, Ordering::SeqCst);
                }
                let _ = tx.send(reason);
            })
            .is_ok()
        {
            dash = Some(rx);
        }
    });
    done.store(true, Ordering::SeqCst);
    // procfix3 (gate-B M2): a dashboard whose terminal is gone or blocked (a write to a tty nobody reads waits for
    // ever) must not hold the process: wait for it at most 10 s; the terminal output is then skipped (log file only).
    let mut terminal_ok = true;
    if let Some(rx) = dash {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(crate::tui::ExitReason::Hangup) => terminal_ok = false,
            Ok(_) => {}
            Err(_) => terminal_ok = false,
        }
    }
    if !terminal_ok {
        let mut log = crate::supervisor::Log::open(&cdir);
        log.line_file_only(
            "WARN",
            &format!(
                "terminal lost or blocked: final summary only here ({:?}, exit {})",
                r,
                r.code()
            ),
        );
        return r.code();
    }
    quiet.store(false, Ordering::SeqCst);
    match &r {
        supervisor::RunExit::Refused(m)
        | supervisor::RunExit::Moved(m)
        | supervisor::RunExit::BinaryChanged(m)
        | supervisor::RunExit::Internal(m) => {
            errln!("rr_campaign: {m}")
        }
        supervisor::RunExit::FinishedOpen(m) | supervisor::RunExit::FinishedError(m) => errln!(
            "rr_campaign: FINISHED, NOT ALL CLOSED (exit {}): {m}; log: {}",
            r.code(),
            cdir.join("log").join("supervisor.log").display()
        ),
        supervisor::RunExit::Finished if with_dashboard => {
            errln!(
                "rr_campaign: FINISHED (exit 0); log: {}",
                cdir.join("log").join("supervisor.log").display()
            )
        }
        supervisor::RunExit::Stopped { code } if with_dashboard => errln!(
            "rr_campaign: STOPPED (exit {code}); resume with the same command; log: {}",
            cdir.join("log").join("supervisor.log").display()
        ),
        other if with_dashboard => errln!("rr_campaign: {other:?} (exit {})", other.code()),
        _ => {}
    }
    r.code()
}

/// One pre-flight line.
fn pf(out: &mut impl Write, status: &str, what: &str) {
    let _ = writeln!(out, "{status:<7} {what}");
}

fn cmd_preflight(rest: &[String]) -> i32 {
    let f = match parse(rest) {
        Ok(f) => f,
        Err(e) => {
            errln!("{e}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let mut out = std::io::stdout();
    let (spec, digest, cdir) = match load_spec(&f) {
        Ok(x) => x,
        Err((c, m)) => {
            pf(&mut out, "REFUSE", &m);
            return c;
        }
    };
    preflight(&spec, &digest, &cdir, &mut out)
}

/// The pre-flight of R6 (b): prints the plan and every check; returns 0 or the code of the FIRST refusal.
pub fn preflight(spec: &CampaignSpec, digest: &str, cdir: &Path, out: &mut impl Write) -> i32 {
    let mut code = exit::OK;
    let refuse = |c: i32, code: &mut i32| {
        if *code == exit::OK {
            *code = c;
        }
    };
    pf(
        out,
        "ok",
        &format!(
            "spec {} (blake3 {}), campaign {}",
            spec.schema,
            &digest[..16],
            spec.campaign
        ),
    );
    if let Err(e) = spec.check_dirs(cdir) {
        pf(out, "REFUSE", &e);
        refuse(exit::PF_SPEC, &mut code);
    }
    if let Some(s) = &spec.synthetic {
        pf(
            out,
            "plan",
            &format!(
                "synthetic: {} items on {} nodes, {} rungs (seed {}), fake children, cap {} MiB per job",
                s.items, s.nodes, s.rungs, s.seed, s.mem_cap_mb
            ),
        );
    }
    let r = &spec.resources;
    pf(
        out,
        "plan",
        &format!(
            "resources: policy {}, cpus {}, max {} jobs, reservation total {} bytes, host floor {} bytes, disk floor {} bytes, grace {} s",
            r.policy,
            r.cpus.as_deref().unwrap_or("(site policy)"),
            r.max_jobs,
            r.mem_total.0,
            r.host_floor.0,
            r.disk_floor.0,
            r.grace_seconds
        ),
    );
    for (name, p) in &spec.programs {
        match crate::atomic::blake3_file(&p.path) {
            Ok((bytes, d)) => {
                if let Some(want) = &p.blake3
                    && *want != d
                {
                    pf(
                        out,
                        "REFUSE",
                        &format!(
                            "binary blake3 mismatch: {name} ({} has {d})",
                            p.path.display()
                        ),
                    );
                    refuse(exit::PF_PROGRAM, &mut code);
                    continue;
                }
                let ver = version_line(&p.path);
                match ver {
                    Ok(v) if v.contains("rustred.child-job.v1") => pf(
                        out,
                        "ok",
                        &format!(
                            "program {name}: {} ({bytes} bytes, blake3 {d}); {}",
                            p.path.display(),
                            v.trim()
                        ),
                    ),
                    Ok(v) => {
                        pf(
                            out,
                            "REFUSE",
                            &format!("program {name}: no contract rustred.child-job.v1 in {v:?}"),
                        );
                        refuse(exit::PF_PROGRAM, &mut code);
                    }
                    Err(e) => {
                        pf(
                            out,
                            "REFUSE",
                            &format!("program {name}: version --json failed: {e}"),
                        );
                        refuse(exit::PF_PROGRAM, &mut code);
                    }
                }
            }
            Err(e) if p.optional => pf(
                out,
                "WARN",
                &format!("optional program {name} unavailable: {e}"),
            ),
            Err(e) => {
                pf(
                    out,
                    "REFUSE",
                    &format!("program {name} missing: {} ({e})", p.path.display()),
                );
                refuse(exit::PF_PROGRAM, &mut code);
            }
        }
    }
    for i in &spec.inputs {
        match i.check() {
            Ok(b) => pf(
                out,
                "ok",
                &format!(
                    "input {} ({}): {} ({b} bytes, blake3 verified)",
                    i.id,
                    i.role,
                    i.path.display()
                ),
            ),
            Err(e) => {
                pf(out, "REFUSE", &e);
                refuse(exit::PF_INPUT, &mut code);
            }
        }
    }
    match &spec.licence.env {
        Some(name) if std::env::var_os(name).is_some() => pf(
            out,
            "ok",
            &format!("licence variable {name}: present (value not shown)"),
        ),
        Some(name) => {
            pf(out, "REFUSE", &format!("licence missing: {name} not set"));
            refuse(exit::PF_LICENCE, &mut code);
        }
        None => pf(out, "ok", "licence: none required by this spec"),
    }
    match crate::resources::host_memory() {
        Ok(m) => {
            let avail = m.effective_available();
            let first_cap = spec.synthetic.as_ref().map_or(0, |s| s.mem_cap_mb << 20);
            if avail < r.host_floor.0 + first_cap {
                pf(
                    out,
                    "REFUSE",
                    &format!(
                        "memory below floor (MemAvailable {avail} < {} + one job cap {first_cap})",
                        r.host_floor.0
                    ),
                );
                refuse(exit::PF_MEMORY, &mut code);
            } else {
                pf(
                    out,
                    "ok",
                    &format!(
                        "memory: MemAvailable {avail} bytes >= floor {} + one job cap",
                        r.host_floor.0
                    ),
                );
            }
        }
        Err(e) => {
            pf(
                out,
                "REFUSE",
                &format!("memory: /proc/meminfo unreadable: {e}"),
            );
            refuse(exit::PF_MEMORY, &mut code);
        }
    }
    let probe = existing_ancestor(cdir);
    let mut disks = vec![probe];
    if let Some(w) = &spec.dirs.work_dir {
        disks.push(existing_ancestor(w));
    }
    for d in disks {
        match statvfs_avail(&d) {
            Some(a) if a >= r.disk_floor.0 => pf(
                out,
                "ok",
                &format!("disk: {} has {a} bytes free >= floor", d.display()),
            ),
            Some(a) => {
                pf(
                    out,
                    "REFUSE",
                    &format!(
                        "disk below floor: {} has {a} bytes free < {}",
                        d.display(),
                        r.disk_floor.0
                    ),
                );
                refuse(exit::PF_DISK, &mut code);
            }
            None => {
                pf(
                    out,
                    "REFUSE",
                    &format!("disk: statvfs {} failed", d.display()),
                );
                refuse(exit::PF_DISK, &mut code);
            }
        }
    }
    match crate::lock::read_owner(cdir) {
        Ok(Some(o)) => match crate::lock::liveness(&o.identity) {
            crate::lock::Liveness::Alive => {
                pf(
                    out,
                    "REFUSE",
                    &format!(
                        "run active since {}, pid {} on {}",
                        o.started_utc, o.identity.pid, o.identity.host
                    ),
                );
                refuse(exit::PF_LOCK, &mut code);
            }
            _ => pf(
                out,
                "ok",
                &format!(
                    "lock: stale record of pid {} (will be taken over)",
                    o.identity.pid
                ),
            ),
        },
        Ok(None) => pf(out, "ok", "lock: free"),
        Err(e) => {
            pf(out, "REFUSE", &format!("lock: {e}"));
            refuse(exit::PF_LOCK, &mut code);
        }
    }
    if cdir
        .join("ledger")
        .join(crate::ledger::MANIFEST_FILE)
        .exists()
    {
        pf(
            out,
            "ok",
            &format!(
                "campaign directory {} exists: run resumes it",
                cdir.display()
            ),
        );
    } else {
        pf(
            out,
            "ok",
            &format!(
                "campaign directory {} is new: run creates it",
                cdir.display()
            ),
        );
    }
    if code == exit::OK {
        pf(
            out,
            "ready",
            &format!(
                "start with: rr_campaign run --spec <SPEC> --dir {}",
                cdir.display()
            ),
        );
    }
    code
}

fn existing_ancestor(p: &Path) -> PathBuf {
    let mut q = p.to_path_buf();
    while !q.exists() {
        match q.parent() {
            Some(x) => q = x.to_path_buf(),
            None => break,
        }
    }
    q
}

fn statvfs_avail(p: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(p.as_os_str().as_bytes()).ok()?;
    // SAFETY: zeroed statvfs is a valid out-parameter; c is a valid C string.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: as above.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    Some(s.f_bavail as u64 * s.f_frsize as u64)
}

/// `<program> version --json` with a 30 s bound (clean environment).
pub(crate) fn version_line(p: &Path) -> Result<String, String> {
    let mut child = std::process::Command::new(p)
        .args(["version", "--json"])
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let t0 = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if t0.elapsed() > Duration::from_secs(30) => {
                // No signal path in the CLI (gate re-audit M-1): the probe is left to finish on its own.
                return Err("timed out".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(e.to_string()),
        }
    }
    let mut s = String::new();
    use std::io::Read;
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut s);
    }
    Ok(s)
}

fn cmd_control(rest: &[String]) -> i32 {
    let f = match parse(rest) {
        Ok(f) => f,
        Err(e) => {
            errln!("{e}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let (Some(cdir), Some(verb)) = (f.positional.first(), f.positional.get(1)) else {
        errln!("{USAGE}");
        return exit::USAGE;
    };
    let target = f.positional.get(2).cloned();
    // lane closeloop4 (closeloop.md 19): the operator's decision on terminal proposals
    if verb == "resolve-terminals" {
        let payload = match (&f.accept, f.reject) {
            (Some(p), false) => {
                let text = match std::fs::read_to_string(p) {
                    Ok(t) => t,
                    Err(e) => {
                        errln!("{}: {e}", p.display());
                        return exit::USAGE;
                    }
                };
                match crate::closeloop::terminals::read_keys(&text) {
                    Ok(k) => serde_json::json!({"accept": k}),
                    Err(e) => {
                        errln!("{}: {e}", p.display());
                        return exit::USAGE;
                    }
                }
            }
            (None, true) => serde_json::json!({"reject": true}),
            _ => {
                errln!("resolve-terminals: exactly one of --accept KEYS_FILE / --reject\n{USAGE}");
                return exit::USAGE;
            }
        };
        return match write_control_payload(
            Path::new(cdir),
            verb,
            f.node.as_deref(),
            f.reason.as_deref(),
            Some(payload),
        ) {
            Ok(p) => {
                outln!("request written: {}", p.display());
                exit::OK
            }
            Err((c, m)) => {
                errln!("{m}");
                c
            }
        };
    }
    match (verb.as_str(), &target) {
        ("pause" | "resume", _) | ("stop", None) => {}
        ("retry" | "skip" | "resolve", Some(_)) => {}
        _ => {
            errln!("{USAGE}");
            return exit::USAGE;
        }
    }
    match write_control_with(
        Path::new(cdir),
        verb,
        target.as_deref(),
        f.reason.as_deref(),
    ) {
        Ok(p) => {
            outln!("request written: {}", p.display());
            exit::OK
        }
        Err((c, m)) => {
            errln!("{m}");
            c
        }
    }
}

/// Writes a control request file atomically (applied by the supervisor at its next tick).
pub fn write_control(
    cdir: &Path,
    verb: &str,
    target: Option<&str>,
) -> Result<PathBuf, (i32, String)> {
    write_control_with(cdir, verb, target, None)
}

/// [`write_control`] with the operator's reason (recorded in the ledger's `control {requested}` record).
pub fn write_control_with(
    cdir: &Path,
    verb: &str,
    target: Option<&str>,
    reason: Option<&str>,
) -> Result<PathBuf, (i32, String)> {
    write_control_payload(cdir, verb, target, reason, None)
}

/// [`write_control_with`] with the data of a work-source decision (lane closeloop4: `resolve-terminals`).
pub fn write_control_payload(
    cdir: &Path,
    verb: &str,
    target: Option<&str>,
    reason: Option<&str>,
    payload: Option<serde_json::Value>,
) -> Result<PathBuf, (i32, String)> {
    if !cdir
        .join("ledger")
        .join(crate::ledger::MANIFEST_FILE)
        .exists()
    {
        return Err((
            exit::NOT_FOUND,
            format!("no campaign in {}", cdir.display()),
        ));
    }
    let dir = cdir.join("control");
    std::fs::create_dir_all(&dir).map_err(|e| (exit::INTERNAL, e.to_string()))?;
    let utc = crate::cpu_policy::utc_now();
    let nonce = blake3::hash(
        format!("{utc}{}{:?}", std::process::id(), std::time::Instant::now()).as_bytes(),
    )
    .to_hex()[..8]
        .to_string();
    let compact: String = utc.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    // gate-B m1: requests are applied in the order of a SEQUENCE NUMBER taken under a lock (two requests of the same
    // second keep the order in which they were written), not in timestamp / random order.
    let seq = supervisor::next_control_seq(&dir).map_err(|e| (exit::INTERNAL, e))?;
    let id = format!("c-{seq:012}-{compact}-{nonce}");
    let req = ControlRequest {
        schema: CONTROL_SCHEMA.into(),
        id: id.clone(),
        utc,
        by: format!("{} (cli)", std::env::var("USER").unwrap_or_default()),
        verb: verb.into(),
        target: target.map(str::to_string),
        reason: reason.map(str::to_string),
        payload,
    };
    let p = dir.join(format!("{id}.json"));
    crate::atomic::write_json_atomic(&p, &req).map_err(|e| (exit::INTERNAL, e.to_string()))?;
    Ok(p)
}

/// `rebuild-manifest CDIR` (ledgerfix2's request, lane int5): the recovery verb for a ledger whose MANIFEST.json is
/// missing (a supervisor start refuses such a directory: ledgerfix2 M1). Takes the campaign lock (REFUSED while a live
/// supervisor holds it: never a second writer), then `ledger::rebuild_manifest` verifies the WHOLE chain from record 1
/// and writes only the new manifest (nothing deleted or rewritten). Prints what it rebuilt. Exit 0 rebuilt, 1 refused
/// (lock held, a manifest present, any doubt about the chain), 2 usage, 3 no ledger directory.
fn cmd_rebuild_manifest(rest: &[String]) -> i32 {
    let [cdir] = rest else {
        errln!("{USAGE}");
        return exit::USAGE;
    };
    let cdir = Path::new(cdir);
    let ldir = crate::ledger::ledger_dir(cdir);
    if !ldir.is_dir() {
        errln!("no ledger directory in {}", cdir.display());
        return exit::NOT_FOUND;
    }
    if ldir.join(crate::ledger::MANIFEST_FILE).exists() {
        errln!(
            "refused: {} exists; nothing to rebuild",
            ldir.join(crate::ledger::MANIFEST_FILE).display()
        );
        return exit::REFUSED;
    }
    let clock = crate::clock::SystemClock::new();
    let argv: Vec<String> = ["rr_campaign".to_string(), "rebuild-manifest".to_string()]
        .into_iter()
        .chain(rest.iter().cloned())
        .collect();
    let (lock, outcome) = match crate::lock::acquire(cdir, &clock, &argv) {
        Ok(x) => x,
        Err(e) => {
            errln!("refused: the campaign lock is not free ({e}); stop the supervisor first");
            return exit::REFUSED;
        }
    };
    if let crate::lock::LockOutcome::TookOver {
        previous, reason, ..
    } = &outcome
    {
        outln!(
            "stale lock of pid {} taken over ({reason:?})",
            previous.identity.pid
        );
    }
    let r = crate::ledger::rebuild_manifest(cdir, &lock);
    let _ = lock.release();
    match r {
        Ok(rep) => {
            outln!(
                "rebuilt {}: {} record(s) verified from record 1, {} sealed segment(s), active segment {}; no snapshot (the next rotation writes one)",
                ldir.join(crate::ledger::MANIFEST_FILE).display(),
                rep.records,
                rep.sealed,
                rep.active
            );
            exit::OK
        }
        Err(e) => {
            errln!("refused: {e}");
            exit::REFUSED
        }
    }
}

fn cmd_verify(rest: &[String]) -> i32 {
    let Some(cdir) = rest.first() else {
        errln!("{USAGE}");
        return exit::USAGE;
    };
    let cdir = Path::new(cdir);
    if !cdir
        .join("ledger")
        .join(crate::ledger::MANIFEST_FILE)
        .exists()
    {
        errln!("no campaign in {}", cdir.display());
        return exit::NOT_FOUND;
    }
    match crate::progress::verify(cdir) {
        Ok(r) => {
            outln!(
                "verify: {} records, {} segments, {} progress files checked, {} differences, {} missing, {} stale{}",
                r.records,
                r.segments,
                r.files_checked,
                r.differences.len(),
                r.missing.len(),
                r.stale.len(),
                if r.clean() { "" } else { ": NOT CLEAN" }
            );
            for d in &r.differences {
                outln!(
                    "  {} {}: expected {} found {}",
                    d.file,
                    d.path,
                    d.expected,
                    d.found
                );
            }
            // gate-B m2: every reason of a non-zero exit is printed (not only differences).
            for m in &r.missing {
                outln!("  missing progress file: {m}");
            }
            for (f, seq) in &r.stale {
                outln!(
                    "  stale progress file: {f} states seq {seq}, the ledger is at {}",
                    r.records
                );
            }
            if r.snapshot_matches == Some(false) {
                outln!("  the latest snapshot does not match the replayed ledger");
            }
            if r.torn_tail_bytes > 0 {
                outln!(
                    "  torn tail: {} byte(s) after the last valid record (repaired by the next `run`)",
                    r.torn_tail_bytes
                );
            }
            if r.clean() {
                exit::OK
            } else {
                exit::VERIFY_DIFFERS
            }
        }
        Err(e) => {
            outln!("verify: ledger refused: {e}");
            exit::VERIFY_DIFFERS
        }
    }
}

/// `status CDIR [--once] [--width N] [--height M] [--json]`: ONE rendering through the dashboard's view-model (the
/// same adapter as the live dashboard and the website numbers): text of the summary screen, or the view-model as
/// JSON. Reads the progress files only (never the lock, never a write).
fn cmd_status(rest: &[String]) -> i32 {
    let f = match parse(rest) {
        Ok(f) => f,
        Err(e) => {
            errln!("{e}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let Some(cdir) = f.positional.first() else {
        errln!("{USAGE}");
        return exit::USAGE;
    };
    let cdir = Path::new(cdir);
    if !cdir
        .join(crate::progress::PROGRESS_DIR)
        .join("campaign.json")
        .exists()
    {
        errln!(
            "no progress file {}",
            cdir.join(crate::progress::PROGRESS_DIR)
                .join("campaign.json")
                .display()
        );
        return exit::NOT_FOUND;
    }
    let source = crate::tui::DirSource::new(cdir);
    let opts = crate::tui::DashboardOptions::default();
    if f.json {
        outln!("{}", crate::tui::view_model_json_text(&source, &opts));
        return exit::OK;
    }
    let (tw, th) = if std::io::stdout().is_terminal() {
        crate::tui::terminal::terminal_area()
            .map(|a| (a.width, a.height))
            .unwrap_or((120, 40))
    } else {
        (120, 40)
    };
    let text = crate::tui::render_once(
        &source,
        &opts,
        f.width.unwrap_or(tw).max(20),
        f.height.unwrap_or(th).max(5),
    );
    let _ = std::io::stdout().write_all(text.as_bytes());
    exit::OK
}

/// `watch CDIR` / `dashboard CDIR`: the read-only live viewer (it never takes the lock, never writes, never signals;
/// q / Ctrl-C close it and the campaign continues).
fn cmd_watch(rest: &[String]) -> i32 {
    let f = match parse(rest) {
        Ok(f) => f,
        Err(e) => {
            errln!("{e}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let Some(cdir) = f.positional.first() else {
        errln!("{USAGE}");
        return exit::USAGE;
    };
    let opts = crate::tui::DashboardOptions {
        plain: f.plain,
        ..Default::default()
    };
    match crate::tui::run_viewer(PathBuf::from(cdir), &opts) {
        crate::tui::ExitReason::TerminalError(e) => {
            errln!("watch: {e}");
            exit::INTERNAL
        }
        _ => exit::OK,
    }
}

fn cmd_result(rest: &[String]) -> i32 {
    let Some(cdir) = rest.first() else {
        errln!("{USAGE}");
        return exit::USAGE;
    };
    match supervisor::result::read_final_result(Path::new(cdir)) {
        Ok((r, _)) => {
            match serde_json::to_string_pretty(&r) {
                Ok(t) => outln!("{t}"),
                Err(e) => {
                    errln!("result: {e}");
                    return exit::INTERNAL;
                }
            }
            exit::OK
        }
        Err(e) => {
            errln!("{e}");
            exit::NOT_FOUND
        }
    }
}
