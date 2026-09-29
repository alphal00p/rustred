use super::super::super::tests::{Directory, state};
use super::*;
use rustred::solver::DomainPowerBounds;
use std::fs;

fn query(id: &str, point: u64, auxiliary: bool, role_declared: bool) -> Query {
    Query {
        id: id.into(),
        auxiliary,
        role_declared,
        owner: vec![true, false],
        lower: vec![point, 0],
        upper: vec![Some(point), Some(0)],
        rank: Some(100),
        powers: DomainPowerBounds::default(),
    }
}
fn row(query: &Query, domain: Option<u32>) -> Value {
    super::super::super::super::input_row(query, domain)
}
fn frontier(query: &Query) -> Value {
    json!({"id":query.id,"kind":"initial_route_source_validity_obligation",
        "owner":mask::<2>(query.owner.as_slice().try_into().unwrap()),"lower":query.lower,"upper":query.upper,"rank":query.rank,
        "power_bounds":power_bounds_json(query.powers),"reached_missing_rule_claim":false})
}
fn file(directory: &Path, name: &str, rows: &[Value]) -> FileRef {
    let mut bytes = Vec::new();
    for row in rows {
        serde_json::to_writer(&mut bytes, row).unwrap();
        bytes.push(b'\n');
    }
    fs::write(directory.join(name), &bytes).unwrap();
    FileRef {
        key: name.into(),
        file: name.into(),
        count: rows.len() as u64,
        bytes: bytes.len() as u64,
        blake3: *blake3::hash(&bytes).as_bytes(),
    }
}

#[test]
fn exact_roles_reused_roots_and_partial_admission_preserve_request_order() {
    let directory = Directory::new();
    let state = state(2);
    let queries = [
        query("helper-but-required", 0, false, false),
        query("ordinary-aux", 1, true, true),
        query("same-root", 0, false, true),
    ];
    let rows = [
        row(&queries[0], Some(0)),
        row(&queries[1], Some(1)),
        row(&queries[2], Some(0)),
    ];
    let inputs = file(&directory.0, "inputs", &rows);
    let frontiers = file(&directory.0, "frontiers", &[]);
    let restored = read_with_phase(
        &directory.0,
        &inputs,
        &frontiers,
        &queries,
        &state.store,
        2,
        |_| Some(Phase::Apply),
    )
    .unwrap();
    assert_eq!(restored.rows, rows);
    assert!(restored.frontiers.is_empty());
    let inputs = file(&directory.0, "inputs", &rows[..2]);
    assert_eq!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            2,
            |_| Some(Phase::Apply)
        )
        .unwrap()
        .rows
        .len(),
        2
    );
    // A prefix cannot hide another already admitted initial ID.
    let inputs = file(&directory.0, "inputs", &rows[..1]);
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            2,
            |_| Some(Phase::Apply)
        )
        .is_err()
    );
}

#[test]
fn root_order_roles_geometry_and_missing_domain_mutations_refuse() {
    let directory = Directory::new();
    let state = state(2);
    let queries = [query("q0", 0, false, false), query("q1", 1, true, true)];
    let rows = [row(&queries[0], Some(0)), row(&queries[1], Some(1))];
    let frontiers = file(&directory.0, "frontiers", &[]);
    for (index, field, value) in [
        (0, "domain", json!(1)),
        (1, "domain", json!(0)),
        (1, "role", json!("required")),
        (0, "role_declared", json!(true)),
        (0, "id", json!("q1")),
        (0, "unexpected", json!(true)),
        (0, "domain", Value::Null),
    ] {
        let mut changed = rows.clone();
        changed[index][field] = value;
        let inputs = file(&directory.0, "inputs", &changed);
        assert!(
            read_with_phase(
                &directory.0,
                &inputs,
                &frontiers,
                &queries,
                &state.store,
                2,
                |_| Some(Phase::Apply)
            )
            .is_err(),
            "{index} {field}"
        );
    }
    let mut changed = rows.clone();
    changed[0].as_object_mut().unwrap().remove("domain");
    let inputs = file(&directory.0, "inputs", &changed);
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            2,
            |_| Some(Phase::Apply)
        )
        .is_err()
    );
    let inputs = file(&directory.0, "inputs", &rows);
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            2,
            |_| Some(Phase::Route)
        )
        .is_err()
    );
}

