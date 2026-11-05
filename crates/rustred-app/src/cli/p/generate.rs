//! `rustred p-generate` (owner: M2-T7c rr-custom).
use std::ffi::OsString;

use crate::cli::error::CliError;

pub(super) fn run(_arguments: Vec<OsString>) -> Result<(), CliError> {
    super::not_implemented("p-generate", "M2-T7c")
}
