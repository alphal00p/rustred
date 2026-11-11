//! `rustred p-import-table` (owner: p5l-converter).
//!
//! ```text
//! rustred p-import-table --table <F.table.jsonl> --graph "<census edges>" --ext <s>,<t>
//!                        --label <name> --lp <count> --out <dir> [--force]
//!                        [--replay <seeds.json>] [--census-family <T.sym>] [--order-map <ORDER-MAP.json>]
//! ```
//!
//! Converts one `table_dump.v1` rule table of a 5L-campaign node into a Custom
//! owner payload over a presentation of the node graph
//! (`rustred::propagator::table_import`): rule order, cases, exceptions and
//! dispatch policies kept exactly, slots relabelled onto the canonical lines
//! of the graph, `q.q = +1 -> -1`. The graph is the census graph of the node
//! (`rules_study/targets.tsv`: `edges`, `S`, `T`; stars are eps units).
//!
//! Writes `<dir>/<label>.rrbin` (the payload) and `<dir>/<label>.json` (the
//! import record: slot map, presentation slots, counts, the load-time check of
//! `custom::verify_custom_payload`). A table the payload format cannot carry
//! fails with its gap (`table import gap <kind>: ...`) and writes nothing.
//!
//! `--replay <seeds.json>` (a JSON list of keys in the TABLE's slot order)
//! reduces each seed with the imported owner's program and writes
//! `<dir>/<label>.replay.json`: per seed the residual and boundary keys in the
//! table's slot order with their coefficients in the presentation's
//! convention (`q.q = -1`), exact in `Q(d)`, and the sign rule to go back.
//!
//! The payload records the table's SLOT ORDER (`table_import::SlotOrderRecord`:
//! the header's order in table slots, the table -> presentation slot map, the
//! descent order they give) and every payload read enforces it.
//! `--census-family` relates the table family to the node's census family
//! (slot permutation + unimodular loop map, recorded; a table that is no
//! relabelling of it is refused); `--order-map` checks an ORDER-MAP.json of
//! the table (Astra imports) against the header and takes its slot map.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rustred::propagator::PresentationId;
use rustred::propagator::custom::{replay_custom_payload, verify_custom_payload};
use rustred::propagator::error::PError;
use rustred::propagator::presentation::Slot;
use rustred::propagator::table_import::{
    P_TABLE_IMPORT_VERSION, TableImportOptions, canonical_node_graph, import_table, slot_map_text,
};
use rustred::propagator::{P_CUSTOM_OWNER_VERSION, P_OWNER_LIBRARY_SEMANTICS};

use crate::AppError;
use crate::cli::args::{ArgError, StreamPath, next_utf8_value, next_value};
use crate::cli::error::CliError;
use crate::cli::io::{preflight_output_destination, read_input, write_output};

/// Schema of the `<label>.json` import record.
pub(crate) const P_IMPORTED_OWNER_SCHEMA: &str = "rustred.p-imported-owner.v1";

#[derive(Debug, PartialEq)]
struct Arguments {
    table: PathBuf,
    graph: String,
    ext: (u8, u8),
    label: String,
    lp: u32,
    out: PathBuf,
    force: bool,
    replay: Option<PathBuf>,
    census_family: Option<PathBuf>,
    order_map: Option<PathBuf>,
}

fn parse(arguments: Vec<OsString>) -> Result<Arguments, ArgError> {
    let mut arguments = arguments.into_iter();
    let (mut table, mut graph, mut ext, mut label) = (None, None, None, None);
    let (mut lp, mut out, mut force, mut replay) = (None, None, false, None);
    let (mut census_family, mut order_map) = (None, None);
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match argument.as_str() {
            "--table" => "--table",
            "--graph" => "--graph",
            "--ext" => "--ext",
            "--label" => "--label",
            "--lp" => "--lp",
            "--out" => "--out",
            "--force" => "--force",
            "--replay" => "--replay",
            "--census-family" => "--census-family",
            "--order-map" => "--order-map",
            _ => return Err(ArgError::UnknownOption(argument)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--table" => table = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--out" => out = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--replay" => replay = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--census-family" => census_family = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--order-map" => order_map = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--graph" => graph = Some(next_utf8_value(&mut arguments, option)?),
            "--label" => label = Some(next_utf8_value(&mut arguments, option)?),
            "--ext" => {
                let value = next_utf8_value(&mut arguments, option)?;
                ext = Some(
                    value
                        .split_once(',')
                        .and_then(|(s, t)| Some((s.trim().parse().ok()?, t.trim().parse().ok()?)))
                        .ok_or(ArgError::InvalidValue {
                            option,
                            value: value.clone(),
                            expected: "two vertices s,t",
                        })?,
                );
            }
            "--lp" => {
                let value = next_utf8_value(&mut arguments, option)?;
                lp = Some(value.parse().map_err(|_| ArgError::InvalidValue {
                    option,
                    value: value.clone(),
                    expected: "a nonnegative LP master count",
                })?);
            }
            _ => force = true,
        }
    }
    Ok(Arguments {
        table: table.ok_or(ArgError::MissingRequiredOption("--table"))?,
        graph: graph.ok_or(ArgError::MissingRequiredOption("--graph"))?,
        ext: ext.ok_or(ArgError::MissingRequiredOption("--ext"))?,
        label: label.ok_or(ArgError::MissingRequiredOption("--label"))?,
        lp: lp.ok_or(ArgError::MissingRequiredOption("--lp"))?,
        out: out.ok_or(ArgError::MissingRequiredOption("--out"))?,
        force,
        replay,
        census_family,
        order_map,
    })
}

