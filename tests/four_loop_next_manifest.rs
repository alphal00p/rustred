#![cfg(feature = "legacy-authored-oracles")]

use std::collections::BTreeSet;

use rustred::four_loop_next_manifest::{
    FOUR_LOOP_NEXT_MANIFEST_RAW_TERM_INCIDENCE_BOUND, FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM,
    FourLoopNextRawRowIdError,
};
use rustred::{
    FOUR_LOOP_NEXT_MANIFEST_CORNER_SEEDS, FOUR_LOOP_NEXT_MANIFEST_DOT_SEEDS,
    FOUR_LOOP_NEXT_MANIFEST_MIXED_SEEDS, FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES,
    FOUR_LOOP_NEXT_MANIFEST_NUMERATOR_SEEDS, FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS,
    FOUR_LOOP_NEXT_MANIFEST_SEEDS, FourLoopGenuineCornerType, FourLoopNextManifest,
    FourLoopNextManifestConfig, FourLoopNextManifestError, FourLoopNextManifestStatus,
    FourLoopNextRawRowId, FourLoopNextSeedPhase, FourLoopTopology,
};

#[test]
fn exact_123_seed_origin_manifest_is_topology_authenticated_and_replayable() {
    let cap_failures: [(&str, fn(&mut FourLoopNextManifestConfig)); 4] = [
        ("selected seeds", |config| {
            config.max_seeds = FOUR_LOOP_NEXT_MANIFEST_SEEDS - 1
        }),
        ("native raw rows", |config| {
            config.max_raw_rows = FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS - 1
        }),
        ("nonzero seed entries", |config| {
            config.max_nonzero_seed_entries = FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES - 1
        }),
        ("raw term incidences", |config| {
            config.max_raw_term_incidences = FOUR_LOOP_NEXT_MANIFEST_RAW_TERM_INCIDENCE_BOUND - 1
        }),
    ];
    for (resource, configure) in cap_failures {
        let mut insufficient = FourLoopNextManifestConfig::default();
        configure(&mut insufficient);
        assert!(matches!(
            FourLoopNextManifest::build(insufficient),
            Err(FourLoopNextManifestError::ResourceLimit {
                resource: actual,
                ..
            }) if actual == resource
        ));
    }

    let manifest = FourLoopNextManifest::build(FourLoopNextManifestConfig::default()).unwrap();
    assert_eq!(
        manifest.status(),
        FourLoopNextManifestStatus::ExactOriginsNormalizationPending
    );
    assert_eq!(manifest.seeds().len(), FOUR_LOOP_NEXT_MANIFEST_SEEDS);
    assert_eq!(
        manifest.raw_row_ids().len(),
        FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS
    );

    // The checksum is independently recomputed here from the public stable
    // serialization: every ordered stable key followed by exactly one LF.
    let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
    for seed in manifest.seeds() {
        for byte in seed.stable_key().bytes().chain([b'\n']) {
            checksum ^= u64::from(byte);
            checksum = checksum.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    assert_eq!(FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM, 0x0bff_80d5_dddb_4340);
    assert_eq!(checksum, 0x0bff_80d5_dddb_4340);
    assert_eq!(manifest.seed_checksum(), checksum);

    let phase_ranges = [
        (FourLoopNextSeedPhase::Corner, 0, 10),
        (FourLoopNextSeedPhase::Dot, 10, 82),
        (FourLoopNextSeedPhase::Numerator, 82, 110),
        (FourLoopNextSeedPhase::Mixed, 110, 123),
    ];
    assert_eq!(
        phase_ranges.map(|(_, start, end)| end - start),
        [
            FOUR_LOOP_NEXT_MANIFEST_CORNER_SEEDS,
            FOUR_LOOP_NEXT_MANIFEST_DOT_SEEDS,
            FOUR_LOOP_NEXT_MANIFEST_NUMERATOR_SEEDS,
            FOUR_LOOP_NEXT_MANIFEST_MIXED_SEEDS,
        ]
    );
    for (phase, start, end) in phase_ranges {
        for (phase_index, seed) in manifest.seeds()[start..end].iter().enumerate() {
            assert_eq!(seed.phase(), phase);
            assert_eq!(usize::from(seed.phase_index()), phase_index);
        }
    }

    let reference_catalog = [
        (
            FourLoopGenuineCornerType::FiveLine,
            0x06b,
            FourLoopTopology::H,
        ),
        (
            FourLoopGenuineCornerType::SixLineA,
            0x06f,
            FourLoopTopology::H,
        ),
        (
            FourLoopGenuineCornerType::SixLineB,
            0x0cf,
            FourLoopTopology::H,
        ),
        (
            FourLoopGenuineCornerType::SevenLineA,
            0x13f,
            FourLoopTopology::H,
        ),
        (
            FourLoopGenuineCornerType::SevenLineB,
            0x07f,
            FourLoopTopology::H,
        ),
        (
            FourLoopGenuineCornerType::SevenLineC,
            0x0df,
            FourLoopTopology::H,
        ),
        (
            FourLoopGenuineCornerType::EightLineA,
            0x17f,
            FourLoopTopology::H,
        ),
        (
            FourLoopGenuineCornerType::EightLineB,
            0x0ff,
            FourLoopTopology::H,
        ),
        (
            FourLoopGenuineCornerType::HNineLine,
            0x1ff,
            FourLoopTopology::H,
        ),
        (
            FourLoopGenuineCornerType::XNineLine,
            0x1ff,
            FourLoopTopology::X,
        ),
    ];
    assert_eq!(
        manifest.seeds()[..FOUR_LOOP_NEXT_MANIFEST_CORNER_SEEDS]
            .iter()
            .map(|seed| seed.corner_type())
            .collect::<Vec<_>>(),
        reference_catalog
            .iter()
            .map(|(corner, _, _)| *corner)
            .collect::<Vec<_>>()
    );
    for seed in manifest.seeds() {
        let (_, reference_mask, reference_topology) = reference_catalog
            .iter()
            .copied()
            .find(|(corner, _, _)| *corner == seed.corner_type())
            .unwrap();
        assert_eq!(seed.corner_type().reference_mask(), reference_mask);
        assert_eq!(seed.topology(), reference_topology);

        let positive_mask = seed
            .powers()
            .iter()
            .enumerate()
            .fold(0_u16, |mask, (position, power)| {
                mask | (u16::from(*power > 0) << position)
            });
        assert_eq!(positive_mask, reference_mask);

        let mut dots = 0;
        let mut numerators = 0;
        for (position, power) in seed.powers().iter().copied().enumerate() {
            if reference_mask & (1_u16 << position) != 0 {
                assert!(power == 1 || power == 2);
                dots += usize::from(power == 2);
            } else {
                assert!(power == 0 || power == -1);
                numerators += usize::from(power == -1);
            }
        }
        assert_eq!(
            (dots, numerators),
            match seed.phase() {
                FourLoopNextSeedPhase::Corner => (0, 0),
                FourLoopNextSeedPhase::Dot => (1, 0),
                FourLoopNextSeedPhase::Numerator => (0, 1),
                FourLoopNextSeedPhase::Mixed => (1, 1),
            }
        );
    }
    assert_eq!(
        manifest
            .seeds()
            .iter()
            .flat_map(|seed| seed.powers())
            .filter(|&&power| power != 0)
            .count(),
        FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES
    );

    let expected_rows = manifest
        .seeds()
        .iter()
        .copied()
        .flat_map(|seed| {
            (0_u8..4).flat_map(move |differentiated_loop| {
                (0_u8..4).map(move |contraction_loop| {
                    FourLoopNextRawRowId::new(seed, differentiated_loop, contraction_loop).unwrap()
                })
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(manifest.raw_row_ids(), expected_rows);
    assert_eq!(
        manifest
            .raw_row_ids()
            .iter()
            .map(|row| row.stable_key())
            .collect::<BTreeSet<_>>()
            .len(),
        FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS
    );
    assert!(
        manifest
            .raw_row_ids()
            .iter()
            .all(|row| { row.differentiated_loop() < 4 && row.contraction_loop() < 4 })
    );
    let first_seed = manifest.seeds()[0];
    assert!(FourLoopNextRawRowId::new(first_seed, 3, 3).is_ok());
    assert!(matches!(
        FourLoopNextRawRowId::new(first_seed, 4, 0),
        Err(FourLoopNextRawRowIdError::DifferentiatedLoopOutOfRange { actual: 4 })
    ));
    assert!(matches!(
        FourLoopNextRawRowId::new(first_seed, 0, 4),
        Err(FourLoopNextRawRowIdError::ContractionLoopOutOfRange { actual: 4 })
    ));

    // Freeze the selected mixed prefix independently of the implementation's
    // compact (corner,dot,numerator) descriptor table.
    let expected_mixed = [
        [1, 1, 1, 1, 1, 1, 2, 0, 1, -1],
        [1, 1, 1, 1, 1, 1, 2, -1, 1, 0],
        [1, 1, 1, 1, 1, 1, 1, -1, 2, 0],
        [1, 1, 1, 1, 2, 0, 1, 1, 0, -1],
        [1, 1, 1, 1, 2, -1, 1, 1, 0, 0],
        [1, 1, 1, 1, 1, 0, 2, 1, 0, -1],
        [1, 2, 1, 1, 1, 1, 1, -1, 0, 0],
        [1, 1, 1, 1, 1, 2, 1, -1, 0, 0],
        [1, 1, 1, 1, 1, 1, 2, 0, 0, -1],
        [1, 1, 1, 1, 1, 1, 2, -1, 0, 0],
        [1, 1, 1, 1, 1, 2, 0, -1, 1, 0],
        [1, 1, 1, 1, 1, 1, 0, -1, 2, 0],
        [1, 1, 1, 1, 1, 1, -1, 0, 2, 0],
    ];
    let actual_mixed = manifest
        .seeds()
        .iter()
        .filter(|seed| seed.phase() == FourLoopNextSeedPhase::Mixed)
        .map(|seed| *seed.powers())
        .collect::<Vec<_>>();
    assert_eq!(actual_mixed, expected_mixed);

    assert_eq!(
        manifest
            .seeds()
            .iter()
            .filter(|seed| seed.topology() == FourLoopTopology::X)
            .count(),
        11
    );
    assert!(manifest.native_collected_terms() > 0);
    assert!(manifest.native_collected_terms() <= FOUR_LOOP_NEXT_MANIFEST_RAW_TERM_INCIDENCE_BOUND);
    manifest.replay().unwrap();
}
