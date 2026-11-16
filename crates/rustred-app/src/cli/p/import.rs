//! `rustred p-import-table` (owner: p5l-converter).
//!
//! ```text
//! rustred p-import-table --table <F.table.jsonl> --graph "<census edges>" --ext <s>,<t>
//!                        --label <name> --lp <count> --out <dir> [--force]
//!                        [--replay <seeds.json>] [--census-family <T.sym>] [--order-map <ORDER-MAP.json>]
//!                        [--coef-form expanded|factored|shared] [--partial-box] [--mixed-box] [--no-verify]
//!                        [--certified-keys <keys.jsonl | .rrkeys>]
//!                        [--recipes <recipes.jsonl> [--recipe-revision <name>] [--recipes-first]]
//! rustred p-import-table --check <dir>/<label>.json [--force]
//!                        [--recipe-cache <dir>] [--recipe-prefilled <dir>[,<dir>..]] [--recipe-workers <n>]
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
//!
//! A BOXED table (header field `boxed_node`, `table_import::BoxedNodeRecord`)
//! carries exact-key point rules over a finite box of the node's keys and its
//! terminals; the import checks the box (point rules, targets and terminals
//! inside it, every box key covered, every chart RHS key in the box and
//! covered) and records it in the payload and in the import record (`boxed`).
//! `--partial-box` accepts a box that is not closed (a pilot table).
//! `--mixed-box` accepts parametric rules (symbolic, face, bank) beside the
//! point rules of a boxed table, in any order (first match), as the header
//! flag `parametric_after_points` does.
//!
//! `--certified-keys <file>` (vk5l WP1) gives a boxed table its CERTIFIED
//! KEY SET, the forward closure its rules are certified on (a closure
//! statement's key list, one key `I(..)` or one JSON row with the key text
//! first per line, in the TABLE's slot order; or a `.rrkeys` file of
//! `rustred::propagator::certified_keys`): rule targets, terminals and chart
//! right-hand sides are checked against it instead of the layer box, it must
//! hold every box key, the payload carries it (presentation slots) and every
//! dispatch of the node checks it (`PError::OutsideCertifiedSet`).
//!
//! `--no-verify` (t162-export) writes the payload and the import record
//! without the load-time check (`"verify": null`), so import and check can
//! run as separate jobs; `--check <dir>/<label>.json` then runs the check on
//! `<dir>/<label>.rrbin` (presentation rebuilt from the record's slots, family
//! fingerprint compared) and writes `<dir>/<label>.check.json`. The record of
//! a boxed payload reports `open_box_keys` (witness keys inside a partial box
//! without rule) and `box_keys_stepped`. vk5l WP1: the check record also
//! reports the load memory of the payload (`load`: payload bytes, owner build
//! time, certified set bytes, RSS after the read, after the build and the
//! peak of the whole check, from `/proc/self/status`).
//!
//! `--recipes <file>` (vk5l WP2) gives a boxed table RECIPE RECORDS
//! (`rustred::propagator::recipes`): one JSON object per line, a
//! `rustred.recipe.v1` recipe (its `domain.keys` are its explicit certified
//! keys, TABLE slots) or `{"recipe": {..}, "relation_digests": {"n0,..": d},
//! "max_trace_rows": n}`. The payload carries them as its recipe bundle (family
//! text and node order from the table header, the slot map of the import, the
//! revision name `--recipe-revision`, default `<label>-r1`); their keys count as
//! covered, and dispatch solves them in process (table rules first, unless
//! `--recipes-first`). `--check` steps them too (each solved, or read from the
//! relation caches `--recipe-cache` (read-write) and `--recipe-prefilled`
//! (read-only overlays), default `RUSTRED_P_RECIPE_CACHE` /
//! `RUSTRED_P_RECIPE_PREFILLED`), reports `recipes` (keys, solves, seconds,
//! cache hits) and, on a recipe failure, writes `<label>.recipe-failure.json`.
//!
//! `--coef-form factored` (coef-form prototype) writes the payload's
//! coefficient section in the factored format
//! (`rustred::persistence::FACTORED_TABLE_VERSION`): every reader decodes it to
//! the identical coefficients; the load-time check runs on the written bytes.
//! `--coef-form shared` writes the same container WITHOUT factoring (one
//! dictionary of the distinct whole numerators and denominators, encounter
//! order; `encode_shared_table`): no factoring cost, bytes determined by the
//! coefficient table alone; checked to decode identically like `factored`.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rustred::persistence::{
    BinaryIoLimits, BinarySection, CoefficientId, DecodedCoefficientTable, FACTORED_TABLE_VERSION,
    SectionTag, encode_factored_table, encode_program, encode_shared_table, inspect_program,
};
use rustred::propagator::PresentationId;
use rustred::propagator::certified_keys::CertifiedKeySet;
use rustred::propagator::custom::{
    CustomPayloadCheck, custom_payload_boxed_node, custom_payload_load, inspect_custom_payload,
    replay_custom_payload, verify_custom_payload,
};
use rustred::propagator::error::PError;
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot, SlotIsp, SlotLine};
use rustred::propagator::recipes::{
    EngineRecipeSolver, RecipeBundle, RecipeCacheRoots, RecipeDispatchOrder, RecipeImport,
    recipe_statistics,
};
use rustred::propagator::table_import::{
    P_TABLE_IMPORT_VERSION, TableImportOptions, canonical_node_graph, import_table_from,
    slot_map_text,
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
    coef_form: CoefForm,
    partial_box: bool,
    mixed_box: bool,
    no_verify: bool,
    certified_keys: Option<PathBuf>,
    recipes: Option<PathBuf>,
    recipe_revision: Option<String>,
    recipes_first: bool,
    stream_payload: bool,
}

