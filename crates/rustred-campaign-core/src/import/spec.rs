//! `TableSetSpec` (schema `rustred.campaign.import-spec.v1`): the ordered list of existing material to import, as DATA
//! (interfaces/import.md section 2). Nothing here knows a node, an arity, a path or a label scheme: they are fields.
//!
//! Parsing is strict: every object is checked against the field list of its kind before it is deserialised, so an
//! unknown or misplaced field is a refusal ([`ImportError::Spec`]), never ignored.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::{ImportError, io_err};

pub const SPEC_SCHEMA: &str = "rustred.campaign.import-spec.v1";

/// A file named by the spec, with an optional expected sha256 (hex). A given digest is VERIFIED.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    pub path: String,
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Required evidence level of a served key (R40 U-9..U-12 words).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// Imported with provenance only (A0-imported).
    #[default]
    Imported,
    /// Identity replay at primes distinct from the reconstruction primes.
    Replay,
    /// Exact per-key proof (instance-span / instance-exact / exact recipe solve).
    PerKey,
}

impl Level {
    pub fn word(self) -> &'static str {
        match self {
            Level::Imported => "imported",
            Level::Replay => "replay",
            Level::PerKey => "per-key",
        }
    }
}

/// Elided witness loci policy (interfaces/import.md section 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElidedPolicy {
    /// Strict: an elided record blocks every key where it is selected.
    #[default]
    Block,
    /// The record serves at its position; a served key is CERTIFIED only with an exact per-key proof for exactly
    /// (record_sha256, key), otherwise it is reported blocked.
    LiftWithPerKeyProof,
}

/// Witness-absent policy (closure `WitnessAbsent`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WitnessAbsentPolicy {
    #[default]
    LiftWithRegularFact,
    Block,
}

/// Closure tier choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TierChoice {
    /// C iff any fact file is given, else K.
    #[default]
    #[serde(rename = "auto")]
    Auto,
    K,
    C,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policies {
    #[serde(default)]
    pub elided: ElidedPolicy,
    #[serde(default)]
    pub witness_absent: WitnessAbsentPolicy,
    #[serde(default)]
    pub tier: TierChoice,
}

/// Column map of a hand statement (JSONL of arrays), interfaces/import.md 7.2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandFormat {
    pub key: usize,
    pub in_box: usize,
    pub label: usize,
    pub rule: usize,
    #[serde(default)]
    pub round: Option<usize>,
    /// When the round column is an array: the element holding the round.
    #[serde(default)]
    pub round_index: Option<usize>,
    /// The label that marks a terminal row (default "terminal").
    #[serde(default = "default_terminal_label")]
    pub terminal_label: String,
}

fn default_terminal_label() -> String {
    "terminal".into()
}

/// Format of the start-set file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartFormat {
    /// One key per line: `[..]`, `{"key":[..]}` or an `I(..)` text.
    KeysJsonl,
    /// closure.md section 4 key-set file.
    KeysetV1,
    /// A hand statement: the keys flagged in-box (needs `hand_format`).
    HandStatement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartSpec {
    pub path: String,
    pub format: StartFormat,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub hand_format: Option<HandFormat>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSpec {
    #[serde(default)]
    pub facts: Vec<FileRef>,
    #[serde(default)]
    pub proofs: Vec<FileRef>,
    #[serde(default)]
    pub solved: Vec<FileRef>,
    #[serde(default)]
    pub expected_recipe_binary: Option<String>,
}

/// Domain mode of an imported record (interfaces/import.md section 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainMode {
    #[default]
    Engine,
    /// The recenter tightens the SELECTION domain (bank ruling 06:16Z).
    QualifiedRecenter,
}

/// A record removed from the dispatch, with the reason recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Removed {
    pub label: i64,
    pub reason: String,
}

/// Hand-name template (interfaces/import.md 7.3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandName {
    pub template: String,
    #[serde(default)]
    pub strip: Vec<String>,
}

/// Kind of a source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    EngineTable,
    TableSet,
    Recipe,
    SuppliedRelations,
    SymmetryCredit,
    Terminals,
}

