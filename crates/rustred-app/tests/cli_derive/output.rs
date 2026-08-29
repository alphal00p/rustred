use super::support::*;

#[test]
fn file_output_is_atomic_and_requires_force_to_replace() {
    let directory = unique_test_directory();
    std::fs::create_dir(&directory).expect("create isolated CLI test directory");
    let destination = directory.join("relations.toml");
    std::fs::write(&destination, b"sentinel\n").expect("seed destination");
    let path = destination.to_str().expect("UTF-8 test path");

    let refused = rustred(
        &["derive", "--input-format", "symbolica", "--output", path],
        ONE_LOOP,
    );
    assert_eq!(refused.status.code(), Some(7));
    assert!(refused.stdout.is_empty());
    assert_eq!(std::fs::read(&destination).unwrap(), b"sentinel\n");

    let replaced = rustred(
        &[
            "derive",
            "--input-format",
            "symbolica",
            "--output",
            path,
            "--force",
        ],
        ONE_LOOP,
    );
    assert_eq!(
        replaced.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&replaced.stderr)
    );
    assert!(replaced.stdout.is_empty());
    let installed = std::fs::read_to_string(&destination).expect("installed output");
    let document: toml::Value = toml::from_str(&installed).expect("installed TOML");
    assert_eq!(document["status"].as_str(), Some("ok"));

    std::fs::remove_file(&destination).expect("remove isolated output");
    std::fs::remove_dir(&directory).expect("remove isolated test directory");
}
