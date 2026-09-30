use super::*;
use crate::cli::args::{Command, parse_args};
use std::ffi::OsString;

fn args(values: &[&str]) -> FamilyCandidatesArgs {
    let Command::FamilyCandidates(args) = parse_args(
        ["rustred", "family-candidates"]
            .into_iter()
            .chain(values.iter().copied())
            .map(OsString::from),
    )
    .unwrap() else {
        panic!("generation expected")
    };
    args
}

#[test]
fn telemetry_preflight_refuses_every_protected_destination_before_reading() {
    for flag in [
        "--input",
        "--output",
        "--report-output",
        "--discovery-strategy",
        "--integral-order",
    ] {
        let value = args(&[
            flag,
            "telemetry-collision",
            "--progress-json",
            "telemetry-collision",
        ]);
        assert!(preflight_progress_path(&value).is_err(), "{flag}");
    }
    let value = args(&[
        "--checkpoint-dir",
        "telemetry-checkpoint",
        "--progress-json",
        "telemetry-checkpoint/latest.json",
    ]);
    assert!(preflight_progress_path(&value).is_err());
    let value = args(&[
        "--checkpoint-dir",
        "telemetry-checkpoint",
        "--progress-json",
        "telemetry-sibling.json",
    ]);
    assert!(preflight_progress_path(&value).is_ok());
}

#[test]
fn telemetry_cannot_create_a_file_at_a_needed_missing_ancestor() {
    for flag in [
        "--input",
        "--output",
        "--report-output",
        "--discovery-strategy",
        "--integral-order",
        "--checkpoint-dir",
    ] {
        let value = args(&[
            flag,
            "telemetry-new-ancestor/child",
            "--progress-json",
            "telemetry-new-ancestor",
        ]);
        assert!(preflight_progress_path(&value).is_err(), "{flag}");
    }
}

#[cfg(unix)]
#[test]
fn telemetry_preflight_resolves_existing_symlink_aliases() {
    let directory = std::env::temp_dir().join(format!(
        "rustred-progress-alias-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let input = directory.join("family.toml");
    let alias = directory.join("alias.json");
    std::fs::write(&input, "untouched").unwrap();
    std::os::unix::fs::symlink(&input, &alias).unwrap();
    let mut value = args(&[]);
    value.input = StreamPath::File(input.clone());
    value.progress_json = Some(alias);
    assert!(preflight_progress_path(&value).is_err());
    assert_eq!(std::fs::read_to_string(input).unwrap(), "untouched");
    std::fs::remove_dir_all(directory).unwrap();
}