fn failed(error: PError) -> CliError {
    CliError::Application(AppError::derivation(format!("p import-table: {error}")))
}

fn json_error(error: serde_json::Error) -> CliError {
    CliError::OutputIo(error.to_string())
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let arguments = parse(arguments).map_err(CliError::Usage)?;
    if !arguments
        .label
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(CliError::Input(format!("label {:?}", arguments.label)));
    }
    let destination =
        |extension: &str| StreamPath::File(arguments.out.join(format!("{}.{extension}", arguments.label)));
    let mut outputs = vec!["rrbin", "json"];
    if arguments.replay.is_some() {
        outputs.push("replay.json");
    }
    std::fs::create_dir_all(&arguments.out).map_err(|error| {
        CliError::OutputIo(format!("cannot create {}: {error}", arguments.out.display()))
    })?;
    for extension in &outputs {
        preflight_output_destination(&destination(extension), arguments.force)?;
    }
    let started = Instant::now();
    let text = read_input(&StreamPath::File(arguments.table.clone()))?;
    let (key, graph) =
        canonical_node_graph(&arguments.graph, arguments.ext.0, arguments.ext.1).map_err(failed)?;
    let options = TableImportOptions {
        id: PresentationId(0),
        node: None,
        lp_count: arguments.lp,
        census_family: match &arguments.census_family {
            Some(path) => Some(read_input(&StreamPath::File(path.clone()))?),
            None => None,
        },
        order_map: match &arguments.order_map {
            Some(path) => Some(
                serde_json::from_str(&read_input(&StreamPath::File(path.clone()))?)
                    .map_err(|error| CliError::Input(format!("{}: {error}", path.display())))?,
            ),
            None => None,
        },
    };
    let imported = import_table(&text, &graph, &options).map_err(failed)?;
    let import_seconds = started.elapsed().as_secs_f64();
    let verify_started = Instant::now();
    let check = verify_custom_payload(&imported.presentation, &imported.payload, &imported.meta)
        .map_err(failed)?;
    let verify_seconds = verify_started.elapsed().as_secs_f64();

    let presentation = &imported.presentation;
    let slots: Vec<serde_json::Value> = presentation
        .slots
        .iter()
        .map(|slot| match slot {
            Slot::Line(line) => serde_json::json!({"line": line.momentum, "eps_units": line.eps_units}),
            Slot::Isp(isp) => serde_json::json!({"isp": [isp.left, isp.right]}),
        })
        .collect();
    let mut record = serde_json::json!({
        "schema": P_IMPORTED_OWNER_SCHEMA,
        "label": arguments.label,
        "table": arguments.table.display().to_string(),
        "graph": arguments.graph,
        "ext": [arguments.ext.0, arguments.ext.1],
        "graph_key": imported.graph_key.to_hex(),
        "canonical_graph": {
            "n": graph.n, "s": graph.s, "t": graph.t,
            "lines": graph.lines.iter().map(|&(u, v, e)| [u, v, e]).collect::<Vec<_>>(),
        },
        "table_family": imported.table_family.name,
        "loops": presentation.loops,
        "arity": presentation.slots.len(),
        "presentation_kind": "caller",
        "presentation_slots": slots,
        "family_fingerprint": presentation.family.fingerprint(),
        "slot_map": imported.slot_map,
        "slot_names": slot_map_text(&imported),
        "q_flipped": imported.q_flipped,
        "line_signs": imported.line_signs,
        "external_square": -1,
        "sign_rule": "presentation coefficient = table coefficient * (-1)^(|target| - |term|), |.| = sum of integer powers",
        "order_name": imported.order_name,
        "ordering": imported.summary.ordering,
        "support": imported.summary.support,
        "two_sided_axes": imported.summary.two_sided_axes,
        "lp_count": imported.meta.lp_count,
        "residuals": imported.meta.residuals,
        "charts": imported.summary.charts.iter().map(|chart| serde_json::json!({
            "chart": chart.chart, "rules": chart.rules, "residuals": chart.residuals,
        })).collect::<Vec<_>>(),
        "rules": imported.rules,
        "rhs_terms": imported.rhs_terms,
        "negated_terms": imported.negated_terms,
        "dispatched_rules": imported.dispatched_rules,
        "affine_rules": imported.affine_rules,
        "rules_with_exceptions": imported.rules_with_exceptions,
        "payload_bytes": imported.payload.len(),
        "verify": {
            "charts": check.charts,
            "rules": check.rules,
            "support_primary": check.support_primary,
            "witness_keys": check.witness_keys,
            "same_sector_children": check.same_sector_children,
            "subsector_children": check.subsector_children,
            "terminals": check.terminals,
        },
        "import_seconds": import_seconds,
        "verify_seconds": verify_seconds,
        "versions": {
            "P_TABLE_IMPORT_VERSION": P_TABLE_IMPORT_VERSION,
            "P_CUSTOM_OWNER_VERSION": P_CUSTOM_OWNER_VERSION,
            "P_OWNER_LIBRARY_SEMANTICS": P_OWNER_LIBRARY_SEMANTICS,
        },
    });
    record["slot_order"] = serde_json::to_value(&imported.slot_order).map_err(json_error)?;
    record["census_family"] = serde_json::json!(arguments.census_family.as_ref().map(|p| p.display().to_string()));
    record["order_map"] = serde_json::json!(arguments.order_map.as_ref().map(|p| p.display().to_string()));

    if let Some(seeds_path) = &arguments.replay {
        let seeds_text = read_input(&StreamPath::File(seeds_path.clone()))?;
        let table_seeds: Vec<Vec<i64>> = serde_json::from_str(&seeds_text)
            .map_err(|error| CliError::Input(format!("{}: {error}", seeds_path.display())))?;
        let arity = imported.slot_map.len();
        let to_presentation = |key: &[i64]| -> Result<Vec<i64>, CliError> {
            if key.len() != arity {
                return Err(CliError::Input(format!("seed {key:?} has not {arity} slots")));
            }
            let mut out = vec![0; arity];
            for (slot, &power) in key.iter().enumerate() {
                out[imported.slot_map[slot]] = power;
            }
            Ok(out)
        };
        let to_table = |key: &[i64]| -> Vec<i64> {
            imported.slot_map.iter().map(|&image| key[image]).collect()
        };
        let seeds = table_seeds
            .iter()
            .map(|seed| to_presentation(seed))
            .collect::<Result<Vec<_>, _>>()?;
        let replay_started = Instant::now();
        let replays = replay_custom_payload(presentation, &imported.payload, &imported.meta, &seeds)
            .map_err(failed)?;
        let replay_seconds = replay_started.elapsed().as_secs_f64();
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
                    "seed": seed,
                    "steps": replay.steps,
                    "residuals": terms(&replay.residuals),
                    "boundary": terms(&replay.boundary),
                })
            })
            .collect();
        let replay_record = serde_json::json!({
            "schema": "rustred.p-imported-owner-replay.v1",
            "label": arguments.label,
            "convention": "presentation (q.q = -1), keys in the table's slot order",
            "seconds": replay_seconds,
            "results": results,
        });
        record["replay_seconds"] = serde_json::json!(replay_seconds);
        let text = serde_json::to_string(&replay_record).map_err(json_error)?;
        write_output(&destination("replay.json"), format!("{text}\n").as_bytes(), arguments.force)?;
    }

    let json = serde_json::to_string_pretty(&record).map_err(json_error)?;
    write_output(&destination("rrbin"), &imported.payload, arguments.force)?;
    write_output(&destination("json"), format!("{json}\n").as_bytes(), arguments.force)?;
    writeln!(
        std::io::stdout().lock(),
        "{}: {} rules over {} charts, {} residuals (LP {}), {} bytes; slot map {:?}{}; order {}{}; verify {} witness keys ({} same-sector, {} subsector children) in {:.3} s; import {:.3} s",
        arguments.label,
        imported.rules,
        imported.summary.charts.len(),
        imported.meta.residuals,
        imported.meta.lp_count,
        imported.payload.len(),
        imported.slot_map,
        if imported.q_flipped { ", q flipped" } else { "" },
        imported.order_name,
        match &imported.slot_order.census {
            Some(census) => format!(
                " (census map {:?} by {}, loop map {})",
                census.census_to_table,
                census.source,
                if census.loop_map.iter().enumerate().all(|(r, row)| row.iter().enumerate().all(|(c, &v)| v == i64::from(r == c))) { "1" } else { "nontrivial" }
            ),
            None => String::new(),
        },
        check.witness_keys,
        check.same_sector_children,
        check.subsector_children,
        verify_seconds,
        import_seconds,
    )
    .map_err(|error| CliError::OutputIo(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_import_table_parses_its_options() {
        let parsed = parse(
            [
                "--table", "t.jsonl", "--graph", "0-1* 0-2", "--ext", "2,3", "--label", "T012",
                "--lp", "1", "--out", "o", "--replay", "s.json",
            ]
            .iter()
            .map(OsString::from)
            .collect(),
        )
        .unwrap();
        assert_eq!((parsed.ext, parsed.lp, parsed.force), ((2, 3), 1, false));
        assert!(matches!(
            parse(vec![OsString::from("--table"), OsString::from("t")]),
            Err(ArgError::MissingRequiredOption(_))
        ));
    }
}
