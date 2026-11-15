//! What the dashboard may ask of its host: the stop counter (supervisor) or "quit the viewer" (watcher).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::signals::{StopHandle, StopLevel};

/// The dashboard's view of its host.
pub trait Controls {
    /// Ctrl-C pressed (a key in raw mode): supervisor -> one stop request (drain, then force); watcher -> quit.
    fn request_stop(&self) -> StopLevel;
    /// Stop requests so far (keys AND signals: SIGINT / SIGTERM / SIGHUP bump the same counter).
    fn stop_count(&self) -> usize;
    /// Grace period of a drain, seconds (the run's `--grace`).
    fn grace_seconds(&self) -> u64 {
        120
    }
    /// True when this process runs the campaign (Ctrl-C stops the RUN); false for a read-only viewer.
    fn hosts_campaign(&self) -> bool;
    /// The supervisor has stopped: the dashboard returns.
    fn campaign_done(&self) -> bool {
        false
    }
    /// SIGWINCH seen since the last call.
    fn take_winch(&self) -> bool {
        false
    }
    /// SIGHUP seen (terminal gone).
    fn hangup_seen(&self) -> bool {
        false
    }
}

impl Controls for StopHandle {
    fn request_stop(&self) -> StopLevel {
        StopHandle::request_stop(self)
    }
    fn stop_count(&self) -> usize {
        self.count()
    }
    fn hosts_campaign(&self) -> bool {
        true
    }
    fn take_winch(&self) -> bool {
        StopHandle::take_winch(self)
    }
    fn hangup_seen(&self) -> bool {
        StopHandle::hangup_seen(self)
    }
}

/// Supervisor-hosted dashboard: the shared stop handle, the grace and a done flag the supervisor sets.
#[derive(Clone, Debug)]
pub struct SupervisorControls {
    pub handle: StopHandle,
    pub grace_s: u64,
    pub done: Arc<AtomicBool>,
}

impl SupervisorControls {
    pub fn new(handle: StopHandle, grace_s: u64) -> Self {
        SupervisorControls {
            handle,
            grace_s,
            done: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Controls for SupervisorControls {
    fn request_stop(&self) -> StopLevel {
        self.handle.request_stop()
    }
    fn stop_count(&self) -> usize {
        self.handle.count()
    }
    fn grace_seconds(&self) -> u64 {
        self.grace_s
    }
    fn hosts_campaign(&self) -> bool {
        true
    }
    fn campaign_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }
    fn take_winch(&self) -> bool {
        self.handle.take_winch()
    }
    fn hangup_seen(&self) -> bool {
        self.handle.hangup_seen()
    }
}

/// Read-only viewer (`watch` / `dashboard CDIR`): Ctrl-C and `q` close the viewer, the campaign is never touched.
/// `handle` may be registered for signals by the caller (`StopSignals::install_into`) so that SIGINT / SIGWINCH reach
/// the viewer too.
#[derive(Clone, Debug, Default)]
pub struct WatchControls {
    pub handle: StopHandle,
    quits: Arc<AtomicUsize>,
}

impl WatchControls {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Controls for WatchControls {
    fn request_stop(&self) -> StopLevel {
        StopLevel::from_count(self.quits.fetch_add(1, Ordering::SeqCst) + 1)
    }
    fn stop_count(&self) -> usize {
        self.quits.load(Ordering::SeqCst) + self.handle.count()
    }
    fn hosts_campaign(&self) -> bool {
        false
    }
    fn take_winch(&self) -> bool {
        self.handle.take_winch()
    }
    fn hangup_seen(&self) -> bool {
        self.handle.hangup_seen()
    }
}
