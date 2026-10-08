use super::*;

#[test]
fn envelope_borrows_sections_without_algebra_or_arity_assumptions() {
    let limits = BinaryIoLimits::default();
    let sections = [
        BinarySection {
            tag: SectionTag::SYMBOLICA_STATE,
            bytes: b"state",
        },
        BinarySection {
            tag: SectionTag::FAMILY,
            bytes: b"arbitrary family metadata",
        },
    ];
    for kind in [
        BinaryProgramKind::Candidates,
        BinaryProgramKind::Certified,
        BinaryProgramKind::TerminalValues,
        BinaryProgramKind::TerminalNormalization,
        BinaryProgramKind::BoundedCertified,
        BinaryProgramKind::DomainRules,
        BinaryProgramKind::TerminalRelations,
        BinaryProgramKind::TerminalCollection,
    ] {
        let bytes = encode_program(kind, &sections, limits).unwrap();
        let view = inspect_program(&bytes, limits).unwrap();
        assert_eq!(view.kind(), kind);
        assert_eq!(view.sections(), sections);
        let section = view.section(SectionTag::FAMILY).unwrap();
        assert!(section.as_ptr() >= bytes.as_ptr());
        assert!(section.as_ptr() < bytes.as_ptr().wrapping_add(bytes.len()));
        assert!(view.section(SectionTag::CERTIFICATE).is_none());
    }
}

#[test]
fn framing_rejects_truncation_trailing_data_duplicate_sections_and_bad_ids() {
    let limits = BinaryIoLimits::default();
    let section = BinarySection {
        tag: SectionTag::PROGRAM,
        bytes: b"payload",
    };
    assert!(encode_program(BinaryProgramKind::Candidates, &[section, section], limits).is_err());
    let bytes = encode_program(BinaryProgramKind::Candidates, &[section], limits).unwrap();
    for length in 0..bytes.len() {
        assert!(
            inspect_program(&bytes[..length], limits).is_err(),
            "length {length}"
        );
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(inspect_program(&extra, limits).is_err());
    for (offset, replacement) in [(0, b'x'), (8, 255), (12, 0), (13, 4), (14, 1), (22, 1)] {
        let mut bad = bytes.clone();
        bad[offset] = replacement;
        assert!(inspect_program(&bad, limits).is_err(), "offset {offset}");
    }
}

#[test]
fn framing_checks_lengths_and_counts_before_reserving() {
    let limits = BinaryIoLimits::default();
    let bytes = encode_program(BinaryProgramKind::Candidates, &[], limits).unwrap();
    let mut bad = bytes.clone();
    bad[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        inspect_program(&bad, limits),
        Err(BinaryIoError::Limit { .. })
    ));
    let limited = BinaryIoLimits {
        max_program_bytes: bytes.len() - 1,
        ..limits
    };
    assert!(matches!(
        inspect_program(&bytes, limited),
        Err(BinaryIoError::Limit { .. })
    ));
    let state = BinarySection {
        tag: SectionTag::SYMBOLICA_STATE,
        bytes: &[0; 4],
    };
    let limited = BinaryIoLimits {
        max_state_bytes: 3,
        ..limits
    };
    assert!(matches!(
        encode_program(BinaryProgramKind::Candidates, &[state], limited),
        Err(BinaryIoError::Limit { .. })
    ));
}

#[test]
fn portable_envelope_has_identical_fixed_width_bytes() {
    let bytes = encode_program(
        BinaryProgramKind::Candidates,
        &[BinarySection {
            tag: SectionTag::PROGRAM,
            bytes: b"x",
        }],
        BinaryIoLimits::default(),
    )
    .unwrap();
    let expected = [
        b'R', b'R', b'P', b'B', b'I', b'N', b'\r', b'\n', 1, 0, 0, 0, 1, 8, 0, 0, 1, 0, 0, 0, 4, 0,
        0, 0, 1, 0, 0, 0, 0, 0, 0, 0, b'x',
    ];
    assert_eq!(bytes, expected);
    assert_eq!(
        inspect_program(&expected, BinaryIoLimits::default())
            .unwrap()
            .section(SectionTag::PROGRAM),
        Some(b"x".as_slice())
    );
}

#[cfg(target_pointer_width = "32")]
#[test]
fn foreign_lengths_exceeding_address_space_fail_before_section_loading() {
    let mut bytes = encode_program(
        BinaryProgramKind::Candidates,
        &[BinarySection {
            tag: SectionTag::PROGRAM,
            bytes: b"x",
        }],
        BinaryIoLimits::default(),
    )
    .unwrap();
    bytes[24..32].copy_from_slice(&(u64::from(u32::MAX) + 1).to_le_bytes());
    assert!(matches!(
        inspect_program(&bytes, BinaryIoLimits::default()),
        Err(BinaryIoError::Invalid("section length exceeds host width"))
    ));
}
