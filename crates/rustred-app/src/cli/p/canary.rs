//! `rustred p-canary` (owner: M2-T10b rr-canary).
use std::ffi::OsString;

use crate::cli::error::CliError;

pub(super) fn run(_arguments: Vec<OsString>) -> Result<(), CliError> {
    super::not_implemented("p-canary", "M2-T10b")
}