fn parse(arguments: Vec<OsString>) -> Result<Arguments, ArgError> {
    let mut arguments = arguments.into_iter();
    let (mut table, mut graph, mut ext, mut label) = (None, None, None, None);
    let (mut lp, mut out, mut force, mut replay) = (None, None, false, None);
    let (mut census_family, mut order_map) = (None, None);
    let mut coef_form = CoefForm::Expanded;
    let (mut partial_box, mut no_verify) = (false, false);
    let mut mixed_box = false;
    let mut stream_payload = false;
    let mut certified_keys = None;
    let (mut recipes, mut recipe_revision, mut recipes_first) = (None, None, false);
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
            "--coef-form" => "--coef-form",
            "--partial-box" => "--partial-box",
            "--no-verify" => "--no-verify",
            "--mixed-box" => "--mixed-box",
            "--certified-keys" => "--certified-keys",
            "--recipes" => "--recipes",
            "--recipe-revision" => "--recipe-revision",
            "--recipes-first" => "--recipes-first",
            "--stream-payload" => "--stream-payload",
            _ => return Err(ArgError::UnknownOption(argument)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--table" => table = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--out" => out = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--replay" => replay = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--certified-keys" => {
                certified_keys = Some(PathBuf::from(next_value(&mut arguments, option)?))
            }
            "--recipes" => recipes = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--recipe-revision" => recipe_revision = Some(next_utf8_value(&mut arguments, option)?),
            "--recipes-first" => recipes_first = true,
            "--census-family" => {
                census_family = Some(PathBuf::from(next_value(&mut arguments, option)?))
            }
            "--order-map" => order_map = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--coef-form" => {
                let value = next_utf8_value(&mut arguments, option)?;
                coef_form = match value.as_str() {
                    "factored" => CoefForm::Factored,
                    "shared" => CoefForm::Shared,
                    "expanded" => CoefForm::Expanded,
                    _ => {
                        return Err(ArgError::InvalidValue {
                            option,
                            value: value.clone(),
                            expected: "expanded, factored or shared",
                        });
                    }
                };
            }
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
            "--partial-box" => partial_box = true,
            "--no-verify" => no_verify = true,
            "--mixed-box" => mixed_box = true,
            "--stream-payload" => stream_payload = true,
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
        coef_form,
        partial_box,
        mixed_box,
        no_verify,
        certified_keys,
        recipes,
        recipe_revision,
        recipes_first,
        stream_payload,
    })
}

pub(super) fn failed(error: PError) -> CliError {
    CliError::Application(AppError::derivation(format!("p import-table: {error}")))
}

