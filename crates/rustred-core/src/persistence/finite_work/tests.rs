use super::*;
use std::sync::Arc;
fn binding() -> GeneratedExecutionBinding {
    GeneratedExecutionBinding {
        physical_arity: 1,
        program_identity: [2; 32],
    }
}
fn limits() -> GeneratedDirectoryLimits {
    GeneratedDirectoryLimits {
        max_arity: 16,
        max_runs: 16,
        max_pages: 32,
        max_directory_payload_bytes: 100_000,
        max_page_records: 100,
        max_page_bytes: 10_000,
        max_referenced_bytes: 100_000,
        max_lookup_pages: 32,
        max_lookup_bytes: 100_000,
    }
}
fn page(values: &[i64]) -> (GeneratedWorkPageRef, ValidatedWorkPage) {
    let p = GeneratedWorkPageRecord {
        execution: binding(),
        records: values
            .iter()
            .map(|&n| GeneratedWorkKey::Route(vec![n]))
            .collect(),
    };
    let bytes = encode_metadata_frame(PAGE_MAGIC, &p, 10_000).unwrap();
    let descriptor = GeneratedWorkPageRef {
        object: GeneratedObjectRef {
            digest: *blake3::hash(&bytes).as_bytes(),
            bytes: bytes.len() as u64,
        },
        records: values.len() as u64,
        first: p.records.first().unwrap().clone(),
        last: p.records.last().unwrap().clone(),
    };
    let decoded =
        ValidatedWorkPage::decode_generated(&bytes, &descriptor, &binding(), 10_000, 100).unwrap();
    (descriptor, decoded)
}
#[test]
fn generated_page_provider_omitted_double_substituted_and_post_visit_errors_fail_closed() {
    enum Mode {
        Omit,
        Double,
        Wrong,
        After,
    }
    struct Provider<'a> {
        page: &'a ValidatedWorkPage,
        wrong: &'a ValidatedWorkPage,
        mode: Mode,
    }
    impl GeneratedPageProvider for Provider<'_> {
        fn visit_validated_page(
            &self,
            _: &GeneratedWorkPageRef,
            _: u64,
            v: &mut dyn FnMut(&ValidatedWorkPage),
        ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
            match self.mode {
                Mode::Omit => {}
                Mode::Double => {
                    v(self.page);
                    v(self.page)
                }
                Mode::Wrong => v(self.wrong),
                Mode::After => {
                    v(self.page);
                    return Err(GeneratedWorkError::new(
                        GeneratedWorkErrorKind::Io,
                        "after visit",
                    ));
                }
            }
            Ok(GeneratedPageReadUsage {
                disk_bytes: 0,
                cache_hit: true,
            })
        }
    }
    let (reference, valid) = page(&[1, 3]);
    let (_, wrong) = page(&[2, 4]);
    let directory = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
        binding(),
        [1; 32],
        Arc::new(vec![vec![reference]]),
        limits(),
    )
    .unwrap();
    for mode in [Mode::Omit, Mode::Double, Mode::Wrong, Mode::After] {
        let error = directory
            .lookup(
                &GeneratedWorkKey::Route(vec![2]),
                &Provider {
                    page: &valid,
                    wrong: &wrong,
                    mode,
                },
            )
            .unwrap_err();
        assert!(matches!(
            error.kind,
            GeneratedWorkErrorKind::Corrupt | GeneratedWorkErrorKind::Io
        ));
    }
}
#[test]
fn generated_page_frame_preserves_legacy_layout_and_full_signed_key_domain() {
    // Original app Work was exactly these two variants, in this field order.
    #[derive(bincode::Encode)]
    enum LegacyKey {
        Route(Vec<i64>),
        Apply { owner: Vec<bool>, powers: Vec<i64> },
    }
    #[derive(bincode::Encode)]
    struct LegacyPage {
        execution: GeneratedExecutionBinding,
        records: Vec<LegacyKey>,
    }
    let old = LegacyPage {
        execution: binding(),
        records: vec![
            LegacyKey::Route(vec![i64::MIN]),
            LegacyKey::Apply {
                owner: vec![true],
                powers: vec![2],
            },
        ],
    };
    let new = GeneratedWorkPageRecord {
        execution: binding(),
        records: vec![
            GeneratedWorkKey::Route(vec![i64::MIN]),
            GeneratedWorkKey::Apply {
                owner: vec![true],
                powers: vec![2],
            },
        ],
    };
    let bytes = encode_metadata_frame(PAGE_MAGIC, &old, 10_000).unwrap();
    assert_eq!(
        bytes,
        encode_metadata_frame(PAGE_MAGIC, &new, 10_000).unwrap()
    );
    let native = GeneratedWorkKey::Route(vec![i64::MIN])
        .to_native::<1>()
        .unwrap();
    assert_eq!(
        GeneratedWorkKey::from_native(&native).unwrap(),
        new.records[0]
    );
}
