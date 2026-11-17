//! THE ONLY CODE THAT OPENS `/proc` (procfix7, gate8 G8-B1 and its class).
//!
//! ONE RULE for every /proc read of this crate (tests/procfix8_tripwire.rs forbids a `/proc` path anywhere else):
//! a read has exactly three outcomes and its type says which, `Result<T, ProcError>`:
//! - `Ok(value)`: the file was read COMPLETELY and parsed;
//! - `Err(ProcError::Gone)`: ENOENT or ESRCH, and nothing else. For `/proc/<pid>/...` this means no process with that
//!   pid exists now; a pid is not reused while its process lives, so the process a caller identified earlier by
//!   (pid, start time) is dead. For `/proc/<pid>/task/<tid>/...` it means that THREAD is gone (the process may live on).
//!   For the `/proc` root itself there is no Gone (a missing /proc is unreadable, never "no processes").
//! - `Err(ProcError::Unreadable(why))`: every other errno (EMFILE, ENFILE, EIO, EACCES, EPERM, ENOMEM, EAGAIN, ...), a
//!   torn read (the file ended before its format says it is complete), a parse failure, a timeout, two enumerations of
//!   a directory that disagree. NEVER gone, never empty, never "not ours": in a classification it is "possibly ours"
//!   (inconclusive, exit 8 naming the pid), in a sweep it keeps the scan inconclusive, in a hunt it keeps the target.
//!
//! No reader returns `Option` for a read result. Composite answers that may legitimately be "none" (no live thread, an
//! empty children list, an empty environment) are explicit `Ok` values derived from COMPLETE reads only.
//!
//! Completeness (a short read in the POSIX sense, fewer bytes than asked, is normal: every read loops until the kernel
//! returns 0; a TORN read, the end reported early, is detected where the format allows it):
//! - stat: one line ending in '\n', parsed; empty or without the line end -> Unreadable (an empty read is retried and
//!   becomes Gone only by a following ENOENT / ESRCH);
//! - status, cgroup, meminfo, stat (cpu): end in '\n'; a requested `Key:` that is missing -> Unreadable;
//! - environ: its length must equal `env_end - env_start` of the process's stat (fields 50 / 51, read before and after
//!   with the same start time; the kernel reads exactly that range), else the read is repeated, then Unreadable;
//! - children: empty or ending in ' ', every token a number; a list is read until two consecutive reads agree;
//! - directory listings (the pid listing, task lists): TWO independent enumerations (libc readdir through std, and the
//!   raw getdents64 system call); an entry that one of them lacks although its process lived through both listings
//!   (started before the listing began and still there after) makes the listing Unreadable; the result is the union.
//!
//! Fault injection (TEST BUILDS ONLY, compiled out of a release build, whose `RR_CAMPAIGN_FAILPOINT` refusal covers it):
//! `RR_CAMPAIGN_FAILPOINT=proc_<kind>_<mode>:<every>[,proc_pid:<pid>]` makes every `<every>`-th read of that kind fail
//! (optionally only reads of one pid); kinds: listing tasks stat status environ cgroup ns nstime children piddir cwd fd
//! statm smaps fdinfo system; modes: emfile eio eacces enomem (that errno), short (a torn read: the second half missing; for a
//! listing: the libc enumeration ends early without an error), garbage (unparsable content; for a listing an entry
//! "7x"), partial (every read() returns at most 7 bytes: the result must be complete and equal).
#![allow(clippy::disallowed_methods)] // THE /proc WRAPPER has its own typed rule (module doc; tests/procfix8_tripwire.rs): every listing here propagates entry errors (Unreadable), every stat is classified (Gone only ENOENT / ESRCH)

use super::bounded::{Bounded, run_bounded};
use super::{ProcStat, page_size, parse_stat};
use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// The production proc root.
const PROC: &str = "/proc";

/// The supervisor's own executable (an exec of ourselves, the spawn barrier's shim; not a read).
pub const SELF_EXE: &str = "/proc/self/exe";

/// Why a /proc read gave no value. See the module documentation: `Gone` is ENOENT / ESRCH only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcError {
    /// ENOENT / ESRCH: the process (for a task path: the thread) does not exist now.
    Gone,
    /// Anything else: never gone, never empty, never "not ours".
    Unreadable(String),
}

impl ProcError {
    pub fn is_gone(&self) -> bool {
        matches!(self, ProcError::Gone)
    }
}

impl fmt::Display for ProcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcError::Gone => write!(f, "gone (ENOENT / ESRCH)"),
            ProcError::Unreadable(w) => write!(f, "unreadable: {w}"),
        }
    }
}

impl std::error::Error for ProcError {}

/// Into an I/O error for callers with `io::Result`: Gone keeps the NotFound kind (ENOENT), Unreadable is `other`.
impl From<ProcError> for io::Error {
    fn from(e: ProcError) -> io::Error {
        match e {
            ProcError::Gone => io::Error::from_raw_os_error(libc::ENOENT),
            ProcError::Unreadable(w) => io::Error::other(w),
        }
    }
}

/// The result of every /proc read.
pub type ProcResult<T> = Result<T, ProcError>;

fn unreadable<T>(w: impl Into<String>) -> ProcResult<T> {
    Err(ProcError::Unreadable(w.into()))
}

/// Classify an I/O error of a read of `what`: ENOENT / ESRCH = Gone, everything else Unreadable.
fn classify(e: &io::Error, what: &str) -> ProcError {
    match e.raw_os_error() {
        Some(libc::ENOENT) | Some(libc::ESRCH) => ProcError::Gone,
        _ => ProcError::Unreadable(format!("{what}: {e}")),
    }
}

/// The kinds of /proc read (fault injection, the test matrix of WS/notes/implementer.procfix7.md, diagnostics).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// The pid directory listing of the proc root.
    Listing,
    /// `/proc/<pid>/task` listings.
    Tasks,
    Stat,
    Status,
    Environ,
    Cgroup,
    /// Namespace links / inodes (`/proc/<pid>/ns/*`) other than the time namespace.
    Ns,
    /// `/proc/self/ns/time` (its own kind: gate8 MJ8-1's injected failure).
    NsTime,
    Children,
    /// The owner of `/proc/<pid>` (its effective uid) and the openability checks.
    PidDir,
    Cwd,
    /// `/proc/<pid>/fd` listings and links.
    Fd,
    Statm,
    Smaps,
    Fdinfo,
    /// System files: boot id, meminfo, cpu times, pid_max, uid_map.
    System,
}

impl Kind {
    pub const ALL: [Kind; 16] = [
        Kind::Listing,
        Kind::Tasks,
        Kind::Stat,
        Kind::Status,
        Kind::Environ,
        Kind::Cgroup,
        Kind::Ns,
        Kind::NsTime,
        Kind::Children,
        Kind::PidDir,
        Kind::Cwd,
        Kind::Fd,
        Kind::Statm,
        Kind::Smaps,
        Kind::Fdinfo,
        Kind::System,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Listing => "listing",
            Kind::Tasks => "tasks",
            Kind::Stat => "stat",
            Kind::Status => "status",
            Kind::Environ => "environ",
            Kind::Cgroup => "cgroup",
            Kind::Ns => "ns",
            Kind::NsTime => "nstime",
            Kind::Children => "children",
            Kind::PidDir => "piddir",
            Kind::Cwd => "cwd",
            Kind::Fd => "fd",
            Kind::Statm => "statm",
            Kind::Smaps => "smaps",
            Kind::Fdinfo => "fdinfo",
            Kind::System => "system",
        }
    }

    fn by_name(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.name() == s)
    }
}

/// A proc root: `/proc` in production ([`ProcRoot::system`]), a fake tree in tests ([`ProcRoot::at`]).
#[derive(Clone)]
pub struct ProcRoot {
    dir: PathBuf,
    faults: Option<Arc<Faults>>,
}

impl fmt::Debug for ProcRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcRoot").field("dir", &self.dir).finish()
    }
}

/// procfix7 round 7 (gate11 G11-M1): tries for a proven task list of a multi-threaded process in the completeness walk
/// (measured on this host, 2026-11-17: 814 of 815 multi-threaded processes proven at the first try, 1 at the second,
/// none failed 16).
pub const WALK_TRIES: usize = 16;

/// procfix7 round 8 (gate12 G12-M1): reads of a non-empty children file until two consecutive ones agree.
pub const CHILDREN_READS: usize = 8;

/// The words of the Unreadable answer of [`ProcRoot::children`] for a list that kept changing (a retry may still prove it).
pub const CHILDREN_UNSTABLE: &str = "children list unstable";

/// The production proc root (with the test build's injected faults, if any).
pub fn sys() -> ProcRoot {
    ProcRoot::system()
}

impl ProcRoot {
    pub fn system() -> Self {
        Self {
            dir: PathBuf::from(PROC),
            faults: global_faults(),
        }
    }

