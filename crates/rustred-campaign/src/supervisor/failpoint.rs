//! Failpoints for interrupt tests and rehearsals (plan 6.5 I1; interfaces/supervisor.md section 14):
//! `RR_CAMPAIGN_FAILPOINT=<site>:<n>` aborts the supervisor itself (SIGABRT, no unwinding) at the n-th pass of `<site>` (1-based). The
//! special site `ledger_enospc` makes the n-th ledger append and every later one fail with ENOSPC.
//!
//! TEST BUILDS ONLY (procfix3, gate-A M-2): the variable is read and the sites exist only with `debug_assertions` (the
//! profile `cargo test` and the test binaries' `rr_campaign` use). A release build (the user's run) compiles all of it out:
//! it never reads `RR_CAMPAIGN_FAILPOINT`, has no abort site and no ENOSPC seam (checked with `strings` on the release
//! binary, see notes/implementer.procfix3.md). REQUEST TO I-INT: a dedicated cargo feature `failpoints` (off by default)
//! would let a release-mode rehearsal opt in explicitly; until then debug builds are the only ones with failpoints.

use crate::ledger::FileOps;
use std::sync::Arc;

/// The variable (test builds only read it).
pub const FAILPOINT_ENV: &str = "RR_CAMPAIGN_FAILPOINT";

#[derive(Debug, Default)]
pub struct Failpoints {
    #[cfg(debug_assertions)]
    site: Option<String>,
    #[cfg(debug_assertions)]
    n: u64,
    #[cfg(debug_assertions)]
    passes: std::sync::atomic::AtomicU64,
    /// procfix5: further sites of a comma-separated value (`after_spawn:2,no_spawn_barrier:1`), each with its own count.
    #[cfg(debug_assertions)]
    more: Vec<Failpoints>,
}

impl Failpoints {
    /// From `RR_CAMPAIGN_FAILPOINT` in a test build; always inactive in a release build.
    pub fn from_env() -> Failpoints {
        #[cfg(debug_assertions)]
        {
            match std::env::var(FAILPOINT_ENV) {
                Ok(v) => Self::parse(&v),
                Err(_) => Failpoints::default(),
            }
        }
        #[cfg(not(debug_assertions))]
        {
            Failpoints::default()
        }
    }

    /// procfix7 (gate6 m-x): every comma-separated part of the value must be `<site>:<n>` with an integer n >= 1. A part
    /// without `:n` used to be dropped silently, so a test with a void failpoint looked like a pass: now the supervisor
    /// refuses to start (exit 1) naming the part. Test builds only (a release build refuses the variable altogether).
    pub fn check(v: &str) -> Result<(), String> {
        for p in v.split(',').map(str::trim) {
            match p.rsplit_once(':') {
                Some((site, n)) if !site.is_empty() && n.parse::<u64>().is_ok_and(|n| n >= 1) => {}
                _ => {
                    return Err(format!(
                        "{FAILPOINT_ENV}={v:?}: part {p:?} is not <site>:<n> with n >= 1: refused (a void failpoint would make the test look like a pass)"
                    ));
                }
            }
        }
        Ok(())
    }

    /// [`Failpoints::check`] of `RR_CAMPAIGN_FAILPOINT` when it is set (test builds); Ok in a release build.
    pub fn check_env() -> Result<(), String> {
        #[cfg(debug_assertions)]
        {
            match std::env::var(FAILPOINT_ENV) {
                Ok(v) => Self::check(&v),
                Err(std::env::VarError::NotPresent) => Ok(()),
                Err(e) => Err(format!("{FAILPOINT_ENV}: {e}")),
            }
        }
        #[cfg(not(debug_assertions))]
        {
            Ok(())
        }
    }

    /// procfix7: the number of a configured NON-counting site (`release_delay_ms:<ms>`: a value, not a pass count).
    /// Always None in a release build.
    #[cfg(debug_assertions)]
    pub fn number(&self, site: &str) -> Option<u64> {
        if self.site.as_deref() == Some(site) {
            return Some(self.n);
        }
        self.more.iter().find_map(|f| f.number(site))
    }

    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub fn number(&self, _site: &str) -> Option<u64> {
        None
    }

