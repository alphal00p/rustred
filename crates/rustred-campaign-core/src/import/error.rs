//! Typed errors of the import (fail closed: every refusal names the source, the file, the line and the label).

use std::fmt;

/// An import error. Every mathematical outcome (a key that is open, a fact that is missing) is a RESULT, never an
/// error; errors are inputs the import cannot classify or that contradict their own digests.
#[derive(Debug)]
pub enum ImportError {
    Io {
        path: String,
        err: std::io::Error,
    },
    /// The spec itself is malformed (unknown kind / field, duplicate source id, inconsistent options).
    Spec(String),
    /// A digest given in the spec or in a SHA256SUMS file does not match the file.
    Digest {
        path: String,
        expected: String,
        got: String,
    },
    /// A line of a source the import cannot classify or read.
    Line {
        source: String,
        path: String,
        line: usize,
        msg: String,
    },
    /// A source whose content contradicts the set (header identity, chart, arity).
    Source {
        source: String,
        msg: String,
    },
    /// A label repeated inside its scope, a selected / removed label that does not exist.
    Label {
        source: String,
        label: i64,
        msg: String,
    },
    /// An evidence file the import cannot read or that contradicts itself.
    Evidence {
        path: String,
        line: usize,
        msg: String,
    },
    /// The core refused something (dispatcher, closure, key codec).
    Core(String),
    /// A feature named by the spec that this version does not implement (refused, never ignored).
    Unsupported(String),
    /// A TABLE refused for what its own records state (interfaces/import.md 18): a `sym_zero` record, a malformed
    /// `sym` record or terminal ledger, a completeness marker that contradicts the file, an order restated otherwise.
    Refused {
        source: String,
        path: String,
        line: usize,
        reason: Box<TableRefusal>,
    },
}

/// Why a table is refused (typed; [`TableRefusal::word`] is the stable short word used in reports).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum TableRefusal {
    /// A `sym_zero` record: zero by symmetry needs a sign flip, impossible with squared propagators.
    SymZero,
    /// A `sym` record the import cannot read (malformed, a member `exceptions` / `rhs`, sign `key`, ...).
    Sym { msg: String },
    /// A `composite_terminal_ledger` record whose keys cannot be read or whose `count` differs from its keys.
    Ledger { msg: String },
    /// A completeness marker (`composite_assembly_complete` / `assembly_end`) that is malformed or contradicts the file:
    /// `field` stated `stated`, the file has `found`.
    Completion {
        marker: String,
        field: String,
        stated: String,
        found: String,
    },
    /// `assembly_provenance.one_physical_order` differs from the header's order name.
    OrderMismatch { stated: String, header: String },
    /// A part of a record that must be held in memory is larger than the per-record in-memory budget (spec
    /// `reader.record_budget_bytes`; lane bigline): `what` (e.g. the line without its rhs value, a coefficient term)
    /// of `bytes` bytes. The record cannot be read or evaluated in memory; never skipped, never counted as served.
    RecordTooLarge {
        record: Option<i64>,
        what: String,
        bytes: u64,
        budget: u64,
    },
    /// The file does not show that it is complete (lane partfix4): no `end` record (a file cut at a line boundary
    /// looks exactly like this), or a rule / `sym` record after the `end` record. Overridable for a missing `end`
    /// only, per source with a recorded reason (`marker_rules.accept_missing_end`).
    Incomplete { msg: String },
}

impl TableRefusal {
    /// Short stable word: `sym_zero`, `sym_malformed`, `ledger_malformed`, `completion:<marker>:<field>`,
    /// `order_mismatch`, `record_too_large`, `table_incomplete`.
    pub fn word(&self) -> String {
        match self {
            TableRefusal::SymZero => "sym_zero".into(),
            TableRefusal::Sym { .. } => "sym_malformed".into(),
            TableRefusal::Ledger { .. } => "ledger_malformed".into(),
            TableRefusal::Completion { marker, field, .. } => {
                format!("completion:{marker}:{field}")
            }
            TableRefusal::OrderMismatch { .. } => "order_mismatch".into(),
            TableRefusal::RecordTooLarge { .. } => "record_too_large".into(),
            TableRefusal::Incomplete { .. } => "table_incomplete".into(),
        }
    }
}

impl fmt::Display for TableRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableRefusal::SymZero => write!(
                f,
                "sym_zero record: zero by symmetry needs a sign flip (impossible with squared propagators)"
            ),
            TableRefusal::Sym { msg } => write!(f, "sym record: {msg}"),
            TableRefusal::Ledger { msg } => write!(f, "composite_terminal_ledger: {msg}"),
            TableRefusal::Completion {
                marker,
                field,
                stated,
                found,
            } => write!(f, "{marker}: {field} states {stated}, the file has {found}"),
            TableRefusal::OrderMismatch { stated, header } => write!(
                f,
                "assembly_provenance.one_physical_order {stated:?} differs from the header order {header:?}"
            ),
            TableRefusal::RecordTooLarge {
                record,
                what,
                bytes,
                budget,
            } => {
                write!(f, "record too large for in-memory evaluation")?;
                if let Some(r) = record {
                    write!(f, " (record {r})")?;
                }
                write!(
                    f,
                    ": {what} of {bytes} bytes exceeds the per-record budget of {budget} bytes"
                )
            }
            TableRefusal::Incomplete { msg } => write!(f, "table_incomplete: {msg}"),
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Io { path, err } => write!(f, "{path}: {err}"),
            ImportError::Spec(m) => write!(f, "spec: {m}"),
            ImportError::Digest {
                path,
                expected,
                got,
            } => write!(
                f,
                "{path}: sha256 {got} differs from the expected {expected}"
            ),
            ImportError::Line {
                source,
                path,
                line,
                msg,
            } => write!(f, "source {source}: {path} line {line}: {msg}"),
            ImportError::Source { source, msg } => write!(f, "source {source}: {msg}"),
            ImportError::Label { source, label, msg } => {
                write!(f, "source {source}: label {label}: {msg}")
            }
            ImportError::Evidence { path, line, msg } => {
                write!(f, "evidence {path} line {line}: {msg}")
            }
            ImportError::Core(m) => write!(f, "core: {m}"),
            ImportError::Unsupported(m) => write!(f, "unsupported: {m}"),
            ImportError::Refused {
                source,
                path,
                line,
                reason,
            } => write!(
                f,
                "source {source}: {path} line {line}: table refused: {reason}"
            ),
        }
    }
}

impl std::error::Error for ImportError {}

pub(crate) fn io_err(path: &std::path::Path, err: std::io::Error) -> ImportError {
    ImportError::Io {
        path: path.display().to_string(),
        err,
    }
}
