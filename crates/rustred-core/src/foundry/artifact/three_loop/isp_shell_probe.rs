//! Non-authoritative inactive-ISP shell probe for the `K = 6` pressure family.
//!
//! This test-only module asks a deliberately narrow question: do complete
//! triangular translates of all nine ordinary K4-family IBPs contain exact
//! endpoint identities for the factorized path and star sectors?  It owns no
//! rule cell, terminal, artifact, cover, or production dispatch.  A passing
//! endpoint replay therefore supplies discovery evidence only; it cannot
//! establish a parametric recurrence or close any part of the Stage 1 cover.

use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::foundry::anchored::{
    AnchoredRule, AnchoredRuleLimits, derive_strictly_descending_rule_for_target,
};
use crate::foundry::cell::{FixedIndexRestriction, RuleCellLimits, SourceViewBatch};
use crate::foundry::completion::frame::PhysicalFrameLimits;
use crate::foundry::completion::source_discovery::{
    CampaignLimits, try_build_triangular_support_frame,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator, RowId,
    TranslatedSourceLimits,
};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain};

use super::{canonical_family, canonical_s4, exact_zero_sectors};

const ARITY: usize = 6;
const ORDINARY_SOURCE_COUNT: usize = 9;
const MAX_INACTIVE_DEGREE: usize = 3;
const CUMULATIVE_OFFSET_COUNTS: [usize; MAX_INACTIVE_DEGREE + 1] = [1, 4, 10, 20];
const EXACT_SHELL_OFFSET_COUNTS: [usize; MAX_INACTIVE_DEGREE + 1] = [1, 3, 6, 10];

const PATH_ROOT: [i64; ARITY] = [0, 0, 1, 0, 1, 1];
const PATH_AXES: [usize; 3] = [0, 1, 3];
const STAR_ROOT: [i64; ARITY] = [0, 0, 1, 1, 0, 1];
const STAR_AXES: [usize; 3] = [0, 1, 4];

#[derive(Clone, Copy, Debug)]
struct ProbeCase {
    degree: usize,
    inactive_powers: [usize; 3],
    /// Coefficients of `1`, `d`, and `d^2` in the normalized numerator.
    numerator: [i64; 3],
    denominator_power: usize,
}

const fn probe(
    degree: usize,
    inactive_powers: [usize; 3],
    numerator: [i64; 3],
    denominator_power: usize,
) -> ProbeCase {
    ProbeCase {
        degree,
        inactive_powers,
        numerator,
        denominator_power,
    }
}

const PATH_CASES: [ProbeCase; 12] = [
    probe(1, [1, 0, 0], [2, 0, 0], 0),
    probe(1, [0, 0, 1], [1, 0, 0], 0),
    probe(2, [2, 0, 0], [12, 4, 0], 1),
    probe(2, [1, 0, 1], [4, 2, 0], 1),
    probe(2, [0, 0, 2], [4, 1, 0], 1),
    probe(2, [0, 1, 1], [1, 0, 0], 0),
    probe(3, [3, 0, 0], [48, 72, 8], 2),
    probe(3, [2, 0, 1], [16, 28, 4], 2),
    probe(3, [1, 0, 2], [16, 2, 0], 1),
    // The MATAD negative-power route currently misreports this endpoint as zero.
    probe(3, [1, 1, 1], [8, 8, 2], 2),
    probe(3, [0, 0, 3], [12, 1, 0], 1),
    probe(3, [0, 1, 2], [4, 1, 0], 1),
];

const STAR_CASES: [ProbeCase; 6] = [
    probe(1, [0, 0, 1], [1, 0, 0], 0),
    probe(2, [0, 0, 2], [4, 1, 0], 1),
    probe(2, [0, 1, 1], [1, 0, 0], 0),
    probe(3, [0, 0, 3], [12, 1, 0], 1),
    probe(3, [0, 1, 2], [4, 1, 0], 1),
    // Independent angular averaging gives `(d^2 - 8) / d^2`, not zero.
    probe(3, [1, 1, 1], [-8, 0, 1], 2),
];

