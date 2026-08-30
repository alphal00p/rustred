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
