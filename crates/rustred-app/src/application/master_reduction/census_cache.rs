//! Ordinary optional census acceleration and required finite-replay provenance.
//!
//! Unlike normalized or inherited terminals, this state records exactly what
//! the saved-scope census encountered. The native inventory reader checks its
//! bindings before reuse. Losing an ordinary cache only costs a fresh census.
//! A completed finite recipe instead requires its faithful identity census at
//! cold source load/export/refinement. It is not accepted for prefix reuse and
//! is not an independent mathematical authority beyond the originating replay.
use super::*;
use crate::{OwnerDomainWalkCensusSource, OwnerDomainWalkInventory};

const FILE: &str = "inventory-census.bin";
const FIELD: &str = "inventory_census_cache";
const SCHEMA: &str = "rustred.walk-inventory-census.v1";
const FINITE_SCHEMA: &str = "rustred.walk-inventory-census.finite.v1";
const MAX_COPY_BYTES: usize = 256 << 20;

pub(super) fn current(directory: &Path, report: &Value) -> Option<OwnerDomainWalkCensusSource> {
    let receipt = &report[FIELD];
    if receipt["schema"] != SCHEMA {
        return None;
    }
    Some(OwnerDomainWalkCensusSource {
        path: storage::safe_child(directory, receipt["file"].as_str()?).ok()?,
        expected_blake3: receipt["blake3"].as_str()?.to_owned(),
        bytes: receipt["bytes"].as_u64()?,
    })
}

pub(super) fn previous(artifact: &Path) -> Option<OwnerDomainWalkCensusSource> {
    let report = master_reduction_inspect(artifact).ok()?;
    if !matches!(
        report["status"].as_str(),
        Some("published_unrefined" | "completed_nonminimal")
    ) {
        return None;
    }
    let directory = if artifact.is_dir() {
        artifact
    } else {
        artifact.parent()?
    };
    current(directory, &report)
}

pub(super) fn save(
    inventory: &OwnerDomainWalkInventory,
    directory: &Path,
    report: &mut Value,
) -> Result<(), AppError> {
    report.as_object_mut().unwrap().remove(FIELD);
    report
        .as_object_mut()
        .unwrap()
        .remove("inventory_census_cache_warning");
    match inventory.write_census(directory.join(FILE)) {
        Ok(Some(mut receipt)) => {
            receipt["file"] = json!(FILE);
            report[FIELD] = receipt;
        }
        Ok(None) if inventory.requires_finite_census_persistence() => {
            return Err(AppError::execution(
                "finite inventory cannot be published without its authenticated binary census",
            ));
        }
        Ok(None) => {}
        Err(error) if inventory.requires_finite_census_persistence() => return Err(error),
        Err(error) => {
            report["inventory_census_cache_warning"] = json!(format!(
                "optional census cache not written; later publication will run a full census: {error}"
            ));
        }
    }
    Ok(())
}

/// Validate required finite provenance at a cold source boundary. Ordinary
/// optional cache metadata remains non-authoritative and is not loaded here.
/// A runtime application exported from a validated source is self-contained;
/// this function is deliberately absent from hot application/cache paths.
pub(super) fn validate_required(directory: &Path, report: &Value) -> Result<(), AppError> {
    required_bytes(directory, report).map(|_| ())
}

