//! Read-only native candidate detail export; diagnostic JSON, not an artifact.
//! Usage: inspect_candidate_program BUNDLE [OPTIONS_JSON]
use rustred_app::{
    CandidateBundleLimits, CandidateProgramInspectionOptions, inspect_generated_candidate_program,
};
use std::error::Error;
use std::io::{BufWriter, Read, Write};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("expected BUNDLE [OPTIONS_JSON]")?;
    let options = if let Some(path) = args.next() {
        let input = std::fs::read(path)?;
        if input.len() > rustred_app::MAX_INPUT_BYTES {
            return Err("options exceed input budget".into());
        }
        serde_json::from_slice(&input)?
    } else {
        CandidateProgramInspectionOptions::default()
    };
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let limits = CandidateBundleLimits::default();
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limits.max_bundle_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    let report = inspect_generated_candidate_program(&bytes, limits, options)?;
    let mut output = BufWriter::new(std::io::stdout().lock());
    report.write_json(&mut output)?;
    output.flush()?;
    Ok(())
}
