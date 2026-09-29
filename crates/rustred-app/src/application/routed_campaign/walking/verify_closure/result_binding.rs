//! Binding of a published `result.json` to the checkpoint generation the
//! verifier read, so that the result audit and the edge-based verifier are
//! known to speak about the same walk state. Every committed record must be
//! published once with identical content (publication-time annotations
//! aside); rows only in the result may only be uncommitted failures; a
//! published `descendant_closed` claim must be re-derived by the oracle.
//! The result is streamed (bounded memory); rows are compared by a digest of
//! their canonical JSON (serde_json maps are key-sorted in this workspace).
use serde::de::{DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;
use std::io::{BufReader, Read};
use std::path::Path;

/// Fields the publisher adds or rewrites when a committed record is
/// published; they are not part of the committed record image.
pub(super) const PUBLICATION_FIELDS: [&str; 4] = [
    "descendant_closed",
    "final_representative_id",
    "responsibility_status",
    "local_classification_discharged",
];

pub(super) type Digest = [u8; 16];

/// Digest of a record with the publication-time fields removed (in place).
pub(super) fn record_digest(value: &mut Value) -> Digest {
    if let Some(object) = value.as_object_mut() {
        for field in PUBLICATION_FIELDS {
            object.remove(field);
        }
    }
    let bytes = serde_json::to_vec(value).expect("a JSON value serializes");
    let mut digest = [0u8; 16];
    digest.copy_from_slice(&blake3::hash(&bytes).as_bytes()[..16]);
    digest
}

pub(super) struct Row {
    pub id: Option<usize>,
    pub claimed_closed: bool,
    pub has_error: bool,
    pub frontiers: usize,
    pub digest: Digest,
}

#[derive(Default)]
pub(super) struct Top {
    pub checkpoint: Value,
    pub frontiers: Value,
    pub failed_nodes: Value,
    pub rows: u64,
    pub has_domains: bool,
    pub full_result: Option<bool>,
    /// Every byte of the file as read (the parse consumes it to EOF).
    pub bytes: u64,
    pub blake3: String,
}

/// Hashes and counts every byte handed to the parser.
struct Hashing<R> {
    inner: R,
    hasher: blake3::Hasher,
    bytes: u64,
}
impl<R: Read> Read for Hashing<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.hasher.update(&buffer[..read]);
        self.bytes += read as u64;
        Ok(read)
    }
}

/// Stream `path`, calling `on_row` for every `domains[]` row.
pub(super) fn stream(path: &Path, mut on_row: impl FnMut(Row)) -> Result<Top, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    // The buffer sits above the hasher, so the hasher sees large reads.
    let mut buffered = BufReader::with_capacity(
        1 << 22,
        Hashing {
            inner: file,
            hasher: blake3::Hasher::new(),
            bytes: 0,
        },
    );
    let mut deserializer = serde_json::Deserializer::from_reader(&mut buffered);
    let mut top = deserializer
        .deserialize_map(TopVisitor {
            on_row: &mut on_row,
        })
        .map_err(|e| format!("{}: {e}", path.display()))?;
    deserializer
        .end()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    drop(deserializer);
    let hashing = buffered.into_inner();
    top.bytes = hashing.bytes;
    top.blake3 = hashing.hasher.finalize().to_hex().to_string();
    Ok(top)
}

struct TopVisitor<'a, F> {
    on_row: &'a mut F,
}
impl<'de, F: FnMut(Row)> Visitor<'de> for TopVisitor<'_, F> {
    type Value = Top;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a walk result object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Top, A::Error> {
        let mut top = Top::default();
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "domains" => {
                    top.has_domains = true;
                    top.rows = map.next_value_seed(RowsSeed {
                        on_row: &mut *self.on_row,
                    })?;
                }
                "checkpoint" => top.checkpoint = map.next_value()?,
                "full_result_in_output_document" => top.full_result = map.next_value()?,
                "frontiers" => top.frontiers = map.next_value()?,
                "failed_nodes" => top.failed_nodes = map.next_value()?,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(top)
    }
}

struct RowsSeed<'a, F> {
    on_row: &'a mut F,
}
impl<'de, F: FnMut(Row)> DeserializeSeed<'de> for RowsSeed<'_, F> {
    type Value = u64;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<u64, D::Error> {
        deserializer.deserialize_seq(self)
    }
}
impl<'de, F: FnMut(Row)> Visitor<'de> for RowsSeed<'_, F> {
    type Value = u64;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an array of domain records")
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<u64, S::Error> {
        let mut rows = 0u64;
        while let Some(mut value) = seq.next_element::<Value>()? {
            rows += 1;
            let row = Row {
                id: value
                    .get("id")
                    .and_then(Value::as_u64)
                    .map(|id| id as usize),
                claimed_closed: value.get("descendant_closed") == Some(&Value::Bool(true)),
                has_error: value.get("error").is_some_and(|error| !error.is_null()),
                frontiers: value
                    .get("frontiers")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len),
                digest: record_digest(&mut value),
            };
            (self.on_row)(row);
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rows_stream_and_digests_ignore_publication_fields() {
        let directory =
            std::env::temp_dir().join(format!("rustred-result-binding-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("result.json");
        let committed =
            json!({"id":0,"error":null,"frontiers":[],"stats":{"events":3},"lower":[0]});
        let mut published = committed.clone();
        published["descendant_closed"] = json!(true);
        published["responsibility_status"] = json!("x");
        let document = json!({"checkpoint":{"generation":3},"domains":[published,
            {"id":1,"error":"boom","frontiers":[{"k":1}]}],"frontiers":1,"failed_nodes":1,"other":[1,2]});
        std::fs::write(&path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
        let mut rows = Vec::new();
        let top = stream(&path, |row| rows.push(row)).unwrap();
        assert_eq!(top.rows, 2);
        let written = std::fs::read(&path).unwrap();
        assert_eq!(top.bytes, written.len() as u64);
        assert_eq!(top.blake3, blake3::hash(&written).to_hex().to_string());
        assert!(top.has_domains);
        assert_eq!(top.checkpoint["generation"], 3);
        assert_eq!(top.frontiers, 1);
        assert_eq!(rows[0].id, Some(0));
        assert!(rows[0].claimed_closed && !rows[0].has_error);
        assert_eq!(rows[0].digest, record_digest(&mut committed.clone()));
        assert!(rows[1].has_error && rows[1].frontiers == 1 && !rows[1].claimed_closed);
        let mut changed = committed.clone();
        changed["stats"]["events"] = json!(4);
        assert_ne!(rows[0].digest, record_digest(&mut changed));
        let _ = std::fs::remove_dir_all(&directory);
    }
}
