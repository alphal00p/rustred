//! One read-only inspection surface for published artifacts and saved campaigns.
//! Pointer metadata never substitutes for mathematical validation.
use super::args::{ArgError, Command, next_utf8_value, next_value, parse_positive_integer};
use super::error::CliError;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InspectArgs {
    artifact: Option<PathBuf>,
    finite_policy: Option<PathBuf>,
    campaign: Option<PathBuf>,
    format: String,
    threads: usize,
}

pub(super) fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut result = InspectArgs {
        artifact: None,
        finite_policy: None,
        campaign: None,
        format: "auto".into(),
        threads: 1,
    };
    let mut seen = BTreeSet::new();
    while let Some(arg) = args.next() {
        let arg = arg.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match arg.as_str() {
            "--artifact" => "--artifact",
            "--finite-policy" => "--finite-policy",
            "--campaign-directory" => "--campaign-directory",
            "--format" => "--format",
            "--threads" => "--threads",
            _ => return Err(ArgError::UnknownOption(arg)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--artifact" => result.artifact = Some(PathBuf::from(next_value(&mut args, option)?)),
            "--finite-policy" => {
                result.finite_policy = Some(PathBuf::from(next_value(&mut args, option)?))
            }
            "--campaign-directory" => {
                result.campaign = Some(PathBuf::from(next_value(&mut args, option)?))
            }
            "--threads" => {
                result.threads =
                    parse_positive_integer(option, next_utf8_value(&mut args, option)?)?
            }
            _ => {
                let value = next_utf8_value(&mut args, option)?;
                if !["auto", "table", "json"].contains(&value.as_str()) {
                    return Err(ArgError::InvalidValue {
                        option,
                        value,
                        expected: "auto, table or json",
                    });
                }
                result.format = value;
            }
        }
    }
    if result.artifact.is_some() == result.campaign.is_some() {
        return Err(ArgError::InvalidCombination(
            "artifact-inspect requires exactly one of --campaign-directory or --artifact",
        ));
    }
    Ok(Command::ArtifactInspect(result))
}

fn read_json(path: &Path, policy: Option<&crate::FiniteDemandPolicy>) -> Result<Value, CliError> {
    let file = std::fs::File::open(path)
        .map_err(|e| CliError::InputIo(format!("{}: {e}", path.display())))?;
    let maximum = policy.map_or(Ok(crate::MAX_INPUT_BYTES), |p| {
        usize::try_from(p.control.max_bytes)
            .map_err(|_| CliError::Input("finite metadata allowance exceeds platform size".into()))
    })?;
    let bytes = super::io::read_bounded(file, "artifact metadata", maximum)?;
    serde_json::from_slice(&bytes).map_err(|e| CliError::Input(format!("{}: {e}", path.display())))
}

fn canonical(path: &Path) -> Result<PathBuf, CliError> {
    path.canonicalize()
        .map_err(|e| CliError::InputIo(format!("{}: {e}", path.display())))
}

fn relative_child(base: &Path, name: &str) -> Result<PathBuf, CliError> {
    let path = Path::new(name);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(CliError::Input(
            "artifact member must be a nonempty relative path".into(),
        ));
    }
    let resolved = canonical(&base.join(path))?;
    if !resolved.starts_with(canonical(base)?) {
        return Err(CliError::Input(
            "artifact member escapes its directory".into(),
        ));
    }
    Ok(resolved)
}