#[test]
fn authenticated_missing_owner_phase_and_source_obligations_are_distinct() {
    assert!(phase(true, true, true) == Some(Phase::Apply));
    assert!(phase(false, false, true) == Some(Phase::Apply));
    assert!(phase(false, true, false) == Some(Phase::Route));
    assert!(phase(false, true, true).is_none());
    let directory = Directory::new();
    let state = state(0);
    let queries = [query("unresolved", 7, true, true)];
    let mut unresolved = row(&queries[0], None);
    unresolved["source_validity_unresolved"] = true.into();
    let inputs = file(&directory.0, "inputs", &[unresolved.clone()]);
    let saved_frontier = frontier(&queries[0]);
    let frontiers = file(&directory.0, "frontiers", &[saved_frontier.clone()]);
    let decoded = read_with_phase(
        &directory.0,
        &inputs,
        &frontiers,
        &queries,
        &state.store,
        0,
        |_| None,
    )
    .unwrap();
    assert_eq!(decoded.rows, [unresolved]);
    assert_eq!(decoded.frontiers, [saved_frontier.clone()]);
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            0,
            |_| Some(Phase::Route)
        )
        .is_err()
    );
    let mut changed = saved_frontier;
    changed["lower"][0] = 8.into();
    let frontiers = file(&directory.0, "frontiers", &[changed]);
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            0,
            |_| None
        )
        .is_err()
    );
}

#[test]
fn legitimate_missing_owner_route_root_is_not_forced_to_apply() {
    let directory = Directory::new();
    let mut state = state(0);
    let queries = [query("missing-owner", 7, false, false)];
    super::super::super::super::admit_initial(
        &mut state,
        &Domain {
            phase: Phase::Route,
            owner: [true, false],
            lower: vec![7, 0],
            upper: vec![Some(7), Some(0)],
            rank: Some(100),
            powers: DomainPowerBounds::default(),
        },
    )
    .unwrap();
    let inputs = file(&directory.0, "inputs", &[row(&queries[0], Some(0))]);
    let frontiers = file(&directory.0, "frontiers", &[]);
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            1,
            |_| Some(Phase::Route)
        )
        .is_ok()
    );
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            1,
            |_| Some(Phase::Apply)
        )
        .is_err()
    );
}

#[test]
fn complete_116_required_67_auxiliary_schema_is_preserved() {
    let directory = Directory::new();
    let state = state(1);
    let queries: Vec<_> = (0..183)
        .map(|id| query(&format!("q-{id}"), 0, id >= 116, true))
        .collect();
    let rows: Vec<_> = queries.iter().map(|query| row(query, Some(0))).collect();
    let inputs = file(&directory.0, "inputs", &rows);
    let frontiers = file(&directory.0, "frontiers", &[]);
    let decoded = read_with_phase(
        &directory.0,
        &inputs,
        &frontiers,
        &queries,
        &state.store,
        1,
        |_| Some(Phase::Apply),
    )
    .unwrap();
    assert_eq!(decoded.rows, rows);
    assert_eq!(
        decoded
            .rows
            .iter()
            .filter(|row| row["role"] == "required")
            .count(),
        116
    );
    assert_eq!(
        decoded
            .rows
            .iter()
            .filter(|row| row["role"] == "auxiliary")
            .count(),
        67
    );
}

#[test]
fn request_sized_unicode_ids_are_supported_but_forged_long_fields_are_bounded() {
    let directory = Directory::new();
    let state = state(1);
    let queries = [query(&"\0🚀".repeat(1024), 0, false, false)];
    let inputs = file(&directory.0, "inputs", &[row(&queries[0], Some(0))]);
    let frontiers = file(&directory.0, "frontiers", &[]);
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            1,
            |_| Some(Phase::Apply)
        )
        .is_ok()
    );
    let queries = [query("q", 0, false, false)];
    let mut changed = row(&queries[0], Some(0));
    changed["id"] = "x".repeat(100_000).into();
    let inputs = file(&directory.0, "inputs", &[changed]);
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            1,
            |_| Some(Phase::Apply)
        )
        .is_err()
    );
    let mut inputs = file(&directory.0, "inputs", &[row(&queries[0], Some(0))]);
    inputs.blake3[0] ^= 1;
    assert!(
        read_with_phase(
            &directory.0,
            &inputs,
            &frontiers,
            &queries,
            &state.store,
            1,
            |_| Some(Phase::Apply)
        )
        .is_err()
    );
}
