//! The background reader of the live dashboard (audit M1): ALL file reading and parsing (progress files, the
//! ledger fallback for running jobs, drill-down requests) happens on this thread; the input / render thread only
//! takes the newest published snapshot, so a slow or hung file system never delays a key (Ctrl-C included).
//!
//! The thread is detached: when the dashboard returns, it is told to stop and exits at its next check; a read that
//! hangs forever keeps only that thread (and the source it holds) alive, never the dashboard.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::app::Fetched;
use super::source::{ItemRow, JobRow, ProgressSource, RawSnapshot};

/// One published read.
#[derive(Debug)]
pub struct Snapshot {
    pub raw: RawSnapshot,
    /// Running jobs from the source's ledger fallback (only asked when the progress files list none).
    pub ledger_jobs: Option<Vec<JobRow>>,
    /// When the read finished.
    pub done: Instant,
    /// How long it took.
    pub took: Duration,
}

/// A drill-down request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Items(String),
    Detail(String),
}

/// Its answer.
#[derive(Debug)]
pub enum Reply {
    Items(String, Result<Vec<ItemRow>, String>),
    Detail(String, Result<Value, String>),
}

/// Handle of the reader thread (owned by the input / render thread).
pub struct Reader {
    latest: Arc<Mutex<Option<Arc<Snapshot>>>>,
    reading_since: Arc<Mutex<Option<Instant>>>,
    requests: Sender<Request>,
    replies: Receiver<Reply>,
    stop: Arc<AtomicBool>,
    started: Instant,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Reader {
    /// Starts the thread: a read at once, then every `refresh`; drill-down requests in between.
    pub fn spawn(source: Arc<dyn ProgressSource>, refresh: Duration) -> Reader {
        let latest: Arc<Mutex<Option<Arc<Snapshot>>>> = Arc::new(Mutex::new(None));
        let reading_since = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (req_tx, req_rx) = mpsc::channel::<Request>();
        let (rep_tx, rep_rx) = mpsc::channel::<Reply>();
        let (l2, r2, s2) = (
            Arc::clone(&latest),
            Arc::clone(&reading_since),
            Arc::clone(&stop),
        );
        let refresh = refresh.max(Duration::from_millis(50));
        let spawned = std::thread::Builder::new()
            .name("rr-dashboard-reader".into())
            .spawn(move || {
                let serve = |r: Request| {
                    let reply = match r {
                        Request::Items(n) => {
                            let v = source.node_items(&n);
                            Reply::Items(n, v)
                        }
                        Request::Detail(i) => {
                            let v = source.item_detail(&i);
                            Reply::Detail(i, v)
                        }
                    };
                    let _ = rep_tx.send(reply);
                };
                let mut next = Instant::now();
                loop {
                    if s2.load(Ordering::SeqCst) {
                        return;
                    }
                    if Instant::now() >= next {
                        let t0 = Instant::now();
                        *lock(&r2) = Some(t0);
                        let raw = source.read();
                        let ledger_jobs = if super::model::needs_job_fallback(&raw) {
                            source.running_jobs()
                        } else {
                            None
                        };
                        let snap = Snapshot {
                            raw,
                            ledger_jobs,
                            done: Instant::now(),
                            took: t0.elapsed(),
                        };
                        *lock(&l2) = Some(Arc::new(snap));
                        *lock(&r2) = None;
                        next = Instant::now() + refresh;
                    }
                    let wait = next.saturating_duration_since(Instant::now());
                    match req_rx.recv_timeout(wait) {
                        Ok(r) => {
                            if s2.load(Ordering::SeqCst) {
                                return;
                            }
                            serve(r);
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            });
        if spawned.is_err() {
            // no thread: nothing will ever be published; the screen keeps saying READING
            stop.store(true, Ordering::SeqCst);
        }
        Reader {
            latest,
            reading_since,
            requests: req_tx,
            replies: rep_rx,
            stop,
            started: Instant::now(),
        }
    }

    /// The newest snapshot (never blocks on the file system; the lock is held only to clone an `Arc`).
    pub fn latest(&self) -> Option<Arc<Snapshot>> {
        lock(&self.latest).clone()
    }

    /// Seconds since the reader started (before the first snapshot arrives).
    pub fn waited(&self) -> Duration {
        self.started.elapsed()
    }

    /// How long the current read has been running (None: idle).
    pub fn reading_for(&self) -> Option<Duration> {
        lock(&self.reading_since).map(|t| t.elapsed())
    }

    pub fn request(&self, r: Request) {
        let _ = self.requests.send(r);
    }

    /// Answers that arrived since the last call.
    pub fn replies(&self) -> Vec<Reply> {
        self.replies.try_iter().collect()
    }

    /// Tells the thread to stop (it exits at its next check; never joined).
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.stop();
    }
}

impl super::app::Fetcher for Reader {
    fn items(&self, node: &str) -> Fetched<Vec<ItemRow>> {
        self.request(Request::Items(node.to_string()));
        Fetched::Pending
    }
    fn detail(&self, item: &str) -> Fetched<Value> {
        self.request(Request::Detail(item.to_string()));
        Fetched::Pending
    }
}