impl SourceKind {
    fn parse(s: &str) -> Option<SourceKind> {
        Some(match s {
            "engine_table" => SourceKind::EngineTable,
            "table_set" => SourceKind::TableSet,
            "recipe" => SourceKind::Recipe,
            "supplied_relations" => SourceKind::SuppliedRelations,
            "symmetry_credit" => SourceKind::SymmetryCredit,
            "terminals" => SourceKind::Terminals,
            _ => return None,
        })
    }
    /// Fields allowed besides the common ones (`id`, `kind`, `class`, `hand_name`).
    fn fields(self) -> &'static [&'static str] {
        match self {
            SourceKind::EngineTable => &["path", "sha256", "records", "removed", "domain"],
            SourceKind::TableSet => &[
                "priority_file",
                "variant",
                "tables_dir",
                "sha256sums",
                "include_origins",
                "exclude_origins",
                "label_scope",
                "removed",
                "domain",
            ],
            SourceKind::Recipe => &["path", "sha256", "engine_digest"],
            SourceKind::SuppliedRelations => &["path", "sha256", "verification"],
            SourceKind::SymmetryCredit => &["group"],
            SourceKind::Terminals => &["keys", "path", "sha256", "preferred"],
        }
    }
}

/// One source of the spec (fields per kind; the loader checks which are required).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpec {
    pub id: String,
    pub kind: SourceKind,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub hand_name: Option<HandName>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub records: Option<Vec<i64>>,
    #[serde(default)]
    pub removed: Vec<Removed>,
    #[serde(default)]
    pub domain: DomainMode,
    #[serde(default)]
    pub priority_file: Option<String>,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub tables_dir: Option<String>,
    #[serde(default)]
    pub sha256sums: Vec<FileRef>,
    #[serde(default)]
    pub include_origins: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_origins: Option<Vec<String>>,
    /// `source` (default) or a scope name shared by several sources (labels unique over the whole scope).
    #[serde(default)]
    pub label_scope: Option<String>,
    #[serde(default)]
    pub engine_digest: Option<String>,
    #[serde(default)]
    pub verification: Option<FileRef>,
    #[serde(default)]
    pub group: Option<FileRef>,
    #[serde(default)]
    pub keys: Option<Vec<Vec<i32>>>,
    #[serde(default)]
    pub preferred: Option<bool>,
}

impl SourceSpec {
    /// The class written into statements (default: the kind).
    pub fn class_word(&self) -> String {
        self.class.clone().unwrap_or_else(|| {
            serde_json::to_value(self.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default()
        })
    }
}

/// The import spec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSetSpec {
    pub schema: String,
    #[serde(default)]
    pub node: Option<String>,
    pub frame_from: String,
    #[serde(default)]
    pub order: Option<Value>,
    #[serde(default = "default_bits")]
    pub bits: u32,
    pub start: StartSpec,
    #[serde(default)]
    pub required_evidence: Level,
    #[serde(default)]
    pub policies: Policies,
    pub sources: Vec<SourceSpec>,
    #[serde(default)]
    pub evidence: EvidenceSpec,
    #[serde(default = "default_true")]
    pub hand_names: bool,
}

fn default_bits() -> u32 {
    6
}
fn default_true() -> bool {
    true
}

const TOP_FIELDS: &[&str] = &[
    "schema",
    "node",
    "frame_from",
    "order",
    "bits",
    "start",
    "required_evidence",
    "policies",
    "sources",
    "evidence",
    "hand_names",
];
const COMMON_SOURCE_FIELDS: &[&str] = &["id", "kind", "class", "hand_name"];

impl TableSetSpec {
    /// Parse and validate a spec text (strict field lists, unique ids, frame source present).
    pub fn from_json(text: &str) -> Result<TableSetSpec, ImportError> {
        let v: Value =
            serde_json::from_str(text).map_err(|e| ImportError::Spec(format!("json: {e}")))?;
        let obj = v
            .as_object()
            .ok_or_else(|| ImportError::Spec("the spec is not an object".into()))?;
        for k in obj.keys() {
            if !TOP_FIELDS.contains(&k.as_str()) {
                return Err(ImportError::Spec(format!("unknown top-level field {k:?}")));
            }
        }
        let srcs = obj
            .get("sources")
            .and_then(Value::as_array)
            .ok_or_else(|| ImportError::Spec("sources must be an array".into()))?;
        for (i, s) in srcs.iter().enumerate() {
            let so = s
                .as_object()
                .ok_or_else(|| ImportError::Spec(format!("source {i} is not an object")))?;
            let kind_s = so.get("kind").and_then(Value::as_str).ok_or_else(|| {
                ImportError::Spec(format!("source {i} without a string kind"))
            })?;
            let kind = SourceKind::parse(kind_s)
                .ok_or_else(|| ImportError::Spec(format!("source {i}: unknown kind {kind_s:?}")))?;
            for k in so.keys() {
                if !COMMON_SOURCE_FIELDS.contains(&k.as_str()) && !kind.fields().contains(&k.as_str())
                {
                    return Err(ImportError::Spec(format!(
                        "source {i} ({kind_s}): field {k:?} is not a field of this kind"
                    )));
                }
            }
        }
        let spec: TableSetSpec =
            serde_json::from_value(v).map_err(|e| ImportError::Spec(format!("spec: {e}")))?;
        spec.validate()?;
        Ok(spec)
    }

