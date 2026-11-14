//! procfix3: the supervisor under a low open-file limit (gate-B M3) and with a terminal that disappears (gate-B M2,
//! m4). Real `rr_campaign` + `rr_fake_child`; run inside a private pid namespace (LIGHT_PIDNS=1, rule L10).

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn run_cmd(spec: &std::path::Path, cdir: &std::path::Path, extra: &[&str]) -> Command {
    let mut c = Command::new(SUP);
    c.arg("run")
        .arg("--spec")
        .arg(spec)
        .arg("--dir")
        .arg(cdir)
        .args(extra)
        .env_remove("RR_CAMPAIGN_FAILPOINT");
    c
}

/// gate-B M3: under a low RLIMIT_NOFILE the supervisor runs out of descriptors either at START (it refuses with exit 71
/// before anything is launched) or at LAUNCH (a launch failure that is the supervisor's resource problem is not a job
/// attempt: the same job is requeued, no new job id, launching backs off, one ledger note, and after a bounded number
/// of failures in a row the campaign stops cleanly with exit 71). The limit is raised from 12 until the campaign
/// finishes: every run ends in bounded time, the launch regime is reached at least once, and with the limit lifted the
/// same directory finishes.
#[test]
fn emfile_at_launch_backs_off_and_stops_with_the_resource_code() {
    let _g = serial();
    let t = Tmp::new("nofile");
    let spec_p = write_spec(
        &t,
        "spec.json",
        &spec(&t, serde_json::json!({"items": 12}), serde_json::json!({})),
    );
    let mut launch_regime = false;
    let mut last_dir = None;
    for limit in 12u64..=24 {
        let d = t.cdir(&format!("run{limit}"));
        let log = std::fs::File::create(d.with_extension("log")).unwrap();
        let mut cmd = run_cmd(&spec_p, &d, &["--plain"]);
        cmd.stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log);
        // SAFETY: setrlimit in the forked child before exec (async-signal-safe).
        unsafe {
            cmd.pre_exec(move || {
                let r = libc::rlimit {
                    rlim_cur: limit,
                    rlim_max: limit,
                };
                if libc::setrlimit(libc::RLIMIT_NOFILE, &r) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let t0 = Instant::now();
        let mut c = cmd.spawn().unwrap();
        let code = wait(&mut c, Duration::from_secs(300));
        let took = t0.elapsed();
        eprintln!("RLIMIT_NOFILE {limit}: exit {code} in {took:?}");
        assert!(took < Duration::from_secs(120), "bounded: {took:?}");
        if code == 0 {
            break;
        }
        assert_eq!(code, 71, "exit 71 = supervisor resources\n{}", log_tail(&d));
        if d.join("ledger").join("MANIFEST.json").exists() {
            let launches = count(&d, |e| {
                matches!(
                    e,
                    Event::Job {
                        ev: JobEvent::Launch { .. },
                        ..
                    }
                )
            });
            assert!(
                launches <= 8,
                "a bounded number of launch attempts: {launches}"
            );
            let st = state(&d);
            assert!(
                st.jobs.len() <= 2,
                "no new job id per failure: {:?}",
                st.jobs.keys()
            );
            if launches >= 1 {
                launch_regime = true;
                assert_eq!(notes(&d, "supervisor_resource_error").len(), 1, "one note");
                assert!(
                    st.recent
                        .iter()
                        .any(|r| r.text.contains("launch failures in a row")),
                    "the reason is in the ledger's events (dashboard): {:?}",
                    st.recent
                );
                let f = ledger_facts(&d);
                assert_eq!(f.requeues, f.interrupted);
                last_dir = Some(d.clone());
            }
        }
    }
    assert!(
        launch_regime,
        "no limit between 12 and 24 reached the launch-failure regime"
    );
    // the limit lifted: the same directory finishes
    let d = last_dir.unwrap();
    assert_eq!(run_to_end(&spec_p, &d, &[]), vec![0], "{}", log_tail(&d));
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

struct Pty {
    master: Option<OwnedFd>,
    slave: Option<OwnedFd>,
}

impl Pty {
    fn open() -> Pty {
        let (mut m, mut s) = (0, 0);
        let ws = libc::winsize {
            ws_row: 40,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: valid out-pointers and winsize; name and termios may be null.
        let r =
            unsafe { libc::openpty(&mut m, &mut s, std::ptr::null_mut(), std::ptr::null(), &ws) };
        assert_eq!(r, 0);
        // Close-on-exec: the child must NOT inherit the master (else the terminal never hangs up when we close ours).
        for fd in [m, s] {
            // SAFETY: fcntl on our own fresh descriptors.
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
        }
        // SAFETY: fresh descriptors owned here.
        unsafe {
            Pty {
                master: Some(OwnedFd::from_raw_fd(m)),
                slave: Some(OwnedFd::from_raw_fd(s)),
            }
        }
    }
    fn raw(o: &Option<OwnedFd>) -> RawFd {
        use std::os::fd::AsRawFd;
        o.as_ref().unwrap().as_raw_fd()
    }
    /// Read whatever the master has for up to `secs`, until `needle` appears.
    fn wait_for(&self, needle: &str, secs: u64) -> String {
        let m = Self::raw(&self.master);
        let mut out = Vec::new();
        let t0 = Instant::now();
        let mut buf = [0u8; 65536];
        while t0.elapsed() < Duration::from_secs(secs) {
            let mut p = libc::pollfd {
                fd: m,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: one valid pollfd.
            if unsafe { libc::poll(&mut p, 1, 100) } > 0 {
                // SAFETY: valid buffer; open master.
                let k = unsafe { libc::read(m, buf.as_mut_ptr().cast(), buf.len()) };
                if k > 0 {
                    out.extend_from_slice(&buf[..k as usize]);
                    if String::from_utf8_lossy(&out).contains(needle) {
                        break;
                    }
                }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }
}

fn cpu_ticks(pid: u32) -> u64 {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    let rest = s.rsplit_once(')').map(|x| x.1).unwrap_or("");
    let f: Vec<&str> = rest.split_whitespace().collect();
    f.get(11).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
        + f.get(12).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
}

/// gate-B M2 / m4: `run` with the dashboard on a pseudo-terminal that is NOT its controlling terminal (no SIGHUP
/// when it goes); the terminal's master is closed while jobs run. The dashboard thread must notice the lost terminal
/// (no 100 % CPU spin), the supervisor continues in plain mode (log file) and exits normally at the end (0, never a
/// panic 101 on the dead stderr).
#[test]
fn lost_terminal_does_not_spin_and_the_campaign_ends_cleanly() {
    let _g = serial();
    let t = Tmp::new("pty");
    let syn = serde_json::json!({"items": 8, "case_ms": [600, 1200], "seed": 3});
    let spec_p = write_spec(
        &t,
        "spec.json",
        &spec(&t, syn, serde_json::json!({"max_jobs": 4})),
    );
    let d = t.cdir("run");
    let pty = Pty::open();
    let s = Pty::raw(&pty.slave);
    let dup = |fd: RawFd| -> Stdio {
        // SAFETY: dup of an open descriptor; owned by the Stdio.
        Stdio::from(unsafe { OwnedFd::from_raw_fd(libc::dup(fd)) })
    };
    let mut cmd = run_cmd(&spec_p, &d, &["--fake-child"]);
    cmd.env("TERM", "xterm-256color")
        .env("LANG", "C")
        .stdin(dup(s))
        .stdout(dup(s))
        .stderr(dup(s));
    // SAFETY: setsid between fork and exec; deliberately NO TIOCSCTTY (the pty is not the controlling terminal).
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut c = cmd.spawn().unwrap();
    let screen = pty.wait_for("RUNNING", 120);
    assert!(screen.contains("RUNNING"), "dashboard drawn: {screen:?}");
    // The terminal disappears.
    let mut pty = pty;
    drop(pty.master.take());
    drop(pty.slave.take());
    std::thread::sleep(Duration::from_secs(2));
    let before = cpu_ticks(c.id());
    std::thread::sleep(Duration::from_secs(3));
    let after = cpu_ticks(c.id());
    let still_running = c.try_wait().unwrap().is_none();
    let code = wait(&mut c, Duration::from_secs(600));
    assert!(
        !still_running || after - before < 150,
        "the supervisor spins after the terminal vanished: {} ticks in 3 s",
        after - before
    );
    assert_eq!(
        code,
        0,
        "{}",
        std::fs::read_to_string(d.join("log/supervisor.log")).unwrap_or_default()
    );
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// gate-B m4: plain mode with stdout / stderr on a terminal that vanishes: no panic (exit 101) at the final lines.
#[test]
fn plain_mode_with_a_vanished_terminal_never_panics() {
    let _g = serial();
    let t = Tmp::new("ptyp");
    let syn = serde_json::json!({"items": 6, "case_ms": [400, 800], "seed": 3});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let d = t.cdir("run");
    let mut pty = Pty::open();
    let s = Pty::raw(&pty.slave);
    let dup = |fd: RawFd| -> Stdio {
        // SAFETY: dup of an open descriptor; owned by the Stdio.
        Stdio::from(unsafe { OwnedFd::from_raw_fd(libc::dup(fd)) })
    };
    let mut cmd = run_cmd(&spec_p, &d, &["--plain", "--fake-child"]);
    cmd.stdin(Stdio::null()).stdout(dup(s)).stderr(dup(s));
    // SAFETY: setsid between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut c = cmd.spawn().unwrap();
    let _ = pty.wait_for("INFO", 60);
    drop(pty.master.take());
    drop(pty.slave.take());
    let code = wait(&mut c, Duration::from_secs(600));
    assert_eq!(code, 0, "never 101 on a dead terminal");
    assert_eq!(verify(&d), 0);
}