fn published_path(
    campaign: &Path,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<Option<PathBuf>, CliError> {
    let pointer = campaign.join("artifacts/latest.json");
    if pointer.is_file() {
        let value = read_json(&pointer, policy)?;
        if value["schema"] != "rustred.saved-artifact-pointer.v1" {
            return Err(CliError::Input("unsupported saved-artifact pointer".into()));
        }
        let name = value["directory"]
            .as_str()
            .ok_or_else(|| CliError::Input("artifact pointer has no directory".into()))?;
        return Ok(Some(relative_child(campaign, name)?));
    }
    // Existing published packages remain inspectable. A legacy absolute
    // pointer is allowed only when it resolves inside this campaign.
    let previous = campaign.join("master-reduction/completed-phase.json");
    if previous.is_file() {
        let value = read_json(&previous, policy)?;
        let name = value["directory"]
            .as_str()
            .ok_or_else(|| CliError::Input("completed phase has no directory".into()))?;
        let path = canonical(&campaign.join(name))?;
        if !path.starts_with(campaign) {
            return Err(CliError::Input(
                "completed phase escapes this campaign; inspect its artifact explicitly".into(),
            ));
        }
        return Ok(Some(path));
    }
    Ok(None)
}

fn scope_documents(
    directory: &Path,
    packaged: bool,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<Vec<Value>, CliError> {
    let inputs = directory.join("inputs");
    let mut documents = vec![read_json(&inputs.join("queries.json"), policy)?];
    if packaged {
        let amendments = read_json(&inputs.join("amendments.json"), policy)?;
        let entries = amendments
            .as_array()
            .ok_or_else(|| CliError::Input("invalid packaged amendment inventory".into()))?;
        if entries.len() > policy.map_or(usize::MAX, |p| p.control.max_files) {
            return Err(CliError::Input("too many amendments to inspect".into()));
        }
        for row in entries {
            let name = row["path"]
                .as_str()
                .ok_or_else(|| CliError::Input("packaged amendment has no path".into()))?;
            documents.push(read_json(&relative_child(&inputs, name)?, policy)?);
        }
    } else {
        let folder = directory.join("amendments");
        if folder.exists() {
            let mut paths = Vec::new();
            for entry in std::fs::read_dir(&folder).map_err(|e| CliError::InputIo(e.to_string()))? {
                let entry = entry.map_err(|e| CliError::InputIo(e.to_string()))?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("amendment-") && name.ends_with(".json") {
                    paths.push(relative_child(&folder, &name)?);
                    if paths.len() > policy.map_or(usize::MAX, |p| p.control.max_files) {
                        return Err(CliError::Input("too many amendments to inspect".into()));
                    }
                }
            }
            paths.sort();
            for path in paths {
                documents.push(read_json(&path, policy)?);
            }
        }
    }
    Ok(documents)
}

fn scope_summary(documents: &[Value]) -> Result<Value, CliError> {
    let mut rows = Vec::new();
    for doc in documents {
        rows.extend(
            doc["queries"]
                .as_array()
                .ok_or_else(|| CliError::Input("scope document has no query array".into()))?,
        );
    }
    let rank: Option<Vec<u64>> = rows
        .iter()
        .map(|r: &&Value| r["max_numerator_rank"].as_u64())
        .collect();
    let degree: Option<Vec<i64>> = rows
        .iter()
        .map(|r: &&Value| r["power_bounds"]["max_power_difference"].as_i64())
        .collect();
    Ok(
        json!({"max_starting_rank":rank.and_then(|r|r.into_iter().max()),
        "max_starting_d":degree.and_then(|r|r.into_iter().max()),"starting_queries":rows.len()}),
    )
}

fn saved_scope_documents(
    argv: &[Value],
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<Vec<Value>, CliError> {
    let mut initial = None;
    let mut amendments = Vec::new();
    for pair in argv.windows(2) {
        if pair[0] == "--queries" || pair[0] == "--amend-queries" {
            let path = pair[1]
                .as_str()
                .ok_or_else(|| CliError::Input("invalid saved query path".into()))?;
            let document = read_json(Path::new(path), policy)?;
            if pair[0] == "--queries" {
                if initial.replace(document).is_some() {
                    return Err(CliError::Input("duplicate saved queries".into()));
                }
            } else {
                amendments.push(document);
            }
        }
    }
    let mut documents =
        vec![initial.ok_or_else(|| CliError::Input("saved command has no queries".into()))?];
    documents.extend(amendments);
    Ok(documents)
}

fn same_published_scope(
    campaign: &Path,
    artifact: &Path,
    requested: &[Value],
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<bool, CliError> {
    Ok(requested == scope_documents(artifact, true, policy)?
        && read_json(&campaign.join("inputs/selection.json"), policy)?
            == read_json(&artifact.join("inputs/original-selection.json"), policy)?)
}

pub(super) fn run(args: InspectArgs) -> Result<(), CliError> {
    let policy = args
        .finite_policy
        .as_ref()
        .map(|p| super::finite_demand::read_policy(p))
        .transpose()?;
    let inspect = |path: &Path| match &policy {
        Some(policy) => crate::master_reduction_inspect_with_policy(path, policy),
        None => crate::master_reduction_inspect(path),
    };
    let mut report = if let Some(campaign) = &args.campaign {
        let campaign = canonical(campaign)?;
        let requested = scope_documents(&campaign, false, policy.as_ref())?;
        let requested_scope = scope_summary(&requested)?;
        if let Some(artifact) = published_path(&campaign, policy.as_ref())? {
            let mut report = inspect(&artifact)?;
            let current = same_published_scope(&campaign, &artifact, &requested, policy.as_ref())?;
            report["artifact"] = json!(artifact);
            report["requested_scope"] = requested_scope;
            report["published_scope_current"] = json!(current);
            report["scope_selection"] = json!(if current {
                "latest publication; current request"
            } else {
                "earlier publication; new request not published"
            });
            report
        } else {
            eprintln!(
                "No published artifact yet: reading the saved checkpoint and normalizing its inventory (read-only; no master refinement)."
            );
            let inventory = super::walk_inventory::campaign_summary(&campaign, args.threads)?;
            // A prepared amendment may not have been admitted by the saved
            // walk. Do not label its requested bounds as verified coverage.
            let active = read_json(&campaign.join("active-run.json"), policy.as_ref())?;
            let run = active["run_directory"]
                .as_str()
                .ok_or_else(|| CliError::Input("active run has no directory".into()))?;
            let name = Path::new(run)
                .file_name()
                .ok_or_else(|| CliError::Input("invalid run directory".into()))?;
            let command = read_json(
                &campaign.join("runs").join(name).join("request.json"),
                policy.as_ref(),
            )?;
            let argv = command
                .get("command")
                .unwrap_or(&command)
                .as_array()
                .ok_or_else(|| CliError::Input("saved command is not an array".into()))?;
            let saved_documents = saved_scope_documents(argv, policy.as_ref())?;
            json!({"status":if inventory["complete"] == true {"inventory_only_not_published"} else {"inventory_incomplete"},
                "artifact":null,"source_checkpoint":campaign.join("checkpoints/main"),
                "scope":scope_summary(&saved_documents)?,"requested_scope":requested_scope,
                "published_scope_current":false,"scope_selection":"checkpoint inventory; no portable publication yet",
                "raw_terminals":inventory["encountered"]["terminals"],
                "normalized_terminals":inventory["normalization"]["canonical_terminals"],
                "remaining_terminals":inventory["normalization"]["canonical_terminals"],
                "refinement_status":"not run", "relation_rows":0,"inventory":inventory})
        }
    } else {
        let artifact = args.artifact.as_ref().expect("parsed source");
        let mut report = inspect(artifact)?;
        report["artifact"] = json!(canonical(artifact)?);
        report["scope_selection"] =
            json!("explicit artifact; not necessarily latest campaign result");
        report
    };
    summarize_refinement(&mut report);
    let tty = std::io::stdout().is_terminal();
    if args.format == "json" || args.format == "auto" && !tty {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| CliError::OutputIo(e.to_string()))?
        );
    } else {
        let width = crossterm::terminal::size().map_or(110, |(w, _)| w as usize);
        println!(
            "{}",
            super::master_table::render_master_table(
                &report,
                tty && std::env::var_os("NO_COLOR").is_none(),
                width
            )
        );
    }
    Ok(())
}

fn summarize_refinement(report: &mut Value) {
    if report["partial_refinement_source"].is_object() && report["finite_search_complete"] == false
    {
        report["refinement_status"] = json!("partial retained; search unfinished");
    } else if report["refinement_strategy"] == "modular-circuits" {
        report["refinement_status"] = json!(if report["modular_search_complete"] == true {
            "modular pass completed; exact span unfinished"
        } else {
            "modular discovery unfinished; exact span unfinished"
        });
    } else if report["refinement_status"].is_null() {
        report["refinement_status"] = json!(if report["status"] == "published_unrefined" {
            "not requested"
        } else if report["status"] == "completed_nonminimal" {
            "finite search completed; nonminimal"
        } else {
            "unfinished"
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_refinement_summary_preserves_unfinished_search() {
        let mut report = json!({"status":"published_unrefined", "finite_search_complete":false});
        summarize_refinement(&mut report);
        assert_eq!(report["refinement_status"], "not requested");
        report["partial_refinement_source"] = json!({"status":"failed"});
        summarize_refinement(&mut report);
        assert_eq!(
            report["refinement_status"],
            "partial retained; search unfinished"
        );
    }

    #[test]
    fn artifact_inspect_requires_one_source_and_valid_format() {
        for args in [
            vec![],
            vec!["--artifact", "a", "--campaign-directory", "c"],
            vec!["--artifact", "a", "--format", "xml"],
        ] {
            assert!(parse(args.into_iter().map(OsString::from)).is_err());
        }
        assert!(
            parse(
                ["--campaign-directory", "c", "--format", "table"]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_ok()
        );
    }

    #[test]
    fn artifact_inspect_requested_scope_includes_extensions() {
        let old =
            json!({"queries":[{"max_numerator_rank":0,"power_bounds":{"max_power_difference":9}}]});
        let new = json!({"queries":[{"max_numerator_rank":2,"power_bounds":{"max_power_difference":10}}]});
        let summary = scope_summary(&[old, new]).unwrap();
        assert_eq!(summary["max_starting_rank"], 2);
        assert_eq!(summary["max_starting_d"], 10);
        assert_eq!(summary["starting_queries"], 2);
    }

    #[test]
    fn artifact_inspect_pointer_and_source_scope_are_not_inferred() {
        let scratch =
            std::env::temp_dir().join(format!("rustred-artifact-inspect-{}", std::process::id()));
        let campaign = scratch.join("campaign");
        let artifact = campaign.join("artifacts/one");
        std::fs::create_dir_all(artifact.join("inputs")).unwrap();
        std::fs::create_dir_all(campaign.join("inputs")).unwrap();
        let save = |path: PathBuf, value: Value| {
            std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap()
        };
        let original =
            json!({"queries":[{"max_numerator_rank":0,"power_bounds":{"max_power_difference":9}}]});
        save(artifact.join("inputs/queries.json"), original.clone());
        save(artifact.join("inputs/amendments.json"), json!([]));
        save(
            artifact.join("inputs/original-selection.json"),
            json!({"owners":["one"]}),
        );
        save(
            campaign.join("inputs/selection.json"),
            json!({"owners":["one"]}),
        );
        assert!(same_published_scope(&campaign, &artifact, &[original.clone()], None).unwrap());
        save(
            campaign.join("inputs/selection.json"),
            json!({"owners":["two"]}),
        );
        assert!(!same_published_scope(&campaign, &artifact, &[original.clone()], None).unwrap());
        save(
            campaign.join("inputs/queries.json"),
            json!({"queries":[{"max_numerator_rank":3}]}),
        );
        let argv = vec![
            json!("rustred"),
            json!("--queries"),
            json!(artifact.join("inputs/queries.json")),
        ];
        assert_eq!(saved_scope_documents(&argv, None).unwrap(), vec![original]);
        let pointer = campaign.join("artifacts/latest.json");
        save(
            pointer.clone(),
            json!({"schema":"rustred.saved-artifact-pointer.v1","directory":"artifacts/one"}),
        );
        assert_eq!(
            published_path(&campaign, None).unwrap(),
            Some(artifact.canonicalize().unwrap())
        );
        save(
            pointer,
            json!({"schema":"rustred.saved-artifact-pointer.v1","directory":"../campaign/artifacts/one"}),
        );
        assert!(published_path(&campaign, None).is_err());
        std::fs::remove_dir_all(scratch).unwrap();
    }
}