fn json_error(error: serde_json::Error) -> CliError {
    CliError::OutputIo(error.to_string())
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    if arguments.first().is_some_and(|first| first == "--check") {
        return run_check(arguments);
    }
    let arguments = parse(arguments).map_err(CliError::Usage)?;
    if !arguments
        .label
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(CliError::Input(format!("label {:?}", arguments.label)));
    }
    let destination = |extension: &str| {
        StreamPath::File(
            arguments
                .out
                .join(format!("{}.{extension}", arguments.label)),
        )
    };
    let mut outputs = vec!["rrbin", "json"];
    if arguments.replay.is_some() {
        outputs.push("replay.json");
    }
    std::fs::create_dir_all(&arguments.out).map_err(|error| {
        CliError::OutputIo(format!(
            "cannot create {}: {error}",
            arguments.out.display()
        ))
    })?;
    for extension in &outputs {
        preflight_output_destination(&destination(extension), arguments.force)?;
    }
    let started = Instant::now();
    // Streaming (t162-export): the table is read record by record, never held.
    let table = std::fs::File::open(&arguments.table).map_err(|error| {
        CliError::InputIo(format!(
            "cannot open input {}: {error}",
            arguments.table.display()
        ))
    })?;
    let (_key, graph) =
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
        partial_box: arguments.partial_box,
        mixed_box: arguments.mixed_box,
        certified_keys: match &arguments.certified_keys {
            Some(path) => Some(read_certified_keys(path, &arguments.table)?),
            None => None,
        },
        recipes: match &arguments.recipes {
            Some(path) => Some(RecipeImport {
                revision: arguments
                    .recipe_revision
                    .clone()
                    .unwrap_or_else(|| format!("{}-r1", arguments.label)),
                dispatch: if arguments.recipes_first {
                    RecipeDispatchOrder::RecipesFirst
                } else {
                    RecipeDispatchOrder::RulesFirst
                },
                records: read_recipe_records(path)?,
            }),
            None => None,
        },
    };
    // vk5l WP6: --stream-payload (requires --no-verify, the expanded --coef-form,
    // no --replay): coefficients spilled to a scratch file beside the output,
    // the payload written to `<label>.rrbin.partial` while it is built and
    // renamed at the end; memory = the hash index + the program records.
    let mut table = Some(table);
    let streamed = if arguments.stream_payload {
        if !arguments.no_verify
            || arguments.coef_form != CoefForm::Expanded
            || arguments.replay.is_some()
        {
            return Err(CliError::Input(
                "--stream-payload needs --no-verify and allows neither --coef-form factored / shared nor --replay"
                    .to_owned(),
            ));
        }
        let path = |extension: &str| {
            arguments
                .out
                .join(format!("{}.{extension}", arguments.label))
        };
        let io = |what: &std::path::Path, error: std::io::Error| {
            CliError::OutputIo(format!("{}: {error}", what.display()))
        };
        let (scratch_path, partial_path) = (path("atoms.scratch"), path("rrbin.partial"));
        let scratch = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&scratch_path)
            .map_err(|error| io(&scratch_path, error))?;
        let partial =
            std::fs::File::create(&partial_path).map_err(|error| io(&partial_path, error))?;
        let mut writer = std::io::BufWriter::with_capacity(8 << 20, partial);
        let result = rustred::propagator::table_import::import_table_streamed(
            std::io::BufReader::with_capacity(1 << 20, table.take().expect("the table file")),
            &graph,
            &options,
            scratch,
            &mut writer,
        );
        let _ = std::fs::remove_file(&scratch_path);
        let imported = result.map_err(failed)?;
        use std::io::Write as _;
        writer
            .flush()
            .and_then(|()| writer.get_ref().sync_all())
            .map_err(|error| io(&partial_path, error))?;
        Some((imported, partial_path))
    } else {
        None
    };
    let (mut imported, partial_payload) = match streamed {
        Some((imported, partial)) => (imported, Some(partial)),
        None => (
            import_table_from(
                std::io::BufReader::with_capacity(1 << 20, table.take().expect("the table file")),
                &graph,
                &options,
            )
            .map_err(failed)?,
            None,
        ),
    };
    let import_seconds = started.elapsed().as_secs_f64();
    let expanded_payload_bytes = imported.payload_len();
    let mut coef_form_seconds = 0.0;
    if arguments.coef_form != CoefForm::Expanded {
        let started = Instant::now();
        imported.payload = factored_payload(&imported.payload, arguments.coef_form)?;
        coef_form_seconds = started.elapsed().as_secs_f64();
    }
    let verify_started = Instant::now();
    let check = if arguments.no_verify {
        None
    } else {
        Some(
            verify_custom_payload(&imported.presentation, &imported.payload, &imported.meta)
                .map_err(failed)?,
        )
    };
    let verify_seconds = verify_started.elapsed().as_secs_f64();

    let presentation = &imported.presentation;
    let slots: Vec<serde_json::Value> = presentation
        .slots
        .iter()
        .map(|slot| match slot {
            Slot::Line(line) => {
                serde_json::json!({"line": line.momentum, "eps_units": line.eps_units})
            }
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
        "payload_bytes": imported.payload_len(),
        "verify": check.as_ref().map(check_json),
        "import_seconds": import_seconds,
        "verify_seconds": verify_seconds,
        "versions": {
            "P_TABLE_IMPORT_VERSION": P_TABLE_IMPORT_VERSION,
            "P_CUSTOM_OWNER_VERSION": P_CUSTOM_OWNER_VERSION,
            "P_OWNER_LIBRARY_SEMANTICS": P_OWNER_LIBRARY_SEMANTICS,
        },
    });
    record["slot_order"] = serde_json::to_value(&imported.slot_order).map_err(json_error)?;
    if let Some(boxed) = &imported.boxed {
        record["boxed"] = serde_json::to_value(boxed).map_err(json_error)?;
    }
    if let Some(bundle) = &imported.recipes {
        record["recipes"] = recipes_summary(bundle);
    }
    if arguments.coef_form != CoefForm::Expanded {
        record["coef_form"] = serde_json::json!({
            "form": arguments.coef_form.name(),
            "FACTORED_TABLE_VERSION": FACTORED_TABLE_VERSION,
            "seconds": coef_form_seconds,
            "expanded_payload_bytes": expanded_payload_bytes,
        });
    }
    record["census_family"] = serde_json::json!(
        arguments
            .census_family
            .as_ref()
            .map(|p| p.display().to_string())
    );
    record["order_map"] = serde_json::json!(
        arguments
            .order_map
            .as_ref()
            .map(|p| p.display().to_string())
    );

    if let Some(seeds_path) = &arguments.replay {
        let seeds_text = read_input(&StreamPath::File(seeds_path.clone()))?;
        let table_seeds: Vec<Vec<i64>> = serde_json::from_str(&seeds_text)
            .map_err(|error| CliError::Input(format!("{}: {error}", seeds_path.display())))?;
        let arity = imported.slot_map.len();
        let to_presentation = |key: &[i64]| -> Result<Vec<i64>, CliError> {
            if key.len() != arity {
                return Err(CliError::Input(format!(
                    "seed {key:?} has not {arity} slots"
                )));
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
        let replays =
            replay_custom_payload(presentation, &imported.payload, &imported.meta, &seeds)
                .map_err(failed)?;
        let replay_seconds = replay_started.elapsed().as_secs_f64();
        let terms =
            |terms: &[(Vec<i64>, rustred::algebra::Coefficient)]| -> Vec<serde_json::Value> {
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
        write_output(
            &destination("replay.json"),
            format!("{text}\n").as_bytes(),
            arguments.force,
        )?;
    }

    let json = serde_json::to_string_pretty(&record).map_err(json_error)?;
    match &partial_payload {
        Some(partial) => {
            let StreamPath::File(target) = destination("rrbin") else {
                unreachable!("file destinations")
            };
            if target.exists() && !arguments.force {
                return Err(CliError::OutputIo(format!("{} exists", target.display())));
            }
            std::fs::rename(partial, &target)
                .map_err(|error| CliError::OutputIo(format!("{}: {error}", target.display())))?;
        }
        None => write_output(&destination("rrbin"), &imported.payload, arguments.force)?,
    }
    write_output(
        &destination("json"),
        format!("{json}\n").as_bytes(),
        arguments.force,
    )?;
    writeln!(
        std::io::stdout().lock(),
        "{}: {} rules over {} charts, {} residuals (LP {}), {} bytes; slot map {:?}{}; order {}{}; verify {} in {:.3} s; import {:.3} s",
        arguments.label,
        imported.rules,
        imported.summary.charts.len(),
        imported.meta.residuals,
        imported.meta.lp_count,
        imported.payload_len(),
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
        check.as_ref().map_or_else(|| "skipped (--no-verify)".to_owned(), check_text),
        verify_seconds,
        import_seconds,
    )
    .map_err(|error| CliError::OutputIo(error.to_string()))
}

/// vk5l WP2: the recipe records of `--recipes` (one JSON object per line).
pub(super) fn read_recipe_records(
    path: &std::path::Path,
) -> Result<Vec<rustred::propagator::recipes::RecipeRecord>, CliError> {
    let text = read_input(&StreamPath::File(path.to_path_buf()))?;
    let mut records = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let at = |detail: String| {
            CliError::Input(format!("{}:{}: {detail}", path.display(), line_number + 1))
        };
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|error| at(error.to_string()))?;
        let (recipe, extra) = if value.get("schema").is_some() {
            (value.clone(), serde_json::Value::Null)
        } else {
            (value["recipe"].clone(), value.clone())
        };
        let mut record =
            RecipeBundle::record_of(&recipe.to_string()).map_err(|error| at(error.to_string()))?;
        if let Some(digests) = extra["relation_digests"].as_object() {
            for (key, digest) in digests {
                let digest = digest
                    .as_str()
                    .ok_or_else(|| at("a relation digest is not a string".into()))?;
                record
                    .relation_digests
                    .insert(key.clone(), digest.to_owned());
            }
        }
        if let Some(rows) = extra["max_trace_rows"].as_u64() {
            record.max_trace_rows = Some(rows as usize);
        }
        records.push(record);
    }
    if records.is_empty() {
        return Err(CliError::Input(format!(
            "{}: no recipe records",
            path.display()
        )));
    }
    Ok(records)
}

