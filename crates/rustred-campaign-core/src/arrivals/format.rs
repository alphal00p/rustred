//! The arrival-set file `rustred.campaign.arrival-set.v1` (interfaces/arrivals.md 3): canonical JSON lines.
//!
//! Line 1 `{"schema", "kind": "header", ..}`: the bindings (parent closure rows, parent tables + digest, edge file +
//! producer, hand-down proof files, decision routes, tool). Then one `{"kind": "arrival", ..}` line per arriving child
//! key in strictly increasing key order: `key`, `senders` (distinct parent keys sending it at SUPPORT level),
//! `exact_senders` (distinct parent keys whose TOTAL coefficient on the key is proved non-zero; `null` when the exact
//! level was not computed for this key), `first` (the smallest sender: parent key, applied key, record sha256, term,
//! term key, how the non-zero was decided) and `first_exact`. Last line `{"kind": "summary", ..}`: counts and
//! `lines_digest` = sha256 over the canonical arrival lines (each + LF). Every line is written by
//! [`super::canonical_json`]; the file is pinned by its sha256.

use std::io::{BufRead, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{ARRIVAL_SET_SCHEMA, canonical_json};
use crate::closure::sha256::Sha256;

/// Who sends a child key first (the smallest parent key in key order, then the smallest (term, record)).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Sender {
    pub parent_key: Vec<i32>,
    pub applied: Vec<i32>,
    pub record_sha256: String,
    pub term: u32,
    pub term_key: Vec<i32>,
    /// How the non-zero coefficient was decided: `nonzero_on_locus:<locus>` | `key` | `key:exact_total` | ...
    pub by: String,
}

/// One arriving child key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrivalLine {
    pub key: Vec<i32>,
    pub senders: u64,
    pub exact_senders: Option<u64>,
    pub first: Sender,
    pub first_exact: Option<Sender>,
    /// The CHAIN DEPTH of the key (hand-down steps from a start key of depth 0, min over senders); None = not tracked.
    pub depth: Option<u32>,
}

impl ArrivalLine {
    pub fn json(&self) -> Value {
        let mut v = json!({"kind": "arrival", "key": self.key, "senders": self.senders, "exact_senders": self.exact_senders,
            "first": self.first, "first_exact": self.first_exact});
        if let Some(d) = self.depth {
            v["depth"] = json!(d);
        }
        v
    }
}

/// A read arrival-set file.
#[derive(Clone, Debug)]
pub struct ArrivalSet {
    pub header: Value,
    pub lines: Vec<ArrivalLine>,
    pub summary: Value,
    /// sha256 of the file bytes (hex).
    pub sha256: String,
}

impl ArrivalSet {
    /// The support-level key set (every line).
    pub fn keys(&self) -> impl Iterator<Item = &Vec<i32>> {
        self.lines.iter().map(|l| &l.key)
    }
    /// The keys proved to receive a non-zero TOTAL coefficient from at least one sender (exact level computed).
    pub fn exact_keys(&self) -> impl Iterator<Item = &Vec<i32>> {
        self.lines
            .iter()
            .filter(|l| l.exact_senders.is_some_and(|n| n > 0))
            .map(|l| &l.key)
    }
}

/// sha256 over the canonical arrival lines (each followed by LF).
pub fn lines_digest<'a>(lines: impl Iterator<Item = &'a ArrivalLine>) -> String {
    let mut h = Sha256::new();
    for l in lines {
        h.update(canonical_json(&l.json()).as_bytes());
        h.update(b"\n");
    }
    format!("sha256:{}", h.finish_hex())
}

/// Write an arrival-set file; returns its sha256 (hex). `lines` must be in strictly increasing key order.
pub fn write(
    path: &Path,
    header: &Value,
    lines: &[ArrivalLine],
    summary: &Value,
) -> Result<String, String> {
    let mut it = lines.iter().cloned().map(Ok);
    write_stream(path, header, &mut it, summary).map(|c| c.sha256)
}

/// What `write_stream` wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    /// sha256 of the file (hex).
    pub sha256: String,
    pub keys: u64,
    pub exact_keys: u64,
    pub exact_level_keys: u64,
}

