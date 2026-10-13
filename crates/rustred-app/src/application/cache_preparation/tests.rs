use super::*;
use crate::{
    AppErrorKind, EntryPowerBudget, FiniteEntryDomain, SavedApplicationCachePackLimits,
    SavedApplicationOptions,
};
use rustred::{
    family::IntegralKey,
    persistence::application_cache::ApplicationCacheBinding,
    solver::{CandidateRoutedCachePolicy, DomainPowerBounds, FiniteRootAdmission, RootRegionInput},
};

fn binding(arity: usize) -> ApplicationCacheBinding {
    ApplicationCacheBinding {
        artifact_digest: [13; 32],
        family_fingerprint: "preparation-test-family".into(),
        physical_arity: arity,
        layer: "guarded-unit-mass-root-v1".into(),
    }
}
fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

#[test]
fn preparation_defaults_are_decimal_and_do_not_change_evaluation() {
    let options = CachePreparationOptions::default();
    assert_eq!(
        options.cache_policy,
        CandidateRoutedCachePolicy::EvictLeastRecentlyUsed
    );
    assert_eq!(options.time_limit_seconds, 36_000);
    assert_eq!(
        SavedApplicationOptions::default().cache_policy,
        CandidateRoutedCachePolicy::EvictLeastRecentlyUsed
    );
    assert_eq!(
        CandidateRoutedCachePolicy::default(),
        CandidateRoutedCachePolicy::FailOnLimit
    );
    for (loops, disk) in [
        (1, 1_000_000_000_u64),
        (2, 1_000_000_000),
        (3, 1_000_000_000),
        (4, 16_000_000_000),
        (5, 32_000_000_000),
    ] {
        // Disk defaults remain representable even on32-bit targets. RAM is
        // explicitly tiny here; platform-dependent defaults are checked below.
        let selected = CachePreparationOptions {
            ram_bytes: Some(17),
            ..options.clone()
        }
        .resolve(loops, Default::default())
        .unwrap();
        assert_eq!(selected.disk_bytes, disk);
        assert_eq!(selected.ram_bytes, 17);
        assert_eq!(selected.storage_limits.max_disk_bytes, disk);
    }
    let lazy = CachePreparationOptions {
        ram_bytes: Some(17),
        ..options
    }
    .resolve(5, CachePreparationCapabilities::validated_lazy_storage())
    .unwrap();
    assert_eq!(lazy.disk_bytes, 128_000_000_000_u64);
    assert_eq!(lazy.to_document()["ram_is_process_rss"], false);
}

#[test]
fn preparation_ram_defaults_and_overrides_are_checked() {
    let options = CachePreparationOptions::default();
    assert_eq!(
        options.resolve(1, Default::default()).unwrap().ram_bytes,
        1_000_000_000
    );
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(
            options.resolve(4, Default::default()).unwrap().ram_bytes,
            16_000_000_000
        );
        assert_eq!(
            options
                .resolve(5, CachePreparationCapabilities::validated_lazy_storage())
                .unwrap()
                .ram_bytes,
            32_000_000_000
        );
    }
    #[cfg(target_pointer_width = "32")]
    assert_eq!(
        options.resolve(5, Default::default()).unwrap_err().kind(),
        AppErrorKind::Limit
    );
    assert_eq!(
        options.resolve(0, Default::default()).unwrap_err().kind(),
        AppErrorKind::Input
    );
    assert_eq!(
        options.resolve(6, Default::default()).unwrap_err().kind(),
        AppErrorKind::Input
    );
    let beyond_default_table = CachePreparationOptions {
        disk_bytes: Some(23),
        ..Default::default()
    }
    .resolve(6, Default::default())
    .unwrap();
    assert_eq!(beyond_default_table.disk_bytes, 23);
    assert_eq!(beyond_default_table.ram_bytes, 23);
    for invalid in [
        CachePreparationOptions {
            disk_bytes: Some(0),
            ..Default::default()
        },
        CachePreparationOptions {
            ram_bytes: Some(0),
            ..Default::default()
        },
        CachePreparationOptions {
            max_roots: 0,
            ..Default::default()
        },
        CachePreparationOptions {
            checkpoint_interval_seconds: 0,
            ..Default::default()
        },
    ] {
        assert_eq!(
            invalid.resolve(1, Default::default()).unwrap_err().kind(),
            AppErrorKind::Input
        );
    }
    let limits = SavedApplicationCachePackLimits {
        max_disk_bytes: 7_000_000_000,
        max_chunks: 17,
        ..Default::default()
    };
    let selected = CachePreparationOptions {
        ram_bytes: Some(19),
        ..Default::default()
    }
    .with_storage_limits(limits)
    .resolve(1, Default::default())
    .unwrap();
    assert_eq!(selected.disk_bytes, 7_000_000_000);
    assert_eq!(selected.retained_coefficient_bytes, 19);
    assert_eq!(selected.storage_limits.max_chunks, 17);
    assert_eq!(selected.storage_limits.chunk, limits.chunk);
}