    /// The configured site, if any (for the supervisor's log line).
    pub fn active(&self) -> Option<String> {
        #[cfg(debug_assertions)]
        {
            self.site.as_ref().map(|s| {
                std::iter::once(format!("{s}:{}", self.n))
                    .chain(self.more.iter().filter_map(Failpoints::active))
                    .collect::<Vec<_>>()
                    .join(",")
            })
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    }

    /// Is `site` the configured site (any pass)? A non-aborting failpoint for decisions (e.g. `final_sweep_unproven`:
    /// the final sweep reports itself unproven). Always false in a release build.
    #[cfg(debug_assertions)]
    pub fn armed(&self, site: &str) -> bool {
        self.site.as_deref() == Some(site) || self.more.iter().any(|f| f.armed(site))
    }

    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub fn armed(&self, _site: &str) -> bool {
        false
    }

    #[cfg(debug_assertions)]
    pub fn parse(v: &str) -> Failpoints {
        let mut parts = v.split(',').map(str::trim).filter(|p| !p.is_empty());
        let one = |p: &str| match p.rsplit_once(':') {
            Some((site, n)) => Failpoints {
                site: Some(site.to_string()),
                n: n.parse().unwrap_or(1).max(1),
                passes: std::sync::atomic::AtomicU64::new(0),
                more: Vec::new(),
            },
            None => Failpoints::default(),
        };
        let mut first = parts.next().map(one).unwrap_or_default();
        first.more = parts.map(one).collect();
        first
    }

    /// procfix6: a NON-aborting counted site: true from the n-th pass of `site` on (`intent_enospc:<n>`: the n-th
    /// intent batch and every later one fail as if the disk were full). Always false in a release build.
    #[cfg(debug_assertions)]
    pub fn reached(&self, site: &str) -> bool {
        use std::sync::atomic::Ordering;
        let mut any = false;
        for f in &self.more {
            any |= f.reached(site);
        }
        if self.site.as_deref() != Some(site) {
            return any;
        }
        let k = self.passes.fetch_add(1, Ordering::SeqCst) + 1;
        any || k >= self.n
    }

    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub fn reached(&self, _site: &str) -> bool {
        false
    }

    /// procfix7 (gate7 B1): a NON-aborting counted site that fires at EXACTLY the n-th pass (one transient fault:
    /// `recovery_read_fails:<n>` fails the n-th read of recovery once). Always false in a release build.
    #[cfg(debug_assertions)]
    pub fn nth(&self, site: &str) -> bool {
        use std::sync::atomic::Ordering;
        let mut any = false;
        for f in &self.more {
            any |= f.nth(site);
        }
        if self.site.as_deref() != Some(site) {
            return any;
        }
        let k = self.passes.fetch_add(1, Ordering::SeqCst) + 1;
        any || k == self.n
    }

    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub fn nth(&self, _site: &str) -> bool {
        false
    }

    /// Pass `site`: abort ourselves at the configured pass (test builds); nothing in a release build.
    #[cfg(debug_assertions)]
    pub fn hit(&self, site: &str) {
        use std::sync::atomic::Ordering;
        for f in &self.more {
            f.hit(site);
        }
        if self.site.as_deref() != Some(site) {
            return;
        }
        let k = self.passes.fetch_add(1, Ordering::SeqCst) + 1;
        if k == self.n {
            // abort(): no unwinding, no Drop, no flush (as a hard kill); the supervisor has no signal path of its own.
            use std::io::Write;
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): test builds: a message right before abort()
            let _ = writeln!(std::io::stderr(), "failpoint {site}:{k}: abort");
            std::process::abort();
        }
    }

    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub fn hit(&self, _site: &str) {}

    /// The ENOSPC file operations when `ledger_enospc:<n>` is configured (test builds; only segment appends fail).
    pub fn ledger_ops(&self) -> Option<Arc<dyn FileOps>> {
        #[cfg(debug_assertions)]
        {
            if self.site.as_deref() != Some("ledger_enospc") {
                return self.more.iter().find_map(Failpoints::ledger_ops);
            }
            (self.site.as_deref() == Some("ledger_enospc")).then(|| {
                Arc::new(enospc::EnospcOps {
                    after: self.n,
                    count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                }) as Arc<dyn FileOps>
            })
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    }
}

#[cfg(debug_assertions)]
mod enospc {
    use crate::ledger::{FileOps, RealFile, SegmentFile};
    use std::io;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Real ledger file operations whose `after`-th segment append (1-based, counted over all segments) and every later
    /// one fail with ENOSPC.
    pub(super) struct EnospcOps {
        pub(super) after: u64,
        pub(super) count: Arc<AtomicU64>,
    }

    struct EnospcFile {
        inner: Box<dyn SegmentFile>,
        after: u64,
        count: Arc<AtomicU64>,
    }

    impl FileOps for EnospcOps {
        fn open_segment(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
            Ok(Box::new(EnospcFile {
                inner: RealFile::open(path)?,
                after: self.after,
                count: Arc::clone(&self.count),
            }))
        }
    }

    impl SegmentFile for EnospcFile {
        fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
            let k = self.count.fetch_add(1, Ordering::SeqCst) + 1;
            if k >= self.after {
                return Err(io::Error::from_raw_os_error(libc::ENOSPC));
            }
            self.inner.append(bytes)
        }
        fn sync(&mut self) -> io::Result<()> {
            self.inner.sync()
        }
        fn truncate(&mut self, len: u64) -> io::Result<()> {
            self.inner.truncate(len)
        }
        fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
            self.inner.write_at(offset, bytes)
        }
    }
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
#[cfg(test)]
mod tests {
    use super::*;

    /// procfix7 (gate6 m-x): a value part without `:n` (or with n = 0 / not a number / an empty part) is refused, naming
    /// the part; well-formed values (also comma-separated) pass.
    #[test]
    fn m_x_failpoint_value_without_count_is_refused() {
        for bad in [
            "no_intent_prealloc",
            "after_spawn:",
            "after_spawn:0",
            "after_spawn:x",
            ":3",
            "a:1,,b:2",
            "",
        ] {
            let e = Failpoints::check(bad).unwrap_err();
            assert!(e.contains("not <site>:<n>"), "{bad:?}: {e}");
        }
        for good in [
            "after_spawn:2",
            "after_spawn:2,no_spawn_barrier:1",
            "release_delay_ms:1500",
        ] {
            assert_eq!(Failpoints::check(good), Ok(()), "{good}");
        }
        #[cfg(debug_assertions)]
        {
            let f = Failpoints::parse("after_spawn:2,release_delay_ms:1500");
            assert_eq!(f.number("release_delay_ms"), Some(1500));
            assert_eq!(f.number("other"), None);
        }
    }
}
