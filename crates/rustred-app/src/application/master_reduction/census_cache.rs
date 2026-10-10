//! Optional raw-census acceleration state, never a master-reduction authority.
//!
//! Unlike normalized or inherited terminals, this state records exactly what
//! the saved-scope census encountered. The native inventory reader checks its
//! bindings before reuse. Losing this file only costs a fresh census.
use super::*;
use crate::{OwnerDomainWalkCensusSource, OwnerDomainWalkInventory};

const FILE: &str = "inventory-census.bin";
const FIELD: &str = "inventory_census_cache";
const SCHEMA: &str = "rustred.walk-inventory-census.v1";
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
        Ok(None) => {}
        Err(error) => {
            report["inventory_census_cache_warning"] = json!(format!(
                "optional census cache not written; later publication will run a full census: {error}"
            ));
        }
    }
    Ok(())
}

/// A portable refined artifact retains its original *raw* census, not a census
/// of its reduced master basis. Invalid/missing optional data is discarded.
pub(super) fn inherit(
    source_directory: &Path,
    directory: &Path,
    report: &mut Value,
) -> Result<(), AppError> {
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
