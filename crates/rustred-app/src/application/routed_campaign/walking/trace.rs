//! Research-only admission and successor-stream trace (W0.4 intel lane).
//!
//! Compiled only with the non-default cargo feature `admission-trace`, and
//! active only when `RUSTRED_ADMISSION_TRACE_DIR` names a directory. It never
//! changes walk behaviour: it observes the coordinator's admissions after the
//! fact and copies the event stream an inspector hands to its sink.
//!
//! Files (little-endian, decoded by `tools/research/idxreplay`):
//! - `coord-<pid>.bin`: one fixed-size record per `Queue::admit*` call in
//!   commit order: outcome kind, the job whose event is being committed, the
//!   resulting ID, the forward/maintenance/retirement counter deltas and the
//!   compact image of the requested domain.
//! - `jobs-<pid>-<k>.bin`: one record per inspection run by inspector thread
//!   k, written whole when the inspection returns: the parent ID and image,
//!   status flags, native seconds, then every event after the job-local exact
//!   cache, in emission order (Admit with its image, KnownReuse,
//!   PreAdmittedOrthantReuse with its target, Frontier; Count and Optional
//!   events are only totalled).
use std::cell::RefCell;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use super::inspection::{Effect, Event, Finished, NativeStats};
use super::queue::{CompactDomain, Domain};

pub(super) const COORD_MAGIC: &[u8; 8] = b"RRTRCRD1";
pub(super) const JOBS_MAGIC: &[u8; 8] = b"RRTRJOB1";
const JOB_MAGIC: u32 = 0x4A4F_4231;

/// Outcome kinds of a coordinator admission record.
pub(super) const EXACT: u8 = 0;
pub(super) const ORTHANT: u8 = 1;
pub(super) const CONTAINED: u8 = 2;
pub(super) const NEW: u8 = 3;
pub(super) const REFUSED: u8 = 4;

static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static SOURCE: AtomicU32 = AtomicU32::new(u32::MAX);
static COORD: Mutex<Option<BufWriter<File>>> = Mutex::new(None);
static THREADS: AtomicUsize = AtomicUsize::new(0);

fn dir() -> Option<&'static Path> {
    DIR.get_or_init(|| std::env::var_os("RUSTRED_ADMISSION_TRACE_DIR").map(PathBuf::from))
        .as_deref()
}

fn create(name: String, magic: &[u8; 8], arity: usize) -> BufWriter<File> {
    let dir = dir().expect("trace directory");
    let path = dir.join(name);
    let file = File::create(&path)
        .unwrap_or_else(|e| panic!("admission trace {}: {e}", path.display()));
    let mut out = BufWriter::with_capacity(1 << 20, file);
    out.write_all(magic).expect("trace header");
    out.write_all(&(arity as u32).to_le_bytes())
        .expect("trace header");
    out
}

/// The job whose events the coordinator commits next (`u32::MAX`: initial).
pub(super) fn set_source(id: usize) {
    if dir().is_some() {
        SOURCE.store(u32::try_from(id).unwrap_or(u32::MAX - 1), Ordering::Relaxed);
    }
}

