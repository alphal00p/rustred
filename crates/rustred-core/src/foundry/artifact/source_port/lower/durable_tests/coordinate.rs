//! A coordinate piece consumes exactly the same full-domain replay seal as
//! an affine piece; singleton coordinates need not be original case fixes.
use std::sync::Arc;

use crate::algebra::IndexedPolynomial;
use crate::family::IntegralKey;
use crate::foundry::artifact::ClosedArtifact;
use crate::foundry::artifact::install::{ClosingArtifactCandidate, install_source_port};
use crate::foundry::cell::{RuleCell, RuleCellGuardDomainProof, SourceViewBatch};
use crate::foundry::completion::LatticeBox;
use crate::foundry::parametric::ParametricGuardOrigin;
use crate::identity::{ParametricIbpGenerator, TranslatedSourceRequest};
use crate::reduction::Reducer;

use super::super::PreparedOriginalDomain;

fn regenerate_parent(
    artifact: &ClosedArtifact,
    cell: &RuleCell,
    extra: Option<IndexedPolynomial>,
) -> PreparedOriginalDomain {
    let generator = ParametricIbpGenerator::try_new(&artifact.family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len()).map(|i| batch.generate(i)).collect();
    let completed = batch.complete(rows).unwrap();
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
    let weights = sources
        .relations()
        .iter()
        .zip(requests)
        .enumerate()
        .map(|(ordinal, (row, (_, weight)))| (ordinal, row.row_id().clone(), weight))
        .collect();
    let conditions = cell
        .rule()
        .nonzero_guards()
        .iter()
        .map(|guard| guard.polynomial().clone())
        .chain(extra)
        .collect();
    PreparedOriginalDomain::try_new(
        &artifact.context,
        sources,
        weights,
        Vec::new(),
        None,
        Arc::from([]),
        conditions,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn coordinate_singleton_guard_seal_roundtrips_and_rejects_genuine_zeros() {
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
                    .upper()
                    == [None; 3]
        })
        .expect("sunset has an unrestricted positive-sector owner");
    let cell = &original.rule_cells[position];
    let context = &original.context;
    let piece = &cell
        .rule()
        .replay_evidence()
        .combined_original_domain()
        .unwrap()
        .application_boxes()[0];
    let a_value = i64::try_from(piece.lower()[0]).unwrap() + 2;
    let b_value = i64::try_from(piece.lower()[1]).unwrap() + 2;
    let a = context
        .sub(&context.index(0).unwrap(), &context.integer(a_value))
        .unwrap();
    let b = context
        .sub(&context.index(1).unwrap(), &context.integer(b_value))
        .unwrap();
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let coupled = context
        .add(
            &context.mul(&context.sub(&a, &b).unwrap(), &d).unwrap(),
            &context
                .add(&a, &context.mul(&context.integer(2), &b).unwrap())
                .unwrap(),
        )
        .unwrap();
    let guard = |constant| {
        context
            .numerator_condition_with_limits(
                &context.add(&coupled, &context.integer(constant)).unwrap(),
                Default::default(),
            )
            .unwrap()
    };
    // On b=B, Q=(a-A)d+a-A+3 has no generic-d zero. Before
    // specialization its two coefficients are genuinely coupled polynomials.
    let safe = guard(3);
    let local_b = u64::try_from(b_value - 1).unwrap();
    let slice = |lower_b, upper_b| {
        let mut lower = piece.lower().to_vec();
        let mut upper = piece.upper().to_vec();
        lower[1] = lower_b;
        upper[1] = upper_b;
        LatticeBox::try_new(lower, upper).unwrap()
    };
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
    let safe_parent = regenerate_parent(&original, cell, Some(safe.clone()));
    let middle = safe_parent
        .verify_cell(
            context,
            original.ordering,
            &[true; 3],
            &zeros,
            slice(local_b, Some(local_b)),
            rhs.clone(),
        )
        .unwrap();
    assert!(middle.fixed_restrictions().is_empty());
    assert_eq!(
        middle.guard_domain_proof(),
        RuleCellGuardDomainProof::ReplayAuthorizedOriginalPredicate
    );
    let retained_guard = middle
        .rule()
        .nonzero_guards()
        .iter()
        .find(|guard| guard.polynomial() == &safe)
        .expect("the original multivariate guard must remain in the payload");
    assert!(retained_guard.origins().iter().any(|origin| matches!(
        origin,
        ParametricGuardOrigin::OriginalDomainCondition { .. }
    )));
    let evidence = middle
        .rule()
        .replay_evidence()
        .combined_original_domain()
        .unwrap();
    assert!(evidence.affine_application_domain().is_none());
    assert!(evidence.affine_exclusions().is_empty());
    assert_eq!(
        evidence.application_boxes()[0],
        slice(local_b, Some(local_b))
    );

    // The wider original piece contains the true zero a=A-1,b=B-1.
    // A proof on its singleton subset cannot authorize that enlargement.
    let widened = safe_parent
        .verify_cell(
            context,
            original.ordering,
            &[true; 3],
            &zeros,
            LatticeBox::try_new(piece.lower().to_vec(), piece.upper().to_vec()).unwrap(),
            rhs.clone(),
        )
        .unwrap_err();
    assert!(widened.to_string().contains("guard zero locus"));
    // With constant zero, a=A,b=B is already a true zero on the singleton.
    let genuine_zero = regenerate_parent(&original, cell, Some(guard(0)))
        .verify_cell(
            context,
            original.ordering,
            &[true; 3],
            &zeros,
            slice(local_b, Some(local_b)),
            rhs.clone(),
        )
        .unwrap_err();
    assert!(genuine_zero.to_string().contains("guard zero locus"));

    let parent = regenerate_parent(&original, cell, None);
    let below = parent
        .verify_cell(
            context,
            original.ordering,
            &[true; 3],
            &zeros,
            slice(piece.lower()[1], Some(local_b - 1)),
            rhs.clone(),
        )
        .unwrap();
    let above = parent
        .verify_cell(
            context,
            original.ordering,
            &[true; 3],
            &zeros,
            slice(local_b + 1, None),
            rhs,
        )
        .unwrap();
    let c_value = i64::try_from(piece.lower()[2]).unwrap() + 2;
    let targets = [b_value - 1, b_value, b_value + 1]
        .map(|b| IntegralKey::try_new([a_value, b, c_value]).unwrap());
    let mut cells = original.rule_cells.clone();
    cells.splice(position..=position, [below, middle, above]);
    let partitioned = install_source_port(ClosingArtifactCandidate {
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
    })
    .unwrap();
    let bytes = partitioned.encode_durable().unwrap();
    let cold = ClosedArtifact::decode_durable(&bytes).unwrap();
    assert_eq!(bytes, cold.encode_durable().unwrap());
    for (offset, target) in targets.iter().enumerate() {
        for other in 0..3 {
            assert_eq!(
                cold.rule_cells[position + other]
                    .assignment_for_target(target)
                    .unwrap()
                    .is_some(),
                offset == other
            );
        }
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
