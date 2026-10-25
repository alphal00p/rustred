//! `rustred saved-application compose-layer` (RR-11 design §3-§4).
//!
//! Load an immutable saved application, bind a relations layer to it, compose
//! the terminal maps exactly (`raw -> remaining -> masters`), check the result
//! modulo a 61-bit prime, apply the size gate and export either a composed
//! package (unchanged schema and codec) or, as the fallback, a layered package.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rustred::reduction::terminal_normalization::{VacuumProductChartLimits, VacuumProductLimits};
use serde_json::{Value, json};

use super::layer::{
    self, BoundLayer, SavedApplicationLayer, SavedApplicationLayerDigest, bind_layer,
    check_composition_mod_p, compose_maps, layer_minimality_claim, layer_summary,
};
use super::{
    PreparedSavedApplication, SavedApplicationIntegral, SavedApplicationOptions, TerminalLayer,
    TerminalMaps, TerminalRow, codec, load_saved_application, native_failure,
};
use crate::AppError;
use crate::application::atomic_file::write_file_atomically;

/// Options of one composition.
#[derive(Clone, Debug)]
pub struct SavedApplicationComposeOptions {
    pub application: SavedApplicationOptions,
    /// Composed decoded maps may be at most this multiple of the base maps.
    pub size_gate: f64,
    /// Write a layered package when the size gate fails (needs a layer-aware loader).
    pub allow_layered: bool,
    /// Compute and report only; write nothing.
    pub dry_run: bool,
    /// Random points of the mod-p self-check over all raw rows.
    pub self_check_points: usize,
    /// Raw rows re-checked after reloading the written package.
    pub reload_check_rows: usize,
    /// Base `rustred.native-master-definitions.v1`; default `<application>/../terminal-definitions.json`.
    pub terminal_definitions: Option<PathBuf>,
    /// Output definitions; default `<out>.terminal-definitions.json`.
    pub terminal_definitions_out: Option<PathBuf>,
}

impl Default for SavedApplicationComposeOptions {
    fn default() -> Self {
        Self {
            application: SavedApplicationOptions::default(),
            size_gate: 2.0,
            allow_layered: false,
            dry_run: false,
            self_check_points: 3,
            reload_check_rows: 16,
            terminal_definitions: None,
            terminal_definitions_out: None,
        }
    }
}

/// What `compose_saved_application_layer` decided and wrote.
#[derive(Clone, Debug)]
pub struct SavedApplicationComposeOutcome {
    /// `composed`, `layered`, `rejected` (size gate) or `dry-run`.
    pub mode: String,
    pub report: Value,
}

fn read_json(path: &Path) -> Result<(Vec<u8>, Value), AppError> {
    let bytes = fs::read(path).map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
    let value = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
    Ok((bytes, value))
}

fn seed(digest: &str) -> u64 {
    u64::from_str_radix(&digest[..16.min(digest.len())], 16).unwrap_or(0x5eed)
}

fn stratified(keys: &[SavedApplicationIntegral], count: usize) -> Vec<SavedApplicationIntegral> {
    if keys.len() <= count {
        return keys.to_vec();
    }
    (0..count)
        .map(|i| keys[i * keys.len() / count].clone())
        .collect()
}

