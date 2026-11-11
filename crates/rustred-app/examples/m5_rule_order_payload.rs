//! Temporary native rule-order experiment, using the current payload format.
//! No core identity or release artifact schema changes. No persistent step cache.
//!
//! export INPUT.rrbin OUTPUT.json
//! apply INPUT.rrbin ORDER.json|- OUTPUT.rrbin REPORT.json
//!
//! ORDER: {"charts":[{"chart_index":0,"order":[original_rule_ids...]}]}.
//! Omitted charts retain the original order. '-' preserves all original bytes.
//! Ordinary partition FirstValid only: applying an order to v3 whole-piece
//! alternatives is refused rather than silently changing their dispatch policy.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::io::Write;
use std::path::Path;

use rustred::persistence::{BinaryIoLimits, BinarySection, SectionTag, encode_program};
use rustred::propagator::bundle_wire::{
    ProgramRecord, StoredProgram, candidate_sections, decode_program_record, encode_program_record,
    validate_dispatch,
};
use rustred::propagator::custom::custom_payload_rules;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_new(path, &bytes)
}

fn program(bytes: &[u8]) -> Result<ProgramRecord> {
    let envelope = candidate_sections(bytes, BinaryIoLimits::default())?;
    match decode_program_record(envelope.section(SectionTag::PROGRAM).ok_or("missing PROGRAM")?, 1 << 30)? {
        StoredProgram::Current(records) => Ok(records),
        StoredProgram::Legacy(_) => Err("legacy v1 order is not inferred by this experiment".into()),
    }
}

fn mask(bits: &[bool]) -> u64 {
    bits.iter().enumerate().fold(0, |m, (i, &b)| if b { m | 1 << i } else { m })
}

