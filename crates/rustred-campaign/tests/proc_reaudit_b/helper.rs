//! Helper programs run as jobs (by the launcher under test) or as decoys (by the driver).
use std::ffi::CString;
use std::time::Duration;

fn sleep_forever() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(3000));
    }
}

extern "C" fn clear_pdeath_this_thread(_: libc::c_int) {
    // SAFETY: prctl with integer arguments (async-signal-safe syscall).
    unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, 0, 0, 0, 0) };
}

/// Run `handler` on our OWN MAIN thread (tid == pid) through SIGUSR1 / tgkill, then wait briefly. Ported harness: helper
/// code runs on a libtest worker thread, but PR_SET_PDEATHSIG (set by the launcher before exec) is per THREAD.
fn on_main_thread(sig: libc::c_int, handler: extern "C" fn(libc::c_int)) {
    // SAFETY: a handler installation, then a signal to our own main thread only.
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = handler as *const () as usize;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(sig, &sa, std::ptr::null_mut());
        let me = libc::getpid();
        libc::syscall(libc::SYS_tgkill, me, me, sig);
    }
    std::thread::sleep(Duration::from_millis(50));
}

fn clear_pdeathsig() {
    // SAFETY: prctl with integer arguments (this thread), then the same on the main thread.
    unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, 0, 0, 0, 0) };
    on_main_thread(libc::SIGUSR1, clear_pdeath_this_thread);
}

extern "C" fn exit_this_thread(_: libc::c_int) {
    // SAFETY: SYS_exit ends only the calling thread (the main thread, which received the signal).
    unsafe { libc::syscall(libc::SYS_exit, 0) };
}

/// The MAIN thread exits alone (SYS_exit), a thread keeps the process alive. Ported harness: this code runs on a libtest
/// worker thread, so the main thread (tid == pid) is made to exit by a handler run on it (tgkill to our own main thread).
fn zombie_main(pidfile: Option<&str>) -> ! {
    if let Some(f) = pidfile {
        crate::write_atomic(std::path::Path::new(f), &std::process::id().to_string());
    }
    std::thread::sleep(Duration::from_millis(200));
    on_main_thread(libc::SIGUSR2, exit_this_thread);
    sleep_forever()
}

pub fn main(a: &[String]) {
    match a.first().map(String::as_str) {
        // rawexec <hex env entries...> -- program args...   (execve with EXACTLY these environment entries)
        Some("rawexec") => {
            let sep = a.iter().position(|x| x == "--").expect("--");
            let env: Vec<CString> = a[1..sep]
                .iter()
                .map(|h| CString::new(crate::unhex(h)).unwrap())
                .collect();
            let argv: Vec<CString> = a[sep + 1..]
                .iter()
                .map(|s| CString::new(s.as_bytes()).unwrap())
                .collect();
            let mut envp: Vec<*const libc::c_char> = env.iter().map(|c| c.as_ptr()).collect();
            envp.push(std::ptr::null());
            let mut argp: Vec<*const libc::c_char> = argv.iter().map(|c| c.as_ptr()).collect();
            argp.push(std::ptr::null());
            // SAFETY: NUL-terminated arrays of valid C strings.
            unsafe { libc::execve(argp[0], argp.as_ptr(), envp.as_ptr()) };
            eprintln!("execve failed: {}", std::io::Error::last_os_error());
            std::process::exit(127);
        }
        Some("nopdeath") => {
            clear_pdeathsig();
            // SAFETY: fork in a single-threaded helper; the child only sleeps.
            let pid = unsafe { libc::fork() };
            if pid == 0 {
                sleep_forever();
            }
            sleep_forever();
        }
        Some("thread-leader") => {
            clear_pdeathsig();
            zombie_main(a.get(1).map(String::as_str));
        }
        Some("zmain") => zombie_main(None),
        Some("stopself") => {
            // SAFETY: raise is async-signal-safe.
            unsafe { libc::raise(libc::SIGSTOP) };
            sleep_forever();
        }
        Some("cow") => {
            let mut b = vec![0u8; 80_000_000];
            for i in (0..b.len()).step_by(4096) {
                b[i] = 1;
            }
            // SAFETY: fork; the child only sleeps.
            let pid = unsafe { libc::fork() };
            if pid == 0 {
                loop {
                    // SAFETY: sleep is async-signal-safe.
                    unsafe { libc::sleep(3000) };
                }
            }
            std::hint::black_box(&b);
            sleep_forever();
        }
        Some("alloc") => {
            let mb: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(200);
            let mut blocks: Vec<Vec<u8>> = vec![];
            for _ in 0..mb / 10 {
                let mut b = vec![0u8; 10_000_000];
                for i in (0..b.len()).step_by(4096) {
                    b[i] = 1;
                }
                blocks.push(b);
                std::thread::sleep(Duration::from_millis(50));
            }
            std::hint::black_box(&blocks);
            sleep_forever();
        }
        // relay <n>: a marked process that keeps moving: fork a child (new session), exit; the child repeats.
        Some("relay") => {
            let hops: u64 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(u64::MAX);
            let pause_us: u64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(200);
            let counter = a.get(3).cloned();
            let mut i = 0u64;
            loop {
                // SAFETY: fork; the child continues the loop (single-threaded).
                let pid = unsafe { libc::fork() };
                if pid != 0 {
                    // SAFETY: _exit without destructors in the parent.
                    unsafe { libc::_exit(0) };
                }
                // SAFETY: setsid in the child.
                unsafe { libc::setsid() };
                i += 1;
                if let Some(f) = &counter
                    && i.is_multiple_of(10)
                {
                    let _ = std::fs::write(f, i.to_string());
                }
                if i >= hops {
                    sleep_forever();
                }
                std::thread::sleep(Duration::from_micros(pause_us));
            }
        }
        m => {
            eprintln!("unknown helper mode {m:?}");
            std::process::exit(2);
        }
    }
}