    /// Read a spec file.
    pub fn read(path: &Path) -> Result<TableSetSpec, ImportError> {
        let t = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
        Self::from_json(&t)
    }

    fn validate(&self) -> Result<(), ImportError> {
        if self.schema != SPEC_SCHEMA {
            return Err(ImportError::Spec(format!(
                "schema {:?}, expected {SPEC_SCHEMA}",
                self.schema
            )));
        }
        if !(4..=16).contains(&self.bits) {
            return Err(ImportError::Spec(format!("bits {} outside 4..=16", self.bits)));
        }
        if self.start.format == StartFormat::HandStatement && self.start.hand_format.is_none() {
            return Err(ImportError::Spec(
                "start format hand_statement needs hand_format".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        for s in &self.sources {
            if s.id.is_empty() || !ids.insert(s.id.as_str()) {
                return Err(ImportError::Spec(format!("source id {:?} empty or repeated", s.id)));
            }
            let need = |f: bool, what: &str| -> Result<(), ImportError> {
                if f {
                    Ok(())
                } else {
                    Err(ImportError::Spec(format!("source {}: {what} required", s.id)))
                }
            };
            match s.kind {
                SourceKind::EngineTable => need(s.path.is_some(), "path")?,
                SourceKind::TableSet => {
                    need(s.priority_file.is_some(), "priority_file")?;
                    need(s.tables_dir.is_some(), "tables_dir")?;
                    need(s.variant.is_some(), "variant")?;
                }
                SourceKind::Recipe => need(s.path.is_some(), "path")?,
                SourceKind::SuppliedRelations => need(s.path.is_some(), "path")?,
                SourceKind::SymmetryCredit => need(s.group.is_some(), "group")?,
                SourceKind::Terminals => need(
                    s.keys.is_some() != s.path.is_some(),
                    "exactly one of keys / path",
                )?,
            }
            let mut rm = BTreeSet::new();
            for r in &s.removed {
                if !rm.insert(r.label) {
                    return Err(ImportError::Spec(format!(
                        "source {}: removed label {} twice",
                        s.id, r.label
                    )));
                }
                if s.records.as_ref().is_some_and(|v| v.contains(&r.label)) {
                    return Err(ImportError::Spec(format!(
                        "source {}: label {} both selected and removed",
                        s.id, r.label
                    )));
                }
            }
            if let Some(sel) = &s.records {
                let set: BTreeSet<_> = sel.iter().collect();
                if set.len() != sel.len() {
                    return Err(ImportError::Spec(format!(
                        "source {}: a selected label repeats",
                        s.id
                    )));
                }
            }
        }
        let fs = self
            .sources
            .iter()
            .find(|s| s.id == self.frame_from)
            .ok_or_else(|| ImportError::Spec(format!("frame_from {:?} names no source", self.frame_from)))?;
        if fs.kind != SourceKind::EngineTable && fs.kind != SourceKind::TableSet {
            return Err(ImportError::Spec(
                "frame_from must name an engine_table or table_set source".into(),
            ));
        }
        Ok(())
    }

    /// `sha256:` of the canonical JSON of the spec (the spec's identity in every output).
    pub fn digest(&self) -> String {
        let v = serde_json::to_value(self).unwrap_or(Value::Null);
        let text = crate::schema::canon::canonical_json(&v, false).unwrap_or_default();
        crate::closure::sha256::sha256_tag(text.as_bytes())
    }
}

/// Resolve a spec path against the base directory (absolute paths unchanged).
pub fn resolve(base: &Path, p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() { pb } else { base.join(pb) }
}