fn required_bytes(directory: &Path, report: &Value) -> Result<Option<Vec<u8>>, AppError> {
    let receipt = &report[FIELD];
    let required = receipt["schema"] == FINITE_SCHEMA
        || receipt["required_for_finite_inventory"] == true
        || report["inventory"]["encountered"]["finite_replay"]["invocations"]
            .as_u64()
            .is_some_and(|count| count > 0);
    if !required {
        return Ok(None);
    }
    if receipt["schema"] != FINITE_SCHEMA {
        return Err(AppError::input(
            "required finite census has an unsupported or missing schema",
        ));
    }
    if receipt["required_for_finite_inventory"] != true || receipt["prefix_reuse"] != false {
        return Err(AppError::input(
            "required finite census receipt policy is malformed",
        ));
    }
    let name = receipt["file"]
        .as_str()
        .ok_or_else(|| AppError::input("required finite census file is missing"))?;
    let path = storage::safe_child(directory, name)?;
    let size = receipt["bytes"]
        .as_u64()
        .filter(|&size| size > 0)
        .ok_or_else(|| AppError::input("required finite census byte count is missing or empty"))?;
    if size > MAX_COPY_BYTES as u64 {
        return Err(AppError::limit("finite census exceeds copy byte limit"));
    }
    let digest = receipt["blake3"]
        .as_str()
        .ok_or_else(|| AppError::input("required finite census digest is missing"))?;
    let digest = blake3::Hash::from_hex(digest)
        .map_err(|_| AppError::input("required finite census digest is malformed"))?;
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        AppError::input(format!("required finite census is unavailable: {error}"))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != size {
        return Err(AppError::input(
            "required finite census file shape or size differs",
        ));
    }
    let data = storage::read_bounded(&path, MAX_COPY_BYTES)?;
    if data.len() as u64 != size || blake3::hash(&data) != digest {
        return Err(AppError::input(
            "required finite census size or digest differs",
        ));
    }
    Ok(Some(data))
}

