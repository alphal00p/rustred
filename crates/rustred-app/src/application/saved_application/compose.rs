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

use super::codec::EncodedMapsLayout;
use super::layer::{
    self, BoundLayer, SavedApplicationLayer, SavedApplicationLayerDigest,
    SavedApplicationFactorizedSources, SavedApplicationMinimalityScope,
    SavedApplicationRuleProvenance, bind_layer, bind_layer_with, check_composition_mod_p, compose_maps, layer_minimality_claim, layer_summary,
};
use super::{
    PreparedSavedApplication, SavedApplicationIntegral, SavedApplicationOptions, TerminalLayer,
    TerminalMaps, TerminalRow, codec, load_saved_application, native_failure,
};
use crate::AppError;
use crate::application::atomic_file::write_file_atomically;

/// Binary limits of the loader that consumes the composed package. The size
/// gate passes only if the composed maps also fit these limits: RustRed main
/// made its own limits opt-in diagnostics, but the consumer pin may not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SavedApplicationConsumerLimits {
    pub name: &'static str,
    pub max_program_bytes: usize,
    pub max_total_atom_bytes: usize,
    pub max_atom_bytes: usize,
    pub max_state_bytes: usize,
    pub max_collection_entries: usize,
}

impl SavedApplicationConsumerLimits {
    /// `BinaryIoLimits::default()` at RustRed 52c74636, the revision vakint's
    /// five-loop loader pins (it loads with `SavedApplicationOptions::default()`).
    pub const VAKINT_PIN_52C74636: Self = Self {
        name: "vakint-pin-52c74636",
        max_program_bytes: 1 << 30,
        max_total_atom_bytes: 512 << 20,
        max_atom_bytes: 16 << 20,
        max_state_bytes: 16 << 20,
        max_collection_entries: 8_000_000,
    };
    /// No consumer limit (only for a consumer pinned at RustRed main or later
    /// with unbounded limits, or with raised `binary_limits`).
    pub const NONE: Self = Self {
        name: "none",
        max_program_bytes: usize::MAX,
        max_total_atom_bytes: usize::MAX,
        max_atom_bytes: usize::MAX,
        max_state_bytes: usize::MAX,
        max_collection_entries: usize::MAX,
    };

    pub fn parse(name: &str) -> Option<Self> {
        [Self::VAKINT_PIN_52C74636, Self::NONE]
            .into_iter()
            .find(|limits| limits.name == name)
    }

    /// Every limit the encoded maps exceed (empty: the consumer loads them).
    pub fn violations(&self, layout: &EncodedMapsLayout) -> Vec<String> {
        let mut violations = Vec::new();
        let mut check = |what: &str, value: usize, limit: usize| {
            if value > limit {
                violations.push(format!("{what} {value} > {limit}"));
            }
        };
        check("maps bytes", layout.total_bytes, self.max_program_bytes);
        check(
            "state + coefficient table bytes",
            layout.state_bytes.saturating_add(layout.atom_table_bytes),
            self.max_program_bytes,
        );
        check(
            "coefficient table bytes",
            layout.atom_table_bytes,
            self.max_total_atom_bytes,
        );
        check(
            "largest coefficient atom bytes",
            layout.max_atom_bytes,
            self.max_atom_bytes,
        );
        check(
            "Symbolica state bytes",
            layout.state_bytes,
            self.max_state_bytes,
        );
        check(
            "coefficient table entries",
            layout.atom_entries,
            self.max_collection_entries,
        );
        check("raw rows", layout.rows, self.max_collection_entries);
        check(
            "remaining terminals",
            layout.remaining,
            self.max_collection_entries,
        );
        violations
    }
}

