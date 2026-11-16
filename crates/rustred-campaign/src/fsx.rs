//! THE FILESYSTEM FACTS RULE (procfix7 round 5, gate9 G9-B1 and its class over the whole crate).
//!
//! gate9 G9-B1: a STAT error of ledger/MANIFEST.json (EIO) was taken as "no ledger" (`Path::exists` maps every error
//! to false), the ledger directory's listing dropped its entry errors (`read_dir(..).flatten()`: "no residue"), and
//! the supervisor CREATED A NEW LEDGER over a crashed campaign, deleting its history, then exited 0 with a live marked
//! process. The rule, for every existence check, listing and read of this crate whose answer decides something:
//!
//! - [`presence`] / [`presence_lstat`]: `Present(metadata)`, `Absent` ONLY on a proven NotFound (ENOENT), or
//!   `Unknown(error)` for everything else (EIO, ENOMEM, EACCES, ENOTDIR, ELOOP, ...). Every decision site handles
//!   `Unknown` explicitly: a typed stop, or the conservative branch. `Path::exists`, `is_file`, `is_dir`,
//!   `try_exists`, `metadata(..).is_ok()` are forbidden in this crate (clippy `disallowed_methods`, clippy.toml;
//!   tests/procfix9_tripwire.rs).
//! - [`list`]: a listing is `Complete(entries)` or `Failed` (any entry error fails the whole listing) or `Absent`
//!   (the directory itself is NotFound). An incomplete listing never means "empty", "no residue", "no jobs".
//! - [`read`] / [`read_to_string`]: `Content`, `Absent` (NotFound only) or `Failed`; a read error is never "no file"
//!   and never an empty / default content.
//!
//! Every call names its SITE (a short static label: the decision it serves) for diagnostics and fault injection.
//!
//! Fault injection (TEST BUILDS ONLY, compiled out of release builds): parts of `RR_CAMPAIGN_FAILPOINT`:
//! `fsx_at:<n>` the n-th helper call of the process fails, `fsx_from:<n>` every call from the n-th on fails,
//! `fsx_errno:<e>` the errno (default EIO = 5), `fsx_count:1` the supervisor logs the number of helper calls at exit
//! (`TEST BUILD fsx helper calls: N`), `fsx_site:<k>` every call at the k-th site of [`SITES`] (1-based) fails. A failing
//! presence check answers Unknown, a failing listing Failed, a failing read Failed.

use std::ffi::OsString;
use std::fs::{FileType, Metadata};
use std::io;
use std::path::{Path, PathBuf};

/// The answer of an existence check.
#[derive(Debug)]
pub enum Presence {
    Present(Metadata),
    /// Proven absent: the call failed with NotFound (ENOENT).
    Absent,
    /// Anything else: the object may exist; never "absent".
    Unknown(io::Error),
}

impl Presence {
    pub fn is_present(&self) -> bool {
        matches!(self, Presence::Present(_))
    }
    /// Present and a directory (lstat: not a symlink to one).
    pub fn is_dir(&self) -> bool {
        matches!(self, Presence::Present(m) if m.is_dir())
    }
    pub fn is_file(&self) -> bool {
        matches!(self, Presence::Present(m) if m.is_file())
    }
    /// The metadata of a present object; Absent is a NotFound error, Unknown its error (for `?` call sites where any
    /// non-presence is an error, like `std::fs::metadata(p)?`).
    pub fn metadata(self) -> io::Result<Metadata> {
        match self {
            Presence::Present(m) => Ok(m),
            Presence::Absent => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
            Presence::Unknown(e) => Err(e),
        }
    }
}

/// One entry of a complete listing.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: OsString,
    pub path: PathBuf,
    /// The entry's type as the directory reports it (no extra stat).
    pub file_type: FileType,
}

impl Entry {
    pub fn name_lossy(&self) -> String {
        self.name.to_string_lossy().into_owned()
    }
}

/// The answer of a directory listing.
#[derive(Debug)]
pub enum Listing {
    /// Every entry was read.
    Complete(Vec<Entry>),
    /// The directory does not exist (NotFound).
    Absent,
    /// The listing failed or is incomplete (an entry error): never "empty".
    Failed(io::Error),
}

/// The answer of a whole-file read.
#[derive(Debug)]
pub enum Read<T> {
    Content(T),
    /// NotFound only.
    Absent,
    Failed(io::Error),
}

impl<T> Read<T> {
    /// Into a Result where Absent is an error too (`what` names the file): for files that must exist.
    pub fn required(self, what: &Path) -> io::Result<T> {
        match self {
            Read::Content(t) => Ok(t),
            Read::Absent => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}: not found", what.display()),
            )),
            Read::Failed(e) => Err(e),
        }
    }
}

