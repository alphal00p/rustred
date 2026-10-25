//! RR-11 relations layer `rustred.saved-application-layer.v2`.
//!
//! A layer re-expresses some remaining terminals of an immutable saved
//! application (the *sources*) exactly over a smaller set of *masters*. The
//! layer carries externally produced exact relations, each row tagged with its
//! rule provenance (RustRed, a Kira stopgap result, an identity or a synthetic
//! test row); this module checks its structure against the base terminal maps
//! and composes the maps exactly (`raw -> remaining -> masters`). It never
//! derives a relation itself.
//!
//! Wire format (authoritative description in the RR-11 design document):
//! `layer.json` holds rows, masters and provenance. Coefficients are either
//! JSON strings in the parameter grammar (integers, parameter names,
//! `+ - * / ^` and parentheses) or tagged references `{"id": n}` into an
//! optional native Symbolica coefficient table `RR-APPLY-LAYER-01` bound by its
//! blake3 digest. A bare JSON number is refused: it would be ambiguous between
//! the rational value and a table entry. Every row carries a rule provenance
//! (row field or the layer default `provenance.kind`).
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use rustred::algebra::{Coefficient, ExactAlgebraLimits};
use rustred::family::{IntegralFamily, IntegralKey};
use rustred::persistence::{
    BinaryIoLimits, CoefficientId, CoefficientTableBuilder, DecodedCoefficientTable,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use symbolica::domains::finite_field::{ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring};
use symbolica::prelude::*;

use super::{SavedApplicationIntegral, TerminalMaps, TerminalRow, native_failure};
use crate::AppError;
use crate::application::atomic_file::write_file_atomically;

/// Schema of `layer.json` written by this crate.
pub const SAVED_APPLICATION_LAYER_SCHEMA: &str = "rustred.saved-application-layer.v2";
/// Legacy schema, still read: same rules as v2 (bare JSON numbers are refused,
/// so a v1 integer table ID can never be misread as a rational).
pub const SAVED_APPLICATION_LAYER_SCHEMA_V1: &str = "rustred.saved-application-layer.v1";
/// Magic of the optional native coefficient table referenced by `layer.json`.
pub const SAVED_APPLICATION_LAYER_TABLE_MAGIC: &[u8; 17] = b"RR-APPLY-LAYER-01";
const LAYER_FILE: &str = "layer.json";
const TABLE_FILE: &str = "layer.rrbin";
const MAX_LAYER_JSON: u64 = 1 << 31;

/// Who produced a layer row (user directive 2026-10-25 02:40: Kira stopgap
/// rules must be tagged in the rows/manifest and are reproduced by RustRed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SavedApplicationRuleProvenance {
    /// An exact relation derived by RustRed.
    Rustred,
    /// An exact relation taken from an external Kira run (results only); a
    /// stopgap to be reproduced by RustRed.
    KiraStopgap,
    /// The trivial row `source = 1 * source`; no relation is derived.
    Identity,
    /// A non-physical test row; packages carrying it must never be published.
    SyntheticTest,
}

impl SavedApplicationRuleProvenance {
    pub const ALL: [Self; 4] = [
        Self::Rustred,
        Self::KiraStopgap,
        Self::Identity,
        Self::SyntheticTest,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rustred => "rustred",
            Self::KiraStopgap => "kira-stopgap",
            Self::Identity => "identity",
            Self::SyntheticTest => "synthetic-test",
        }
    }
    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == text)
    }
}

/// Layer-level default row provenance from `provenance.kind`: one of the row
/// provenances, `mixed` (every row states its own) or absent (likewise).
fn default_provenance(
    provenance: &Value,
) -> Result<Option<SavedApplicationRuleProvenance>, AppError> {
    match provenance.get("kind") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(kind)) if kind == "mixed" => Ok(None),
        Some(Value::String(kind)) => SavedApplicationRuleProvenance::parse(kind)
            .map(Some)
            .ok_or_else(|| {
                AppError::input(format!(
                    "layer provenance.kind {kind:?} is not one of rustred, kira-stopgap, \
                     identity, synthetic-test, mixed"
                ))
            }),
        Some(other) => Err(AppError::input(format!(
            "layer provenance.kind must be a string, not {other}"
        ))),
    }
}

/// Identity of the base package a layer was computed against.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedApplicationLayerBase {
    /// blake3 of the decoded base `terminal-maps.rrbin` (its descriptor digest).
    pub terminal_maps_blake3: String,
    pub source_manifest_blake3: String,
    pub source_scope_binding: String,
    pub raw_terminals: u64,
    pub remaining_terminals: u64,
    /// Informational: blake3 of the base `application.json` bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_blake3: Option<String>,
}

/// A master of the layer: a signed index tuple in the layer family, plus
/// free-form quality/provenance fields (class, origin, lines, dots, ...).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedApplicationLayerMaster {
    pub key: Vec<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// A remaining terminal deliberately left outside the layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedApplicationLayerUnreduced {
    pub key: Vec<i64>,
    pub reason: String,
}

/// Per-class master counts and their reference (addendum A1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedApplicationLayerClassCount {
    pub class: Value,
    pub masters: u64,
    pub expected: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedApplicationLayerMinimality {
    /// e.g. `refine-5l/expected-counts.json`.
    pub reference: String,
    /// blake3 of the reference file bytes. A minimality claim needs it and
    /// compose-layer must be given the file (`--minimality-reference`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_blake3: Option<String>,
    /// Number of census classes the reference declares for the reduced
    /// (non-factorized) terminals; every one must be listed for a claim.
    pub reference_classes: u64,
    pub classes: Vec<SavedApplicationLayerClassCount>,
}

/// One fully reduced layer row: `source = sum_i coefficient_i * master_i`.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedApplicationLayerRow {
    pub source: IntegralKey,
    pub terms: Vec<(IntegralKey, Coefficient)>,
    /// Nonzero conditions; together they must cover every coefficient
    /// denominator of the row (each irreducible denominator factor divides
    /// one of their numerators or denominators).
    pub conditions: Vec<Coefficient>,
    pub provenance: SavedApplicationRuleProvenance,
    /// Verification level of the row as recorded by its producer (e.g. the
    /// rollout manifest's `ii+i`, `ii`, `rustred-validated`); a short token,
    /// counted per level in the package metadata. `None`: not stated.
    pub verification: Option<String>,
}

/// A resolved layer: every coefficient lives in the family's coefficient context.
#[derive(Clone, Debug)]
pub struct SavedApplicationLayer {
    pub family_fingerprint: String,
    pub parameters: Vec<String>,
    pub base: SavedApplicationLayerBase,
    pub rows: Vec<SavedApplicationLayerRow>,
    pub masters: Vec<SavedApplicationLayerMaster>,
    pub unreduced: Vec<SavedApplicationLayerUnreduced>,
    pub minimality: Option<SavedApplicationLayerMinimality>,
    pub provenance: Value,
}

/// A wire coefficient: a string in the parameter grammar, or `{"id": n}`.
#[derive(Clone, Debug, PartialEq)]
enum CoefficientWire {
    Table(u32),
    Text(String),
}

