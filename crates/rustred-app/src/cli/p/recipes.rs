//! `rustred p-recipes` (vk5l WP2): recipes of boxed nodes outside a library.
//!
//! ```text
//! rustred p-recipes solve   --family <F.sym> --order <JSON> --recipe <R.json> (--key <k,..> | --keys <K.jsonl>)
//!                           --out <dir> [--max-trace-rows <n>] [--threads <t>]
//! rustred p-recipes keys    --record <dir>/<label>.json
//! rustred p-recipes prefill --record <dir>/<label>.json --out <cache root> [--threads <t>] [--only <text>]
//! rustred p-recipes replay  --record <dir>/<label>.json --seeds <seeds.json> --out <replay.json>
//!                           [--recipe-cache <dir>] [--recipe-prefilled <dir>[,..]] [--recipe-workers <n>]
//! ```
//!
//! `solve` is the recipe verb (`rr_node solve-recipe`, `node::recipe::run_batch`)
//! in this binary: the reproduction line of every `PError::RecipeFailed` names
//! it; a single `--key` that does not solve prints its failure object on stderr
//! and exits 10. `keys` lists the keys a payload's dispatch sends to a recipe.
//! `prefill` solves all of them into the relation cache under `--out` (the
//! layout of `rustred::propagator::relation_entries::RelationStore` (relation entries), saved after
//! every key, so a rerun resumes; `--threads` engine workers), writes
//! `<cache dir>/PREFILL.json` and exits 10 when a key failed; the directory is
//! the optional pre-filled download, loaded read-only with
//! `RUSTRED_P_RECIPE_PREFILLED` (or `--recipe-prefilled`). `replay` reduces seeds
//! (TABLE slots) with the payload's program, recipes included, as
//! `p-import-table --replay` does.
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rustred::propagator::custom::{
    custom_payload_boxed_node, custom_payload_recipe_keys, custom_payload_recipe_prefill,
    replay_custom_payload,
};
use rustred::propagator::recipes::{
    EngineRecipeSolver, RecipeCacheRoots, RelationSource, override_recipe_solver,
    recipe_cache_directory, recipes_of_source, set_recipe_cache_roots,
};

use super::import::{
    failed, install_recipe_options, load_owner, recipe_failure_out, recipe_statistics_json,
};
use crate::cli::error::CliError;

fn usage(message: impl Into<String>) -> CliError {
    CliError::Input(format!("p-recipes: {}", message.into()))
}

/// `--name value` pairs and flags of one subcommand.
struct Options(Vec<(String, Option<String>)>);

impl Options {
    fn parse(arguments: Vec<OsString>, flags: &[&str], values: &[&str]) -> Result<Self, CliError> {
        let mut out = Vec::new();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| usage("a non-UTF-8 argument"))?;
            if flags.contains(&argument.as_str()) {
                out.push((argument, None));
            } else if values.contains(&argument.as_str()) {
                let value = arguments
                    .next()
                    .and_then(|value| value.into_string().ok())
                    .ok_or_else(|| usage(format!("{argument} needs a value")))?;
                out.push((argument, Some(value)));
            } else {
                return Err(usage(format!("unknown option {argument}")));
            }
        }
        Ok(Self(out))
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(option, _)| option == name)
            .and_then(|(_, value)| value.as_deref())
    }

    fn need(&self, name: &str) -> Result<&str, CliError> {
        self.get(name)
            .ok_or_else(|| usage(format!("{name} is required")))
    }

    fn number(&self, name: &str) -> Result<Option<usize>, CliError> {
        self.get(name)
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| usage(format!("{name} {value:?}: a number")))
            })
            .transpose()
    }
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let mut arguments = arguments.into_iter();
    let command = arguments
        .next()
        .and_then(|command| command.into_string().ok())
        .ok_or_else(|| usage("give a subcommand: solve, keys, prefill, replay"))?;
    let rest: Vec<OsString> = arguments.collect();
    match command.as_str() {
        "solve" => solve(rest),
        "keys" => keys(rest),
        "prefill" => prefill(rest),
        "replay" => replay(rest),
        other => Err(usage(format!(
            "unknown subcommand {other:?} (solve, keys, prefill, replay)"
        ))),
    }
}

