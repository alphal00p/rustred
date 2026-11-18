//! vk5l-physrun level 2: resumable record pass. With `RUSTRED_RESUME_DIR`
//! (and a modular arithmetic that spills its trace), the scheduler state is
//! saved at phase boundaries (at most every `RUSTRED_RESUME_SECONDS`,
//! default 1800) into `<dir>/<request fingerprint>/state.bin`, beside the
//! spilled ops (`ops.bin`). A later request with the same fingerprint
//! restores it and continues with the next phase; the spilled ops are cut
//! back to the saved count. `RUSTRED_RESUME_STOP_AFTER_PHASE=<n>` stops a
//! request after saving at phase n (tests). The sequential and the pipelined
//! scheduler (several threads) record the same trace and write the same state
//! file (pending keys in compact-form order), so a state saved at one thread
//! count resumes at any other.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::OrderedKernel;
use crate::algebra::Coefficient;

pub(crate) const RESUME_VERSION: &str = "rustred.resume-state.v1";
/// The leading bytes of a state file written since the trace digest (format
/// v2: a BLAKE3 digest of the trace prefix follows the op count). The request
/// fingerprint keeps using [`RESUME_VERSION`], so states of both formats are
/// found in the same request directory; v1 states are read with a length
/// check of the trace only (a warning says so).
/// 12d2 state format: magic, phase, recorder section (op count, trace
/// digest, ...), scheduler sections, BLAKE3 of the program identity, and last
/// the BLAKE3 of every preceding byte of the file (the state's own digest).
pub(crate) const STATE_FORMAT: &str = "rustred.resume-state.v3";

pub(crate) fn dir() -> Option<PathBuf> {
    std::env::var_os("RUSTRED_RESUME_DIR").map(PathBuf::from)
}

pub(crate) fn interval() -> f64 {
    std::env::var("RUSTRED_RESUME_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800.0)
}

static CALLER_COMMIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PENDING_COMMIT: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

pub(crate) fn set_caller_commit(on: bool) {
    CALLER_COMMIT.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn caller_commit() -> bool {
    match std::env::var("RUSTRED_RESUME_COMMIT").as_deref() {
        Ok("caller") => true,
        Ok("engine") => false,
        _ => CALLER_COMMIT.load(std::sync::atomic::Ordering::Relaxed),
    }
}

fn remove_request(dir: &Path) {
    let _ = std::fs::remove_file(dir.join("state.bin"));
    let _ = std::fs::remove_file(dir.join("ops.bin"));
    let _ = std::fs::remove_dir(dir);
}

/// The result of the request in `dir` exists: remove its state and trace, or
/// keep them for [`commit`] (caller-commit mode).
pub(crate) fn result_ready(dir: PathBuf) {
    if caller_commit() {
        eprintln!(
            "rustred resume: result ready; state and trace in {} kept until the caller commits its output",
            dir.display()
        );
        PENDING_COMMIT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(dir);
    } else {
        remove_request(&dir);
    }
}

pub(crate) fn commit() -> usize {
    let pending = std::mem::take(
        &mut *PENDING_COMMIT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    for dir in &pending {
        remove_request(dir);
    }
    pending.len()
}

pub(crate) fn stop_after_phase() -> Option<usize> {
    std::env::var("RUSTRED_RESUME_STOP_AFTER_PHASE")
        .ok()
        .and_then(|v| v.parse().ok())
}

/// Write a coefficient (numerator, denominator) with the step-cache codec.
pub(crate) fn put_coefficient(w: &mut super::step_cache::codec::Writer, c: &Coefficient) {
    w.polynomial(&c.numerator);
    w.polynomial(&c.denominator);
}

pub(crate) fn get_coefficient(
    r: &mut super::step_cache::codec::Reader<'_>,
    template: &Coefficient,
) -> Result<Coefficient, String> {
    let numerator = r.polynomial(&template.numerator)?;
    let denominator = r.polynomial(&template.denominator)?;
    Ok(Coefficient {
        numerator,
        denominator,
    })
}

/// The directory of one request: blake3 of the version, the encoded input
/// keys with their coefficients and the conditions. `None` when a key has
/// no compact encoding (the request is then not resumable).
pub(crate) fn request_dir<K: OrderedKernel>(
    root: &Path,
    initial: &BTreeMap<K::Key, Coefficient>,
    conditions: &[Coefficient],
    selection: &[u8],
) -> Option<PathBuf> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RESUME_VERSION.as_bytes());
    let mut bytes = Vec::new();
    for (key, value) in initial {
        bytes.clear();
        if !K::tail_encode(key, &mut bytes) {
            return None;
        }
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
        hasher.update(value.to_string().as_bytes());
    }
    for c in conditions {
        hasher.update(c.to_string().as_bytes());
    }
    // 12d2: a target selection (empty for a full run: the directory of a full
    // run is unchanged, so earlier full-run states stay reachable).
    if !selection.is_empty() {
        hasher.update(b"rustred.resume-selection");
        hasher.update(&(selection.len() as u64).to_le_bytes());
        hasher.update(selection);
    }
    Some(root.join(hasher.finalize().to_hex().to_string()))
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::reduction::ordered_reduction::step_cache::codec::{Reader, Writer};

    /// A coefficient of the resume state comes back equal.
    #[test]
    fn resume_coefficients_round_trip() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let d = context.parameter("d").unwrap();
        let limits = crate::algebra::ExactAlgebraLimits::default();
        let value = context
            .try_div(
                &context.try_add(&d, &context.integer(3), limits).unwrap(),
                &context.try_mul(&d, &d, limits).unwrap(),
                limits,
            )
            .unwrap();
        let mut w = Writer { bytes: Vec::new() };
        put_coefficient(&mut w, &value);
        put_coefficient(&mut w, &context.integer(-7));
        let mut r = Reader::new(&w.bytes);
        let template = context.zero();
        assert_eq!(get_coefficient(&mut r, &template).unwrap(), value);
        assert_eq!(
            get_coefficient(&mut r, &template).unwrap(),
            context.integer(-7)
        );
        assert!(r.is_empty());
    }
}
