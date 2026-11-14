//! `EVENTS.jsonl` v1 (child-job.md 3.5): line schema, the child's event sink (one `write_all` per line, fsync after
//! `job_started`, `rule_committed`, `job_finished`), and the supervisor's torn-line tolerant incremental reader.

use crate::EVENTS_VERSION;
use crate::exit::ExitCode;
use crate::outcome::TimeDecision;
use crate::spec::{Counters, Scope, WorkCounter};
use crate::stop::{CaseResult, JobStatus, StopCode, StopSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Instant, SystemTime};

/// Phases (event `phase`, heartbeat `phase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prepare,
    ZeroCensus,
    CoverLoad,
    CaseSearch,
    ExactLift,
    Guards,
    Admission,
    Numeric,
    SharedNumeric,
    DeferredCheck,
    Finishing,
}

/// The kind-specific part of an event line (tag `kind`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    JobStarted {
        job_id: String,
        attempt: u32,
        spec_blake3: String,
        pid: u32,
        program: String,
        version: String,
        git_commit: String,
        resume_from: Option<u32>,
    },
    Phase {
        phase: Phase,
    },
    CaseStarted {
        ordinal: u64,
        case: Value,
        pending: u64,
        numerical: u64,
    },
    CaseFinished {
        ordinal: u64,
        result: CaseResult,
        rule: Option<u64>,
        children: u64,
        rules_committed: u64,
    },
    RuleCommitted {
        index: u64,
        line_offset: u64,
        line_bytes: u64,
        canon_blake3: String,
        rhs: u64,
        trace_rows: u64,
        monomials: u64,
    },
    /// The legacy `NodeEvent` JSON verbatim.
    Engine {
        event: Value,
    },
    TimeDecision(TimeDecision),
    Heartbeat {
        phase: Phase,
        case_ordinal: Option<u64>,
        pending: u64,
        rules_committed: u64,
        counters: Counters,
        rss_kb: u64,
        vmhwm_kb: u64,
        helpers: u64,
        stop_requested: bool,
    },
    BudgetHit {
        counter: WorkCounter,
        scope: Scope,
        limit: u64,
        value: u64,
    },
    StopRequested {
        source: StopSource,
        count: u32,
    },
    ResumeWritten {
        rules_committed: u64,
        pending: u64,
        table_bytes: u64,
    },
    JobFinished {
        exit_code: ExitCode,
        status: JobStatus,
        stop_code: Option<StopCode>,
    },
}

impl Event {
    /// Kinds that carry `utc`.
    pub fn carries_utc(&self) -> bool {
        matches!(
            self,
            Event::JobStarted { .. } | Event::Heartbeat { .. } | Event::JobFinished { .. }
        )
    }
    /// Kinds after which the sink fsyncs.
    pub fn fsync_after(&self) -> bool {
        matches!(
            self,
            Event::JobStarted { .. } | Event::RuleCommitted { .. } | Event::JobFinished { .. }
        )
    }
}

/// One line: envelope + event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventLine {
    pub v: u32,
    pub seq: u64,
    /// Seconds since job start, 3 decimals.
    pub t: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utc: Option<String>,
    #[serde(flatten)]
    pub event: Event,
}

impl EventLine {
    /// The compact JSON line with its "\n".
    pub fn to_line(&self) -> Vec<u8> {
        let mut v = serde_json::to_vec(self).expect("event serialises");
        v.push(b'\n');
        v
    }

    pub fn parse(line: &[u8]) -> Result<EventLine, String> {
        let e: EventLine = serde_json::from_slice(line).map_err(|e| e.to_string())?;
        if e.v != EVENTS_VERSION {
            return Err(format!(
                "events version {} (expected {EVENTS_VERSION})",
                e.v
            ));
        }
        if e.utc.is_some() != e.event.carries_utc() {
            return Err(
                "utc must be present exactly on job_started, heartbeat, job_finished".into(),
            );
        }
        Ok(e)
    }
}

/// Round to 3 decimals (the envelope's `t`).
pub fn round_ms(s: f64) -> f64 {
    (s * 1000.0).round() / 1000.0
}

struct SinkInner {
    file: File,
    seq: u64,
    /// Set after the first write error: the sink stops writing (events are diagnostics, never acceptance).
    failed: Option<io::ErrorKind>,
}