/// A portable refined artifact retains its original *raw* census, not a census
/// of its reduced master basis. Invalid/missing optional data is discarded.
pub(super) fn inherit(
    source_directory: &Path,
    directory: &Path,
    report: &mut Value,
) -> Result<(), AppError> {
    if let Some(data) = required_bytes(source_directory, report)? {
        write_file_atomically(&directory.join(FILE), &data, true).map_err(|error| {
            AppError::execution(format!("required finite census was not copied: {error}"))
        })?;
        report[FIELD]["file"] = json!(FILE);
        report
            .as_object_mut()
            .unwrap()
            .remove("inventory_census_cache_warning");
        return Ok(());
    }
    let candidate = current(source_directory, report);
    report.as_object_mut().unwrap().remove(FIELD);
    report
        .as_object_mut()
        .unwrap()
        .remove("inventory_census_cache_warning");
    let Some(candidate) = candidate else {
        return Ok(());
    };
    if candidate.bytes > MAX_COPY_BYTES as u64 {
        return Ok(());
    }
    let Ok(data) = storage::read_bounded(&candidate.path, MAX_COPY_BYTES) else {
        return Ok(());
    };
    if data.len() as u64 != candidate.bytes
        || blake3::hash(&data).to_hex().as_str() != candidate.expected_blake3
    {
        return Ok(());
    }
    if let Err(error) = write_file_atomically(&directory.join(FILE), &data, true) {
        report["inventory_census_cache_warning"] = json!(format!(
            "optional census cache not copied; later publication will run a full census: {error}"
        ));
        return Ok(());
    }
    report[FIELD] = json!({
        "schema":SCHEMA,"file":FILE,"bytes":candidate.bytes,
        "blake3":candidate.expected_blake3,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scratch;
    use super::*;

    fn receipt(data: &[u8]) -> Value {
        json!({FIELD:{"schema":SCHEMA,"file":FILE,"bytes":data.len(),
            "blake3":blake3::hash(data).to_hex().to_string()}})
    }

    #[test]
    fn finite_census_inheritance_keeps_original_bytes_scope_and_never_enables_prefix_reuse() {
        let from = Scratch::new();
        let to = Scratch::new();
        let bytes = b"opaque original native finite census; scope identity stays inside";
        write_file_atomically(&from.0.join(FILE), bytes, true).unwrap();
        let mut report = receipt(bytes);
        report[FIELD]["schema"] = json!(FINITE_SCHEMA);
        report[FIELD]["required_for_finite_inventory"] = json!(true);
        report[FIELD]["prefix_reuse"] = json!(false);
        let before = report.clone();
        inherit(&from.0, &to.0, &mut report).unwrap();
        assert_eq!(report, before);
        assert_eq!(std::fs::read(to.0.join(FILE)).unwrap(), bytes);
        assert!(current(&to.0, &report).is_none());
        for status in ["published_unrefined", "paused", "failed"] {
            let mut source = before.clone();
            source["status"] = json!(status);
            validate_required(&from.0, &source).unwrap();
            source[FIELD]["blake3"] = json!("bad-required-digest");
            assert!(validate_required(&from.0, &source).is_err());
        }
        for mutation in 0..4 {
            let mut bad = before.clone();
            match mutation {
                0 => bad[FIELD]["blake3"] = json!("wrong"),
                1 => bad[FIELD]["bytes"] = json!(bytes.len() + 1),
                2 => bad[FIELD]["bytes"] = json!(u64::MAX),
                _ => bad[FIELD]["file"] = json!("../outside"),
            }
            assert!(inherit(&from.0, &to.0, &mut bad).is_err());
            assert_eq!(std::fs::read(from.0.join(FILE)).unwrap(), bytes);
        }
        let mut unknown = before;
        unknown[FIELD]["schema"] = json!("unknown-census-schema");
        assert!(inherit(&from.0, &to.0, &mut unknown).is_err());
        unknown[FIELD]
            .as_object_mut()
            .unwrap()
            .remove("required_for_finite_inventory");
        unknown["inventory"] = json!({"encountered":{"finite_replay":{"invocations":1}}});
        assert!(inherit(&from.0, &to.0, &mut unknown).is_err());
        let mut optional = receipt(bytes);
        optional[FIELD]["schema"] = json!("unknown-optional-schema");
        inherit(&from.0, &to.0, &mut optional).unwrap();
        assert!(optional.get(FIELD).is_none());
    }

    #[test]
    fn refined_cache_copy_is_portable_and_missing_or_corrupt_cache_is_optional() {
        let from = Scratch::new();
        let to = Scratch::new();
        let bytes = b"opaque native census; native reader validates semantics";
        write_file_atomically(&from.0.join(FILE), bytes, true).unwrap();
        let mut report = receipt(bytes);
        inherit(&from.0, &to.0, &mut report).unwrap();
        assert_eq!(report, receipt(bytes));
        assert_eq!(std::fs::read(to.0.join(FILE)).unwrap(), bytes);
        assert_eq!(current(&to.0, &report).unwrap().path, to.0.join(FILE));
        for malformed in [
            json!({FIELD:{"schema":SCHEMA,"file":"../outside","bytes":1,"blake3":"x"}}),
            json!({FIELD:{"schema":SCHEMA,"file":"absent","bytes":1,"blake3":"x"}}),
            json!({FIELD:{"schema":SCHEMA,"file":FILE,"bytes":bytes.len(),"blake3":"wrong"}}),
            json!({FIELD:{"schema":SCHEMA,"file":FILE,"bytes":u64::MAX,"blake3":"wrong"}}),
            json!({}),
        ] {
            let mut malformed = malformed;
            inherit(&from.0, &to.0, &mut malformed).unwrap();
            assert!(malformed.get(FIELD).is_none());
        }
    }

    #[test]
    fn optional_cache_copy_failure_does_not_block_refinement() {
        let from = Scratch::new();
        let to = Scratch::new();
        let bytes = b"valid optional blob";
        write_file_atomically(&from.0.join(FILE), bytes, true).unwrap();
        std::fs::create_dir(to.0.join(FILE)).unwrap();
        let mut report = receipt(bytes);
        inherit(&from.0, &to.0, &mut report).unwrap();
        assert!(report.get(FIELD).is_none());
        assert!(report["inventory_census_cache_warning"].is_string());
        assert_eq!(std::fs::read(from.0.join(FILE)).unwrap(), bytes);
    }
}
