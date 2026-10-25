//! `rustred saved-application compose-layer | layer-base` (RR-11).
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;

use super::args::{ArgError, Command, next_utf8_value, next_value, parse_positive_integer};
use super::error::CliError;
use crate::{SavedApplicationComposeOptions, compose_saved_application_layer};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SavedApplicationArgs {
    ComposeLayer {
        application: PathBuf,
        layer: PathBuf,
        out: PathBuf,
        terminal_definitions: Option<PathBuf>,
        terminal_definitions_out: Option<PathBuf>,
        size_gate: f64,
        consumer_limits: String,
        fill_unreduced: bool,
        allow_factorized_sources: bool,
        minimality_reference: Option<PathBuf>,
        minimality_reference_reviewed: bool,
        check_values_out: Option<PathBuf>,
        layered: bool,
        dry_run: bool,
        workers: usize,
        self_check_points: usize,
    },
    LayerBase {
        application: PathBuf,
    },
    IdentityLayer {
        application: PathBuf,
        out: PathBuf,
        workers: usize,
    },
}

// The only float, the size gate, is parsed finite and nonnegative.
impl Eq for SavedApplicationArgs {}

pub(super) fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let Some(sub) = args.next() else {
        return Err(ArgError::MissingSubcommand("saved-application"));
    };
    let sub = sub.into_string().map_err(ArgError::NonUtf8Option)?;
    let mut seen = BTreeSet::new();
    let mut application = None;
    let mut layer = None;
    let mut out = None;
    let mut terminal_definitions = None;
    let mut terminal_definitions_out = None;
    let mut size_gate = 2.0;
    let mut consumer_limits = String::from("vakint-pin-52c74636");
    let mut fill_unreduced = false;
    let mut allow_factorized_sources = false;
    let mut minimality_reference = None;
    let mut minimality_reference_reviewed = false;
    let mut check_values_out = None;
    let mut layered = false;
    let mut dry_run = false;
    let mut workers = 1;
    let mut self_check_points = 3;
    while let Some(arg) = args.next() {
        let arg = arg.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match arg.as_str() {
            "--application" => "--application",
            "--layer" if sub == "compose-layer" => "--layer",
            "--out" if sub == "compose-layer" || sub == "identity-layer" => "--out",
            "--terminal-definitions" if sub == "compose-layer" => "--terminal-definitions",
            "--terminal-definitions-out" if sub == "compose-layer" => "--terminal-definitions-out",
            "--size-gate" if sub == "compose-layer" => "--size-gate",
            "--consumer-limits" if sub == "compose-layer" => "--consumer-limits",
            "--fill-unreduced" if sub == "compose-layer" => "--fill-unreduced",
            "--allow-factorized-sources" if sub == "compose-layer" => {
                "--allow-factorized-sources"
            }
            "--minimality-reference" if sub == "compose-layer" => "--minimality-reference",
            "--minimality-reference-reviewed" if sub == "compose-layer" => {
                "--minimality-reference-reviewed"
            }
            "--check-values-out" if sub == "compose-layer" => "--check-values-out",
            "--layered" if sub == "compose-layer" => "--layered",
            "--dry-run" if sub == "compose-layer" => "--dry-run",
            "--workers" if sub == "compose-layer" || sub == "identity-layer" => "--workers",
            "--self-check-points" if sub == "compose-layer" => "--self-check-points",
            _ => return Err(ArgError::UnknownOption(arg)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--application" => application = Some(PathBuf::from(next_value(&mut args, option)?)),
            "--layer" => layer = Some(PathBuf::from(next_value(&mut args, option)?)),
            "--out" => out = Some(PathBuf::from(next_value(&mut args, option)?)),
            "--terminal-definitions" => {
                terminal_definitions = Some(PathBuf::from(next_value(&mut args, option)?))
            }
            "--terminal-definitions-out" => {
                terminal_definitions_out = Some(PathBuf::from(next_value(&mut args, option)?))
            }
            "--size-gate" => {
                let value = next_utf8_value(&mut args, option)?;
                size_gate = value
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .ok_or(ArgError::InvalidValue {
                        option,
                        value,
                        expected: "a nonnegative ratio",
                    })?;
            }
            "--consumer-limits" => {
                let value = next_utf8_value(&mut args, option)?;
                if crate::SavedApplicationConsumerLimits::parse(&value).is_none() {
                    return Err(ArgError::InvalidValue {
                        option,
                        value,
                        expected: "vakint-pin-52c74636 or none",
                    });
                }
                consumer_limits = value;
            }
            "--fill-unreduced" => fill_unreduced = true,
            "--allow-factorized-sources" => allow_factorized_sources = true,
            "--minimality-reference" => {
                minimality_reference = Some(PathBuf::from(next_value(&mut args, option)?))
            }
            "--minimality-reference-reviewed" => minimality_reference_reviewed = true,
            "--check-values-out" => {
                check_values_out = Some(PathBuf::from(next_value(&mut args, option)?))
            }
            "--layered" => layered = true,
            "--dry-run" => dry_run = true,
            "--workers" => {
                workers = parse_positive_integer(option, next_utf8_value(&mut args, option)?)?
            }
            _ => {
                self_check_points =
                    parse_positive_integer(option, next_utf8_value(&mut args, option)?)?
            }
        }
    }
    let application = application.ok_or(ArgError::MissingRequiredOption("--application"))?;
    match sub.as_str() {
        "compose-layer" => Ok(Command::SavedApplication(
            SavedApplicationArgs::ComposeLayer {
                application,
                layer: layer.ok_or(ArgError::MissingRequiredOption("--layer"))?,
                out: out.ok_or(ArgError::MissingRequiredOption("--out"))?,
                terminal_definitions,
                terminal_definitions_out,
                size_gate,
                consumer_limits,
                fill_unreduced,
                allow_factorized_sources,
                minimality_reference,
                minimality_reference_reviewed,
                check_values_out,
                layered,
                dry_run,
                workers,
                self_check_points,
            },
        )),
        "layer-base" => Ok(Command::SavedApplication(SavedApplicationArgs::LayerBase {
            application,
        })),
        "identity-layer" => Ok(Command::SavedApplication(
            SavedApplicationArgs::IdentityLayer {
                application,
                out: out.ok_or(ArgError::MissingRequiredOption("--out"))?,
                workers,
            },
        )),
        _ => Err(ArgError::UnknownSubcommand {
            command: "saved-application",
            subcommand: sub,
        }),
    }
}

