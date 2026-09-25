//! Lossless campaign journal and bounded, read-only native event observation.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use crate::cli::error::CliError;

const MAX_RECORD_BYTES: usize = 1024 * 1024;
const MAX_POLL_BYTES: usize = 4 * MAX_RECORD_BYTES;

/// Evidence records cannot let a later duplicate key silently replace an
/// earlier failure or nonzero counter, including inside nested progress.
pub(super) fn parse_unique_json(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice::<UniqueValue>(bytes).map(|value| value.0)
}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("JSON with unique object fields")
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Bool(value)))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(value.into())))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(value.into())))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|number| UniqueValue(Value::Number(number)))
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value.to_owned())))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut array = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueValue>()? {
                    array.push(value.0);
                }
                Ok(UniqueValue(Value::Array(array)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Self::Value, A::Error> {
                let mut fields = serde_json::Map::new();
                while let Some(key) = object.next_key::<String>()? {
                    if fields.contains_key(&key) {
                        return Err(de::Error::custom(format!("duplicate JSON field {key:?}")));
                    }
                    fields.insert(key, object.next_value::<UniqueValue>()?.0);
                }
                Ok(UniqueValue(Value::Object(fields)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

pub(super) struct EventLog(File);

impl EventLog {
    pub(super) fn open(path: &Path) -> Result<Self, CliError> {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)
            .map_err(|error| {
                CliError::OutputIo(format!("open campaign events {}: {error}", path.display()))
            })?;
        // A crash may leave a partial last record. Preserve those bytes for
        // diagnostics, but keep the next complete event on its own JSONL line.
        let boundary = (|| -> io::Result<()> {
            if file.metadata()?.len() != 0 {
                file.seek(SeekFrom::End(-1))?;
                let mut last = [0_u8; 1];
                file.read_exact(&mut last)?;
                if last[0] != b'\n' {
                    file.write_all(b"\n")?;
                    file.flush()?;
                }
            }
            Ok(())
        })();
        boundary.map_err(|error| {
            CliError::OutputIo(format!(
                "recover campaign event boundary {}: {error}",
                path.display()
            ))
        })?;
        Ok(Self(file))
    }

    /// Every accepted event is written as one complete JSONL record. The status
    /// snapshot may coalesce heartbeats, but this journal never coalesces events.
    pub(super) fn emit(&mut self, event: &Value) -> Result<(), CliError> {
        serde_json::to_writer(&mut self.0, event)
            .map_err(|error| CliError::OutputIo(format!("serialize campaign event: {error}")))?;
        self.0
            .write_all(b"\n")
            .and_then(|()| self.0.flush())
            .map_err(|error| CliError::OutputIo(format!("write campaign event: {error}")))
    }
}

#[derive(Default)]
pub(super) struct NativeTail {
    identity: Option<(u64, u64)>,
    offset: u64,
    pending: Vec<u8>,
    discarding: bool,
    pub(super) latest: Value,
    pub(super) saved_checkpoint: Option<Value>,
    pub(super) checkpoint_write: Option<Value>,
    pub(super) completion: Option<Value>,
    pub(super) progress_updated_at_unix_seconds: Option<f64>,
    pub(super) invalid_records: u64,
    pub(super) oversized_records: u64,
    pub(super) rotations: u64,
    pub(super) backlogged: bool,
    last_progress: Option<Value>,
}

impl NativeTail {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn poll(&mut self, path: &Path) -> Result<(), CliError> {
        self.read(path).map_err(|error| {
            CliError::InputIo(format!("read native events {}: {error}", path.display()))
        })
    }

    fn read(&mut self, path: &Path) -> io::Result<()> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        let metadata = file.metadata()?;
        let identity = identity(&metadata);
        if self
            .identity
            .is_some_and(|previous| previous != identity || metadata.len() < self.offset)
        {
            self.offset = 0;
            self.pending.clear();
            self.discarding = false;
            self.latest = Value::Null;
            self.last_progress = None;
            self.saved_checkpoint = None;
            self.checkpoint_write = None;
            self.completion = None;
            self.progress_updated_at_unix_seconds = None;
            self.rotations = self.rotations.saturating_add(1);
        }
        self.identity = Some(identity);
        file.seek(SeekFrom::Start(self.offset))?;
        // An old file must not become fresh merely because a viewer opened it.
        let modified = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|time| time.as_secs_f64());
        let mut remaining = MAX_POLL_BYTES;
        let mut chunk = [0_u8; 64 * 1024];
        while remaining != 0 {
            let limit = remaining.min(chunk.len());
            let count = file.read(&mut chunk[..limit])?;
            if count == 0 {
                break;
            }
            self.offset += count as u64;
            remaining -= count;
            for &byte in &chunk[..count] {
                if byte == b'\n' {
                    self.finish_record(modified);
                } else if !self.discarding {
                    if self.pending.len() == MAX_RECORD_BYTES {
                        self.pending.clear();
                        self.discarding = true;
                        self.oversized_records = self.oversized_records.saturating_add(1);
                    } else {
                        self.pending.push(byte);
                    }
                }
            }
        }
        self.backlogged = self.offset < file.metadata()?.len();
        Ok(())
    }

    fn finish_record(&mut self, modified: Option<f64>) {
        if self.discarding {
            self.discarding = false;
        } else if !self.pending.is_empty() {
            match parse_unique_json(&self.pending) {
                Ok(value) if value.is_object() => self.observe(value, modified),
                _ => self.invalid_records = self.invalid_records.saturating_add(1),
            }
        }
        self.pending.clear();
    }

    fn observe(&mut self, record: Value, modified: Option<f64>) {
        let native = record
            .get("progress")
            .filter(|value| value.is_object())
            .unwrap_or(&record);
        let counters = native
            .get("snapshot")
            .filter(|value| value.is_object())
            .unwrap_or(native);
        let age = record
            .get("progress_age_seconds")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(0.0);
        let observed = modified.map(|time| (time - age).max(0.0));
        if self.last_progress.as_ref() != Some(native) {
            self.progress_updated_at_unix_seconds = observed;
            self.last_progress = Some(native.clone());
        } else if let Some(observed) = observed {
            // The final repeated heartbeat has the most precise age relative
            // to file modification time. Refine older, but never fresher.
            self.progress_updated_at_unix_seconds = Some(
                self.progress_updated_at_unix_seconds
                    .map_or(observed, |previous| previous.min(observed)),
            );
        }
        if let Some(saved) = counters
            .get("checkpoint")
            .or_else(|| native.get("checkpoint"))
            .filter(|value| value.is_object() && value["state"] == "saved")
        {
            let generation = saved["generation"].as_u64().unwrap_or(0);
            let previous = self
                .saved_checkpoint
                .as_ref()
                .and_then(|value| value["generation"].as_u64());
            if previous.is_none_or(|previous| generation >= previous) {
                self.saved_checkpoint = Some(saved.clone());
                if self.checkpoint_write.as_ref().is_some_and(|writing| {
                    writing["generation"]
                        .as_u64()
                        .is_none_or(|writing| writing <= generation)
                }) {
                    self.checkpoint_write = None;
                }
            }
        }
        if let Some(writing) = counters
            .get("checkpoint_write")
            .or_else(|| native.get("checkpoint_write"))
            .filter(|value| value.is_object() && value["state"] == "writing")
        {
            let newer = self
                .saved_checkpoint
                .as_ref()
                .and_then(|saved| saved["generation"].as_u64())
                .is_none_or(|saved| {
                    writing["generation"]
                        .as_u64()
                        .is_none_or(|generation| generation > saved)
                });
            if newer {
                self.checkpoint_write = Some(writing.clone());
            }
        }
        if native["event"] == "finished" || counters["phase"] == "finished" {
            self.completion = Some(native.clone());
        }
        self.latest = record;
    }

    pub(super) fn diagnostics(&self) -> Value {
        json!({"invalid_records":self.invalid_records,"oversized_records":self.oversized_records,
            "rotations":self.rotations,"backlogged":self.backlogged,"partial_record_bytes":self.pending.len()})
    }
}

#[cfg(unix)]
fn identity(metadata: &fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}

#[cfg(not(unix))]
fn identity(metadata: &fs::Metadata) -> (u64, u64) {
    (
        metadata
            .created()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |time| time.as_secs()),
        0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let directory = std::env::temp_dir().join(format!(
                "rustred-native-tail-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).unwrap();
            Self(directory)
        }
        fn path(&self) -> PathBuf {
            self.0.join("events.jsonl")
        }
        fn append(&self, data: &[u8]) {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.path())
                .unwrap()
                .write_all(data)
                .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn partial_invalid_and_oversize_records_are_bounded() {
        let fixture = Fixture::new();
        let mut tail = NativeTail::new();
        tail.poll(&fixture.path()).unwrap();
        fixture.append(b"{\"event\":\"par");
        tail.poll(&fixture.path()).unwrap();
        assert!(tail.latest.is_null());
        fixture.append(b"tial\"}\nnot-json\n");
        fixture.append(&vec![b'x'; MAX_RECORD_BYTES + 1]);
        fixture.append(b"\n{\"event\":\"last\"}\n");
        tail.poll(&fixture.path()).unwrap();
        assert_eq!(tail.latest["event"], "last");
        assert_eq!(tail.invalid_records, 1);
        assert_eq!(tail.oversized_records, 1);
        assert!(tail.pending.len() <= MAX_RECORD_BYTES);
    }

    #[test]
    fn duplicate_keys_at_any_depth_cannot_establish_completion() {
        let fixture = Fixture::new();
        fixture.append(br#"{"event":"finished","failed_nodes":1,"failed_nodes":0}"#);
        fixture.append(b"\n");
        fixture.append(br#"{"event":"heartbeat","progress":{"event":"finished","parallel":{"active_workers":2,"active_workers":0}}}"#);
        fixture.append(b"\n");
        let mut tail = NativeTail::new();
        tail.poll(&fixture.path()).unwrap();
        assert!(tail.completion.is_none());
        assert!(tail.latest.is_null());
        assert_eq!(tail.invalid_records, 2);
        let value =
            json!({"null":null,"true":true,"float":0.25,"signed":-2,"array":[1,"x",{"a":false}]});
        assert_eq!(
            parse_unique_json(&serde_json::to_vec(&value).unwrap()).unwrap(),
            value
        );
    }

    #[test]
    fn reads_are_bounded_and_rotation_clears_old_completion() {
        let fixture = Fixture::new();
        fixture.append(&vec![b'x'; MAX_POLL_BYTES + 30]);
        let mut tail = NativeTail::new();
        tail.poll(&fixture.path()).unwrap();
        assert_eq!(tail.offset, MAX_POLL_BYTES as u64);
        assert!(tail.backlogged);
        fixture.append(b"\n{\"event\":\"finished\",\"status\":\"paused\"}\n");
        tail.poll(&fixture.path()).unwrap();
        assert!(tail.completion.is_some());
        fs::rename(fixture.path(), fixture.0.join("old.jsonl")).unwrap();
        fixture.append(b"{\"event\":\"new\"}\n");
        tail.poll(&fixture.path()).unwrap();
        assert_eq!(tail.latest["event"], "new");
        assert!(tail.completion.is_none());
        assert_eq!(tail.rotations, 1);
        fs::write(fixture.path(), b"{}\n").unwrap();
        tail.poll(&fixture.path()).unwrap();
        assert_eq!(tail.rotations, 2);
    }

    #[test]
    fn saved_checkpoint_survives_writing_and_heartbeats_without_freshening_progress() {
        let mut tail = NativeTail::new();
        tail.observe(json!({"event":"checkpoint_saved","checkpoint":{"state":"saved","generation":3,"saved_unix_time":90}}), Some(100.0));
        let writing = json!({"event":"checkpoint_started","checkpoint_write":{"state":"writing","generation":4}});
        tail.observe(writing.clone(), Some(101.0));
        tail.observe(
            json!({"event":"heartbeat","progress":writing,"progress_age_seconds":2.0}),
            Some(103.0),
        );
        assert_eq!(tail.saved_checkpoint.as_ref().unwrap()["generation"], 3);
        assert_eq!(tail.checkpoint_write.as_ref().unwrap()["generation"], 4);
        assert_eq!(tail.progress_updated_at_unix_seconds, Some(101.0));
        tail.observe(
            json!({"event":"checkpoint_saved","checkpoint":{"state":"saved","generation":4}}),
            Some(104.0),
        );
        tail.observe(
            json!({"event":"heartbeat","progress":{"event":"progress"}}),
            Some(105.0),
        );
        assert_eq!(tail.saved_checkpoint.as_ref().unwrap()["generation"], 4);
        assert!(tail.checkpoint_write.is_none());
        tail.observe(
            json!({"event":"checkpoint_saved","checkpoint":{"state":"saved","generation":2}}),
            Some(106.0),
        );
        assert_eq!(tail.saved_checkpoint.as_ref().unwrap()["generation"], 4);
    }

    #[test]
    fn replayed_heartbeat_uses_file_age_and_exposes_completion() {
        let mut tail = NativeTail::new();
        let finished =
            json!({"event":"finished","status":"completed","completed_nodes":17,"frontiers":0});
        tail.observe(
            json!({"event":"heartbeat","progress":finished,"progress_age_seconds":12.0}),
            Some(100.0),
        );
        assert_eq!(tail.progress_updated_at_unix_seconds, Some(88.0));
        assert_eq!(tail.completion.as_ref().unwrap()["completed_nodes"], 17);
    }

    #[test]
    fn campaign_log_keeps_every_record_and_json_escapes_controls() {
        let fixture = Fixture::new();
        let mut events = EventLog::open(&fixture.path()).unwrap();
        for index in 0..5 {
            events
                .emit(&json!({"index":index,"error":"bad\n\u{1b}[31m"}))
                .unwrap();
        }
        let bytes = fs::read(fixture.path()).unwrap();
        assert!(!bytes.contains(&0x1b));
        let lines = std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 5);
        for (index, line) in lines.iter().enumerate() {
            assert_eq!(serde_json::from_str::<Value>(line).unwrap()["index"], index);
        }
    }

    #[test]
    fn replayed_identical_heartbeats_refine_age_without_freshening() {
        let mut tail = NativeTail::new();
        let progress = json!({"event":"working","completed_nodes":17});
        for age in [0.0, 5.0, 50.0] {
            tail.observe(
                json!({"event":"heartbeat","progress":progress,"progress_age_seconds":age}),
                Some(100.0),
            );
        }
        assert_eq!(tail.progress_updated_at_unix_seconds, Some(50.0));
        tail.observe(
            json!({"event":"heartbeat","progress":progress,"progress_age_seconds":0.0}),
            Some(110.0),
        );
        assert_eq!(tail.progress_updated_at_unix_seconds, Some(50.0));
    }

    #[test]
    fn journal_reopen_preserves_partial_bytes_and_separates_next_event() {
        let fixture = Fixture::new();
        fixture.append(b"{\"event\":\"interrupted");
        EventLog::open(&fixture.path())
            .unwrap()
            .emit(&json!({"event":"resumed"}))
            .unwrap();
        let mut tail = NativeTail::new();
        tail.poll(&fixture.path()).unwrap();
        assert_eq!(tail.invalid_records, 1);
        assert_eq!(tail.latest["event"], "resumed");
        assert!(
            fs::read(fixture.path())
                .unwrap()
                .starts_with(b"{\"event\":\"interrupted\n")
        );
    }
}
