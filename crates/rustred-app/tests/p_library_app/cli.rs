//! The `rustred p-*` commands (owner: M2-T8b rr-package): `rustred p-package`.
use std::path::Path;
use std::process::{Command, Output};

use rustred::propagator::error::PError;
use rustred_app::propagator::{
    PLibraryOptions, PLibrarySource, equivalent_p_libraries, load_p_library,
};

use super::package::{Scratch, scope_three, scope_two, tree};

fn p_package_command(arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rustred"));
    command.arg("p-package").args(arguments);
    command
}

fn p_package(arguments: &[&str]) -> Output {
    p_package_command(arguments).output().unwrap()
}

fn succeeded(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

fn path(directory: &Path) -> &str {
    directory.to_str().unwrap()
}

/// Two `rustred p-package` processes write byte-identical trees; the package
/// loads (with the descent certificate) as the program assembled here; a
/// third run reusing the first package's Custom payloads writes the same tree.
#[test]
fn p_package_writes_byte_identical_packages_of_the_assembled_program() {
    let scratch = Scratch::new("cli");
    let (first, second, reused) = (
        scratch.join("first"),
        scratch.join("second"),
        scratch.join("reused"),
    );
    let stdout = succeeded(&p_package(&["--max-loops", "2", "--out", path(&first)]));
    print!("{stdout}");
    assert!(
        stdout.starts_with(&scope_two().library.identity()),
        "{stdout}"
    );
    assert!(stdout.contains("total,"), "{stdout}");
    assert!(stdout.contains("the same program"), "{stdout}");
    succeeded(&p_package(&["--max-loops", "2", "--out", path(&second)]));
    succeeded(&p_package(&[
        "--max-loops",
        "2",
        "--out",
        path(&reused),
        "--reuse-custom",
        path(&first),
    ]));
    let tree_first = tree(&first);
    assert!(tree_first.contains_key("library.json"));
    assert_eq!(tree(&second), tree_first, "two runs differ");
    assert_eq!(tree(&reused), tree_first, "the reusing run differs");
    // p-package attaches the basis layer by itself: at scope 1-2 the Unit row
    // alone, no master.
    let layer: serde_json::Value =
        serde_json::from_slice(&tree_first["layer/p-layer.json"]).unwrap();
    assert_eq!(layer["rows"].as_array().unwrap().len(), 1);
    assert_eq!(layer["masters"], serde_json::json!([]));

    let loaded = load_p_library(
        PLibrarySource::Directory(first),
        &PLibraryOptions {
            verify_descent: true,
        },
    )
    .unwrap();
    assert!(equivalent_p_libraries(&scope_two().library, &loaded).unwrap());
}

/// Usage errors exit 2; a non-empty output directory and an unreadable
/// `--reuse-custom` package are errors of the run (exit 5), nothing written.
#[test]
fn p_package_refuses_bad_arguments_and_occupied_directories() {
    let scratch = Scratch::new("cli-refusals");
    assert_eq!(p_package(&["--max-loops", "2"]).status.code(), Some(2));
    assert_eq!(p_package(&["--out", "x"]).status.code(), Some(2));
    let occupied = scratch.join("occupied");
    std::fs::create_dir_all(&occupied).unwrap();
    std::fs::write(occupied.join("keep"), b"x").unwrap();
    let output = p_package(&["--max-loops", "1", "--out", path(&occupied)]);
    assert_eq!(output.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&output.stderr).contains("non-empty"));
    assert_eq!(std::fs::read(occupied.join("keep")).unwrap(), b"x");
    let output = p_package(&[
        "--max-loops",
        "1",
        "--out",
        path(&scratch.join("never")),
        "--reuse-custom",
        path(&scratch.join("absent")),
    ]);
    assert_eq!(output.status.code(), Some(5));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing file library.json"), "{stderr}");
    assert!(!scratch.join("never").exists());
    // The error the command reports is the library's own.
    let error = load_p_library(
        PLibrarySource::Directory(scratch.join("absent")),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .unwrap_err();
    assert!(matches!(error, PError::MalformedLibrary(_)));
}

/// Scope 1-3, the built-in library: two `rustred p-package --max-loops 3`
/// processes (run side by side) write byte-identical trees, which load with
/// the certificate as the program assembled here; a third run reusing the
/// first package's Custom payloads (NO, t1star05) writes the same bytes.
#[test]
#[ignore = "slow: three scope 1-3 p-package runs, each generating or reusing NO and t1star05"]
fn p_package_writes_byte_identical_scope_one_to_three_packages() {
    let scratch = Scratch::new("cli-three");
    let (first, second, reused) = (
        scratch.join("first"),
        scratch.join("second"),
        scratch.join("reused"),
    );
    let start = std::time::Instant::now();
    let runs: Vec<_> = [&first, &second]
        .into_iter()
        .map(|out| {
            p_package_command(&["--max-loops", "3", "--out", path(out)])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let outputs: Vec<Output> = (runs.into_iter())
        .map(|run| run.wait_with_output().unwrap())
        .collect();
    let generated_seconds = start.elapsed().as_secs_f64();
    let stdout = succeeded(&outputs[0]);
    print!("{stdout}");
    succeeded(&outputs[1]);
    let start = std::time::Instant::now();
    let stdout_reused = succeeded(&p_package(&[
        "--max-loops",
        "3",
        "--out",
        path(&reused),
        "--reuse-custom",
        path(&first),
    ]));
    let reused_seconds = start.elapsed().as_secs_f64();
    print!("{stdout_reused}");
    println!(
        "two generating runs side by side {generated_seconds:.1} s, the reusing run {reused_seconds:.1} s"
    );

    let tree_first = tree(&first);
    assert_eq!(tree(&second), tree_first, "two runs differ");
    assert_eq!(tree(&reused), tree_first, "the reusing run differs");
    // p-package attaches the basis layer by itself: three rows (Unit, NO,
    // t1star05) onto no and t105.
    let layer: serde_json::Value =
        serde_json::from_slice(&tree_first["layer/p-layer.json"]).unwrap();
    assert_eq!(layer["rows"].as_array().unwrap().len(), 3);
    assert_eq!(layer["masters"], serde_json::json!(["no", "t105"]));

    let verify = PLibraryOptions {
        verify_descent: true,
    };
    let loaded = load_p_library(PLibrarySource::Directory(first), &verify).unwrap();
    let loaded_reused = load_p_library(PLibrarySource::Directory(reused), &verify).unwrap();
    let assembled = scope_three();
    assert!(
        stdout.starts_with(&assembled.library.identity()),
        "{stdout}"
    );
    assert!(loaded.basis_map().is_some());
    assert!(equivalent_p_libraries(&assembled.library, &loaded).unwrap());
    assert!(equivalent_p_libraries(&loaded, &loaded_reused).unwrap());
}