fn export(input: &Path, output: &Path) -> Result<()> {
    let bytes = std::fs::read(input)?;
    let records = program(&bytes)?;
    validate_dispatch(&records)?;
    let native_text = custom_payload_rules(&bytes)?;
    let mut text = native_text.iter();
    let mut charts = Vec::new();
    for (chart_index, chart) in records.sectors.iter().enumerate() {
        let mut exported = Vec::new();
        let mut case_counts = BTreeMap::<String, usize>::new();
        for (original_rule_id, rule) in chart.rules.iter().enumerate() {
            let decoded = text.next().ok_or("missing native text rule")?;
            if decoded.ordinal != original_rule_id || decoded.chart != mask(&chart.sector) {
                return Err("native text and wire rule orders disagree".into());
            }
            *case_counts.entry(format!("{:?}", rule.case)).or_default() += 1;
            let mut lines_raised = BTreeSet::new();
            let mut lines_lowered = BTreeSet::new();
            let mut rhs_shifts = Vec::new();
            let mut net_rank_changes = Vec::new();
            for term in &rule.rhs {
                let shifts: Vec<Option<i32>> = rule.target.values.iter().enumerate().map(|(slot, &target)| {
                    (rule.target.symbolic[slot] == term.integral.symbolic[slot])
                        .then_some(i32::from(term.integral.values[slot]) - i32::from(target))
                }).collect();
                let mut net = Some(0i32);
                for (slot, (&active, shift)) in chart.sector.iter().zip(&shifts).enumerate() {
                    if active {
                        if shift.is_some_and(|s| s > 0) { lines_raised.insert(slot); }
                        if shift.is_some_and(|s| s < 0) { lines_lowered.insert(slot); }
                    } else {
                        net = net.zip(*shift).map(|(a, b)| a - b);
                    }
                }
                net_rank_changes.push(net);
                rhs_shifts.push(shifts);
            }
            let all_rank_changes_known = net_rank_changes.iter().all(Option::is_some);
            let max_rank_change = if all_rank_changes_known {
                net_rank_changes.iter().filter_map(|&x| x).max()
            } else { None };
            let rank_raising_terms = if all_rank_changes_known {
                Some(net_rank_changes.iter().filter(|x| x.is_some_and(|n| n > 0)).count())
            } else { None };
            let distinct_rows: BTreeSet<_> = rule.sources.iter().map(|s| s.basis_row).collect();
            let distinct_seeds: BTreeSet<_> = rule.sources.iter().map(|s| (
                s.integral.symbolic.clone(), s.integral.values.clone(), s.shifts.clone(),
            )).collect();
            let coefficient_size_bytes: usize = decoded.rhs.iter().map(|(_, c)| c.len()).sum();
            exported.push(serde_json::json!({
                "original_rule_id": original_rule_id,
                "chart_index": chart_index,
                "case": {"kind":rule.case.kind,"fixed_axes":rule.case.fixed_axes,"fixed_values":rule.case.fixed_values,"equation_ids":rule.case.equations},
                "target": {"symbolic":rule.target.symbolic,"values":rule.target.values},
                "rhs_terms": rule.rhs.len(),
                "rhs_shifts": rhs_shifts,
                "rhs_generic_rank_changes": net_rank_changes,
                "max_numerator_rank_change": max_rank_change,
                "rank_raising_terms": rank_raising_terms,
                "rank_change_scope": "generic net shifts on original off-support slots, P2 model; not a domain-wide bound at support crossings; mixed fixed/symbolic differences are null",
                "lines_raised": lines_raised,
                "lines_lowered": lines_lowered,
                "exceptions": {"branches":rule.exclusions.len(),"predicates":rule.exclusions.iter().map(Vec::len).sum::<usize>(),"coefficient_ids":rule.exclusions,"text":decoded.exclusions},
                "equations_text": decoded.equations,
                "coefficient_size_bytes": coefficient_size_bytes,
                "coefficient_size_bytes_basis": "sum of canonical native coefficient text byte lengths, with repeated RHS uses counted",
                "coefficient_degree": null,
                "coefficient_degree_availability": "not computed by this structural/text exporter",
                "eps_pole_flag": null,
                "eps_pole_availability": "not inferred from text; no factorization performed",
                "sources_count": rule.sources.len(),
                "distinct_ibp_rows": distinct_rows.len(),
                "distinct_seeds": distinct_seeds.len(),
                "source_basis_row_ids": distinct_rows,
                "exact_backend": null,
                "provenance": {"source_descriptors":"native saved RuleRecord.sources","exact_backend":"not stored in this payload record"},
                "ordering_scores": {},
                "after_baseline_whole_piece_alternative": decoded.dispatched,
            }));
        }
        charts.push(serde_json::json!({
            "chart_index": chart_index,
            "support":chart.sector,
            "original_order":(0..chart.rules.len()).collect::<Vec<_>>(),
            "rules":exported,
            "duplicate_case_groups":case_counts.values().filter(|&&n|n>1).count(),
            "residuals":chart.finite_residuals.iter().map(|r|serde_json::json!({"symbolic":r.symbolic,"values":r.values})).collect::<Vec<_>>(),
        }));
    }
    if text.next().is_some() { return Err("extra native text rules".into()); }
    write_json(output, &serde_json::json!({
        "schema":"astra.m5.native-rule-order-experiment.v1",
        "payload_path": input,
        "payload_schema": records.schema,
        "family_fingerprint": records.family_fingerprint,
        "root_sector": records.root_sector,
        "ordering_stable_id": records.integral_order,
        "dispatch": if records.rule_dispatch.is_empty() { "FirstValid" } else { "native partition FirstValid, then first whole-piece alternative; apply refuses this policy" },
        "ordered_id_scope":"original ordinal within the raw chart of this input payload; never keyed by case",
        "owner_ordering_provenance":{"heuristic":"original saved order","parameters":{},"reference_measurements":null},
        "charts": charts,
        "unknown_metadata_is_explicit":true,
    }))
}

fn permutation(value: &serde_json::Value, count: usize) -> Result<Vec<usize>> {
    let list = value.as_array().ok_or("order must be an array")?;
    if list.len() != count { return Err(format!("order has {} entries, expected {count}", list.len()).into()); }
    let mut seen = vec![false; count];
    let mut out = Vec::with_capacity(count);
    for value in list {
        let raw = value.as_u64().ok_or("rule IDs must be nonnegative integers")?;
        let id = usize::try_from(raw)?;
        if id >= count || seen[id] { return Err(format!("invalid or repeated original rule ID {id}").into()); }
        seen[id] = true;
        out.push(id);
    }
    Ok(out)
}