/// vk5l WP2: the recipe bundle in one JSON object (import record).
pub(super) fn recipes_summary(bundle: &RecipeBundle) -> serde_json::Value {
    use rustred::propagator::recipes::RecipeDomain;
    let keys: usize = bundle
        .records
        .iter()
        .map(|record| match &record.domain {
            RecipeDomain::Keys(keys) => keys.len(),
            RecipeDomain::Predicate => 0,
        })
        .sum();
    serde_json::json!({
        "schema": bundle.schema,
        "P_RECIPE_VERSION": bundle.version,
        "node": bundle.node,
        "revision": bundle.revision,
        "dispatch": bundle.dispatch,
        "records": bundle.records.len(),
        "explicit_keys": keys,
        "predicate_records": bundle
            .records
            .iter()
            .filter(|record| record.domain == RecipeDomain::Predicate)
            .count(),
        "ids": bundle
            .records
            .iter()
            .map(|record| serde_json::json!({"id": record.id, "digest": record.recipe_digest, "kind": record.kind()}))
            .collect::<Vec<_>>(),
        "determinism_id": rustred::node::recipe::DETERMINISM_ID,
    })
}

/// vk5l WP1: the certified key set of `--certified-keys`, in the table's
/// slot order: its arity and node from the table's header line.
fn read_certified_keys(
    path: &std::path::Path,
    table: &std::path::Path,
) -> Result<CertifiedKeySet, CliError> {
    use std::io::BufRead;
    let file = std::fs::File::open(table).map_err(|error| {
        CliError::InputIo(format!("cannot open input {}: {error}", table.display()))
    })?;
    let mut line = String::new();
    std::io::BufReader::new(file)
        .read_line(&mut line)
        .map_err(|error| CliError::InputIo(format!("{}: {error}", table.display())))?;
    let header: serde_json::Value = serde_json::from_str(&line)
        .map_err(|error| CliError::Input(format!("{}: header: {error}", table.display())))?;
    let arity = header["mask"]
        .as_str()
        .map(str::len)
        .ok_or_else(|| CliError::Input(format!("{}: header without mask", table.display())))?;
    let node = header["boxed_node"]["node"]
        .as_str()
        .or_else(|| header["family"].as_str())
        .unwrap_or("?");
    let bytes = std::fs::read(path)
        .map_err(|error| CliError::InputIo(format!("cannot read {}: {error}", path.display())))?;
    CertifiedKeySet::read(&bytes, node, arity, "table").map_err(failed)
}

