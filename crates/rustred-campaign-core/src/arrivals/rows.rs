//! The parent's closure rows: a check rows file (core `import::statement::row_json` lines: `key`, `in_start`, `status`,
//! `server {kind, record_sha256, ..}`), as written by `check_streamed` (Tier K keys-only superset or Tier C). The arrival
//! walk needs, per closure key, the record that serves it (first match) and the key the record is applied at.
//!
//! Fail closed: a row served by a RELATION (supplied / recipe relation) or a CREDIT without its image is refused (the
//! walk would miss that key's hand-down terms); terminal and open rows send nothing (no relation is applied there) and
//! are counted.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;

use serde_json::Value;

use crate::closure::sha256::Sha256;
use crate::import::ImportedSet;

/// A rule-served closure key: the key, the key the record is applied at (the image for a credit), the record.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServedRow {
    pub key: Vec<i32>,
    pub applied: Vec<i32>,
    pub table: usize,
    pub record: usize,
    pub in_start: bool,
}

/// The parent's closure as read from a rows file.
#[derive(Clone, Debug, Default)]
pub struct ParentRows {
    /// Rule-served rows, sorted by key.
    pub served: Vec<ServedRow>,
    /// Every closure key -> in_start (served, terminal and open rows).
    pub keys: BTreeMap<Vec<i32>, bool>,
    pub terminal: u64,
    pub open: u64,
    /// sha256 of the rows file bytes (hex).
    pub sha256: String,
    pub path: String,
}

impl ParentRows {
    /// Read a rows file against the parent's imported set (records found by their sha256 in the set).
    pub fn read(set: &ImportedSet, path: &Path) -> Result<ParentRows, String> {
        let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut h = Sha256::new();
        let mut out = ParentRows {
            path: path.display().to_string(),
            ..Default::default()
        };
        for (i, l) in std::io::BufReader::with_capacity(1 << 20, f).lines().enumerate() {
            let l = l.map_err(|e| format!("{}: {e}", path.display()))?;
            h.update(l.as_bytes());
            h.update(b"\n");
            if l.trim().is_empty() {
                continue;
            }
            let at = || format!("{} line {}", path.display(), i + 1);
            let v: Value = serde_json::from_str(&l).map_err(|e| format!("{}: {e}", at()))?;
            let key: Vec<i32> = serde_json::from_value(v["key"].clone()).map_err(|e| format!("{}: key: {e}", at()))?;
            let in_start = v["in_start"] == true;
            if out.keys.insert(key.clone(), in_start).is_some() {
                return Err(format!("{}: key {key:?} twice", at()));
            }
            match v["status"].as_str() {
                Some("served") => {}
                Some("terminal") => {
                    out.terminal += 1;
                    continue;
                }
                Some("open") => {
                    out.open += 1;
                    continue;
                }
                s => return Err(format!("{}: status {s:?}", at())),
            }
            let kind = v["server"]["kind"].as_str().unwrap_or("");
            let applied = match kind {
                "rule" => key.clone(),
                "credit" => match v["server"].get("image").or_else(|| v.get("image")) {
                    Some(img) if !img.is_null() => serde_json::from_value(img.clone())
                        .map_err(|e| format!("{}: image: {e}", at()))?,
                    _ => {
                        return Err(format!(
                            "{}: a credit row without its image (the walk cannot apply the record): refused",
                            at()
                        ));
                    }
                },
                k => {
                    return Err(format!(
                        "{}: a key served by {k:?} (not a table rule): the arrival walk refuses (fail closed)",
                        at()
                    ));
                }
            };
            // a sym-served rule row carries its image too (import5): the record is applied there
            let applied = match (kind, v["server"].get("image")) {
                ("rule", Some(img)) if !img.is_null() => {
                    serde_json::from_value(img.clone()).map_err(|e| format!("{}: image: {e}", at()))?
                }
                _ => applied,
            };
            let sha = v["server"]["record_sha256"]
                .as_str()
                .ok_or_else(|| format!("{}: server without record_sha256", at()))?;
            let (t, r) = set
                .find_sha(sha)
                .ok_or_else(|| format!("{}: record {sha} is not in the parent's table set", at()))?;
            out.served.push(ServedRow {
                key,
                applied,
                table: t,
                record: r,
                in_start,
            });
        }
        out.served.sort();
        out.sha256 = h.finish_hex();
        Ok(out)
    }
}