impl Serialize for CoefficientWire {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            Self::Text(text) => serializer.serialize_str(text),
            Self::Table(id) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("id", id)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for CoefficientWire {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        match Value::deserialize(deserializer)? {
            Value::String(text) => Ok(Self::Text(text)),
            Value::Object(map) if map.len() == 1 => match map.get("id").and_then(Value::as_u64) {
                Some(id) => u32::try_from(id)
                    .map(Self::Table)
                    .map_err(|_| D::Error::custom("coefficient table ID exceeds u32")),
                None => Err(D::Error::custom(
                    "a coefficient object must be {\"id\": <table entry>}",
                )),
            },
            Value::Number(number) => Err(D::Error::custom(format!(
                "bare JSON number coefficient {number} is ambiguous: write exact values as \
                 strings (\"{number}\") and table entries as {{\"id\": {number}}}"
            ))),
            _ => Err(D::Error::custom(
                "a coefficient is a string in the parameter grammar or {\"id\": n}",
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TermWire {
    master: Vec<i64>,
    coefficient: CoefficientWire,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RowWire {
    source: Vec<i64>,
    terms: Vec<TermWire>,
    #[serde(default)]
    conditions: Vec<CoefficientWire>,
    /// Defaults to the layer's `provenance.kind`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provenance: Option<SavedApplicationRuleProvenance>,
    /// Optional verification level token (wire v2, additive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verification: Option<String>,
}

/// A verification level is a short printable token (at most 96 characters
/// of `[A-Za-z0-9 +()._:;,/-]`, not blank).
fn checked_verification(
    source: &[i64],
    level: &Option<String>,
) -> Result<Option<String>, AppError> {
    let Some(level) = level else {
        return Ok(None);
    };
    if level.trim().is_empty()
        || level.len() > 96
        || !level
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " +()._:;,/-".contains(c))
    {
        return Err(AppError::input(format!(
            "layer row {source:?}: verification level {level:?} is not a short printable token"
        )));
    }
    Ok(Some(level.clone()))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TableDescriptor {
    path: String,
    bytes: u64,
    blake3: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerWire {
    schema: String,
    family_fingerprint: String,
    parameters: Vec<String>,
    base: SavedApplicationLayerBase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    coefficient_table: Option<TableDescriptor>,
    rows: Vec<RowWire>,
    masters: Vec<SavedApplicationLayerMaster>,
    #[serde(default)]
    unreduced: Vec<SavedApplicationLayerUnreduced>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    minimality: Option<SavedApplicationLayerMinimality>,
    #[serde(default)]
    provenance: Value,
}

#[derive(bincode::Encode, bincode::Decode)]
struct TableRecord {
    family_fingerprint: String,
    state: Vec<u8>,
    atoms: Vec<u8>,
}

/// Digests binding a read layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedApplicationLayerDigest {
    /// blake3 of the `layer.json` bytes; the table is bound through its descriptor.
    pub layer_blake3: String,
    pub table_blake3: Option<String>,
}

fn layer_json_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join(LAYER_FILE)
    } else {
        path.to_path_buf()
    }
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, AppError> {
    let metadata =
        fs::metadata(path).map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
    if metadata.len() > maximum {
        return Err(AppError::limit(format!(
            "{} exceeds the layer input allowance",
            path.display()
        )));
    }
    fs::read(path).map_err(|e| AppError::input(format!("{}: {e}", path.display())))
}

fn safe_relative(base: &Path, name: &str) -> Result<PathBuf, AppError> {
    let relative = Path::new(name);
    if name.is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(AppError::input(
            "layer coefficient table path must be a plain relative path",
        ));
    }
    Ok(base.join(relative))
}

/// Parse one coefficient in the layer text grammar into `family`'s context.
pub fn parse_layer_coefficient(
    family: &IntegralFamily,
    text: &str,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, AppError> {
    if text.trim().is_empty() {
        return Err(AppError::input("empty layer coefficient"));
    }
    if !text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || " \t+-*/^()_".contains(c))
    {
        return Err(AppError::input(format!(
            "layer coefficient {text:?} uses characters outside the parameter grammar"
        )));
    }
    let context = family.coefficient_context();
    let atom = try_parse!(text, default_namespace = "rustred")
        .map_err(|e| AppError::input(format!("layer coefficient {text:?}: {e}")))?;
    let coefficient = atom
        .as_view()
        .try_to_rational_polynomial(&Q, &Z, Some(context.one().get_variables().clone()))
        .map_err(|e| {
            AppError::input(format!(
                "layer coefficient {text:?} is not a rational function: {e}"
            ))
        })?;
    if !context.contains(&coefficient) {
        return Err(AppError::input(format!(
            "layer coefficient {text:?} uses symbols outside the family parameters {:?}",
            context.parameter_names()
        )));
    }
    context
        .validate_with_limits(&coefficient, limits)
        .map_err(native_failure)?;
    Ok(coefficient)
}

impl SavedApplicationLayer {
    /// Read `layer.json` (a file, or a directory containing it) and resolve
    /// every coefficient into `family`'s coefficient context.
    pub fn read(
        path: &Path,
        family: &IntegralFamily,
        limits: ExactAlgebraLimits,
        binary_limits: BinaryIoLimits,
    ) -> Result<(Self, SavedApplicationLayerDigest), AppError> {
        let path = layer_json_path(path);
        let bytes = read_bounded(&path, MAX_LAYER_JSON)?;
        let wire: LayerWire = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
        if wire.schema != SAVED_APPLICATION_LAYER_SCHEMA
            && wire.schema != SAVED_APPLICATION_LAYER_SCHEMA_V1
        {
            return Err(AppError::input(format!(
                "unsupported layer schema {:?}",
                wire.schema
            )));
        }
        if wire.family_fingerprint != family.fingerprint() {
            return Err(AppError::input(
                "layer family fingerprint differs from the base family",
            ));
        }
        let names = family.coefficient_context().parameter_names();
        if wire.parameters.as_slice() != names {
            return Err(AppError::input(format!(
                "layer parameters {:?} differ from the family parameters {names:?}",
                wire.parameters
            )));
        }
        let directory = path.parent().unwrap_or(Path::new("."));
        let (table, table_blake3) = match &wire.coefficient_table {
            None => (None, None),
            Some(descriptor) => {
                let table_path = safe_relative(directory, &descriptor.path)?;
                let table_bytes = read_bounded(&table_path, descriptor.bytes)?;
                let digest = blake3::hash(&table_bytes).to_hex().to_string();
                if table_bytes.len() as u64 != descriptor.bytes || digest != descriptor.blake3 {
                    return Err(AppError::input(
                        "layer coefficient table differs from its descriptor",
                    ));
                }
                (
                    Some(decode_table(&table_bytes, family, binary_limits)?),
                    Some(digest),
                )
            }
        };
        let resolve = |value: &CoefficientWire| -> Result<Coefficient, AppError> {
            match value {
                CoefficientWire::Text(text) => parse_layer_coefficient(family, text, limits),
                CoefficientWire::Table(id) => {
                    let table = table.as_ref().ok_or_else(|| {
                        AppError::input("layer coefficient ID without a coefficient table")
                    })?;
                    let coefficient = table
                        .coefficient(
                            CoefficientId::try_from_index(*id as usize).map_err(native_failure)?,
                        )
                        .map_err(native_failure)?
                        .clone();
                    family
                        .coefficient_context()
                        .validate_with_limits(&coefficient, limits)
                        .map_err(native_failure)?;
                    Ok(coefficient)
                }
            }
        };
        let key = |powers: &[i64]| -> Result<IntegralKey, AppError> {
            if powers.len() != family.denominator_count() {
                return Err(AppError::input(format!(
                    "layer key {powers:?} has arity {}, family has {}",
                    powers.len(),
                    family.denominator_count()
                )));
            }
            IntegralKey::try_new(powers.to_vec()).map_err(native_failure)
        };
        let layer_provenance = default_provenance(&wire.provenance)?;
        let rows = wire
            .rows
            .iter()
            .map(|row| {
                let provenance = row.provenance.or(layer_provenance).ok_or_else(|| {
                    AppError::input(format!(
                        "layer row {:?} has no provenance and the layer declares no \
                         provenance.kind default",
                        row.source
                    ))
                })?;
                Ok(SavedApplicationLayerRow {
                    provenance,
                    verification: checked_verification(&row.source, &row.verification)?,
                    source: key(&row.source)?,
                    terms: row
                        .terms
                        .iter()
                        .map(|term| Ok((key(&term.master)?, resolve(&term.coefficient)?)))
                        .collect::<Result<_, AppError>>()?,
                    conditions: row
                        .conditions
                        .iter()
                        .map(resolve)
                        .collect::<Result<_, AppError>>()?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        for master in &wire.masters {
            key(&master.key)?;
        }
        for unreduced in &wire.unreduced {
            key(&unreduced.key)?;
        }
        Ok((
            Self {
                family_fingerprint: wire.family_fingerprint,
                parameters: wire.parameters,
                base: wire.base,
                rows,
                masters: wire.masters,
                unreduced: wire.unreduced,
                minimality: wire.minimality,
                provenance: wire.provenance,
            },
            SavedApplicationLayerDigest {
                layer_blake3: blake3::hash(&bytes).to_hex().to_string(),
                table_blake3,
            },
        ))
    }

    /// Write `directory/layer.json` and `directory/layer.rrbin` (native
    /// coefficient table). The directory must not exist yet.
    pub fn write(
        &self,
        directory: &Path,
        binary_limits: BinaryIoLimits,
    ) -> Result<SavedApplicationLayerDigest, AppError> {
        fs::create_dir(directory)
            .map_err(|e| AppError::input(format!("{}: {e}", directory.display())))?;
        let mut table = CoefficientTableBuilder::new(binary_limits);
        let mut id = |c: &Coefficient| -> Result<CoefficientWire, AppError> {
            Ok(CoefficientWire::Table(
                table.intern(c).map_err(native_failure)?.index() as u32,
            ))
        };
        let mut rows = Vec::with_capacity(self.rows.len());
        for row in &self.rows {
            let mut terms = Vec::with_capacity(row.terms.len());
            for (master, coefficient) in &row.terms {
                terms.push(TermWire {
                    master: master.powers().to_vec(),
                    coefficient: id(coefficient)?,
                });
            }
            let conditions = row
                .conditions
                .iter()
                .map(&mut id)
                .collect::<Result<_, _>>()?;
            rows.push(RowWire {
                source: row.source.powers().to_vec(),
                terms,
                conditions,
                provenance: Some(row.provenance),
                verification: row.verification.clone(),
            });
        }
        let encoded = table.finish().map_err(native_failure)?;
        let mut table_bytes = SAVED_APPLICATION_LAYER_TABLE_MAGIC.to_vec();
        bincode::encode_into_std_write(
            TableRecord {
                family_fingerprint: self.family_fingerprint.clone(),
                state: encoded.state,
                atoms: encoded.atoms,
            },
            &mut table_bytes,
            bincode::config::standard(),
        )
        .map_err(native_failure)?;
        let table_blake3 = blake3::hash(&table_bytes).to_hex().to_string();
        write_file_atomically(&directory.join(TABLE_FILE), &table_bytes, false)
            .map_err(AppError::execution)?;
        let wire = LayerWire {
            schema: SAVED_APPLICATION_LAYER_SCHEMA.into(),
            family_fingerprint: self.family_fingerprint.clone(),
            parameters: self.parameters.clone(),
            base: self.base.clone(),
            coefficient_table: Some(TableDescriptor {
                path: TABLE_FILE.into(),
                bytes: table_bytes.len() as u64,
                blake3: table_blake3.clone(),
            }),
            rows,
            masters: self.masters.clone(),
            unreduced: self.unreduced.clone(),
            minimality: self.minimality.clone(),
            provenance: self.provenance.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&wire).map_err(native_failure)?;
        write_file_atomically(&directory.join(LAYER_FILE), &bytes, false)
            .map_err(AppError::execution)?;
        Ok(SavedApplicationLayerDigest {
            layer_blake3: blake3::hash(&bytes).to_hex().to_string(),
            table_blake3: Some(table_blake3),
        })
    }
}

fn decode_table(
    bytes: &[u8],
    family: &IntegralFamily,
    binary_limits: BinaryIoLimits,
) -> Result<DecodedCoefficientTable, AppError> {
    if !bytes.starts_with(SAVED_APPLICATION_LAYER_TABLE_MAGIC) {
        return Err(AppError::input("invalid layer coefficient table magic"));
    }
    let payload = &bytes[SAVED_APPLICATION_LAYER_TABLE_MAGIC.len()..];
    let (record, consumed): (TableRecord, usize) =
        super::super::candidate_bundle::decode_record(payload, binary_limits.max_program_bytes)?;
    if consumed != payload.len() {
        return Err(AppError::input("trailing bytes in layer coefficient table"));
    }
    if record.family_fingerprint != family.fingerprint() {
        return Err(AppError::input(
            "layer coefficient table belongs to another family",
        ));
    }
    DecodedCoefficientTable::import_generated(&record.state, &record.atoms, binary_limits)
        .map_err(native_failure)
}

/// The base-package identity fields a layer must carry, computed from a
/// saved application directory without decoding any native payload.
pub fn saved_application_layer_base(
    application: &Path,
) -> Result<SavedApplicationLayerBase, AppError> {
    let manifest = if application.is_dir() {
        application.join("application.json")
    } else {
        application.to_path_buf()
    };
    let bytes = read_bounded(&manifest, 64 << 20)?;
    let report: Value = serde_json::from_slice(&bytes).map_err(native_failure)?;
    let text = |value: &Value, name: &str| -> Result<String, AppError> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| AppError::input(format!("base application has no {name}")))
    };
    let metadata = &report["metadata"];
    Ok(SavedApplicationLayerBase {
        terminal_maps_blake3: text(&report["terminal_maps"]["blake3"], "terminal_maps.blake3")?,
        source_manifest_blake3: text(
            &metadata["source_manifest_blake3"],
            "source_manifest_blake3",
        )?,
        source_scope_binding: text(&metadata["source_scope_binding"], "source_scope_binding")?,
        raw_terminals: metadata["raw_terminals"]
            .as_u64()
            .ok_or_else(|| AppError::input("base application has no raw_terminals"))?,
        remaining_terminals: metadata["remaining_terminals"]
            .as_u64()
            .ok_or_else(|| AppError::input("base application has no remaining_terminals"))?,
        application_blake3: Some(blake3::hash(&bytes).to_hex().to_string()),
    })
}

/// Statistics of one exact composition.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SavedApplicationLayerComposition {
    pub raw_rows: usize,
    pub rows_touched: usize,
    pub source_occurrences: usize,
    pub terms_before: usize,
    pub terms_after: usize,
    pub max_terms_before: usize,
    pub max_terms_after: usize,
    pub base_remaining: usize,
    pub sources: usize,
    pub masters: usize,
    pub new_master_keys: usize,
    pub unreduced: usize,
    pub composed_remaining: usize,
    pub conditions_added_rows: usize,
}

/// A validated layer bound to its base maps.
pub(super) struct BoundLayer<'a> {
    pub(super) layer: &'a SavedApplicationLayer,
    pub(super) family: std::sync::Arc<IntegralFamily>,
    pub(super) sources: BTreeMap<SavedApplicationIntegral, usize>,
    pub(super) masters: BTreeSet<SavedApplicationIntegral>,
    /// Layer rows in family-qualified form.
    pub(super) rows: Vec<BTreeMap<SavedApplicationIntegral, Coefficient>>,
    /// Declared `unreduced` terminals.
    pub(super) unreduced: BTreeSet<SavedApplicationIntegral>,
    /// Base remaining terminals that are neither sources, masters, factorized
    /// nor declared unreduced: the layer is silent about them.
    pub(super) undeclared: BTreeSet<SavedApplicationIntegral>,
    /// Sources that are factorized base terminals (RR-11 P1).
    pub(super) factorized_sources: BTreeSet<SavedApplicationIntegral>,
}

impl BoundLayer<'_> {
    pub(super) fn qualify(&self, key: &IntegralKey) -> SavedApplicationIntegral {
        SavedApplicationIntegral {
            family_fingerprint: self.layer.family_fingerprint.clone(),
            integral: key.clone(),
        }
    }
    pub(super) fn composed_remaining(
        &self,
        base: &BTreeSet<SavedApplicationIntegral>,
    ) -> BTreeSet<SavedApplicationIntegral> {
        base.iter()
            .filter(|key| !self.sources.contains_key(*key))
            .chain(self.masters.iter())
            .cloned()
            .collect()
    }
}

/// Is every irreducible factor of the coefficient denominator (with any
/// multiplicity) a factor of some condition numerator or denominator, up to an
/// integer? Then the denominator is nonzero wherever the conditions are. A
/// condition may be a power or a product of denominator factors: the factor
/// removed at each step is a gcd with one condition polynomial.
pub(super) fn covered(denominator: &Coefficient, conditions: &[Coefficient]) -> bool {
    let mut rest = denominator.denominator.clone();
    if rest.is_constant() {
        return true;
    }
    let factors = conditions
        .iter()
        .flat_map(|c| [&c.numerator, &c.denominator])
        .filter(|p| !p.is_constant())
        .collect::<Vec<_>>();
    for factor in factors {
        loop {
            let common = rest.gcd(factor);
            if common.is_constant() {
                break;
            }
            match rest.try_div(&common) {
                Some(quotient) => rest = quotient,
                None => break,
            }
            if rest.is_constant() {
                return true;
            }
        }
    }
    rest.is_constant()
}

/// Whether factorized remaining terminals (product route) may be layer
/// sources (RR-11 proposal P1, adopted for scratch layers 2026-10-25 08:30).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum SavedApplicationFactorizedSources {
    /// Factorized terminals stay on the product route; never sources.
    #[default]
    Refused,
    /// A factorized terminal may be a source when its row states a
    /// verification level and every master of the row is itself a factorized
    /// base terminal: the row is an exact relation among product-route
    /// terminals (e.g. a multi-block product onto the pure-product masters),
    /// so the evaluation stays on the product route.
    VerifiedOntoProducts,
}

/// Check a layer against base maps (L1). `factorized` are remaining
/// terminals on the product route, which are never layer sources
/// (see [`bind_layer_with`] for RR-11 P1).
pub(super) fn bind_layer<'a>(
    base: &TerminalMaps,
    layer: &'a SavedApplicationLayer,
    factorized: &BTreeSet<SavedApplicationIntegral>,
    limits: ExactAlgebraLimits,
) -> Result<BoundLayer<'a>, AppError> {
    bind_layer_with(
        base,
        layer,
        factorized,
        SavedApplicationFactorizedSources::Refused,
        limits,
    )
}

