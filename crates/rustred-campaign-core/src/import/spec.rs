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
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
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

/// Per-key lift policy of records whose witness loci are ELIDED or ABSENT (interfaces/import.md section 3; the
/// coordinator's / user's ruling 2026-11-14: a first-class evidence level).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElidedPolicy {
    /// An elided record blocks every key where it is selected; a witness-absent record serves only through its
    /// regularity fact (`witness_absent`).
    Block,
    /// DEFAULT. Such a record serves a key iff an exact per-key proof (`EXACT_IN_SPAN_AT_KEY` /
    /// `EXACT_PROVED_AT_KEY`) is bound to exactly (record_sha256, key): the closure's strict dispatcher consults the
    /// evidence oracle (closure `PerKeyLift::Proof`); the key's level is `per-key`; without the proof the key is open
    /// (`blocked`). The dispatcher stays strict everywhere (no witness check is turned off).
    #[default]
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

/// Witness check of the dispatcher (interfaces/import.md 13.7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WitnessCheck {
    /// Strict (default): witness loci evaluated, elided / absent loci block unless lifted by evidence.
    #[default]
    On,
    /// DIAGNOSTIC: every table built with the witness check OFF (the independent checker's assumption for pre-guard
    /// tables); the closure is then `diagnostic` (`dispatch_not_strict`) and NOTHING is certified. Only for key-set
    /// comparisons with a delivery while the per-key facts do not exist.
    OffDiagnostic,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policies {
    #[serde(default)]
    pub elided: ElidedPolicy,
    #[serde(default)]
    pub witness_check: WitnessCheck,
    #[serde(default)]
    pub witness_absent: WitnessAbsentPolicy,
    #[serde(default)]
    pub tier: TierChoice,
}

/// Column map of a hand statement (JSONL of arrays), interfaces/import.md 7.2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandFormat {
    pub key: Col,
    /// Absent: the statement does not say (the in-box flag is then not compared).
    #[serde(default)]
    pub in_box: Option<Col>,
    pub label: Col,
    pub rule: Col,
    #[serde(default)]
    pub round: Option<Col>,
    /// When the round column is an array: the element holding the round.
    #[serde(default)]
    pub round_index: Option<usize>,
    /// The label that marks a terminal row (default "terminal").
    #[serde(default = "default_terminal_label")]
    pub terminal_label: String,
    /// The statement lists only SOME closure keys (e.g. the relation-served ones): keys it does not list are not
    /// differences and the key sets are not compared.
    #[serde(default)]
    pub partial: bool,
}

/// A column of a hand statement: a position in an array line, or a field name in an object line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Col {
    Index(usize),
    Field(String),
}

impl HandFormat {
    /// A positional (array-line) format with the default terminal label, no round, complete statement.
    pub fn positional(key: usize, in_box: usize, label: usize, rule: usize) -> HandFormat {
        HandFormat {
            key: Col::Index(key),
            in_box: Some(Col::Index(in_box)),
            label: Col::Index(label),
            rule: Col::Index(rule),
            round: None,
            round_index: None,
            terminal_label: default_terminal_label(),
            partial: false,
        }
    }
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
    /// A box document (`rustred.campaign.box.v1`, interfaces/campaign-core-domain.md section 7): its keys.
    Box,
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
            SourceKind::EngineTable => &[
                "path",
                "sha256",
                "records",
                "removed",
                "domain",
                "table_terminals",
                "marker_rules",
                "sym_group",
            ],
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
                "table_terminals",
                "marker_rules",
                "sym_group",
            ],
            SourceKind::Recipe => &["path", "sha256", "engine_digest", "assignment"],
            SourceKind::SuppliedRelations => {
                &["path", "sha256", "verification", "verify", "diagnostic"]
            }
            SourceKind::SymmetryCredit => &["group", "family", "group_format", "max_candidates"],
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
    /// `symmetry_credit`: the family file of the VERIFIED group (interfaces/import.md 13); absent = the legacy
    /// hand-written `rustred.campaign.credit-group.v1` file in `group` (unverified: the closure is `diagnostic`).
    #[serde(default)]
    pub family: Option<FileRef>,
    #[serde(default)]
    pub group_format: Option<super::symmetry::GroupFormat>,
    #[serde(default)]
    pub max_candidates: Option<u64>,
    /// `recipe`: PER-KEY ASSIGNMENT side file (until recipe v1.2 `domain.keys` is frozen; interfaces/import.md 14): JSONL
    /// lines `{"recipe": "<recipe id>", "keys": [[..], ..]}` (or `"key": [..]`); lines naming another recipe id are
    /// ignored, so one file can serve every recipe. The recipe then serves (and is asked for) ONLY those keys.
    #[serde(default)]
    pub assignment: Option<FileRef>,
    /// `supplied_relations`: re-derive every relation from the verified group at its key (interfaces/import.md 13.2).
    #[serde(default)]
    pub verify: Option<super::symmetry::GroupSpec>,
    #[serde(default)]
    pub keys: Option<Vec<Vec<i32>>>,
    #[serde(default)]
    pub preferred: Option<bool>,
    /// `supplied_relations` only: KEYS-ONLY relations of an external checker (no coefficients, no verification): the
    /// closure follows them as if they were certified, but a key they serve is NEVER certified (evidence none, basis
    /// `keys_only_relation_diagnostic`) and the check is blocked by `keys_only_relations_diagnostic` (lane import3: the
    /// key-set comparison with a checker's delivery before the solves exist). Refused with `verify` / `verification`.
    #[serde(default)]
    pub diagnostic: bool,
    /// Table sources (lane import5, interfaces/import.md 18.2): which terminals the TABLE declares (default `ledger`).
    /// Serialized only when given (spec digests of earlier specs unchanged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_terminals: Option<TableTerminals>,
    /// Table sources (18.3): the completeness check's escape for minimised tables that kept the original marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_rules: Option<MarkerRules>,
    /// Table sources (18.4): the verified group the `sym` records are checked against (default: derived from the
    /// frame header's `family_source`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sym_group: Option<super::symmetry::GroupSpec>,
}

