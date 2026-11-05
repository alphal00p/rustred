//! `rustred p-package` (owner: M2-T8b rr-package).
use std::ffi::OsString;

use crate::cli::error::CliError;

pub(super) fn run(_arguments: Vec<OsString>) -> Result<(), CliError> {
    super::not_implemented("p-package", "M2-T8b")
}
