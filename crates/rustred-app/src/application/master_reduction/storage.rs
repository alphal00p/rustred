//! Durable, portable phase-two files. Metadata never contains algebraic atoms.
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::application::atomic_file::{write_file_atomically, write_file_atomically_with};
use crate::{AppError, OwnerDomainWalkRequest};
use serde_json::{Value, json};

pub(super) fn read_json(path: &Path) -> Result<Value, AppError> {
    let data = read_bounded(path, 64 * 1024 * 1024)?;
    serde_json::from_slice(&data).map_err(|e| AppError::input(format!("{}: {e}", path.display())))
}

pub(super) fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, AppError> {
    let file = File::open(path).map_err(io)?;
    if file.metadata().map_err(io)?.len() > max as u64 {
        return Err(AppError::limit(format!(
            "{} exceeds input limit",
            path.display()
        )));
    }
    let mut data = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut data)
        .map_err(io)?;
    if data.len() > max {
        return Err(AppError::limit("input grew while reading"));
    }
    Ok(data)
}

pub(super) fn write_json(path: &Path, value: &Value) -> Result<(), AppError> {
    let data = serde_json::to_vec_pretty(value).map_err(io)?;
    write_file_atomically(path, &data, true).map_err(io)
}

pub(super) fn digest_file(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path).map_err(io)?;
    let mut hash = blake3::Hasher::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(io)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().to_hex().to_string())
}

pub(super) fn scope_binding(
    request: &OwnerDomainWalkRequest,
    checkpoint: &Path,
    depth: u32,
) -> Result<String, AppError> {
    let mut hash = blake3::Hasher::new();
    hash.update(b"rustred-master-scope-v1");
    hash.update(request.checkpoint_binding().as_bytes());
    hash.update(digest_file(&checkpoint.join("latest.json"))?.as_bytes());
    hash.update(&depth.to_le_bytes());
    for amendment in &request.amendments {
        hash.update(amendment.digest().as_bytes());
    }
    Ok(hash.finalize().to_hex().to_string())
}

pub(super) fn scope_summary(request: &OwnerDomainWalkRequest) -> Result<Value, AppError> {
    let initial: Value = serde_json::from_str(&request.matching.queries_json).map_err(io)?;
    let mut rows = initial["queries"].as_array().cloned().unwrap_or_default();
    for amendment in &request.amendments {
        let document: Value = serde_json::from_str(&amendment.text).map_err(io)?;
        if let Some(queries) = document["queries"].as_array() {
            rows.extend(queries.iter().cloned());
        }
    }
    let ranks: Option<Vec<_>> = rows
        .iter()
        .map(|row| row["max_numerator_rank"].as_u64())
        .collect();
    let degrees: Option<Vec<_>> = rows
        .iter()
        .map(|row| row["power_bounds"]["max_power_difference"].as_i64())
        .collect();
    Ok(
        json!({"queries":"inputs/queries.json","amendments":"inputs/amendments.json",
        "starting_queries":rows.len(),"max_starting_rank":ranks.and_then(|v|v.into_iter().max()),
        "max_starting_d":degrees.and_then(|v|v.into_iter().max()),
        "meaning":"bounds over the recorded starting domains, not every integral with these R/D values; generated descendants are unrestricted"}),
    )
}

