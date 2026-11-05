//! `rustred p-basis` (owner: M2-T9 rr-basis).
use std::ffi::OsString;

use crate::cli::error::CliError;

pub(super) fn run(_arguments: Vec<OsString>) -> Result<(), CliError> {
    super::not_implemented("p-basis", "M2-T9")
}
