//! `rustred p-*`: the p-library commands (plan M2 section 4). Each subcommand
//! lives in its own file, owned by its M2 task; this dispatch is M2-I0's.
use std::ffi::OsString;
use std::io::Write;

use super::error::CliError;

mod basis;
mod canary;
mod census;
mod generate;
mod package;
mod validate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PSubcommand {
    Census,
    Generate,
    Basis,
    Package,
    Validate,
    Canary,
}

pub(super) fn run(subcommand: PSubcommand, arguments: Vec<OsString>) -> Result<(), CliError> {
    match subcommand {
        PSubcommand::Census => census::run(arguments),
        PSubcommand::Generate => generate::run(arguments),
        PSubcommand::Basis => basis::run(arguments),
        PSubcommand::Package => package::run(arguments),
        PSubcommand::Validate => validate::run(arguments),
        PSubcommand::Canary => canary::run(arguments),
    }
}

/// A subcommand whose owner task has not landed: the message on stderr and
/// exit status 2, nothing else.
fn not_implemented(command: &str, task: &str) -> Result<(), CliError> {
    let _ = writeln!(
        std::io::stderr().lock(),
        "rustred {command}: not implemented in this build ({task})"
    );
    std::process::exit(2)
}