/// The decision sites (labels) of this crate; `fsx_site:<k>` injects at the k-th (test builds).
pub const SITES: &[&str] = &[
    "ledger.manifest",
    "ledger.dir",
    "ledger.listing",
    "ledger.segment",
    "ledger.snapshot",
    "supervisor.control",
    "supervisor.run_dir",
    "supervisor.dirs",
    "supervisor.job_spec",
    "supervisor.outcome",
    "supervisor.sentinel",
    "lock.file",
    "atomic.target",
    "atomic.listing",
    "workdir",
    "guard.fallback",
    "guard.reserve",
    "cgroup",
    "cli",
    "closeloop.input",
    "closeloop.output",
    "adapters.outcome",
    "progress",
    "tui",
    "cpu_policy",
    "idlock",
    "real.program",
];

fn is_absent(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::NotFound
}

/// Existence by `stat` (follows symlinks).
#[allow(clippy::disallowed_methods)] // the one place that calls the std forms
pub fn presence(site: &'static str, p: &Path) -> Presence {
    if let Some(e) = fault(site) {
        return Presence::Unknown(e);
    }
    match std::fs::metadata(p) {
        Ok(m) => Presence::Present(m),
        Err(e) if is_absent(&e) => Presence::Absent,
        Err(e) => Presence::Unknown(e),
    }
}

/// Existence by `lstat` (does not follow a final symlink).
#[allow(clippy::disallowed_methods)]
pub fn presence_lstat(site: &'static str, p: &Path) -> Presence {
    if let Some(e) = fault(site) {
        return Presence::Unknown(e);
    }
    match std::fs::symlink_metadata(p) {
        Ok(m) => Presence::Present(m),
        Err(e) if is_absent(&e) => Presence::Absent,
        Err(e) => Presence::Unknown(e),
    }
}

/// A COMPLETE listing of `dir` (any entry error fails the listing).
#[allow(clippy::disallowed_methods)]
pub fn list(site: &'static str, dir: &Path) -> Listing {
    if let Some(e) = fault(site) {
        return Listing::Failed(e);
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if is_absent(&e) => return Listing::Absent,
        Err(e) => return Listing::Failed(e),
    };
    let mut out = Vec::new();
    for e in rd {
        let e = match e {
            Ok(e) => e,
            Err(e) => return Listing::Failed(e),
        };
        let file_type = match e.file_type() {
            Ok(t) => t,
            Err(e) => return Listing::Failed(e),
        };
        out.push(Entry {
            name: e.file_name(),
            path: e.path(),
            file_type,
        });
    }
    Listing::Complete(out)
}

/// A complete listing as a Result: Absent and Failed are errors (`what` for the message).
pub fn list_required(site: &'static str, dir: &Path) -> io::Result<Vec<Entry>> {
    match list(site, dir) {
        Listing::Complete(v) => Ok(v),
        Listing::Absent => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{}: not found", dir.display()),
        )),
        Listing::Failed(e) => Err(io::Error::new(
            e.kind(),
            format!("{}: listing failed: {e}", dir.display()),
        )),
    }
}

/// Read a whole file.
#[allow(clippy::disallowed_methods)]
pub fn read(site: &'static str, p: &Path) -> Read<Vec<u8>> {
    if let Some(e) = fault(site) {
        return Read::Failed(e);
    }
    match std::fs::read(p) {
        Ok(b) => Read::Content(b),
        Err(e) if is_absent(&e) => Read::Absent,
        Err(e) => Read::Failed(e),
    }
}

/// Open a file for reading (for a partial read: a header line).
pub fn open(site: &'static str, p: &Path) -> Read<std::fs::File> {
    if let Some(e) = fault(site) {
        return Read::Failed(e);
    }
    match std::fs::File::open(p) {
        Ok(f) => Read::Content(f),
        Err(e) if is_absent(&e) => Read::Absent,
        Err(e) => Read::Failed(e),
    }
}

/// Read a whole UTF-8 file (invalid UTF-8 = Failed).
pub fn read_to_string(site: &'static str, p: &Path) -> Read<String> {
    match read(site, p) {
        Read::Content(b) => match String::from_utf8(b) {
            Ok(s) => Read::Content(s),
            Err(e) => Read::Failed(io::Error::new(io::ErrorKind::InvalidData, e)),
        },
        Read::Absent => Read::Absent,
        Read::Failed(e) => Read::Failed(e),
    }
}

/// The canonical path of `p` (symlinks resolved). A path that does not exist YET is resolved through its parent (the
/// canonical parent joined with the last component); every other error is an error (never the raw path: a comparison
/// of raw paths can miss an overlap through a symlink).
#[allow(clippy::disallowed_methods)]
pub fn canonical(site: &'static str, p: &Path) -> io::Result<PathBuf> {
    if let Some(e) = fault(site) {
        return Err(e);
    }
    match std::fs::canonicalize(p) {
        Ok(c) => Ok(c),
        Err(e) if is_absent(&e) => match (p.parent(), p.file_name()) {
            (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
                Ok(canonical(site, parent)?.join(name))
            }
            _ => Err(e),
        },
        Err(e) => Err(e),
    }
}

