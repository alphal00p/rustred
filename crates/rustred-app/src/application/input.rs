use std::collections::BTreeMap;

use rustred::input::{
    COMPACT_SCHEMA, Compiler, Limits, Project, TextGramEntry, TextProject, TextPropagator,
};
use serde::Deserialize;

use super::error::AppError;
use super::model::MetadataValue;
use super::options::InputFormat;

const PROJECT_TOML_V1_SCHEMA: &str = "rustred.project.toml.v1";
const MAX_METADATA_ENTRIES: usize = 1_024;
const MAX_METADATA_KEY_BYTES: usize = 1_024;
const MAX_METADATA_VALUE_BYTES: usize = 64 * 1024;
const MAX_METADATA_ARRAY_ITEMS: usize = 4_096;
const MAX_TOTAL_METADATA_VALUES: usize = 16_384;
const MAX_TOTAL_METADATA_BYTES: usize = 1024 * 1024;

pub(crate) struct PreparedProject {
    pub(crate) input_form: &'static str,
    pub(crate) input_schema: String,
    pub(crate) metadata: BTreeMap<String, MetadataValue>,
    pub(crate) normalized: Project,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectDocumentV1 {
    schema: String,
    integral: Option<String>,
    parameters: Option<Vec<String>>,
    #[serde(default)]
    metadata: BTreeMap<String, MetadataValue>,
    family: Option<ExplicitFamilyV1>,
    kinematics: Option<ExplicitKinematicsV1>,
    target: Option<ExplicitTargetV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplicitFamilyV1 {
    name: Option<String>,
    loop_momenta: Vec<String>,
    #[serde(default)]
    external_momenta: Vec<String>,
    dimension: String,
    denominators: Vec<ExplicitDenominatorV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplicitDenominatorV1 {
    id: String,
    expression: String,
    power_shift: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplicitKinematicsV1 {
    external_gram: Option<Vec<Vec<String>>>,
    #[serde(default)]
    gram: Vec<ExplicitGramEntryV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplicitGramEntryV1 {
    left: String,
    right: String,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplicitTargetV1 {
    powers: Option<Vec<i64>>,
    numerator: Option<String>,
}

pub(crate) fn prepare_input(
    source: &str,
    requested_format: InputFormat,
) -> Result<PreparedProject, AppError> {
    let detected = match requested_format {
        InputFormat::Auto if looks_like_symbolica(source) => InputFormat::Symbolica,
        InputFormat::Auto => InputFormat::Toml,
        explicit => explicit,
    };
    match detected {
        InputFormat::Symbolica => prepare_raw_symbolica(source),
        InputFormat::Toml => prepare_toml(source),
        InputFormat::Auto => unreachable!("auto input is resolved above"),
    }
}

fn prepare_raw_symbolica(source: &str) -> Result<PreparedProject, AppError> {
    prepare_symbolica_root(source, None, BTreeMap::new(), "raw_symbolica")
}

fn prepare_toml(source: &str) -> Result<PreparedProject, AppError> {
    let document: ProjectDocumentV1 = toml::from_str(source)
        .map_err(|error| AppError::input(format!("invalid RustRed project TOML: {error}")))?;
    prepare_project_document(document)
}

/// Reuse the existing strict Symbolica-project frontend for one compact
/// campaign root. The campaign layer owns only the root container; expression
/// parsing and parameter inference stay in the same Symbolica compiler used by
/// `derive`.
pub(crate) fn prepare_symbolica_root(
    integral: &str,
    parameters: Option<Vec<String>>,
    metadata: BTreeMap<String, MetadataValue>,
    input_form: &'static str,
) -> Result<PreparedProject, AppError> {
    validate_metadata(&metadata)?;
    let compiler = parser()?;
    let normalized = compiler
        .compile_compact(integral, parameters)
        .map_err(|error| AppError::input(format!("invalid Symbolica integral input: {error}")))?;
    Ok(PreparedProject {
        input_form,
        input_schema: COMPACT_SCHEMA.to_owned(),
        metadata,
        normalized,
    })
}

/// Prepare the existing `rustred.project.toml.v1` schema nested under one
/// campaign root. This is the single reuse seam for both hybrid and fully
/// explicit roots.
pub(crate) fn prepare_project_document(
    document: ProjectDocumentV1,
) -> Result<PreparedProject, AppError> {
    if document.schema != PROJECT_TOML_V1_SCHEMA {
        return Err(AppError::schema(format!(
            "unsupported project schema {:?}; expected {:?}",
            document.schema, PROJECT_TOML_V1_SCHEMA
        )));
    }
    validate_metadata(&document.metadata)?;
    match (document.integral, document.family) {
        (Some(integral), None) => {
            if document.kinematics.is_some() || document.target.is_some() {
                return Err(AppError::input(
                    "hybrid TOML keeps kinematics and target clauses inside integral = \"\"\"I(...)\"\"\"; only parameters and metadata may supplement it"
                        .to_owned(),
                ));
            }
            let compiler = parser()?;
            let normalized = compiler
                .compile_compact(&integral, document.parameters)
                .map_err(|error| {
                    AppError::input(format!("invalid hybrid Symbolica integral input: {error}"))
                })?;
            Ok(PreparedProject {
                input_form: "hybrid_toml",
                input_schema: document.schema,
                metadata: document.metadata,
                normalized,
            })
        }
        (None, Some(family)) => prepare_explicit_document(
            document.schema,
            document.parameters,
            document.metadata,
            family,
            document.kinematics.unwrap_or_default(),
            document.target,
        ),
        (Some(_), Some(_)) => Err(AppError::input(
            "project TOML must choose exactly one of integral (hybrid mode) and family (explicit mode)"
                .to_owned(),
        )),
        (None, None) => Err(AppError::input(
            "project TOML needs either integral = \"\"\"I(...)\"\"\" or a [family] table"
                .to_owned(),
        )),
    }
}

fn prepare_explicit_document(
    schema: String,
    parameters: Option<Vec<String>>,
    metadata: BTreeMap<String, MetadataValue>,
    family: ExplicitFamilyV1,
    kinematics: ExplicitKinematicsV1,
    target: Option<ExplicitTargetV1>,
) -> Result<PreparedProject, AppError> {
    let denominator_count = family.denominators.len();
    if denominator_count > Limits::default().max_propagators {
        return Err(AppError::limit(format!(
            "explicit family has {denominator_count} denominators, exceeding the parser limit {}",
            Limits::default().max_propagators
        )));
    }
    let (supplied_powers, numerator) = match target {
        Some(target) => (target.powers, target.numerator),
        None => (None, None),
    };
    let target_powers = match supplied_powers {
        Some(powers) if powers.len() != denominator_count => {
            return Err(AppError::input(format!(
                "target.powers has {} entries, expected one for each of {} denominators",
                powers.len(),
                denominator_count
            )));
        }
        Some(powers) => powers,
        None => {
            let mut powers = Vec::new();
            powers
                .try_reserve_exact(denominator_count)
                .map_err(|_| AppError::limit("cannot reserve default target powers".to_owned()))?;
            powers.resize(denominator_count, 1);
            powers
        }
    };
    let mut propagators = Vec::new();
    propagators
        .try_reserve_exact(family.denominators.len())
        .map_err(|_| AppError::limit("cannot reserve explicit denominator records".to_owned()))?;
    for (ordinal, denominator) in family.denominators.into_iter().enumerate() {
        propagators.push(TextPropagator {
            id: denominator.id,
            expression: denominator.expression,
            target_power: target_powers[ordinal],
            power_shift: denominator.power_shift,
        });
    }
    let external_gram = explicit_gram_entries(&family.external_momenta, kinematics)?;
    let parts = TextProject {
        name: family.name,
        parameters,
        loop_momenta: family.loop_momenta,
        external_momenta: family.external_momenta,
        dimension: family.dimension,
        propagators,
        external_gram,
        numerator,
    };
    let normalized = parser()?
        .compile_text(parts)
        .map_err(|error| AppError::input(format!("invalid explicit project family: {error}")))?;
    Ok(PreparedProject {
        input_form: "explicit_toml",
        input_schema: schema,
        metadata,
        normalized,
    })
}

fn explicit_gram_entries(
    external_momenta: &[String],
    kinematics: ExplicitKinematicsV1,
) -> Result<Vec<TextGramEntry>, AppError> {
    if kinematics.external_gram.is_some() && !kinematics.gram.is_empty() {
        return Err(AppError::input(
            "kinematics must use either external_gram (a full matrix) or gram entries, not both"
                .to_owned(),
        ));
    }
    if let Some(matrix) = kinematics.external_gram {
        if matrix.len() != external_momenta.len() {
            return Err(AppError::input(format!(
                "kinematics.external_gram has {} rows, expected {}",
                matrix.len(),
                external_momenta.len()
            )));
        }
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(matrix.len())
            .map_err(|_| AppError::limit("cannot reserve external Gram matrix rows".to_owned()))?;
        for (row, values) in matrix.into_iter().enumerate() {
            if values.len() != external_momenta.len() {
                return Err(AppError::input(format!(
                    "kinematics.external_gram row {row} has {} entries, expected {}",
                    values.len(),
                    external_momenta.len()
                )));
            }
            retained.push(values);
        }
        for left in 0..retained.len() {
            for right in left + 1..retained.len() {
                if retained[left][right].trim() != retained[right][left].trim() {
                    return Err(AppError::input(format!(
                        "kinematics.external_gram is textually asymmetric at ({left},{right}); use identical Symbolica strings or sparse upper-triangular gram entries"
                    )));
                }
            }
        }
        let successor = external_momenta
            .len()
            .checked_add(1)
            .ok_or_else(|| AppError::limit("external Gram entry count overflowed".to_owned()))?;
        let entry_count = external_momenta
            .len()
            .checked_mul(successor)
            .map(|value| value / 2)
            .ok_or_else(|| AppError::limit("external Gram entry count overflowed".to_owned()))?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(entry_count)
            .map_err(|_| AppError::limit("cannot reserve external Gram entries".to_owned()))?;
        for left in 0..external_momenta.len() {
            for right in left..external_momenta.len() {
                entries.push(TextGramEntry {
                    left: external_momenta[left].clone(),
                    right: external_momenta[right].clone(),
                    value: retained[left][right].clone(),
                });
            }
        }
        Ok(entries)
    } else {
        Ok(kinematics
            .gram
            .into_iter()
            .map(|entry| TextGramEntry {
                left: entry.left,
                right: entry.right,
                value: entry.value,
            })
            .collect())
    }
}

fn parser() -> Result<Compiler, AppError> {
    Compiler::new(Limits::default()).map_err(|error| {
        AppError::input(format!(
            "cannot initialize Symbolica input grammar: {error}"
        ))
    })
}

pub(crate) fn validate_metadata(
    metadata: &BTreeMap<String, MetadataValue>,
) -> Result<(), AppError> {
    if metadata.len() > MAX_METADATA_ENTRIES {
        return Err(AppError::limit(format!(
            "metadata has {} entries, exceeding the limit {MAX_METADATA_ENTRIES}",
            metadata.len()
        )));
    }
    let mut total = 0usize;
    let mut values_seen = 0usize;
    for (key, value) in metadata {
        if key.is_empty() {
            return Err(AppError::input("metadata keys cannot be empty".to_owned()));
        }
        if key.len() > MAX_METADATA_KEY_BYTES {
            return Err(AppError::limit(format!(
                "metadata key {key:?} exceeds {MAX_METADATA_KEY_BYTES} bytes"
            )));
        }
        let values: &[String] = match value {
            MetadataValue::String(value) => std::slice::from_ref(value),
            MetadataValue::StringArray(values) => {
                if values.len() > MAX_METADATA_ARRAY_ITEMS {
                    return Err(AppError::limit(format!(
                        "metadata array {key:?} has {} items, exceeding {MAX_METADATA_ARRAY_ITEMS}",
                        values.len()
                    )));
                }
                values
            }
        };
        values_seen = values_seen
            .checked_add(values.len())
            .ok_or_else(|| AppError::limit("metadata value count overflowed".to_owned()))?;
        if values_seen > MAX_TOTAL_METADATA_VALUES {
            return Err(AppError::limit(format!(
                "metadata has {values_seen} scalar values, exceeding {MAX_TOTAL_METADATA_VALUES}"
            )));
        }
        total = total
            .checked_add(key.len())
            .ok_or_else(|| AppError::limit("metadata byte count overflowed".to_owned()))?;
        for value in values {
            if value.len() > MAX_METADATA_VALUE_BYTES {
                return Err(AppError::limit(format!(
                    "metadata value for {key:?} exceeds {MAX_METADATA_VALUE_BYTES} bytes"
                )));
            }
            total = total
                .checked_add(value.len())
                .ok_or_else(|| AppError::limit("metadata byte count overflowed".to_owned()))?;
        }
        if total > MAX_TOTAL_METADATA_BYTES {
            return Err(AppError::limit(format!(
                "metadata retains {total} bytes, exceeding the {MAX_TOTAL_METADATA_BYTES}-byte limit"
            )));
        }
    }
    Ok(())
}

pub(crate) fn looks_like_symbolica(source: &str) -> bool {
    let source = source.trim_start();
    // Accept exactly the root spellings emitted/accepted by Symbolica for the
    // RustRed namespace. In particular, `{}` is Symbolica's explicit empty
    // namespace-parameter list; arbitrary foreign namespace prefixes remain
    // TOML (and subsequently fail that strict parser) in auto mode.
    for head in ["I", "rustred::I", "rustred::{}::I"] {
        if let Some(rest) = source.strip_prefix(head) {
            return rest.trim_start().starts_with('(');
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_detection_is_unambiguous_at_the_root() {
        assert!(looks_like_symbolica(
            "  I(loops(k), externals(), dimension(d), prop(D,k^2,1))"
        ));
        assert!(looks_like_symbolica("rustred::I (loops(k))"));
        assert!(looks_like_symbolica("rustred::{}::I (loops(k))"));
        assert!(!looks_like_symbolica(
            "schema = \"rustred.project.toml.v1\""
        ));
        assert!(!looks_like_symbolica("integral = \"I(...)\""));
        assert!(!looks_like_symbolica("foreign::I(loops(k))"));
        assert!(!looks_like_symbolica("rustred::other::I(loops(k))"));
        assert!(!looks_like_symbolica("rustred::{}::Integral(loops(k))"));
    }

    #[test]
    fn metadata_is_bounded_and_plain_text_only() {
        let mut valid = BTreeMap::new();
        valid.insert(
            "description".to_owned(),
            MetadataValue::String("one loop".to_owned()),
        );
        valid.insert(
            "tags".to_owned(),
            MetadataValue::StringArray(vec!["vacuum".to_owned(), "one-loop".to_owned()]),
        );
        assert!(validate_metadata(&valid).is_ok());

        let mut invalid = BTreeMap::new();
        invalid.insert(String::new(), MetadataValue::String("value".to_owned()));
        assert!(validate_metadata(&invalid).is_err());
    }
}
