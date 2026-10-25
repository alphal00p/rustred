//! RR-11 layer tests on synthetic rules (design §3 "Tests", acceptance L1).
use super::compose::{SavedApplicationComposeOptions, compose_saved_application_layer};
use super::layer::*;
use super::tests::{Scratch, prepared_fixture};
use super::*;
use rustred::algebra::CoefficientContext;
use rustred::family::AffineDenominator;
use serde_json::json;

fn family() -> Arc<IntegralFamily> {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    Arc::new(
        IntegralFamily::new(
            "layer-composition",
            vec!["q".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![context.one()],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    )
}

fn key(family: &IntegralFamily, power: i64) -> SavedApplicationIntegral {
    SavedApplicationIntegral {
        family_fingerprint: family.fingerprint().to_owned(),
        integral: IntegralKey::try_new([power]).unwrap(),
    }
}

fn c(family: &IntegralFamily, text: &str) -> Coefficient {
    parse_layer_coefficient(family, text, Default::default()).unwrap()
}

/// Raw rows 1..=6 over remaining {1,2,3,4}; rows 5 and 6 are nontrivial.
fn base(family: &Arc<IntegralFamily>) -> TerminalMaps {
    let guard = Arc::new(vec![c(family, "d-4")]);
    let mut rows = BTreeMap::new();
    for power in 1..=4 {
        rows.insert(
            key(family, power),
            TerminalRow {
                terms: BTreeMap::from([(key(family, power), c(family, "1"))]),
                conditions: Arc::new(vec![]),
            },
        );
    }
    rows.insert(
        key(family, 5),
        TerminalRow {
            terms: BTreeMap::from([
                (key(family, 1), c(family, "d/(d-4)")),
                (key(family, 2), c(family, "2")),
                (key(family, 3), c(family, "d-3")),
            ]),
            conditions: guard.clone(),
        },
    );
    rows.insert(
        key(family, 6),
        TerminalRow {
            terms: BTreeMap::from([
                (key(family, 2), c(family, "1/(d-4)")),
                (key(family, 4), c(family, "-1")),
            ]),
            conditions: guard,
        },
    );
    TerminalMaps {
        families: BTreeMap::from([(family.fingerprint().to_owned(), family.clone())]),
        rows,
        remaining: (1..=4).map(|p| key(family, p)).collect(),
        layer: None,
    }
}

fn master(power: i64) -> SavedApplicationLayerMaster {
    SavedApplicationLayerMaster {
        key: vec![power],
        class: Some(json!(1)),
        origin: Some("test".into()),
        extra: BTreeMap::new(),
    }
}

fn row(
    family: &IntegralFamily,
    source: i64,
    terms: &[(i64, &str)],
    conditions: &[&str],
) -> SavedApplicationLayerRow {
    SavedApplicationLayerRow {
        source: IntegralKey::try_new([source]).unwrap(),
        terms: terms
            .iter()
            .map(|(m, t)| (IntegralKey::try_new([*m]).unwrap(), c(family, t)))
            .collect(),
        conditions: conditions.iter().map(|t| c(family, t)).collect(),
    }
}

fn layer_with(
    family: &IntegralFamily,
    rows: Vec<SavedApplicationLayerRow>,
    masters: &[i64],
) -> SavedApplicationLayer {
    SavedApplicationLayer {
        family_fingerprint: family.fingerprint().to_owned(),
        parameters: vec!["d".into()],
        base: SavedApplicationLayerBase {
            terminal_maps_blake3: String::new(),
            source_manifest_blake3: String::new(),
            source_scope_binding: String::new(),
            raw_terminals: 6,
            remaining_terminals: 4,
            application_blake3: None,
        },
        rows,
        masters: masters.iter().copied().map(master).collect(),
        unreduced: vec![],
        minimality: None,
        provenance: json!({"engine": "synthetic"}),
    }
}

/// Sources 2 and 3 onto masters 1 (kept terminal) and 7 (new key).
fn layer(family: &IntegralFamily) -> SavedApplicationLayer {
    layer_with(
        family,
        vec![
            row(family, 2, &[(1, "(d-2)/(2*d-7)"), (7, "1")], &["2*d-7"]),
            row(family, 3, &[(7, "3")], &[]),
        ],
        &[1, 7],
    )
}

#[test]
fn composition_is_exact_and_equals_sequential_application_mod_p() {
    let family = family();
    let base = base(&family);
    let layer = layer(&family);
    let bound = bind_layer(&base, &layer, &BTreeSet::new(), Default::default()).unwrap();
    let (composed, statistics) = compose_maps(&base, &bound, Default::default()).unwrap();
    composed.validate(Default::default()).unwrap();
    assert_eq!(
        composed.remaining,
        BTreeSet::from([key(&family, 1), key(&family, 4), key(&family, 7)])
    );
    // 5 = d/(d-4) [1] + 2 ((d-2)/(2d-7) [1] + [7]) + 3 (d-3) [7]
    let five = &composed.rows[&key(&family, 5)];
    assert_eq!(
        five.terms,
        BTreeMap::from([
            (key(&family, 1), c(&family, "d/(d-4) + 2*(d-2)/(2*d-7)")),
            (key(&family, 7), c(&family, "2 + 3*(d-3)")),
        ])
    );
    assert_eq!(*five.conditions, vec![c(&family, "d-4"), c(&family, "2*d-7")]);
    // Untouched rows keep their shared guard allocation.
    assert!(Arc::ptr_eq(
        &composed.rows[&key(&family, 4)].conditions,
        &base.rows[&key(&family, 4)].conditions
    ));
    // Row 3 -> 3 [7]; row 1 unchanged.
    assert_eq!(
        composed.rows[&key(&family, 3)].terms,
        BTreeMap::from([(key(&family, 7), c(&family, "3"))])
    );
    assert_eq!(statistics.rows_touched, 4);
    assert_eq!(statistics.source_occurrences, 5);
    assert_eq!(statistics.new_master_keys, 1);
    let sample: Vec<_> = base.rows.keys().cloned().collect();
    let (checked, points) =
        check_composition_mod_p(&base, &bound, &composed, &sample, 3, 42).unwrap();
    assert_eq!((checked, points.len()), (18, 3));
    // A corrupted composed coefficient is caught by the independent check.
    let mut broken = TerminalMaps {
        families: composed.families.clone(),
        rows: composed.rows.clone(),
        remaining: composed.remaining.clone(),
        layer: None,
    };
    broken
        .rows
        .get_mut(&key(&family, 5))
        .unwrap()
        .terms
        .insert(key(&family, 7), c(&family, "3*d-6"));
    assert!(check_composition_mod_p(&base, &bound, &broken, &sample, 2, 7).is_err());
    // The codec roundtrip of the composed maps is exact.
    let bytes = codec::encode(&composed, Default::default()).unwrap();
    let restored = codec::decode(&bytes, Default::default()).unwrap();
    assert_eq!(restored.rows, composed.rows);
    assert_eq!(restored.remaining, composed.remaining);
}

#[test]
fn identity_layer_reproduces_maps_and_bytes() {
    let family = family();
    let base = base(&family);
    let identity = layer_with(
        &family,
        (1..=4).map(|p| row(&family, p, &[(p, "1")], &[])).collect(),
        &[1, 2, 3, 4],
    );
    let bound = bind_layer(&base, &identity, &BTreeSet::new(), Default::default()).unwrap();
    let (composed, statistics) = compose_maps(&base, &bound, Default::default()).unwrap();
    assert_eq!(composed.rows, base.rows);
    assert_eq!(composed.remaining, base.remaining);
    assert_eq!(statistics.new_master_keys, 0);
    for (k, row) in &base.rows {
        assert!(Arc::ptr_eq(&row.conditions, &composed.rows[k].conditions));
    }
    assert_eq!(
        codec::encode(&composed, Default::default()).unwrap(),
        codec::encode(&base, Default::default()).unwrap()
    );
}

fn rejected(base: &TerminalMaps, layer: &SavedApplicationLayer, factorized: &[i64]) -> String {
    let family = base.families.values().next().unwrap();
    let factorized = factorized.iter().map(|p| key(family, *p)).collect();
    match bind_layer(base, layer, &factorized, Default::default()) {
        Ok(_) => panic!("layer must be rejected"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn malformed_layers_are_rejected() {
    let family = family();
    let base = base(&family);
    let ok = layer(&family);
    assert!(bind_layer(&base, &ok, &BTreeSet::new(), Default::default()).is_ok());
    // Wrong family.
    let mut wrong = ok.clone();
    wrong.family_fingerprint = "rustred-integral-family-v2;other".into();
    assert!(rejected(&base, &wrong, &[]).contains("family"));
    // Source not remaining (5 is a raw key only).
    let bad = layer_with(&family, vec![row(&family, 5, &[(7, "1")], &[])], &[7]);
    assert!(rejected(&base, &bad, &[]).contains("not a remaining terminal"));
    // Missing condition for a denominator.
    let bad = layer_with(&family, vec![row(&family, 2, &[(7, "1/(2*d-7)")], &[])], &[7]);
    assert!(rejected(&base, &bad, &[]).contains("not covered"));
    // A condition with a different integer normalization still covers.
    let good = layer_with(
        &family,
        vec![row(&family, 2, &[(7, "1/(2*d-8)")], &["d-4"])],
        &[7],
    );
    assert!(bind_layer(&base, &good, &BTreeSet::new(), Default::default()).is_ok());
    // Duplicated source.
    let bad = layer_with(
        &family,
        vec![row(&family, 2, &[(7, "1")], &[]), row(&family, 2, &[(7, "2")], &[])],
        &[7],
    );
    assert!(rejected(&base, &bad, &[]).contains("duplicate layer source"));
    // Zero coefficient.
    let mut bad = layer_with(&family, vec![row(&family, 2, &[(7, "1")], &[])], &[7]);
    bad.rows[0].terms[0].1 = family.coefficient_context().zero();
    assert!(rejected(&base, &bad, &[]).contains("zero coefficient"));
    // Undeclared master.
    let bad = layer_with(&family, vec![row(&family, 2, &[(8, "1")], &[])], &[7]);
    assert!(rejected(&base, &bad, &[]).contains("not a declared master"));
    // Rows must be fully reduced: 2 -> 3 while 3 is also a source.
    let bad = layer_with(
        &family,
        vec![row(&family, 2, &[(3, "1")], &[]), row(&family, 3, &[(3, "1")], &[])],
        &[3],
    );
    assert!(rejected(&base, &bad, &[]).contains("fully reduced"));
    // A master that is a source must have the identity row.
    let bad = layer_with(&family, vec![row(&family, 2, &[(2, "2")], &[])], &[2]);
    assert!(rejected(&base, &bad, &[]).contains("identity"));
    // Unused master.
    let bad = layer_with(&family, vec![row(&family, 2, &[(7, "1")], &[])], &[7, 9]);
    assert!(rejected(&base, &bad, &[]).contains("not used"));
    // Factorized terminals stay on the product route.
    assert!(rejected(&base, &ok, &[3]).contains("factorized"));
    // Unreduced entries must be remaining non-sources.
    let mut bad = ok.clone();
    bad.unreduced = vec![SavedApplicationLayerUnreduced {
        key: vec![2],
        reason: "rank".into(),
    }];
    assert!(rejected(&base, &bad, &[]).contains("unreduced"));
    // Empty row.
    let bad = layer_with(&family, vec![row(&family, 2, &[], &[])], &[]);
    assert!(rejected(&base, &bad, &[]).contains("empty"));
}

#[test]
fn coefficient_text_grammar_is_strict() {
    let family = family();
    assert_eq!(c(&family, "(d-2)/(2*d-7)"), c(&family, "(2-d)/(7-2*d)"));
    assert_eq!(c(&family, "d^2-1"), c(&family, "(d-1)*(d+1)"));
    for bad in ["", "x+1", "1.5", "d;1", "sin(d)"] {
        assert!(
            parse_layer_coefficient(&family, bad, Default::default()).is_err(),
            "{bad:?} must be rejected"
        );
    }
}

#[test]
fn layer_files_roundtrip_with_native_table_and_text() {
    let scratch = Scratch::new();
    let family = family();
    let layer = layer(&family);
    let directory = scratch.0.join("layer");
    let digest = layer.write(&directory, Default::default()).unwrap();
    let (read, read_digest) =
        SavedApplicationLayer::read(&directory, &family, Default::default(), Default::default())
            .unwrap();
    assert_eq!(digest, read_digest);
    assert_eq!(read.rows, layer.rows);
    assert_eq!(read.masters, layer.masters);
    // The same rows in the text form give the same resolved layer.
    let text = json!({
        "schema": SAVED_APPLICATION_LAYER_SCHEMA,
        "family_fingerprint": family.fingerprint(),
        "parameters": ["d"],
        "base": layer.base,
        "rows": [
            {"source": [2], "terms": [{"master": [1], "coefficient": "(d-2)/(2*d-7)"},
                                      {"master": [7], "coefficient": "1"}],
             "conditions": ["2*d-7"]},
            {"source": [3], "terms": [{"master": [7], "coefficient": "3"}]}
        ],
        "masters": [{"key": [1], "class": 1, "origin": "test"},
                    {"key": [7], "class": 1, "origin": "test"}],
        "provenance": {"engine": "synthetic"}
    });
    let text_path = scratch.0.join("text.json");
    std::fs::write(&text_path, serde_json::to_vec(&text).unwrap()).unwrap();
    let (from_text, _) =
        SavedApplicationLayer::read(&text_path, &family, Default::default(), Default::default())
            .unwrap();
    assert_eq!(from_text.rows, layer.rows);
    // Tampered table bytes or an unknown field are refused.
    let table = directory.join("layer.rrbin");
    let mut bytes = std::fs::read(&table).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(&table, bytes).unwrap();
    assert!(
        SavedApplicationLayer::read(&directory, &family, Default::default(), Default::default())
            .is_err()
    );
    let mut unknown = text.clone();
    unknown["rows"][0]["weight"] = json!(1);
    std::fs::write(&text_path, serde_json::to_vec(&unknown).unwrap()).unwrap();
    assert!(
        SavedApplicationLayer::read(&text_path, &family, Default::default(), Default::default())
            .is_err()
    );
}

#[test]
fn minimality_claim_requires_complete_matching_class_counts() {
    let family = family();
    let mut layer = layer(&family);
    assert!(!layer_minimality_claim(&layer).0);
    let classes = |counts: &[(u64, u64)]| {
        counts
            .iter()
            .enumerate()
            .map(|(i, (m, e))| SavedApplicationLayerClassCount {
                class: json!(i),
                masters: *m,
                expected: *e,
                sources: None,
            })
            .collect::<Vec<_>>()
    };
    layer.minimality = Some(SavedApplicationLayerMinimality {
        reference: "expected-counts.json".into(),
        reference_classes: 1,
        classes: classes(&[(2, 2)]),
    });
    assert!(layer_minimality_claim(&layer).0);
    // A deviating class, a missing class, or unreduced terminals forbid the claim.
    layer.minimality.as_mut().unwrap().classes = classes(&[(2, 1)]);
    assert!(!layer_minimality_claim(&layer).0);
    layer.minimality.as_mut().unwrap().reference_classes = 2;
    layer.minimality.as_mut().unwrap().classes = classes(&[(2, 2)]);
    assert!(!layer_minimality_claim(&layer).0);
    layer.minimality.as_mut().unwrap().reference_classes = 1;
    layer.unreduced = vec![SavedApplicationLayerUnreduced {
        key: vec![4],
        reason: "T2b".into(),
    }];
    assert!(!layer_minimality_claim(&layer).0);
    layer.unreduced.clear();
    // Counts must sum to the layer's masters.
    layer.minimality.as_mut().unwrap().classes = classes(&[(3, 3)]);
    assert!(!layer_minimality_claim(&layer).0);
}

/// Package-level: export a base runtime, compose a text layer through the
/// public entry point, reload, and compare reductions; then the layered
/// fallback, its loader, and the refusal by a layer-unaware loader.
#[test]
fn compose_layer_package_roundtrip_composed_and_layered() {
    let scratch = Scratch::new();
    let mut original = prepared_fixture(&scratch.0.join("source"));
    let fingerprint = original.family().fingerprint().to_owned();
    let family = original.output_family_shared(&fingerprint).unwrap();
    let context = family.coefficient_context().clone();
    let base_dir = scratch.0.join("pack/ibps");
    std::fs::create_dir(scratch.0.join("pack")).unwrap();
    original.export_runtime(&base_dir).unwrap();
    // The fixture has no source campaign; give it the identity fields a real
    // publication carries (the loader does not interpret them).
    let manifest = base_dir.join("application.json");
    let mut base_report: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    base_report["metadata"]["source_manifest_blake3"] = json!("f".repeat(64));
    base_report["metadata"]["source_scope_binding"] = json!("e".repeat(64));
    std::fs::write(&manifest, serde_json::to_vec(&base_report).unwrap()).unwrap();
    let input = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
    ];
    let expected = original.reduce_linear_combination(input.clone()).unwrap();
    assert_eq!(expected.terms().len(), 1);
    // Base terminal definitions next to ibps/, as in the vakint pack.
    std::fs::write(
        scratch.0.join("pack/terminal-definitions.json"),
        serde_json::to_vec(&json!({
            "schema": "rustred.native-master-definitions.v1",
            "families": [{"fingerprint": family.fingerprint(), "loop_count": 1}],
            "masters": [{"family_fingerprint": family.fingerprint(), "loop_count": 1,
                         "master_id": "P1", "powers": [1]}],
            "minimality_claim": false, "refinement_binding": "base",
            "metadata": {"remaining_terminals": 1}
        }))
        .unwrap(),
    )
    .unwrap();
    let base = saved_application_layer_base(&base_dir).unwrap();
    let layer = json!({
        "schema": SAVED_APPLICATION_LAYER_SCHEMA,
        "family_fingerprint": family.fingerprint(),
        "parameters": ["d"],
        "base": base,
        "rows": [{"source": [1], "terms": [{"master": [2], "coefficient": "2/(d-2)"}],
                  "conditions": ["d-2"]}],
        "masters": [{"key": [2], "class": "tadpole", "origin": "engine"}],
        "minimality": {"reference": "synthetic", "reference_classes": 1,
                       "classes": [{"class": "tadpole", "masters": 1, "expected": 1}]},
        "provenance": {"engine": "synthetic"}
    });
    let layer_path = scratch.0.join("layer.json");
    std::fs::write(&layer_path, serde_json::to_vec(&layer).unwrap()).unwrap();
    let factor = context
        .try_div(&context.integer(2), &c(&family, "d-2"), Default::default())
        .unwrap();
    let mut expected_terms = BTreeMap::new();
    for (k, v) in expected.terms() {
        assert_eq!(k.integral.powers(), [1]);
        expected_terms.insert(
            key(&family, 2),
            context.try_mul(v, &factor, Default::default()).unwrap(),
        );
    }

    // Composed package (gate passes).
    let out = scratch.0.join("composed");
    let outcome = compose_saved_application_layer::<1>(
        &base_dir,
        &layer_path,
        &out,
        &SavedApplicationComposeOptions::default(),
    )
    .unwrap();
    assert_eq!(outcome.mode, "composed", "{}", outcome.report);
    assert_eq!(outcome.report["minimality_claim"], true);
    let mut composed = load_saved_application::<1>(&out, Default::default()).unwrap();
    assert_eq!(
        composed.remaining_terminals(),
        &BTreeSet::from([key(&family, 2)])
    );
    assert_eq!(composed.metadata()["remaining_terminals"], 1);
    assert_eq!(
        composed.metadata()["refinement_layer"]["layer_blake3"],
        outcome.report["layer_blake3"]
    );
    let actual = composed.reduce_linear_combination(input.clone()).unwrap();
    assert_eq!(actual.terms(), &expected_terms);
    assert!(actual.nonzero_conditions().contains(&c(&family, "d-2")));
    let definitions: Value = serde_json::from_slice(
        &std::fs::read(scratch.0.join("composed.terminal-definitions.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(definitions["masters"][0]["powers"], json!([2]));
    assert_eq!(definitions["base_refinement_binding"], "base");
    assert_eq!(definitions["metadata"]["remaining_terminals"], 1);
    // Never replaces an existing package.
    assert!(
        compose_saved_application_layer::<1>(
            &base_dir,
            &layer_path,
            &out,
            &Default::default()
        )
        .is_err()
    );

    // Size gate failure without --layered writes nothing.
    let rejected = scratch.0.join("rejected");
    let options = SavedApplicationComposeOptions {
        size_gate: 0.0,
        ..Default::default()
    };
    let outcome =
        compose_saved_application_layer::<1>(&base_dir, &layer_path, &rejected, &options).unwrap();
    assert_eq!(outcome.mode, "rejected");
    assert!(!rejected.exists());

    // Layered fallback: base maps unchanged, final substitution at runtime.
    let layered = scratch.0.join("layered");
    let options = SavedApplicationComposeOptions {
        size_gate: 0.0,
        allow_layered: true,
        ..Default::default()
    };
    let outcome =
        compose_saved_application_layer::<1>(&base_dir, &layer_path, &layered, &options).unwrap();
    assert_eq!(outcome.mode, "layered", "{}", outcome.report);
    let report: Value =
        serde_json::from_slice(&std::fs::read(layered.join("application.json")).unwrap()).unwrap();
    let base_report: Value =
        serde_json::from_slice(&std::fs::read(base_dir.join("application.json")).unwrap())
            .unwrap();
    assert_eq!(
        report["terminal_maps"]["blake3"],
        base_report["terminal_maps"]["blake3"]
    );
    assert!(report.get("terminal_layer").is_some());
    let mut runtime = load_saved_application::<1>(&layered, Default::default()).unwrap();
    assert_eq!(
        runtime.remaining_terminals(),
        &BTreeSet::from([key(&family, 2)])
    );
    let actual = runtime.reduce_linear_combination(input).unwrap();
    assert_eq!(actual.terms(), &expected_terms);
    assert!(actual.nonzero_conditions().contains(&c(&family, "d-2")));
    // A loader unaware of the layer sees only the base maps; the composed
    // count in the metadata makes it refuse instead of loading a wrong basis.
    let mut legacy = report.clone();
    legacy.as_object_mut().unwrap().remove("terminal_layer");
    std::fs::write(
        layered.join("application.json"),
        serde_json::to_vec(&legacy).unwrap(),
    )
    .unwrap();
    let error = load_saved_application::<1>(&layered, Default::default())
        .err()
        .unwrap();
    assert!(error.to_string().contains("terminal counts"), "{error}");
    // Unknown top-level keys are refused, never ignored.
    let mut future = report;
    future["terminal_future"] = json!({});
    std::fs::write(
        layered.join("application.json"),
        serde_json::to_vec(&future).unwrap(),
    )
    .unwrap();
    let error = load_saved_application::<1>(&layered, Default::default())
        .err()
        .unwrap();
    assert!(error.to_string().contains("not supported"), "{error}");

    // A layer computed against another base is refused.
    let mut stale = layer;
    stale["base"]["terminal_maps_blake3"] = json!("0".repeat(64));
    std::fs::write(&layer_path, serde_json::to_vec(&stale).unwrap()).unwrap();
    let error = compose_saved_application_layer::<1>(
        &base_dir,
        &layer_path,
        &scratch.0.join("stale"),
        &Default::default(),
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("terminal_maps_blake3"), "{error}");
}
