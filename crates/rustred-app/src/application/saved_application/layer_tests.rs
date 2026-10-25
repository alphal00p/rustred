//! RR-11 layer tests on synthetic rules (design §3 "Tests", acceptance L1).
use super::compose::{
    SavedApplicationComposeOptions, SavedApplicationConsumerLimits, compose_saved_application_layer,
};
use super::layer::*;
use super::*;
use rustred::algebra::CoefficientContext;
use rustred::family::AffineDenominator;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Self-cleaning scratch directory under the process temp dir.
pub struct Scratch(pub PathBuf);
impl Scratch {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-layer-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One-propagator tadpole owner with entries at powers 2 and 4 and a single
/// identity terminal row (same construction as the saved-application tests).
fn prepared_fixture(source: &Path) -> PreparedSavedApplication<1> {
    use crate::{
        CandidateOwnerBundle, FamilyCandidatesRequest, family_candidates,
        load_generated_candidate_owners,
    };
    std::fs::create_dir_all(source.join("objects")).unwrap();
    let source_text = r#"
schema="rustred.project.toml.v1"
[family]
name="saved_application_layer_test"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
    let mut request = FamilyCandidatesRequest::new(source_text);
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(2);
    let bundle = family_candidates(request).unwrap();
    let bytes = bundle.bundle();
    let (family, _) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &rustred::sector::Mask::try_new([true]).unwrap(),
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    std::fs::write(source.join("objects/owner.rrbin"), bytes).unwrap();
    let selection = json!({"family_fingerprint":family.fingerprint(),
        "owners":[{"path":"objects/owner.rrbin","bytes":bytes.len(),"mask":"1"}],
        "initial_frontier_routes":[]})
    .to_string();
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"two", "owner":"1", "lower":[1], "upper":[1], "max_numerator_rank":0},
        {"id":"four", "owner":"1", "lower":[3], "upper":[3], "max_numerator_rank":0}
    ]})
    .to_string();
    let context = family.coefficient_context();
    let guard = context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(4),
            Default::default(),
        )
        .unwrap();
    let qualified = SavedApplicationIntegral {
        family_fingerprint: family.fingerprint().to_owned(),
        integral: IntegralKey::try_new([1]).unwrap(),
    };
    let terminals = TerminalMaps {
        families: BTreeMap::from([(family.fingerprint().to_owned(), family.clone())]),
        rows: BTreeMap::from([(
            qualified.clone(),
            TerminalRow {
                terms: BTreeMap::from([(qualified.clone(), context.one())]),
                conditions: Arc::new(vec![guard]),
            },
        )]),
        remaining: BTreeSet::from([qualified]),
        layer: None,
    };
    let payloads =
        serde_json::from_value(json!([{"path":"objects/owner.rrbin","bytes":bytes.len(),
        "blake3":blake3::hash(bytes).to_hex().to_string()}]))
        .unwrap();
    let metadata = json!({"family_fingerprint":family.fingerprint(),"raw_terminals":1,"remaining_terminals":1,
        "source_keys_checked":1,"publication_assurance":"trusted_saved_scope","minimality_claim":false});
    PreparedSavedApplication::from_parts(
        package::ApplicationInputs {
            selection,
            queries,
            owner_base: source.to_path_buf(),
            payloads,
        },
        terminals,
        metadata,
        Default::default(),
    )
    .unwrap()
}

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
        provenance: SavedApplicationRuleProvenance::SyntheticTest,
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
        provenance: json!({"kind": "synthetic-test", "engine": "synthetic"}),
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

