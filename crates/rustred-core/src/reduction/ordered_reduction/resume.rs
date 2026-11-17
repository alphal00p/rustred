//! vk5l-physrun level 2: resumable record pass. With `RUSTRED_RESUME_DIR`
//! (and a modular arithmetic that spills its trace), the scheduler state is
//! saved at phase boundaries (at most every `RUSTRED_RESUME_SECONDS`,
//! default 1800) into `<dir>/<request fingerprint>/state.bin`, beside the
//! spilled ops (`ops.bin`). A later request with the same fingerprint
//! restores it and continues with the next phase; the spilled ops are cut
//! back to the saved count. `RUSTRED_RESUME_STOP_AFTER_PHASE=<n>` stops a
//! request after saving at phase n (tests).
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::OrderedKernel;
use crate::algebra::Coefficient;

pub(crate) const RESUME_VERSION: &str = "rustred.resume-state.v1";

pub(crate) fn dir() -> Option<PathBuf> {
    std::env::var_os("RUSTRED_RESUME_DIR").map(PathBuf::from)
}

pub(crate) fn interval() -> f64 {
    std::env::var("RUSTRED_RESUME_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(1800.0)
}

pub(crate) fn stop_after_phase() -> Option<usize> {
    std::env::var("RUSTRED_RESUME_STOP_AFTER_PHASE").ok().and_then(|v| v.parse().ok())
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
    Ok(Coefficient { numerator, denominator })
}

/// The directory of one request: blake3 of the version, the encoded input
/// keys with their coefficients and the conditions. `None` when a key has
/// no compact encoding (the request is then not resumable).
pub(crate) fn request_dir<K: OrderedKernel>(
    root: &Path,
    initial: &BTreeMap<K::Key, Coefficient>,
    conditions: &[Coefficient],
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
            .try_div(&context.try_add(&d, &context.integer(3), limits).unwrap(), &context.try_mul(&d, &d, limits).unwrap(), limits)
            .unwrap();
        let mut w = Writer { bytes: Vec::new() };
        put_coefficient(&mut w, &value);
        put_coefficient(&mut w, &context.integer(-7));
        let mut r = Reader::new(&w.bytes);
        let template = context.zero();
        assert_eq!(get_coefficient(&mut r, &template).unwrap(), value);
        assert_eq!(get_coefficient(&mut r, &template).unwrap(), context.integer(-7));
        assert!(r.is_empty());
    }
}
