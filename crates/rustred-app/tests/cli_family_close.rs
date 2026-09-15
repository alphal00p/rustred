use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const INPUT: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "cli_user_family"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
numerator = "1"
"#;

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rustred-family-close-test-{}-{nonce}",
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

fn run(arguments: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(arguments)
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn success(arguments: &[&str], input: &[u8]) -> Vec<u8> {
    let output = run(arguments, input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output.stdout
}

#[test]
fn external_input_generates_artifact_for_fresh_process_inspection_and_reduction() {
    let bytes = success(
        &[
            "family-close",
            "--input-format",
            "toml",
            "--permutation",
            "0",
        ],
        INPUT.as_bytes(),
    );
    assert!(!bytes.is_empty());
    let inspection = success(&["campaign", "inspect", "--artifact", "-"], &bytes);
    let inspection: toml::Value =
        toml::from_str(std::str::from_utf8(&inspection).unwrap()).unwrap();
    assert_eq!(inspection["artifact"]["arity"].as_integer(), Some(1));
    let reduction = success(
        &["campaign", "reduce", "--artifact", "-", "--powers", "3"],
        &bytes,
    );
    let reduction: toml::Value = toml::from_str(std::str::from_utf8(&reduction).unwrap()).unwrap();
    assert_eq!(reduction["status"].as_str(), Some("reduced"));
}

#[test]
fn existing_artifact_destination_is_preserved_before_semantic_work() {
    let directory = Directory::new();
    let path = directory.0.join("family.rr");
    std::fs::write(&path, b"preserve-me").unwrap();
    let output = run(
        &["family-close", "--output", path.to_str().unwrap()],
        b"invalid family input",
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert_eq!(std::fs::read(path).unwrap(), b"preserve-me");
}

#[test]
fn rejected_family_leaves_no_partial_artifact() {
    let directory = Directory::new();
    let path = directory.0.join("rejected.rr");
    let output = run(
        &["family-close", "--output", path.to_str().unwrap()],
        INPUT.replace("q^2-1", "q^2-2").as_bytes(),
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!path.exists());
}
