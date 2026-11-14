//! Pseudo-terminal test (plan 6.5 I5, M8 drill in miniature): `examples/dashboard_demo` hosts the dashboard on a
//! fixture inside a pty (openpty); the test resizes the terminal, sends Ctrl-C bytes and `q`, and checks the screen,
//! the exit reason and that the terminal modes are restored (raw mode off, alternate screen left).
//!
//! The example binary is built by `cargo test -p rustred-campaign` (examples are compiled with the tests); when this
//! test is run alone with `--test tui_pty`, build it first (`cargo build -p rustred-campaign --example dashboard_demo`).

use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The example binary: next to the test executable, or in the workspace target directory (the host's cargo
/// configuration puts test executables into a separate build directory). The newest candidate wins. Panics with an
/// instruction when it is not built (a silently skipped terminal test would hide a regression).
fn demo() -> Option<PathBuf> {
    let mut c: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(d) = exe.parent().and_then(|p| p.parent())
    {
        c.push(d.join("examples/dashboard_demo"));
    }
    let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| ws.join("target"));
    c.push(target.join("debug/examples/dashboard_demo"));
    let best = c
        .into_iter()
        .filter(|p| p.is_file())
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    Some(best.unwrap_or_else(|| {
        panic!("examples/dashboard_demo is not built: run `cargo test -p rustred-campaign` (builds examples) or `cargo build -p rustred-campaign --example dashboard_demo` first")
    }))
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
        let mut ws = winsize(cols, rows);
        // SAFETY: valid out-pointers for the two descriptors and a valid winsize; name and termios may be null.
        let r = unsafe { libc::openpty(&mut m, &mut s, std::ptr::null_mut(), std::ptr::null(), &mut ws) };
        assert_eq!(r, 0, "openpty: {}", std::io::Error::last_os_error());
        let out = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (o2, s2) = (Arc::clone(&out), Arc::clone(&stop));
        let reader = std::thread::spawn(move || {
            let mut buf = [0u8; 65536];
            while !s2.load(Ordering::SeqCst) {
                let mut pfd = libc::pollfd { fd: m, events: libc::POLLIN, revents: 0 };
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
        Pty { master: m, slave: s, out, stop, reader: Some(reader) }
    }

    fn spawn(&self, args: &[&str]) -> Child {
        let s = self.slave;
        let dup = |fd: RawFd| -> Stdio {
            // SAFETY: dup of an open descriptor; the new descriptor is owned by the Stdio.
            Stdio::from(unsafe { OwnedFd::from_raw_fd(libc::dup(fd)) })
        };
        let mut cmd = Command::new(demo().unwrap());
        cmd.args(args)
            .env("TERM", "xterm-256color")
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
        cmd.spawn().expect("spawn dashboard_demo")
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
                panic!("timeout waiting for {needle:?}; output:\n{}", printable(&self.text()));
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
    s.replace('\u{1b}', "<ESC>")
}

fn wait_exit(child: &mut Child, secs: u64) -> std::process::ExitStatus {
    let t0 = Instant::now();
    loop {
        if let Some(st) = child.try_wait().unwrap() {
            return st;
        }
        if t0.elapsed() > Duration::from_secs(secs) {
            let _ = child.kill();
            panic!("dashboard_demo did not exit");
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn assert_restored(p: &Pty, text: &str) {
    let lf = p.lflags();
    assert!(lf & libc::ICANON != 0, "canonical mode restored");
    assert!(lf & libc::ECHO != 0, "echo restored");
    assert!(lf & libc::ISIG != 0, "signals restored (raw mode off)");
    let enter = text.find("\u{1b}[?1049h").expect("alternate screen entered");
    let leave = text.rfind("\u{1b}[?1049l").expect("alternate screen left");
    assert!(leave > enter);
    assert!(text[leave..].contains("\u{1b}[?25h") || text[..leave].ends_with("\u{1b}[?25h") || text.contains("\u{1b}[?25h"), "cursor shown");
}

#[test]
fn supervisor_dashboard_in_a_pty_resize_ctrl_c_twice_restores_terminal() {
    let Some(_) = demo() else { return };
    let p = Pty::open(120, 40);
    let mut child = p.spawn(&["--fixture", "midrun", "--supervisor", "--grace", "30"]);
    p.wait_for(0, "RUNNING", 20);
    p.wait_for(0, "Rules and certificates", 20);
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
    assert!(text[leave..].contains("rr_campaign  p5l-boxed-final"), "{}", printable(&text[leave..]));
}

#[test]
fn supervisor_single_ctrl_c_drains_until_done() {
    let Some(_) = demo() else { return };
    let p = Pty::open(80, 24);
    let mut child = p.spawn(&["--fixture", "midrun", "--supervisor", "--grace", "2"]);
    p.wait_for(0, "Work list by ladder level", 20);
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
    let Some(_) = demo() else { return };
    let p = Pty::open(79, 23);
    let mut child = p.spawn(&["--fixture", "stale"]);
    p.wait_for(0, "enlarge to 80x24", 20);
    p.wait_for(0, "STALE", 20);
    p.send(b"q");
    let st = wait_exit(&mut child, 20);
    assert!(st.success());
    p.wait_for(0, "exit reason: ViewerQuit", 10);
    assert_restored(&p, &p.text());
}

#[test]
fn plain_mode_when_stdout_is_not_a_terminal() {
    let Some(d) = demo() else { return };
    let child = Command::new(d)
        .args(["--fixture", "midrun"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    // SIGINT to our own child: the viewer stops
    // SAFETY: signalling the child we spawned.
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let out = child.wait_with_output().unwrap();
    let so = String::from_utf8_lossy(&out.stdout);
    let se = String::from_utf8_lossy(&out.stderr);
    assert!(!so.contains('\u{1b}'), "no escape codes in plain mode: {so}");
    assert!(so.contains("event item_accepted T178 F087"), "{so}");
    assert!(so.contains(" RUNNING seq 18,402 cpu 21/32 ram 9.8/30.0G"), "{so}");
    assert!(se.contains("exit reason: ViewerQuit"), "{se}");
}
