use super::super::super::finite_replay::{OwnerDomainWalkFiniteReplayLimits, Recipe};
use super::*;

fn recipe() -> Recipe {
    Recipe {
        version: 2,
        limits: OwnerDomainWalkFiniteReplayLimits {
            max_nodes: 100,
            max_rule_applications: 100,
            max_transport_calls: 100,
            max_transport_operations: 100,
            max_transport_endpoints: 100,
            max_coalescing_additions: 100,
            max_positive_layers: 64,
            max_seed_points: 1024,
            max_seed_bytes: 1024 * 1024,
        },
    }
}
fn setup() -> (EpochState<2>, JobResult<2>) {
    let mut state = state_with(&[boxed([2, 0], [2, 0]), boxed([3, 0], [3, 0])]);
    let mut dispatch = Dispatch::new();
    let j = jobs(&mut state, &mut dispatch, 2);
    let mut r = result(&j[0], &[]);
    r.kind = NativeKind::FiniteReplay;
    r.finite_replay = Some(recipe());
    r.emitted = 1;
    r.accepted = 1;
    r.stats_events = 1;
    r.stats_json = br#"{"events":1,"successors":0}"#.to_vec();
    (state, r)
}

#[test]
fn finite_replay_p1_binds_policy_and_refuses_ordinary_or_partial_payload() {
    let config = MergeConfig {
        finite_replay: Some(recipe().limits),
        ..CONFIG
    };
    let (mut state, r) = setup();
    assert!(merge::p1_check(&mut state, vec![r.encode()], config).is_ok());
    let (mut state, r) = setup();
    assert!(merge::p1_check(&mut state, vec![r.encode()], CONFIG).is_err());
    for defect in 0..10 {
        let (mut state, mut r) = setup();
        match defect {
            0 => r.finite_replay.as_mut().unwrap().version = 1,
            1 => r.finite_replay.as_mut().unwrap().limits.max_nodes += 1,
            2 => {
                r.parent = 1;
                r.seq = state.in_flight[&1].seq;
            }
            3 => {
                r.scope = Some(Scope {
                    anchor: 1,
                    cut: 1,
                    residual: Default::default(),
                })
            }
            4 => r.error = Some("hidden native error".into()),
            5 => r.frontiers.push(b"{}".to_vec()),
            6 => r.known_reuse = 1,
            7 => r.optional[0] = 1,
            8 => r.emitted = 0,
            _ => r.misses.push(Miss {
                ordinal: 0,
                digest: 0,
                image: state.store.domains[1],
                target: None,
            }),
        }
        assert!(
            merge::p1_check(&mut state, vec![r.encode()], config).is_err(),
            "defect{defect}"
        );
    }
}

#[test]
fn finite_replay_typed_record_roundtrip_and_forged_authority_refusal() {
    let (state, r) = setup();
    let checked = merge::CheckedResult {
        result: r,
        class: Class::C0,
        cause: None,
        recurring_panic: false,
        anchors: None,
    };
    let record =
        records::typed::Record::native(0, &state.store.domains[0], &checked, 1, 0, false).unwrap();
    let mut bytes = Vec::new();
    records::wire::append(&record, &mut bytes).unwrap();
    assert_eq!(records::wire::read(&mut bytes.as_slice()).unwrap(), record);
    let projected = record.project().unwrap();
    assert_eq!(projected["record_kind"], "finite_replay_summary");
    assert_eq!(projected["finite_replay_recipe"]["version"], 2);
    for defect in 0..8 {
        let mut forged = record.clone();
        let records::typed::Body::Native(native) = &mut forged.authority.body else {
            unreachable!()
        };
        match defect {
            0 => forged.authority.id = 1,
            1 => forged.authority.image.upper[0] += 1,
            2 => native.scope = records::typed::Scope::Whole,
            3 => native.kind = NativeKind::Apply as u8,
            4 => native.distinct_edges = 1,
            5 => native.stats_events = 0,
            6 => native.frontiers = 1,
            _ => {
                if let records::typed::Scope::FiniteReplay(recipe) = &mut native.scope {
                    recipe.version = 1
                }
            }
        }
        assert!(forged.authority.validate().is_err(), "defect{defect}");
    }
    // Diagnostics cannot replace the typed recipe or its geometry authority.
    let mut diagnostics = record.clone();
    diagnostics.diagnostics.stats_json =
        br#"{"events":1,"finite_replay_recipe":{"version":99}}"#.to_vec();
    assert_eq!(
        diagnostics.project().unwrap()["finite_replay_recipe"]["version"],
        2
    );
}

#[test]
fn finite_replay_result_wire_requires_conditional_recipe_and_no_trailing_bytes() {
    let (_, r) = setup();
    let bytes = r.encode();
    assert_eq!(JobResult::<2>::decode(&bytes).unwrap(), r);
    // The opt-in trailer is version u32 plus nine u64 allowances. Removing it
    // must not turn kind5 into a generic no-successor native result.
    assert!(JobResult::<2>::decode(&bytes[..bytes.len() - 76]).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(JobResult::<2>::decode(&trailing).is_err());
    let mut legacy = r.clone();
    legacy.kind = NativeKind::Apply;
    legacy.finite_replay = None;
    let legacy_bytes = legacy.encode();
    assert_eq!(legacy_bytes.len() + 76, bytes.len());
    assert_eq!(JobResult::<2>::decode(&legacy_bytes).unwrap(), legacy);
    let mut extra = legacy_bytes;
    extra.extend_from_slice(&bytes[bytes.len() - 76..]);
    assert!(JobResult::<2>::decode(&extra).is_err());
}