/// Rename `from` to `to` ONLY if `to` does not exist (renameat2 RENAME_NOREPLACE): a move aside never overwrites.
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let a = std::ffi::CString::new(from.as_os_str().as_bytes())
        .map_err(|_| io::Error::other("path with NUL"))?;
    let b = std::ffi::CString::new(to.as_os_str().as_bytes())
        .map_err(|_| io::Error::other("path with NUL"))?;
    // SAFETY: renameat2 with two NUL-terminated paths relative to the cwd; integers only otherwise.
    let r = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            a.as_ptr(),
            libc::AT_FDCWD,
            b.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

// ------------------------------------------------------------------------------------------------ fault injection

#[cfg(debug_assertions)]
mod inject {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};

    pub struct Cfg {
        pub at: Option<u64>,
        pub from: Option<u64>,
        pub site: Option<usize>,
        pub errno: i32,
        pub count: bool,
    }

    pub static CALLS: AtomicU64 = AtomicU64::new(0);
    /// Per-site call counts (`fsx_count:1` only).
    pub static BY_SITE: std::sync::Mutex<std::collections::BTreeMap<&'static str, u64>> =
        std::sync::Mutex::new(std::collections::BTreeMap::new());

    pub fn cfg() -> &'static Option<Cfg> {
        static C: OnceLock<Option<Cfg>> = OnceLock::new();
        C.get_or_init(|| {
            let v = std::env::var(crate::supervisor::failpoint::FAILPOINT_ENV).ok()?;
            let mut c = Cfg {
                at: None,
                from: None,
                site: None,
                errno: libc::EIO,
                count: false,
            };
            let mut any = false;
            for p in v.split(',').map(str::trim) {
                let Some((k, n)) = p.rsplit_once(':') else {
                    continue;
                };
                let Ok(n) = n.parse::<u64>() else {
                    continue;
                };
                match k {
                    "fsx_at" => c.at = Some(n),
                    "fsx_from" => c.from = Some(n),
                    "fsx_site" => c.site = usize::try_from(n).ok(),
                    "fsx_errno" => c.errno = i32::try_from(n).unwrap_or(libc::EIO),
                    "fsx_count" => c.count = true,
                    _ => continue,
                }
                any = true;
            }
            any.then_some(c)
        })
    }

    pub fn hit(site: &'static str) -> Option<std::io::Error> {
        let c = cfg().as_ref()?;
        let n = CALLS.fetch_add(1, Ordering::SeqCst) + 1;
        if c.count {
            *BY_SITE
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .entry(site)
                .or_default() += 1;
        }
        // `fsx_site:<k>` is 1-based (the failpoint syntax refuses 0)
        let by_site = c
            .site
            .and_then(|k| super::SITES.get(k.checked_sub(1)?))
            .is_some_and(|s| *s == site);
        if c.at == Some(n) || c.from.is_some_and(|f| n >= f) || by_site {
            return Some(std::io::Error::from_raw_os_error(c.errno));
        }
        None
    }
}

#[cfg(debug_assertions)]
fn fault(site: &'static str) -> Option<io::Error> {
    inject::hit(site)
}

#[cfg(not(debug_assertions))]
#[inline(always)]
fn fault(_site: &'static str) -> Option<io::Error> {
    None
}

/// Test builds with `fsx_count:1`: the helper calls so far per site label (empty otherwise and in release builds).
pub fn site_counts() -> Vec<(&'static str, u64)> {
    #[cfg(debug_assertions)]
    {
        inject::BY_SITE
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect()
    }
    #[cfg(not(debug_assertions))]
    {
        Vec::new()
    }
}

/// Test builds with `fsx_count:1`: the number of helper calls so far (None otherwise and in release builds).
pub fn calls_counted() -> Option<u64> {
    #[cfg(debug_assertions)]
    {
        match inject::cfg() {
            Some(c) if c.count => Some(inject::CALLS.load(std::sync::atomic::Ordering::SeqCst)),
            _ => None,
        }
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

    #[test]
    fn absent_only_for_not_found() {
        let d = std::env::temp_dir().join(format!("rr-fsx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(matches!(presence("cli", &d), Presence::Absent));
        assert!(matches!(list("cli", &d), Listing::Absent));
        assert!(matches!(read("cli", &d.join("x")), Read::Absent));
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("f"), b"x").unwrap();
        assert!(presence("cli", &d).is_dir());
        assert!(presence("cli", &d.join("f")).is_file());
        // ENOTDIR: a component is not a directory: Unknown, never Absent
        assert!(matches!(
            presence("cli", &d.join("f").join("y")),
            Presence::Unknown(_)
        ));
        match list("cli", &d) {
            Listing::Complete(v) => assert_eq!(v.len(), 2),
            other => panic!("{other:?}"),
        }
        // a directory read as a file: Failed (EISDIR), not Absent
        assert!(matches!(read("cli", &d.join("sub")), Read::Failed(_)));
        // rename without replacing
        std::fs::write(d.join("g"), b"y").unwrap();
        assert!(rename_noreplace(&d.join("f"), &d.join("g")).is_err());
        assert_eq!(std::fs::read(d.join("g")).unwrap(), b"y");
        rename_noreplace(&d.join("f"), &d.join("h")).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }
}