/// Options of one composition.
#[derive(Clone, Debug)]
pub struct SavedApplicationComposeOptions {
    pub application: SavedApplicationOptions,
    /// Composed decoded maps may be at most this multiple of the base maps.
    pub size_gate: f64,
    /// The composed maps must also fit the consumer's binary limits.
    pub consumer_limits: SavedApplicationConsumerLimits,
    /// Record base terminals the layer is silent about as unreduced instead
    /// of refusing the layer.
    pub fill_unreduced: bool,
    /// RR-11 P1: factorized remaining terminals (product route) may be
    /// sources when their rows state a verification level and use only
    /// factorized base terminals as masters.
    pub allow_factorized_sources: bool,
    /// The minimality reference file, checked against `reference_blake3`
    /// and parsed (`refine-5l.expected-counts.v1`): expected per-class counts
    /// come from it.
    pub minimality_reference: Option<PathBuf>,
    /// The reference was independently reviewed; without this the
    /// minimality claim is always withheld (false).
    pub minimality_reference_reviewed: bool,
    /// Write per-row values at one point mod p (base, parsed layer and
    /// composed rows, plus the touched raw rows) for an external checker.
    /// Written in every mode, dry-run included; never replaces a file.
    pub check_values_out: Option<PathBuf>,
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
            consumer_limits: SavedApplicationConsumerLimits::VAKINT_PIN_52C74636,
            fill_unreduced: false,
            allow_factorized_sources: false,
            minimality_reference: None,
            minimality_reference_reviewed: false,
            check_values_out: None,
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
    /// `composed`, `layered`, `rejected` (size gate or consumer limits) or `dry-run`.
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
        return Err(AppError::input(
            "size gate must be a nonnegative finite ratio",
        ));
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
    let mismatch =
        |field: &str| AppError::input(format!("layer base {field} differs from the package"));
    if Some(base.terminal_maps_blake3.as_str()) != base_report["terminal_maps"]["blake3"].as_str() {
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

    // Factorized terminals (product route) are sources only under RR-11 P1.
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
    let factorized_policy = if options.allow_factorized_sources {
        SavedApplicationFactorizedSources::VerifiedOntoProducts
    } else {
        SavedApplicationFactorizedSources::Refused
    };
    let bound = bind_layer_with(
        &prepared.terminals,
        &layer,
        &factorized,
        factorized_policy,
        limits,
    )?;
    // The layer must account for every non-factorized remaining terminal:
    // a source, a master, or a declared (or explicitly filled) unreduced entry.
    if !bound.undeclared.is_empty() && !options.fill_unreduced {
        let first = bound.undeclared.iter().next().unwrap();
        return Err(AppError::input(format!(
            "layer is silent about {} remaining non-factorized base terminals that are neither \
             sources, masters nor unreduced entries (first {:?}); list them in `unreduced` or \
             pass --fill-unreduced",
            bound.undeclared.len(),
            first.integral.powers()
        )));
    }
    let filled_unreduced: Vec<Vec<i64>> = if options.fill_unreduced {
        bound
            .undeclared
            .iter()
            .map(|k| k.integral.powers().to_vec())
            .collect()
    } else {
        Vec::new()
    };
    let unreduced_nonfactorized = bound
        .unreduced
        .iter()
        .filter(|k| !factorized.contains(*k))
        .count()
        + filled_unreduced.len();
    // A composed package substitutes sources only through raw rows; a source
    // reached otherwise would silently stay unreduced.
    let sources_without_raw_row: Vec<_> = bound
        .sources
        .keys()
        .filter(|s| !prepared.terminals.rows.contains_key(*s))
        .collect();
    if let Some(first) = sources_without_raw_row.first() {
        return Err(AppError::input(format!(
            "{} layer sources are not raw-row keys of the base (first {:?})",
            sources_without_raw_row.len(),
            first.integral.powers()
        )));
    }
    let compose_started = Instant::now();
    let (composed, statistics) = compose_maps(&prepared.terminals, &bound, limits)?;
    composed.validate(options.application)?;
    // Consistency with the base: a master that is a raw-row key must compose
    // to exactly itself, else the layer contradicts the base relations.
    let master_raw_rows = layer::check_masters_compose_to_identity(&composed, &bound)?;
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
    let check_values = match &options.check_values_out {
        None => Value::Null,
        Some(path) => {
            if path.exists() {
                return Err(AppError::input(format!(
                    "{} already exists; check values never replace a file",
                    path.display()
                )));
            }
            let values = layer::composition_check_values(
                &prepared.terminals,
                &bound,
                &composed,
                seed(&digest.layer_blake3) ^ 0x5EED,
            )?;
            let mut values = values;
            values["layer_blake3"] = json!(digest.layer_blake3);
            let bytes = serde_json::to_vec(&values).map_err(native_failure)?;
            write_file_atomically(path, &bytes, false).map_err(AppError::execution)?;
            json!({"path": path, "blake3": blake3::hash(&bytes).to_hex().to_string(),
                   "touched_rows": values["touched_rows"].as_array().map_or(0, Vec::len)})
        }
    };

    // Size gate on the decoded native maps: the ratio and the consumer limits.
    let encode_started = Instant::now();
    let (composed_bytes, layout) = codec::encode_with_layout(&composed, options.application)?;
    let composed_len = composed_bytes.len() as u64;
    let composed_blake3 = blake3::hash(&composed_bytes).to_hex().to_string();
    let identical_bytes = composed_blake3
        == base_report["terminal_maps"]["blake3"]
            .as_str()
            .unwrap_or_default();
    drop(composed_bytes);
    let encode_seconds = encode_started.elapsed().as_secs_f64();
    let ratio = composed_len as f64 / base_maps_bytes.max(1) as f64;
    let ratio_passed = ratio <= options.size_gate;
    let violations = options.consumer_limits.violations(&layout);
    let gate_passed = ratio_passed && violations.is_empty();
    let effective_bytes = (options.size_gate * base_maps_bytes as f64)
        .min(options.consumer_limits.max_program_bytes as f64);
    let mode = if options.dry_run {
        "dry-run"
    } else if gate_passed {
        "composed"
    } else if options.allow_layered {
        "layered"
    } else {
        "rejected"
    };

    // Minimality, against the base and a content-bound reference.
    let mut parsed_reference = None;
    let reference = match (&options.minimality_reference, layer.minimality.as_ref()) {
        (Some(path), minimality) => {
            let bytes =
                fs::read(path).map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
            parsed_reference = Some(layer::SavedApplicationMinimalityReference::parse(&bytes)?);
            let actual = blake3::hash(&bytes).to_hex().to_string();
            match minimality.and_then(|m| m.reference_blake3.as_ref()) {
                Some(expected) if *expected == actual => {
                    json!({"path": path, "blake3": actual, "verified": true})
                }
                Some(expected) => {
                    return Err(AppError::input(format!(
                        "minimality reference {} has blake3 {actual}; the layer binds {expected}",
                        path.display()
                    )));
                }
                None => json!({"path": path, "blake3": actual, "verified": false,
                               "note": "layer has no minimality.reference_blake3"}),
            }
        }
        (None, _) => json!({"verified": false, "note": "no --minimality-reference given"}),
    };
    let scope = SavedApplicationMinimalityScope {
        unreduced: unreduced_nonfactorized,
        factorized: factorized.len(),
        reference_verified: reference["verified"] == true,
        reference: parsed_reference,
        reference_reviewed: options.minimality_reference_reviewed,
    };
    let (minimality_claim, minimality_scope) = layer_minimality_claim(&layer, &scope);
    let identical_rows = composed.rows == prepared.terminals.rows
        && composed.remaining == prepared.terminals.remaining;
    let provenance_counts = layer.provenance_counts();
    let authority = layer.authority(&digest.layer_blake3[..16]);
    let mut report = json!({
        "schema": "rustred.saved-application-compose-report.v2",
        "mode": mode,
        "application": application_dir,
        "layer": layer_json,
        "layer_blake3": digest.layer_blake3,
        "output": if matches!(mode, "composed" | "layered") { json!(out) } else { Value::Null },
        "factorized_terminals": factorized.len(),
        "factorized_sources": {
            "policy": if options.allow_factorized_sources { "rr11-p1-verified-onto-products" } else { "refused" },
            "sources": bound.factorized_sources.len(),
            "keys": bound.factorized_sources.iter().map(|k| k.integral.powers().to_vec()).collect::<Vec<_>>(),
        },
        "composition": statistics,
        "consistency": {
            "masters_with_raw_rows_composing_to_identity": master_raw_rows,
            "sources_without_raw_row": 0,
        },
        "unreduced": {
            "declared": bound.unreduced.len(),
            "declared_nonfactorized_plus_filled": unreduced_nonfactorized,
            "filled": filled_unreduced.len(),
        },
        "provenance_counts": provenance_counts,
        "verification_counts": layer.verification_counts(),
        "check_values": check_values,
        "authority": authority,
        "identical_to_base_rows": identical_rows,
        "identical_to_base_bytes": identical_bytes,
        "size_gate": {
            "base_decoded_bytes": base_maps_bytes,
            "composed_decoded_bytes": composed_len,
            "composed_blake3": composed_blake3,
            "ratio": ratio,
            "gate": options.size_gate,
            "ratio_passed": ratio_passed,
            "consumer_limits": {
                "limits": options.consumer_limits,
                "layout": layout,
                "violations": violations,
                "passed": violations.is_empty(),
            },
            "effective_max_decoded_bytes": effective_bytes,
            "effective_max_ratio": effective_bytes / base_maps_bytes.max(1) as f64,
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
        "minimality_reference": reference,
        "minimality_reference_reviewed": options.minimality_reference_reviewed,
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

    // Output paths; nothing appears at `out` before every check passed.
    let definitions_in = options
        .terminal_definitions
        .clone()
        .unwrap_or_else(|| application_dir.join("../terminal-definitions.json"));
    let definitions_out = options.terminal_definitions_out.clone().unwrap_or_else(|| {
        let mut name = out.file_name().unwrap_or_default().to_os_string();
        name.push(".terminal-definitions.json");
        out.with_file_name(name)
    });
    if definitions_in.is_file() && definitions_out.exists() {
        return Err(AppError::input(format!(
            "{} already exists",
            definitions_out.display()
        )));
    }
    let staging = Staging::new(out, &definitions_out)?;

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
    refinement["consistency"] = report["consistency"].clone();
    refinement["self_check"] = report["self_check"].clone();
    refinement["factorized_terminals"] = json!(factorized.len());
    refinement["factorized_sources"] = report["factorized_sources"].clone();
    refinement["unreduced_filled"] = json!(filled_unreduced);
    refinement["minimality_reference"] = reference.clone();
    let mut metadata = metadata;
    // The loader records its own worker count; keep the base's values.
    for key in ["preparation_workers", "owner_preparation"] {
        match base_report["metadata"].get(key) {
            Some(value) => metadata[key] = value.clone(),
            None => {
                if let Some(object) = metadata.as_object_mut() {
                    object.remove(key);
                }
            }
        }
    }
    metadata["remaining_terminals"] = json!(composed.remaining.len());
    metadata["minimality_claim"] = json!(minimality_claim);
    if minimality_claim {
        metadata["minimality_scope"] = json!(minimality_scope);
    }
    if mode == "layered" {
        // A loader unaware of `terminal_layer` compares this with the base
        // rows only and refuses, even when the basis size is unchanged.
        metadata["source_keys_checked"] =
            json!(prepared.terminals.rows.len() + bound.sources.len());
        refinement["layer_rows"] = json!(bound.sources.len());
    }
    let base_authority = metadata["authority"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    metadata["authority"] = json!(format!("{base_authority}; {authority}"));
    let (base_maps, expected_maps_blake3, sample_base_rows) = if mode == "composed" {
        (
            std::mem::replace(&mut prepared.terminals, composed),
            composed_blake3.clone(),
            BTreeMap::new(),
        )
    } else {
        drop(composed);
        let layered = layered_maps(&prepared.terminals, &bound);
        let base = std::mem::replace(&mut prepared.terminals, layered);
        // The base rows are needed in the package unchanged; move them back.
        let TerminalMaps { rows, .. } = base;
        prepared.terminals.rows = rows;
        // The export re-encodes the base rows; bind the re-encoding (it differs
        // from the original bytes only in process-local Symbolica symbol IDs).
        let reencoded = codec::encode(&prepared.terminals, options.application)?;
        let reencoded_blake3 = blake3::hash(&reencoded).to_hex().to_string();
        drop(reencoded);
        refinement["base_terminal_maps_reencoded_blake3"] = json!(reencoded_blake3);
        refinement["base_terminal_maps_reencoded_identical"] = json!(
            Some(reencoded_blake3.as_str()) == base_report["terminal_maps"]["blake3"].as_str()
        );
        let keys: Vec<_> = prepared.terminals.rows.keys().cloned().collect();
        let sample: BTreeMap<_, _> = stratified(&keys, options.reload_check_rows)
            .into_iter()
            .map(|k| {
                let row = prepared.terminals.rows[&k].clone();
                (k, row)
            })
            .collect();
        (
            TerminalMaps {
                families: prepared.terminals.families.clone(),
                rows: BTreeMap::new(),
                remaining: BTreeSet::new(),
                layer: None,
            },
            reencoded_blake3,
            sample,
        )
    };
    metadata["refinement_layer"] = refinement;
    prepared.terminal_products = None;
    prepared.terminal_product_charts = None;
    prepared.metadata = metadata;
    let export_started = Instant::now();
    prepared.export_runtime(&staging.package)?;
    let export_seconds = export_started.elapsed().as_secs_f64();
    let (_, written) = read_json(&staging.package.join("application.json"))?;
    if written["terminal_maps"]["blake3"].as_str() != Some(expected_maps_blake3.as_str()) {
        return Err(AppError::execution(
            "written terminal maps differ from the encoding the gate and digests were computed on",
        ));
    }

    // 8. Terminal definitions for the staging tool.
    let definitions = if definitions_in.is_file() {
        let written = write_definitions(
            &definitions_in,
            &staging.definitions,
            &prepared,
            if mode == "composed" {
                &base_maps.remaining
            } else {
                &prepared.terminals.layer.as_ref().unwrap().base_remaining
            },
            &digest,
            minimality_claim,
            &layer,
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
        let reloaded = load_saved_application::<N>(&staging.package, options.application)?;
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
        let reloaded = load_saved_application::<N>(&staging.package, options.application)?;
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
        for (key, row) in &sample_base_rows {
            if reloaded.terminals.rows.get(key) != Some(row) {
                return Err(AppError::execution(
                    "reloaded layered package changed a base row",
                ));
            }
        }
        json!({"remaining": reloaded.remaining_terminals().len(),
               "base_rows_equal": sample_base_rows.len()})
    };
    staging.commit(out, &definitions_out, definitions_in.is_file())?;
    report["reload_check"] = reloaded_check;
    report["reload_check"]["layer_rows_equal"] = layered_rows_equal;
    report["reload_check"]["seconds"] = json!(reload_started.elapsed().as_secs_f64());
    report["terminal_definitions"] = definitions;
    report["written_terminal_maps_blake3"] = json!(expected_maps_blake3);
    report["seconds"]["export"] = json!(export_seconds);
    report["seconds"]["total"] = json!(started.elapsed().as_secs_f64());
    Ok(SavedApplicationComposeOutcome {
        mode: mode.into(),
        report,
    })
}

/// Temporary sibling output paths; removed on drop unless committed.
struct Staging {
    package: PathBuf,
    definitions: PathBuf,
    committed: bool,
}

impl Staging {
    fn new(out: &Path, definitions_out: &Path) -> Result<Self, AppError> {
        let tag = format!(
            ".compose-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        );
        let sibling = |path: &Path| {
            let mut name = std::ffi::OsString::from(".");
            name.push(path.file_name().unwrap_or_default());
            name.push(&tag);
            path.with_file_name(name)
        };
        let staging = Self {
            package: sibling(out),
            definitions: sibling(definitions_out),
            committed: false,
        };
        if staging.package.exists() || staging.definitions.exists() {
            return Err(AppError::input("compose staging path already exists"));
        }
        Ok(staging)
    }

    fn commit(
        mut self,
        out: &Path,
        definitions_out: &Path,
        with_definitions: bool,
    ) -> Result<(), AppError> {
        if out.exists() || (with_definitions && definitions_out.exists()) {
            return Err(AppError::input(format!(
                "{} appeared during composition; compose-layer never replaces a package",
                out.display()
            )));
        }
        if with_definitions {
            fs::rename(&self.definitions, definitions_out)
                .map_err(|e| AppError::execution(format!("{}: {e}", definitions_out.display())))?;
        }
        if let Err(error) = fs::rename(&self.package, out) {
            if with_definitions {
                let _ = fs::rename(definitions_out, &self.definitions);
            }
            return Err(AppError::execution(format!("{}: {error}", out.display())));
        }
        self.committed = true;
        Ok(())
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.package);
            let _ = fs::remove_file(&self.definitions);
        }
    }
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
    layer: &SavedApplicationLayer,
) -> Result<usize, AppError> {
    let (_, mut definitions) = read_json(input)?;
    if definitions["schema"] != "rustred.native-master-definitions.v1" {
        return Err(AppError::input(
            "unsupported base terminal definitions schema",
        ));
    }
    let entries = definitions["masters"]
        .as_array()
        .cloned()
        .ok_or_else(|| AppError::input("base terminal definitions have no masters"))?;
    let mut by_key = BTreeMap::new();
    for entry in entries {
        let fingerprint = entry["family_fingerprint"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let powers: Vec<i64> =
            serde_json::from_value(entry["powers"].clone()).map_err(native_failure)?;
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
        let id = (
            key.family_fingerprint.clone(),
            key.integral.powers().to_vec(),
        );
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
    definitions["refinement_provenance"] = json!({
        "counts": layer.provenance_counts(),
        "flagged_sources": layer.flagged_sources(),
        "authority": layer.authority(&digest.layer_blake3[..16]),
    });
    if definitions["metadata"].is_object() {
        definitions["metadata"]["remaining_terminals"] = json!(count);
        definitions["metadata"]["minimality_claim"] = json!(minimality_claim);
        definitions["metadata"]["refinement_layer"] = json!(digest.layer_blake3);
        definitions["metadata"]["refinement_provenance_counts"] = json!(layer.provenance_counts());
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
            provenance: SavedApplicationRuleProvenance::Identity,
            verification: None,
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
