use super::*;
use crate::cli::args::parse_args;

fn args(values: &[&str]) -> Result<WalkInventoryArgs, ArgError> {
    match parse_args(
        ["rustred", "walk-inventory"]
            .into_iter()
            .chain(values.iter().copied())
            .map(OsString::from),
    )? {
        Command::WalkInventory(args) => Ok(args),
        _ => panic!("inventory command expected"),
    }
}

#[test]
fn parses_exclusive_sources_bounded_pages_and_stdout() {
    let command = args(&["--command", "request.json"]).unwrap();
    assert_eq!(command.threads, 1);
    assert_eq!(command.page_size, 25);
    assert_eq!(command.output, StreamPath::Stdio);
    assert!(!command.normalize_terminals);
    let campaign = args(&[
        "--campaign-directory",
        "campaign",
        "--threads",
        "4",
        "--normalize-terminals",
        "--rules-start",
        "2",
        "--terminals-start",
        "8",
        "--normalized-terminals-start",
        "1",
        "--page-size",
        "1000",
        "--output",
        "-",
        "--force",
    ])
    .unwrap();
    assert!(campaign.normalize_terminals && campaign.force);
    assert_eq!(campaign.normalized_terminals_start, 1);
    assert_eq!(
        (
            campaign.rules_start,
            campaign.terminals_start,
            campaign.page_size
        ),
        (2, 8, 1000)
    );
    assert!(args(&[]).is_err());
    assert!(args(&["--command", "a", "--campaign-directory", "b"]).is_err());
    for invalid in ["0", "1001", "-1", "arbitrary"] {
        assert!(args(&["--command", "a", "--page-size", invalid]).is_err());
    }
    assert!(args(&["--command", "a", "--threads", "0"]).is_err());
    assert!(args(&["--command", "a", "--rules-start", "-1"]).is_err());
    assert!(args(&["--command", "a", "--normalized-terminals-start", "1"]).is_err());
    assert!(matches!(
        args(&["--command", "a", "--command", "b"]),
        Err(ArgError::DuplicateOption("--command"))
    ));
    assert!(matches!(
        args(&["--command", "a", "--force", "--force"]),
        Err(ArgError::DuplicateOption("--force"))
    ));
}

#[test]
fn incomplete_inventory_never_reports_success() {
    assert!(inventory_verdict(&json!({"complete":true,"verification":{"verdict":"PASS"}})).is_ok());
    for complete in [false, true] {
        let error = inventory_verdict(
            &json!({"complete":complete,"verification":{"verdict":"INCOMPLETE"}}),
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 9);
        let error =
            inventory_verdict(&json!({"complete":complete,"verification":{"verdict":"FAIL"}}))
                .unwrap_err();
        assert_eq!(error.exit_code(), 1);
    }
    assert!(
        inventory_verdict(&json!({"complete":false,"verification":{"verdict":"PASS"}})).is_err()
    );
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../TMP");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "walk-inventory-cli-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn campaign_source_selects_published_run_without_executing_or_selecting_newest() {
    let directory = Directory::new();
    let old = directory.0.join("runs/first");
    let newer = directory.0.join("runs/later");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&newer).unwrap();
    std::fs::write(old.join("request.json"), "[]").unwrap();
    std::fs::write(newer.join("request.json"), "[]").unwrap();
    std::fs::write(
        directory.0.join("active-run.json"),
        r#"{"run_directory":"/previous/location/runs/first","command":["must-not-execute"]}"#,
    )
    .unwrap();
    assert_eq!(
        campaign_command(&directory.0).unwrap(),
        old.join("request.json").canonicalize().unwrap()
    );
    std::fs::write(directory.0.join("active-run.json"), "{}").unwrap();
    assert!(campaign_command(&directory.0).is_err());
}

#[test]
fn force_cannot_overwrite_checkpoint_or_input_payloads() {
    let directory = Directory::new();
    let checkpoint = directory.0.join("checkpoint");
    let inputs = directory.0.join("inputs");
    std::fs::create_dir(&checkpoint).unwrap();
    std::fs::create_dir(&inputs).unwrap();
    let original = checkpoint.join("latest.json");
    std::fs::write(&original, "immutable").unwrap();
    let protected = [checkpoint, inputs.clone()];
    assert!(protect_output(&StreamPath::File(original.clone()), &protected).is_err());
    assert!(protect_output(&StreamPath::File(inputs.join("new-owner.rr")), &protected).is_err());
    assert!(
        protect_output(
            &StreamPath::File(directory.0.join("report.json")),
            &protected
        )
        .is_ok()
    );
    assert!(protect_output(&StreamPath::Stdio, &protected).is_ok());
    assert_eq!(std::fs::read_to_string(original).unwrap(), "immutable");
    #[cfg(unix)]
    {
        let alias = directory.0.join("input-alias");
        std::os::unix::fs::symlink(&inputs, &alias).unwrap();
        assert!(protect_output(&StreamPath::File(alias.join("new.rr")), &protected).is_err());
    }
}

#[test]
fn explicit_command_protects_external_input_documents_and_amendments() {
    let directory = Directory::new();
    for name in ["request", "documents", "owners", "checkpoint", "output"] {
        std::fs::create_dir(directory.0.join(name)).unwrap();
    }
    let document = |name| directory.0.join("documents").join(name);
    let manifest = document("selection.json");
    let queries = document("queries.json");
    let amendment = document("amendment.json");
    let executable = document("frozen-rustred");
    let absent_stop = document("request.stop");
    for path in [&manifest, &queries, &amendment, &executable] {
        std::fs::write(path, "immutable").unwrap();
    }
    let argv = vec![
        executable.as_os_str().to_owned(),
        "owner-domain-match".into(),
        "--manifest".into(),
        manifest.as_os_str().to_owned(),
        "--queries".into(),
        queries.as_os_str().to_owned(),
        "--output".into(),
        directory.0.join("output/result.json").into_os_string(),
        "--owner-base".into(),
        directory.0.join("owners").into_os_string(),
        "--follow-successors".into(),
        "--resume".into(),
        directory.0.join("checkpoint").into_os_string(),
        "--amend-queries".into(),
        amendment.as_os_str().to_owned(),
        "--stop-file".into(),
        absent_stop.as_os_str().to_owned(),
    ];
    let protected = protected_command_paths(&argv).unwrap();
    assert!(!absent_stop.exists());
    assert!(protect_output(&StreamPath::File(absent_stop.clone()), &protected).is_err());
    assert!(!absent_stop.exists());
    assert!(
        protect_output(
            &StreamPath::File(directory.0.join("checkpoint/latest.json")),
            &protected
        )
        .is_err()
    );
    for path in [&manifest, &queries, &amendment, &executable] {
        assert!(protect_output(&StreamPath::File(path.clone()), &protected).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "immutable");
    }
    assert!(
        protect_output(
            &StreamPath::File(directory.0.join("report.json")),
            &protected
        )
        .is_ok()
    );
}

#[cfg(unix)]
#[test]
fn campaign_request_symlink_cannot_escape_saved_runs() {
    let directory = Directory::new();
    let outside = directory.0.join("outside");
    let runs = directory.0.join("runs");
    std::fs::create_dir(&outside).unwrap();
    std::fs::create_dir(&runs).unwrap();
    std::fs::write(outside.join("request.json"), "[]").unwrap();
    std::os::unix::fs::symlink(&outside, runs.join("escape")).unwrap();
    std::fs::write(
        directory.0.join("active-run.json"),
        r#"{"run_directory":"runs/escape"}"#,
    )
    .unwrap();
    assert!(campaign_command(&directory.0).is_err());
}