const PATH_ORBIT_COUNTS: [usize; MAX_INACTIVE_DEGREE + 1] = [0, 2, 4, 6];
const STAR_ORBIT_COUNTS: [usize; MAX_INACTIVE_DEGREE + 1] = [0, 1, 2, 3];

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    assert_eq!(prepared.len(), ORDINARY_SOURCE_COUNT);
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn campaign_limits(degree: usize) -> CampaignLimits {
    let offset_count = CUMULATIVE_OFFSET_COUNTS[degree];
    let source_count = ORDINARY_SOURCE_COUNT * offset_count;
    let translated_sources = TranslatedSourceLimits {
        max_requested_offsets: offset_count,
        max_requested_source_translations: source_count,
        max_translated_sources: source_count,
        max_translated_term_entries: source_count * 12,
        max_translated_condition_entries: source_count * 12,
        max_retained_condition_source_entries: source_count * 24,
        max_retained_index_coordinate_cells: source_count * 128,
        ..TranslatedSourceLimits::default()
    };
    let physical_frame = PhysicalFrameLimits {
        translated_sources,
        max_arity: ARITY,
        max_degree: degree,
        max_offsets: offset_count,
        max_offset_coordinate_cells: ARITY * offset_count,
        max_source_instances: source_count,
        max_physical_columns: source_count * 3,
        max_physical_column_coordinate_cells: source_count * 3 * ARITY,
        max_physical_entries: source_count * 12,
        max_csr_row_offsets: source_count + 1,
    };
    CampaignLimits {
        max_request_arity: ARITY,
        max_submitted_requests: source_count,
        max_canonical_candidate_requests: source_count,
        max_accumulated_requests: source_count,
        max_request_coordinate_cells: ARITY * source_count,
        max_merge_comparisons: 2_048,
        max_retained_probe_coordinates: ARITY,
        translated_sources,
        physical_frame,
        ..CampaignLimits::default()
    }
}

fn anchored_limits(degree: usize) -> AnchoredRuleLimits {
    let source_count = ORDINARY_SOURCE_COUNT * CUMULATIVE_OFFSET_COUNTS[degree];
    AnchoredRuleLimits {
        max_source_rows: source_count,
        max_integral_columns: 512,
        max_augmented_columns: 1_024,
        max_input_nonzero_entries: 2_000,
        max_integral_key_power_cells: 32_000,
        max_guard_provenance_index_cells: 32_000,
        max_ordering_key_coordinate_cells: 64_000,
        max_native_decomposition_nonzero_entries: 32_000,
        max_back_substitution_output_nonzero_entries: 80_000,
        max_back_substitution_live_nonzero_entries: 240_000,
        max_rule_guards: 1_024,
        max_guard_origins: 4_096,
        max_guard_provenance_sources: 4_096,
        max_elimination_pivots: 16_384,
        max_source_combination_terms: source_count,
        max_replay_exact_operations: 2_000_000,
        ..AnchoredRuleLimits::default()
    }
}

fn shell_degree(offset: &[i64], axes: &[usize; 3]) -> usize {
    axes.iter()
        .map(|&axis| usize::try_from(-offset[axis]).unwrap())
        .sum()
}

fn target(root: [i64; ARITY], axes: [usize; 3], powers: [usize; 3]) -> [i64; ARITY] {
    let mut target = root;
    for (&axis, power) in axes.iter().zip(powers) {
        target[axis] -= i64::try_from(power).unwrap();
    }
    target
}

fn projection_limits(degree: usize, zero_sector_count: usize) -> RuleCellLimits {
    let source_count = ORDINARY_SOURCE_COUNT * CUMULATIVE_OFFSET_COUNTS[degree];
    RuleCellLimits {
        max_source_views: source_count,
        max_fixed_restrictions: ARITY,
        max_pruned_terms: source_count * 12,
        max_retained_terms: source_count * 12,
        max_guards: source_count * 12,
        max_projected_source_terms: source_count * 12,
        max_projection_group_routes: source_count * 48,
        max_projection_zero_sectors: zero_sector_count,
        ..RuleCellLimits::default()
    }
}