#[test]
fn explicit_workloads_use_full_keys_and_exact_artifact_binding() {
    let left = CachePreparationWorkload::explicit(
        vec![binding(2)],
        [(0, key(&[1, 2])), (0, key(&[2, 1])), (0, key(&[1, 2]))],
        3,
    )
    .unwrap();
    let right = CachePreparationWorkload::explicit(
        vec![binding(2)],
        [(0, key(&[2, 1])), (0, key(&[1, 2])), (0, key(&[1, 2]))],
        3,
    )
    .unwrap();
    assert_eq!(left.digest(), right.digest());
    assert_eq!(left.targets().len(), 2);
    assert_eq!(left.targets()[0].source_occurrences(), 2);
    let mut other = binding(2);
    other.artifact_digest[0] ^= 1;
    let changed = CachePreparationWorkload::explicit(
        vec![other],
        [(0, key(&[1, 2])), (0, key(&[2, 1])), (0, key(&[1, 2]))],
        3,
    )
    .unwrap();
    assert_ne!(left.digest(), changed.digest());
    assert!(CachePreparationWorkload::explicit(vec![binding(2)], [(1, key(&[1, 2]))], 1).is_err());
    assert!(CachePreparationWorkload::explicit(vec![binding(2)], [(0, key(&[1]))], 1).is_err());
    assert_eq!(
        CachePreparationWorkload::explicit(
            vec![binding(2)],
            std::iter::repeat_with(|| (0, key(&[1, 2]))),
            3
        )
        .unwrap_err()
        .kind(),
        AppErrorKind::Limit
    );
}

#[test]
fn native_boundary_workload_preserves_correlated_domains_and_padding() {
    let region = RootRegionInput {
        support: [true, false, true],
        lower: vec![0, 0, 0],
        upper: vec![Some(5), Some(3), Some(5)],
        rank: Some(3),
        powers: DomainPowerBounds {
            max_positive_power: Some(7),
            min_power_difference: Some(4),
            max_power_difference: Some(4),
        },
    };
    let admitted = FiniteRootAdmission::try_new([region.clone()], 1).unwrap();
    let workload =
        CachePreparationWorkload::saved_entry_boundaries(binding(3), &[region], 3).unwrap();
    assert_eq!(
        workload
            .targets()
            .iter()
            .map(|t| t.source_occurrences())
            .sum::<u64>(),
        3
    );
    for target in workload.targets() {
        admitted.validate_entry(target.integral()).unwrap();
    }
    let padded = RootRegionInput {
        support: [true, false, false, false],
        lower: vec![0, 0, 0],
        upper: vec![Some(3), Some(0), Some(0)],
        rank: Some(0),
        powers: Default::default(),
    };
    let workload = CachePreparationWorkload::saved_entry_boundaries(binding(3), &[padded], 3);
    if rustred::fits_storage(3, 4) {
        assert!(
            workload
                .unwrap()
                .targets()
                .iter()
                .all(|target| target.integral().powers().len() == 3)
        );
    } else {
        // Padding is a native capacity-dispatch capability, not something
        // this preparation layer may enable in an exact-arity build.
        assert_eq!(workload.unwrap_err().kind(), AppErrorKind::Input);
    }
}