/// vk5l WP1: `VmRSS` and `VmHWM` of this process in bytes.
fn rss_bytes() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<u64>().ok())
            .map_or(0, |kb| kb * 1024)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

fn check_json(check: &CustomPayloadCheck) -> serde_json::Value {
    serde_json::json!({
        "charts": check.charts,
        "rules": check.rules,
        "support_primary": check.support_primary,
        "witness_keys": check.witness_keys,
        "same_sector_children": check.same_sector_children,
        "subsector_children": check.subsector_children,
        "terminals": check.terminals,
        "open_box_keys": check.open_box_keys,
        "box_keys_stepped": check.box_keys_stepped,
        "certified_keys": check.certified_keys,
        "recipe_keys": check.recipe_keys,
    })
}

fn check_text(check: &CustomPayloadCheck) -> String {
    format!(
        "{} witness keys ({} same-sector, {} subsector children, {} terminals, {} open box keys, {} box keys stepped)",
        check.witness_keys,
        check.same_sector_children,
        check.subsector_children,
        check.terminals,
        check.open_box_keys,
        check.box_keys_stepped
    )
}

/// vk5l WP2: an imported payload loaded from its import record.
pub(super) struct LoadedOwner {
    pub label: String,
    pub presentation: Presentation,
    pub payload: Vec<u8>,
    pub payload_path: PathBuf,
    pub meta: rustred::propagator::library::CustomOwnerMeta,
    /// Table slot -> presentation slot.
    pub slot_map: Vec<usize>,
    pub read_seconds: f64,
}

/// The payload `<dir>/<label>.rrbin` of an import record, its presentation
/// rebuilt from the record's slots (family fingerprint compared).
pub(super) fn load_owner(record_path: &std::path::Path) -> Result<LoadedOwner, CliError> {
    let record: serde_json::Value =
        serde_json::from_str(&read_input(&StreamPath::File(record_path.to_path_buf()))?)
            .map_err(|error| CliError::Input(format!("{}: {error}", record_path.display())))?;
    let field = |name: &str| {
        record
            .get(name)
            .ok_or_else(|| CliError::Input(format!("{}: no field {name:?}", record_path.display())))
    };
    if field("schema")?.as_str() != Some(P_IMPORTED_OWNER_SCHEMA) {
        return Err(CliError::Input(format!(
            "{}: not a {P_IMPORTED_OWNER_SCHEMA} record",
            record_path.display()
        )));
    }
    let bad = |name: &str| CliError::Input(format!("{}: bad {name}", record_path.display()));
    let numbers = |value: &serde_json::Value| -> Option<Vec<i64>> {
        value
            .as_array()?
            .iter()
            .map(serde_json::Value::as_i64)
            .collect()
    };
    let slots = (field("presentation_slots")?
        .as_array()
        .ok_or_else(|| bad("slots"))?
        .iter())
    .map(|slot| {
        if let Some(line) = slot.get("line") {
            Some(Slot::Line(SlotLine {
                momentum: numbers(line)?,
                eps_units: u8::try_from(slot.get("eps_units")?.as_u64()?).ok()?,
            }))
        } else {
            let isp = slot.get("isp")?.as_array()?;
            Some(Slot::Isp(SlotIsp {
                left: numbers(isp.first()?)?,
                right: numbers(isp.get(1)?)?,
            }))
        }
    })
    .collect::<Option<Vec<_>>>()
    .ok_or_else(|| bad("presentation_slots"))?;
    let name = field("table_family")?
        .as_str()
        .ok_or_else(|| bad("table_family"))?;
    let loops = (field("loops")?.as_u64())
        .and_then(|loops| u8::try_from(loops).ok())
        .ok_or_else(|| bad("loops"))?;
    let lp_count = (field("lp_count")?.as_u64())
        .and_then(|count| u32::try_from(count).ok())
        .ok_or_else(|| bad("lp_count"))?;
    let label = field("label")?
        .as_str()
        .ok_or_else(|| bad("label"))?
        .to_owned();
    let slot_map: Vec<usize> = field("slot_map")?
        .as_array()
        .ok_or_else(|| bad("slot_map"))?
        .iter()
        .map(|value| value.as_u64().map(|value| value as usize))
        .collect::<Option<_>>()
        .ok_or_else(|| bad("slot_map"))?;
    let payload_path = record_path.with_file_name(format!("{label}.rrbin"));
    let context = rustred::algebra::CoefficientContext::try_new(["d"])
        .map_err(|error| CliError::Input(format!("context: {error}")))?;
    let presentation = Presentation::from_slots(
        PresentationId(0),
        PresentationKind::Caller,
        name,
        loops,
        slots,
        &context,
        None,
    )
    .map_err(failed)?;
    if Some(presentation.family.fingerprint()) != field("family_fingerprint")?.as_str() {
        return Err(CliError::Input(format!(
            "{}: the record's slots do not give its family fingerprint",
            record_path.display()
        )));
    }
    let read_started = Instant::now();
    let payload = std::fs::read(&payload_path).map_err(|error| {
        CliError::InputIo(format!("cannot read {}: {error}", payload_path.display()))
    })?;
    let read_seconds = read_started.elapsed().as_secs_f64();
    let meta = (inspect_custom_payload(&payload).and_then(|summary| summary.meta(lp_count)))
        .map_err(failed)?;
    Ok(LoadedOwner {
        label,
        presentation,
        payload,
        payload_path,
        meta,
        slot_map,
        read_seconds,
    })
}