fn read(path: &str) -> Result<String, CliError> {
    std::fs::read_to_string(path)
        .map_err(|error| CliError::InputIo(format!("cannot read {path}: {error}")))
}

/// The recipe verb (`rr_node solve-recipe` semantics, without job mode).
fn solve(arguments: Vec<OsString>) -> Result<(), CliError> {
    use rustred::node::recipe;
    let options = Options::parse(
        arguments,
        &[],
        &[
            "--family",
            "--order",
            "--recipe",
            "--key",
            "--keys",
            "--out",
            "--max-trace-rows",
            "--threads",
        ],
    )?;
    let family_path = options.need("--family")?;
    let recipe_path = options.need("--recipe")?;
    let out = PathBuf::from(options.need("--out")?);
    let family = read(family_path)?;
    let recipe_text = read(recipe_path)?;
    let order = rustred::node::order::NodeOrder::from_json(options.need("--order")?)
        .map_err(|error| usage(format!("--order: {error}")))?;
    let keys = match (options.get("--key"), options.get("--keys")) {
        (Some(key), None) => vec![recipe::KeyInput {
            key: key
                .split(',')
                .map(|part| part.trim().parse::<i64>())
                .collect::<Result<_, _>>()
                .map_err(|_| usage("--key: integers"))?,
            tag: None,
        }],
        (None, Some(path)) => recipe::parse_keys(&read(path)?).map_err(usage)?,
        _ => return Err(usage("give exactly one of --key K1,K2,.. / --keys FILE")),
    };
    if out.exists()
        && std::fs::read_dir(&out)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(true)
    {
        return Err(usage(format!(
            "--out {} exists and is not empty",
            out.display()
        )));
    }
    recipe::Recipe::parse(&recipe_text).map_err(usage)?;
    let canon = |path: &str| {
        std::fs::canonicalize(path)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| path.to_owned())
    };
    let mut opts = recipe::BatchOptions::new(recipe::binary_identity());
    opts.threads = options.number("--threads")?.unwrap_or(1);
    opts.max_trace_rows = options.number("--max-trace-rows")?;
    opts.family_path = canon(family_path);
    opts.recipe_path = canon(recipe_path);
    opts.order_text = order.to_json();
    let single = keys.len() == 1;
    let code = recipe::run_batch(&family, &order, &recipe_text, &keys, &out, &opts)
        .map_err(|error| usage(format!("solve: {error}")))?;
    if single && code != 0 {
        // stand-alone reproduction: the failure object on stderr as one line
        if let Ok(text) = std::fs::read_to_string(out.join("results.jsonl"))
            && let Some(first) = text.lines().next()
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(first)
        {
            let _ = writeln!(std::io::stderr().lock(), "{}", value["failure"]);
        }
    }
    let outcome = std::fs::read_to_string(out.join("OUTCOME.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_default();
    let _ = writeln!(
        std::io::stdout().lock(),
        "{}",
        serde_json::json!({"exit_code": code, "counts": outcome["counts"], "out": out.display().to_string()})
    );
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

fn keys(arguments: Vec<OsString>) -> Result<(), CliError> {
    let options = Options::parse(arguments, &[], &["--record"])?;
    let loaded = load_owner(std::path::Path::new(options.need("--record")?))?;
    let targets = custom_payload_recipe_keys(&loaded.presentation, &loaded.payload, &loaded.meta)
        .map_err(failed)?;
    let mut stdout = std::io::stdout().lock();
    for target in targets {
        let _ = writeln!(
            stdout,
            "{}",
            serde_json::json!({"key_table": target.key_table, "key": target.key, "recipe": target.recipe_id})
        );
    }
    Ok(())
}

fn prefill(arguments: Vec<OsString>) -> Result<(), CliError> {
    let options = Options::parse(
        arguments,
        &[],
        &["--record", "--out", "--threads", "--only"],
    )?;
    let record_path = PathBuf::from(options.need("--record")?);
    let root = PathBuf::from(options.need("--out")?);
    let threads = options.number("--threads")?.unwrap_or(1).max(1);
    let loaded = load_owner(&record_path)?;
    let boxed = custom_payload_boxed_node(&loaded.payload)
        .map_err(failed)?
        .ok_or_else(|| usage("the payload is not boxed: no recipes"))?;
    // The bundle, for the cache directory's name.
    let bundle = {
        let source = rustred::propagator::custom::custom_payload_family_source(&loaded.payload)
            .map_err(failed)?;
        recipes_of_source(&source)
            .map_err(failed)?
            .ok_or_else(|| usage("the payload carries no recipe bundle"))?
    };
    // Only the prefill's own cache: never read another root while filling it.
    set_recipe_cache_roots(RecipeCacheRoots {
        user: Some(root.clone()),
        prefilled: Vec::new(),
    });
    override_recipe_solver(
        &boxed.node,
        std::sync::Arc::new(EngineRecipeSolver::new(threads)),
    );
    let directory = recipe_cache_directory(&root, &bundle);
    let started = Instant::now();
    let progress = |entry: &rustred::propagator::custom::RecipePrefillEntry| {
        let status = match &entry.source {
            Ok(source) => format!("{source:?}"),
            Err(error) => format!("FAILED: {error}"),
        };
        let _ = writeln!(
            std::io::stderr().lock(),
            "prefill {} {:?} {:.1} s {status}",
            entry.target.recipe_id,
            entry.target.key_table,
            entry.seconds
        );
    };
    let entries = custom_payload_recipe_prefill(
        &loaded.presentation,
        &loaded.payload,
        &loaded.meta,
        threads,
        options.get("--only"),
        &progress,
    )
    .map_err(failed)?;
    let wall = started.elapsed().as_secs_f64();
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let mut failures = Vec::new();
    let rows: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            let source = match &entry.source {
                Ok(RelationSource::Solved) => "solved",
                Ok(RelationSource::Memo) => "memo",
                Ok(RelationSource::Prefilled) => "prefilled",
                Ok(RelationSource::UserCache) => "cached",
                Err(_) => "failed",
            };
            *counts.entry(source.to_owned()).or_default() += 1;
            let failure = match &entry.source {
                Err(rustred::propagator::error::PError::RecipeFailed(failure)) => {
                    failures.push(failure.to_json());
                    serde_json::Value::String(failure.to_string())
                }
                Err(error) => serde_json::Value::String(error.to_string()),
                Ok(_) => serde_json::Value::Null,
            };
            serde_json::json!({
                "recipe": entry.target.recipe_id, "key_table": entry.target.key_table,
                "key": entry.target.key, "source": source, "seconds": entry.seconds,
                "children": entry.children, "relation_digest": entry.relation_digest,
                "failure": failure,
            })
        })
        .collect();
    let report = serde_json::json!({
        "schema": "rustred.p-recipe-prefill.v1",
        "record": record_path.display().to_string(),
        "node": bundle.node, "revision": bundle.revision,
        "P_RECIPE_VERSION": bundle.version,
        "determinism_id": rustred::node::recipe::DETERMINISM_ID,
        "binary": rustred::node::recipe::binary_identity(),
        "cache_directory": directory.display().to_string(),
        "threads": threads, "wall_seconds": wall,
        "keys": entries.len(), "counts": counts,
        "statistics": recipe_statistics_json(),
        "entries": rows,
        "failures": failures,
    });
    std::fs::create_dir_all(&directory)
        .map_err(|error| CliError::OutputIo(format!("{}: {error}", directory.display())))?;
    let text = serde_json::to_string_pretty(&report)
        .map_err(|error| CliError::OutputIo(error.to_string()))?;
    std::fs::write(directory.join("PREFILL.json"), format!("{text}\n"))
        .map_err(|error| CliError::OutputIo(error.to_string()))?;
    let _ = writeln!(
        std::io::stdout().lock(),
        "prefill {} rev {}: {} keys in {wall:.1} s ({counts:?}) -> {}",
        bundle.node,
        bundle.revision,
        entries.len(),
        directory.display()
    );
    if !failures.is_empty() {
        std::process::exit(10);
    }
    Ok(())
}

