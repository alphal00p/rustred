use super::*;
use crate::cli::args::{Command, parse_args};
use std::ffi::OsString;
use std::path::PathBuf;

fn args() -> OwnerDomainMatchArgs {
    let Command::OwnerDomainMatch(args) = parse_args(
        "rustred owner-domain-match --manifest manifest --queries queries \
         --output output --events events --stop-file stop --owner-base . \
         --follow-successors --checkpoint checkpoint-test-dir"
            .split_whitespace()
            .map(OsString::from),
    )
    .unwrap() else {
        panic!("owner-domain command")
    };
    args
}

#[test]
fn dedicated_checkpoint_directory_is_disjoint_from_other_streams() {
    let original = args();
    preflight_checkpoint_paths(&original).unwrap();
    for which in 0..7 {
        let mut changed = original.clone();
        let path = PathBuf::from("checkpoint-test-dir/stream");
        match which {
            0 => changed.manifest = path,
            1 => changed.queries = path,
            2 => changed.output = path,
            3 => changed.events = Some(path),
            4 => changed.stop_file = Some(path),
            5 => changed.owner_base = path,
            _ => changed.output = "checkpoint-test-dir".into(),
        }
        assert!(
            preflight_checkpoint_paths(&changed).is_err(),
            "stream {which}"
        );
    }
}

#[test]
fn path_normalization_cannot_bypass_checkpoint_separation() {
    let mut changed = args();
    changed.output = "checkpoint-test-dir/nested/../output".into();
    assert!(preflight_checkpoint_paths(&changed).is_err());
    changed.output = "checkpoint-test-dir/../separate-output".into();
    preflight_checkpoint_paths(&changed).unwrap();
}
