//! Test gates: run one test alone in its own process, and make every
//! license-gated skip visible.
//!
//! Isolation: some tests observe process-global state that sibling tests mutate
//! concurrently: Symbolica's append-only symbol, variable-list and finite-field
//! tables, or file descriptors that a sibling's `fork` briefly duplicates.
//! Such a test calls `isolated(path)` first. In the parent test process it
//! re-executes this test binary filtered to exactly that test, requires that
//! exactly one test ran and passed, and returns `false` (the caller returns);
//! in the child it returns `true` and the caller runs its body with no
//! sibling test in the process.
use std::process::Command;

const VARIABLE: &str = "RUSTRED_ISOLATED_TEST";

/// `test` is the libtest name, e.g. `cli::shards::supervisor::tests::name`.
pub(crate) fn isolated(test: &str) -> bool {
    if std::env::var(VARIABLE).is_ok_and(|value| value == test) {
        return true;
    }
    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(VARIABLE, test)
        .output()
        .expect("isolated test process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("test result: ok. 1 passed"),
        "isolated test {test} failed ({}):\n{stdout}\n{stderr}",
        output.status
    );
    // Forward the child's skip markers past this process's output capture.
    for line in stderr.lines().filter(|line| line.starts_with("SKIPPED ")) {
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "{line}");
    }
    false
}

/// Opt-in strict mode for gates: with this variable set to `1`, a missing
/// license fails the gated test instead of skipping it.
const REQUIRE_LICENSE: &str = "RUSTRED_TESTS_REQUIRE_LICENSE";

/// A skip marker on the real stderr, bypassing libtest's output capture, so
/// it shows in every suite run (captured `eprintln!` output of a passing test
/// is discarded). The test then returns and passes vacuously.
pub(crate) fn skip(test: &str, reason: &str) {
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr(),
        "SKIPPED {test}: {reason}; the test exercised nothing and passes vacuously"
    );
}

/// Gate for tests that need licensed (multi-core) Symbolica: true when
/// licensed; otherwise a visible skip marker (or a failure under
/// `RUSTRED_TESTS_REQUIRE_LICENSE=1`) and false.
pub(crate) fn licensed_or_skip(test: &str) -> bool {
    if symbolica::license::LicenseManager::is_licensed() {
        return true;
    }
    assert!(
        std::env::var(REQUIRE_LICENSE).as_deref() != Ok("1"),
        "{test} requires SYMBOLICA_LICENSE ({REQUIRE_LICENSE}=1)"
    );
    skip(
        test,
        "SYMBOLICA_LICENSE absent (licensed Symbolica required)",
    );
    false
}

/// Gate for the `workers`-wide variant of a test: true when the host
/// core-budget preflight admits `workers` (always for one worker). Otherwise
/// false with a visible skip marker for `<test> W<workers>`: through
/// `licensed_or_skip` when the license is missing (so strict mode fails), or
/// with the preflight error (e.g. fewer CPUs in the affinity mask).
pub(crate) fn workers_or_skip(test: &str, workers: usize) -> bool {
    let Err(error) = rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers)
    else {
        return true;
    };
    let test = format!("{test} W{workers}");
    if licensed_or_skip(&test) {
        skip(&test, &format!("{error:?}"));
    }
    false
}