#[test]
fn native_finite_workload_is_exact_and_never_silently_truncated() {
    let domain = FiniteEntryDomain::new(
        vec![true, false],
        EntryPowerBudget {
            max_positive_power: 3,
            max_numerator_rank: 2,
            min_power_difference: Some(1),
            max_power_difference: None,
        },
    )
    .unwrap();
    let expected = domain
        .targets()
        .collect::<Result<std::collections::BTreeSet<_>, _>>()
        .unwrap();
    let workload = CachePreparationWorkload::finite_entry_domains(
        vec![binding(2)],
        &[(0, domain.clone())],
        expected.len(),
    )
    .unwrap();
    assert_eq!(
        workload
            .targets()
            .iter()
            .map(|target| target.integral().clone())
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    assert_eq!(
        CachePreparationWorkload::finite_entry_domains(
            vec![binding(2)],
            &[(0, domain)],
            expected.len() - 1,
        )
        .unwrap_err()
        .kind(),
        AppErrorKind::Limit
    );
}

#[test]
fn relative_primer_counts_every_pinch_with_dots_relative_to_surviving_support() {
    for (arity, expected) in [(1, 5), (3, 77), (6, 2320)] {
        let parents = [(0, vec![true; arity])];
        let workload = CachePreparationWorkload::parent_pinches(
            vec![binding(arity)],
            &parents,
            Default::default(),
            expected,
        )
        .unwrap();
        assert_eq!(workload.targets().len(), expected);
        assert!(
            workload.targets().iter().any(|target| target
                .integral()
                .powers()
                .iter()
                .all(|&n| n == 0))
        );
        for target in workload.targets() {
            let positive = target
                .integral()
                .powers()
                .iter()
                .filter(|&&n| n > 0)
                .count() as u64;
            let dots = target
                .integral()
                .powers()
                .iter()
                .filter(|&&n| n > 0)
                .map(|&n| n as u64)
                .sum::<u64>()
                - positive;
            let rank = target
                .integral()
                .powers()
                .iter()
                .filter(|&&n| n < 0)
                .map(|&n| n.unsigned_abs())
                .sum::<u64>();
            assert!(dots <= 2 && rank <= 1);
            assert_eq!(target.source_occurrences(), 1);
        }
        assert_eq!(
            CachePreparationWorkload::parent_pinches(
                vec![binding(arity)],
                &parents,
                Default::default(),
                expected - 1,
            )
            .unwrap_err()
            .kind(),
            AppErrorKind::Limit
        );
    }
    let one = CachePreparationWorkload::parent_pinches(
        vec![binding(1)],
        &[(0, vec![true])],
        Default::default(),
        5,
    )
    .unwrap();
    assert_eq!(
        one.targets()
            .iter()
            .map(|target| target.integral().clone())
            .collect::<Vec<_>>(),
        [-1, 0, 1, 2, 3].map(|n| key(&[n]))
    );
}

#[test]
fn primer_noncontiguous_physical_lines_preserve_auxiliaries_and_duplicate_budget() {
    let parent = (0, vec![false, true, false, true, false]);
    let one = CachePreparationWorkload::parent_pinches(
        vec![binding(5)],
        &[parent.clone()],
        Default::default(),
        60,
    )
    .unwrap();
    assert_eq!(one.targets().len(), 60);
    let undotted = CachePreparationWorkload::parent_pinches(
        vec![binding(5)],
        &[parent.clone()],
        CachePreparationPrimerBudget::Relative {
            max_extra_dots: 0,
            max_numerator_rank: 0,
        },
        4,
    )
    .unwrap();
    assert_eq!(undotted.targets().len(), 4);
    assert!(
        undotted.targets().iter().all(|target| target
            .integral()
            .powers()
            .iter()
            .all(|&n| n == 0 || n == 1))
    );
    for target in one.targets() {
        for axis in [0, 2, 4] {
            assert!(target.integral().powers()[axis] <= 0);
        }
    }
    assert!(
        one.targets()
            .iter()
            .any(|target| target.integral() == &key(&[-1, 1, 0, 1, 0]))
    );
    let repeated = CachePreparationWorkload::parent_pinches(
        vec![binding(5)],
        &[parent.clone(), parent.clone()],
        Default::default(),
        120,
    )
    .unwrap();
    assert_eq!(repeated.targets().len(), one.targets().len());
    assert!(
        repeated
            .targets()
            .iter()
            .all(|target| target.source_occurrences() == 2)
    );
    assert_eq!(
        CachePreparationWorkload::parent_pinches(
            vec![binding(5)],
            &[parent.clone(), parent],
            Default::default(),
            119,
        )
        .unwrap_err()
        .kind(),
        AppErrorKind::Limit
    );
}

#[test]
fn primer_absolute_budgets_match_native_domains_and_large_arity_needs_no_bitmask() {
    let budget = EntryPowerBudget {
        max_positive_power: 4,
        max_numerator_rank: 2,
        min_power_difference: Some(2),
        max_power_difference: Some(2),
    };
    let workload = CachePreparationWorkload::parent_pinches(
        vec![binding(3)],
        &[(0, vec![true, false, true])],
        CachePreparationPrimerBudget::Absolute(budget),
        1000,
    )
    .unwrap();
    let domains = [
        vec![false, false, false],
        vec![true, false, false],
        vec![false, false, true],
        vec![true, false, true],
    ]
    .map(|support| (0, FiniteEntryDomain::new(support, budget).unwrap()));
    let expected =
        CachePreparationWorkload::finite_entry_domains(vec![binding(3)], &domains, 1000).unwrap();
    assert_eq!(workload.targets(), expected.targets());
    assert_eq!(workload.digest(), expected.digest());
    let zero_budget = EntryPowerBudget {
        max_positive_power: 0,
        max_numerator_rank: 0,
        min_power_difference: None,
        max_power_difference: None,
    };
    let wide = CachePreparationWorkload::parent_pinches(
        vec![binding(64)],
        &[(0, vec![true; 64])],
        CachePreparationPrimerBudget::Absolute(zero_budget),
        1,
    )
    .unwrap();
    assert_eq!(wide.targets().len(), 1);
    assert_eq!(wide.targets()[0].integral(), &key(&[0; 64]));
    let empty = CachePreparationWorkload::parent_pinches(
        vec![binding(64)],
        &[(0, vec![true; 64])],
        CachePreparationPrimerBudget::Absolute(EntryPowerBudget {
            min_power_difference: Some(1),
            ..zero_budget
        }),
        0,
    )
    .unwrap();
    assert!(empty.targets().is_empty());
    assert_eq!(
        CachePreparationWorkload::parent_pinches(
            vec![binding(64)],
            &[(0, vec![true; 64])],
            Default::default(),
            1,
        )
        .unwrap_err()
        .kind(),
        AppErrorKind::Limit
    );
}

#[test]
fn primer_validates_all_namespaces_and_masks_before_counting_and_balances_families() {
    let bindings = vec![binding(1), binding(3)];
    let parents = [(0, vec![true]), (1, vec![true; 3])];
    let workload = CachePreparationWorkload::parent_pinches(
        bindings.clone(),
        &parents,
        Default::default(),
        82,
    )
    .unwrap();
    assert_eq!(workload.targets().len(), 82);
    assert_eq!(
        workload
            .execution_order()
            .take(4)
            .map(|target| target.namespace())
            .collect::<Vec<_>>(),
        [0, 1, 0, 1]
    );
    assert_eq!(
        CachePreparationWorkload::parent_pinches(
            bindings.clone(),
            &parents,
            Default::default(),
            81,
        )
        .unwrap_err()
        .kind(),
        AppErrorKind::Limit
    );
    // The valid first parent exceeds cap0, but malformed later declarations
    // must still be rejected as input, before any proposal counting.
    for invalid in [(2, vec![true]), (1, vec![]), (1, vec![true; 4])] {
        assert_eq!(
            CachePreparationWorkload::parent_pinches(
                bindings.clone(),
                &[(0, vec![true]), invalid],
                Default::default(),
                0,
            )
            .unwrap_err()
            .kind(),
            AppErrorKind::Input
        );
    }
    assert_eq!(
        CachePreparationWorkload::parent_pinches(
            vec![binding(1), binding(1)],
            &[],
            Default::default(),
            0,
        )
        .unwrap_err()
        .kind(),
        AppErrorKind::Input
    );
    assert_eq!(
        CachePreparationWorkload::parent_pinches(vec![], &[], Default::default(), 0,)
            .unwrap_err()
            .kind(),
        AppErrorKind::Input
    );
    assert_eq!(
        CachePreparationWorkload::parent_pinches(
            vec![binding(1)],
            &[],
            CachePreparationPrimerBudget::Absolute(EntryPowerBudget {
                max_positive_power: 1,
                max_numerator_rank: 0,
                min_power_difference: Some(2),
                max_power_difference: Some(1),
            }),
            0,
        )
        .unwrap_err()
        .kind(),
        AppErrorKind::Input
    );
    assert_eq!(
        CachePreparationWorkload::parent_pinches(
            vec![binding(1)],
            &[(0, vec![true])],
            CachePreparationPrimerBudget::Relative {
                max_extra_dots: u32::MAX,
                max_numerator_rank: 0
            },
            10,
        )
        .unwrap_err()
        .kind(),
        AppErrorKind::Limit
    );
}
