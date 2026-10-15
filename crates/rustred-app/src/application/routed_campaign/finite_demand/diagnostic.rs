//! Generic opt-in driver. All scope, rank, arity and resource data are inputs.
use super::trace::{Policy, Request, run_with_progress};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("diagnostic steering input exceeds 1 MiB".into());
    }
    Ok(bytes)
}
fn write(path: &Path, value: &Value) -> Result<()> {
    let mut output = BufWriter::new(OpenOptions::new().create_new(true).write(true).open(path)?);
    serde_json::to_writer(&mut output, value)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}
fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[test]
#[ignore = "manual input-controlled finite-demand diagnostic; external resource supervisor required"]
fn input_controlled_complete_finite_demand() -> Result<()> {
    if std::env::var("RUSTRED_RUN_FINITE_DEMAND").as_deref() != Ok("1") {
        return Err(
            "set RUSTRED_RUN_FINITE_DEMAND=1 under the external resource supervisor".into(),
        );
    }
    let path = |name| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .ok_or_else(|| format!("explicit {name} required"))
    };
    let command = path("RUSTRED_FINITE_DEMAND_COMMAND")?;
    let policy = path("RUSTRED_FINITE_DEMAND_POLICY")?;
    let output = path("RUSTRED_FINITE_DEMAND_OUTPUT")?;
    std::fs::create_dir(&output)?;
    let started = Instant::now();
    let cancel = Arc::new(AtomicBool::new(false));
    let term = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&cancel))?;
    let interrupt = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&cancel))?;
    let mut receipt = json!({"schema":"rustred.finite-demand-driver.v1","status":"incomplete",
        "authority":"diagnostic only; not a checkpoint or publication artifact",
        "driver_source_blake3":digest(include_bytes!("diagnostic.rs")),
        "runner_source_blake3":digest(include_bytes!("trace.rs")),
        "rayon_threads":rayon::current_num_threads()});
    let outcome = (|| -> Result<()> {
        let command_bytes = read(&command)?;
        let policy_bytes = read(&policy)?;
        let args: Vec<String> = serde_json::from_slice(&command_bytes)?;
        let policy: Policy = serde_json::from_slice(&policy_bytes)?;
        // Parsing reads only the supplied source files, not checkpoint state.
        let source =
            crate::cli::walk_request_from_argv(args.into_iter().map(OsString::from).collect())?;
        receipt["command_blake3"] = json!(digest(&command_bytes));
        receipt["policy_blake3"] = json!(digest(&policy_bytes));
        let result = run_with_progress(
            Request {
                source: &source,
                policy: &policy,
            },
            &cancel,
            |event| {
                eprintln!("{event}");
            },
        )?;
        write(&output.join("result.json"), &result.document)?;
        if !result.completed || cancel.load(Ordering::Acquire) {
            return Err("finite demand incomplete".into());
        }
        Ok(())
    })();
    let outcome = outcome.and_then(|()| {
        if cancel.load(Ordering::Acquire) {
            Err("finite demand cancelled".into())
        } else {
            Ok(())
        }
    });
    receipt["status"] = json!(if outcome.is_ok() {
        "passed"
    } else {
        "incomplete"
    });
    receipt["cancelled"] = json!(cancel.load(Ordering::Acquire));
    receipt["error"] = json!(outcome.as_ref().err().map(ToString::to_string));
    receipt["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    write(&output.join("receipt.json"), &receipt)?;
    signal_hook::low_level::unregister(term);
    signal_hook::low_level::unregister(interrupt);
    outcome
}
