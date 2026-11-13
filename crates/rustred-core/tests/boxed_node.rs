//! boxed-node (lane t162-export): a REAL boxed sample of the five-loop node
//! T162 through the importer and the Custom owner's program.
//!
//! The sample (`RR_BOXED_SAMPLE`, default below; built read-only from lane
//! t162-box's pilot chunk tables by `SCRATCHPAD/t162-export/tools/mksample.py`)
//! holds 20 point rules of the pilot box `D <= 2, r <= 1`, its two terminals
//! and the chunk's `symmetry_group` record. Each seed (`RR_BOXED_SEEDS`) is
//! reduced by the imported payload and compared with the table's own
//! right-hand sides expanded independently (exact in `Q(d)`, table frame,
//! then the import's slot map and sign rule); terminals come back as
//! themselves; keys of the chart outside the box are `PError::OutsideBox`; a
//! box key without a rule in this partial table is `PError::Uncovered`.
//!
//! Needs arity 20 (`RUSTRED_RUNTIME_ARITIES=..,20` with the feature
//! `runtime-arity-selection`); without it, or without the sample file, the
//! test returns early (it prints why).
use std::collections::BTreeMap;
use std::path::PathBuf;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::compiled_runtime_arities;
use rustred::propagator::PresentationId;
use rustred::propagator::basis::parse_coefficient;
use rustred::propagator::custom::{
    custom_payload_boxed_node, replay_custom_payload, verify_custom_payload,
};
use rustred::propagator::error::PError;
use rustred::propagator::table_import::{TableImportOptions, canonical_node_graph, import_table};

const DEFAULT_SAMPLE: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/t162-export/sample/T162-sample20.table.jsonl";
const DEFAULT_SEEDS: &str =
    "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/t162-export/sample/seeds20.json";
/// T162 in `rules_study/targets.tsv`: edges, S, T.
const T162_EDGES: &str = "0-1 0-2 0-3 0-4 1-2 1-3 1-4 2-3 2-4";

fn key_of(text: &str) -> Vec<i64> {
    text.trim_start_matches("I(")
        .trim_end_matches(')')
        .split(',')
        .map(|part| part.trim().parse().expect("a numeric key"))
        .collect()
}

fn add(map: &mut BTreeMap<Vec<i64>, Coefficient>, key: Vec<i64>, value: Coefficient) {
    match map.get_mut(&key) {
        Some(slot) => *slot = &*slot + &value,
        None => {
            map.insert(key, value);
        }
    }
}

/// `seed` expanded through the table's rules (table frame): terminals and
/// keys off the chart, with their table coefficients.
fn expand(
    seed: &[i64],
    rules: &BTreeMap<Vec<i64>, Vec<(Vec<i64>, Coefficient)>>,
    terminals: &[Vec<i64>],
    chart: &[bool],
    context: &CoefficientContext,
) -> BTreeMap<Vec<i64>, Coefficient> {
    // Topological expansion: repeatedly replace the largest-weight rule key
    // (the rules are acyclic; any order of substitution gives the same sum).
    let mut pending: BTreeMap<Vec<i64>, Coefficient> = BTreeMap::new();
    pending.insert(seed.to_vec(), context.one());
    let mut done: BTreeMap<Vec<i64>, Coefficient> = BTreeMap::new();
    let on_chart = |key: &[i64]| key.iter().zip(chart).all(|(&p, &line)| (p > 0) == line);
    let mut guard = 0;
    while let Some(key) = pending
        .keys()
        .find(|key| rules.contains_key(*key))
        .cloned()
        .or_else(|| pending.keys().next().cloned())
    {
        guard += 1;
        assert!(guard < 100_000, "expansion does not terminate");
        let weight = pending.remove(&key).unwrap();
        if weight.is_zero() {
            continue;
        }
        if let Some(rhs) = rules.get(&key) {
            // A key is substituted only when no pending key reaches it again:
            // with acyclic rules the sum is order independent, so substitute
            // now and re-collect.
            for (child, coefficient) in rhs {
                let value = &weight * coefficient;
                if on_chart(child) && !terminals.contains(child) {
                    add(&mut pending, child.clone(), value);
                } else {
                    add(&mut done, child.clone(), value);
                }
            }
        } else {
            assert!(
                !on_chart(&key) || terminals.contains(&key),
                "the chart key {key:?} has no rule in the sample"
            );
            add(&mut done, key, weight);
        }
    }
    done.retain(|_, value| !value.is_zero());
    done
}

