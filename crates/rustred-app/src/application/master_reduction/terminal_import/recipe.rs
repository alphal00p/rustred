use super::*;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Descriptor {
    pub path: PathBuf,
    pub blake3: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Baseline {
    pub artifact: PathBuf,
    pub blake3: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelectedRow {
    pub source: Vec<i64>,
    pub witness: usize,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Recipe {
    pub schema: String,
    pub baseline: Baseline,
    pub numerator_symmetry: Descriptor,
    pub include_raw_symmetry: bool,
    pub selected_saved_rows: Vec<SelectedRow>,
    #[serde(default)]
    pub expected_result: Option<Descriptor>,
}
impl Recipe {
    pub fn parse(bytes: &[u8]) -> Result<Self, AppError> {
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|e| AppError::input(format!("proof recipe: {e}")))?;
        if value.schema != IMPORT_SCHEMA
            || !digest(&value.baseline.blake3)
            || !digest(&value.numerator_symmetry.blake3)
            || value
                .expected_result
                .as_ref()
                .is_some_and(|d| !digest(&d.blake3))
        {
            return Err(AppError::input(
                "invalid proof-import schema or content digest",
            ));
        }
        let mut selected = BTreeSet::new();
        for row in &value.selected_saved_rows {
            IntegralKey::try_new(row.source.clone()).map_err(io)?;
            if !selected.insert((&row.source, row.witness)) {
                return Err(AppError::input("duplicate selected saved symmetry row"));
            }
        }
        Ok(value)
    }
}
pub(super) fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
pub(super) fn resolve(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}
pub(super) fn checked(path: &Path, digest: &str) -> Result<Vec<u8>, AppError> {
    let bytes = storage::read_bounded(path, profile::native_io_limits(None).max_program_bytes)?;
    if blake3::hash(&bytes).to_hex().as_str() != digest {
        return Err(AppError::input(format!(
            "proof-import source content changed: {}",
            path.display()
        )));
    }
    Ok(bytes)
}
