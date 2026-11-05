//! `rustred p-census` (owner: M2-T2 rr-census).
use std::ffi::OsString;

use crate::cli::error::CliError;

pub(super) fn run(_arguments: Vec<OsString>) -> Result<(), CliError> {
    super::not_implemented("p-census", "M2-T2")
}
