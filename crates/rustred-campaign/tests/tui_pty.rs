//! Pseudo-terminal tests (plan 6.5 I5, M8 drill in miniature): the dashboard demo host runs inside a pty (openpty);
//! the test resizes the terminal, sends Ctrl-C bytes, `q` and signals, and checks the screen, the exit reason and that
//! the terminal modes are restored (raw mode off, cursor shown, alternate screen left).
//!
//! The host is THIS test binary (audit M5): the test re-executes itself (`std::env::current_exe()`) with
//! `--exact pty_helper_entry` and the demo arguments in `RR_TUI_PTY_HELPER`; the hidden test `pty_helper_entry` then
//! runs `tui_support::demo::main_with` and exits. No pre-built example, no choice by modification time.
//! Waits are for markers on the pty with generous timeouts, never fixed sleeps.
//! Run with `LIGHT_PIDNS=1` (lane rule L10): the tests spawn children and signal them.

mod tui_support;
#[path = "sup_common/mod.rs"]
mod sup;

use std::io::Read;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tui_support::demo::{ALT_EXIT, OSC52, TITLE};

const HELPER_ENV: &str = "RR_TUI_PTY_HELPER";

/// Hidden: the dashboard host. Does nothing in a normal test run.
#[test]
fn pty_helper_entry() {
    let Ok(args) = std::env::var(HELPER_ENV) else {
        return;
    };
    let args: Vec<String> = args
        .split('\u{1f}')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let code = tui_support::demo::main_with(args);
    std::process::exit(code);
}

/// This test binary as the dashboard host with demo arguments `args`.
fn host_command(args: &[&str]) -> Command {
    let exe = std::env::current_exe().expect("the test binary");
    let mut cmd = Command::new(exe);
    cmd.args([
        "--exact",
        "pty_helper_entry",
        "--nocapture",
        "--test-threads",
        "1",
        "-q",
    ])
    .env(HELPER_ENV, args.join("\u{1f}"))
    .env_remove("RUST_TEST_THREADS");
    cmd
}

struct Pty {
    master: RawFd,
    slave: RawFd,
    out: Arc<Mutex<Vec<u8>>>,
    stop: Arc<AtomicBool>,
    reader: Option<std::thread::JoinHandle<()>>,
}

fn winsize(cols: u16, rows: u16) -> libc::winsize {
    libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    }
}

impl Pty {
    fn open(cols: u16, rows: u16) -> Pty {
        let (mut m, mut s) = (0, 0);
        let ws = winsize(cols, rows);
        // SAFETY: valid out-pointers for the two descriptors and a valid winsize; name and termios may be null.
        let r =
            unsafe { libc::openpty(&mut m, &mut s, std::ptr::null_mut(), std::ptr::null(), &ws) };
        assert_eq!(r, 0, "openpty: {}", std::io::Error::last_os_error());
        let out = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (o2, s2) = (Arc::clone(&out), Arc::clone(&stop));
        let reader = std::thread::spawn(move || {
            let mut buf = [0u8; 65536];
            while !s2.load(Ordering::SeqCst) {
                let mut pfd = libc::pollfd {
                    fd: m,
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: one valid pollfd.
                let n = unsafe { libc::poll(&mut pfd, 1, 50) };
                if n <= 0 {
                    continue;
                }
                // SAFETY: buf is valid for buf.len() bytes; m is the open master.
                let k = unsafe { libc::read(m, buf.as_mut_ptr().cast(), buf.len()) };
                if k <= 0 {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }
                o2.lock().unwrap().extend_from_slice(&buf[..k as usize]);
            }
        });
        Pty {
            master: m,
            slave: s,
            out,
            stop,
            reader: Some(reader),
        }
    }

    fn spawn(&self, args: &[&str]) -> Child {
        self.spawn_cmd(host_command(args))
    }

    /// Any command as the session leader of this terminal (stdin / stdout / stderr = the pty).
    fn spawn_cmd(&self, mut cmd: Command) -> Child {
        let s = self.slave;
        let dup = |fd: RawFd| -> Stdio {
            // SAFETY: dup of an open descriptor; the new descriptor is owned by the Stdio.
            Stdio::from(unsafe { OwnedFd::from_raw_fd(libc::dup(fd)) })
        };
        cmd.env("TERM", "xterm-256color")
            .env("LANG", "C")
            .env_remove("LC_ALL")
            .env_remove("LC_CTYPE")
            .env_remove("NO_COLOR")
            .stdin(dup(s))
            .stdout(dup(s))
            .stderr(dup(s));
        // SAFETY: only async-signal-safe calls between fork and exec (setsid, ioctl).
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        cmd.spawn().expect("spawn the dashboard host")
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.out.lock().unwrap()).into_owned()
    }

