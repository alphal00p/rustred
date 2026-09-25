//! Opt-in independent root searches. Children share immutable inputs, never a
//! coverage ledger; each must discharge all of its own descendants.
mod artifact;
mod checkpoint;
mod config;
pub(super) mod events;
pub(super) mod monitor;
mod plan;
mod resources;
mod supervisor;

use super::error::CliError;
use std::{ffi::OsString, path::PathBuf};

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let mut config = None;
    let mut directory = None;
    let mut resume = false;
    let mut args = arguments.into_iter();
    while let Some(option) = args.next() {
        match option.to_str() {
            Some("--config") if config.is_none() => {
                config = Some(PathBuf::from(
                    args.next().ok_or_else(|| bad("--config requires a path"))?,
                ))
            }
            Some("--directory") if directory.is_none() => {
                directory = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| bad("--directory requires a path"))?,
                ))
            }
            Some("--resume") if !resume => resume = true,
            _ => {
                return Err(bad(
                    "expected --config PATH --directory PATH [--resume]; duplicate options are rejected",
                ));
            }
        }
    }
    supervisor::run(
        config.as_deref(),
        &directory.ok_or_else(|| bad("--directory is required"))?,
        resume,
    )
}

fn bad(message: impl Into<String>) -> CliError {
    CliError::Input(message.into())
}
fn io_error(error: impl std::fmt::Display) -> CliError {
    CliError::OutputIo(error.to_string())
}

pub(super) fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