pub(super) fn run(arguments: SavedApplicationArgs) -> Result<(), CliError> {
    match arguments {
        SavedApplicationArgs::LayerBase { application } => {
            let base = crate::saved_application_layer_base(&application)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&base).map_err(|e| CliError::Input(e.to_string()))?
            );
            Ok(())
        }
        SavedApplicationArgs::IdentityLayer {
            application,
            out,
            workers,
        } => {
            let mut options = crate::SavedApplicationOptions::default();
            options.preparation_workers = workers;
            let layer = crate::saved_application_identity_layer::<15>(&application, options)?;
            let digest = layer.write(&out, options.binary_limits)?;
            println!(
                "{}",
                serde_json::json!({"layer": out.join("layer.json"), "layer_blake3": digest.layer_blake3,
                    "coefficient_table_blake3": digest.table_blake3, "sources": layer.rows.len(),
                    "provenance": layer.provenance})
            );
            Ok(())
        }
        SavedApplicationArgs::ComposeLayer {
            application,
            layer,
            out,
            terminal_definitions,
            terminal_definitions_out,
            size_gate,
            consumer_limits,
            fill_unreduced,
            allow_factorized_sources,
            minimality_reference,
            minimality_reference_reviewed,
            check_values_out,
            layered,
            dry_run,
            workers,
            self_check_points,
        } => {
            let mut options = SavedApplicationComposeOptions {
                size_gate,
                consumer_limits: crate::SavedApplicationConsumerLimits::parse(&consumer_limits)
                    .ok_or_else(|| CliError::Input("unknown consumer limits".into()))?,
                fill_unreduced,
                allow_factorized_sources,
                minimality_reference,
                minimality_reference_reviewed,
                check_values_out,
                allow_layered: layered,
                dry_run,
                self_check_points,
                terminal_definitions,
                terminal_definitions_out,
                ..Default::default()
            };
            options.application.preparation_workers = workers;
            let arity = crate::saved_application_arity(&application)?;
            if arity > 15 {
                return Err(CliError::Input(format!(
                    "saved application arity {arity} exceeds the supported 15"
                )));
            }
            let outcome =
                compose_saved_application_layer::<15>(&application, &layer, &out, &options)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome.report)
                    .map_err(|e| CliError::Input(e.to_string()))?
            );
            if outcome.mode == "rejected" {
                return Err(CliError::Input(
                    "composed maps exceed the size gate or the consumer limits; rerun with --layered \
                     for a layered package (needs a layer-aware consumer)"
                        .into(),
                ));
            }
            Ok(())
        }
    }
}