/// vk5l WP2: `--recipe-cache`, `--recipe-prefilled`, `--recipe-workers` of a
/// command that steps recipe keys: the cache roots (else the environment's)
/// and the engine solver of the payload's node.
pub(super) fn install_recipe_options(
    payload: &[u8],
    user: Option<PathBuf>,
    prefilled: Option<String>,
    workers: Option<usize>,
) -> Result<(), CliError> {
    if user.is_some() || prefilled.is_some() {
        let mut roots = rustred::propagator::recipes::recipe_cache_roots();
        if let Some(user) = user {
            roots.user = Some(user);
        }
        if let Some(list) = prefilled {
            roots.prefilled = list
                .split([',', ':'])
                .filter(|part| !part.is_empty())
                .map(PathBuf::from)
                .collect();
        }
        rustred::propagator::recipes::set_recipe_cache_roots(RecipeCacheRoots {
            user: roots.user,
            prefilled: roots.prefilled,
            ..Default::default()
        });
    }
    if let Some(workers) = workers
        && let Some(boxed) = custom_payload_boxed_node(payload).map_err(failed)?
    {
        rustred::propagator::recipes::override_recipe_solver(
            &boxed.node,
            std::sync::Arc::new(EngineRecipeSolver::new(workers)),
        );
    }
    Ok(())
}

/// vk5l WP2: the recipe counters of this process as JSON.
pub(super) fn recipe_statistics_json() -> serde_json::Value {
    let stats = recipe_statistics();
    let relations = rustred::propagator::relation_entries::relation_cache_statistics();
    let relation_cache = serde_json::json!({
        "shipped_hits": relations.shipped_hits, "cache_hits": relations.cache_hits,
        "misses": relations.misses, "inserts": relations.inserts,
        "inserts_skipped_cap": relations.inserts_skipped_cap, "foreign": relations.foreign,
        "corrupt": relations.corrupt, "bytes_written": relations.bytes_written,
    });
    let roots = rustred::propagator::recipes::recipe_cache_roots();
    serde_json::json!({
        "solves": stats.solves,
        "solve_seconds": stats.solve_seconds,
        "memo_hits": stats.memo_hits,
        "prefilled_hits": stats.prefilled_hits,
        "user_cache_hits": stats.user_cache_hits,
        "failures": stats.failures,
        "outside_solves": stats.outside_solves,
        "outside_failures": stats.outside_failures,
        "outside_rule_steps": stats.outside_rule_steps,
        "outside_entry_hits": stats.outside_entry_hits,
        "outside_budget_hits": stats.outside_budget_hits,
        "relation_cache": relation_cache,
        "cache_roots": {
            "user": roots.user.map(|path| path.display().to_string()),
            "prefilled": roots.prefilled.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
        },
    })
}

/// vk5l WP2: a recipe failure written as `<path>` (JSON) next to its error.
pub(super) fn recipe_failure_out(error: PError, path: &std::path::Path) -> CliError {
    if let PError::RecipeFailed(failure) = &error {
        let text = serde_json::to_string_pretty(&failure.to_json()).unwrap_or_default();
        let _ = std::fs::write(path, format!("{text}\n"));
        let _ = writeln!(
            std::io::stderr().lock(),
            "rustred: recipe failure record written to {}",
            path.display()
        );
    }
    failed(error)
}

