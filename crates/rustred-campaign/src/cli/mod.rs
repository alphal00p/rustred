//! Library functions of the `rr_campaign` commands (interfaces/supervisor.md sections 9-12). Argument parsing is
//! hand-written like the rest of the engine (no clap). Every function returns the process exit code.

use crate::supervisor::exit;
use crate::supervisor::spec::CampaignSpec;
use crate::supervisor::{self, CONTROL_SCHEMA, ControlRequest, RunOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const USAGE: &str = "usage:
  rr_campaign run --spec SPEC.json [--dir CDIR] [--plain] [--fake-child] [--accept-binary-change] [--cores N] [--mem-total 30G]
  rr_campaign preflight --spec SPEC.json [--dir CDIR]
  rr_campaign control CDIR pause|resume|stop|retry ITEM|skip ITEM
  rr_campaign verify CDIR
  rr_campaign status CDIR [--once]
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
    once: bool,
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
            "--once" => f.once = true,
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
    let r = supervisor::run(RunOptions {
        spec,
        spec_blake3: digest,
        cdir,
        plain: f.plain || true,
        fake_child: f.fake_child,
        accept_binary_change: f.accept_binary_change,
        cores: f.cores,
        mem_total: f.mem_total,
        argv: full,
        tick: Duration::from_millis(200),
    });
    match &r {
        supervisor::RunExit::Refused(m)
        | supervisor::RunExit::BinaryChanged(m)
        | supervisor::RunExit::Internal(m) => {
            eprintln!("rr_campaign: {m}")
        }
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
                let _ = child.kill();
                let _ = child.wait();
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
    let (Some(cdir), Some(verb)) = (rest.first(), rest.get(1)) else {
        eprintln!("{USAGE}");
        return exit::USAGE;
    };
    let target = rest.get(2).cloned();
    match (verb.as_str(), &target) {
        ("pause" | "resume" | "stop", None) => {}
        ("retry" | "skip", Some(_)) => {}
        _ => {
            eprintln!("{USAGE}");
            return exit::USAGE;
        }
    }
    match write_control(Path::new(cdir), verb, target.as_deref()) {
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

fn cmd_status(rest: &[String]) -> i32 {
    let Some(cdir) = rest.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("{USAGE}");
        return exit::USAGE;
    };
    let p = Path::new(cdir).join("progress").join("campaign.json");
    let Ok(b) = std::fs::read(&p) else {
        eprintln!("no progress file {}", p.display());
        return exit::NOT_FOUND;
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&b) else {
        eprintln!("unreadable progress file {}", p.display());
        return exit::NOT_FOUND;
    };
    let g = |path: &[&str]| -> String {
        let mut x = &v;
        for k in path {
            match x.get(k) {
                Some(y) => x = y,
                None => return "null".into(),
            }
        }
        x.to_string()
    };
    println!(
        "campaign {}  phase {}  ledger seq {}  written {}",
        g(&["campaign_id"]),
        g(&["campaign", "phase"]),
        g(&["ledger", "seq"]),
        g(&["written_utc"])
    );
    println!(
        "jobs: {}  live {}  cpus {} / {}",
        g(&["jobs", "total"]),
        g(&["resources", "live_jobs"]),
        g(&["resources", "cpus_in_use"]),
        g(&["resources", "cpus_allowed"])
    );
    println!("items by phase: {}", g(&["items", "by_phase"]));
    println!(
        "rules accepted {}  terminals {}  eta {}",
        g(&["rules", "accepted"]),
        g(&["terminals", "declared"]),
        g(&["eta"])
    );
    exit::OK
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