/// Which terminals a table declares (interfaces/import.md 18.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableTerminals {
    /// The keys of `composite_terminal_ledger` records (default).
    #[default]
    Ledger,
    /// The ledger keys and every `residual` record's key (as `rr_node reduce` reads a table).
    LedgerAndResiduals,
    /// Nothing from the table (the spec's `terminals` sources only).
    None,
}

/// `marker_rules` (interfaces/import.md 18.3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkerRules {
    /// The reason a marker's `rules` GREATER than the file's rule records is accepted (e.g. "minimised table: the
    /// minimiser kept the original assembly's marker"); recorded in the load report.
    pub accept_fewer: String,
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

/// DEFAULT per-record in-memory budget of the table readers (bytes), used when the spec's `reader` does not set
/// `record_budget_bytes` (interfaces/import.md 19): the size above which a record part that must be held refuses,
/// typed. Memory policy L17 (2026-11-15: up to 64 GB per job): the default does not bite below 64 GiB.
pub const DEFAULT_RECORD_BUDGET_BYTES: u64 = 64 << 30;

/// DEFAULT length above which a line is STREAMED instead of read whole (`reader.stream_lines_above`; the path only,
/// never a result). Measured 2026-11-15 on the 14 five-loop tables with the longest lines (lane bigline): the largest
/// record part that must be held (the line without its `rhs` value) was 14.4 MB, the largest single coefficient term
/// 2.35 MB, the longest line 5.05 GB; 64 MiB keeps every ordinary line on the whole-line path and bounds a load's
/// memory per record to about that (index pass: 256 MiB of lines in flight + one line + its parse).
pub const DEFAULT_STREAM_LINES_ABOVE: u64 = 64 << 20;

/// Per-record memory limits of the table readers (spec field `reader`; lane bigline, interfaces/import.md 19).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReaderSpec {
    /// The most bytes of ONE record a reader holds at once (a whole line, the line without its `rhs` value, the key
    /// texts of a record, one coefficient term). A record whose kept part exceeds it is refused, typed
    /// (`record_too_large`); a coefficient beyond it cannot be evaluated in memory (served by a child job). Default
    /// [`DEFAULT_RECORD_BUDGET_BYTES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_budget_bytes: Option<u64>,
    /// Lines longer than this (bytes, without the newline) are streamed instead of read whole. Default
    /// [`DEFAULT_STREAM_LINES_ABOVE`] (or the budget when smaller); at most the budget. It changes no result (the
    /// streamed and the whole-line readers give identical records, digests and index files), only the path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_lines_above: Option<u64>,
    /// Records of each table re-read from the TABLE BYTES at load and re-hashed against the index's `record_sha256`
    /// (spread over the file, the first and the last included); a mismatch refuses the index and rebuilds it.
    /// Default [`DEFAULT_SPOT_CHECK_RECORDS`]; 0 = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_check_records: Option<usize>,
}