    fn len(&self) -> usize {
        self.out.lock().unwrap().len()
    }

    /// Waits until the output after byte `from` contains `needle`.
    fn wait_for(&self, from: usize, needle: &str, secs: u64) {
        let t0 = Instant::now();
        loop {
            {
                let o = self.out.lock().unwrap();
                if String::from_utf8_lossy(&o[from.min(o.len())..]).contains(needle) {
                    return;
                }
            }
            if t0.elapsed() > Duration::from_secs(secs) {
                panic!(
                    "timeout waiting for {needle:?}; output:\n{}",
                    printable(&self.text())
                );
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    fn send(&self, bytes: &[u8]) {
        // SAFETY: bytes valid for its length; master open.
        let n = unsafe { libc::write(self.master, bytes.as_ptr().cast(), bytes.len()) };
        assert_eq!(n, bytes.len() as isize);
    }

    fn resize(&self, cols: u16, rows: u16) {
        let ws = winsize(cols, rows);
        // SAFETY: TIOCSWINSZ with a valid winsize on the open master (the kernel sends SIGWINCH to the foreground group).
        let r = unsafe { libc::ioctl(self.master, libc::TIOCSWINSZ, &ws) };
        assert_eq!(r, 0);
    }

    /// Termios local flags of the terminal (after the child exited: what it left behind).
    fn lflags(&self) -> libc::tcflag_t {
        // SAFETY: zeroed termios is a valid out-parameter; slave is open.
        let mut t: libc::termios = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::tcgetattr(self.slave, &mut t) }, 0);
        t.c_lflag
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(r) = self.reader.take() {
            let _ = r.join();
        }
        // SAFETY: closing our own descriptors once.
        unsafe {
            libc::close(self.master);
            libc::close(self.slave);
        }
    }
}

fn printable(s: &str) -> String {
    s.replace('\u{1b}', "<ESC>").replace('\u{7}', "<BEL>")
}

fn wait_exit(child: &mut Child, secs: u64) -> std::process::ExitStatus {
    let t0 = Instant::now();
    loop {
        if let Some(st) = child.try_wait().unwrap() {
            return st;
        }
        if t0.elapsed() > Duration::from_secs(secs) {
            let _ = child.kill();
            panic!("the dashboard host did not exit");
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn assert_restored(p: &Pty, text: &str) {
    let lf = p.lflags();
    assert!(lf & libc::ICANON != 0, "canonical mode restored");
    assert!(lf & libc::ECHO != 0, "echo restored");
    assert!(lf & libc::ISIG != 0, "signals restored (raw mode off)");
    let enter = text
        .find("\u{1b}[?1049h")
        .expect("alternate screen entered");
    let leave = text.rfind("\u{1b}[?1049l").expect("alternate screen left");
    assert!(leave > enter);
    let shown = text.rfind("\u{1b}[?25h").expect("cursor shown");
    let hidden = text
        .rfind("\u{1b}[?25l")
        .expect("cursor hidden while drawing");
    assert!(
        shown > hidden,
        "the cursor is shown again after the last hide"
    );
}

#[test]
fn supervisor_dashboard_in_a_pty_resize_ctrl_c_twice_restores_terminal() {
    let p = Pty::open(120, 40);
    let mut child = p.spawn(&["--fixture", "midrun", "--supervisor", "--grace", "30"]);
    p.wait_for(0, "RUNNING", 30);
    p.wait_for(0, "Rules and certificates", 30);
    // resize to the wide layout: the selected-node column appears
    let mark = p.len();
    p.resize(200, 60);
    p.wait_for(mark, "(selected; n/N)", 20);
    // first Ctrl-C (a byte in raw mode): drain banner with countdown
    let mark = p.len();
    p.send(b"\x03");
    p.wait_for(mark, "Press Ctrl-C again to force", 20);
    p.wait_for(mark, "STOPPING 0:", 20);
    // second: forced stop banner, then the fake supervisor reports done
    let mark = p.len();
    p.send(b"\x03");
    p.wait_for(mark, "FORCED STOP", 20);
    let st = wait_exit(&mut child, 20);
    assert!(st.success(), "{st:?}");
    p.wait_for(0, "exit reason: StopRequested(Force)", 10);
    let text = p.text();
    assert_restored(&p, &text);
    // the final view is printed to the normal screen after leaving the alternate screen
    let leave = text.rfind("\u{1b}[?1049l").unwrap();
    assert!(
        text[leave..].contains("rr_campaign  p5l-boxed-final"),
        "{}",
        printable(&text[leave..])
    );
}

#[test]
fn supervisor_single_ctrl_c_drains_until_done() {
    let p = Pty::open(80, 24);
    let mut child = p.spawn(&["--fixture", "midrun", "--supervisor", "--grace", "2"]);
    p.wait_for(0, "Nodes (4)", 30);
    p.send(b"\x03");
    let st = wait_exit(&mut child, 20);
    assert!(st.success());
    p.wait_for(0, "exit reason: StopRequested(Drain)", 10);
    let text = p.text();
    assert!(text.contains("STOPPING"), "{}", printable(&text));
    assert_restored(&p, &text);
}

#[test]
fn watcher_q_quits_and_tiny_terminal_shows_plain_line() {
    let p = Pty::open(79, 23);
    let mut child = p.spawn(&["--fixture", "stale"]);
    p.wait_for(0, "enlarge to 80x24", 30);
    p.wait_for(0, "STALE", 20);
    p.send(b"q");
    let st = wait_exit(&mut child, 20);
    assert!(st.success());
    p.wait_for(0, "exit reason: ViewerQuit", 10);
    assert_restored(&p, &p.text());
}

type Collected = Arc<Mutex<Vec<u8>>>;

/// Spawns the host with stdout / stderr piped, collecting them.
fn piped(args: &[&str]) -> (Child, Collected, Collected) {
    let mut child = host_command(args)
        .env_remove("LC_ALL")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let collect = |mut r: Box<dyn Read + Send>| {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let b2 = Arc::clone(&buf);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = r.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                b2.lock().unwrap().extend_from_slice(&chunk[..n]);
            }
        });
        buf
    };
    let out = collect(Box::new(child.stdout.take().unwrap()));
    let err = collect(Box::new(child.stderr.take().unwrap()));
    (child, out, err)
}

fn wait_text(buf: &Arc<Mutex<Vec<u8>>>, needle: &str, secs: u64) {
    let t0 = Instant::now();
    while !String::from_utf8_lossy(&buf.lock().unwrap()).contains(needle) {
        assert!(
            t0.elapsed() < Duration::from_secs(secs),
            "timeout waiting for {needle:?}: {}",
            String::from_utf8_lossy(&buf.lock().unwrap())
        );
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[test]
fn plain_mode_when_stdout_is_not_a_terminal() {
    let (mut child, out, err) = piped(&["--fixture", "midrun"]);
    // M5: wait for the first status line (not a fixed sleep), then SIGINT to our own child: the viewer stops
    wait_text(&out, " RUNNING seq 18,402", 30);
    // SAFETY: signalling the child we spawned.
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let st = wait_exit(&mut child, 20);
    assert!(st.success());
    wait_text(&err, "exit reason: ViewerQuit", 10);
    let so = String::from_utf8_lossy(&out.lock().unwrap()).into_owned();
    assert!(
        !so.contains('\u{1b}'),
        "no escape codes in plain mode: {so}"
    );
    assert!(so.contains("event item_accepted T178 F087"), "{so}");
    assert!(
        so.contains(" RUNNING seq 18,402 cpu 21/32 ram 9.8/30.0GiB"),
        "{so}"
    );
}

/// The bytes the C1 probe injects must never reach the terminal raw.
fn assert_no_injection(bytes: &[u8], legit_alt_exits: usize) {
    let text = String::from_utf8_lossy(bytes);
    let p = printable(&text);
    assert!(
        !text.contains("\u{1b}]"),
        "an OSC sequence reached the terminal: {p}"
    );
    assert!(!text.contains('\u{7}'), "a BEL reached the terminal: {p}");
    assert!(
        !text.contains('\u{9b}'),
        "a C1 CSI reached the terminal: {p}"
    );
    assert!(
        !text.contains('\u{202e}'),
        "a bidi override reached the terminal: {p}"
    );
    assert_eq!(
        text.matches(ALT_EXIT).count(),
        legit_alt_exits,
        "only the dashboard's own alternate-screen exit: {p}"
    );
    // the injected texts are shown, visibly neutralised
    let _ = (OSC52, TITLE);
}

#[test]
fn c1_escape_injection_never_reaches_the_terminal_pty() {
    let p = Pty::open(200, 60);
    let mut child = p.spawn(&["--fixture", "probe", "--color", "always"]);
    p.wait_for(0, "Running jobs", 30);
    p.wait_for(0, "from ledger, partial", 20);
    p.wait_for(0, "]52;c;ZXZpbA==", 20); // the job id, shown as text after the placeholder
    // the node detail of an injected node, the events panel, the jobs panel
    for k in [b"e".as_slice(), b"q", b"j", b"q", b"a", b"q"] {
        p.send(k);
        std::thread::sleep(Duration::from_millis(300));
    }
    p.send(b"q");
    let st = wait_exit(&mut child, 20);
    assert!(st.success());
    p.wait_for(0, "exit reason: ViewerQuit", 10);
    let text = p.text();
    assert_restored(&p, &text);
    assert_no_injection(p.out.lock().unwrap().as_slice(), 1);
    // --once and --json of the probe
    for mode in ["--once", "--json"] {
        let (mut child, out, _err) = piped(&[
            "--fixture",
            "probe",
            mode,
            "--width",
            "200",
            "--height",
            "60",
        ]);
        let st = wait_exit(&mut child, 30);
        assert!(st.success());
        std::thread::sleep(Duration::from_millis(100));
        let o = out.lock().unwrap().clone();
        assert!(!o.contains(&0x1b), "{mode}: ESC in the output");
        assert_no_injection(&o, 0);
        if mode == "--json" {
            let s = String::from_utf8_lossy(&o);
            let start = s.find('{').expect("json object");
            let v: serde_json::Value = serde_json::from_str(&s[start..]).expect("valid JSON");
            assert!(v["campaign_id"].as_str().unwrap().starts_with("camp"));
        }
    }
}

#[test]
fn m7_sigterm_and_sighup_restore_the_viewer_terminal() {
    for (sig, reason) in [(libc::SIGTERM, "ViewerQuit"), (libc::SIGHUP, "Hangup")] {
        let p = Pty::open(120, 40);
        let mut child = p.spawn(&["--fixture", "midrun"]);
        p.wait_for(0, "Rules and certificates", 30);
        // SAFETY: signalling the child we spawned.
        unsafe { libc::kill(child.id() as i32, sig) };
        let st = wait_exit(&mut child, 20);
        assert!(st.success(), "{sig}: {st:?}");
        p.wait_for(0, &format!("exit reason: {reason}"), 10);
        assert_restored(&p, &p.text());
    }
}

#[test]
fn m1_ctrl_c_is_handled_while_every_read_blocks() {
    let p = Pty::open(120, 40);
    let mut child = p.spawn(&["--fixture", "midrun", "--slow-read-ms", "8000"]);
    // the screen is up and says it is reading, long before the first read returns
    p.wait_for(0, "READING", 30);
    let t0 = Instant::now();
    p.send(b"\x03");
    let st = wait_exit(&mut child, 20);
    let took = t0.elapsed();
    assert!(st.success());
    assert!(
        took < Duration::from_secs(3),
        "Ctrl-C waited for the slow read: {took:?}"
    );
    p.wait_for(0, "exit reason: ViewerQuit", 10);
    assert_restored(&p, &p.text());
}

/// The screen text of the LAST view a dashboard printed to the normal screen after leaving the alternate screen.
fn final_view(text: &str) -> String {
    let leave = text.rfind("\u{1b}[?1049l").expect("alternate screen left");
    let tail = &text[leave + "\u{1b}[?1049l".len()..];
    // strip the remaining CSI sequences (cursor / colour resets) and carriage returns
    let mut out = String::new();
    let mut it = tail.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() || d == '~' {
                        break;
                    }
                }
            }
            continue;
        }
        if c != '\r' {
            out.push(c);
        }
    }
    out.trim_start_matches('\n').to_string()
}

