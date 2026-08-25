#![cfg(feature = "legacy-authored-oracles")]

use std::collections::BTreeMap;
use std::mem::size_of;

use rustred::{
    FOUR_LOOP_NEXT_INVENTORY_CACHED_UNIT_PATH_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_CLASSIFICATION_CACHE_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_COEFFICIENT_ADDITION_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_COEFFICIENT_MULTIPLICATION_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_FULL_POWER_KEY_BOUND, FOUR_LOOP_NEXT_INVENTORY_MAPPER_CACHE_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_PATH_BOUND, FOUR_LOOP_NEXT_INVENTORY_RAW_ROWS,
    FOUR_LOOP_NEXT_INVENTORY_RECURSION_DEPTH, FOUR_LOOP_NEXT_INVENTORY_RETAINED_DYNAMIC_BYTE_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_ROW_PATH_BOUND, FOUR_LOOP_NEXT_INVENTORY_UNIT_CACHE_ENTRY_BOUND,
    FourLoopNextCompactPath, FourLoopNextInventory, FourLoopNextInventoryConfig,
    FourLoopNextInventoryError, FourLoopNextInventoryStatus, FourLoopNextLeaf,
};

#[test]
fn frozen_resource_envelopes_fail_before_inventory_work() {
    let defaults = FourLoopNextInventoryConfig::default();
    let mut failures: Vec<(&str, Box<dyn Fn(&mut FourLoopNextInventoryConfig)>)> = vec![
        (
            "raw inventory rows",
            Box::new(|c| c.max_raw_rows = FOUR_LOOP_NEXT_INVENTORY_RAW_ROWS - 1),
        ),
        (
            "retained paths",
            Box::new(|c| c.max_paths = FOUR_LOOP_NEXT_INVENTORY_PATH_BOUND - 1),
        ),
        (
            "retained compact path bytes",
            Box::new(|c| c.max_path_bytes = defaults.max_path_bytes - 1),
        ),
        (
            "interned leaves",
            Box::new(|c| c.max_leaves = FOUR_LOOP_NEXT_INVENTORY_FULL_POWER_KEY_BOUND - 1),
        ),
        (
            "leaf shallow bytes",
            Box::new(|c| c.max_leaf_shallow_bytes = defaults.max_leaf_shallow_bytes - 1),
        ),
        (
            "collected boundary contributors",
            Box::new(|c| c.max_boundary_contributors = defaults.max_boundary_contributors - 1),
        ),
        (
            "boundary contributor bytes",
            Box::new(|c| {
                c.max_boundary_contributor_bytes = defaults.max_boundary_contributor_bytes - 1
            }),
        ),
        (
            "raw boundary occurrences",
            Box::new(|c| c.max_boundary_occurrences = defaults.max_boundary_occurrences - 1),
        ),
        (
            "boundary occurrence bytes",
            Box::new(|c| {
                c.max_boundary_occurrence_bytes = defaults.max_boundary_occurrence_bytes - 1
            }),
        ),
        (
            "cached unit expansion entries",
            Box::new(|c| {
                c.max_unit_cache_entries = FOUR_LOOP_NEXT_INVENTORY_UNIT_CACHE_ENTRY_BOUND - 1
            }),
        ),
        (
            "cached unit paths",
            Box::new(|c| {
                c.max_cached_unit_paths = FOUR_LOOP_NEXT_INVENTORY_CACHED_UNIT_PATH_BOUND - 1
            }),
        ),
        (
            "cached unit path shallow bytes",
            Box::new(|c| {
                c.max_cached_unit_path_shallow_bytes =
                    defaults.max_cached_unit_path_shallow_bytes - 1
            }),
        ),
        (
            "dynamic polynomial mappers",
            Box::new(|c| c.max_cached_mappers = FOUR_LOOP_NEXT_INVENTORY_MAPPER_CACHE_BOUND - 1),
        ),
        (
            "scalar classification cache entries",
            Box::new(|c| {
                c.max_classification_cache_entries =
                    FOUR_LOOP_NEXT_INVENTORY_CLASSIFICATION_CACHE_BOUND - 1
            }),
        ),
        (
            "coefficient multiplications",
            Box::new(|c| {
                c.max_coefficient_multiplications =
                    FOUR_LOOP_NEXT_INVENTORY_COEFFICIENT_MULTIPLICATION_BOUND - 1
            }),
        ),
        (
            "coefficient additions",
            Box::new(|c| {
                c.max_coefficient_additions =
                    FOUR_LOOP_NEXT_INVENTORY_COEFFICIENT_ADDITION_BOUND - 1
            }),
        ),
        (
            "coefficient operand/result dense universe",
            Box::new(|c| {
                c.max_coefficient_operation_terms = defaults.max_coefficient_operation_terms - 1
            }),
        ),
        (
            "retained coefficient terms",
            Box::new(|c| {
                c.max_retained_coefficient_terms = defaults.max_retained_coefficient_terms - 1
            }),
        ),
        (
            "retained coefficient serialized bytes",
            Box::new(|c| {
                c.max_retained_coefficient_serialized_bytes =
                    defaults.max_retained_coefficient_serialized_bytes - 1
            }),
        ),
        (
            "peak charged bytes",
            Box::new(|c| {
                c.max_retained_dynamic_bytes =
                    FOUR_LOOP_NEXT_INVENTORY_RETAINED_DYNAMIC_BYTE_BOUND - 1
            }),
        ),
    ];
    for (resource, configure) in failures.drain(..) {
        let mut config = defaults;
        configure(&mut config);
        assert!(matches!(
            FourLoopNextInventory::build(config),
            Err(FourLoopNextInventoryError::ResourceLimit { resource: actual, .. })
                if actual == resource
        ));
    }

    let mut depth = defaults;
    depth.max_recursion_depth = FOUR_LOOP_NEXT_INVENTORY_RECURSION_DEPTH - 1;
    assert!(matches!(
        FourLoopNextInventory::build(depth),
        Err(FourLoopNextInventoryError::RecursionDepth { .. })
    ));
}