    /// A fake proc tree (tests): the same rules on the files below `dir`.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            faults: None,
        }
    }

    /// Test builds: this root with these injected faults.
    pub fn with_faults(mut self, f: Faults) -> Self {
        self.faults = Some(Arc::new(f));
        self
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn is_system(&self) -> bool {
        self.dir == Path::new(PROC)
    }

    fn path(&self, rel: &str) -> PathBuf {
        if rel.is_empty() {
            self.dir.clone()
        } else {
            self.dir.join(rel)
        }
    }

    fn what(&self, rel: &str) -> String {
        self.path(rel).display().to_string()
    }

    fn fault(&self, kind: Kind, pid: Option<u32>) -> Option<Mode> {
        self.faults.as_ref().and_then(|f| f.hit(kind, pid))
    }

    // ------------------------------------------------------------------------------------------------ primitives

    /// Read a whole file (looping until the kernel returns 0; EINTR retried).
    fn read_file(&self, kind: Kind, pid: Option<u32>, rel: &str) -> ProcResult<Vec<u8>> {
        let what = self.what(rel);
        let fault = self.fault(kind, pid);
        if let Some(Mode::Errno(e)) = fault {
            return Err(ProcError::Unreadable(format!(
                "{what}: {} (injected)",
                io::Error::from_raw_os_error(e)
            )));
        }
        let mut f = std::fs::File::open(self.path(rel)).map_err(|e| classify(&e, &what))?;
        let chunk = if fault == Some(Mode::Partial) {
            7
        } else {
            64 * 1024
        };
        let mut out = Vec::new();
        let mut buf = vec![0u8; chunk];
        loop {
            match f.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => out.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(classify(&e, &what)),
            }
        }
        match fault {
            Some(Mode::Short) => out.truncate(out.len() / 2),
            Some(Mode::Garbage) => out = b"\x01garbage\x02 7x".to_vec(),
            _ => {}
        }
        Ok(out)
    }

    fn read_text(&self, kind: Kind, pid: Option<u32>, rel: &str) -> ProcResult<String> {
        let b = self.read_file(kind, pid, rel)?;
        String::from_utf8(b).or_else(|_| unreadable(format!("{}: not UTF-8", self.what(rel))))
    }

    /// A text file whose complete content ends in '\n' (status, cgroup, meminfo, cpu stat, uid_map, fdinfo).
    fn read_lines(&self, kind: Kind, pid: Option<u32>, rel: &str) -> ProcResult<String> {
        let t = self.read_text(kind, pid, rel)?;
        if !t.ends_with('\n') {
            return unreadable(format!(
                "{}: torn read ({} bytes, no line end)",
                self.what(rel),
                t.len()
            ));
        }
        Ok(t)
    }

    fn read_link(&self, kind: Kind, pid: Option<u32>, rel: &str) -> ProcResult<PathBuf> {
        let what = self.what(rel);
        match self.fault(kind, pid) {
            Some(Mode::Errno(e)) => {
                return unreadable(format!(
                    "{what}: {} (injected)",
                    io::Error::from_raw_os_error(e)
                ));
            }
            Some(Mode::Short) | Some(Mode::Garbage) => {
                return unreadable(format!("{what}: garbage link (injected)"));
            }
            _ => {}
        }
        std::fs::read_link(self.path(rel)).map_err(|e| classify(&e, &what))
    }

    fn metadata(&self, kind: Kind, pid: Option<u32>, rel: &str) -> ProcResult<std::fs::Metadata> {
        let what = self.what(rel);
        match self.fault(kind, pid) {
            Some(Mode::Errno(e)) => {
                return unreadable(format!(
                    "{what}: {} (injected)",
                    io::Error::from_raw_os_error(e)
                ));
            }
            Some(Mode::Short) | Some(Mode::Garbage) => {
                return unreadable(format!("{what}: metadata unreadable (injected)"));
            }
            _ => {}
        }
        std::fs::metadata(self.path(rel)).map_err(|e| classify(&e, &what))
    }

    /// The numeric entries of a directory by the libc enumeration (std::fs::read_dir). `garbage`: an entry that starts
    /// with a digit but is not a number.
    fn libc_entries(&self, rel: &str, fault: Option<Mode>) -> ProcResult<Vec<u32>> {
        let what = self.what(rel);
        if let Some(Mode::Errno(e)) = fault {
            return unreadable(format!(
                "{what}: {} (injected)",
                io::Error::from_raw_os_error(e)
            ));
        }
        let rd = std::fs::read_dir(self.path(rel)).map_err(|e| classify(&e, &what))?;
        let mut out = Vec::new();
        for e in rd {
            let e = e.map_err(|e| ProcError::Unreadable(format!("{what} listing: {e}")))?;
            let name = e.file_name();
            let Some(s) = name.to_str() else {
                continue;
            };
            if let Some(n) = numeric_name(s, &what)? {
                out.push(n);
            }
        }
        match fault {
            Some(Mode::Short) => out.truncate(out.len() / 2),
            Some(Mode::Garbage) => {
                numeric_name("7x", &what)?;
            }
            _ => {}
        }
        Ok(out)
    }

    /// The numeric entries of a directory by the raw getdents64 system call (independent of the libc enumeration).
    fn getdents_entries(&self, rel: &str) -> ProcResult<Vec<u32>> {
        use std::os::unix::ffi::OsStrExt;
        let what = self.what(rel);
        let c = std::ffi::CString::new(self.path(rel).as_os_str().as_bytes())
            .map_err(|_| ProcError::Unreadable(format!("{what}: path with NUL")))?;
        // SAFETY: open of a NUL-terminated path; the descriptor is owned below.
        let fd = unsafe {
            libc::open(
                c.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(classify(&io::Error::last_os_error(), &what));
        }
        // SAFETY: a fresh descriptor owned by nobody else.
        let fd = unsafe {
            use std::os::fd::FromRawFd;
            std::os::fd::OwnedFd::from_raw_fd(fd)
        };
        let mut out = Vec::new();
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            use std::os::fd::AsRawFd;
            // SAFETY: getdents64 into our buffer of the given length.
            let n = unsafe {
                libc::syscall(
                    libc::SYS_getdents64,
                    fd.as_raw_fd(),
                    buf.as_mut_ptr(),
                    buf.len(),
                )
            };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return unreadable(format!("{what}: getdents64: {e}"));
            }
            if n == 0 {
                break;
            }
            let n = n as usize;
            let mut off = 0usize;
            while off + 19 <= n {
                let reclen = u16::from_ne_bytes([buf[off + 16], buf[off + 17]]) as usize;
                if reclen < 20 || off + reclen > n {
                    return unreadable(format!("{what}: getdents64 record malformed"));
                }
                let name = &buf[off + 19..off + reclen];
                let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(name.len())];
                if let Ok(s) = std::str::from_utf8(name)
                    && let Some(v) = numeric_name(s, &what)?
                {
                    out.push(v);
                }
                off += reclen;
            }
        }
        Ok(out)
    }

    /// THE LISTING RULE: the numeric entries of `rel` by two independent enumerations; `start_of(entry)` reads the start
    /// time of an entry that one enumeration lacks. Gone only when the directory itself is gone (never for the root).
    fn numeric_listing(
        &self,
        kind: Kind,
        pid: Option<u32>,
        rel: &str,
        start_of: &dyn Fn(u32) -> ProcResult<u64>,
    ) -> ProcResult<Vec<u32>> {
        let fault = self.fault(kind, pid);
        let root = rel.is_empty();
        let to_root = |e: ProcError| match e {
            ProcError::Gone if root => {
                ProcError::Unreadable(format!("{}: the proc root is missing", self.what(rel)))
            }
            other => other,
        };
        let t1 = now_ticks();
        let mut a = self.libc_entries(rel, fault).map_err(to_root)?;
        let t2 = now_ticks();
        let mut b = self.getdents_entries(rel).map_err(to_root)?;
        // test builds: both enumerations end early IN AGREEMENT (gate10 G10-B1: they share the kernel's getdents64)
        if let Some(keep) = self.faults.as_ref().and_then(|f| f.trunc(kind)) {
            a.truncate(keep);
            b.truncate(keep);
        }
        // test builds: one pid missing from both enumerations of the root listing (gate11 m11-mut M6)
        if root && let Some(f) = self.faults.as_ref() {
            for h in [f.hide, f.drop_root].into_iter().flatten() {
                a.retain(|&e| e != h);
                b.retain(|&e| e != h);
            }
        }
        let sa: BTreeSet<u32> = a.iter().copied().collect();
        let sb: BTreeSet<u32> = b.iter().copied().collect();
        // An entry one enumeration lacks: fine if its process started after that enumeration began or is gone now;
        // otherwise it lived through both and the enumeration that lacks it is incomplete.
        for (&e, began, which) in sb
            .difference(&sa)
            .map(|e| (e, t1, "readdir"))
            .chain(sa.difference(&sb).map(|e| (e, t2, "getdents64")))
        {
            match start_of(e) {
                Err(ProcError::Gone) => {}
                Err(ProcError::Unreadable(w)) => {
                    return unreadable(format!(
                        "{}: the two enumerations disagree on {e} and its start time is unreadable ({w})",
                        self.what(rel)
                    ));
                }
                Ok(s) if s.saturating_add(2) > began => {}
                Ok(s) => {
                    return unreadable(format!(
                        "{}: listing incomplete: {e} (start {s} ticks, before the listing began at {began}) missing from the {which} enumeration",
                        self.what(rel)
                    ));
                }
            }
        }
        Ok(sa.union(&sb).copied().collect())
    }

    // ------------------------------------------------------------------------------------------------ typed readers

    /// Every pid of the proc root (the listing rule). procfix7 round 6 (gate10 G10-B1: the two enumerations share the
    /// kernel's getdents64, so they can end early IN AGREEMENT): the listing must also contain its SENTINELS, processes
    /// known to exist: this process and, on the system root, pid 1 of the namespace. A listing without them is
    /// Unreadable (never "no such process"). A listing that decides ABSENCE uses [`ProcRoot::pids_proven`].
    pub fn pids(&self) -> ProcResult<Vec<u32>> {
        let v = self.numeric_listing(Kind::Listing, None, "", &|p| {
            self.stat(p).map(|s| s.start_ticks)
        })?;
        if self.is_system() {
            let me = std::process::id();
            for s in [me, 1] {
                if !v.contains(&s) {
                    return unreadable(format!(
                        "{}: listing incomplete: it lacks pid {s}, which is known to exist ({})",
                        self.what(""),
                        if s == me {
                            "this process"
                        } else {
                            "the namespace's init"
                        }
                    ));
                }
            }
        }
        Ok(v)
    }

    /// procfix7 round 6 (gate10 G10-B1, THE LISTING RULE for absence): the proc root listing WITH a completeness proof:
    /// (1) its sentinels ([`ProcRoot::pids`]) and every pid of `known_alive` the caller knows exists (its live children
    /// from waitid / pidfds) that is still alive; (2) an INDEPENDENT second source that does not go through getdents64:
    /// the children-list walk from pid 1 through `/proc/<p>/task/<p>/children` (a read of the kernel's children lists,
    /// a separate structure): every process it reaches that is alive and started before the listing began must be in
    /// the listing. A missing one makes the listing Unreadable ("listing incomplete"), never a smaller set. procfix7 round
    /// 7 (gate11 G11-M1): the walk reads the children of EVERY thread ([`ProcRoot::children_all_threads_proven`]), from
    /// pid 1 and from this process. It still cannot see: processes outside this pid namespace's view (a parent or sibling
    /// namespace: never descendants of this process), the subtree of pid 2 (kernel threads, never ours), a process
    /// re-parented while the walk passes (missed by the walk only: the walk is a lower bound the listing must contain),
    /// and, under a hidepid /proc, processes of other users.
    pub fn pids_proven(&self, known_alive: &[u32]) -> ProcResult<Vec<u32>> {
        // procfix7 round 6 (gate10 batch_rv): the walk runs BEFORE the listing, so that the caller's reads follow the
        // listing as closely as before (a walk between the listing and the reads widened the window in which a short-
        // lived process listed is gone when read). A process the walk reached that is alive after the listing and
        // started before it began existed during the whole listing: a complete listing contains it.
        let mut reached: BTreeSet<u32> = BTreeSet::new();
        // from the namespace's init and from this process (a process forked by a non-leader thread, e.g. a test harness
        // spawning the supervisor, is not on its parent's leader list; this process's own children are)
        let mut stack: Vec<u32> = vec![1, std::process::id()];
        while let Some(p) = stack.pop() {
            // gate11 G11-M1: the children of EVERY thread (a process forked by a non-leader thread is on that list only)
            match self.children_all_threads_proven(p) {
                Ok(kids) => {
                    for c in kids {
                        if reached.insert(c) {
                            stack.push(c);
                        }
                    }
                }
                // a process (or its leader) gone during the walk: its children were re-parented, the next one reaches them
                Err(ProcError::Gone) => {}
                Err(ProcError::Unreadable(w)) => {
                    return unreadable(format!(
                        "{}: the completeness walk (children lists from pid 1) failed at pid {p}: {w}",
                        self.what("")
                    ));
                }
            }
        }
        let began = now_ticks();
        let v = self.pids()?;
        let listed: BTreeSet<u32> = v.iter().copied().collect();
        for &w in reached.iter().chain(known_alive.iter()) {
            if listed.contains(&w) {
                continue;
            }
            match self.stat(w) {
                Err(ProcError::Gone) => {}                               // exited since
                Ok(st) if st.start_ticks.saturating_add(2) > began => {} // started after the listing began
                Ok(st) => {
                    return unreadable(format!(
                        "{}: listing incomplete: pid {w} ({}, start {}) is alive and was not listed",
                        self.what(""),
                        st.comm,
                        st.start_ticks
                    ));
                }
                Err(ProcError::Unreadable(e)) => {
                    return unreadable(format!(
                        "{}: listing completeness unknown: pid {w} is not listed and its stat is unreadable ({e})",
                        self.what("")
                    ));
                }
            }
        }
        Ok(v)
    }

    /// The thread ids of `pid` (the listing rule). Gone: the process is gone. procfix7 round 6 (G10-B1): the listing
    /// must contain the LEADER (tid == pid: it stays listed, a zombie included, until the group is reaped) and, when the
    /// kernel's atomic thread count (`Threads:`, read before and after) is the same both times, at least that many
    /// threads; otherwise it is Unreadable ("task listing incomplete").
    pub fn tasks(&self, pid: u32) -> ProcResult<Vec<u32>> {
        let before = self.status(pid).and_then(|s| s.threads());
        let v = self.numeric_listing(Kind::Tasks, Some(pid), &format!("{pid}/task"), &|t| {
            self.task_stat(pid, t).map(|s| s.start_ticks)
        })?;
        if !v.contains(&pid) {
            return match self.stat(pid) {
                Err(ProcError::Gone) => Err(ProcError::Gone),
                Err(e) => Err(e),
                Ok(_) => unreadable(format!(
                    "{}: task listing incomplete: it lacks the leader {pid}",
                    self.what(&format!("{pid}/task"))
                )),
            };
        }
        if let (Ok(b), Ok(a)) = (before, self.status(pid).and_then(|s| s.threads()))
            && a == b
            && (v.len() as u32) < a
        {
            return unreadable(format!(
                "{}: task listing incomplete: {} listed, the kernel counts {a} threads (unchanged during the listing)",
                self.what(&format!("{pid}/task")),
                v.len()
            ));
        }
        Ok(v)
    }

    /// `/proc/<pid>/stat`, complete and parsed.
    pub fn stat(&self, pid: u32) -> ProcResult<ProcStat> {
        Ok(self.stat_full(pid, None)?.st)
    }

    /// `/proc/<pid>/task/<tid>/stat` (Gone: that thread is gone). The pid field of the result is `pid`.
    pub fn task_stat(&self, pid: u32, tid: u32) -> ProcResult<ProcStat> {
        Ok(self.stat_full(pid, Some(tid))?.st)
    }

    /// The stat, the environment bounds (fields 50 / 51; None on a kernel that does not report them) and the flags
    /// (field 9).
    fn stat_full(&self, pid: u32, tid: Option<u32>) -> ProcResult<StatFull> {
        let rel = match tid {
            Some(t) => format!("{pid}/task/{t}/stat"),
            None => format!("{pid}/stat"),
        };
        let mut b = self.read_file(Kind::Stat, Some(pid), &rel)?;
        // An empty read (a task being released): read again; only ENOENT / ESRCH makes it Gone.
        for _ in 0..2 {
            if !b.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
            b = self.read_file(Kind::Stat, Some(pid), &rel)?;
        }
        if b.is_empty() {
            return unreadable(format!("{}: empty", self.what(&rel)));
        }
        if b.last() != Some(&b'\n') {
            return unreadable(format!(
                "{}: torn read ({} bytes, no line end)",
                self.what(&rel),
                b.len()
            ));
        }
        let text = String::from_utf8_lossy(&b);
        let st = parse_stat(pid, &text, page_size())
            .ok_or_else(|| ProcError::Unreadable(format!("{}: unparsable", self.what(&rel))))?;
        let fields: Vec<&str> = text
            .rfind(')')
            .and_then(|c| text.get(c + 2..))
            .map(|t| t.split_whitespace().collect())
            .unwrap_or_default();
        let env = match (
            fields.get(47).and_then(|v| v.parse::<u64>().ok()),
            fields.get(48).and_then(|v| v.parse::<u64>().ok()),
        ) {
            (Some(s), Some(e)) => Some((s, e)),
            _ => None,
        };
        let flags = fields
            .get(6)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        Ok(StatFull { st, env, flags })
    }

    /// `/proc/<pid>/status`, complete.
    pub fn status(&self, pid: u32) -> ProcResult<Status> {
        let rel = format!("{pid}/status");
        Ok(Status {
            text: self.read_lines(Kind::Status, Some(pid), &rel)?,
            what: self.what(&rel),
        })
    }

    /// The environment of `pid` (through the live thread `tid` when the main thread is a zombie), COMPLETE (its length
    /// equals the bounds the kernel reads), bounded by `timeout` (an environ read takes the target's mmap lock).
    /// Gone: for `tid`, that thread is gone.
    pub fn environ(&self, pid: u32, tid: Option<u32>, timeout: Duration) -> ProcResult<Vec<u8>> {
        let me = self.clone();
        match run_bounded(timeout, move || me.environ_now(pid, tid)) {
            Bounded::Done(r) => r,
            Bounded::TimedOut => unreadable(format!(
                "environ of {pid}: no answer within {} ms",
                timeout.as_millis()
            )),
            Bounded::SpawnFailed(e) => {
                unreadable(format!("environ of {pid}: no helper thread: {e}"))
            }
        }
    }

    fn environ_now(&self, pid: u32, tid: Option<u32>) -> ProcResult<Vec<u8>> {
        let rel = match tid {
            Some(t) => format!("{pid}/task/{t}/environ"),
            None => format!("{pid}/environ"),
        };
        let mut last: String;
        let mut changed = 0u32;
        // gate8 m8-ew: the EXEC WINDOW is judged by evidence, not by a constant re-read: inside execve the new image's
        // environment bounds are not set yet (stat fields 50 / 51 read 0 / 0) and the environ reads empty. While that
        // evidence holds (a live process, not a kernel thread, the same start time), the read is repeated with a growing
        // pause, at most EXEC_WINDOW_BOUND; still in the window then: Unreadable (never "no marker").
        let window_ends = std::time::Instant::now() + EXEC_WINDOW_BOUND;
        let mut nap = Duration::from_micros(100);
        loop {
            let a = self.stat_full(pid, tid)?;
            let env = self.read_file(Kind::Environ, Some(pid), &rel)?;
            let b = self.stat_full(pid, tid)?;
            if a.st.start_ticks != b.st.start_ticks || a.env != b.env {
                changed += 1;
                last = "the process changed (exec) during the read".into();
                if changed >= 3 {
                    break;
                }
                continue;
            }
            let in_window = env.is_empty()
                && a.env == Some((0, 0))
                && !a.st.is_dead()
                && a.flags & PF_KTHREAD == 0;
            if in_window {
                if std::time::Instant::now() >= window_ends {
                    return unreadable(format!(
                        "{}: still inside execve after {} ms (environment bounds not set)",
                        self.what(&rel),
                        EXEC_WINDOW_BOUND.as_millis()
                    ));
                }
                std::thread::sleep(nap);
                nap = (nap * 2).min(Duration::from_millis(5));
                continue;
            }
            match a.env {
                Some((start, end)) => {
                    let want = end.saturating_sub(start);
                    if env.len() as u64 == want {
                        return Ok(env);
                    }
                    last = format!("torn read: {} of {want} bytes", env.len());
                }
                // No bounds reported (an old kernel, or a fake tree): two equal reads.
                None => {
                    let again = self.read_file(Kind::Environ, Some(pid), &rel)?;
                    if again == env {
                        return Ok(env);
                    }
                    last = "two reads differ".into();
                }
            }
            changed += 1;
            if changed >= 3 {
                break;
            }
        }
        unreadable(format!("{}: {last}", self.what(&rel)))
    }

    /// Can `/proc/<pid>/environ` be opened? Ok: yes. Gone. Unreadable: refused (EACCES: non-dumpable, setuid) or any
    /// other failure (the caller treats both as "cannot be inspected").
    pub fn environ_access(&self, pid: u32) -> ProcResult<()> {
        let rel = format!("{pid}/environ");
        let what = self.what(&rel);
        match self.fault(Kind::PidDir, Some(pid)) {
            Some(Mode::Errno(e)) => {
                return unreadable(format!(
                    "{what}: {} (injected)",
                    io::Error::from_raw_os_error(e)
                ));
            }
            Some(Mode::Short) | Some(Mode::Garbage) => {
                return unreadable(format!("{what}: open refused (injected)"));
            }
            _ => {}
        }
        std::fs::File::open(self.path(&rel))
            .map(|_| ())
            .map_err(|e| classify(&e, &what))
    }

    /// The owner of `/proc/<pid>`: the process's EFFECTIVE uid (also for a non-dumpable process; kernel threads: 0).
    pub fn owner_uid(&self, pid: u32) -> ProcResult<u32> {
        Ok(self
            .metadata(Kind::PidDir, Some(pid), &pid.to_string())?
            .uid())
    }

    /// `/proc/<pid>/task/<tid>/children`, complete (Gone: that thread is gone). procfix7 round 8 (gate12 G12-M1): the
    /// kernel builds the file element by element, re-taking its lock for each; the next element is found from the
    /// previous one while that one is still a child, otherwise BY INDEX (fs/proc/array.c `get_children_pid`), also when a
    /// longer file is continued in a new seq_file buffer: children that left the list (reaped, or moved away because the
    /// thread exits) ahead of that index shift it and the read SKIPS live children. So an empty read is taken as it is
    /// (its first step found no child), and any other read only when the next read agrees with it (at most
    /// [`CHILDREN_READS`] reads), else Unreadable ([`CHILDREN_UNSTABLE`]). Why two agreeing reads are complete: a skip
    /// needs an entry the read ALREADY returned to leave the list, and such an entry cannot be in the following read, so
    /// the two differ. Entries added meanwhile go to the tail and shift nothing.
    pub fn children(&self, pid: u32, tid: u32) -> ProcResult<Vec<u32>> {
        let rel = format!("{pid}/task/{tid}/children");
        let mut prev = self.children_once(pid, &rel)?;
        if prev.is_empty() {
            return Ok(prev);
        }
        prev.sort_unstable();
        for _ in 1..CHILDREN_READS {
            let mut next = self.children_once(pid, &rel)?;
            next.sort_unstable();
            if next == prev {
                return Ok(next);
            }
            prev = next;
        }
        unreadable(format!(
            "{}: {CHILDREN_UNSTABLE}: no two consecutive of {CHILDREN_READS} reads agreed (the kernel builds the list by \
             index when entries leave it, which skips live children)",
            self.what(&rel)
        ))
    }

    /// procfix7 round 8b (item c'): the children of every thread of `pid`, each list read ONCE, whatever can be read
    /// (for processes whose lists cannot hold a marked process: [`ProcRoot::children_all_threads_proven`]).
    fn children_best_effort(&self, pid: u32) -> Vec<u32> {
        let tids = self.tasks(pid).unwrap_or_else(|_| vec![pid]);
        let mut out = Vec::new();
        for t in tids {
            if let Ok(k) = self.children_once(pid, &format!("{pid}/task/{t}/children")) {
                out.extend(k);
            }
        }
        out
    }

    /// procfix7 round 8b (item c'): is `pid` an ancestor of THIS process (the re-parent targets of its descendants)? The
    /// chain is read once per process AND PER PROC ROOT (pid and start time of each ancestor; procfix7 round 9, gate13
    /// m13-chain: one process-wide chain was filled by the first caller's root, so a fake root of a test decided the
    /// answer for the real one) and compared with the start time now.
    fn is_own_ancestor(&self, pid: u32) -> bool {
        type Chains = std::sync::Mutex<std::collections::BTreeMap<PathBuf, Vec<(u32, u64)>>>;
        static CHAINS: std::sync::OnceLock<Chains> = std::sync::OnceLock::new();
        let chain = {
            let mut m = CHAINS
                .get_or_init(Default::default)
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            m.entry(self.dir.clone())
                .or_insert_with(|| {
                    let mut v = Vec::new();
                    let mut p = std::process::id();
                    for _ in 0..64 {
                        let Ok(st) = self.stat(p) else { break };
                        if st.ppid == 0 {
                            break;
                        }
                        p = st.ppid;
                        match self.stat(p) {
                            Ok(a) => v.push((p, a.start_ticks)),
                            Err(_) => break,
                        }
                    }
                    v
                })
                .clone()
        };
        chain
            .iter()
            .any(|&(a, start)| a == pid && self.stat(pid).is_ok_and(|st| st.start_ticks == start))
    }

    /// One read of a children file.
    fn children_once(&self, pid: u32, rel: &str) -> ProcResult<Vec<u32>> {
        let b = self.read_file(Kind::Children, Some(pid), rel)?;
        if !b.is_empty() && b.last() != Some(&b' ') {
            return unreadable(format!(
                "{}: torn read ({} bytes, no trailing space)",
                self.what(rel),
                b.len()
            ));
        }
        let t = std::str::from_utf8(&b)
            .map_err(|_| ProcError::Unreadable(format!("{}: not UTF-8", self.what(rel))))?;
        let hide = self.faults.as_ref().and_then(|f| f.hide);
        t.split_whitespace()
            .map(|w| {
                w.parse::<u32>().map_err(|_| {
                    ProcError::Unreadable(format!("{}: unparsable {w:?}", self.what(rel)))
                })
            })
            // test builds: the hidden pid is on no children list either (gate11 m11-mut M6)
            .filter(|c| c.as_ref().map_or(true, |&c| Some(c) != hide))
            .collect()
    }

    /// procfix7 round 7 (gate11 G11-M1): the children of EVERY thread of `pid`, for the completeness walk. A process
    /// forked by a NON-leader thread (or an orphan re-parented onto the live non-leader thread of a subreaper) is on that
    /// thread's list only. Single-threaded (status `Threads:` <= 1): the leader's list. Otherwise the task list is
    /// trusted only with its completeness proof: the leader listed ([`ProcRoot::tasks`]), the kernel's `Threads:` the same
    /// before and after the listing and equal to the number listed, and the SAME task list after the children lists were
    /// read (a thread that exits hands its children to another thread of the group). Up to [`WALK_TRIES`] tries with a
    /// back-off, else Unreadable (never a partial answer). THIS process is exempt (its own descendants are covered by the
    /// kernel's wait answer at the end): one read of each thread's list. Gone: the process is gone.
    pub fn children_all_threads_proven(&self, pid: u32) -> ProcResult<Vec<u32>> {
        // procfix7 round 8: THIS process (the supervisor) is exempt from the proof, as since round 7 (round 7 kept the
        // children it could read after 16 tries): its descendants are covered by the kernel's wait answer at the end (own-tree check, atomic). One read
        // of each thread's list: a job that forks without a pause keeps the supervisor's own list (its re-parented
        // incarnations and their zombies) changing for good, and proving it would cost every scan up to 16 x 8 reads of
        // a list of thousands of entries (procfix9 g9_b2_fork_churner_live_session: no exit within 900 s).
        if pid == std::process::id() {
            let mut out = Vec::new();
            for t in self.tasks(pid)? {
                match self.children_once(pid, &format!("{pid}/task/{t}/children")) {
                    Ok(k) => out.extend(k),
                    Err(ProcError::Gone) => {}
                    Err(e) => return Err(e),
                }
            }
            return Ok(out);
        }
        let status = self.status(pid)?;
        let first = status.threads()?;
        // procfix7 round 8b (host step 1, item c'): a children list needs its PROOF only where it can hold a marked
        // process. A marked process descends by fork from a job leader of ours, or was re-parented to a subreaper among
        // its ancestors; a child's REAL uid is its parent's unless the child has the privilege to change it, so its
        // possible parents have the real uid ours or 0 (root), or are ancestors of this supervisor (the re-parent
        // targets). Any other process (another user's, e.g. a builder with hundreds of churning threads: 7 unverified /
        // 3 unproven ends on the host) is read ONCE, best effort, never "unprovable".
        let must_prove = match status.real_uid() {
            Ok(u) => u == super::our_uid() || u == 0 || self.is_own_ancestor(pid),
            Err(_) => true,
        };
        if !must_prove {
            return Ok(self.children_best_effort(pid));
        }
        let mut why = String::new();
        if first <= 1 {
            // procfix7 round 8 (gate12 G12-M1): a children list that keeps changing is "not proven yet" (try again)
            for attempt in 0..WALK_TRIES {
                if attempt > 0 {
                    std::thread::sleep(Duration::from_micros(250u64 << attempt.min(5)));
                }
                match self.children(pid, pid) {
                    Err(ProcError::Unreadable(w)) if w.contains(CHILDREN_UNSTABLE) => why = w,
                    r => return r,
                }
            }
            return unreadable(format!(
                "{}: the children list cannot be proven complete after {WALK_TRIES} tries ({why})",
                self.what(&format!("{pid}/task/{pid}"))
            ));
        }
        for attempt in 0..WALK_TRIES {
            // threads that come and go (helper threads) leave no quiet moment at once: back off (0.25 ms .. 8 ms)
            if attempt > 0 {
                std::thread::sleep(Duration::from_micros(250u64 << attempt.min(5)));
            }
            let before = self.status(pid)?.threads()?;
            // one thread exiting and another starting during the listing keeps the count EQUAL while the listing misses
            // the newcomer: `tasks` then says "incomplete"; here that only means "not proven yet" (try again)
            let mut ts = match self.tasks(pid) {
                Ok(v) => v,
                Err(ProcError::Unreadable(w)) if w.contains("task listing incomplete") => {
                    why = w;
                    continue;
                }
                Err(e) => return Err(e),
            };
            let after = self.status(pid)?.threads()?;
            ts.sort_unstable();
            let listed = ts.clone();
            // procfix7 round 8 (gate12 stream, 10 ms): a thread that EXITS during the reads hands its children to the first
            // thread of the group not exiting (forget_original_parent -> find_alive_thread, under the tasklist lock, its
            // exit state set in the same step) but stays listed and counted until it is released: a pass that read the
            // receiver before the move and the exiting thread after it lacks those children while the task list and
            // Threads: still agree. That receiver is the LEADER while the leader is not exiting (it heads the group's thread
            // list): the leader's list is read LAST and its PF_EXITING (never cleared) must still be clear after the reads.
            // A leader exiting or a zombie: each thread's exit state is read before and after the reads instead; one that
            // ENTERED an exit state (Z / X) or is gone after them = not proven yet.
            let lead = self.stat_full(pid, Some(pid))?;
            let leader_holds = !lead.st.is_dead() && lead.flags & PF_EXITING == 0;
            if leader_holds && let Some(i) = ts.iter().position(|&t| t == pid) {
                let l = ts.remove(i);
                ts.push(l);
            }
            let dead_before: Vec<bool> = if leader_holds {
                Vec::new()
            } else {
                ts.iter()
                    .map(|&t| matches!(self.task_stat(pid, t), Ok(st) if st.is_dead()))
                    .collect()
            };
            let mut out = Vec::new();
            let mut thread_gone = false;
            let mut unstable = None;
            for &t in &ts {
                match self.children(pid, t) {
                    Ok(k) => out.extend(k),
                    Err(ProcError::Gone) => {
                        thread_gone = true;
                        break;
                    }
                    // procfix7 round 8 (gate12 G12-M1): a children list that kept changing: not proven yet
                    Err(ProcError::Unreadable(w)) if w.contains(CHILDREN_UNSTABLE) => {
                        unstable = Some(w);
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
            if let Some(w) = unstable {
                why = w;
                continue;
            }
            let entered_exit = !thread_gone
                && if leader_holds {
                    self.stat_full(pid, Some(pid))
                        .map_or(true, |l| l.st.is_dead() || l.flags & PF_EXITING != 0)
                } else {
                    ts.iter().zip(&dead_before).any(|(&t, &d0)| {
                        !d0 && self.task_stat(pid, t).map_or(true, |st| st.is_dead())
                    })
                };
            if entered_exit {
                why = "a thread exited while the children lists were read (its children moved to another thread)"
                    .to_string();
                continue;
            }
            if before != after || ts.len() != after as usize {
                why = format!(
                    "{} listed, the kernel counted {before} then {after} threads",
                    ts.len()
                );
                continue;
            }
            let mut again = self.tasks(pid).unwrap_or_default();
            again.sort_unstable();
            if thread_gone || again != listed {
                why = "its threads changed while their children lists were read".to_string();
                continue;
            }
            return Ok(out);
        }
        unreadable(format!(
            "{}: the children of every thread cannot be proven complete after {WALK_TRIES} tries ({why})",
            self.what(&format!("{pid}/task"))
        ))
    }

    /// The children of every thread of `pid`, read until two consecutive reads agree (at most 4; the kernel documents
    /// the files as unreliable while the lists change). Gone: the process is gone. A thread that exits between its
    /// listing and its read reports nothing (its children are re-parented and seen by the next read).
    pub fn children_of(&self, pid: u32) -> ProcResult<BTreeSet<u32>> {
        let once = || -> ProcResult<BTreeSet<u32>> {
            let mut out = BTreeSet::new();
            for tid in self.tasks(pid)? {
                match self.children(pid, tid) {
                    Ok(v) => out.extend(v),
                    Err(ProcError::Gone) => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(out)
        };
        let mut prev = once()?;
        for _ in 0..3 {
            let next = once()?;
            if next == prev {
                return Ok(next);
            }
            prev = next;
        }
        unreadable(format!(
            "children of {pid}: the lists kept changing over 4 reads"
        ))
    }

    /// `/proc/<pid>/cgroup` (the unified line `0::<path>`), bounded.
    pub fn cgroup_v2(&self, pid: u32, timeout: Duration) -> ProcResult<CgroupLine> {
        let me = self.clone();
        let t = match run_bounded(timeout, move || {
            me.read_lines(Kind::Cgroup, Some(pid), &format!("{pid}/cgroup"))
        }) {
            Bounded::Done(r) => r?,
            Bounded::TimedOut => {
                return unreadable(format!("cgroup of {pid}: no answer within the bound"));
            }
            Bounded::SpawnFailed(e) => return unreadable(format!("cgroup of {pid}: {e}")),
        };
        Ok(cgroup_line(&t))
    }

    /// The complete text of `/proc/<pid>/cgroup`, bounded.
    pub fn cgroup_text(&self, pid: u32, timeout: Duration) -> ProcResult<String> {
        let me = self.clone();
        match run_bounded(timeout, move || {
            me.read_lines(Kind::Cgroup, Some(pid), &format!("{pid}/cgroup"))
        }) {
            Bounded::Done(r) => r,
            Bounded::TimedOut => unreadable(format!("cgroup of {pid}: no answer within the bound")),
            Bounded::SpawnFailed(e) => unreadable(format!("cgroup of {pid}: {e}")),
        }
    }

    /// `/proc/self/cgroup` (the unified line).
    pub fn self_cgroup_v2(&self) -> ProcResult<CgroupLine> {
        Ok(cgroup_line(&self.read_lines(
            Kind::Cgroup,
            None,
            "self/cgroup",
        )?))
    }

    /// The inode of `/proc/self/ns/<kind>` (Gone: the kernel has no such namespace type).
    pub fn self_ns_ino(&self, kind: &str) -> ProcResult<u64> {
        Ok(self
            .metadata(ns_kind(kind), None, &format!("self/ns/{kind}"))?
            .ino())
    }

    /// The link text of `/proc/self/ns/<kind>` (`pid:[4026531836]`).
    pub fn self_ns_link(&self, kind: &str) -> ProcResult<String> {
        let l = self.read_link(ns_kind(kind), None, &format!("self/ns/{kind}"))?;
        let s = l.to_string_lossy().into_owned();
        if !s.starts_with(&format!("{kind}:[")) || !s.ends_with(']') {
            return unreadable(format!("self/ns/{kind}: unexpected link {s:?}"));
        }
        Ok(s)
    }

    /// `/proc/<pid>/cwd`.
    pub fn cwd(&self, pid: u32) -> ProcResult<PathBuf> {
        self.read_link(Kind::Cwd, Some(pid), &format!("{pid}/cwd"))
    }

    /// The targets of every descriptor of `pid` (Gone: the process is gone; a descriptor closed while listed is left
    /// out).
    pub fn fd_targets(&self, pid: u32) -> ProcResult<Vec<PathBuf>> {
        self.fd_targets_rel(Some(pid), &format!("{pid}/fd"))
    }

    /// The descriptors of this process with their targets.
    pub fn self_fds(&self) -> ProcResult<Vec<(i32, PathBuf)>> {
        let rel = "self/fd";
        let what = self.what(rel);
        match self.fault(Kind::Fd, None) {
            Some(Mode::Errno(e)) => {
                return unreadable(format!(
                    "{what}: {} (injected)",
                    io::Error::from_raw_os_error(e)
                ));
            }
            Some(Mode::Short) | Some(Mode::Garbage) => {
                return unreadable(format!("{what}: garbage descriptor table (injected)"));
            }
            _ => {}
        }
        let rd = std::fs::read_dir(self.path(rel))
            .map_err(|e| ProcError::Unreadable(format!("{what}: {e}")))?;
        let mut out = Vec::new();
        for e in rd {
            let e = e.map_err(|e| ProcError::Unreadable(format!("{what} listing: {e}")))?;
            let Some(n) = e.file_name().to_str().and_then(|s| s.parse::<i32>().ok()) else {
                continue;
            };
            match std::fs::read_link(e.path()) {
                Ok(t) => out.push((n, t)),
                // the listing's own descriptor, closed by now
                Err(e) if e.raw_os_error() == Some(libc::ENOENT) => {}
                Err(e) => return unreadable(format!("{what}/{n}: {e}")),
            }
        }
        Ok(out)
    }

    fn fd_targets_rel(&self, pid: Option<u32>, rel: &str) -> ProcResult<Vec<PathBuf>> {
        let what = self.what(rel);
        match self.fault(Kind::Fd, pid) {
            Some(Mode::Errno(e)) => {
                return unreadable(format!(
                    "{what}: {} (injected)",
                    io::Error::from_raw_os_error(e)
                ));
            }
            Some(Mode::Short) | Some(Mode::Garbage) => {
                return unreadable(format!("{what}: garbage descriptor table (injected)"));
            }
            _ => {}
        }
        let rd = std::fs::read_dir(self.path(rel)).map_err(|e| classify(&e, &what))?;
        let mut out = Vec::new();
        for e in rd {
            let e = e.map_err(|e| ProcError::Unreadable(format!("{what} listing: {e}")))?;
            match std::fs::read_link(e.path()) {
                Ok(t) => out.push(t),
                // a descriptor closed between the listing and the read
                Err(e) if e.raw_os_error() == Some(libc::ENOENT) => {}
                Err(e) => return unreadable(format!("{what}: {e}")),
            }
        }
        Ok(out)
    }

    /// `Pid:` of `/proc/self/fdinfo/<fd>` for a pidfd: Ok(None) when its process has exited (-1 / 0).
    pub fn self_fdinfo_pid(&self, fd: i32) -> ProcResult<PidfdPid> {
        let rel = format!("self/fdinfo/{fd}");
        let t = self.read_lines(Kind::Fdinfo, None, &rel)?;
        let v = t
            .lines()
            .find_map(|l| l.strip_prefix("Pid:"))
            .ok_or_else(|| ProcError::Unreadable(format!("{}: no Pid line", self.what(&rel))))?
            .trim()
            .parse::<i64>()
            .map_err(|e| ProcError::Unreadable(format!("{}: Pid: {e}", self.what(&rel))))?;
        Ok(match u32::try_from(v) {
            Ok(p) if p > 0 => PidfdPid::Pid(p),
            _ => PidfdPid::Exited,
        })
    }

    /// Exact resident set size of `pid` from `statm` (field 2, pages), in bytes.
    pub fn statm_rss(&self, pid: u32) -> ProcResult<u64> {
        let rel = format!("{pid}/statm");
        let t = self.read_lines(Kind::Statm, Some(pid), &rel)?;
        t.split_whitespace()
            .nth(1)
            .and_then(|v| v.parse::<u64>().ok())
            .and_then(|p| p.checked_mul(page_size()))
            .ok_or_else(|| ProcError::Unreadable(format!("{}: unparsable", self.what(&rel))))
    }

    /// `Pss:` of `/proc/<pid>/smaps_rollup` in bytes, bounded.
    pub fn smaps_pss(&self, pid: u32, timeout: Duration) -> ProcResult<u64> {
        let me = self.clone();
        let rel = format!("{pid}/smaps_rollup");
        let what = self.what(&rel);
        let t = match run_bounded(timeout, move || me.read_lines(Kind::Smaps, Some(pid), &rel)) {
            Bounded::Done(r) => r?,
            Bounded::TimedOut => return unreadable(format!("{what}: no answer within the bound")),
            Bounded::SpawnFailed(e) => return unreadable(format!("{what}: {e}")),
        };
        t.lines()
            .find_map(|l| {
                l.strip_prefix("Pss:")
                    .and_then(|r| r.split_whitespace().next())
                    .and_then(|kb| kb.parse::<u64>().ok())
                    .and_then(|kb| kb.checked_mul(1024))
            })
            .ok_or_else(|| ProcError::Unreadable(format!("{what}: no Pss line")))
    }

    // ------------------------------------------------------------------------------------------------ system files

    /// `/proc/sys/kernel/random/boot_id` (a 36-character UUID).
    pub fn boot_id(&self) -> ProcResult<String> {
        let rel = "sys/kernel/random/boot_id";
        let t = self.read_lines(Kind::System, None, rel)?;
        let b = t.trim().to_string();
        if b.len() != 36 || !b.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-') {
            return unreadable(format!("{}: not a boot id: {b:?}", self.what(rel)));
        }
        Ok(b)
    }

    /// `/proc/meminfo` (complete text).
    pub fn meminfo(&self) -> ProcResult<String> {
        self.read_lines(Kind::System, None, "meminfo")
    }

    /// `/proc/stat` (complete text).
    pub fn cpu_stat(&self) -> ProcResult<String> {
        self.read_lines(Kind::System, None, "stat")
    }

    /// `/proc/sys/kernel/pid_max`.
    pub fn pid_max(&self) -> ProcResult<u32> {
        let rel = "sys/kernel/pid_max";
        let t = self.read_lines(Kind::System, None, rel)?;
        t.trim()
            .parse::<u32>()
            .map_err(|e| ProcError::Unreadable(format!("{}: {e}", self.what(rel))))
    }

    /// `/proc/self/uid_map` (complete text).
    pub fn self_uid_map(&self) -> ProcResult<String> {
        self.read_lines(Kind::System, None, "self/uid_map")
    }
}

fn ns_kind(kind: &str) -> Kind {
    if kind == "time" {
        Kind::NsTime
    } else {
        Kind::Ns
    }
}

/// A stat with the fields the environ rule needs.
struct StatFull {
    st: ProcStat,
    /// (env_start, env_end), fields 50 / 51 (None: not reported).
    env: Option<(u64, u64)>,
    /// Field 9.
    flags: u64,
}

/// `PF_KTHREAD` of the stat flags: a kernel thread (no environment, never a window).
const PF_KTHREAD: u64 = 0x0020_0000;

/// `PF_EXITING` of the stat flags: the task has begun to exit (set at the start of do_exit, never cleared).
const PF_EXITING: u64 = 0x0000_0004;

/// The longest exec window waited for (gate8 measured up to 1.8 ms; under a CPU quota it stretches).
pub const EXEC_WINDOW_BOUND: Duration = Duration::from_millis(100);

/// What a pidfd's fdinfo names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PidfdPid {
    Pid(u32),
    /// -1 / 0: its process has exited.
    Exited,
}

/// The unified (v2) line of a cgroup file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CgroupLine {
    /// `0::<path>`.
    Path(String),
    /// The file was read completely and has no unified line (a v1-only host).
    NoUnified,
}

fn cgroup_line(t: &str) -> CgroupLine {
    match t.lines().find_map(|l| l.strip_prefix("0::")) {
        Some(p) => CgroupLine::Path(p.trim().to_string()),
        None => CgroupLine::NoUnified,
    }
}

/// A complete `/proc/<pid>/status`.
#[derive(Clone, Debug)]
pub struct Status {
    text: String,
    what: String,
}

impl Status {
    /// The value of `key:` (trimmed); a missing key is Unreadable.
    pub fn field(&self, key: &str) -> ProcResult<&str> {
        let k = format!("{key}:");
        self.text
            .lines()
            .find_map(|l| l.strip_prefix(&k))
            .map(str::trim)
            .ok_or_else(|| ProcError::Unreadable(format!("{}: no {key} line", self.what)))
    }

    fn num(&self, key: &str, idx: usize) -> ProcResult<u32> {
        let v = self.field(key)?;
        v.split_whitespace()
            .nth(idx)
            .and_then(|x| x.parse::<u32>().ok())
            .ok_or_else(|| ProcError::Unreadable(format!("{}: {key} unparsable {v:?}", self.what)))
    }

    /// The real uid (first field of `Uid:`).
    pub fn real_uid(&self) -> ProcResult<u32> {
        self.num("Uid", 0)
    }

    pub fn ppid(&self) -> ProcResult<u32> {
        self.num("PPid", 0)
    }

    /// procfix7 round 5 (gate9 G9-B2): `Threads:` is the kernel's `signal->nr_threads`, ONE count read under the
    /// sighand lock: every thread of the group that is not yet released, the ZOMBIE LEADER included (a group leader is
    /// released only after every other thread). Unlike a task LISTING it cannot miss a thread created after it began.
    pub fn threads(&self) -> ProcResult<u32> {
        self.num("Threads", 0)
    }

    /// The state letter of `State:` (the leader thread's; `Z` for a zombie leader).
    pub fn state(&self) -> ProcResult<char> {
        let v = self.field("State")?;
        v.chars()
            .next()
            .ok_or_else(|| ProcError::Unreadable(format!("{}: State empty", self.what)))
    }

    /// `Name:` (diagnostics).
    pub fn name(&self) -> ProcResult<String> {
        self.field("Name").map(str::to_string)
    }

    /// `NSpid:` fields (the pid in each nested namespace, outermost first).
    pub fn nspid(&self) -> ProcResult<Vec<u32>> {
        let v = self.field("NSpid")?;
        v.split_whitespace()
            .map(|x| {
                x.parse::<u32>().map_err(|_| {
                    ProcError::Unreadable(format!("{}: NSpid unparsable {v:?}", self.what))
                })
            })
            .collect()
    }
}

/// A directory entry name: Ok(Some(n)) a number, Ok(None) a non-numeric name (`self`, `sys`, ...), Unreadable for a
/// name that starts with a digit but is not a number (garbage).
fn numeric_name(s: &str, what: &str) -> ProcResult<Option<u32>> {
    if !s.starts_with(|c: char| c.is_ascii_digit()) {
        return Ok(None);
    }
    s.parse::<u32>()
        .map(Some)
        .map_err(|_| ProcError::Unreadable(format!("{what}: garbage entry {s:?}")))
}

/// Now on the start-time clock of /proc (CLOCK_BOOTTIME in clock ticks; inside a time namespace both are shifted alike).
fn now_ticks() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes into our timespec.
    let r = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    if r != 0 {
        // Unknown: treat every disagreement as incomplete (u64::MAX: every start is "before the listing began").
        return u64::MAX;
    }
    let hz = super::ticks_per_second();
    ((ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9) * hz) as u64
}