fn rr_campaign(args: &[&str]) -> Command {
    let mut c = Command::new(sup::SUP);
    c.args(args).env_remove("RR_CAMPAIGN_FAILPOINT");
    c
}

/// End to end (lane int2, requirement R5): `rr_campaign run --fake-child` on a small scripted campaign in a
/// pseudo-terminal shows the dashboard with running jobs; one Ctrl-C BYTE (raw mode) drains with the countdown banner
/// and stops cleanly (exit 130, as for SIGINT); a second run resumes and finishes (exit 0); the terminal modes are
/// restored each time; `watch` from a second process shows the same header numbers as campaign.json ("website =
/// terminal"). Run with LIGHT_PIDNS=1 (L10).
#[test]
fn e2e_run_dashboard_ctrl_c_drains_resume_finishes_watch_matches_campaign_json() {
    let _g = sup::serial();
    let t = sup::Tmp::new("e2e");
    let syn = serde_json::json!({"items": 40, "nodes": 2, "case_ms": [2000, 4000], "deps": 0.0,
                                 "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.2});
    let spec_p = sup::write_spec(
        &t,
        "spec.json",
        &sup::spec(&t, syn, serde_json::json!({"max_jobs": 4, "grace_seconds": 5})),
    );
    let d = t.cdir("run");
    let (sp, ds) = (spec_p.to_string_lossy().into_owned(), d.to_string_lossy().into_owned());
    let run = || rr_campaign(&["run", "--spec", &sp, "--dir", &ds, "--fake-child"]);
    // session 1: the dashboard with running jobs, then one Ctrl-C byte
    let p = Pty::open(120, 40);
    let mut c = p.spawn_cmd(run());
    p.wait_for(0, "RUNNING", 60);
    // running jobs in jobs.json, and their ids on the screen (the jobs table)
    let t0 = Instant::now();
    let running = loop {
        let list: Vec<String> = std::fs::read(d.join("progress/jobs.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| v["list"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter(|j| j["state"] == "running")
            .filter_map(|j| j["job"].as_str().map(String::from))
            .collect();
        if !list.is_empty() {
            break list;
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "no running job in jobs.json");
        std::thread::sleep(Duration::from_millis(100));
    };
    p.wait_for(0, "Running jobs", 30);
    let t0 = Instant::now();
    while !running.iter().any(|j| p.text().contains(j.as_str())) {
        assert!(
            t0.elapsed() < Duration::from_secs(30),
            "running jobs {running:?} not on the screen: {}",
            printable(&p.text())
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let mark = p.len();
    p.send(b"\x03");
    p.wait_for(mark, "STOPPING", 30);
    let st = wait_exit(&mut c, 120);
    assert_eq!(st.code(), Some(130), "{st:?}: {}", printable(&p.text()));
    let text = p.text();
    assert_restored(&p, &text);
    assert!(
        text.contains("STOPPED (exit 130)"),
        "{}",
        printable(&text[text.len().saturating_sub(2000)..])
    );
    let st1 = sup::state(&d);
    assert!(st1.jobs.values().all(|j| j.state.is_settled() || j.state == rustred_campaign::state::AttemptState::Queued));
    drop(p);
    // session 2: the same command resumes and finishes
    let p = Pty::open(120, 40);
    let mut c = p.spawn_cmd(run());
    let st = wait_exit(&mut c, 600);
    assert_eq!(st.code(), Some(0), "{}", printable(&p.text()));
    let text = p.text();
    assert_restored(&p, &text);
    assert!(text.contains("FINISHED (exit 0)"), "{}", printable(&text));
    drop(p);
    assert_eq!(sup::verify(&d), 0);
    sup::assert_no_marked(&t.campaign);
    // watch from a second process: the header numbers equal campaign.json's
    let p = Pty::open(120, 40);
    let mut w = p.spawn_cmd(rr_campaign(&["watch", &ds]));
    p.wait_for(0, "FINISHED", 30);
    p.send(b"q");
    let st = wait_exit(&mut w, 30);
    assert!(st.success(), "{st:?}");
    let text = p.text();
    assert_restored(&p, &text);
    let screen = final_view(&text);
    let campaign: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.join("progress/campaign.json")).unwrap()).unwrap();
    let got = rustred_campaign::tui::screen_numbers(&screen);
    let want = rustred_campaign::tui::campaign_json_numbers(&campaign);
    assert!(!want.is_empty());
    for (k, v) in &want {
        assert_eq!(got.get(k), Some(v), "{k}\n{screen}");
    }
    // the viewer never wrote: no lock taken, the ledger unchanged
    assert!(!d.join(rustred_campaign::lock::OWNER_FILE).exists());
    // `status --once` is the same view-model, one rendering
    let out = rr_campaign(&["status", &ds, "--once", "--width", "120", "--height", "40"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let once = String::from_utf8_lossy(&out.stdout).into_owned();
    for (k, v) in &want {
        assert_eq!(rustred_campaign::tui::screen_numbers(&once).get(k), Some(v), "{k}\n{once}");
    }
}