/// `--check <dir>/<label>.json [--force]`: the load-time check of an imported
/// payload as its own job (t162-export).
fn run_check(arguments: Vec<OsString>) -> Result<(), CliError> {
    let mut arguments = arguments.into_iter().skip(1);
    let record_path =
        PathBuf::from(next_value(&mut arguments, "--check").map_err(CliError::Usage)?);
    let mut force = false;
    // vk5l WP6: --load-only measures the owner build (load memory and time)
    // and writes the check record without stepping any key.
    let mut load_only = false;
    let (mut cache, mut prefilled, mut workers) = (None, None, None);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--force") if !force => force = true,
            Some("--load-only") if !load_only => load_only = true,
            Some("--recipe-cache") if cache.is_none() => {
                cache = Some(PathBuf::from(
                    next_value(&mut arguments, "--recipe-cache").map_err(CliError::Usage)?,
                ));
            }
            Some("--recipe-prefilled") if prefilled.is_none() => {
                prefilled = Some(
                    next_utf8_value(&mut arguments, "--recipe-prefilled")
                        .map_err(CliError::Usage)?,
                );
            }
            Some("--recipe-workers") if workers.is_none() => {
                let value =
                    next_utf8_value(&mut arguments, "--recipe-workers").map_err(CliError::Usage)?;
                workers = Some(value.parse().map_err(|_| {
                    CliError::Usage(ArgError::InvalidValue {
                        option: "--recipe-workers",
                        value: value.clone(),
                        expected: "a positive worker count",
                    })
                })?);
            }
            _ => {
                return Err(CliError::Usage(ArgError::UnknownOption(
                    argument.to_string_lossy().into_owned(),
                )));
            }
        }
    }
    let (rss_start, _) = rss_bytes();
    let loaded = load_owner(&record_path)?;
    let LoadedOwner {
        label,
        presentation,
        payload,
        payload_path,
        meta,
        read_seconds,
        ..
    } = &loaded;
    let check_path = record_path.with_file_name(format!("{label}.check.json"));
    preflight_output_destination(&StreamPath::File(check_path.clone()), force)?;
    install_recipe_options(payload, cache, prefilled, workers)?;
    let (rss_read, _) = rss_bytes();
    // vk5l WP1 (e): the load memory of the owner alone (program built, every
    // chart prepared, certified set decoded), before the check steps keys.
    let built = custom_payload_load(presentation, payload, meta).map_err(failed)?;
    let (rss_built, peak_built) = rss_bytes();
    if load_only {
        let gb = |bytes: u64| bytes as f64 / 1e9;
        let out = serde_json::json!({
            "schema": "rustred.p-imported-owner-load.v1",
            "label": label,
            "payload": payload_path.display().to_string(),
            "payload_bytes": payload.len(),
            "load": {
                "payload_bytes": payload.len(),
                "read_seconds": read_seconds,
                "build_seconds": built.build_seconds,
                "node": built.node,
                "certified_keys": built.certified_keys,
                "certified_heap_bytes": built.certified_heap_bytes,
                "rss_start_gb": gb(rss_start),
                "rss_after_read_gb": gb(rss_read),
                "rss_after_build_gb": gb(rss_built),
                "peak_after_build_gb": gb(peak_built),
                "note": "the payload bytes are held (eager read); the owner program is built, no key is stepped",
            },
        });
        let text = serde_json::to_string_pretty(&out).map_err(json_error)?;
        write_output(
            &StreamPath::File(record_path.with_file_name(format!("{label}.load.json"))),
            format!("{text}\n").as_bytes(),
            force,
        )?;
        return writeln!(
            std::io::stdout().lock(),
            "{label}: load of {} bytes: read {read_seconds:.3} s, build {:.3} s, RSS after build {:.2} GB (peak {:.2} GB)",
            payload.len(),
            built.build_seconds,
            gb(rss_built),
            gb(peak_built)
        )
        .map_err(|error| CliError::OutputIo(error.to_string()));
    }
    let started = Instant::now();
    let failure_path = record_path.with_file_name(format!("{label}.recipe-failure.json"));
    let check = verify_custom_payload(presentation, payload, meta)
        .map_err(|error| recipe_failure_out(error, &failure_path))?;
    let seconds = started.elapsed().as_secs_f64();
    let (rss_end, peak) = rss_bytes();
    let gb = |bytes: u64| bytes as f64 / 1e9;
    let mut out = serde_json::json!({
        "schema": "rustred.p-imported-owner-check.v1",
        "label": label,
        "payload": payload_path.display().to_string(),
        "payload_bytes": payload.len(),
        "verify": check_json(&check),
        "verify_seconds": seconds,
        "load": {
            "payload_bytes": payload.len(),
            "read_seconds": read_seconds,
            "build_seconds": built.build_seconds,
            "node": built.node,
            "certified_keys": built.certified_keys,
            "certified_heap_bytes": built.certified_heap_bytes,
            "rss_start_gb": gb(rss_start),
            "rss_after_read_gb": gb(rss_read),
            "rss_after_build_gb": gb(rss_built),
            "peak_after_build_gb": gb(peak_built),
            "rss_after_check_gb": gb(rss_end),
            "peak_gb": gb(peak),
            "note": "the payload bytes are held during the check (eager read); a library load reads a lazy payload on use",
        },
    });
    if check.recipe_keys > 0 {
        out["recipes"] = recipe_statistics_json();
        out["recipes"]["recipe_keys"] = serde_json::json!(check.recipe_keys);
    }
    let text = serde_json::to_string_pretty(&out).map_err(json_error)?;
    write_output(
        &StreamPath::File(check_path),
        format!("{text}\n").as_bytes(),
        force,
    )?;
    writeln!(
        std::io::stdout().lock(),
        "{label}: check of {} bytes: {} in {seconds:.3} s",
        payload.len(),
        check_text(&check)
    )
    .map_err(|error| CliError::OutputIo(error.to_string()))
}