/// Compose a relations layer onto a saved application and export the result.
pub fn compose_saved_application_layer<const N: usize>(
    application: &Path,
    layer_path: &Path,
    out: &Path,
    options: &SavedApplicationComposeOptions,
) -> Result<SavedApplicationComposeOutcome, AppError> {
    let started = Instant::now();
    if !(options.size_gate.is_finite() && options.size_gate >= 0.0) {
        return Err(AppError::input("size gate must be a nonnegative finite ratio"));
    }
    if !options.dry_run && out.exists() {
        return Err(AppError::input(format!(
            "{} already exists; compose-layer never replaces a package",
            out.display()
        )));
    }
    let manifest = if application.is_dir() {
        application.join("application.json")
    } else {
        application.to_path_buf()
    };
    let application_dir = manifest.parent().unwrap_or(Path::new(".")).to_path_buf();
    let (base_bytes, base_report) = read_json(&manifest)?;
    if base_report.get("terminal_layer").is_some() {
        return Err(AppError::input(
            "composing onto a layered package is not supported; compose onto its base",
        ));
    }
    let base_maps_bytes = base_report["terminal_maps"]["bytes"]
        .as_u64()
        .ok_or_else(|| AppError::input("base application has no terminal_maps.bytes"))?;

    // 1. Load and authenticate the base package.
    let mut prepared = load_saved_application::<N>(&application_dir, options.application)?;
    let load_seconds = started.elapsed().as_secs_f64();

    // 2-3. Family and base identity.
    let layer_json = if layer_path.is_dir() {
        layer_path.join("layer.json")
    } else {
        layer_path.to_path_buf()
    };
    let (_, head) = read_json(&layer_json)?;
    let fingerprint = head["family_fingerprint"]
        .as_str()
        .ok_or_else(|| AppError::input("layer has no family_fingerprint"))?;
    let family = prepared
        .terminals
        .families
        .get(fingerprint)
        .cloned()
        .ok_or_else(|| AppError::input("layer family is absent from the base terminal maps"))?;
    let (layer, digest) = SavedApplicationLayer::read(
        &layer_json,
        &family,
        options.application.reduction_limits.exact_algebra,
        options.application.binary_limits,
    )?;
    let metadata = prepared.metadata.clone();
    let base = &layer.base;
    let mismatch = |field: &str| AppError::input(format!("layer base {field} differs from the package"));
    if Some(base.terminal_maps_blake3.as_str()) != base_report["terminal_maps"]["blake3"].as_str()
    {
        return Err(mismatch("terminal_maps_blake3"));
    }
    if Some(base.source_manifest_blake3.as_str()) != metadata["source_manifest_blake3"].as_str() {
        return Err(mismatch("source_manifest_blake3"));
    }
    if Some(base.source_scope_binding.as_str()) != metadata["source_scope_binding"].as_str() {
        return Err(mismatch("source_scope_binding"));
    }
    if base.raw_terminals != prepared.terminals.rows.len() as u64 {
        return Err(mismatch("raw_terminals"));
    }
    if base.remaining_terminals != prepared.terminals.remaining.len() as u64 {
        return Err(mismatch("remaining_terminals"));
    }
    if let Some(expected) = &base.application_blake3 {
        if *expected != blake3::hash(&base_bytes).to_hex().to_string() {
            return Err(mismatch("application_blake3"));
        }
    }

    // Factorized terminals (product route) may never be sources.
    let product_started = Instant::now();
    prepared.prepare_terminal_products(VacuumProductLimits::default())?;
    prepared.prepare_terminal_product_charts(VacuumProductChartLimits::default())?;
    let factorized: BTreeSet<SavedApplicationIntegral> = prepared
        .terminals
        .remaining
        .iter()
        .filter(|t| {
            prepared.terminal_product(t).is_some() || prepared.terminal_product_chart(t).is_some()
        })
        .cloned()
        .collect();
    let product_seconds = product_started.elapsed().as_secs_f64();

    // 4. Bind; 5-6. compose exactly.
    let limits = options.application.reduction_limits.exact_algebra;
    let bound = bind_layer(&prepared.terminals, &layer, &factorized, limits)?;
    let compose_started = Instant::now();
    let (composed, statistics) = compose_maps(&prepared.terminals, &bound, limits)?;
    composed.validate(options.application)?;
    let compose_seconds = compose_started.elapsed().as_secs_f64();

    // Self-check modulo p over every raw row.
    let check_started = Instant::now();
    let all_rows: Vec<_> = prepared.terminals.rows.keys().cloned().collect();
    let (checked, points) = check_composition_mod_p(
        &prepared.terminals,
        &bound,
        &composed,
        &all_rows,
        options.self_check_points,
        seed(&digest.layer_blake3),
    )?;
    let check_seconds = check_started.elapsed().as_secs_f64();

    // Size gate on the decoded native maps.
    let encode_started = Instant::now();
    let composed_bytes = codec::encode(&composed, options.application)?;
    let composed_len = composed_bytes.len() as u64;
    let composed_blake3 = blake3::hash(&composed_bytes).to_hex().to_string();
    let identical_bytes = composed_blake3
        == base_report["terminal_maps"]["blake3"]
            .as_str()
            .unwrap_or_default();
    drop(composed_bytes);
    let encode_seconds = encode_started.elapsed().as_secs_f64();
    let ratio = composed_len as f64 / base_maps_bytes.max(1) as f64;
    let gate_passed = ratio <= options.size_gate;
    let mode = if options.dry_run {
        "dry-run"
    } else if gate_passed {
        "composed"
    } else if options.allow_layered {
        "layered"
    } else {
        "rejected"
    };
    let (minimality_claim, minimality_scope) = layer_minimality_claim(&layer);
    let identical_rows = composed.rows == prepared.terminals.rows
        && composed.remaining == prepared.terminals.remaining;
    let mut report = json!({
        "schema": "rustred.saved-application-compose-report.v1",
        "mode": mode,
        "application": application_dir,
        "layer": layer_json,
        "layer_blake3": digest.layer_blake3,
        "output": if matches!(mode, "composed" | "layered") { json!(out) } else { Value::Null },
        "factorized_terminals": factorized.len(),
        "composition": statistics,
        "identical_to_base_rows": identical_rows,
        "identical_to_base_bytes": identical_bytes,
        "size_gate": {
            "base_decoded_bytes": base_maps_bytes,
            "composed_decoded_bytes": composed_len,
            "composed_blake3": composed_blake3,
            "ratio": ratio,
            "gate": options.size_gate,
            "passed": gate_passed,
        },
        "self_check": {
            "prime": layer::CHECK_PRIME,
            "points": points,
            "row_point_pairs": checked,
            "passed": true,
        },
        "minimality_claim": minimality_claim,
        "minimality_scope": minimality_scope,
        "seconds": {
            "load": load_seconds, "products": product_seconds, "compose": compose_seconds,
            "self_check": check_seconds, "encode": encode_seconds,
        },
    });
    if matches!(mode, "dry-run" | "rejected") {
        report["seconds"]["total"] = json!(started.elapsed().as_secs_f64());
        return Ok(SavedApplicationComposeOutcome {
            mode: mode.into(),
            report,
        });
    }

    // 7. Export with the existing codec and transport.
    let base_remaining = prepared.terminals.remaining.len();
    let mut refinement = layer_summary(&layer, &digest);
    refinement["mode"] = json!(mode);
    refinement["base_remaining_terminals"] = json!(base_remaining);
    refinement["base_terminal_maps_blake3"] = base_report["terminal_maps"]["blake3"].clone();
    refinement["base_application_blake3"] = json!(blake3::hash(&base_bytes).to_hex().to_string());
    refinement["base_decoded_bytes"] = json!(base_maps_bytes);
    refinement["composed_decoded_bytes"] = json!(composed_len);
    refinement["size_gate"] = report["size_gate"].clone();
    refinement["composition"] = report["composition"].clone();
    refinement["self_check"] = report["self_check"].clone();
    refinement["factorized_terminals"] = json!(factorized.len());
    let mut metadata = metadata;
    metadata["remaining_terminals"] = json!(composed.remaining.len());
    metadata["minimality_claim"] = json!(minimality_claim);
    if minimality_claim {
        metadata["minimality_scope"] = json!(minimality_scope);
    }
    metadata["refinement_layer"] = refinement;
    let authority = metadata["authority"].as_str().unwrap_or_default().to_owned();
    metadata["authority"] = json!(format!(
        "{authority}; refinement layer {}: RustRed relations, checked mod p at compose time",
        &digest.layer_blake3[..16]
    ));
    let base_maps = if mode == "composed" {
        std::mem::replace(&mut prepared.terminals, composed)
    } else {
        drop(composed);
        let layered = layered_maps(&prepared.terminals, &bound);
        let base = std::mem::replace(&mut prepared.terminals, layered);
        // The base rows are needed in the package unchanged; move them back.
        let TerminalMaps { rows, .. } = base;
        prepared.terminals.rows = rows;
        TerminalMaps {
            families: prepared.terminals.families.clone(),
            rows: BTreeMap::new(),
            remaining: BTreeSet::new(),
            layer: None,
        }
    };
    prepared.terminal_products = None;
    prepared.terminal_product_charts = None;
    prepared.metadata = metadata;
    let export_started = Instant::now();
    prepared.export_runtime(out)?;
    let export_seconds = export_started.elapsed().as_secs_f64();

    // 8. Terminal definitions for the staging tool.
    let definitions_in = options
        .terminal_definitions
        .clone()
        .unwrap_or_else(|| application_dir.join("../terminal-definitions.json"));
    let definitions_out = options.terminal_definitions_out.clone().unwrap_or_else(|| {
        let mut name = out.file_name().unwrap_or_default().to_os_string();
        name.push(".terminal-definitions.json");
        out.with_file_name(name)
    });
    let definitions = if definitions_in.is_file() {
        let written = write_definitions(
            &definitions_in,
            &definitions_out,
            &prepared,
            if mode == "composed" {
                &base_maps.remaining
            } else {
                &prepared.terminals.layer.as_ref().unwrap().base_remaining
            },
            &digest,
            minimality_claim,
        )?;
        json!({"input": definitions_in, "output": definitions_out, "masters": written})
    } else {
        json!({"input": definitions_in, "output": Value::Null, "note": "base terminal definitions not found; none written"})
    };

    // 9. Reload the written package and re-check a stratified sample.
    let composed_rows: Vec<_> = prepared.terminals.rows.keys().cloned().collect();
    let sample = stratified(&composed_rows, options.reload_check_rows);
    let layered_rows_equal;
    let reload_started = Instant::now();
    let reloaded_check = if mode == "composed" {
        drop(prepared);
        let reloaded = load_saved_application::<N>(out, options.application)?;
        let bound = bind_layer(&base_maps, &layer, &BTreeSet::new(), limits)?;
        let (count, _) = check_composition_mod_p(
            &base_maps,
            &bound,
            &reloaded.terminals,
            &sample,
            1,
            seed(&digest.layer_blake3) ^ 0xA5A5,
        )?;
        layered_rows_equal = Value::Null;
        json!({"rows": sample.len(), "row_point_pairs": count,
               "remaining": reloaded.remaining_terminals().len()})
    } else {
        let expected_layer_rows: BTreeMap<_, _> = prepared
            .terminals
            .layer
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .map(|(k, r)| (k.clone(), r.terms.clone()))
            .collect();
        drop(prepared);
        let reloaded = load_saved_application::<N>(out, options.application)?;
        let actual: BTreeMap<_, _> = reloaded
            .terminals
            .layer
            .as_ref()
            .ok_or_else(|| AppError::execution("layered package reloaded without its layer"))?
            .rows
            .iter()
            .map(|(k, r)| (k.clone(), r.terms.clone()))
            .collect();
        layered_rows_equal = json!(actual == expected_layer_rows);
        if actual != expected_layer_rows {
            return Err(AppError::execution(
                "reloaded layered package differs from the written layer rows",
            ));
        }
        json!({"remaining": reloaded.remaining_terminals().len()})
    };
    report["reload_check"] = reloaded_check;
    report["reload_check"]["layer_rows_equal"] = layered_rows_equal;
    report["reload_check"]["seconds"] = json!(reload_started.elapsed().as_secs_f64());
    report["terminal_definitions"] = definitions;
    report["seconds"]["export"] = json!(export_seconds);
    report["seconds"]["total"] = json!(started.elapsed().as_secs_f64());
    Ok(SavedApplicationComposeOutcome {
        mode: mode.into(),
        report,
    })
}