#[test]
fn exact_full_preclosure_inventory_is_replayable_and_projects_boundaries() {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default()).unwrap();
    let stats = inventory.stats();
    eprintln!("four-loop next inventory stats: {stats:#?}");
    assert_eq!(
        inventory.status(),
        FourLoopNextInventoryStatus::ExactPreclosureInventory
    );
    assert_eq!(inventory.rows().len(), FOUR_LOOP_NEXT_INVENTORY_RAW_ROWS);
    assert_eq!(
        inventory.stats().raw_rows(),
        FOUR_LOOP_NEXT_INVENTORY_RAW_ROWS
    );
    assert_eq!(stats.paths(), 26_078);
    assert_eq!(stats.leaves(), 2_794);
    assert_eq!(stats.raw_boundary_occurrences(), 4_230);
    assert_eq!(stats.collected_boundary_contributors(), 4_214);
    assert_eq!(stats.blocked_rows(), 1_289);
    assert_eq!(stats.unit_cache_entries(), 2_945);
    assert_eq!(stats.unit_cache_hits(), 8_296);
    assert_eq!(stats.cached_unit_paths(), 4_019);
    assert_eq!(stats.dynamic_mappers(), 40);
    assert_eq!(stats.classification_cache_entries(), 68);
    assert_eq!(stats.coefficient_multiplications(), 8_753);
    assert_eq!(stats.coefficient_additions(), 28);
    assert_eq!(stats.retained_coefficient_terms(), 11_106);
    assert_eq!(stats.retained_coefficient_serialized_bytes(), 8_452);
    assert_eq!(stats.peak_charged_bytes(), 1_070_904);
    assert!(inventory.stats().paths() <= FOUR_LOOP_NEXT_INVENTORY_PATH_BOUND);
    assert!(inventory.stats().leaves() <= FOUR_LOOP_NEXT_INVENTORY_FULL_POWER_KEY_BOUND);
    assert!(
        inventory
            .rows()
            .iter()
            .all(|row| row.paths().len() <= FOUR_LOOP_NEXT_INVENTORY_ROW_PATH_BOUND)
    );
    assert_eq!(size_of::<FourLoopNextCompactPath>(), 8);

    let mut occurrence_cursor = inventory.boundary_occurrences().iter();
    let mut independent_raw_counts = BTreeMap::<u32, usize>::new();
    let mut independent_nonzero_rows = BTreeMap::<u32, usize>::new();
    let mut duplicate_row_targets = 0_usize;
    let mut canceled_row_targets = 0_usize;
    for (row_index, row) in inventory.rows().iter().enumerate() {
        let mut raw_groups = BTreeMap::<u32, Vec<u32>>::new();
        for (path_index, path) in row.paths().iter().copied().enumerate() {
            if !matches!(
                inventory.leaves()[path.leaf_id() as usize],
                FourLoopNextLeaf::Boundary(_)
            ) {
                continue;
            }
            let occurrence = occurrence_cursor.next().expect("missing raw occurrence");
            assert_eq!(usize::from(occurrence.row_index()), row_index);
            assert_eq!(occurrence.path_index() as usize, path_index);
            raw_groups
                .entry(path.leaf_id())
                .or_default()
                .push(path_index as u32);
            *independent_raw_counts.entry(path.leaf_id()).or_default() += 1;
        }
        duplicate_row_targets += raw_groups
            .values()
            .filter(|contributors| contributors.len() > 1)
            .count();
        for blocker in row.collected_boundaries() {
            assert!(!blocker.coefficient().is_zero());
            let contributors = raw_groups
                .remove(&blocker.leaf_id())
                .expect("a collected blocker must have raw contributors");
            assert_eq!(blocker.contributor_path_indices(), contributors);
            *independent_nonzero_rows
                .entry(blocker.leaf_id())
                .or_default() += 1;
        }
        // Whole-inventory replay below independently rebuilds exact
        // coefficients.  A raw group omitted by the nonzero projection is
        // therefore an exact row-local cancellation, not a dropped target.
        canceled_row_targets += raw_groups.len();
        assert_eq!(row.is_blocked(), !row.collected_boundaries().is_empty());
    }
    assert!(occurrence_cursor.next().is_none());
    assert_eq!(
        inventory.boundary_target_summaries().len(),
        independent_raw_counts.len()
    );
    for summary in inventory.boundary_target_summaries() {
        assert_eq!(
            independent_raw_counts[&summary.leaf_id()],
            summary.raw_contributor_paths()
        );
        assert_eq!(
            independent_nonzero_rows
                .get(&summary.leaf_id())
                .copied()
                .unwrap_or(0),
            summary.nonzero_rows()
        );
        assert!(inventory.boundary_key(summary.leaf_id()).is_ok());
    }

    let mut root_same_n2 = None;
    let mut boundary_sample = None;
    let mut recursive_samples = [None; 3];
    let mut recursive_counts = [0_usize; 3];
    for (row_index, row) in inventory.rows().iter().enumerate() {
        for (path_index, path) in row.paths().iter().copied().enumerate() {
            let leaf = &inventory.leaves()[path.leaf_id() as usize];
            let depth = usize::from(path.recursive_depth());
            recursive_counts[depth] += 1;
            recursive_samples[depth].get_or_insert((row_index, path_index));
            if root_same_n2.is_none()
                && path.recursive_depth() == 0
                && matches!(leaf, FourLoopNextLeaf::Genuine(column)
                    if column.powers().iter().filter(|&&power| power < 0)
                        .map(|power| power.unsigned_abs()).sum::<u32>() == 2)
            {
                root_same_n2 = Some((row_index, path_index));
            }
            if boundary_sample.is_none() && matches!(leaf, FourLoopNextLeaf::Boundary(_)) {
                boundary_sample = Some((row_index, path_index));
            }
        }
    }
    eprintln!(
        "four-loop next census: depth_paths={recursive_counts:?}, raw_occurrences={}, \
         boundary_targets={}, duplicate_row_targets={duplicate_row_targets}, \
         canceled_row_targets={canceled_row_targets}",
        inventory.boundary_occurrences().len(),
        inventory.boundary_target_summaries().len(),
    );
    assert_eq!(recursive_counts.iter().sum::<usize>(), stats.paths());
    assert_eq!(recursive_counts, [14_766, 10_313, 999]);
    assert_eq!(inventory.boundary_target_summaries().len(), 1_066);
    assert_eq!(duplicate_row_targets, 28);
    assert_eq!(canceled_row_targets, 8);
    for (depth, sample) in recursive_samples.into_iter().enumerate() {
        let (row_index, path_index) = sample.expect("every recursive depth must occur");
        let replayed = inventory.replay_path(row_index, path_index).unwrap();
        assert_eq!(
            replayed.leaf(),
            &inventory.leaves()[inventory.rows()[row_index].paths()[path_index].leaf_id() as usize]
        );
        assert_eq!(
            usize::from(inventory.rows()[row_index].paths()[path_index].recursive_depth()),
            depth
        );
    }
    let (n2_row, n2_path) = root_same_n2.expect("a root-same N2 path must occur");
    assert_eq!(
        inventory.replay_path(n2_row, n2_path).unwrap().leaf(),
        &inventory.leaves()[inventory.rows()[n2_row].paths()[n2_path].leaf_id() as usize]
    );
    let (boundary_row, boundary_path) =
        boundary_sample.expect("a factorized boundary path must occur");
    assert!(matches!(
        inventory
            .replay_path(boundary_row, boundary_path)
            .unwrap()
            .leaf(),
        FourLoopNextLeaf::Boundary(_)
    ));

    let (tamper_row_index, tamper_path) = inventory
        .rows()
        .iter()
        .enumerate()
        .find_map(|(row_index, row)| row.paths().first().copied().map(|path| (row_index, path)))
        .unwrap();
    let corruptions = [
        tamper_path.with_reserved_bit_for_replay(),
        tamper_path.with_leaf_id_for_replay((inventory.leaves().len() + 1) as u32),
        tamper_path.with_raw_term_index_for_replay(127),
        tamper_path.with_root_branch_for_replay(127),
        tamper_path.with_recursive_depth_for_replay(3),
        tamper_path.with_recursive_branch_for_replay(1, 1),
    ];
    for corrupted in corruptions {
        assert!(
            inventory
                .replay_compact_path(tamper_row_index, corrupted)
                .is_err()
        );
    }
    for wanted_depth in [1_u8, 2] {
        let (row_index, path_index) = recursive_samples[usize::from(wanted_depth)]
            .expect("depth-one and depth-two paths are mandatory");
        let path = inventory.rows()[row_index].paths()[path_index];
        let used_slot = usize::from(wanted_depth - 1);
        let mutated = path.with_recursive_branch_for_replay(used_slot, 127);
        assert!(inventory.replay_compact_path(row_index, mutated).is_err());
    }
    assert!(matches!(
        inventory.replay_path(inventory.rows().len(), 0),
        Err(FourLoopNextInventoryError::RowIndexOutOfRange { .. })
    ));
    inventory.replay().unwrap();
}
