use std::fs;
use std::path::PathBuf;

#[test]
fn repository_cli_example_uses_the_closed_two_loop_artifact_workflow() {
    let Some(script) = repository_example_script() else {
        return;
    };

    assert!(script.contains("campaign generate \\\n  --family unit-mass-vacuum-k3"));
    assert!(script.contains("campaign inspect \\\n  --artifact \"$artifact\""));
    assert!(script.contains("campaign reduce \\\n  --artifact \"$artifact\" \\\n  --powers 2,2,1"));
    assert!(!script.contains(" derive "));
    assert!(!script.contains(".symbolica"));
}

#[test]
fn repository_cli_example_exposes_the_completed_k6_artifact_consumer() {
    let Some(script) = repository_k6_example_script() else {
        return;
    };

    assert!(script.contains("\"$rustred_bin\" family-close \\\n"));
    assert!(script.contains("examples/input/three_loop_k6.toml"));
    assert!(!script.contains("spired-generate-k6"));
    assert!(script.contains("campaign inspect --artifact \"$artifact\""));
    assert!(script.contains("campaign reduce --artifact \"$artifact\""));
    assert!(script.contains("--powers 2,1,1,1,1,1"));
    assert!(!script.contains("RUSTRED_K6_GENERATOR"));
    assert!(script.contains("RUSTRED_BIN"));
}

fn repository_example_script() -> Option<String> {
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = repository_root.join("examples/cli/run.sh");
    if path.is_file() {
        return Some(fs::read_to_string(path).expect("read the repository CLI example"));
    }
    assert!(
        !repository_root.join(".git").exists(),
        "Git checkout is missing the documented CLI example"
    );
    None
}

fn repository_k6_example_script() -> Option<String> {
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = repository_root.join("examples/cli/run_k6_closing_artifact.sh");
    if path.is_file() {
        return Some(fs::read_to_string(path).expect("read the repository K6 CLI example"));
    }
    assert!(
        !repository_root.join(".git").exists(),
        "Git checkout is missing the documented K6 CLI example"
    );
    None
}