/// The child's event sink: thread-safe, one `write_all` of a fully formatted line per event, `seq` assigned under
/// the lock so the file order is the `seq` order.
pub struct EventSink {
    inner: Mutex<SinkInner>,
    start: Instant,
}

impl EventSink {
    /// Create `path`, which must not exist (O_EXCL + O_NOFOLLOW: a planted file or symlink is refused, audit B F7).
    pub fn create(path: &Path) -> io::Result<EventSink> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .create_new(true)
            .append(true)
            .custom_flags(crate::seal::O_NOFOLLOW)
            .open(path)?;
        Ok(EventSink {
            inner: Mutex::new(SinkInner {
                file,
                seq: 0,
                failed: None,
            }),
            start: Instant::now(),
        })
    }

    /// Seconds since the sink was created.
    pub fn elapsed_s(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// Emit one event. Returns the `seq` used. A write error is returned once and the sink then goes quiet.
    pub fn emit(&self, event: Event) -> io::Result<u64> {
        let utc = event
            .carries_utc()
            .then(|| crate::time::utc_string(SystemTime::now()));
        let fsync = event.fsync_after();
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let seq = g.seq;
        let line = EventLine {
            v: EVENTS_VERSION,
            seq,
            t: round_ms(self.elapsed_s()),
            utc,
            event,
        }
        .to_line();
        if let Some(k) = g.failed {
            return Err(io::Error::new(k, "event sink failed earlier"));
        }
        g.seq += 1;
        let r = g
            .file
            .write_all(&line)
            .and_then(|_| if fsync { g.file.sync_data() } else { Ok(()) });
        if let Err(e) = &r {
            g.failed = Some(e.kind());
        }
        r.map(|_| seq)
    }

    /// The next `seq` (= number of events emitted); `last_seq` of OUTCOME is this minus one.
    pub fn next_seq(&self) -> u64 {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).seq
    }

    /// Try to emit within `timeout` (hard-safe stop step 4: the main thread may hold the lock). Gives up silently.
    pub fn try_emit_for(&self, event: Event, timeout: std::time::Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            match self.inner.try_lock() {
                Ok(g) => {
                    drop(g);
                    return self.emit(event).is_ok();
                }
                Err(std::sync::TryLockError::Poisoned(_)) => return self.emit(event).is_ok(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return false;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }
    }
}

/// Incremental, bounded, torn-line tolerant reader of an EVENTS file (supervisor side). Each `poll` reads what was
/// appended since the last call; a last line without "\n" is kept for the next poll; a line longer than `max_line`
/// is skipped (memory stays bounded by `max_line`) and reported as a [`TailItem::Bad`].
pub struct EventsTail {
    /// File offset of the first byte of the current (incomplete) line.
    offset: u64,
    /// Bytes of the current line already read from disk.
    pending: u64,
    partial: Vec<u8>,
    skipping: bool,
    max_line: usize,
}

/// One item of [`EventsTail::poll`].
#[derive(Debug, Clone, PartialEq)]
pub enum TailItem {
    Event(Box<EventLine>),
    /// A complete line that is not a valid event (message, line length).
    Bad(String, u64),
}

impl EventsTail {
    pub fn new(max_line: usize) -> EventsTail {
        EventsTail {
            offset: 0,
            pending: 0,
            partial: Vec::new(),
            skipping: false,
            max_line,
        }
    }

    /// Bytes consumed so far (complete lines only).
    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn poll(&mut self, path: &Path) -> io::Result<Vec<TailItem>> {
        let mut f = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        f.seek(SeekFrom::Start(self.offset + self.pending))?;
        let mut out = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            for &b in &buf[..n] {
                if b == b'\n' {
                    if self.skipping {
                        out.push(TailItem::Bad(
                            "line longer than the reader's bound".into(),
                            self.pending,
                        ));
                    } else {
                        match EventLine::parse(&self.partial) {
                            Ok(e) => out.push(TailItem::Event(Box::new(e))),
                            Err(m) => out.push(TailItem::Bad(m, self.pending)),
                        }
                    }
                    self.offset += self.pending + 1;
                    self.pending = 0;
                    self.partial.clear();
                    self.skipping = false;
                } else {
                    self.pending += 1;
                    if !self.skipping {
                        if self.partial.len() >= self.max_line {
                            self.skipping = true;
                            self.partial = Vec::new();
                        } else {
                            self.partial.push(b);
                        }
                    }
                }
            }
        }
        Ok(out)
    }
}