/// Base rows unchanged; layer rows as a final substitution (design §4).
fn layered_maps(base: &TerminalMaps, bound: &BoundLayer<'_>) -> TerminalMaps {
    let mut rows = BTreeMap::new();
    for (index, (source, _)) in bound
        .layer
        .rows
        .iter()
        .map(|row| (bound.qualify(&row.source), ()))
        .enumerate()
    {
        rows.insert(
            source,
            TerminalRow {
                terms: bound.rows[index].clone(),
                conditions: std::sync::Arc::new(bound.layer.rows[index].conditions.clone()),
            },
        );
    }
    TerminalMaps {
        families: base.families.clone(),
        rows: BTreeMap::new(),
        remaining: bound.composed_remaining(&base.remaining),
        layer: Some(TerminalLayer {
            rows,
            base_remaining: base.remaining.clone(),
        }),
    }
}

fn write_definitions<const N: usize>(
    input: &Path,
    output: &Path,
    prepared: &PreparedSavedApplication<N>,
    base_remaining: &BTreeSet<SavedApplicationIntegral>,
    digest: &SavedApplicationLayerDigest,
    minimality_claim: bool,
) -> Result<usize, AppError> {
    let (_, mut definitions) = read_json(input)?;
    if definitions["schema"] != "rustred.native-master-definitions.v1" {
        return Err(AppError::input("unsupported base terminal definitions schema"));
    }
    let entries = definitions["masters"]
        .as_array()
        .cloned()
        .ok_or_else(|| AppError::input("base terminal definitions have no masters"))?;
    let mut by_key = BTreeMap::new();
    for entry in entries {
        let fingerprint = entry["family_fingerprint"].as_str().unwrap_or_default().to_owned();
        let powers: Vec<i64> = serde_json::from_value(entry["powers"].clone()).map_err(native_failure)?;
        by_key.insert((fingerprint, powers), entry);
    }
    let base_keys: BTreeSet<_> = base_remaining
        .iter()
        .map(|k| (k.family_fingerprint.clone(), k.integral.powers().to_vec()))
        .collect();
    if base_keys != by_key.keys().cloned().collect::<BTreeSet<_>>() {
        return Err(AppError::input(
            "base terminal definitions do not describe the base package terminals",
        ));
    }
    let loop_count = definitions["families"][0]["loop_count"].clone();
    let mut masters = Vec::new();
    let mut new_index = 0;
    for key in prepared.remaining_terminals() {
        let id = (key.family_fingerprint.clone(), key.integral.powers().to_vec());
        masters.push(match by_key.get(&id) {
            Some(entry) => entry.clone(),
            None => {
                new_index += 1;
                json!({"family_fingerprint": id.0, "loop_count": loop_count,
                       "master_id": format!("layer-new-{new_index}"), "powers": id.1})
            }
        });
    }
    let count = masters.len();
    definitions["masters"] = Value::Array(masters);
    definitions["minimality_claim"] = json!(minimality_claim);
    let previous = definitions["refinement_binding"].clone();
    definitions["base_refinement_binding"] = previous;
    definitions["refinement_binding"] = json!(digest.layer_blake3);
    if definitions["metadata"].is_object() {
        definitions["metadata"]["remaining_terminals"] = json!(count);
        definitions["metadata"]["minimality_claim"] = json!(minimality_claim);
        definitions["metadata"]["refinement_layer"] = json!(digest.layer_blake3);
    }
    let bytes = serde_json::to_vec_pretty(&definitions).map_err(native_failure)?;
    write_file_atomically(output, &bytes, false).map_err(AppError::execution)?;
    Ok(count)
}

