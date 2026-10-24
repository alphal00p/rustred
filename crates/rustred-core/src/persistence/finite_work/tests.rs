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
        diagnostics_enabled: true,
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
        let provider = Provider {
            page: &valid,
            wrong: &wrong,
            mode,
        };
        let error = directory
            .lookup(&GeneratedWorkKey::Route(vec![2]), &provider)
            .unwrap_err();
        assert!(matches!(
            error.kind,
            GeneratedWorkErrorKind::Corrupt | GeneratedWorkErrorKind::Io
        ));
        let error = TrustedGeneratedResidentWork::hydrate(
            &directory,
            &provider,
            100_000,
            0,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .err()
        .unwrap();
        assert!(matches!(
            error.kind,
            GeneratedWorkErrorKind::Corrupt | GeneratedWorkErrorKind::Io
        ));
    }
}

#[test]
fn resident_hydration_is_complete_generation_bound_and_preserves_full_phase_owner_keys() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct Provider {
        bytes: Vec<u8>,
        visits: AtomicUsize,
    }
    impl GeneratedPageProvider for Provider {
        fn visit_validated_page(
            &self,
            expected: &GeneratedWorkPageRef,
            _: u64,
            visitor: &mut dyn FnMut(&ValidatedWorkPage),
        ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
            self.visits.fetch_add(1, Ordering::Relaxed);
            let page = ValidatedWorkPage::decode_generated(
                &self.bytes,
                expected,
                &binding(),
                10_000,
                100,
            )?;
            visitor(&page);
            Ok(GeneratedPageReadUsage {
                disk_bytes: self.bytes.len() as u64,
                cache_hit: false,
            })
        }
    }
    let keys = vec![
        GeneratedWorkKey::Route(vec![0]),
        GeneratedWorkKey::Route(vec![3]),
        GeneratedWorkKey::Apply {
            owner: vec![false],
            powers: vec![0],
        },
        GeneratedWorkKey::Apply {
            owner: vec![true],
            powers: vec![2],
        },
    ];
    let bytes = encode_metadata_frame(
        PAGE_MAGIC,
        &GeneratedWorkPageRecord {
            execution: binding(),
            records: keys.clone(),
        },
        10_000,
    )
    .unwrap();
    let reference = GeneratedWorkPageRef {
        object: GeneratedObjectRef {
            digest: *blake3::hash(&bytes).as_bytes(),
            bytes: bytes.len() as u64,
        },
        records: keys.len() as u64,
        first: keys[0].clone(),
        last: keys.last().unwrap().clone(),
    };
    let directory = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
        binding(),
        [4; 32],
        Arc::new(vec![vec![reference]]),
        limits(),
    )
    .unwrap();
    let provider = Provider {
        bytes,
        visits: AtomicUsize::new(0),
    };
    let cancel = AtomicBool::new(false);
    for grant in [0, 1] {
        assert!(
            TrustedGeneratedResidentWork::hydrate(&directory, &provider, grant, 0, &cancel)
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(provider.visits.load(Ordering::Relaxed), 0);
    let resident =
        TrustedGeneratedResidentWork::hydrate(&directory, &provider, 100_000, 4096, &cancel)
            .unwrap()
            .unwrap();
    assert_eq!(provider.visits.load(Ordering::Relaxed), 1);
    for key in &keys {
        assert!(resident.contains(key).unwrap());
    }
    assert!(
        !resident
            .contains(&GeneratedWorkKey::Route(vec![2]))
            .unwrap()
    );
    assert!(
        !resident
            .contains(&GeneratedWorkKey::Apply {
                owner: vec![true],
                powers: vec![3]
            })
            .unwrap()
    );
    assert_eq!(
        resident
            .contains(&GeneratedWorkKey::Apply {
                owner: vec![false],
                powers: vec![2]
            })
            .unwrap_err()
            .kind,
        GeneratedWorkErrorKind::Corrupt
    );
    let wrong_generation = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
        binding(),
        [5; 32],
        directory.runs.clone(),
        limits(),
    )
    .unwrap();
    assert_eq!(
        resident
            .validate_directory(&wrong_generation)
            .unwrap_err()
            .kind,
        GeneratedWorkErrorKind::Corrupt
    );
    let mut changed = binding();
    changed.program_identity = [7; 32];
    let wrong_execution = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
        changed,
        [4; 32],
        directory.runs.clone(),
        limits(),
    )
    .unwrap();
    assert_eq!(
        resident
            .validate_directory(&wrong_execution)
            .unwrap_err()
            .kind,
        GeneratedWorkErrorKind::Corrupt
    );
    let statistics = resident.statistics().unwrap();
    assert_eq!(statistics.records, keys.len());
    assert!(
        statistics.retained_payload_estimate + statistics.reserved_coexisting_bytes
            <= statistics.max_live_payload_bytes
    );
    cancel.store(true, Ordering::Release);
    assert_eq!(
        TrustedGeneratedResidentWork::hydrate(&directory, &provider, 0, 0, &cancel)
            .err()
            .unwrap()
            .kind,
        GeneratedWorkErrorKind::Cancelled
    );
    cancel.store(false, Ordering::Release);
    let mut broken = provider;
    broken.bytes[0] ^= 1;
    assert_eq!(
        TrustedGeneratedResidentWork::hydrate(&directory, &broken, 100_000, 0, &cancel)
            .err()
            .unwrap()
            .kind,
        GeneratedWorkErrorKind::Corrupt
    );
}

