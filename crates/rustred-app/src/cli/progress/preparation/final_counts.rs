//! Optional read-only recovery for viewers of older, still-running producers.
//!
//! Completed parents stop publishing heartbeats. An older producer may drop
//! that final payload as stale. Resolve only recorded progress paths (never
//! guess from a topology name), then bind the final data to this invocation.
//! Missing/malformed observations remain unknown and cannot affect a solver.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::super::family_close::telemetry::{MAX_SNAPSHOT_BYTES, NativeSnapshot, finite_seconds};
use super::model::{MAX_STATUS_BYTES, Status};
use serde::Deserialize;

#[derive(Deserialize)]
struct Request {
    started_unix: f64,
    jobs: Vec<RequestJob>,
}
#[derive(Deserialize)]
struct RequestJob {
    id: String,
    command: Vec<String>,
    native_progress: Option<PathBuf>,
}
#[derive(Deserialize)]
struct Batch {
    schema: String,
    jobs: Vec<BatchJob>,
}
#[derive(Deserialize)]
struct BatchJob {
    id: String,
    arguments: Vec<String>,
}

fn read(path: &Path, allowance: u64) -> Option<(Vec<u8>, f64)> {
    if std::fs::symlink_metadata(path)
        .ok()?
        .file_type()
        .is_symlink()
    {
        return None;
    }
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > allowance {
        return None;
    }
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs_f64();
    let mut bytes = Vec::new();
    file.take(allowance.checked_add(1)?)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= allowance).then_some((bytes, modified))
}

fn argument(arguments: &[String], flag: &str) -> Option<PathBuf> {
    let mut found = None;
    for (offset, value) in arguments.iter().enumerate() {
        if value == flag {
            if found.is_some() {
                return None;
            }
            let path = PathBuf::from(arguments.get(offset + 1)?);
            if !path.is_absolute() {
                return None;
            }
            found = Some(path);
        }
    }
    found
}

fn recorded(snapshot: &Path, status: &Status) -> Option<(f64, BTreeMap<String, PathBuf>)> {
    let directory = snapshot.parent()?;
    // Bind the sidecar to the displayed attempt, not a neighboring campaign.
    if directory.file_name()?.to_str()? != status.attempt_id
        || directory.parent()?.canonicalize().ok()?
            != Path::new(&status.run_id).canonicalize().ok()?
    {
        return None;
    }
    let (bytes, _) = read(&directory.join("request.json"), MAX_STATUS_BYTES)?;
    let request: Request = serde_json::from_slice(&bytes).ok()?;
    if !finite_seconds(request.started_unix) || request.started_unix > status.timestamp_unix_seconds
    {
        return None;
    }
    let mut paths = BTreeMap::new();
    for job in request.jobs {
        if job
            .command
            .get(1)
            .is_some_and(|command| command == "family-candidates-batch")
        {
            let config = argument(&job.command, "--config")?;
            let (bytes, _) = read(&config, MAX_STATUS_BYTES)?;
            let batch: Batch = serde_json::from_slice(&bytes).ok()?;
            if batch.schema != "rustred.family-candidates-batch.v1" {
                return None;
            }
            for parent in batch.jobs {
                let path = argument(&parent.arguments, "--progress-json")?;
                if paths.insert(parent.id, path).is_some() {
                    return None;
                }
            }
        } else if let Some(path) = job
            .native_progress
            .or_else(|| argument(&job.command, "--progress-json"))
        {
            if !path.is_absolute() || paths.insert(job.id, path).is_some() {
                return None;
            }
        }
    }
    Some((request.started_unix, paths))
}

pub(super) fn recover(snapshot: &Path, status: &mut Status) {
    if !status
        .jobs
        .iter()
        .any(|job| job.state == "completed" && job.native().is_none())
    {
        return;
    }
    let recorded = recorded(snapshot, status);
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(status.timestamp_unix_seconds, |value| value.as_secs_f64());
    for job in &mut status.jobs {
        if job.state != "completed"
            || job.native().is_some()
            || job.pid.is_none()
            || job.pid_start.is_none()
        {
            continue;
        }
        let bound = job
            .started_unix
            .zip(job.native_progress_path.as_ref())
            .or_else(|| {
                recorded
                    .as_ref()
                    .and_then(|(start, paths)| paths.get(&job.id).map(|path| (*start, path)))
            });
        let Some((started, path)) = bound else {
            continue;
        };
        if !path.is_absolute()
            || !finite_seconds(started)
            || started > status.timestamp_unix_seconds
        {
            continue;
        }
        let Some((bytes, modified)) = read(path, MAX_SNAPSHOT_BYTES as u64) else {
            continue;
        };
        if modified < started {
            continue;
        }
        let Ok(native) = serde_json::from_slice::<NativeSnapshot>(&bytes) else {
            continue;
        };
        if native.validate().is_ok() {
            job.retain_final(native, modified, now);
        }
    }
}

#[cfg(test)]
mod tests;
