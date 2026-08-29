use std::fs;
use std::path::PathBuf;

use super::support::successful_toml;

const EXPECTED_ROWS: [&str; 4] = [
    "ordinary-ibp:0:0",
    "ordinary-ibp:0:1",
    "ordinary-ibp:1:0",
    "ordinary-ibp:1:1",
];

#[test]
fn repository_cli_example_generates_the_complete_two_loop_source_set() {
    let Some(source) = repository_example_source() else {
        return;
    };
    let (_, document) = successful_toml(
        &[
            "derive",
            "--input-format",
            "symbolica",
            "--relations",
            "ordinary",
            "--n-cores",
            "1",
        ],
        &source,
    );

    assert_eq!(
        document["family"]["name"].as_str(),
        Some("equal_mass_sunset")
    );
    assert_eq!(
        document["family"]["loop_momenta"]
            .as_array()
            .expect("loop-momentum array")
            .iter()
            .map(|value| value.as_str().expect("loop-momentum string"))
            .collect::<Vec<_>>(),
        ["k1", "k2"]
    );
    assert_eq!(
        document["relation_counts"]["generated_ordinary"].as_integer(),
        Some(4)
    );
    assert_eq!(
        document["relation_counts"]["generated_li"].as_integer(),
        Some(0)
    );
    assert_eq!(
        document["relations"]
            .as_array()
            .expect("relation array")
            .iter()
            .map(|relation| relation["stable_id"].as_str().expect("stable row ID"))
            .collect::<Vec<_>>(),
        EXPECTED_ROWS
    );
}

fn repository_example_source() -> Option<String> {
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = repository_root.join("examples/cli/two_loop_single_mass_vacuum.symbolica");
    if path.is_file() {
        return Some(fs::read_to_string(path).expect("read the repository CLI example"));
    }
    assert!(
        !repository_root.join(".git").exists(),
        "Git checkout is missing the documented CLI example"
    );
    None
}