#[test]
fn resident_metadata_rebind_charges_both_directories_and_refreshes_retained_bytes() {
    struct NoPages;
    impl GeneratedPageProvider for NoPages {
        fn visit_validated_page(
            &self,
            _: &GeneratedWorkPageRef,
            _: u64,
            _: &mut dyn FnMut(&ValidatedWorkPage),
        ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
            panic!("empty directory has no pages");
        }
    }
    let directory = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
        binding(),
        [1; 32],
        Arc::new(Vec::new()),
        limits(),
    )
    .unwrap();
    let prepare = || {
        TrustedGeneratedResidentWork::hydrate(
            &directory,
            &NoPages,
            1200,
            400,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap()
        .unwrap()
        .prepare_metadata_only([1; 32])
        .unwrap()
    };
    // The successor alone fits, but coexistence with the predecessor does not.
    assert!(
        prepare()
            .refresh_coexisting_bytes(1300, 900)
            .unwrap()
            .is_none()
    );
    let resident = prepare()
        .refresh_coexisting_bytes(1100, 700)
        .unwrap()
        .unwrap()
        .after_durable_commit([2; 32]);
    assert_eq!(
        resident.statistics().unwrap().reserved_coexisting_bytes,
        700
    );
    assert_eq!(resident.statistics().unwrap().records, 0);
    assert_eq!(
        prepare()
            .refresh_coexisting_bytes(600, 700)
            .err()
            .unwrap()
            .kind,
        GeneratedWorkErrorKind::Corrupt
    );
}

#[test]
fn resident_hydration_rejects_cross_run_duplicates_instead_of_minting_partial_absence() {
    struct Provider(Vec<(GeneratedWorkPageRef, ValidatedWorkPage)>);
    impl GeneratedPageProvider for Provider {
        fn visit_validated_page(
            &self,
            expected: &GeneratedWorkPageRef,
            _: u64,
            visitor: &mut dyn FnMut(&ValidatedWorkPage),
        ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
            visitor(
                &self
                    .0
                    .iter()
                    .find(|(reference, _)| reference == expected)
                    .unwrap()
                    .1,
            );
            Ok(GeneratedPageReadUsage {
                disk_bytes: 0,
                cache_hit: true,
            })
        }
    }
    let provider = Provider(vec![page(&[1, 3]), page(&[2, 3])]);
    let directory = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
        binding(),
        [1; 32],
        Arc::new(
            provider
                .0
                .iter()
                .map(|(reference, _)| vec![reference.clone()])
                .collect(),
        ),
        limits(),
    )
    .unwrap();
    assert_eq!(
        TrustedGeneratedResidentWork::hydrate(
            &directory,
            &provider,
            100_000,
            0,
            &std::sync::atomic::AtomicBool::new(false)
        )
        .err()
        .unwrap()
        .kind,
        GeneratedWorkErrorKind::Corrupt
    );
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
