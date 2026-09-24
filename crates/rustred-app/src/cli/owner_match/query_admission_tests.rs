use super::*;
use crate::cli::args::Command;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-query-admission-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn args(&self, bytes: Option<usize>, follow: bool) -> OwnerDomainMatchArgs {
        let mut raw = vec![
            "--manifest".into(),
            self.0.join("manifest.json").into_os_string(),
            "--queries".into(),
            self.0.join("queries.json").into_os_string(),
            "--output".into(),
            self.0.join("result.json").into_os_string(),
            "--events".into(),
            self.0.join("events.jsonl").into_os_string(),
            "--no-progress".into(),
        ];
        if let Some(bytes) = bytes {
            raw.extend([
                OsString::from("--max-query-bytes"),
                bytes.to_string().into(),
            ]);
        }
        if follow {
            raw.push("--follow-successors".into());
        }
        let Command::OwnerDomainMatch(args) = crate::cli::args::parse_args(
            [
                OsString::from("rustred"),
                OsString::from("owner-domain-match"),
            ]
            .into_iter()
            .chain(raw),
        )
        .unwrap() else {
            panic!("owner match")
        };
        args
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_query_reader_uses_explicit_byte_boundary_before_manifest_read() {
    for follow in [false, true] {
        let files = Files::new();
        let text = " ".repeat(1024 * 1024 + 1);
        std::fs::write(files.0.join("queries.json"), &text).unwrap();
        let default = run_admitted(files.args(None, follow)).unwrap_err();
        assert!(
            default.to_string().contains("1048576-byte CLI limit"),
            "{default}"
        );
        let one_over = run_admitted(files.args(Some(text.len() - 1), follow)).unwrap_err();
        assert!(one_over.to_string().contains("byte CLI limit"));
        let exact = run_admitted(files.args(Some(text.len()), follow)).unwrap_err();
        assert!(!exact.to_string().contains("byte CLI limit"), "{exact}");
        assert!(!files.0.join("result.json").exists());
    }
}

#[test]
fn cli_match_and_walk_error_receipts_preserve_requested_input_allowances() {
    for follow in [false, true] {
        let files = Files::new();
        std::fs::write(files.0.join("queries.json"), "{}").unwrap();
        std::fs::write(files.0.join("manifest.json"), "not JSON").unwrap();
        assert!(run_admitted(files.args(Some(2), follow)).is_err());
        let result: serde_json::Value =
            serde_json::from_slice(&std::fs::read(files.0.join("result.json")).unwrap()).unwrap();
        assert_eq!(result["requested_max_queries"], 256);
        assert_eq!(result["requested_max_query_bytes"], 2);
        assert_eq!(result["status"], "preparation_error");
        assert!(result.get("parsed_query_bytes").is_none());
    }
}
