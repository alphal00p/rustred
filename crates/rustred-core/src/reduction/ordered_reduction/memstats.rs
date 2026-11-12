//! rr-frontier diagnostics (switch, off by default): where the bytes of a
//! pending frontier key go.
//!
//! `RUSTRED_ORDERED_FRONTIER_MEMSTATS=<file>` makes the sequential scheduler
//! take snapshots of its resident state (frontier, terminals, the weights'
//! own tables such as the modular trace) and write the largest one as JSON
//! when the run ends. `RUSTRED_ORDERED_FRONTIER_MEMSTATS_AT=<n>` takes the
//! snapshot when the frontier first holds `n` keys (the peak of an earlier
//! run); without it a snapshot is taken at every 25 % growth of the peak.
//!
//! A snapshot measures by construction: it clones a container and reads the
//! allocator's in-use bytes (glibc `mallinfo2().uordblks`, chunk headers
//! included) before and after, so the delta is the exact heap footprint of
//! the container with everything it owns. Element heaps are measured with
//! `malloc_usable_size` (+ the 8-byte glibc chunk header). Nothing here
//! changes a decision of the run: snapshots only read and clone.

use std::sync::OnceLock;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod sys {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    pub struct Mallinfo2 {
        pub arena: usize,
        pub ordblks: usize,
        pub smblks: usize,
        pub hblks: usize,
        pub hblkhd: usize,
        pub usmblks: usize,
        pub fsmblks: usize,
        pub uordblks: usize,
        pub fordblks: usize,
        pub keepcost: usize,
    }
    unsafe extern "C" {
        pub fn malloc_usable_size(ptr: *mut std::ffi::c_void) -> usize;
        pub fn mallinfo2() -> Mallinfo2;
    }
}

/// Allocator bytes of one live heap block (usable size + glibc chunk
/// header); 0 for a dangling (empty) allocation.
pub(crate) fn block<T>(ptr: *const T, capacity: usize) -> usize {
    if capacity == 0 || std::mem::size_of::<T>() == 0 {
        return 0;
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // SAFETY: `ptr` is the start of a live global-allocator block of a
        // nonzero-capacity Vec/Box/Arc (the caller's contract); the global
        // allocator is the system malloc (or a malloc override that also
        // provides malloc_usable_size).
        unsafe { sys::malloc_usable_size(ptr as *mut std::ffi::c_void) + 8 }
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        capacity * std::mem::size_of::<T>()
    }
}

/// Allocator in-use bytes (glibc: small-heap chunks + mmapped blocks).
pub(crate) fn in_use() -> usize {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // SAFETY: plain query of the glibc allocator.
        let info = unsafe { sys::mallinfo2() };
        info.uordblks + info.hblkhd
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        0
    }
}

/// Allocator arena bytes (glibc: main + thread arenas + mmapped blocks).
pub(crate) fn arena() -> (usize, usize) {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // SAFETY: plain query of the glibc allocator.
        let info = unsafe { sys::mallinfo2() };
        (info.arena + info.hblkhd, info.fordblks)
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        (0, 0)
    }
}

pub(crate) fn rss_bytes() -> usize {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<usize>().ok())
        .map_or(0, |pages| pages * 4096)
}

/// The heap delta of building `value` (exact on glibc; 0 elsewhere).
pub(crate) fn measure_clone<T>(build: impl FnOnce() -> T) -> usize {
    let before = in_use();
    let value = build();
    let after = in_use();
    drop(value);
    after.saturating_sub(before)
}

pub(crate) struct Settings {
    pub path: std::path::PathBuf,
    pub at: Option<usize>,
}

pub(crate) fn settings() -> Option<&'static Settings> {
    static SETTINGS: OnceLock<Option<Settings>> = OnceLock::new();
    SETTINGS
        .get_or_init(|| {
            let path = std::env::var_os("RUSTRED_ORDERED_FRONTIER_MEMSTATS")?;
            let at = std::env::var("RUSTRED_ORDERED_FRONTIER_MEMSTATS_AT")
                .ok()
                .and_then(|v| v.parse().ok());
            Some(Settings {
                path: path.into(),
                at,
            })
        })
        .as_ref()
}

/// Snapshot schedule of one run.
pub(crate) struct Probe {
    pub settings: &'static Settings,
    pub next: usize,
    pub best: Option<(usize, serde_json::Value)>,
}

impl Probe {
    pub fn new() -> Option<Self> {
        let settings = settings()?;
        Some(Self {
            settings,
            next: settings.at.unwrap_or(1024).max(1),
            best: None,
        })
    }

    pub fn due(&self, len: usize) -> bool {
        len >= self.next
    }

    pub fn taken(&mut self, len: usize, snapshot: serde_json::Value) {
        if self.best.as_ref().is_none_or(|(best, _)| len >= *best) {
            self.best = Some((len, snapshot));
        }
        self.next = if self.settings.at.is_some() {
            usize::MAX
        } else {
            len + len / 4 + 1
        };
    }

    pub fn write(&self, extra: serde_json::Value) {
        let value = serde_json::json!({
            "schema": "rr-frontier.memstats.v1",
            "snapshot": self.best.as_ref().map(|(_, snapshot)| snapshot.clone()),
            "end": extra,
        });
        let _ = std::fs::write(
            &self.settings.path,
            serde_json::to_string_pretty(&value).unwrap_or_default(),
        );
    }
}
