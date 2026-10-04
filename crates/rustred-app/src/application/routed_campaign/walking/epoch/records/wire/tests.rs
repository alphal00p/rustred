use super::*;
use crate::application::routed_campaign::walking::epoch::{
    anchors::{AnchorKind, AnchorRecord, AnchorRef, AnchorScope, Lent, Piece},
    job::{BreakReason, ErrorKind, JobResult, NativeKind, Scope},
    merge::{CheckedAnchors, CheckedResult, Class, RecordBuilder},
    records::JsonReference as Builder,
};
use crate::application::routed_campaign::walking::queue::{CompactDomain, Domain, Phase};
use rustred::solver::DomainPowerBounds;

fn image(phase: Phase) -> CompactDomain<2> {
    CompactDomain::try_from_domain(&Domain {
        phase,
        owner: [true, false],
        lower: vec![0, 3],
        upper: vec![Some(9), None],
        rank: Some(u32::MAX),
        powers: DomainPowerBounds {
            max_positive_power: Some(u64::MAX),
            min_power_difference: Some(i64::MIN),
            max_power_difference: Some(i64::MAX),
        },
    })
    .unwrap()
}
fn entry(phase: Phase, class: Class) -> CheckedResult<2> {
    CheckedResult {
        class,
        cause: None,
        recurring_panic: false,
        anchors: None,
        result: JobResult {
            seq: 2,
            parent: 4,
            v0: 1,
            kind: if phase == Phase::Route {
                NativeKind::Route
            } else {
                NativeKind::Apply
            },
            error_kind: if class == Class::C2 {
                ErrorKind::NativeFailure
            } else {
                ErrorKind::None
            },
            break_reason: BreakReason::None,
            panic: false,
            emitted: 3,
            accepted: 3,
            stats_events: 3,
            successors: 2,
            conditional: 1,
            known_reuse: 2,
            job_duplicates: 4,
            optional: if phase == Phase::Apply {
                [5, 6, 7]
            } else {
                [0; 3]
            },
            route_masks: if phase == Phase::Route { 17 } else { 0 },
            route_joint_pruned: 0,
            seconds: 1.2345678901234567,
            stats_json: br#"{"events":3,"detail":"unchanged"}"#.to_vec(),
            error: Some("native detail\nline".into()),
            frontiers: if class == Class::C4 {
                vec![br#"{"kind":"example","detail":[1,2]}"#.to_vec()]
            } else {
                vec![]
            },
            refusals: vec![br#"{"reason":"not positive"}"#.to_vec()],
            refusals_truncated: true,
            scope: None,
            g2: None,
            finite_replay: None,
            lookup: None,
            misses: vec![],
        },
    }
}
fn check(e: &CheckedResult<2>, image: &CompactDomain<2>) {
    let record = Record::native(4, image, e, 2, 3, false).unwrap();
    assert_eq!(
        record.project().unwrap(),
        Builder.native(4, image, e, 2, 3, false).unwrap()
    );
    let mut bytes = Vec::new();
    append(&record, &mut bytes).unwrap();
    assert_eq!(read(&mut bytes.as_slice()).unwrap(), record);
    let mut input = bytes.as_slice();
    let (authority, count) = super::authority(&mut input).unwrap();
    assert_eq!(authority, record.authority);
    skip_diagnostics(&mut input, count).unwrap();
    assert!(input.is_empty());
}

#[test]
fn binary_projection_matches_reference_native_alias_scope_and_full_width_fields() {
    for phase in [Phase::Apply, Phase::Route] {
        for class in [Class::C0, Class::C2, Class::C4] {
            check(&entry(phase, class), &image(phase));
        }
    }
    let image = image(Phase::Apply);
    for exhausted in [false, true] {
        let record = Record::alias(4, &image, 3, 2, exhausted);
        assert_eq!(
            record.project().unwrap(),
            Builder.alias(4, &image, 3, 2, exhausted)
        );
        let mut bytes = vec![];
        append(&record, &mut bytes).unwrap();
        assert_eq!(read(&mut bytes.as_slice()).unwrap(), record);
    }
    let mut e = entry(Phase::Apply, Class::C0);
    e.result.kind = NativeKind::ApplyPartial;
    e.result.scope = Some(Scope {
        anchor: 0,
        cut: 7,
        residual: DomainPowerBounds {
            max_power_difference: Some(6),
            ..Default::default()
        },
    });
    e.anchors = Some(CheckedAnchors {
        record: AnchorRecord {
            node: 4,
            kind: AnchorKind::InitialDBand,
            dispatch_version: 1,
            scope: AnchorScope::DBandCut(7),
            anchors: vec![AnchorRef {
                anchor: 0,
                stamp: None,
                lent: Lent::Full,
            }],
        },
        tokens: vec![],
        q_digest: 0,
    });
    check(&e, &image);
    e.result.kind = NativeKind::G2Residual;
    e.result.scope = None;
    e.anchors = Some(CheckedAnchors {
        record: AnchorRecord {
            node: 4,
            kind: AnchorKind::G2Residual,
            dispatch_version: 1,
            scope: AnchorScope::Residual(vec![Piece {
                d_lo: None,
                d_hi: Some(-1),
                lower: vec![0, 1],
                upper: vec![9, u16::MAX],
            }]),
            anchors: vec![AnchorRef {
                anchor: 0,
                stamp: Some(1),
                lent: Lent::LowSlice,
            }],
        },
        tokens: vec![],
        q_digest: 0,
    });
    check(&e, &image);
    let mut e = entry(Phase::Apply, Class::C4);
    e.result.kind = NativeKind::Abandoned;
    check(&e, &image);
    let mut e = entry(Phase::Apply, Class::C2);
    e.result.panic = true;
    e.result.stats_json.clear();
    check(&e, &image);
}

#[test]
fn frames_reject_wrong_schema_truncation_lengths_and_authority_trailing_bytes() {
    let record = Record::alias(0, &image(Phase::Apply), 1, 2, false);
    let mut good = vec![];
    append(&record, &mut good).unwrap();
    for end in 0..good.len() {
        assert!(read(&mut &good[..end]).is_err(), "truncation {end}");
    }
    let mut bad = good.clone();
    bad[3] = b'2';
    assert!(read(&mut bad.as_slice()).is_err());
    for offset in [4, 8] {
        let mut bad = good.clone();
        bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read(&mut bad.as_slice()).is_err());
    }
    let mut bad = good.clone();
    let n = u32::from_le_bytes(bad[4..8].try_into().unwrap());
    bad.insert(HEADER_BYTES + n as usize, 0);
    bad[4..8].copy_from_slice(&(n + 1).to_le_bytes());
    assert!(
        read(&mut bad.as_slice())
            .unwrap_err()
            .to_string()
            .contains("trailing")
    );
}

#[test]
fn malformed_record_never_appends_partial_frame_and_diagnostic_bytes_are_preserved() {
    let mut record = Record::native(
        4,
        &image(Phase::Apply),
        &entry(Phase::Apply, Class::C0),
        2,
        0,
        false,
    )
    .unwrap();
    let mut bytes = b"prior whole frames".to_vec();
    let before = bytes.clone();
    record.authority.image.lower.clear();
    assert!(append(&record, &mut bytes).is_err());
    assert_eq!(bytes, before);
    record.authority.image = super::super::typed::Image::of(&image(Phase::Apply));
    // Diagnostics are opaque on the binary hot path, but export refuses invalid
    // JSON rather than inventing a placeholder or dropping the original bytes.
    record.diagnostics.stats_json = b"not JSON".to_vec();
    bytes.clear();
    append(&record, &mut bytes).unwrap();
    let restored = read(&mut bytes.as_slice()).unwrap();
    assert_eq!(restored.diagnostics.stats_json, b"not JSON");
    assert!(restored.project().is_err());
}