// ------------------------------------------------------------------------------------------------ fault injection

/// An injected fault mode (test builds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Errno(i32),
    /// A torn read: the second half missing (a listing: the libc enumeration ends early, no error).
    Short,
    /// Unparsable content.
    Garbage,
    /// Every read() returns at most 7 bytes (a POSIX short read): the result must be complete.
    Partial,
}

impl Mode {
    pub const ALL: [Mode; 7] = [
        Mode::Errno(libc::EMFILE),
        Mode::Errno(libc::EIO),
        Mode::Errno(libc::EACCES),
        Mode::Errno(libc::ENOMEM),
        Mode::Short,
        Mode::Garbage,
        Mode::Partial,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Errno(libc::EMFILE) => "emfile",
            Mode::Errno(libc::EIO) => "eio",
            Mode::Errno(libc::EACCES) => "eacces",
            Mode::Errno(libc::ENOMEM) => "enomem",
            Mode::Errno(_) => "errno",
            Mode::Short => "short",
            Mode::Garbage => "garbage",
            Mode::Partial => "partial",
        }
    }

    fn by_name(s: &str) -> Option<Mode> {
        Some(match s {
            "emfile" => Mode::Errno(libc::EMFILE),
            "eio" => Mode::Errno(libc::EIO),
            "eacces" => Mode::Errno(libc::EACCES),
            "enomem" => Mode::Errno(libc::ENOMEM),
            "short" => Mode::Short,
            "garbage" => Mode::Garbage,
            "partial" => Mode::Partial,
            _ => return None,
        })
    }
}