#[cfg_attr(test, test)]
pub fn composition_is_exact_and_equals_sequential_application_mod_p() {
    let family = family();
    let base = base(&family);
    let layer = layer(&family);
    let bound = bind_layer(&base, &layer, &BTreeSet::new(), Default::default()).unwrap();
    // Terminal 4 is neither a source, a master nor declared unreduced.
    assert_eq!(bound.undeclared, BTreeSet::from([key(&family, 4)]));
    let (composed, statistics) = compose_maps(&base, &bound, Default::default()).unwrap();
    composed.validate(Default::default()).unwrap();
    // Master 1 is a raw-row key and stays the identity.
    assert_eq!(
        check_masters_compose_to_identity(&composed, &bound).unwrap(),
        1
    );
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
    assert_eq!(
        *five.conditions,
        vec![c(&family, "d-4"), c(&family, "2*d-7")]
    );
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

#[cfg_attr(test, test)]
pub fn identity_layer_reproduces_maps_and_bytes() {
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

#[cfg_attr(test, test)]
pub fn malformed_layers_are_rejected() {
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
    let bad = layer_with(
        &family,
        vec![row(&family, 2, &[(7, "1/(2*d-7)")], &[])],
        &[7],
    );
    assert!(rejected(&base, &bad, &[]).contains("not covered"));
    // A condition with a different integer normalization still covers, and
    // so do conditions that are powers or products of denominator factors.
    for (coefficient, condition) in [
        ("1/(2*d-8)", "d-4"),
        ("1/(d-4)", "(d-4)^2"),
        ("1/((d-4)^3*(2*d-7))", "(d-4)*(2*d-7)"),
        ("1/((d-4)*(d-5))", "1/((d-4)^2*(d-5))"),
    ] {
        let good = layer_with(
            &family,
            vec![row(&family, 2, &[(7, coefficient)], &[condition])],
            &[7],
        );
        assert!(
            bind_layer(&base, &good, &BTreeSet::new(), Default::default()).is_ok(),
            "{coefficient} under {condition}"
        );
    }
    let bad = layer_with(
        &family,
        vec![row(&family, 2, &[(7, "1/((d-4)*(d-5))")], &["(d-4)^2"])],
        &[7],
    );
    assert!(rejected(&base, &bad, &[]).contains("not covered"));
    // Duplicated source.
    let bad = layer_with(
        &family,
        vec![
            row(&family, 2, &[(7, "1")], &[]),
            row(&family, 2, &[(7, "2")], &[]),
        ],
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
        vec![
            row(&family, 2, &[(3, "1")], &[]),
            row(&family, 3, &[(3, "1")], &[]),
        ],
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
    // Unreduced entries must be remaining terminals, neither sources nor masters.
    for k in [2, 1] {
        let mut bad = ok.clone();
        bad.unreduced = vec![SavedApplicationLayerUnreduced {
            key: vec![k],
            reason: "rank".into(),
        }];
        assert!(rejected(&base, &bad, &[]).contains("unreduced"));
    }
    let mut declared = ok.clone();
    declared.unreduced = vec![SavedApplicationLayerUnreduced {
        key: vec![4],
        reason: "T2b".into(),
    }];
    let bound = bind_layer(&base, &declared, &BTreeSet::new(), Default::default()).unwrap();
    assert!(bound.undeclared.is_empty());
    // An identity-provenance row must be source = source.
    let mut bad = layer_with(&family, vec![row(&family, 2, &[(7, "1")], &[])], &[7]);
    bad.rows[0].provenance = SavedApplicationRuleProvenance::Identity;
    assert!(rejected(&base, &bad, &[]).contains("provenance identity"));
    // Empty row; empty layer.
    let bad = layer_with(&family, vec![row(&family, 2, &[], &[])], &[]);
    assert!(rejected(&base, &bad, &[]).contains("empty"));
    let bad = layer_with(&family, vec![], &[]);
    assert!(rejected(&base, &bad, &[]).contains("no rows"));
}

/// A master that is a raw-row key (here 5, reduced by the base onto 1, 2, 3)
/// must compose to itself; a layer contradicting the base is refused.
#[cfg_attr(test, test)]
pub fn masters_with_raw_rows_must_compose_to_identity() {
    let family = family();
    let base = base(&family);
    // 5 = d/(d-4) [1] + 2 [2] + (d-3) [3]  =>  3 = ([5] - d/(d-4) [1] - 2 [2]) / (d-3).
    let consistent = layer_with(
        &family,
        vec![row(
            &family,
            3,
            &[(5, "1/(d-3)"), (1, "-d/((d-4)*(d-3))"), (2, "-2/(d-3)")],
            &["d-3", "d-4"],
        )],
        &[1, 2, 5],
    );
    let bound = bind_layer(&base, &consistent, &BTreeSet::new(), Default::default()).unwrap();
    let (composed, _) = compose_maps(&base, &bound, Default::default()).unwrap();
    assert_eq!(
        composed.rows[&key(&family, 5)].terms,
        BTreeMap::from([(key(&family, 5), c(&family, "1"))])
    );
    assert_eq!(
        check_masters_compose_to_identity(&composed, &bound).unwrap(),
        3
    );
    let wrong = layer_with(&family, vec![row(&family, 3, &[(5, "1")], &[])], &[5]);
    let bound = bind_layer(&base, &wrong, &BTreeSet::new(), Default::default()).unwrap();
    let (composed, _) = compose_maps(&base, &bound, Default::default()).unwrap();
    let error = check_masters_compose_to_identity(&composed, &bound)
        .unwrap_err()
        .to_string();
    assert!(error.contains("inconsistent"), "{error}");
}

#[cfg_attr(test, test)]
pub fn coefficient_text_grammar_is_strict() {
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

#[cfg_attr(test, test)]
pub fn layer_files_roundtrip_with_native_table_and_text() {
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
        "provenance": {"kind": "synthetic-test", "engine": "synthetic"}
    });
    let text_path = scratch.0.join("text.json");
    std::fs::write(&text_path, serde_json::to_vec(&text).unwrap()).unwrap();
    let (from_text, _) =
        SavedApplicationLayer::read(&text_path, &family, Default::default(), Default::default())
            .unwrap();
    assert_eq!(from_text.rows, layer.rows);
    let read_text = |value: &Value| {
        std::fs::write(&text_path, serde_json::to_vec(value).unwrap()).unwrap();
        SavedApplicationLayer::read(&text_path, &family, Default::default(), Default::default())
    };
    // The written layer uses tagged table references.
    let written: Value =
        serde_json::from_slice(&std::fs::read(directory.join("layer.json")).unwrap()).unwrap();
    assert_eq!(written["schema"], "rustred.saved-application-layer.v2");
    assert!(written["rows"][0]["terms"][0]["coefficient"]["id"].is_u64());
    assert_eq!(written["rows"][0]["provenance"], "synthetic-test");
    // A bare JSON number is refused (v2 and legacy v1), never read as a table ID.
    for schema in [
        SAVED_APPLICATION_LAYER_SCHEMA,
        SAVED_APPLICATION_LAYER_SCHEMA_V1,
    ] {
        let mut bare = text.clone();
        bare["schema"] = json!(schema);
        bare["rows"][1]["terms"][0]["coefficient"] = json!(3);
        let error = read_text(&bare).err().unwrap().to_string();
        assert!(error.contains("ambiguous"), "{error}");
        let mut legacy = text.clone();
        legacy["schema"] = json!(schema);
        assert_eq!(read_text(&legacy).unwrap().0.rows, layer.rows);
    }
    // {"id": n} without a table, or a malformed object, is refused.
    let mut tagged = text.clone();
    tagged["rows"][1]["terms"][0]["coefficient"] = json!({"id": 0});
    assert!(read_text(&tagged).is_err());
    tagged["rows"][1]["terms"][0]["coefficient"] = json!({"value": "3"});
    assert!(read_text(&tagged).is_err());
    // Provenance: layer default, row override, unknown kinds and missing tags.
    let mut stopgap = text.clone();
    stopgap["provenance"] = json!({"kind": "kira-stopgap"});
    stopgap["rows"][1]["provenance"] = json!("rustred");
    let (read, _) = read_text(&stopgap).unwrap();
    assert_eq!(
        read.rows.iter().map(|r| r.provenance).collect::<Vec<_>>(),
        [
            SavedApplicationRuleProvenance::KiraStopgap,
            SavedApplicationRuleProvenance::Rustred
        ]
    );
    assert_eq!(read.provenance_counts()["kira-stopgap"], 1);
    assert_eq!(read.provenance_counts()["rustred"], 1);
    assert_eq!(read.flagged_sources()["kira-stopgap"], vec![vec![2]]);
    let authority = read.authority("0123456789abcdef");
    assert!(
        authority.contains("1 kira-stopgap rows") && authority.contains("1 RustRed relation rows"),
        "{authority}"
    );
    let mut mixed = stopgap.clone();
    mixed["provenance"] = json!({"kind": "mixed"});
    assert!(
        read_text(&mixed)
            .err()
            .unwrap()
            .to_string()
            .contains("no provenance")
    );
    let mut untagged = stopgap.clone();
    untagged["provenance"] = json!({"engine": "x"});
    untagged["rows"][1]
        .as_object_mut()
        .unwrap()
        .remove("provenance");
    assert!(read_text(&untagged).is_err());
    let mut unknown_kind = stopgap.clone();
    unknown_kind["provenance"] = json!({"kind": "kira"});
    assert!(
        read_text(&unknown_kind)
            .err()
            .unwrap()
            .to_string()
            .contains("kind")
    );
    let mut unknown_row = stopgap;
    unknown_row["rows"][0]["provenance"] = json!("fire-fly");
    assert!(read_text(&unknown_row).is_err());
    std::fs::write(&text_path, serde_json::to_vec(&text).unwrap()).unwrap();
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

#[cfg_attr(test, test)]
pub fn minimality_claim_requires_complete_matching_class_counts() {
    let family = family();
    let mut layer = layer(&family);
    let complete = SavedApplicationMinimalityScope {
        unreduced: 0,
        factorized: 2,
        reference_verified: true,
    };
    assert!(!layer_minimality_claim(&layer, &complete).0);
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
        reference_blake3: Some("ab".repeat(32)),
        reference_classes: 1,
        classes: classes(&[(2, 2)]),
    });
    let (claim, scope) = layer_minimality_claim(&layer, &complete);
    assert!(claim);
    assert!(scope.contains("2 factorized terminals"), "{scope}");
    // Unreduced base terminals (declared or filled), or an unverified
    // reference, forbid the claim whatever the layer states.
    let partial = SavedApplicationMinimalityScope {
        unreduced: 1,
        ..complete.clone()
    };
    assert!(!layer_minimality_claim(&layer, &partial).0);
    let unverified = SavedApplicationMinimalityScope {
        reference_verified: false,
        ..complete.clone()
    };
    assert!(!layer_minimality_claim(&layer, &unverified).0);
    // A deviating class, a missing class, or class counts not summing to the masters.
    layer.minimality.as_mut().unwrap().classes = classes(&[(2, 1)]);
    assert!(!layer_minimality_claim(&layer, &complete).0);
    layer.minimality.as_mut().unwrap().reference_classes = 2;
    layer.minimality.as_mut().unwrap().classes = classes(&[(2, 2)]);
    assert!(!layer_minimality_claim(&layer, &complete).0);
    layer.minimality.as_mut().unwrap().reference_classes = 1;
    layer.minimality.as_mut().unwrap().classes = classes(&[(3, 3)]);
    assert!(!layer_minimality_claim(&layer, &complete).0);
}

/// Package-level: export a base runtime, compose a text layer through the
/// public entry point, reload, and compare reductions; then the layered
/// fallback, its loader, and the refusal by a layer-unaware loader.
#[cfg_attr(test, test)]
pub fn compose_layer_package_roundtrip_composed_and_layered() {
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
    let mut base_report: Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
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
                       "reference_blake3": blake3::hash(b"reference counts").to_hex().to_string(),
                       "classes": [{"class": "tadpole", "masters": 1, "expected": 1}]},
        "provenance": {"kind": "synthetic-test", "engine": "synthetic"}
    });
    let reference = scratch.0.join("expected-counts.json");
    std::fs::write(&reference, b"reference counts").unwrap();
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

    // Without the reference file the claim is withheld; a wrong file is refused.
    let unverified = compose_saved_application_layer::<1>(
        &base_dir,
        &layer_path,
        &scratch.0.join("unverified"),
        &SavedApplicationComposeOptions {
            dry_run: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(unverified.report["minimality_claim"], false);
    let other = scratch.0.join("other-counts.json");
    std::fs::write(&other, b"other counts").unwrap();
    let error = compose_saved_application_layer::<1>(
        &base_dir,
        &layer_path,
        &scratch.0.join("wrong-reference"),
        &SavedApplicationComposeOptions {
            dry_run: true,
            minimality_reference: Some(other),
            ..Default::default()
        },
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("reference"), "{error}");

    // Composed package (gate passes); the loader's worker count is not recorded.
    let out = scratch.0.join("composed");
    let mut options = SavedApplicationComposeOptions {
        minimality_reference: Some(reference.clone()),
        ..Default::default()
    };
    options.application.preparation_workers = 3;
    let outcome =
        compose_saved_application_layer::<1>(&base_dir, &layer_path, &out, &options).unwrap();
    assert_eq!(outcome.mode, "composed", "{}", outcome.report);
    assert_eq!(outcome.report["minimality_claim"], true);
    assert_eq!(outcome.report["provenance_counts"]["synthetic-test"], 1);
    assert!(
        std::fs::read_dir(&scratch.0).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with('.')),
        "staging leftovers"
    );
    let written: Value =
        serde_json::from_slice(&std::fs::read(out.join("application.json")).unwrap()).unwrap();
    assert_eq!(
        written["metadata"]["preparation_workers"],
        base_report["metadata"]["preparation_workers"]
    );
    assert_eq!(
        written["terminal_maps"]["blake3"],
        outcome.report["written_terminal_maps_blake3"]
    );
    let authority = written["metadata"]["authority"].as_str().unwrap();
    assert!(authority.contains("SYNTHETIC TEST"), "{authority}");
    assert!(!authority.contains("RustRed relation"), "{authority}");
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
    assert_eq!(
        definitions["refinement_provenance"]["counts"]["synthetic-test"],
        1
    );
    assert_eq!(definitions["minimality_claim"], true);
    assert_eq!(definitions["base_refinement_binding"], "base");
    assert_eq!(definitions["metadata"]["remaining_terminals"], 1);
    // Never replaces an existing package.
    assert!(
        compose_saved_application_layer::<1>(&base_dir, &layer_path, &out, &Default::default())
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
    // So does a composed package beyond the consumer's binary limits, even
    // when the ratio gate passes.
    let tight = SavedApplicationComposeOptions {
        consumer_limits: SavedApplicationConsumerLimits {
            name: "tight",
            max_program_bytes: 64,
            ..SavedApplicationConsumerLimits::VAKINT_PIN_52C74636
        },
        ..Default::default()
    };
    let outcome =
        compose_saved_application_layer::<1>(&base_dir, &layer_path, &rejected, &tight).unwrap();
    assert_eq!(outcome.mode, "rejected", "{}", outcome.report);
    assert_eq!(outcome.report["size_gate"]["ratio_passed"], true);
    assert_eq!(
        outcome.report["size_gate"]["consumer_limits"]["passed"],
        false
    );
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
        serde_json::from_slice(&std::fs::read(base_dir.join("application.json")).unwrap()).unwrap();
    assert_eq!(
        report["terminal_maps"]["blake3"],
        base_report["terminal_maps"]["blake3"]
    );
    assert!(report.get("terminal_layer").is_some());
    assert_eq!(
        report["metadata"]["refinement_layer"]["base_terminal_maps_reencoded_blake3"],
        report["terminal_maps"]["blake3"]
    );
    assert_eq!(outcome.report["reload_check"]["base_rows_equal"], 1);
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
    assert!(
        error.to_string().contains("terminal_maps_blake3"),
        "{error}"
    );
}
