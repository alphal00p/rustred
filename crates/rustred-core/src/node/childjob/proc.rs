//! Process layer of the child-job contract (child-job.md 3.4.1, 3.4.6, 3.11; rulings R-a, R-h): signal flag, fork
//! hooks for every helper the engine forks, and the helper registry every stop path kills and reaps.
//!
//! Everything here is async-signal-safe where it has to be: the handler only does two relaxed atomic stores; the
//! post-fork hook only calls `prctl`, `getppid`, `signal` and `_exit`.

use std::sync::atomic::{AtomicI32, AtomicU32, Ordering::Relaxed};

/// Number of SIGTERM / SIGINT / SIGHUP received since [`install_signals`].
static SIGNALS: AtomicU32 = AtomicU32::new(0);
/// The last of those signals.
static LAST_SIG: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_signal(sig: libc::c_int) {
    LAST_SIG.store(sig, Relaxed);
    SIGNALS.fetch_add(1, Relaxed);
}

/// Install the counting handlers for SIGTERM, SIGINT and SIGHUP (job mode only, first thing in `main`, before any
/// thread or fork). Legacy invocations never call it: their signal behaviour is unchanged.
pub fn install_signals() {
    for s in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        // SAFETY: the handler only performs atomic stores (async-signal-safe); the struct is zeroed, then the
        // handler and an empty mask are set.
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = on_signal as *const () as usize;
            sa.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(s, &sa, std::ptr::null_mut());
        }
    }
}

/// (signals received so far, last signal number).
pub fn signals() -> (u32, i32) {
    (SIGNALS.load(Relaxed), LAST_SIG.load(Relaxed))
}

/// The pid of this process.
pub fn pid() -> i32 {
    // SAFETY: getpid has no preconditions.
    unsafe { libc::getpid() }
}

/// Call in a forked helper IMMEDIATELY after `fork` returned 0, with the parent's pid read BEFORE the fork
/// (3.4.6): the helper dies with its parent (PR_SET_PDEATHSIG = SIGKILL, also on SIGKILL / OOM kill of the parent),
/// leaves at once if the parent is already gone (the race between fork and prctl), and in job mode ignores
/// SIGTERM / SIGINT / SIGHUP (a group signal must not kill it behind the parent's back; the parent kills its helpers
/// itself). Only async-signal-safe calls. In legacy mode only the first two (the escaping-helper fix, ruling R-h).
pub fn after_fork_child(parent: i32) {
    // SAFETY: prctl / getppid / signal / _exit are async-signal-safe and act on this process only.
    unsafe {
        libc::prctl(
            libc::PR_SET_PDEATHSIG,
            libc::SIGKILL as libc::c_ulong,
            0,
            0,
            0,
        );
        if libc::getppid() != parent {
            libc::_exit(0);
        }
        if super::active() {
            for s in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
                libc::signal(s, libc::SIG_IGN);
            }
        }
    }
}

const SLOTS: usize = 32;
#[allow(clippy::declare_interior_mutable_const)]
const FREE: AtomicI32 = AtomicI32::new(0);
/// Live helpers (pids; 0 = free slot).
static HELPERS: [AtomicI32; SLOTS] = [FREE; SLOTS];
/// Largest `ru_maxrss` (KiB) of any helper reaped through [`kill_helpers`] or [`note_reaped`].
static HELPERS_PEAK_KB: AtomicI32 = AtomicI32::new(0);

/// Register a live helper (parent side, right after `fork`). A full registry still leaves the helper covered by
/// PDEATHSIG and by the supervisor's group kill.
pub fn register_helper(pid: i32) {
    if pid <= 0 {
        return;
    }
    for s in &HELPERS {
        if s.compare_exchange(0, pid, Relaxed, Relaxed).is_ok() {
            return;
        }
    }
}

/// Forget a helper the caller reaped itself.
pub fn unregister_helper(pid: i32) {
    for s in &HELPERS {
        let _ = s.compare_exchange(pid, 0, Relaxed, Relaxed);
    }
}

/// Live registered helpers.
pub fn live_helpers() -> Vec<i32> {
    HELPERS
        .iter()
        .map(|s| s.load(Relaxed))
        .filter(|&p| p > 0)
        .collect()
}

/// SIGKILL and reap every registered helper (every stop path of the job). Returns the helpers' peak RSS (KiB).
pub fn kill_helpers() -> u64 {
    for s in &HELPERS {
        let pid = s.swap(0, Relaxed);
        if pid > 0 {
            // SAFETY: plain syscalls on our own registered child.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                let mut status = 0;
                let mut ru: libc::rusage = std::mem::zeroed();
                if libc::wait4(pid, &mut status, 0, &mut ru) == pid {
                    HELPERS_PEAK_KB.fetch_max(ru.ru_maxrss.clamp(0, i32::MAX as libc::c_long) as i32, Relaxed);
                }
            }
        }
    }
    helpers_peak_kb()
}

/// Peak RSS (KiB) of the helpers reaped through this module.
pub fn helpers_peak_kb() -> u64 {
    HELPERS_PEAK_KB.load(Relaxed).max(0) as u64
}

/// Resident set size of a process in KiB (`/proc/<pid>/statm`; 0 = self).
pub fn rss_kb(pid: i32) -> u64 {
    let p = if pid == 0 {
        "/proc/self/statm".to_string()
    } else {
        format!("/proc/{pid}/statm")
    };
    // SAFETY: sysconf has no preconditions.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let page_kb = if page > 0 { page as u64 / 1024 } else { 4 };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).and_then(|x| x.parse::<u64>().ok()))
        .map_or(0, |pages| pages * page_kb)
}

/// CPU seconds of this process (user + system).
pub fn cpu_seconds() -> f64 {
    // SAFETY: getrusage fills the zeroed struct.
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut ru) != 0 {
            return 0.0;
        }
        let t = |v: libc::timeval| v.tv_sec as f64 + v.tv_usec as f64 / 1e6;
        t(ru.ru_utime) + t(ru.ru_stime)
    }
}

/// End the process at once without running destructors (hard-safe stop: the main thread may hold locks or be
/// inside a write).
pub fn exit_now(code: i32) -> ! {
    // SAFETY: _exit never returns and touches no Rust state.
    unsafe { libc::_exit(code) }
}