fn apply(input: &Path, order_path: &Path, output: &Path, report_path: &Path) -> Result<()> {
    let bytes = std::fs::read(input)?;
    let original = program(&bytes)?;
    validate_dispatch(&original)?;
    if !original.rule_dispatch.is_empty() {
        return Err("ordering experiment requires ordinary FirstValid: nonempty native whole-piece rule_dispatch is not rewritten".into());
    }
    let mut records = original.clone();
    let mut permutations: Vec<Vec<usize>> = records.sectors.iter().map(|s|(0..s.rules.len()).collect()).collect();
    let mut provenance = serde_json::json!({"heuristic":"original saved order","parameters":{}});
    if order_path != Path::new("-") {
        let order: serde_json::Value = serde_json::from_slice(&std::fs::read(order_path)?)?;
        let charts = order["charts"].as_array().ok_or("order JSON needs charts array")?;
        let mut seen = BTreeSet::new();
        for chart in charts {
            let index = usize::try_from(chart["chart_index"].as_u64().ok_or("chart_index must be an integer")?)?;
            if index >= records.sectors.len() || !seen.insert(index) {
                return Err(format!("invalid or repeated chart_index {index}").into());
            }
            permutations[index] = permutation(&chart["order"], original.sectors[index].rules.len())?;
        }
        provenance = order.get("owner_ordering_provenance").cloned().unwrap_or_else(||serde_json::json!({"heuristic":"explicit permutation","parameters":{},"source":order_path}));
    }
    let mut changed_charts = Vec::new();
    for (index, order) in permutations.iter().enumerate() {
        let prior = &original.sectors[index];
        records.sectors[index].rules = order.iter().map(|&old|prior.rules[old].clone()).collect();
        if order.iter().enumerate().any(|(new,&old)|new!=old) { changed_charts.push(index); }
        // Full native record equality retains cases, exceptions, coefficients,
        // source descriptors and duplicate cases. No rule content is rebuilt.
        if records.sectors[index].finite_residuals != prior.finite_residuals {
            return Err("residual inventory changed".into());
        }
        for (new,&old) in order.iter().enumerate() {
            if records.sectors[index].rules[new] != prior.rules[old] {
                return Err("native RuleRecord permutation invariant failed".into());
            }
        }
    }
    let encoded = if changed_charts.is_empty() {
        bytes.clone()
    } else {
        let encoded_program = encode_program_record(&records)?;
        let envelope = candidate_sections(&bytes, BinaryIoLimits::default())?;
        let sections: Vec<_> = envelope.sections().iter().map(|section| BinarySection {
            tag:section.tag,
            bytes:if section.tag==SectionTag::PROGRAM { &encoded_program } else { section.bytes },
        }).collect();
        encode_program(envelope.kind(), &sections, BinaryIoLimits::default())?
    };
    if program(&encoded)? != records { return Err("native program roundtrip mismatch".into()); }
    let before = candidate_sections(&bytes, BinaryIoLimits::default())?;
    let after = candidate_sections(&encoded, BinaryIoLimits::default())?;
    for tag in [SectionTag::SYMBOLICA_STATE, SectionTag::COEFFICIENTS, SectionTag::FAMILY] {
        if before.section(tag)!=after.section(tag) { return Err("non-PROGRAM bytes changed".into()); }
    }
    // Create outputs only after all permutation/content invariants pass.
    write_new(output, &encoded)?;
    write_json(report_path, &serde_json::json!({
        "schema":"astra.m5.native-rule-order-experiment.v1",
        "input":input,"order_file":order_path,"output":output,
        "dispatch":"FirstValid in saved native rule vector order",
        "changed_charts":changed_charts,
        "orders":permutations.iter().enumerate().map(|(chart_index,order)|serde_json::json!({"chart_index":chart_index,"order":order})).collect::<Vec<_>>(),
        "owner_ordering_provenance":provenance,
        "default_original_order":changed_charts.is_empty(),
        "input_output_bytes_identical":encoded==bytes,
        "rule_record_permutation_invariant":true,
        "residual_inventory_unchanged":true,
        "non_PROGRAM_sections_byte_identical":true,
        "duplicate_case_policy":"all distinct ordered rule entries retained; no case-keyed map",
        "core_identity_or_format_change":false,
        "step_cache_policy":"use fresh process and no persistent step cache; final artifact version/identity is coordinator-owned",
        "acceptance":"PENDING runtime byte-identical PReduction value lines plus T-BFS; structural permutation alone is not acceptance",
    }))
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    match args.get(1).and_then(|s|s.to_str()) {
        Some("export") if args.len()==4 => export(Path::new(&args[2]),Path::new(&args[3])),
        Some("apply") if args.len()==6 => apply(Path::new(&args[2]),Path::new(&args[3]),Path::new(&args[4]),Path::new(&args[5])),
        _ => Err("usage: m5_rule_order_payload export INPUT.rrbin OUTPUT.json | apply INPUT.rrbin ORDER.json|- OUTPUT.rrbin REPORT.json".into()),
    }
}