/// Payloads are copied, not symlinked, so removing the old campaign cannot
/// break the final package. Copying streams bytes and preserves native codecs.
pub(super) fn package_inputs(
    request: &OwnerDomainWalkRequest,
    directory: &Path,
    expected_digests: &[String],
) -> Result<Value, AppError> {
    let inputs = directory.join("inputs");
    fs::create_dir_all(inputs.join("objects")).map_err(io)?;
    let mut selection: Value =
        serde_json::from_str(&request.matching.selection_json).map_err(io)?;
    let mut payloads = Vec::new();
    for group in ["owners", "domain_rule_overlays", "preferred_owner_programs"] {
        let Some(entries) = selection.get_mut(group).and_then(Value::as_array_mut) else {
            continue;
        };
        for entry in entries {
            let path = entry["path"]
                .as_str()
                .ok_or_else(|| AppError::input("owner path missing"))?;
            let source = request.matching.owner_base.join(path);
            let digest = digest_file(&source)?;
            if expected_digests.get(payloads.len()) != Some(&digest) {
                return Err(AppError::input(
                    "owner payload differs from the cold-verified saved scope",
                ));
            }
            let relative = format!("objects/{digest}.rrbin");
            let destination = inputs.join(&relative);
            if !destination.exists() {
                write_file_atomically_with(&destination, false, |output| {
                    let mut input = File::open(&source).map_err(|e| e.to_string())?;
                    std::io::copy(&mut input, output).map_err(|e| e.to_string())?;
                    Ok(())
                })
                .map_err(io)?;
            }
            if digest_file(&destination)? != digest {
                return Err(AppError::input("packaged owner differs from source"));
            }
            let size = fs::metadata(&destination).map_err(io)?.len();
            if entry["bytes"].as_u64() != Some(size) {
                return Err(AppError::input("owner payload size changed"));
            }
            entry["path"] = json!(relative);
            // Original source manifests are provenance, not runtime dependencies.
            if let Some(object) = entry.as_object_mut() {
                object.remove("manifest");
            }
            payloads
                .push(json!({"path":format!("inputs/{relative}"),"blake3":digest,"bytes":size}));
        }
    }
    if payloads.len() != expected_digests.len() {
        return Err(AppError::input("verified owner payload count mismatch"));
    }
    let selection_bytes = serde_json::to_vec(&selection).map_err(io)?;
    let program_binding = blake3::hash(&selection_bytes).to_hex().to_string();
    write_file_atomically(&inputs.join("selection.json"), &selection_bytes, true).map_err(io)?;
    write_file_atomically(
        &inputs.join("queries.json"),
        request.matching.queries_json.as_bytes(),
        true,
    )
    .map_err(io)?;
    write_file_atomically(
        &inputs.join("original-selection.json"),
        request.matching.selection_json.as_bytes(),
        true,
    )
    .map_err(io)?;
    let mut amendments = Vec::new();
    for amendment in &request.amendments {
        let name = format!("amendment-{}.json", amendment.digest());
        write_file_atomically(&inputs.join(&name), amendment.text.as_bytes(), true).map_err(io)?;
        amendments.push(json!({"path":name,"blake3":amendment.digest()}));
    }
    write_json(&inputs.join("amendments.json"), &json!(amendments))?;
    Ok(
        json!({"program_binding":program_binding,"selection":"inputs/selection.json", "queries":"inputs/queries.json","amendments":"inputs/amendments.json","payloads":payloads}),
    )
}

pub(super) fn safe_child(directory: &Path, name: &str) -> Result<PathBuf, AppError> {
    if name.is_empty()
        || Path::new(name)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(AppError::input("invalid relative artifact member"));
    }
    let path = directory.join(name);
    if path.exists()
        && !path
            .canonicalize()
            .map_err(io)?
            .starts_with(directory.canonicalize().map_err(io)?)
    {
        return Err(AppError::input("artifact member escapes directory"));
    }
    Ok(path)
}

pub(super) fn io(error: impl std::fmt::Display) -> AppError {
    AppError::execution(error.to_string())
}

/// OS advisory locks are released even after abrupt process death. A stale
/// marker file must never require manual deletion to resume safely.
pub(super) struct WriterLock {
    _file: File,
}
impl WriterLock {
    pub fn acquire(directory: &Path) -> Result<Self, AppError> {
        fs::create_dir_all(directory).map_err(io)?;
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(directory.join("writer.lock"))
            .map_err(io)?;
        file.try_lock().map_err(|e| {
            AppError::input(format!("master-reduction directory is already in use: {e}"))
        })?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn master_paths_reject_escape() {
        for name in ["../state", "/state", "x/../state", ""] {
            assert!(safe_child(Path::new("."), name).is_err());
        }
    }
}