/// Injected faults: every `every`-th read of a kind (optionally of one pid) fails with a mode.
#[derive(Debug, Default)]
pub struct Faults {
    rules: Vec<Rule>,
    /// procfix7 round 6 (gate10 G10-B1, mutation tests): BOTH enumerations of the proc root listing end after this many
    /// numeric entries, in agreement, without an error (`proc_trunc_at:<n>` keeps n - 1 entries).
    trunc_root: Option<usize>,
    /// The same for task listings (`proc_trunc_tasks_at:<n>`).
    trunc_tasks: Option<usize>,
    /// procfix7 round 7 (gate11 m11-mut M6): `proc_hide_pid:<pid>`: that pid is missing from BOTH enumerations of the
    /// proc root listing AND from every children list (a process no listing and no walk shows: only a direct read of
    /// its own pid, the sweeps' seeds, can find it).
    hide: Option<u32>,
    /// procfix7 round 7 (gate11 G11-M1 tests): `proc_drop_root_pid:<pid>`: that pid is missing from BOTH enumerations of
    /// the proc root listing only (the completeness walk may still reach it).
    drop_root: Option<u32>,
}

#[derive(Debug)]
struct Rule {
    kind: Kind,
    mode: Mode,
    every: u64,
    pid: Option<u32>,
    count: std::sync::atomic::AtomicU64,
}