#[test]
fn a_real_t162_boxed_sample_reduces_to_its_terminals_and_subsector_keys() {
    if !compiled_runtime_arities().contains(&20) {
        eprintln!("skipped: arity 20 is not compiled (RUSTRED_RUNTIME_ARITIES)");
        return;
    }
    let sample = PathBuf::from(std::env::var("RR_BOXED_SAMPLE").unwrap_or(DEFAULT_SAMPLE.into()));
    let seeds_path = PathBuf::from(std::env::var("RR_BOXED_SEEDS").unwrap_or(DEFAULT_SEEDS.into()));
    let (Ok(text), Ok(seeds_text)) = (
        std::fs::read_to_string(&sample),
        std::fs::read_to_string(&seeds_path),
    ) else {
        eprintln!("skipped: no sample at {}", sample.display());
        return;
    };
    let seeds: Vec<Vec<i64>> = serde_json::from_str(&seeds_text).unwrap();
    let (_, graph) = canonical_node_graph(T162_EDGES, 3, 4).unwrap();
    let options = TableImportOptions {
        id: PresentationId(0),
        node: None,
        lp_count: 2,
        census_family: None,
        order_map: None,
        partial_box: true,
    };
    let imported = import_table(&text, &graph, &options).unwrap();
    let boxed = imported.boxed.clone().expect("a boxed import");
    assert_eq!(boxed.box_keys, 660);
    assert_eq!(boxed.terminals.len(), 2);
    assert!(!boxed.complete && boxed.point_rules as usize == imported.rules);
    assert_eq!(boxed.symmetry_groups.len(), 1);
    assert_eq!(
        custom_payload_boxed_node(&imported.payload).unwrap(),
        Some(boxed.clone())
    );
    let presentation = &imported.presentation;
    let check = verify_custom_payload(presentation, &imported.payload, &imported.meta).unwrap();
    eprintln!(
        "verify: {} witness keys stepped, {} open box keys skipped, {} terminals",
        check.witness_keys, check.open_box_keys, check.terminals
    );
    let context = presentation.family.coefficient_context();
    let parse =
        |text: &str| parse_coefficient(context, text, ExactAlgebraLimits::default()).unwrap();
    // The table's own rules, table frame.
    let mut rules: BTreeMap<Vec<i64>, Vec<(Vec<i64>, Coefficient)>> = BTreeMap::new();
    let mut terminals: Vec<Vec<i64>> = Vec::new();
    let mut chart: Vec<bool> = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let record: serde_json::Value = serde_json::from_str(line).unwrap();
        match record["type"].as_str() {
            Some("rule") => {
                let rhs = record["rhs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|term| {
                        (
                            key_of(term[0].as_str().unwrap()),
                            parse(term[1].as_str().unwrap()),
                        )
                    })
                    .collect();
                rules.insert(key_of(record["target"].as_str().unwrap()), rhs);
            }
            Some("residual") => terminals.push(key_of(record["key"].as_str().unwrap())),
            Some("chart") => {
                chart = record["chart"]
                    .as_str()
                    .unwrap()
                    .bytes()
                    .map(|b| b == b'1')
                    .collect();
            }
            _ => {}
        }
    }
    let slot_map = &imported.slot_map;
    let to_presentation = |key: &[i64]| {
        let mut out = vec![0; key.len()];
        for (slot, &power) in key.iter().enumerate() {
            out[slot_map[slot]] = power;
        }
        out
    };
    let weight = |key: &[i64]| key.iter().sum::<i64>();
    let mut compared = 0;
    for seed in &seeds {
        let expected = expand(seed, &rules, &terminals, &chart, context);
        let replay = replay_custom_payload(
            presentation,
            &imported.payload,
            &imported.meta,
            &[to_presentation(seed)],
        )
        .unwrap()
        .remove(0);
        let mut got: BTreeMap<Vec<i64>, Coefficient> = BTreeMap::new();
        for (key, value) in replay.residuals.iter().chain(&replay.boundary) {
            got.insert(key.clone(), value.clone());
        }
        let expected: BTreeMap<Vec<i64>, Coefficient> = expected
            .into_iter()
            .map(|(key, value)| {
                // q.q = +1 -> -1: (-1)^(|seed| - |term|).
                let value = if (weight(seed) - weight(&key)).rem_euclid(2) == 1 {
                    -value
                } else {
                    value
                };
                (to_presentation(&key), value)
            })
            .collect();
        assert_eq!(got, expected, "seed {seed:?}");
        let presented_terminals: Vec<Vec<i64>> =
            terminals.iter().map(|key| to_presentation(key)).collect();
        assert!(
            replay
                .residuals
                .iter()
                .all(|(key, _)| presented_terminals.contains(key)),
            "seed {seed:?}: a residual that is no terminal"
        );
        compared += 1;
    }
    assert_eq!(compared, seeds.len());
    // A terminal comes back as itself.
    for terminal in &terminals {
        let replay = replay_custom_payload(
            presentation,
            &imported.payload,
            &imported.meta,
            &[to_presentation(terminal)],
        )
        .unwrap()
        .remove(0);
        assert_eq!(
            replay.residuals,
            vec![(to_presentation(terminal), context.one())]
        );
        assert!(replay.boundary.is_empty());
    }
    // Outside the box: D = 0, r = 2 and D = 3: OutsideBox with the box.
    let mut corner = vec![0_i64; 20];
    corner[..9].fill(1);
    let mut rank_two = corner.clone();
    rank_two[9] = -1;
    rank_two[10] = -1;
    let mut dots_three = corner.clone();
    dots_three[0] = 4;
    for key in [rank_two, dots_three] {
        match replay_custom_payload(
            presentation,
            &imported.payload,
            &imported.meta,
            &[to_presentation(&key)],
        ) {
            Err(PError::OutsideBox {
                key: refused,
                node,
                definition,
                ..
            }) => {
                assert_eq!(refused.powers(), to_presentation(&key).as_slice());
                assert_eq!(node, "T162");
                assert!(definition.contains("pilot_d2r1"), "{definition}");
            }
            other => panic!("{key:?}: expected OutsideBox, got {other:?}"),
        }
    }
    // Inside the box but not in this partial sample: the plain Uncovered.
    let mut open = corner.clone();
    open[0] = 2;
    open[1] = 2;
    open[12] = -1;
    assert!(!rules.contains_key(&open));
    assert!(matches!(
        replay_custom_payload(
            presentation,
            &imported.payload,
            &imported.meta,
            &[to_presentation(&open)],
        ),
        Err(PError::Uncovered { .. })
    ));
    eprintln!(
        "boxed sample: {} rules, {} seeds compared, {} terminals, payload {} bytes",
        imported.rules,
        compared,
        terminals.len(),
        imported.payload.len()
    );
}
