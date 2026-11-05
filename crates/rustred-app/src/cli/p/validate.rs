//! `rustred p-validate` (owner: M2-T8a rr-assemble).
use std::ffi::OsString;

use crate::cli::error::CliError;

pub(super) fn run(_arguments: Vec<OsString>) -> Result<(), CliError> {
    super::not_implemented("p-validate", "M2-T8a")
}
