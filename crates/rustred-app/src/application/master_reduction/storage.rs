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
    containing_sector_depth: u32,
    circuit_symmetry_assistance: bool,
    finite_feedback: bool,
    normalization_profile: super::MasterNormalizationProfile,
) -> Result<String, AppError> {
    let mut hash = blake3::Hasher::new();
    hash.update(b"rustred-master-scope-v1");
    hash.update(request.checkpoint_binding().as_bytes());
    hash.update(digest_file(&checkpoint.join("latest.json"))?.as_bytes());
    hash.update(&depth.to_le_bytes());
    if containing_sector_depth != 0 {
        hash.update(b"containing-sector-seeds-v1");
        hash.update(&containing_sector_depth.to_le_bytes());
    }
    if circuit_symmetry_assistance {
        hash.update(b"circuit-symmetry-assistance-v1");
    }
    if finite_feedback {
        hash.update(super::collection::FEEDBACK_RECIPE.as_bytes());
    }
    normalization_profile.hash(&mut hash);
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
        json!({"program_binding":program_binding,"selection":"inputs/selection.json", "queries":"inputs/queries.json","amendments":"inputs/amendments.json","payloads":payloads,
            "queries_blake3":blake3::hash(request.matching.queries_json.as_bytes()).to_hex().to_string(),
            "amendments_blake3":digest_file(&inputs.join("amendments.json"))?}),
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

pub(super) fn reject_overlapping_directories(source: &Path, output: &Path) -> Result<(), AppError> {
    let mut ancestor = std::path::absolute(output).map_err(io)?;
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        suffix.push(
            ancestor
                .file_name()
                .ok_or_else(|| AppError::input("invalid output directory"))?
                .to_owned(),
        );
        if !ancestor.pop() {
            return Err(AppError::input("output directory has no existing ancestor"));
        }
    }
    let mut resolved = ancestor.canonicalize().map_err(io)?;
    for component in suffix.iter().rev() {
        resolved.push(component);
    }
    if resolved.starts_with(source) || source.starts_with(&resolved) {
        return Err(AppError::input(
            "source and output artifact directories must be distinct and non-nested",
        ));
    }
    Ok(())
}

/// Immutable content is linked where possible, copied otherwise. Either form
/// survives moving/removing the source artifact and has no path dependency.
pub(super) fn clone_inputs(source: &Path, output: &Path, report: &Value) -> Result<(), AppError> {
    let inputs = &report["inputs"];
    let payloads = inputs["payloads"]
        .as_array()
        .ok_or_else(|| AppError::input("published artifact has no portable input inventory"))?;
    let mut members = std::collections::BTreeMap::new();
    for payload in payloads {
        let name = payload["path"]
            .as_str()
            .ok_or_else(|| AppError::input("payload path missing"))?;
        let path = safe_child(source, name)?;
        if Some(fs::metadata(&path).map_err(io)?.len()) != payload["bytes"].as_u64()
            || Some(digest_file(&path)?.as_str()) != payload["blake3"].as_str()
        {
            return Err(AppError::input(
                "published owner payload differs from its inventory",
            ));
        }
        members.insert(
            name.to_owned(),
            payload["blake3"].as_str().unwrap().to_owned(),
        );
    }
    for field in ["selection", "queries", "amendments"] {
        let name = inputs[field]
            .as_str()
            .ok_or_else(|| AppError::input("portable input metadata path missing"))?;
        members.insert(name.to_owned(), digest_file(&safe_child(source, name)?)?);
    }
    let selection = safe_child(source, inputs["selection"].as_str().unwrap())?;
    if Some(digest_file(&selection)?.as_str()) != inputs["program_binding"].as_str() {
        return Err(AppError::input(
            "published selection differs from its program binding",
        ));
    }
    members.insert(
        inputs["selection"].as_str().unwrap().to_owned(),
        inputs["program_binding"].as_str().unwrap().to_owned(),
    );
    let query_digest = inputs["queries_blake3"]
        .as_str()
        .or_else(|| report["inventory"]["verification"]["queries"]["blake3"].as_str())
        .ok_or_else(|| AppError::input("published query identity missing"))?;
    members.insert(
        inputs["queries"].as_str().unwrap().to_owned(),
        query_digest.to_owned(),
    );
    let amendment_path = safe_child(source, inputs["amendments"].as_str().unwrap())?;
    let amendments = read_json(&amendment_path)?;
    if let Some(digest) = inputs["amendments_blake3"].as_str() {
        members.insert(
            inputs["amendments"].as_str().unwrap().to_owned(),
            digest.to_owned(),
        );
    } else {
        let recorded = report["inventory"]["verification"]["amendments"]["digests"]
            .as_array()
            .ok_or_else(|| AppError::input("published amendment chain identity missing"))?;
        let actual: Vec<_> = amendments
            .as_array()
            .ok_or_else(|| AppError::input("invalid amendment inventory"))?
            .iter()
            .map(|row| row["blake3"].clone())
            .collect();
        if &actual != recorded {
            return Err(AppError::input(
                "published amendment index differs from the verified scope",
            ));
        }
    }
    for amendment in amendments
        .as_array()
        .ok_or_else(|| AppError::input("invalid amendment inventory"))?
    {
        let name = amendment["path"]
            .as_str()
            .ok_or_else(|| AppError::input("amendment path missing"))?;
        let member = format!("inputs/{name}");
        let path = safe_child(source, &member)?;
        if Some(digest_file(&path)?.as_str()) != amendment["blake3"].as_str() {
            return Err(AppError::input(
                "published amendment differs from its digest",
            ));
        }
        members.insert(member, amendment["blake3"].as_str().unwrap().to_owned());
    }
    if source.join("inputs/original-selection.json").exists() {
        members.insert(
            "inputs/original-selection.json".into(),
            digest_file(&source.join("inputs/original-selection.json"))?,
        );
    }
    let mut directories = std::collections::BTreeSet::new();
    for (name, expected) in members {
        let from = safe_child(source, &name)?;
        let to = safe_child(output, &name)?;
        fs::create_dir_all(
            to.parent()
                .ok_or_else(|| AppError::input("invalid artifact member"))?,
        )
        .map_err(io)?;
        if !to.exists() && fs::hard_link(&from, &to).is_err() {
            write_file_atomically_with(&to, false, |destination| {
                let mut file = File::open(&from).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, destination).map_err(|e| e.to_string())?;
                Ok(())
            })
            .map_err(io)?;
        }
        if digest_file(&to)? != expected {
            return Err(AppError::input(
                "cloned portable input differs from its captured identity",
            ));
        }
        let mut ancestor = to.parent();
        while let Some(directory) = ancestor.filter(|path| path.starts_with(output)) {
            directories.insert(directory.to_path_buf());
            ancestor = directory.parent();
        }
    }
    // Hardlinks require directory durability too. Children are synced before
    // parents; no publication manifest is installed until this completes.
    for directory in directories.into_iter().rev() {
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(io)?;
    }
    Ok(())
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