/// An identity layer over every non-factorized remaining terminal of a saved
/// application (each source is its own master). Composing it must reproduce
/// the base maps exactly; it is the RR-11 identity acceptance input.
pub fn saved_application_identity_layer<const N: usize>(
    application: &Path,
    options: SavedApplicationOptions,
) -> Result<SavedApplicationLayer, AppError> {
    let manifest = if application.is_dir() {
        application.join("application.json")
    } else {
        application.to_path_buf()
    };
    let directory = manifest.parent().unwrap_or(Path::new(".")).to_path_buf();
    let base = layer::saved_application_layer_base(&directory)?;
    let mut prepared = load_saved_application::<N>(&directory, options)?;
    prepared.prepare_terminal_products(VacuumProductLimits::default())?;
    prepared.prepare_terminal_product_charts(VacuumProductChartLimits::default())?;
    let fingerprint = prepared.family().fingerprint().to_owned();
    let one = prepared.family().coefficient_context().one();
    let mut rows = Vec::new();
    let mut masters = Vec::new();
    let mut factorized = 0;
    for terminal in prepared.remaining_terminals() {
        if terminal.family_fingerprint != fingerprint {
            return Err(AppError::input(
                "identity layer supports a single terminal family",
            ));
        }
        if prepared.terminal_product(terminal).is_some()
            || prepared.terminal_product_chart(terminal).is_some()
        {
            factorized += 1;
            continue;
        }
        rows.push(layer::SavedApplicationLayerRow {
            source: terminal.integral.clone(),
            terms: vec![(terminal.integral.clone(), one.clone())],
            conditions: vec![],
        });
        masters.push(layer::SavedApplicationLayerMaster {
            key: terminal.integral.powers().to_vec(),
            class: None,
            origin: Some("present_as_terminal".into()),
            extra: BTreeMap::new(),
        });
    }
    Ok(SavedApplicationLayer {
        family_fingerprint: fingerprint,
        parameters: prepared
            .family()
            .coefficient_context()
            .parameter_names()
            .to_vec(),
        base,
        rows,
        masters,
        unreduced: vec![],
        minimality: None,
        provenance: json!({
            "kind": "identity",
            "purpose": "RR-11 identity acceptance: composing must reproduce the base maps",
            "factorized_terminals_excluded": factorized,
            "engine": "none (no relation is derived)",
        }),
    })
}
