//! Concrete sunset validation for the generic static affine-case inventory.
//!
//! The production compiler receives only an authenticated family, context,
//! and queue.  These fixed sectors are oracles for completeness and LiteRed's
//! global integer-translation grouping; no recurrence is supplied here.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedResidualAffineCaseInventoryCertificate,
    GeneratedResidualAffineCaseInventoryCompiler, GeneratedResidualAffineCaseInventoryLimits,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpGenerator, SectorMask,
};
use symbolica::domains::integer::Integer;

fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn compile_inventory(
    sector_bits: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    GeneratedResidualAffineCaseInventoryCertificate,
) {
    let family = equal_mass_sunset(&format!("affine-case-inventory-sunset-{sector_bits}"));
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_from_bit_string(sector_bits).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let queue = Arc::new(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap(),
    );
    let inventory = GeneratedResidualAffineCaseInventoryCompiler::compile(
        &family,
        &context,
        queue,
        GeneratedResidualAffineCaseInventoryLimits::default(),
    )
    .unwrap();
    (family, context, inventory)
}

fn validate_complete_exact_inventory(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: &GeneratedResidualAffineCaseInventoryCertificate,
) {
    inventory.replay(family, context).unwrap();
    let stats = inventory.stats();
    assert_eq!(stats.terminals(), inventory.terminals().len());
    assert_eq!(stats.actionable_cases(), inventory.cases().len());
    assert_eq!(stats.groups(), inventory.groups().len());
    assert_eq!(stats.covers(), inventory.source_queue().work_items().len());
    assert_eq!(
        stats.source_empty_terminals()
            + stats.boolean_empty_terminals()
            + stats.affine_empty_terminals()
            + stats.unsupported_terminals()
            + stats.guard_contradictions()
            + stats.actionable_cases(),
        stats.terminals(),
        "every terminal must retain exactly one typed outcome"
    );

    for pair in inventory.terminals().windows(2) {
        assert!(pair[0].locator() < pair[1].locator());
        if pair[0].locator().work_item_ordinal() == pair[1].locator().work_item_ordinal() {
            assert!(Arc::ptr_eq(pair[0].source_cover(), pair[1].source_cover()));
        }
    }
    for pair in inventory.cases().windows(2) {
        assert!(pair[0].locator() < pair[1].locator());
    }

    let mut seen = vec![false; inventory.cases().len()];
    for (group_ordinal, group) in inventory.groups().iter().enumerate() {
        assert_eq!(group.ordinal(), group_ordinal);
        assert_eq!(group.anchor_case_ordinal(), group.case_ordinals()[0]);
        assert_eq!(group.case_ordinals().len(), group.anchor_offsets().len());
        let anchor = &inventory.cases()[group.anchor_case_ordinal()];
        for (ordinal_within_group, (&case_ordinal, offset)) in group
            .case_ordinals()
            .iter()
            .zip(group.anchor_offsets())
            .enumerate()
        {
            assert!(!seen[case_ordinal]);
            seen[case_ordinal] = true;
            let case = &inventory.cases()[case_ordinal];
            let terminal = inventory
                .terminals()
                .iter()
                .find(|terminal| terminal.locator() == case.locator())
                .expect("every actionable case retains its source terminal");
            assert_eq!(case.ordinal(), case_ordinal);
            assert_eq!(case.group_ordinal(), group_ordinal);
            assert_eq!(case.ordinal_within_group(), ordinal_within_group);
            assert!(Arc::ptr_eq(case.source_cover(), terminal.source_cover()));
            assert!(Arc::ptr_eq(
                case.source_branch(),
                terminal
                    .source_branch()
                    .expect("actionable terminal retains its affine branch")
            ));
            assert!(Arc::ptr_eq(
                case.guard_composition(),
                terminal
                    .guard_composition()
                    .expect("actionable terminal retains its guard composition")
            ));
            assert_eq!(case.affine_map().ambient_arity(), group.ambient_arity());
            assert_eq!(case.affine_map().free_positions(), group.free_positions());
            assert_eq!(offset.len(), group.ambient_arity());
            for row in 0..group.ambient_arity() {
                assert_eq!(
                    offset[row],
                    case.constants()[row].clone() - &anchor.constants()[row]
                );
                for (free_parameter_ordinal, &column) in group.free_positions().iter().enumerate() {
                    assert_eq!(
                        group
                            .compact_linear_coefficient(row, free_parameter_ordinal)
                            .unwrap(),
                        case.affine_map().linear_coefficient(row, column).unwrap()
                    );
                }
            }
        }
    }
    assert!(seen.into_iter().all(|seen| seen));
}

fn assert_group_merges_across_an_intervening_actionable_group(
    inventory: &GeneratedResidualAffineCaseInventoryCertificate,
    group: &rustred::GeneratedResidualAffineContiguousCaseGroup,
) {
    assert!(
        group
            .case_ordinals()
            .windows(2)
            .any(
                |pair| (pair[0] + 1..pair[1]).any(|intervening_case_ordinal| inventory.cases()
                    [intervening_case_ordinal]
                    .group_ordinal()
                    != group.ordinal())
            ),
        "the natural group must merge across an intervening actionable case from another group; an adjacent-actionable-run implementation is insufficient"
    );
}

#[test]
fn sunset_011_has_a_genuine_global_integer_translation_group() {
    let (family, context, inventory) = compile_inventory("011");
    validate_complete_exact_inventory(&family, &context, &inventory);
    let group = inventory
        .groups()
        .iter()
        .find(|group| group.case_ordinals().len() >= 2 && group.free_positions() == [0])
        .expect("sunset sector 011 must expose its natural multi-case affine class");
    assert_eq!(
        group.compact_linear_coefficients(),
        [Integer::from(1), Integer::from(0), Integer::from(0)]
    );
    assert_group_merges_across_an_intervening_actionable_group(&inventory, group);
    let anchor = &inventory.cases()[group.anchor_case_ordinal()];
    assert!(
        group.case_ordinals()[1..]
            .iter()
            .any(|&ordinal| inventory.cases()[ordinal].constants() != anchor.constants())
    );
}

#[test]
fn sunset_101_has_a_genuine_global_integer_translation_group() {
    let (family, context, inventory) = compile_inventory("101");
    validate_complete_exact_inventory(&family, &context, &inventory);
    let group = inventory
        .groups()
        .iter()
        .find(|group| group.case_ordinals().len() >= 2 && group.free_positions() == [1])
        .expect("sunset sector 101 must expose its natural multi-case affine class");
    assert_eq!(
        group.compact_linear_coefficients(),
        [Integer::from(0), Integer::from(1), Integer::from(0)]
    );
    assert_group_merges_across_an_intervening_actionable_group(&inventory, group);
    let anchor = &inventory.cases()[group.anchor_case_ordinal()];
    assert!(
        group.case_ordinals()[1..]
            .iter()
            .any(|&ordinal| inventory.cases()[ordinal].constants() != anchor.constants())
    );
}