fn projected_sources_from_frame(
    generator: &ParametricIbpGenerator<'_>,
    frame: &crate::foundry::completion::frame::SelectedSourceFrame,
    completed: &CompletedIbpSourceRows,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
    root: [i64; ARITY],
    degree: usize,
) -> SourceViewBatch {
    let frame_requests = frame
        .plan()
        .source_instances()
        .iter()
        .map(|source| {
            (
                source.provenance().source_ordinal(),
                source.provenance().offset().clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let offsets = frame_requests
        .iter()
        .map(|(_, offset)| offset.clone())
        .collect::<BTreeSet<IntegralShift>>();
    assert_eq!(offsets.len(), CUMULATIVE_OFFSET_COUNTS[degree]);

    // Uniform ceilings over all nine ordinary sources make this support a
    // true Cartesian source-by-offset set.  Retranslation from the producer's
    // retained offsets is therefore exact, not a wider surrogate selection;
    // the set equality below keeps that premise executable.
    let translated = generator
        .translate_completed_source_rows(
            completed,
            offsets,
            campaign_limits(degree).translated_sources,
        )
        .unwrap();
    let retranslated_requests = translated
        .sources()
        .iter()
        .map(|source| {
            assert_eq!(
                source.provenance().source_row(),
                completed
                    .source_row_id(source.provenance().source_ordinal())
                    .unwrap()
            );
            (
                source.provenance().source_ordinal(),
                source.provenance().offset().clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(retranslated_requests, frame_requests);
    let domain = SectorInteriorDomain::try_new(
        Mask::try_from_indices(&root).unwrap(),
        root.map(|power| InteriorBounds::new(power, power)),
    )
    .unwrap();
    let fixed = root
        .into_iter()
        .enumerate()
        .map(|(position, value)| FixedIndexRestriction::new(position, value));
    let limits = projection_limits(degree, zero_sectors.len());
    let projected = SourceViewBatch::try_project_complete_residual(
        translated,
        generator.context(),
        domain,
        fixed,
        canonicalizer,
        zero_sectors,
        limits,
    )
    .unwrap();
    assert_eq!(
        projected.len(),
        ORDINARY_SOURCE_COUNT * CUMULATIVE_OFFSET_COUNTS[degree]
    );
    assert!(
        projected
            .verify_residual_projection(generator.context(), canonicalizer, zero_sectors, limits)
            .unwrap()
    );
    projected
}

fn derive_target(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
    root: [i64; ARITY],
    target: [i64; ARITY],
) -> AnchoredRule {
    let degree = root
        .iter()
        .zip(target)
        .map(|(&root, target)| usize::try_from(root - target).unwrap())
        .sum();
    derive_strictly_descending_rule_for_target(
        generator.context(),
        sources.relations(),
        &root,
        &target,
        OrderingPolicy::default(),
        anchored_limits(degree),
    )
    .unwrap()
}

fn stabilizer_representatives(
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    root: [i64; ARITY],
    axes: [usize; 3],
    degree: usize,
) -> BTreeMap<[i64; ARITY], Vec<[usize; 3]>> {
    let sector = Mask::try_from_indices(&root).unwrap();
    let mut classes = BTreeMap::<[i64; ARITY], Vec<[usize; 3]>>::new();
    for first in 0..=degree {
        for second in 0..=degree - first {
            let powers = [first, second, degree - first - second];
            let key = IntegralKey::try_new(target(root, axes, powers)).unwrap();
            let representative = stabilizer_representative(canonicalizer, &sector, &key);
            classes.entry(representative).or_default().push(powers);
        }
    }
    classes
}

fn stabilizer_representative(
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    sector: &Mask,
    key: &IntegralKey,
) -> [i64; ARITY] {
    canonicalizer
        .orbit(key)
        .unwrap()
        .images()
        .iter()
        .filter(|image| Mask::try_from_indices(image.integral().powers()).unwrap() == *sector)
        .map(|image| <[i64; ARITY]>::try_from(image.integral().powers()).unwrap())
        .min()
        .unwrap()
}

fn assert_frame_shape_and_provenance(
    frame: &crate::foundry::completion::frame::SelectedSourceFrame,
    completed: &CompletedIbpSourceRows,
    axes: [usize; 3],
    degree: usize,
) {
    assert_eq!(frame.completed_source_row_count(), ORDINARY_SOURCE_COUNT);
    assert_eq!(
        frame.plan().row_count(),
        ORDINARY_SOURCE_COUNT * CUMULATIVE_OFFSET_COUNTS[degree]
    );
    let mut shell_rows = [0usize; MAX_INACTIVE_DEGREE + 1];
    let mut requests = BTreeSet::new();
    let mut stable_provenance = Vec::new();
    for row in 0..frame.plan().row_count() {
        let instance = &frame.plan().source_instances()[row];
        let provenance = instance.provenance();
        let offset = provenance.offset().values();
        assert_eq!(offset.len(), ARITY);
        assert!(
            offset
                .iter()
                .enumerate()
                .all(|(position, value)| axes.contains(&position) || *value == 0)
        );
        assert!(axes.iter().all(|&axis| offset[axis] <= 0));
        let shell = shell_degree(offset, &axes);
        assert!(shell <= degree);
        shell_rows[shell] += 1;
        assert!(requests.insert((provenance.source_ordinal(), offset.to_vec(),)));
        assert_eq!(
            provenance.source_row(),
            completed
                .source_row_id(provenance.source_ordinal())
                .unwrap()
        );
        assert_eq!(
            frame.plan().source_for_row(row).unwrap().provenance(),
            provenance
        );
        stable_provenance.push(provenance.stable_string());
    }
    assert_eq!(
        shell_rows,
        std::array::from_fn(|shell| {
            if shell <= degree {
                EXACT_SHELL_OFFSET_COUNTS[shell] * ORDINARY_SOURCE_COUNT
            } else {
                0
            }
        })
    );
    assert!(stable_provenance.windows(2).all(|pair| pair[0] != pair[1]));
}

fn expected_coefficient(generator: &ParametricIbpGenerator<'_>, case: ProbeCase) -> Coefficient {
    let base = generator.context().base();
    let d = base.parameter("d").unwrap();
    let d_squared = base.try_mul(&d, &d, Default::default()).unwrap();
    let mut numerator = base.integer(case.numerator[0]);
    for (coefficient, monomial) in [
        (case.numerator[1], d.clone()),
        (case.numerator[2], d_squared.clone()),
    ] {
        if coefficient != 0 {
            let term = base
                .try_mul(&base.integer(coefficient), &monomial, Default::default())
                .unwrap();
            numerator = base.try_add(&numerator, &term, Default::default()).unwrap();
        }
    }
    let denominator = match case.denominator_power {
        0 => base.one(),
        1 => d,
        2 => d_squared,
        power => panic!("unsupported pinned denominator power {power}"),
    };
    base.try_div(&numerator, &denominator, Default::default())
        .unwrap()
}

fn assert_single_root_term(
    generator: &ParametricIbpGenerator<'_>,
    rule: &AnchoredRule,
    root: [i64; ARITY],
    case: ProbeCase,
) {
    assert_eq!(rule.right_hand_side().len(), 1, "{rule:#?}");
    let term = &rule.right_hand_side()[0];
    assert_eq!(term.integral().powers(), root);
    assert!(term.descent().verify());
    let expected = expected_coefficient(generator, case);
    let difference = generator
        .context()
        .base()
        .try_sub(term.coefficient(), &expected, Default::default())
        .unwrap();
    assert!(
        difference.is_zero(),
        "target {:?}: expected {expected}, got {}",
        rule.pivot().powers(),
        term.coefficient()
    );
    assert!(!term.coefficient().is_zero());
    assert!(!rule.source_combination().is_empty());
    assert!(rule.source_combination().iter().all(|source| {
        matches!(source.row_id(), RowId::OrdinaryIbp { .. })
            && source.source_ordinal()
                < ORDINARY_SOURCE_COUNT * CUMULATIVE_OFFSET_COUNTS[case.degree]
    }));
    assert!(rule.replay().source_rows_used() > 0);
    assert!(rule.replay().integral_columns_checked() > 0);
    assert!(rule.replay().exact_operations() > 0);
}

#[test]
fn inactive_triangular_shells_replay_path_and_star_orbits_through_degree_three() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let completed = complete_ordinary(&generator);
    let families: [(
        [i64; ARITY],
        [usize; 3],
        &[ProbeCase],
        [usize; MAX_INACTIVE_DEGREE + 1],
    ); 2] = [
        (PATH_ROOT, PATH_AXES, &PATH_CASES, PATH_ORBIT_COUNTS),
        (STAR_ROOT, STAR_AXES, &STAR_CASES, STAR_ORBIT_COUNTS),
    ];

    for (root, axes, cases, orbit_counts) in families {
        let sector = Mask::try_from_indices(&root).unwrap();
        for degree in 1..=MAX_INACTIVE_DEGREE {
            let ceilings = [degree; ORDINARY_SOURCE_COUNT];
            let frame = try_build_triangular_support_frame(
                &generator,
                &completed,
                sector.clone(),
                &axes,
                &ceilings,
                campaign_limits(degree),
            )
            .unwrap();
            let permuted_axes = [axes[2], axes[0], axes[1]];
            let independently_enumerated = try_build_triangular_support_frame(
                &generator,
                &completed,
                sector.clone(),
                &permuted_axes,
                &ceilings,
                campaign_limits(degree),
            )
            .unwrap();
            assert_eq!(frame, independently_enumerated);
            assert_frame_shape_and_provenance(&frame, &completed, axes, degree);
            let sources = projected_sources_from_frame(
                &generator,
                &frame,
                &completed,
                &canonicalizer,
                &zero_sectors,
                root,
                degree,
            );

            let classes = stabilizer_representatives(&canonicalizer, root, axes, degree);
            assert_eq!(classes.len(), orbit_counts[degree]);
            let mut covered_classes = BTreeSet::new();
            let level_cases = cases.iter().filter(|case| case.degree == degree);
            for &case in level_cases {
                assert_eq!(case.inactive_powers.iter().sum::<usize>(), degree);
                let selected_target = target(root, axes, case.inactive_powers);
                let selected_key = IntegralKey::try_new(selected_target).unwrap();
                let class = stabilizer_representative(&canonicalizer, &sector, &selected_key);
                assert!(classes.contains_key(&class));
                assert!(covered_classes.insert(class));

                let rule = derive_target(&generator, &sources, root, selected_target);
                assert_eq!(rule.pivot().powers(), selected_target);
                assert_single_root_term(&generator, &rule, root, case);
            }
            assert_eq!(
                covered_classes,
                classes.keys().copied().collect::<BTreeSet<_>>()
            );
        }
    }
}
