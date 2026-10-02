use super::*;

fn record() -> ProgramRecord {
    let key = IntegralRecord {
        symbolic: vec![true],
        values: vec![0],
    };
    ProgramRecord {
        schema: CANDIDATE_BUNDLE_SCHEMA.into(),
        status: STATUS.into(),
        solver_policy: SOLVER_POLICY.into(),
        family_source: "fixture".into(),
        input_format: "toml".into(),
        family_fingerprint: "fixture".into(),
        root_sector: vec![true],
        permutation: None,
        integral_order: "fixture".into(),
        sectors: vec![SectorRecord {
            sector: vec![true],
            finite_residuals: vec![],
            rules: vec![RuleRecord {
                case: CaseRecord {
                    kind: "coordinate".into(),
                    fixed_axes: vec![],
                    fixed_values: vec![],
                    equations: vec![],
                },
                target: key,
                rhs: vec![],
                sources: vec![],
                exclusions: vec![],
            }],
        }],
        rule_dispatch: Vec::new(),
    }
}

#[test]
fn empty_dispatch_has_exact_old_v2_wire_and_read_identity() {
    let records = record();
    // A tuple has the exact old sequential struct encoding, independently of
    // the new ProgramRecord and V2Ref definitions.
    let old = bincode::encode_to_vec(
        (
            &records.schema,
            &records.status,
            &records.solver_policy,
            &records.family_source,
            &records.input_format,
            &records.family_fingerprint,
            &records.root_sector,
            &records.permutation,
            &records.integral_order,
            &records.sectors,
        ),
        bincode::config::standard(),
    )
    .unwrap();
    let bytes = encode(&records).unwrap();
    assert_eq!(bytes, old);
    let (decoded, consumed) = decode_v2(&bytes).unwrap();
    assert_eq!(consumed, bytes.len());
    assert_eq!(decoded, records);
    validate(&decoded, Default::default()).unwrap();
    assert_eq!(policy(&decoded, 0, 0), RuleDispatchPolicy::Partition);
}

#[test]
fn sparse_v3_roundtrip_is_bound_and_rejects_unknown_duplicate_or_out_of_range_markers() {
    let old = record();
    let mut records = old.clone();
    records.schema = DISPATCH_CANDIDATE_BUNDLE_SCHEMA.into();
    records.rule_dispatch.push(RuleDispatchRecord {
        sector: 0,
        rule: 0,
        policy: 1,
    });
    validate(&records, Default::default()).unwrap();
    let bytes = encode(&records).unwrap();
    assert_ne!(bytes, encode(&old).unwrap());
    let (decoded, consumed): (ProgramRecord, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(consumed, bytes.len());
    assert_eq!(decoded, records);
    assert_eq!(
        policy(&decoded, 0, 0),
        RuleDispatchPolicy::AfterBaselinePartitionWholePiece
    );
    let mut invalid = records.clone();
    invalid.rule_dispatch.clear();
    assert!(validate(&invalid, Default::default()).is_err());
    invalid = records.clone();
    invalid.schema = CANDIDATE_BUNDLE_SCHEMA.into();
    assert!(validate(&invalid, Default::default()).is_err());
    for tag in [0, 2, 255] {
        invalid = records.clone();
        invalid.rule_dispatch[0].policy = tag;
        assert!(validate(&invalid, Default::default()).is_err());
    }
    for (sector, rule) in [(0, 1), (1, 0), (usize::MAX, usize::MAX)] {
        invalid = records.clone();
        invalid.rule_dispatch[0].sector = sector;
        invalid.rule_dispatch[0].rule = rule;
        assert!(validate(&invalid, Default::default()).is_err());
    }
    invalid = records.clone();
    invalid.rule_dispatch.push(invalid.rule_dispatch[0].clone());
    assert!(validate(&invalid, Default::default()).is_err());
    let limits = CandidateBundleLimits {
        max_collection_entries: 0,
        ..Default::default()
    };
    assert_eq!(
        validate(&records, limits).unwrap_err().kind(),
        crate::AppErrorKind::Limit
    );
}