/// Coefficient section form of the written payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoefForm {
    /// one native Symbolica atom per coefficient (the importer's own output)
    Expanded,
    /// factored container, irreducible factors (`encode_factored_table`)
    Factored,
    /// factored container, whole polynomials, no factoring (`encode_shared_table`)
    Shared,
}

impl CoefForm {
    fn name(self) -> &'static str {
        match self {
            CoefForm::Expanded => "expanded",
            CoefForm::Factored => "factored",
            CoefForm::Shared => "shared",
        }
    }
}

/// The payload with its coefficient section re-encoded in `form` (factored or
/// shared), every other section byte-identical; checked to decode to the
/// identical coefficients.
fn factored_payload(payload: &[u8], form: CoefForm) -> Result<Vec<u8>, CliError> {
    let io = |error: rustred::persistence::BinaryIoError| {
        CliError::Input(format!("coef-form factored payload: {error:?}"))
    };
    let limits = BinaryIoLimits::default();
    let envelope = inspect_program(payload, limits).map_err(io)?;
    let section = |tag| {
        envelope.section(tag).ok_or_else(|| {
            CliError::Input("coef-form: payload without coefficient sections".into())
        })
    };
    let state = section(SectionTag::SYMBOLICA_STATE)?;
    let atoms = section(SectionTag::COEFFICIENTS)?;
    let table = DecodedCoefficientTable::import_generated(state, atoms, limits).map_err(io)?;
    let coefficients = table.into_coefficients().map_err(io)?;
    let factored = match form {
        CoefForm::Shared => encode_shared_table(&coefficients),
        _ => encode_factored_table(&coefficients),
    }
    .map_err(io)?;
    let sections: Vec<BinarySection<'_>> = envelope
        .sections()
        .iter()
        .map(|s| BinarySection {
            tag: s.tag,
            bytes: if s.tag == SectionTag::COEFFICIENTS {
                &factored
            } else {
                s.bytes
            },
        })
        .collect();
    let output = encode_program(envelope.kind(), &sections, limits).map_err(io)?;
    let check = inspect_program(&output, limits).map_err(io)?;
    let decoded = DecodedCoefficientTable::import_generated(
        check
            .section(SectionTag::SYMBOLICA_STATE)
            .ok_or_else(|| CliError::Input("coef-form: state section lost".into()))?,
        check
            .section(SectionTag::COEFFICIENTS)
            .ok_or_else(|| CliError::Input("coef-form: coefficient section lost".into()))?,
        limits,
    )
    .map_err(io)?;
    if decoded.len() != coefficients.len() {
        return Err(CliError::Input(
            "coef-form: coefficient count changed".into(),
        ));
    }
    for (i, original) in coefficients.iter().enumerate() {
        let id = CoefficientId::try_from_index(i).map_err(io)?;
        let value = decoded.coefficient(id).map_err(io)?;
        if value != original || value.get_variables() != original.get_variables() {
            return Err(CliError::Input(format!(
                "coef-form: coefficient {i} differs"
            )));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_import_table_parses_its_options() {
        let parsed = parse(
            [
                "--table",
                "t.jsonl",
                "--graph",
                "0-1* 0-2",
                "--ext",
                "2,3",
                "--label",
                "T012",
                "--lp",
                "1",
                "--out",
                "o",
                "--replay",
                "s.json",
                "--partial-box",
            ]
            .iter()
            .map(OsString::from)
            .collect(),
        )
        .unwrap();
        assert_eq!((parsed.ext, parsed.lp, parsed.force), ((2, 3), 1, false));
        assert!(parsed.partial_box);
        assert!(!parsed.no_verify);
        assert_eq!(parsed.coef_form, CoefForm::Expanded);
        let base = [
            "--table", "t", "--graph", "0-1", "--ext", "0,1", "--label", "x", "--lp", "0", "--out",
            "o",
        ];
        for (value, form) in [
            ("shared", CoefForm::Shared),
            ("factored", CoefForm::Factored),
            ("expanded", CoefForm::Expanded),
        ] {
            let args = base
                .iter()
                .chain(["--coef-form", value].iter())
                .map(OsString::from)
                .collect();
            assert_eq!(parse(args).unwrap().coef_form, form);
        }
        let bad = base
            .iter()
            .chain(["--coef-form", "zip"].iter())
            .map(OsString::from)
            .collect();
        assert!(parse(bad).is_err());
        let no_verify = parse(
            [
                "--table",
                "t",
                "--graph",
                "0-1",
                "--ext",
                "0,1",
                "--label",
                "x",
                "--lp",
                "0",
                "--out",
                "o",
                "--no-verify",
            ]
            .iter()
            .map(OsString::from)
            .collect(),
        )
        .unwrap();
        assert!(no_verify.no_verify);
        assert!(matches!(
            parse(vec![OsString::from("--table"), OsString::from("t")]),
            Err(ArgError::MissingRequiredOption(_))
        ));
    }
}