fn narrow(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// One `Queue::admit*` outcome with its counter deltas.
pub(super) fn admission<const N: usize>(
    kind: u8,
    target: usize,
    image: &CompactDomain<N>,
    forward: usize,
    maintenance: usize,
    retired: usize,
) {
    if dir().is_none() {
        return;
    }
    let mut guard = COORD.lock().unwrap_or_else(|e| e.into_inner());
    let out = guard.get_or_insert_with(|| {
        create(format!("coord-{}.bin", std::process::id()), COORD_MAGIC, N)
    });
    let mut record = Vec::with_capacity(32 + 4 * N + 34);
    record.push(kind);
    record.extend_from_slice(&[0, 0, 0]);
    record.extend_from_slice(&SOURCE.load(Ordering::Relaxed).to_le_bytes());
    record.extend_from_slice(&narrow(target).to_le_bytes());
    record.extend_from_slice(&narrow(forward).to_le_bytes());
    record.extend_from_slice(&(maintenance as u64).to_le_bytes());
    record.extend_from_slice(&narrow(retired).to_le_bytes());
    image.trace_image(&mut record);
    out.write_all(&record).expect("admission trace write");
}

/// Flush the coordinator stream (end of a walk or a save point).
pub(super) fn flush() {
    if let Ok(mut guard) = COORD.lock()
        && let Some(out) = guard.as_mut()
    {
        let _ = out.flush();
    }
}

thread_local! {
    static JOBS: RefCell<Option<BufWriter<File>>> = const { RefCell::new(None) };
}

fn flags(successor: bool, conditional: bool) -> u8 {
    u8::from(successor) | u8::from(conditional) << 1
}

fn event_record<const N: usize>(buffer: &mut Vec<u8>, event: &Event<N>, total: &mut u64) -> bool {
    *total = total.saturating_add(event.count as u64);
    let count = narrow(event.count).to_le_bytes();
    match &event.effect {
        Effect::Admit {
            domain,
            successor,
            conditional,
        } => {
            match CompactDomain::try_from_domain(domain) {
                Ok(image) => {
                    buffer.push(1);
                    buffer.push(flags(*successor, *conditional));
                    buffer.extend_from_slice(&count);
                    image.trace_image(buffer);
                }
                Err(_) => {
                    buffer.push(9);
                    buffer.push(flags(*successor, *conditional));
                    buffer.extend_from_slice(&count);
                }
            }
            true
        }
        Effect::KnownReuse {
            successor,
            conditional,
        } => {
            buffer.push(2);
            buffer.push(flags(*successor, *conditional));
            buffer.extend_from_slice(&count);
            true
        }
        Effect::PreAdmittedOrthantReuse {
            target,
            successor,
            conditional,
        } => {
            buffer.push(3);
            buffer.push(flags(*successor, *conditional));
            buffer.extend_from_slice(&count);
            buffer.extend_from_slice(&narrow(*target).to_le_bytes());
            true
        }
        Effect::Frontier {
            successor,
            conditional,
            ..
        } => {
            buffer.push(4);
            buffer.push(flags(*successor, *conditional));
            buffer.extend_from_slice(&count);
            true
        }
        Effect::Count | Effect::Optional(_) => false,
    }
}

/// Run one inspection, copying its sink-bound events into the job trace.
pub(super) fn inspection<const N: usize>(
    id: usize,
    domain: &Domain<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
    inspect: impl FnOnce(&mut dyn FnMut(Event<N>) -> ControlFlow<()>) -> Finished,
) -> Finished {
    if dir().is_none() {
        return inspect(&mut |event| emit(event));
    }
    let mut buffer = Vec::new();
    let mut records = 0_u32;
    let mut total = 0_u64;
    let mut stopped = false;
    let finished = inspect(&mut |event| {
        if event_record(&mut buffer, &event, &mut total) {
            records = records.saturating_add(1);
        }
        let flow = emit(event);
        stopped |= flow.is_break();
        flow
    });
    let partial = finished.initial_overlap_scope().is_some();
    let route = matches!(finished.stats, NativeStats::Route(_));
    let status = u8::from(finished.error.is_some())
        | u8::from(partial) << 1
        | u8::from(stopped) << 2
        | u8::from(route) << 3;
    let mut header = Vec::with_capacity(40 + 4 * N + 34);
    header.extend_from_slice(&JOB_MAGIC.to_le_bytes());
    header.extend_from_slice(&narrow(id).to_le_bytes());
    header.extend_from_slice(&narrow(buffer.len()).to_le_bytes());
    header.extend_from_slice(&records.to_le_bytes());
    header.extend_from_slice(&total.to_le_bytes());
    header.extend_from_slice(&finished.seconds.to_le_bytes());
    header.push(status);
    header.extend_from_slice(&[0, 0, 0]);
    match CompactDomain::try_from_domain(domain) {
        Ok(image) => image.trace_image(&mut header),
        Err(_) => header.resize(header.len() + CompactDomain::<N>::TRACE_IMAGE_BYTES, 0),
    }
    JOBS.with(|jobs| {
        let mut jobs = jobs.borrow_mut();
        let out = jobs.get_or_insert_with(|| {
            let k = THREADS.fetch_add(1, Ordering::Relaxed);
            create(
                format!("jobs-{}-{k:03}.bin", std::process::id()),
                JOBS_MAGIC,
                N,
            )
        });
        out.write_all(&header).expect("job trace write");
        out.write_all(&buffer).expect("job trace write");
        out.flush().expect("job trace flush");
    });
    finished
}