/// [`bind_layer`] with an explicit policy for factorized sources (RR-11 P1).
pub(super) fn bind_layer_with<'a>(
    base: &TerminalMaps,
    layer: &'a SavedApplicationLayer,
    factorized: &BTreeSet<SavedApplicationIntegral>,
    factorized_sources: SavedApplicationFactorizedSources,
    limits: ExactAlgebraLimits,
) -> Result<BoundLayer<'a>, AppError> {
    let family = base
        .families
        .get(&layer.family_fingerprint)
        .cloned()
        .ok_or_else(|| AppError::input("layer family is absent from the base terminal maps"))?;
    let context = family.coefficient_context();
    if layer.parameters.as_slice() != context.parameter_names() {
        return Err(AppError::input("layer parameters differ from the family"));
    }
    if layer.rows.is_empty() {
        return Err(AppError::input("layer has no rows"));
    }
    let qualify = |key: &IntegralKey| -> Result<SavedApplicationIntegral, AppError> {
        if key.powers().len() != family.denominator_count() {
            return Err(AppError::input("layer key has the wrong arity"));
        }
        Ok(SavedApplicationIntegral {
            family_fingerprint: layer.family_fingerprint.clone(),
            integral: key.clone(),
        })
    };
    let mut masters = BTreeSet::new();
    for master in &layer.masters {
        let key = qualify(&IntegralKey::try_new(master.key.clone()).map_err(native_failure)?)?;
        if !masters.insert(key) {
            return Err(AppError::input(format!(
                "duplicate layer master {:?}",
                master.key
            )));
        }
    }
    let mut sources = BTreeMap::new();
    let mut factorized_source_keys = BTreeSet::new();
    for (index, row) in layer.rows.iter().enumerate() {
        let source = qualify(&row.source)?;
        if !base.remaining.contains(&source) {
            return Err(AppError::input(format!(
                "layer source {:?} is not a remaining terminal of the base",
                row.source.powers()
            )));
        }
        if factorized.contains(&source) {
            if factorized_sources == SavedApplicationFactorizedSources::Refused {
                return Err(AppError::input(format!(
                    "layer source {:?} is a factorized terminal (product route); \
                     factorized sources need RR-11 P1 (--allow-factorized-sources)",
                    row.source.powers()
                )));
            }
            if row.verification.is_none() {
                return Err(AppError::input(format!(
                    "factorized layer source {:?} states no verification level (RR-11 P1)",
                    row.source.powers()
                )));
            }
            for (master, _) in &row.terms {
                if !factorized.contains(&qualify(master)?) {
                    return Err(AppError::input(format!(
                        "factorized layer source {:?} uses master {:?}, which is not a \
                         factorized base terminal: RR-11 P1 rows must stay on the product route",
                        row.source.powers(),
                        master.powers()
                    )));
                }
            }
            factorized_source_keys.insert(source.clone());
        }
        if sources.insert(source, index).is_some() {
            return Err(AppError::input(format!(
                "duplicate layer source {:?}",
                row.source.powers()
            )));
        }
    }
    let mut rows = Vec::with_capacity(layer.rows.len());
    let mut used = BTreeSet::new();
    for row in &layer.rows {
        let source = qualify(&row.source)?;
        if row.terms.is_empty() {
            return Err(AppError::input(format!(
                "layer row {:?} is empty (a zero source must be stated by the base)",
                row.source.powers()
            )));
        }
        for condition in &row.conditions {
            context
                .validate_with_limits(condition, limits)
                .map_err(native_failure)?;
            if condition.is_zero() {
                return Err(AppError::input(
                    "layer row has an impossible zero condition",
                ));
            }
        }
        let mut terms = BTreeMap::new();
        for (master, coefficient) in &row.terms {
            let master = qualify(master)?;
            context
                .validate_with_limits(coefficient, limits)
                .map_err(native_failure)?;
            if coefficient.is_zero() {
                return Err(AppError::input(format!(
                    "layer row {:?} has a zero coefficient",
                    row.source.powers()
                )));
            }
            if !masters.contains(&master) {
                return Err(AppError::input(format!(
                    "layer row {:?} uses {:?}, which is not a declared master",
                    row.source.powers(),
                    master.integral.powers()
                )));
            }
            if master != source && sources.contains_key(&master) {
                return Err(AppError::input(
                    "layer row uses another source; rows must be fully reduced",
                ));
            }
            if !covered(coefficient, &row.conditions) {
                return Err(AppError::input(format!(
                    "layer row {:?} has a coefficient denominator not covered by its conditions",
                    row.source.powers()
                )));
            }
            if terms.insert(master.clone(), coefficient.clone()).is_some() {
                return Err(AppError::input("duplicate master in one layer row"));
            }
            used.insert(master);
        }
        let is_identity =
            terms.len() == 1 && terms.get(&source).is_some_and(|c| *c == context.one());
        if row.provenance == SavedApplicationRuleProvenance::Identity && !is_identity {
            return Err(AppError::input(format!(
                "layer row {:?} has provenance identity but is not source = source",
                row.source.powers()
            )));
        }
        if masters.contains(&source) && !is_identity {
            return Err(AppError::input(format!(
                "layer source {:?} is also a master but its row is not the identity",
                row.source.powers()
            )));
        }
        rows.push(terms);
    }
    if let Some(unused) = masters.difference(&used).next() {
        return Err(AppError::input(format!(
            "layer master {:?} is not used by any row",
            unused.integral.powers()
        )));
    }
    let mut unreduced = BTreeSet::new();
    for item in &layer.unreduced {
        let key = qualify(&IntegralKey::try_new(item.key.clone()).map_err(native_failure)?)?;
        if !base.remaining.contains(&key)
            || sources.contains_key(&key)
            || masters.contains(&key)
            || !unreduced.insert(key)
        {
            return Err(AppError::input(format!(
                "layer unreduced entry {:?} is not a distinct remaining terminal that is \
                 neither a source nor a master",
                item.key
            )));
        }
    }
    let undeclared = base
        .remaining
        .iter()
        .filter(|key| {
            !sources.contains_key(*key)
                && !masters.contains(*key)
                && !factorized.contains(*key)
                && !unreduced.contains(*key)
        })
        .cloned()
        .collect();
    Ok(BoundLayer {
        layer,
        family,
        sources,
        masters,
        rows,
        unreduced,
        undeclared,
        factorized_sources: factorized_source_keys,
    })
}

