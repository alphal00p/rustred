//! MISSING TEST INPUTS (lane int8, 2026-11-18). A clean-up deleted three /common inputs of #[ignore]d real-data tests, and
//! those tests (with ~100 other sites) printed "SKIP" into captured output and passed: a SILENT pass. Now:
//! - [`missing`]: an absent input (data file, fixture, binary, licence) FAILS the test. Only the explicit
//!   `RUSTRED_MISSING_INPUTS=skip` (a host without the data) turns it into an ANNOUNCED skip.
//! - [`optional`]: an input that is optional BY DESIGN (an environment precondition such as the private pid namespace of
//!   rule L10): always an announced skip.
//!
//! An announced skip writes one line `SKIP-INPUT <test>: <what>` to the REAL stdout, which the test harness does not
//! capture: every suite log shows it, and the suite summary counts the lines.
#![allow(dead_code)]

use std::io::Write;

fn announce(what: std::fmt::Arguments) {
    let t = std::thread::current();
    let name = t.name().unwrap_or("?").to_string();
    let mut o = std::io::stdout();
    let _ = writeln!(o, "SKIP-INPUT {name}: {what}");
    let _ = o.flush();
}

/// An absent input: FAILS (panic), or an announced skip under the explicit `RUSTRED_MISSING_INPUTS=skip`.
pub fn missing(what: std::fmt::Arguments) {
    if std::env::var("RUSTRED_MISSING_INPUTS").as_deref() == Ok("skip") {
        announce(what);
    } else {
        panic!(
            "MISSING TEST INPUT: {what} (an absent input fails; RUSTRED_MISSING_INPUTS=skip announces a skip instead)"
        );
    }
}

/// An input that is optional BY DESIGN (environment precondition): an announced skip.
pub fn optional(what: std::fmt::Arguments) {
    announce(what);
}
