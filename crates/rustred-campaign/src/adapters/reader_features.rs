//! READER FEATURES (TRICKS 65 residual, lane int8, 2026-11-18). A package or table may need reader features of the engine
//! that reads it (slot aliases, positioned recipes, alias equations, ...): a reader built before a feature SKIPS what it
//! does not know (TRICKS 65: an old reader skipped a `slot_aliases` line and served a key from the wrong rule). The engine
//! refuses a v4 payload it cannot read (batch 12a); this module lets an adapter refuse EARLIER, at validation (job
//! planning), when the program it pins does not declare a feature its input requires.
//!
//! Generic (L8): the REQUIRED features come from the input itself (a package's `library.json` or a table's header line,
//! field [`FIELD`]); the SUPPORTED features are what the spec declares for the pinned program (its frozen manifest or
//! its `version --json`). No feature name is a constant here. An input that declares nothing requires nothing; a field
//! that is present but not an array of strings is an error, never "nothing".

use std::io::BufRead;
use std::path::Path;

use serde_json::Value;

/// The field naming the reader features an input requires (array of strings).
pub const FIELD: &str = "required_features";

fn features_of(v: &Value, what: &str) -> Result<Vec<String>, String> {
    match v.get(FIELD) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| {
                x.as_str()
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| {
                        format!("{what}: `{FIELD}` must be an array of non-empty strings")
                    })
            })
            .collect(),
        Some(_) => Err(format!("{what}: `{FIELD}` must be an array of strings")),
    }
}

/// The reader features a package directory requires (its `library.json`, field [`FIELD`]).
pub fn required_by_package(dir: &Path) -> Result<Vec<String>, String> {
    let p = dir.join("library.json");
    let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let v: Value = serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", p.display()))?;
    features_of(&v, &p.display().to_string())
}

/// The reader features a table requires (its header line = the first line, field [`FIELD`]).
pub fn required_by_table(path: &Path) -> Result<Vec<String>, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut line = String::new();
    std::io::BufReader::new(f)
        .read_line(&mut line)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(line.trim_end())
        .map_err(|e| format!("{}: header line: {e}", path.display()))?;
    if v.get("type").and_then(Value::as_str) != Some("header") {
        return Err(format!(
            "{}: the first line is not a header",
            path.display()
        ));
    }
    features_of(&v, &format!("{} header", path.display()))
}

/// The required features the supported set lacks (in the input's order, without repeats).
pub fn missing(supported: &[String], required: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in required {
        if !supported.iter().any(|s| s == r) && !out.contains(r) {
            out.push(r.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "rr-reader-features-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_package_declares_what_it_requires_and_nothing_means_nothing() {
        let d = tmp("pkg");
        std::fs::write(
            d.join("library.json"),
            r#"{"schema":"x","required_features":["slot_aliases","positioned_recipes"]}"#,
        )
        .unwrap();
        assert_eq!(
            required_by_package(&d).unwrap(),
            vec!["slot_aliases".to_string(), "positioned_recipes".to_string()]
        );
        std::fs::write(d.join("library.json"), r#"{"schema":"x"}"#).unwrap();
        assert!(required_by_package(&d).unwrap().is_empty());
        std::fs::write(
            d.join("library.json"),
            r#"{"required_features":"slot_aliases"}"#,
        )
        .unwrap();
        assert!(
            required_by_package(&d).is_err(),
            "a malformed field is an error, never 'nothing'"
        );
        std::fs::remove_file(d.join("library.json")).unwrap();
        assert!(
            required_by_package(&d).is_err(),
            "a missing library.json is an error"
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_table_declares_it_in_its_header_line() {
        let d = tmp("table");
        let t = d.join("F.table.jsonl");
        std::fs::write(
            &t,
            "{\"type\":\"header\",\"schema\":\"table_dump.v1\",\"required_features\":[\"alias_equations\"]}\n{\"type\":\"rule\"}\n",
        )
        .unwrap();
        assert_eq!(
            required_by_table(&t).unwrap(),
            vec!["alias_equations".to_string()]
        );
        std::fs::write(&t, "{\"type\":\"rule\"}\n").unwrap();
        assert!(required_by_table(&t).is_err(), "no header line: an error");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn missing_names_exactly_the_unsupported_features() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(missing(&s(&["a", "b"]), &s(&["a"])).is_empty());
        assert_eq!(
            missing(&s(&["a"]), &s(&["b", "a", "b", "c"])),
            s(&["b", "c"])
        );
        assert_eq!(missing(&[], &s(&["slot_aliases"])), s(&["slot_aliases"]));
    }
}