/// Consistency with the base: every layer master that is also a raw-row key
/// must compose to exactly itself (`M = 1 * M`); otherwise the layer and the
/// base relations disagree, or the new basis is still linearly dependent.
/// Returns the number of such masters.
pub(super) fn check_masters_compose_to_identity(
    composed: &TerminalMaps,
    bound: &BoundLayer<'_>,
) -> Result<usize, AppError> {
    let one = bound.family.coefficient_context().one();
    let mut checked = 0;
    for master in &bound.masters {
        if let Some(row) = composed.rows.get(master) {
            checked += 1;
            if row.terms.len() != 1 || row.terms.get(master) != Some(&one) {
                return Err(AppError::input(format!(
                    "layer master {:?} is a raw row of the base but composes to a non-identity \
                     row with {} terms: the layer is inconsistent with the base maps",
                    master.integral.powers(),
                    row.terms.len()
                )));
            }
        }
    }
    Ok(checked)
}

/// Exact composition `raw -> remaining -> masters` of every base row.
/// Rows untouched by the layer keep their coefficients and shared guards.
pub(super) fn compose_maps(
    base: &TerminalMaps,
    bound: &BoundLayer<'_>,
    limits: ExactAlgebraLimits,
) -> Result<(TerminalMaps, SavedApplicationLayerComposition), AppError> {
    let context = bound.family.coefficient_context();
    let rows: Vec<_> = base.rows.iter().collect();
    type Composed = (
        BTreeMap<SavedApplicationIntegral, Coefficient>,
        Vec<usize>,
        usize,
    );
    let composed = rows
        .par_iter()
        .map(|(_, row)| -> Result<Option<Composed>, AppError> {
            if !row.terms.keys().any(|t| bound.sources.contains_key(t)) {
                return Ok(None);
            }
            let mut output = BTreeMap::<SavedApplicationIntegral, Coefficient>::new();
            let mut used = Vec::new();
            let mut occurrences = 0;
            let mut add = |key: &SavedApplicationIntegral, value: Coefficient| {
                match output.remove(key) {
                    Some(previous) => {
                        let sum = context
                            .try_add(&previous, &value, limits)
                            .map_err(native_failure)?;
                        if !sum.is_zero() {
                            output.insert(key.clone(), sum);
                        }
                    }
                    None => {
                        if !value.is_zero() {
                            output.insert(key.clone(), value);
                        }
                    }
                }
                Ok::<(), AppError>(())
            };
            for (terminal, a) in &row.terms {
                match bound.sources.get(terminal) {
                    None => add(terminal, a.clone())?,
                    Some(&index) => {
                        occurrences += 1;
                        used.push(index);
                        for (master, b) in &bound.rows[index] {
                            let product = if *b == context.one() {
                                a.clone()
                            } else {
                                context.try_mul(a, b, limits).map_err(native_failure)?
                            };
                            add(master, product)?;
                        }
                    }
                }
            }
            used.sort_unstable();
            used.dedup();
            Ok(Some((output, used, occurrences)))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let mut statistics = SavedApplicationLayerComposition {
        raw_rows: rows.len(),
        base_remaining: base.remaining.len(),
        sources: bound.sources.len(),
        masters: bound.masters.len(),
        new_master_keys: bound
            .masters
            .iter()
            .filter(|m| !base.remaining.contains(*m))
            .count(),
        unreduced: bound.layer.unreduced.len(),
        ..Default::default()
    };
    // Deterministic sequential merge; equal (base guard, used rows) pairs share one Arc.
    let mut guards = HashMap::<(usize, Vec<usize>), std::sync::Arc<Vec<Coefficient>>>::new();
    let mut output = BTreeMap::new();
    for ((key, row), composed) in rows.into_iter().zip(composed) {
        statistics.terms_before += row.terms.len();
        statistics.max_terms_before = statistics.max_terms_before.max(row.terms.len());
        let new_row = match composed {
            None => row.clone(),
            Some((terms, used, occurrences)) => {
                statistics.rows_touched += 1;
                statistics.source_occurrences += occurrences;
                let with_conditions: Vec<usize> = used
                    .into_iter()
                    .filter(|&i| !bound.layer.rows[i].conditions.is_empty())
                    .collect();
                let conditions = if with_conditions.is_empty() {
                    row.conditions.clone()
                } else {
                    statistics.conditions_added_rows += 1;
                    let pointer = std::sync::Arc::as_ptr(&row.conditions) as usize;
                    guards
                        .entry((pointer, with_conditions.clone()))
                        .or_insert_with(|| {
                            let mut merged = row.conditions.as_ref().clone();
                            for index in &with_conditions {
                                for condition in &bound.layer.rows[*index].conditions {
                                    if !merged.contains(condition) {
                                        merged.push(condition.clone());
                                    }
                                }
                            }
                            std::sync::Arc::new(merged)
                        })
                        .clone()
                };
                TerminalRow { terms, conditions }
            }
        };
        statistics.terms_after += new_row.terms.len();
        statistics.max_terms_after = statistics.max_terms_after.max(new_row.terms.len());
        output.insert(key.clone(), new_row);
    }
    let remaining = bound.composed_remaining(&base.remaining);
    statistics.composed_remaining = remaining.len();
    Ok((
        TerminalMaps {
            families: base.families.clone(),
            rows: output,
            remaining,
            layer: None,
        },
        statistics,
    ))
}

/// Deterministic splitmix64 stream for reproducible check points.
pub(super) struct SplitMix(pub(super) u64);
impl SplitMix {
    pub(super) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// 2^61 - 1.
pub(super) const CHECK_PRIME: u64 = 2_305_843_009_213_693_951;

type Fp = symbolica::domains::finite_field::FiniteFieldElement<u64>;

fn evaluate(coefficient: &Coefficient, field: &Zp64, point: &[Fp]) -> Option<Fp> {
    let numerator = coefficient.numerator.evaluate_with_coeff_map(
        |value| value.to_finite_field(field),
        point,
        field,
    );
    let denominator = coefficient.denominator.evaluate_with_coeff_map(
        |value| value.to_finite_field(field),
        point,
        field,
    );
    if field.is_zero(&denominator) {
        None
    } else {
        Some(field.div(&numerator, &denominator))
    }
}

fn accumulate(
    output: &mut BTreeMap<SavedApplicationIntegral, u64>,
    field: &Zp64,
    key: &SavedApplicationIntegral,
    value: &Fp,
) {
    let entry = output.entry(key.clone()).or_insert(0);
    let sum = field.add(&field.to_element(*entry), value);
    *entry = field.from_element(&sum);
    if *entry == 0 {
        output.remove(key);
    }
}

/// Mod-p check (design §3 step 9): for each sampled raw row, evaluating the
/// base row followed by the layer substitution equals evaluating the
/// composed row, at `points` random parameter values modulo `CHECK_PRIME`.
/// Points where a denominator vanishes are re-drawn. Returns the checked
/// (row, point) pairs.
pub(super) fn check_composition_mod_p(
    base: &TerminalMaps,
    bound: &BoundLayer<'_>,
    composed: &TerminalMaps,
    sample: &[SavedApplicationIntegral],
    points: usize,
    seed: u64,
) -> Result<(usize, Vec<Vec<u64>>), AppError> {
    let field = Zp64::new(CHECK_PRIME);
    let parameters = bound.family.coefficient_context().parameter_names().len();
    let mut random = SplitMix(seed);
    let mut chosen = Vec::new();
    let mut attempts = 0;
    let mut checked = 0;
    while chosen.len() < points {
        attempts += 1;
        if attempts > 16 * points.max(1) {
            return Err(AppError::execution(
                "layer self-check could not draw regular evaluation points",
            ));
        }
        let point_values: Vec<u64> = (0..parameters)
            .map(|_| random.next() % CHECK_PRIME)
            .collect();
        let point: Vec<Fp> = point_values.iter().map(|v| field.to_element(*v)).collect();
        let pair_results = sample
            .par_iter()
            .map(|key| -> Result<Option<bool>, AppError> {
                let old = &base.rows[key];
                let new = composed
                    .rows
                    .get(key)
                    .ok_or_else(|| AppError::execution("composed maps lost a raw row"))?;
                let mut expected = BTreeMap::new();
                for (terminal, a) in &old.terms {
                    let Some(a) = evaluate(a, &field, &point) else {
                        return Ok(None);
                    };
                    match bound.sources.get(terminal) {
                        None => accumulate(&mut expected, &field, terminal, &a),
                        Some(&index) => {
                            for (master, b) in &bound.rows[index] {
                                let Some(b) = evaluate(b, &field, &point) else {
                                    return Ok(None);
                                };
                                accumulate(&mut expected, &field, master, &field.mul(&a, &b));
                            }
                        }
                    }
                }
                let mut actual = BTreeMap::new();
                for (master, c) in &new.terms {
                    let Some(c) = evaluate(c, &field, &point) else {
                        return Ok(None);
                    };
                    accumulate(&mut actual, &field, master, &c);
                }
                Ok(Some(expected == actual))
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        if pair_results.iter().any(Option::is_none) {
            continue;
        }
        if let Some(position) = pair_results.iter().position(|ok| *ok != Some(true)) {
            return Err(AppError::execution(format!(
                "layer self-check failed for raw row {:?} at point {point_values:?} mod {CHECK_PRIME}",
                sample[position].integral.powers()
            )));
        }
        checked += sample.len();
        chosen.push(point_values);
    }
    Ok((checked, chosen))
}

/// Values of the base rows, the parsed layer rows and the composed rows at
/// one regular point modulo `CHECK_PRIME`, plus the raw rows the layer
/// touches (C3 review fix 2026-10-25). An external checker re-parses the
/// layer's TEXT rows itself, compares them with `layer_rows`, redoes the
/// substitution `base -> layer` on `base_rows` and compares with
/// `composed_rows`: an arithmetic check that does not reuse the composition
/// code or the parsed rows. Rows are emitted for every raw row.
pub(super) fn composition_check_values(
    base: &TerminalMaps,
    bound: &BoundLayer<'_>,
    composed: &TerminalMaps,
    seed: u64,
) -> Result<Value, AppError> {
    let field = Zp64::new(CHECK_PRIME);
    let parameters = bound
        .family
        .coefficient_context()
        .parameter_names()
        .to_vec();
    let mut random = SplitMix(seed);
    let powers = |k: &SavedApplicationIntegral| json!(k.integral.powers());
    for _ in 0..64 {
        let point_values: Vec<u64> = (0..parameters.len())
            .map(|_| random.next() % CHECK_PRIME)
            .collect();
        let point: Vec<Fp> = point_values.iter().map(|v| field.to_element(*v)).collect();
        let terms_json =
            |terms: &BTreeMap<SavedApplicationIntegral, Coefficient>| -> Option<Value> {
                let mut out = Vec::with_capacity(terms.len());
                for (key, coefficient) in terms {
                    let value = evaluate(coefficient, &field, &point)?;
                    out.push(json!([powers(key), field.from_element(&value).to_string()]));
                }
                Some(Value::Array(out))
            };
        let rows_json =
            |rows: &BTreeMap<SavedApplicationIntegral, TerminalRow>| -> Option<Vec<Value>> {
                rows.iter()
                    .map(|(key, row)| {
                        Some(json!({"key": powers(key), "terms": terms_json(&row.terms)?}))
                    })
                    .collect()
            };
        let Some(base_rows) = rows_json(&base.rows) else {
            continue;
        };
        let Some(composed_rows) = rows_json(&composed.rows) else {
            continue;
        };
        let mut layer_rows = Vec::with_capacity(bound.sources.len());
        let mut regular = true;
        for (source, index) in &bound.sources {
            match terms_json(&bound.rows[*index]) {
                Some(terms) => layer_rows.push(json!({"source": powers(source), "terms": terms})),
                None => {
                    regular = false;
                    break;
                }
            }
        }
        if !regular {
            continue;
        }
        let touched: Vec<Value> = base
            .rows
            .iter()
            .filter_map(|(key, row)| {
                let source_terms = row
                    .terms
                    .keys()
                    .filter(|t| bound.sources.contains_key(*t))
                    .count();
                (source_terms > 0).then(|| {
                    json!({"key": powers(key), "terms": row.terms.len(),
                           "source_terms": source_terms,
                           "is_source": bound.sources.contains_key(key)})
                })
            })
            .collect();
        return Ok(json!({
            "schema": "rustred.saved-application-layer-check-values.v1",
            "prime": CHECK_PRIME.to_string(),
            "parameters": parameters,
            "point": point_values.iter().map(u64::to_string).collect::<Vec<_>>(),
            "family_fingerprint": bound.layer.family_fingerprint,
            "base_remaining": base.remaining.iter().map(powers).collect::<Vec<_>>(),
            "composed_remaining": composed.remaining.iter().map(powers).collect::<Vec<_>>(),
            "touched_rows": touched,
            "layer_rows": layer_rows,
            "base_rows": base_rows,
            "composed_rows": composed_rows,
        }));
    }
    Err(AppError::execution(
        "check values: no regular evaluation point in 64 draws",
    ))
}

/// Schema of the per-class reference counts (`refine-5l/expected-counts.json`).
pub const MINIMALITY_REFERENCE_SCHEMA: &str = "refine-5l.expected-counts.v1";

/// The parsed minimality reference: expected master counts per census class
/// (`class_rep`), taken from the reference file itself, never from the layer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedApplicationMinimalityReference {
    /// Non-factorized classes (`tide_factorized == false`) and their expected
    /// counts (`symmetry_reduced_estimate`, an integer).
    pub expected: BTreeMap<u64, u64>,
    /// Factorized classes (product route; outside a layer claim).
    pub factorized: BTreeSet<u64>,
    /// Classes whose estimate is not a nonnegative integer.
    pub non_integral: BTreeSet<u64>,
}

impl SavedApplicationMinimalityReference {
    /// Parse a `refine-5l.expected-counts.v1` file.
    pub fn parse(bytes: &[u8]) -> Result<Self, AppError> {
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|e| AppError::input(format!("minimality reference: {e}")))?;
        if value["schema"].as_str() != Some(MINIMALITY_REFERENCE_SCHEMA) {
            return Err(AppError::input(format!(
                "minimality reference schema {} is not {MINIMALITY_REFERENCE_SCHEMA}",
                value["schema"]
            )));
        }
        let classes = value["classes"]
            .as_array()
            .ok_or_else(|| AppError::input("minimality reference has no classes array"))?;
        let mut reference = Self::default();
        for class in classes {
            let rep = class["class_rep"].as_u64().ok_or_else(|| {
                AppError::input(format!(
                    "minimality reference class without class_rep: {class}"
                ))
            })?;
            let factorized = class["tide_factorized"].as_bool().ok_or_else(|| {
                AppError::input(format!(
                    "minimality reference class {rep}: no tide_factorized"
                ))
            })?;
            let estimate = class["symmetry_reduced_estimate"].as_f64().ok_or_else(|| {
                AppError::input(format!(
                    "minimality reference class {rep}: no symmetry_reduced_estimate"
                ))
            })?;
            let duplicate = reference.expected.contains_key(&rep)
                || reference.factorized.contains(&rep)
                || reference.non_integral.contains(&rep);
            if duplicate {
                return Err(AppError::input(format!(
                    "minimality reference lists class {rep} twice"
                )));
            }
            if factorized {
                reference.factorized.insert(rep);
            } else if estimate >= 0.0 && estimate.fract() == 0.0 && estimate < 1e9 {
                reference.expected.insert(rep, estimate as u64);
            } else {
                reference.non_integral.insert(rep);
            }
        }
        Ok(reference)
    }
}

/// What the composition knows about the scope of a minimality claim.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedApplicationMinimalityScope {
    /// Non-factorized base terminals left unreduced: declared `unreduced`
    /// entries plus any filled in by compose-layer (`--fill-unreduced`).
    pub unreduced: usize,
    /// Factorized base terminals (product route, outside the layer).
    pub factorized: usize,
    /// The reference file was given and its blake3 equals `reference_blake3`.
    pub reference_verified: bool,
    /// The parsed reference file (expected counts come only from here).
    pub reference: Option<SavedApplicationMinimalityReference>,
    /// The reference was independently reviewed (operator flag
    /// `--minimality-reference-reviewed`); without it the claim is withheld.
    pub reference_reviewed: bool,
}

/// Integer census class of a layer master (`class`: a number or a decimal string).
fn master_class(master: &SavedApplicationLayerMaster) -> Option<u64> {
    match master.class.as_ref()? {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

/// The `minimality_claim` rule (addendum A1, RR-11 §3 step 7; C3 review fix
/// 2026-10-25), evaluated against the base and the reference file: true only
/// if every non-factorized remaining terminal of the base is a source or a
/// master (nothing unreduced, declared or not), the reference file's content
/// hash equals the layer's `reference_blake3`, every layer master states its
/// census class, the per-class master counts COMPUTED from `masters` equal the
/// counts the REFERENCE file gives for every non-factorized class (and no
/// master sits in a class the reference does not list), the layer's declared
/// per-class counts (if any) agree with both, and the reference was
/// independently reviewed. Masters in factorized classes are reported and
/// excluded (product route).
pub fn layer_minimality_claim(
    layer: &SavedApplicationLayer,
    scope: &SavedApplicationMinimalityScope,
) -> (bool, String) {
    let Some(minimality) = &layer.minimality else {
        return (false, "no minimality block in the layer".into());
    };
    if scope.unreduced > 0 {
        return (
            false,
            format!(
                "{} non-factorized base terminals left unreduced",
                scope.unreduced
            ),
        );
    }
    if !scope.reference_verified {
        return (
            false,
            format!(
                "minimality reference {} not verified against reference_blake3",
                minimality.reference
            ),
        );
    }
    let Some(reference) = &scope.reference else {
        return (false, "minimality reference file not parsed".into());
    };
    if !reference.non_integral.is_empty() {
        return (
            false,
            format!(
                "reference classes {:?} have non-integral estimates",
                reference.non_integral
            ),
        );
    }
    let mut computed: BTreeMap<u64, u64> = BTreeMap::new();
    let mut factorized_masters = 0usize;
    for master in &layer.masters {
        let Some(class) = master_class(master) else {
            return (
                false,
                format!(
                    "layer master {:?} states no integer census class",
                    master.key
                ),
            );
        };
        if reference.factorized.contains(&class) {
            factorized_masters += 1;
        } else if reference.expected.contains_key(&class) {
            *computed.entry(class).or_default() += 1;
        } else {
            return (
                false,
                format!(
                    "layer master {:?} is in class {class}, which the reference does not list",
                    master.key
                ),
            );
        }
    }
    for (class, expected) in &reference.expected {
        let masters = computed.get(class).copied().unwrap_or_default();
        if masters != *expected {
            return (
                false,
                format!("class {class}: {masters} layer masters, reference expects {expected}"),
            );
        }
    }
    if minimality.reference_classes != reference.expected.len() as u64 {
        return (
            false,
            format!(
                "layer declares {} reference classes, the reference has {} non-factorized classes",
                minimality.reference_classes,
                reference.expected.len()
            ),
        );
    }
    let mut seen = BTreeSet::new();
    for class in &minimality.classes {
        let id = match &class.class {
            Value::Number(n) => n.as_u64(),
            Value::String(t) => t.parse().ok(),
            _ => None,
        };
        let Some(id) = id else {
            return (
                false,
                format!("declared class {} is not an integer", class.class),
            );
        };
        if !seen.insert(id) {
            return (false, format!("class {id} declared twice"));
        }
        let computed_count = computed.get(&id).copied().unwrap_or_default();
        if class.masters != computed_count || Some(&class.expected) != reference.expected.get(&id) {
            return (
                false,
                format!(
                    "declared class {id} ({} masters, expected {}) disagrees with the computed \
                     {computed_count} / reference {:?}",
                    class.masters,
                    class.expected,
                    reference.expected.get(&id)
                ),
            );
        }
    }
    let agreed = format!(
        "every non-factorized base terminal is a layer source or master; per-class master \
         counts computed from the layer equal {} (blake3 {}) for all {} non-factorized classes; \
         {factorized_masters} layer masters in factorized classes and {} factorized base \
         terminals stay on the product route and are not covered",
        minimality.reference,
        minimality.reference_blake3.as_deref().unwrap_or_default(),
        reference.expected.len(),
        scope.factorized
    );
    if !scope.reference_reviewed {
        return (
            false,
            format!(
                "claim withheld: the reference is not marked independently reviewed \
                 (--minimality-reference-reviewed); otherwise: {agreed}"
            ),
        );
    }
    (true, agreed)
}

impl SavedApplicationLayer {
    /// Number of rows per provenance (every provenance listed, zeros included).
    pub fn provenance_counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts: BTreeMap<_, _> = SavedApplicationRuleProvenance::ALL
            .into_iter()
            .map(|p| (p.as_str(), 0))
            .collect();
        for row in &self.rows {
            *counts.get_mut(row.provenance.as_str()).unwrap() += 1;
        }
        counts
    }

    /// Number of rows per verification level (`unstated` for rows without one).
    pub fn verification_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for row in &self.rows {
            *counts
                .entry(
                    row.verification
                        .clone()
                        .unwrap_or_else(|| "unstated".into()),
                )
                .or_insert(0) += 1;
        }
        counts
    }

    /// Source keys of the rows that are not RustRed relations or identities
    /// (kira-stopgap, synthetic-test): the rows still to be reproduced.
    pub fn flagged_sources(&self) -> BTreeMap<&'static str, Vec<Vec<i64>>> {
        let mut flagged = BTreeMap::new();
        for row in &self.rows {
            if matches!(
                row.provenance,
                SavedApplicationRuleProvenance::KiraStopgap
                    | SavedApplicationRuleProvenance::SyntheticTest
            ) {
                flagged
                    .entry(row.provenance.as_str())
                    .or_insert_with(Vec::new)
                    .push(row.source.powers().to_vec());
            }
        }
        flagged
    }

    /// The package authority statement of the refinement layer, built from
    /// the row provenance counts (never assumed).
    pub fn authority(&self, layer_id: &str) -> String {
        let counts = self.provenance_counts();
        let mut parts = Vec::new();
        for provenance in SavedApplicationRuleProvenance::ALL {
            let count = counts[provenance.as_str()];
            if count == 0 {
                continue;
            }
            parts.push(match provenance {
                SavedApplicationRuleProvenance::Rustred => {
                    format!("{count} RustRed relation rows")
                }
                SavedApplicationRuleProvenance::KiraStopgap => format!(
                    "{count} kira-stopgap rows (exact results of an external Kira process, \
                     user-approved stopgap, to be reproduced by RustRed)"
                ),
                SavedApplicationRuleProvenance::Identity => {
                    format!("{count} identity rows (no relation derived)")
                }
                SavedApplicationRuleProvenance::SyntheticTest => format!(
                    "{count} SYNTHETIC TEST rows (non-physical; this package must never be \
                     published)"
                ),
            });
        }
        format!(
            "refinement layer {layer_id}: {}; composition checked mod p at compose time",
            parts.join(", ")
        )
    }
}

/// Summary of a layer for metadata (no coefficients).
pub(super) fn layer_summary(
    layer: &SavedApplicationLayer,
    digest: &SavedApplicationLayerDigest,
) -> Value {
    json!({
        "schema": SAVED_APPLICATION_LAYER_SCHEMA,
        "layer_blake3": digest.layer_blake3,
        "coefficient_table_blake3": digest.table_blake3,
        "base": layer.base,
        "sources": layer.rows.len(),
        "masters": layer.masters.len(),
        "unreduced": layer.unreduced,
        "minimality": layer.minimality,
        "provenance": layer.provenance,
        "provenance_counts": layer.provenance_counts(),
        "verification_counts": layer.verification_counts(),
        "flagged_sources": layer.flagged_sources(),
    })
}