impl Faults {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule: every `every`-th read of `kind` (of `pid` only, if given; listings ignore the pid) fails with `mode`.
    pub fn rule(mut self, kind: Kind, mode: Mode, every: u64, pid: Option<u32>) -> Self {
        self.rules.push(Rule {
            kind,
            mode,
            every: every.max(1),
            pid,
            count: std::sync::atomic::AtomicU64::new(0),
        });
        self
    }

    fn hit(&self, kind: Kind, pid: Option<u32>) -> Option<Mode> {
        use std::sync::atomic::Ordering;
        for r in &self.rules {
            if r.kind != kind {
                continue;
            }
            if let (Some(want), Some(p)) = (r.pid, pid)
                && want != p
            {
                continue;
            }
            if r.pid.is_some() && pid.is_none() && !matches!(kind, Kind::Listing) {
                continue;
            }
            let n = r.count.fetch_add(1, Ordering::SeqCst) + 1;
            if n % r.every == 0 {
                return Some(r.mode);
            }
        }
        None
    }

    /// From the test build's failpoint value (`proc_<kind>_<mode>:<every>`, `proc_pid:<pid>`, `proc_trunc_at:<n>`,
    /// `proc_trunc_tasks_at:<n>`); None without a proc rule.
    pub fn parse(v: &str) -> Option<Faults> {
        let mut pid = None;
        let mut rules = Vec::new();
        let (mut trunc_root, mut trunc_tasks) = (None, None);
        let mut hide = None;
        let mut drop_root = None;
        for p in v.split(',').map(str::trim) {
            let Some((site, n)) = p.rsplit_once(':') else {
                continue;
            };
            let Ok(n) = n.parse::<u64>() else {
                continue;
            };
            if site == "proc_pid" {
                pid = u32::try_from(n).ok();
                continue;
            }
            if site == "proc_trunc_at" {
                trunc_root = usize::try_from(n.saturating_sub(1)).ok();
                continue;
            }
            if site == "proc_trunc_tasks_at" {
                trunc_tasks = usize::try_from(n.saturating_sub(1)).ok();
                continue;
            }
            if site == "proc_hide_pid" {
                hide = u32::try_from(n).ok();
                continue;
            }
            if site == "proc_drop_root_pid" {
                drop_root = u32::try_from(n).ok();
                continue;
            }
            let Some(rest) = site.strip_prefix("proc_") else {
                continue;
            };
            let Some((k, m)) = rest.rsplit_once('_') else {
                continue;
            };
            if let (Some(k), Some(m)) = (Kind::by_name(k), Mode::by_name(m)) {
                rules.push((k, m, n));
            }
        }
        if rules.is_empty()
            && trunc_root.is_none()
            && trunc_tasks.is_none()
            && hide.is_none()
            && drop_root.is_none()
        {
            return None;
        }
        let mut f = Faults::new();
        for (k, m, n) in rules {
            f = f.rule(k, m, n, pid);
        }
        f.trunc_root = trunc_root;
        f.trunc_tasks = trunc_tasks;
        f.hide = hide;
        f.drop_root = drop_root;
        Some(f)
    }

