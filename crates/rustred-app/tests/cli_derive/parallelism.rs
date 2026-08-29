use super::support::*;

#[test]
fn licensed_available_core_widths_through_four_are_byte_identical() {
    let serial_arguments = [
        "derive",
        "--input-format",
        "symbolica",
        "--relations",
        "all",
        "--n-cores",
        "1",
    ];
    let (serial, serial_document) = successful_toml(&serial_arguments, ONE_LOOP_TWO_EXTERNALS);
    let available = std::thread::available_parallelism().unwrap().get();
    for n_cores in [2_usize, 3, 4]
        .into_iter()
        .filter(|width| *width <= available)
    {
        let n_cores = n_cores.to_string();
        let parallel_arguments = [
            "derive",
            "--input-format",
            "symbolica",
            "--relations",
            "all",
            "--n-cores",
            n_cores.as_str(),
        ];
        let (parallel, parallel_document) =
            successful_toml(&parallel_arguments, ONE_LOOP_TWO_EXTERNALS);
        assert_eq!(
            parallel.stdout, serial.stdout,
            "--n-cores {n_cores} changed the canonical derive output"
        );
        assert_eq!(parallel_document, serial_document);
    }
}

#[test]
fn rayon_global_environment_cannot_override_explicit_n_cores() {
    let requested = std::thread::available_parallelism().unwrap().get().min(4);
    if requested < 2 {
        return;
    }
    let requested = requested.to_string();
    let serial_arguments = ["derive", "--input-format", "symbolica", "--n-cores", "1"];
    let parallel_arguments = [
        "derive",
        "--input-format",
        "symbolica",
        "--n-cores",
        requested.as_str(),
    ];
    let (serial, serial_document) = successful_toml_with_environment(
        &serial_arguments,
        ONE_LOOP_TWO_EXTERNALS,
        &[("RAYON_NUM_THREADS", "32")],
    );
    let (parallel, parallel_document) = successful_toml_with_environment(
        &parallel_arguments,
        ONE_LOOP_TWO_EXTERNALS,
        &[("RAYON_NUM_THREADS", "1")],
    );
    assert_eq!(parallel.stdout, serial.stdout);
    assert_eq!(parallel_document, serial_document);

    // The global Rayon setting must not silently downgrade the explicit CLI
    // request to one core. Removing only the process-local Symbolica license
    // therefore still makes the multicore request fail its license policy.
    let unlicensed = rustred_with_environment(
        &parallel_arguments,
        ONE_LOOP_TWO_EXTERNALS,
        &[("RAYON_NUM_THREADS", "1")],
        true,
    );
    assert_eq!(unlicensed.status.code(), Some(8));
    assert!(unlicensed.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&unlicensed.stderr);
    assert!(stderr.starts_with("rustred: execution:"), "{stderr}");
    assert!(
        stderr.contains(&format!("n_cores {requested} requires a Symbolica license")),
        "{stderr}"
    );
}

#[test]
fn n_cores_rejects_zero_missing_duplicate_and_malformed_values() {
    assert_usage_error(
        &["derive", "--n-cores", "0"],
        "invalid value \"0\" for --n-cores; expected a positive integer",
    );
    assert_usage_error(&["derive", "--n-cores"], "option --n-cores needs a value");
    assert_usage_error(
        &["derive", "--n-cores", "2", "--n-cores", "3"],
        "option --n-cores was supplied twice",
    );
    assert_usage_error(
        &["derive", "--n-cores", "many"],
        "invalid value \"many\" for --n-cores; expected a positive integer",
    );
}
