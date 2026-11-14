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
  rr_campaign run --spec SPEC.json [--dir CDIR] [--plain] [--fake-child] [--accept-binary-change] [--adopt-moved-campaign] [--cores N] [--mem-total 30G] [--accept-truncation-to SEQ]
      (the live dashboard when stdout is a terminal and --plain is not given; Ctrl-C = drain, twice = force)
  rr_campaign preflight --spec SPEC.json [--dir CDIR]
  rr_campaign watch CDIR [--plain]          (alias: dashboard; read-only viewer: q / Ctrl-C close it, the campaign is unaffected)
  rr_campaign status CDIR [--once] [--width N] [--height M] [--json]
  rr_campaign control CDIR pause [NODE|ITEM]|resume [NODE|ITEM]|stop|retry ITEM|skip ITEM [--reason TEXT]
  rr_campaign verify CDIR
  rr_campaign result CDIR";

/// Dispatch `argv` (without the program name).
pub fn main_with(args: &[String]) -> i32 {
    let Some(cmd) = args.first() else {
        eprintln!("{USAGE}");
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
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            exit::OK
        }
        other => {
            eprintln!("unknown command {other:?}\n{USAGE}");
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
    positional: Vec<String>,
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
            "--accept-truncation-to" => {
                f.accept_truncation_to = Some(
                    val("--accept-truncation-to")?
                        .parse()
                        .map_err(|_| "--accept-truncation-to: a ledger seq")?,
                )
            }
            "--cores" => f.cores = Some(val("--cores")?.parse().map_err(|_| "--cores: a number")?),
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
            eprintln!("{e}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let (spec, digest, cdir) = match load_spec(&f) {
        Ok(x) => x,
        Err((c, m)) => {
            eprintln!("{m}");
            return c;
        }
    };
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
    let mut dash: Option<std::thread::JoinHandle<crate::tui::ExitReason>> = None;
    let opts = RunOptions {
        spec,
        spec_blake3: digest,
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
        dash = std::thread::Builder::new()
            .name("dashboard".into())
            .spawn(move || {
                let reason = crate::tui::run_dashboard(source, &controls, &o);
                // Terminal restored: later supervisor log lines go to stderr again.
                q.store(false, Ordering::SeqCst);
                reason
            })
            .ok();
    });
    done.store(true, Ordering::SeqCst);
    if let Some(h) = dash {
        let _ = h.join();
    }
    quiet.store(false, Ordering::SeqCst);
    match &r {
        supervisor::RunExit::Refused(m)
        | supervisor::RunExit::Moved(m)
        | supervisor::RunExit::BinaryChanged(m)
        | supervisor::RunExit::Internal(m) => {
            eprintln!("rr_campaign: {m}")
        }
        supervisor::RunExit::Finished if with_dashboard => {
            eprintln!(
                "rr_campaign: FINISHED (exit 0); log: {}",
                cdir.join("log").join("supervisor.log").display()
            )
        }
        supervisor::RunExit::Stopped { code } if with_dashboard => eprintln!(
            "rr_campaign: STOPPED (exit {code}); resume with the same command; log: {}",
            cdir.join("log").join("supervisor.log").display()
        ),
        other if with_dashboard => eprintln!("rr_campaign: {other:?} (exit {})", other.code()),
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
            eprintln!("{e}\n{USAGE}");
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
fn version_line(p: &Path) -> Result<String, String> {
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
            eprintln!("{e}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let (Some(cdir), Some(verb)) = (f.positional.first(), f.positional.get(1)) else {
        eprintln!("{USAGE}");
        return exit::USAGE;
    };
    let target = f.positional.get(2).cloned();
    match (verb.as_str(), &target) {
        ("pause" | "resume", _) | ("stop", None) => {}
        ("retry" | "skip", Some(_)) => {}
        _ => {
            eprintln!("{USAGE}");
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
            println!("request written: {}", p.display());
            exit::OK
        }
        Err((c, m)) => {
            eprintln!("{m}");
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
    let id = format!("c-{compact}-{nonce}");
    let req = ControlRequest {
        schema: CONTROL_SCHEMA.into(),
        id: id.clone(),
        utc,
        by: format!("{} (cli)", std::env::var("USER").unwrap_or_default()),
        verb: verb.into(),
        target: target.map(str::to_string),
        reason: reason.map(str::to_string),
    };
    let p = dir.join(format!("{id}.json"));
    crate::atomic::write_json_atomic(&p, &req).map_err(|e| (exit::INTERNAL, e.to_string()))?;
    Ok(p)
}

fn cmd_verify(rest: &[String]) -> i32 {
    let Some(cdir) = rest.first() else {
        eprintln!("{USAGE}");
        return exit::USAGE;
    };
    let cdir = Path::new(cdir);
    if !cdir
        .join("ledger")
        .join(crate::ledger::MANIFEST_FILE)
        .exists()
    {
        eprintln!("no campaign in {}", cdir.display());
        return exit::NOT_FOUND;
    }
    match crate::progress::verify(cdir) {
        Ok(r) => {
            println!(
                "verify: {} records, {} segments, {} progress files checked, {} differences",
                r.records,
                r.segments,
                r.files_checked,
                r.differences.len()
            );
            for d in &r.differences {
                println!(
                    "  {} {}: expected {} found {}",
                    d.file, d.path, d.expected, d.found
                );
            }
            if r.clean() {
                exit::OK
            } else {
                exit::VERIFY_DIFFERS
            }
        }
        Err(e) => {
            println!("verify: ledger refused: {e}");
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
            eprintln!("{e}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let Some(cdir) = f.positional.first() else {
        eprintln!("{USAGE}");
        return exit::USAGE;
    };
    let cdir = Path::new(cdir);
    if !cdir
        .join(crate::progress::PROGRESS_DIR)
        .join("campaign.json")
        .exists()
    {
        eprintln!(
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
        println!("{}", crate::tui::view_model_json_text(&source, &opts));
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
    print!("{text}");
    exit::OK
}

/// `watch CDIR` / `dashboard CDIR`: the read-only live viewer (it never takes the lock, never writes, never signals;
/// q / Ctrl-C close it and the campaign continues).
fn cmd_watch(rest: &[String]) -> i32 {
    let f = match parse(rest) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let Some(cdir) = f.positional.first() else {
        eprintln!("{USAGE}");
        return exit::USAGE;
    };
    let opts = crate::tui::DashboardOptions {
        plain: f.plain,
        ..Default::default()
    };
    match crate::tui::run_viewer(PathBuf::from(cdir), &opts) {
        crate::tui::ExitReason::TerminalError(e) => {
            eprintln!("watch: {e}");
            exit::INTERNAL
        }
        _ => exit::OK,
    }
}

fn cmd_result(rest: &[String]) -> i32 {
    let Some(cdir) = rest.first() else {
        eprintln!("{USAGE}");
        return exit::USAGE;
    };
    match supervisor::result::read_final_result(Path::new(cdir)) {
        Ok((r, _)) => {
            println!("{}", serde_json::to_string_pretty(&r).expect("json"));
            exit::OK
        }
        Err(e) => {
            eprintln!("{e}");
            exit::NOT_FOUND
        }
    }
}