/// Write an arrival-set file from a stream of lines (strictly increasing keys, checked), one line in memory at a
/// time; the summary (last line) gets the counts and the lines digest. Same bytes as `write` for the same lines. The
/// file is written as `<path>.partial` and renamed when complete: on an error nothing is left at `path` (an existing
/// file there is kept).
pub fn write_stream(
    path: &Path,
    header: &Value,
    lines: &mut dyn Iterator<Item = Result<ArrivalLine, String>>,
    summary: &Value,
) -> Result<Written, String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".partial");
    let tmp = std::path::PathBuf::from(tmp);
    let r = write_stream_to(&tmp, header, lines, summary);
    match &r {
        Ok(_) => std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?,
        Err(_) => {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    r
}

fn write_stream_to(
    path: &Path,
    header: &Value,
    lines: &mut dyn Iterator<Item = Result<ArrivalLine, String>>,
    summary: &Value,
) -> Result<Written, String> {
    let mut h = header.clone();
    h["schema"] = json!(ARRIVAL_SET_SCHEMA);
    h["kind"] = json!("header");
    let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
    let mut hs = Sha256::new();
    let mut put = |t: &str, w: &mut std::io::BufWriter<std::fs::File>| -> Result<(), String> {
        hs.update(t.as_bytes());
        hs.update(b"\n");
        writeln!(w, "{t}").map_err(|e| format!("{}: {e}", path.display()))
    };
    put(&canonical_json(&h), &mut w)?;
    let mut ld = Sha256::new();
    let (mut keys, mut exact_keys, mut exact_level_keys) = (0u64, 0u64, 0u64);
    let mut prev: Option<Vec<i32>> = None;
    for l in lines {
        let l = l?;
        if prev.as_ref().is_some_and(|p| *p >= l.key) {
            return Err(format!(
                "arrival lines not strictly increasing at {:?}",
                l.key
            ));
        }
        let t = canonical_json(&l.json());
        ld.update(t.as_bytes());
        ld.update(b"\n");
        put(&t, &mut w)?;
        keys += 1;
        exact_keys += l.exact_senders.is_some_and(|n| n > 0) as u64;
        exact_level_keys += l.exact_senders.is_some() as u64;
        prev = Some(l.key);
    }
    let mut s = summary.clone();
    s["schema"] = json!(ARRIVAL_SET_SCHEMA);
    s["kind"] = json!("summary");
    s["keys"] = json!(keys);
    s["exact_keys"] = json!(exact_keys);
    s["exact_level_keys"] = json!(exact_level_keys);
    s["lines_digest"] = json!(format!("sha256:{}", ld.finish_hex()));
    put(&canonical_json(&s), &mut w)?;
    w.flush().map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Written {
        sha256: hs.finish_hex(),
        keys,
        exact_keys,
        exact_level_keys,
    })
}

/// Read an arrival-set file (schema, order, uniqueness, summary counts and lines digest verified).
pub fn read(path: &Path) -> Result<ArrivalSet, String> {
    let mut lines: Vec<ArrivalLine> = vec![];
    let r = read_streamed(path, &mut |a| {
        lines.push(a);
        Ok(())
    })?;
    Ok(ArrivalSet {
        header: r.header,
        lines,
        summary: r.summary,
        sha256: r.sha256,
    })
}

/// What `read_streamed` verified (the lines went to the caller).
#[derive(Clone, Debug)]
pub struct Streamed {
    pub header: Value,
    pub summary: Value,
    /// sha256 of the file (hex).
    pub sha256: String,
    pub keys: u64,
}