/// DEFAULT number of records spot-checked against the table bytes per table load (`reader.spot_check_records`).
pub const DEFAULT_SPOT_CHECK_RECORDS: usize = 64;

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
    /// Per-record memory limits of the readers (absent = the defaults; absent from the spec digest when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reader: Option<ReaderSpec>,
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
    "reader",
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
            let kind_s = so
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| ImportError::Spec(format!("source {i} without a string kind")))?;
            let kind = SourceKind::parse(kind_s)
                .ok_or_else(|| ImportError::Spec(format!("source {i}: unknown kind {kind_s:?}")))?;
            for k in so.keys() {
                if !COMMON_SOURCE_FIELDS.contains(&k.as_str())
                    && !kind.fields().contains(&k.as_str())
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

    /// Records spot-checked per table load (`reader.spot_check_records`, else [`DEFAULT_SPOT_CHECK_RECORDS`]).
    pub fn spot_check_records(&self) -> usize {
        self.reader
            .and_then(|r| r.spot_check_records)
            .unwrap_or(DEFAULT_SPOT_CHECK_RECORDS)
    }

    /// The reader limits of this spec (`reader`, else the defaults).
    pub fn read_limits(&self) -> crate::record::ReadLimits {
        let r = self.reader.unwrap_or_default();
        let budget = r.record_budget_bytes.unwrap_or(DEFAULT_RECORD_BUDGET_BYTES);
        crate::record::ReadLimits {
            record_budget: budget,
            whole_line_max: r
                .stream_lines_above
                .unwrap_or(DEFAULT_STREAM_LINES_ABOVE)
                .min(budget),
        }
    }

    fn validate(&self) -> Result<(), ImportError> {
        if let Some(r) = &self.reader {
            if r.record_budget_bytes == Some(0) {
                return Err(ImportError::Spec(
                    "reader.record_budget_bytes must be positive".into(),
                ));
            }
            let budget = r.record_budget_bytes.unwrap_or(DEFAULT_RECORD_BUDGET_BYTES);
            if let Some(w) = r.stream_lines_above
                && w > budget
            {
                return Err(ImportError::Spec(format!(
                    "reader.stream_lines_above {w} exceeds the record budget {budget}"
                )));
            }
        }
        if self.schema != SPEC_SCHEMA {
            return Err(ImportError::Spec(format!(
                "schema {:?}, expected {SPEC_SCHEMA}",
                self.schema
            )));
        }
        if !(4..=16).contains(&self.bits) {
            return Err(ImportError::Spec(format!(
                "bits {} outside 4..=16",
                self.bits
            )));
        }
        if self.start.format == StartFormat::HandStatement && self.start.hand_format.is_none() {
            return Err(ImportError::Spec(
                "start format hand_statement needs hand_format".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        for s in &self.sources {
            if s.id.is_empty() || !ids.insert(s.id.as_str()) {
                return Err(ImportError::Spec(format!(
                    "source id {:?} empty or repeated",
                    s.id
                )));
            }
            let need = |f: bool, what: &str| -> Result<(), ImportError> {
                if f {
                    Ok(())
                } else {
                    Err(ImportError::Spec(format!(
                        "source {}: {what} required",
                        s.id
                    )))
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
                SourceKind::SuppliedRelations => {
                    need(s.path.is_some(), "path")?;
                    if s.diagnostic && (s.verify.is_some() || s.verification.is_some()) {
                        return Err(ImportError::Spec(format!(
                            "source {}: diagnostic keys-only relations cannot carry verify / verification",
                            s.id
                        )));
                    }
                }
                SourceKind::SymmetryCredit => need(
                    s.group.is_some() || s.family.is_some() || s.group_format.is_some(),
                    "group, family or group_format",
                )?,
                SourceKind::Terminals => need(
                    s.keys.is_some() != s.path.is_some(),
                    "exactly one of keys / path",
                )?,
            }
            if s.diagnostic && s.kind != SourceKind::SuppliedRelations {
                return Err(ImportError::Spec(format!(
                    "source {}: `diagnostic` is a supplied_relations field",
                    s.id
                )));
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
            .ok_or_else(|| {
                ImportError::Spec(format!("frame_from {:?} names no source", self.frame_from))
            })?;
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
