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
    Source { source: String, msg: String },
    /// A label repeated inside its scope, a selected / removed label that does not exist.
    Label {
        source: String,
        label: i64,
        msg: String,
    },
    /// An evidence file the import cannot read or that contradicts itself.
    Evidence { path: String, line: usize, msg: String },
    /// The core refused something (dispatcher, closure, key codec).
    Core(String),
    /// A feature named by the spec that this version does not implement (refused, never ignored).
    Unsupported(String),
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
            } => write!(f, "{path}: sha256 {got} differs from the expected {expected}"),
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