/// Read an arrival-set file line by line, handing each verified arrival line to `f` (one line in memory at a time);
/// the same checks as `read` (schema, canonical lines, strictly increasing keys, summary counts, lines digest). A file
/// that fails a check returns Err AFTER `f` saw the lines before the failure: callers keep nothing on Err.
pub fn read_streamed(
    path: &Path,
    f: &mut dyn FnMut(ArrivalLine) -> Result<(), String>,
) -> Result<Streamed, String> {
    let fh = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hs = Sha256::new();
    let mut ld = Sha256::new();
    let (mut header, mut summary) = (None, None);
    let mut prev: Option<Vec<i32>> = None;
    let mut n = 0u64;
    for (i, l) in std::io::BufReader::with_capacity(1 << 20, fh)
        .lines()
        .enumerate()
    {
        let l = l.map_err(|e| format!("{}: {e}", path.display()))?;
        hs.update(l.as_bytes());
        hs.update(b"\n");
        let v: Value = serde_json::from_str(&l)
            .map_err(|e| format!("{} line {}: {e}", path.display(), i + 1))?;
        let at = || format!("{} line {}", path.display(), i + 1);
        if summary.is_some() {
            return Err(format!("{}: a line after the summary", at()));
        }
        match v["kind"].as_str() {
            Some("header") if i == 0 => {
                if v["schema"] != ARRIVAL_SET_SCHEMA {
                    return Err(format!("{}: schema {}", at(), v["schema"]));
                }
                header = Some(v);
            }
            Some("arrival") if header.is_some() => {
                let a = ArrivalLine {
                    key: serde_json::from_value(v["key"].clone())
                        .map_err(|e| format!("{}: key: {e}", at()))?,
                    senders: v["senders"]
                        .as_u64()
                        .ok_or_else(|| format!("{}: senders", at()))?,
                    exact_senders: v["exact_senders"].as_u64(),
                    first: serde_json::from_value(v["first"].clone())
                        .map_err(|e| format!("{}: first: {e}", at()))?,
                    first_exact: match &v["first_exact"] {
                        Value::Null => None,
                        x => Some(
                            serde_json::from_value(x.clone())
                                .map_err(|e| format!("{}: first_exact: {e}", at()))?,
                        ),
                    },
                    depth: v["depth"].as_u64().map(|d| d as u32),
                };
                if a.senders == 0 {
                    return Err(format!("{}: a key without a sender", at()));
                }
                let c = canonical_json(&a.json());
                if c != canonical_json(&v) {
                    return Err(format!("{}: not a canonical arrival line", at()));
                }
                if prev.as_ref().is_some_and(|p| *p >= a.key) {
                    return Err(format!("{}: keys not strictly increasing", at()));
                }
                ld.update(c.as_bytes());
                ld.update(b"\n");
                prev = Some(a.key.clone());
                n += 1;
                f(a)?;
            }
            Some("summary") if header.is_some() => summary = Some(v),
            _ => return Err(format!("{}: unexpected line kind {}", at(), v["kind"])),
        }
    }
    let header = header.ok_or_else(|| format!("{}: no header", path.display()))?;
    let summary = summary.ok_or_else(|| format!("{}: no summary (truncated?)", path.display()))?;
    if summary["keys"].as_u64() != Some(n) {
        return Err(format!(
            "{}: summary keys {} != {} lines",
            path.display(),
            summary["keys"],
            n
        ));
    }
    if summary["lines_digest"].as_str() != Some(format!("sha256:{}", ld.finish_hex()).as_str()) {
        return Err(format!("{}: lines_digest mismatch", path.display()));
    }
    Ok(Streamed {
        header,
        summary,
        sha256: hs.finish_hex(),
        keys: n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(k: &[i32], n: u64) -> ArrivalLine {
        let s = Sender {
            parent_key: vec![2, 1],
            applied: vec![2, 1],
            record_sha256: "ab".repeat(32),
            term: 1,
            term_key: vec![2, 0],
            by: "key".into(),
        };
        ArrivalLine {
            key: k.to_vec(),
            senders: n,
            exact_senders: Some(n),
            first: s.clone(),
            first_exact: Some(s),
            depth: (n > 1).then_some(n as u32 - 1),
        }
    }

    #[test]
    fn round_trip_and_refusals() {
        let d = std::env::temp_dir().join(format!("arrivals-format-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("a.jsonl");
        let ls = vec![line(&[1, 1], 2), line(&[2, 1], 1)];
        let sha = write(&p, &json!({"parent": {"node": "P"}}), &ls, &json!({})).unwrap();
        let a = read(&p).unwrap();
        assert_eq!(a.sha256, sha);
        assert_eq!(a.lines, ls);
        assert_eq!(a.summary["exact_keys"], 2);
        // unsorted input refused at write; the file written before is untouched and no partial file is left
        assert!(
            write(
                &p,
                &json!({}),
                &[line(&[2, 1], 1), line(&[1, 1], 1)],
                &json!({})
            )
            .is_err()
        );
        assert_eq!(read(&p).unwrap().sha256, sha);
        assert!(!d.join("a.jsonl.partial").exists());
        // a forged count is refused at read (lines digest)
        let text = std::fs::read_to_string(&p)
            .unwrap()
            .replace("\"senders\":2", "\"senders\":3");
        std::fs::write(&p, text).unwrap();
        assert!(read(&p).unwrap_err().contains("lines_digest"));
        // a truncated file is refused
        write(&p, &json!({}), &ls, &json!({})).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        let cut: Vec<&str> = text.lines().take(2).collect();
        std::fs::write(&p, cut.join("\n") + "\n").unwrap();
        assert!(read(&p).unwrap_err().contains("summary"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
