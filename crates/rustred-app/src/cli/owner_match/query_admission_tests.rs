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
fn finite_replay_cli_budget_preflight_does_not_admit_inputs_or_create_checkpoint() {
    let files = Files::new();
    std::fs::write(files.0.join("queries.json"), "not JSON").unwrap();
    std::fs::write(files.0.join("manifest.json"), "not JSON").unwrap();
    let raw = [
        OsString::from("rustred"),
        "owner-domain-match".into(),
        "--manifest".into(),
        files.0.join("manifest.json").into_os_string(),
        "--queries".into(),
        files.0.join("queries.json").into_os_string(),
        "--output".into(),
        files.0.join("result.json").into_os_string(),
        "--checkpoint".into(),
        files.0.join("cp").into_os_string(),
    ];
    let flags = "--max-query-bytes 1024 --follow-successors --publication-policy epoch \
        --transfer-unreserved-lookahead 16 --unbounded-work --finite-replay-initial-domain \
        --finite-replay-budget-preflight --finite-replay-max-nodes 16000000 \
        --finite-replay-max-rule-applications 16000000 --finite-replay-max-transport-calls 16000000 \
        --finite-replay-max-transport-operations 1024000000 --finite-replay-max-transport-endpoints 128000000 \
        --finite-replay-max-coalescing-additions 256000000 --reduction-max-rule-applications 16000000 \
        --reduction-max-pending-frames 16000000 --reduction-max-coalescing-additions 256000000";
    let Command::OwnerDomainMatch(args) = crate::cli::args::parse_args(
        raw.into_iter()
            .chain(flags.split_whitespace().map(OsString::from)),
    )
    .unwrap() else {
        panic!("owner-domain-match")
    };
    let typed = walk_request(match_request(&args).unwrap(), &args);
    let expected = typed.finite_replay_budget_summary().unwrap();
    run(args).unwrap();
    let actual: Value =
        serde_json::from_slice(&std::fs::read(files.0.join("result.json")).unwrap()).unwrap();
    assert_eq!(actual["budget"], expected);
    assert_eq!(
        actual["budget"]["effective"]["max_rule_applications"],
        16_000_000
    );
    assert_eq!(
        actual["budget"]["effective"]["max_pending_frames"],
        16_000_000
    );
    assert_eq!(
        actual["budget"]["effective"]["max_coalescing_additions"],
        256_000_000
    );
    assert_eq!(actual["native_prepared"], false);
    assert!(!files.0.join("cp").exists());
    assert!(!files.0.join("events.jsonl").exists());
}

#[test]
fn finite_replay_cli_reduction_mapping_changes_only_three_existing_aggregate_fields() {
    let files = Files::new();
    let mut args = files.args(None, true);
    let mut request = OwnerDomainMatchRequest::new("{}".into(), "{}".into());
    let before = format!("{:?}", request.reduction_limits);
    set_reduction_aggregates(&mut request, &args);
    assert_eq!(before, format!("{:?}", request.reduction_limits));
    args.reduction_max_rule_applications = 0;
    args.reduction_max_pending_frames = 7;
    args.reduction_max_coalescing_additions = 9;
    args.unbounded_work = true;
    set_reduction_aggregates(&mut request, &args);
    let walk = walk_request(request, &args);
    assert_eq!(walk.matching.reduction_limits.max_rule_applications, 0);
    assert_eq!(walk.matching.reduction_limits.max_pending_frames, 7);
    assert_eq!(walk.matching.reduction_limits.max_coalescing_additions, 9);
    let mut restored = walk.matching.reduction_limits;
    let d = rustred::reduction::ReductionLimits::default();
    restored.max_rule_applications = d.max_rule_applications;
    restored.max_pending_frames = d.max_pending_frames;
    restored.max_coalescing_additions = d.max_coalescing_additions;
    assert_eq!(before, format!("{restored:?}"));
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

#[test]
fn unbounded_walk_receipt_reports_effective_refinement_allowance() {
    let files = Files::new();
    std::fs::write(files.0.join("queries.json"), "{}").unwrap();
    std::fs::write(files.0.join("manifest.json"), "not JSON").unwrap();
    let mut args = files.args(Some(2), true);
    args.unbounded_work = true;
    assert_eq!(args.max_bounded_refinement_cells, 0);
    assert!(run_admitted(args).is_err());
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(files.0.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["requested_unbounded_work"], true);
    assert_eq!(result["max_bounded_refinement_cells"], json!(usize::MAX));
    assert_eq!(result["status"], "preparation_error");
}