    /// Test builds: both enumerations of the proc root listing keep only their first `keep` numeric entries.
    pub fn truncate_root(mut self, keep: usize) -> Self {
        self.trunc_root = Some(keep);
        self
    }

    /// Test builds: both enumerations of every task listing keep only their first `keep` numeric entries.
    pub fn truncate_tasks(mut self, keep: usize) -> Self {
        self.trunc_tasks = Some(keep);
        self
    }

    /// The truncation of a listing of `kind` (tests), if any.
    fn trunc(&self, kind: Kind) -> Option<usize> {
        match kind {
            Kind::Listing => self.trunc_root,
            Kind::Tasks => self.trunc_tasks,
            _ => None,
        }
    }
}

/// The faults of the production root: test builds parse `RR_CAMPAIGN_FAILPOINT` once; a release build has none.
fn global_faults() -> Option<Arc<Faults>> {
    #[cfg(debug_assertions)]
    {
        static G: std::sync::OnceLock<Option<Arc<Faults>>> = std::sync::OnceLock::new();
        G.get_or_init(|| {
            std::env::var(crate::supervisor::failpoint::FAILPOINT_ENV)
                .ok()
                .and_then(|v| Faults::parse(&v))
                .map(Arc::new)
        })
        .clone()
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
#[cfg(test)]
mod tests {
    use super::*;

    fn me() -> u32 {
        std::process::id()
    }

    /// procfix7 round 6 (gate10 G10-B1): a proc root listing that both enumerations end early IN AGREEMENT is never a
    /// complete answer: truncated before this process (its sentinel) it is Unreadable; truncated before a live orphan
    /// that the children-list walk from pid 1 reaches (a setsid'd daemon re-parented to the namespace's init), the
    /// proven listing is Unreadable as well; the full listing passes both.
    #[test]
    fn a_truncated_proc_listing_is_never_complete() {
        if !crate::launch::in_private_namespace() {
            eprintln!("SKIPPED: spawns a process: only inside a private pid namespace (rule L10)");
            return;
        }
        let out = std::process::Command::new("sh")
            .args(["-c", "setsid sleep 30 </dev/null >/dev/null 2>&1 & echo $!"])
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .output()
            .unwrap();
        let daemon: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
        // the orphan is re-parented to the namespace's init (this test binary is pid 1 under LIGHT_PIDNS)
        let t0 = std::time::Instant::now();
        while !ProcRoot::system()
            .children(1, 1)
            .is_ok_and(|k| k.contains(&daemon))
        {
            assert!(
                t0.elapsed() < Duration::from_secs(10),
                "the orphan never reached pid 1's list"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        // older than the listing's start-time slack (2 clock ticks: a process started DURING a listing may be missing)
        std::thread::sleep(Duration::from_millis(100));
        let full = ProcRoot::system().pids_proven(&[]).unwrap();
        assert!(full.contains(&daemon) && full.contains(&me()));
        // truncated before everything: the sentinels are missing
        let r = ProcRoot::system().with_faults(Faults::new().truncate_root(0));
        assert!(
            matches!(r.pids(), Err(ProcError::Unreadable(w)) if w.contains("listing incomplete")),
            "an empty listing is never 'no process'"
        );
        // truncated just before the daemon (the sentinels this process and pid 1 kept when they come first)
        let idx = full.iter().position(|&p| p == daemon).unwrap();
        let r = ProcRoot::system().with_faults(Faults::new().truncate_root(idx));
        let kept: Vec<u32> = full[..idx].to_vec();
        if kept.contains(&me()) && kept.contains(&1) {
            assert!(r.pids().is_ok(), "the sentinels alone are present");
            assert!(
                matches!(r.pids_proven(&[]), Err(ProcError::Unreadable(w)) if w.contains(&format!("pid {daemon}"))),
                "the walk from pid 1 reaches the hidden orphan"
            );
        } else {
            assert!(matches!(r.pids_proven(&[]), Err(ProcError::Unreadable(_))));
        }
        // a caller's known live process missing from a truncated listing is Unreadable too
        let r = ProcRoot::system().with_faults(Faults::new().truncate_root(idx));
        assert!(matches!(
            r.pids_proven(&[daemon]),
            Err(ProcError::Unreadable(_))
        ));
        let _ = std::process::Command::new("sh")
            .args(["-c", &format!("kill -KILL {daemon}")])
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .status();
    }

    /// procfix7 round 6 (gate10 G10-B1, every listing truncated at every k): the TASK listing of a process with a
    /// stable thread count (a child with 4 threads that only sleep), ended early by both enumerations in agreement after
    /// k entries for every k below the count, is never a complete answer (the leader sentinel when k = 0, the kernel's
    /// unchanged Threads: count otherwise); the full listing passes.
    #[test]
    fn a_truncated_task_listing_is_never_complete() {
        if !crate::launch::in_private_namespace() {
            eprintln!("SKIPPED: spawns a process: only inside a private pid namespace (rule L10)");
            return;
        }
        let mut child = std::process::Command::new("python3")
            .args([
                "-I",
                "-c",
                "import threading,time\n[threading.Thread(target=time.sleep,args=(60,),daemon=True).start() for _ in range(3)]\ntime.sleep(60)",
            ])
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .stdin(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        let t0 = std::time::Instant::now();
        while ProcRoot::system()
            .status(pid)
            .and_then(|s| s.threads())
            .ok()
            != Some(4)
        {
            assert!(
                t0.elapsed() < Duration::from_secs(20),
                "the child never ran 4 threads"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let full = ProcRoot::system().tasks(pid).unwrap();
        assert_eq!(full.len(), 4, "{full:?}");
        for k in 0..full.len() {
            let r = ProcRoot::system().with_faults(Faults::new().truncate_tasks(k));
            match r.tasks(pid) {
                Err(ProcError::Unreadable(w)) => {
                    assert!(w.contains("task listing incomplete"), "k {k}: {w}")
                }
                other => {
                    panic!("k {k}: a task listing truncated after {k} of 4 entries: {other:?}")
                }
            }
        }
        let r = ProcRoot::system().with_faults(Faults::new().truncate_tasks(full.len()));
        assert_eq!(r.tasks(pid).unwrap().len(), 4);
        let _ = child.kill();
        let _ = child.wait();
    }

    /// Every kind of read of THIS process on the real /proc, under every fault mode: never Gone, never a wrong value.
    /// Errno modes and short / garbage reads are Unreadable; a partial read is complete and equal to the clean read.
    #[test]
    fn every_kind_under_every_mode_is_unreadable_or_complete_never_gone() {
        let pid = me();
        let read = |r: &ProcRoot, k: Kind| -> ProcResult<String> {
            Ok(match k {
                Kind::Listing => format!("{:?}", r.pids()?.contains(&pid)),
                Kind::Tasks => format!("{:?}", r.tasks(pid)?.contains(&pid)),
                Kind::Stat => format!("{}", r.stat(pid)?.start_ticks),
                Kind::Status => r.status(pid)?.ppid()?.to_string(),
                Kind::Environ => format!("{}", r.environ(pid, None, Duration::from_secs(5))?.len()),
                Kind::Cgroup => format!("{:?}", r.self_cgroup_v2()?),
                Kind::Ns => format!("{} {}", r.self_ns_ino("pid")?, r.self_ns_link("pid")?),
                Kind::NsTime => format!("{} {}", r.self_ns_ino("time")?, r.self_ns_link("time")?),
                Kind::Children => format!("{:?}", r.children(pid, pid)?),
                Kind::PidDir => format!("{} {:?}", r.owner_uid(pid)?, r.environ_access(pid)),
                Kind::Cwd => r.cwd(pid)?.display().to_string(),
                Kind::Fd => format!("{}", !r.fd_targets(pid)?.is_empty()),
                Kind::Statm => format!("{}", r.statm_rss(pid)? > 0),
                Kind::Smaps => format!("{}", r.smaps_pss(pid, Duration::from_secs(5))? > 0),
                Kind::Fdinfo => {
                    // fdinfo of a pidfd on ourselves
                    // SAFETY: pidfd_open on our own pid; closed below.
                    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
                    let v = r.self_fdinfo_pid(fd);
                    // SAFETY: closing the descriptor we opened.
                    unsafe { libc::close(fd) };
                    format!("{:?}", v?)
                }
                Kind::System => format!(
                    "{} {} {} {} {}",
                    r.boot_id()?,
                    r.meminfo()?.len() > 10,
                    r.cpu_stat()?.len() > 10,
                    r.pid_max()?,
                    r.self_uid_map()?.len() > 2
                ),
            })
        };
        let clean = ProcRoot::system();
        let mut table = Vec::new();
        for k in Kind::ALL {
            let want = read(&clean, k).unwrap_or_else(|e| panic!("clean read of {k:?}: {e}"));
            for m in Mode::ALL {
                let r = ProcRoot::system().with_faults(Faults::new().rule(k, m, 1, None));
                let got = read(&r, k);
                match (m, &got) {
                    (_, Err(ProcError::Gone)) => panic!("{k:?} under {m:?}: GONE"),
                    (Mode::Partial, Ok(v)) => assert_eq!(v, &want, "{k:?} partial reads"),
                    // sysctl files (/proc/sys: boot id, pid_max) end a read that does not start at offset 0 (the
                    // kernel's integer / string handlers): a small-buffer read of them IS torn, and is Unreadable
                    (Mode::Partial, Err(ProcError::Unreadable(w))) if k == Kind::System => {
                        assert!(w.contains("torn read"), "{k:?} partial reads: {w}")
                    }
                    (Mode::Partial, Err(e)) => panic!("{k:?} partial reads: {e}"),
                    // a fault of a kind that this composite does not read in the injected way never makes it wrong
                    (_, Ok(v)) => assert_eq!(
                        v, &want,
                        "{k:?} under {m:?} gave a value that differs from the clean read"
                    ),
                    (_, Err(ProcError::Unreadable(_))) => {}
                }
                table.push(format!(
                    "{} x {}: {}",
                    k.name(),
                    m.name(),
                    match &got {
                        Ok(_) => "complete",
                        Err(ProcError::Unreadable(_)) => "UNREADABLE",
                        Err(ProcError::Gone) => "GONE",
                    }
                ));
            }
        }
        // The errno and garbage modes are Unreadable for every kind (no composite hides them).
        for line in &table {
            if line.contains(" x emfile")
                || line.contains(" x eio")
                || line.contains(" x eacces")
                || line.contains(" x enomem")
                || line.contains(" x garbage")
            {
                assert!(line.ends_with("UNREADABLE"), "{line}");
            }
        }
        eprintln!("{}", table.join("\n"));
    }

    /// A fake tree: missing entries are Gone, the proc root is never Gone, a torn stat is Unreadable.
    #[test]
    fn gone_only_for_enoent_and_never_for_the_root() {
        let d = std::env::temp_dir().join(format!("rr-procfs-{}-{}", me(), now_ticks()));
        std::fs::create_dir_all(d.join("5")).unwrap();
        std::fs::write(
            d.join("5/stat"),
            "5 (x) S 1 5 5 0 -1 0 0 0 0 0 1 1 0 0 20 0 1 0 77 0 0\n",
        )
        .unwrap();
        let r = ProcRoot::at(&d);
        assert_eq!(r.stat(5).unwrap().start_ticks, 77);
        assert_eq!(r.stat(6), Err(ProcError::Gone));
        std::fs::write(d.join("5/stat"), "5 (x) S 1 5 5 0 -1 0 0 0").unwrap();
        assert!(matches!(r.stat(5), Err(ProcError::Unreadable(_))));
        let missing = ProcRoot::at(d.join("nope"));
        assert!(matches!(missing.pids(), Err(ProcError::Unreadable(_))));
        assert_eq!(missing.tasks(5), Err(ProcError::Gone));
        // garbage entry in a listing
        std::fs::create_dir_all(d.join("12x")).unwrap();
        assert!(matches!(r.pids(), Err(ProcError::Unreadable(_))));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A listing whose libc enumeration ends early (no error) is caught by the getdents64 enumeration: every live old
    /// process lacks from it -> Unreadable.
    #[test]
    fn a_silently_truncated_listing_is_unreadable() {
        // procfix7 round 7: the property, not one message text (the old expectation passed or failed with the tests
        // running beside it): a listing whose readdir enumeration is halved is Unreadable, or it lacks only processes
        // started too recently to be required (2 ticks before the listing; the rule excuses those). Never an OLD process
        // silently missing.
        let full = ProcRoot::system().pids().unwrap();
        let t0 = now_ticks();
        let r =
            ProcRoot::system().with_faults(Faults::new().rule(Kind::Listing, Mode::Short, 1, None));
        match r.pids() {
            Err(ProcError::Unreadable(_)) => {}
            Ok(v) => {
                for p in full.iter().filter(|p| !v.contains(p)) {
                    match ProcRoot::system().stat(*p) {
                        Err(ProcError::Gone) => {}
                        Ok(st) => assert!(
                            st.start_ticks.saturating_add(2) > t0,
                            "the old process {p} (start {}) is silently missing from {v:?}",
                            st.start_ticks
                        ),
                        Err(e) => panic!("{p}: {e:?}"),
                    }
                }
            }
            Err(e) => panic!("{e:?}"),
        }
        let r = ProcRoot::system();
        assert!(r.pids().unwrap().contains(&me()));
    }

    #[test]
    fn faults_parse_from_the_failpoint_value() {
        let f = Faults::parse("after_spawn:2,proc_stat_eio:2,proc_pid:42").unwrap();
        assert_eq!(f.hit(Kind::Stat, Some(41)), None);
        assert_eq!(f.hit(Kind::Stat, Some(42)), None);
        assert_eq!(f.hit(Kind::Stat, Some(42)), Some(Mode::Errno(libc::EIO)));
        assert!(Faults::parse("after_spawn:2").is_none());
    }

    /// procfix7 round 8b (host step 1, item c'): a children list needs its proof only where it can hold a marked
    /// process. Two multi-threaded processes whose task list can never be proven (Threads: 3, two tasks listed): one of
    /// ANOTHER user (real uid 4242) -> its lists are read once, best effort, Ok with its children; one of OUR real uid ->
    /// Unreadable after the tries (a marked process could be on its lists).
    #[test]
    fn round8b_a_foreign_process_never_makes_the_walk_unprovable() {
        let d = std::env::temp_dir().join(format!("rr-procfs-8b-{}-{}", me(), now_ticks()));
        let mk = |pid: u32, ruid: u32, kids: &str| {
            let p = d.join(pid.to_string());
            for t in [pid, pid + 1] {
                std::fs::create_dir_all(p.join(format!("task/{t}"))).unwrap();
                std::fs::write(
                    p.join(format!("task/{t}/stat")),
                    format!("{t} (x) S 1 {pid} {pid} 0 -1 0 0 0 0 0 1 1 0 0 20 0 3 0 77 0 0\n"),
                )
                .unwrap();
                std::fs::write(
                    p.join(format!("task/{t}/children")),
                    if t == pid { kids } else { "" },
                )
                .unwrap();
            }
            std::fs::write(
                p.join("stat"),
                format!("{pid} (x) S 1 {pid} {pid} 0 -1 0 0 0 0 0 1 1 0 0 20 0 3 0 77 0 0\n"),
            )
            .unwrap();
            std::fs::write(
                p.join("status"),
                format!("Name:\tx\nUid:\t{ruid}\t{ruid}\t{ruid}\t{ruid}\nThreads:\t3\n"),
            )
            .unwrap();
        };
        let ours = crate::resources::our_uid();
        let foreign = if ours == 4242 { 4343 } else { 4242 };
        mk(500, foreign, "777 778 ");
        mk(600, ours, "888 ");
        let r = ProcRoot::at(&d);
        let mut v = r.children_all_threads_proven(500).unwrap();
        v.sort_unstable();
        assert_eq!(
            v,
            vec![777, 778],
            "another user's process: read once, never unprovable"
        );
        match r.children_all_threads_proven(600) {
            Err(ProcError::Unreadable(w)) => assert!(w.contains("cannot be proven"), "{w}"),
            other => panic!("our own unprovable process must stay Unreadable: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
