//! Exercise a real complete artifact with an exact affine owner partition.
use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::ClosedArtifact;
use crate::foundry::artifact::install::{ClosingArtifactCandidate, install_source_port};
use crate::foundry::cell::SourceViewBatch;
use crate::foundry::parametric::AffineApplicationDomain;
use crate::identity::{ParametricIbpGenerator, TranslatedSourceRequest};
use crate::reduction::Reducer;
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

#[test]
fn affine_partition_cold_roundtrip_routes_both_sides_and_preserves_reduction() {
    let original = super::super::tests::generated::<3>(
        crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap(),
    );
    let original_bytes = original.encode_durable().unwrap();
    let position = original
        .rule_cells
        .iter()
        .position(|cell| {
            cell.rule().sector().active_bits() == [true; 3]
                && cell.fixed_restrictions().is_empty()
                && cell
                    .rule()
                    .replay_evidence()
                    .combined_original_domain()
                    .unwrap()
                    .application_boxes()[0]
                    .free_dimension()
                    == 3
        })
        .expect("generated sunset must have an unrestricted positive-sector owner");
    let cell = &original.rule_cells[position];
    let context = &original.context;
    let piece = &cell
        .rule()
        .replay_evidence()
        .combined_original_domain()
        .unwrap()
        .application_boxes()[0];
    let copy_piece = || {
        crate::foundry::completion::LatticeBox::try_new(
            piece.lower().to_vec(),
            piece.upper().to_vec(),
        )
        .unwrap()
    };
    let first = context.base().variables().len();
    let equation = context
        .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap()
        .raw()
        .numerator
        .clone();
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &[equation],
        &[first, first + 1, first + 2],
        &[true; 3],
    )
    .unwrap() else {
        panic!("equal indices must remain a coupled affine case")
    };
    let equality = Arc::new(AffineApplicationDomain::from_case(&case, &[true; 3]).unwrap());

    // Regenerate the very same original translated rows. Neither a synthetic
    // replay seal nor a cloned private source arena authorizes these children.
    let generator = ParametricIbpGenerator::try_new(&original.family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let generated = (0..batch.len()).map(|i| batch.generate(i)).collect();
    let completed = batch.complete(generated).unwrap();
    let mut requests: Vec<_> = cell
        .rule()
        .source_combination()
        .iter()
        .map(|contribution| {
            let provenance =
                cell.sources().provenance()[contribution.source_ordinal()].translated();
            (
                TranslatedSourceRequest::new(
                    provenance.source_ordinal(),
                    provenance.offset().clone(),
                ),
                contribution.coefficient().clone(),
            )
        })
        .collect();
    requests.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let selected = generator
        .translate_selected_completed_source_rows(
            &completed,
            requests.iter().map(|(request, _)| request.clone()),
            Default::default(),
        )
        .unwrap();
    assert!(
        selected
            .requests()
            .iter()
            .eq(requests.iter().map(|(request, _)| request))
    );
    let ordinals: Vec<_> = (0..selected.len()).collect();
    let sources = Arc::new(
        SourceViewBatch::try_select(
            selected.into_translated_batch(),
            &ordinals,
            Default::default(),
        )
        .unwrap(),
    );
    let weights: Vec<_> = sources
        .relations()
        .iter()
        .zip(requests)
        .enumerate()
        .map(|(ordinal, (relation, (_, weight)))| (ordinal, relation.row_id().clone(), weight))
        .collect();
    let conditions: Vec<_> = cell
        .rule()
        .nonzero_guards()
        .iter()
        .map(|guard| guard.polynomial().clone())
        .collect();
    let rhs: Vec<_> = cell
        .rule()
        .right_hand_side()
        .iter()
        .map(|term| (term.shift().clone(), term.coefficient().clone()))
        .collect();
    let zeros: Vec<_> = original
        .zero_sectors
        .iter()
        .map(|zero| zero.sector().active_bits())
        .collect();
    let parent = |target, exclusions| {
        super::super::PreparedOriginalDomain::try_new(
            context,
            Arc::clone(&sources),
            weights.clone(),
            Vec::new(),
            target,
            exclusions,
            conditions.clone(),
            Default::default(),
        )
        .unwrap()
    };
    // A cold cell may not obtain vacuous replay/descent authority from a
    // rectangle lying entirely on the excluded equality. Generation skips
    // such sign cells; decoding must reject them rather than seal them.
    let off_parent = parent(None, Arc::from([Arc::clone(&equality)]));
    let local = piece.lower()[0].max(piece.lower()[1]);
    let excluded_piece = || {
        crate::foundry::completion::LatticeBox::try_new(
            [local, local, piece.lower()[2]],
            [Some(local), Some(local), piece.upper()[2]],
        )
        .unwrap()
    };
    assert!(
        off_parent
            .application_is_proved_empty(&excluded_piece(), &[true; 3])
            .unwrap()
    );
    assert!(
        off_parent
            .coefficient_vanishes(context, &context.one(), &excluded_piece(), &[true; 3])
            .unwrap()
    );
    assert!(
        off_parent
            .application_is_proved_empty(&excluded_piece(), &[false; 3])
            .is_err()
    );
    let vacuous = off_parent
        .verify_cell(
            context,
            original.ordering,
            &[true; 3],
            &zeros,
            excluded_piece(),
            rhs.clone(),
        )
        .unwrap_err();
    assert!(vacuous.to_string().contains("entirely excluded or empty"));
    assert!(
        !off_parent
            .application_is_proved_empty(&copy_piece(), &[true; 3])
            .unwrap()
    );
    let off = parent(None, Arc::from([Arc::clone(&equality)]))
        .verify_cell(
            context,
            original.ordering,
            &[true; 3],
            &zeros,
            copy_piece(),
            rhs.clone(),
        )
        .unwrap();
    let on = parent(Some(equality), Arc::from([]))
        .verify_cell(
            context,
            original.ordering,
            &[true; 3],
            &zeros,
            copy_piece(),
            rhs,
        )
        .unwrap();
    let base = i64::try_from(*piece.lower().iter().max().unwrap()).unwrap() + 3;
    let targets = [
        IntegralKey::try_new([base, base, base]).unwrap(),
        IntegralKey::try_new([base + 1, base, base]).unwrap(),
    ];
    assert!(on.assignment_for_target(&targets[0]).unwrap().is_some());
    assert!(off.assignment_for_target(&targets[0]).unwrap().is_none());
    assert!(on.assignment_for_target(&targets[1]).unwrap().is_none());
    assert!(off.assignment_for_target(&targets[1]).unwrap().is_some());
    let mut cells = original.rule_cells.clone();
    cells.splice(position..=position, [off, on]);
    drop(generator);
    let candidate = ClosingArtifactCandidate {
        schema: original.schema,
        algorithm_id: original.algorithm_id,
        arity: original.arity,
        ordering: original.ordering,
        supported_root_power_bounds: original.supported_root_power_bounds,
        family: original.family,
        context: original.context,
        source_relations: original.source_relations,
        rules: original.rules,
        rule_cells: cells,
        canonicalizer: original.canonicalizer,
        dependencies: original.dependencies,
        factorization_rules: original.factorization_rules,
        masters: original.masters,
        zero_sectors: original.zero_sectors,
        common_mass_homogeneity: original.common_mass_homogeneity,
    };
    let partitioned = install_source_port(candidate).unwrap();
    let bytes = partitioned.encode_durable().unwrap();
    let cold = ClosedArtifact::decode_durable(&bytes).unwrap();
    assert_eq!(bytes, cold.encode_durable().unwrap());
    assert_eq!(cold.rule_cells.len(), partitioned.rule_cells.len());
    for (ordinal, target) in targets.iter().enumerate() {
        assert_eq!(
            cold.rule_cells[position]
                .assignment_for_target(target)
                .unwrap()
                .is_some(),
            ordinal == 1
        );
        assert_eq!(
            cold.rule_cells[position + 1]
                .assignment_for_target(target)
                .unwrap()
                .is_some(),
            ordinal == 0
        );
    }
    let reference = ClosedArtifact::decode_durable(&original_bytes).unwrap();
    let mut expected = Reducer::new(&reference).unwrap();
    let mut actual = Reducer::new(&cold).unwrap();
    for target in targets {
        assert_eq!(
            actual.reduce_unit_mass(&target).unwrap().terms(),
            expected.reduce_unit_mass(&target).unwrap().terms()
        );
    }
}