fn replay(arguments: Vec<OsString>) -> Result<(), CliError> {
    let options = Options::parse(
        arguments,
        &[],
        &[
            "--record",
            "--seeds",
            "--out",
            "--recipe-cache",
            "--recipe-prefilled",
            "--recipe-workers",
        ],
    )?;
    let record_path = PathBuf::from(options.need("--record")?);
    let loaded = load_owner(&record_path)?;
    install_recipe_options(
        &loaded.payload,
        options.get("--recipe-cache").map(PathBuf::from),
        options.get("--recipe-prefilled").map(str::to_owned),
        options.number("--recipe-workers")?,
    )?;
    let seeds_text = read(options.need("--seeds")?)?;
    let table_seeds: Vec<Vec<i64>> =
        serde_json::from_str(&seeds_text).map_err(|error| usage(format!("--seeds: {error}")))?;
    let slot_map = &loaded.slot_map;
    let arity = slot_map.len();
    let to_presentation = |key: &[i64]| -> Result<Vec<i64>, CliError> {
        if key.len() != arity {
            return Err(usage(format!("seed {key:?} has not {arity} slots")));
        }
        let mut out = vec![0; arity];
        for (slot, &power) in key.iter().enumerate() {
            out[slot_map[slot]] = power;
        }
        Ok(out)
    };
    let to_table = |key: &[i64]| -> Vec<i64> { slot_map.iter().map(|&image| key[image]).collect() };
    let seeds = table_seeds
        .iter()
        .map(|seed| to_presentation(seed))
        .collect::<Result<Vec<_>, _>>()?;
    let started = Instant::now();
    let failure_path = PathBuf::from(format!("{}.recipe-failure.json", options.need("--out")?));
    let replays =
        replay_custom_payload(&loaded.presentation, &loaded.payload, &loaded.meta, &seeds)
            .map_err(|error| recipe_failure_out(error, &failure_path))?;
    let seconds = started.elapsed().as_secs_f64();
    let terms = |terms: &[(Vec<i64>, rustred::algebra::Coefficient)]| -> Vec<serde_json::Value> {
        terms
            .iter()
            .map(|(key, value)| serde_json::json!([to_table(key), value.to_string()]))
            .collect()
    };
    let results: Vec<serde_json::Value> = replays
        .iter()
        .zip(&table_seeds)
        .map(|(replay, seed)| {
            serde_json::json!({
                "seed": seed, "steps": replay.steps,
                "residuals": terms(&replay.residuals), "boundary": terms(&replay.boundary),
            })
        })
        .collect();
    let record = serde_json::json!({
        "schema": "rustred.p-imported-owner-replay.v1",
        "label": loaded.label,
        "convention": "presentation (q.q = -1), keys in the table's slot order",
        "seconds": seconds,
        "recipes": recipe_statistics_json(),
        "results": results,
    });
    let text =
        serde_json::to_string(&record).map_err(|error| CliError::OutputIo(error.to_string()))?;
    std::fs::write(options.need("--out")?, format!("{text}\n"))
        .map_err(|error| CliError::OutputIo(error.to_string()))?;
    let _ = writeln!(
        std::io::stdout().lock(),
        "replay of {} seeds in {seconds:.1} s: {}",
        table_seeds.len(),
        recipe_statistics_json()
    );
    Ok(())
}
