//! Declared-chart persistence is distinct from stronger search-domain inference.
use super::*;
use rustred::family::IntegralKey;
use rustred::solver::{
    CandidateReductionError, Case, ExceptionalConditions, Integral, RuleCandidate, SearchStats,
    SectorRule, Seed, SeedSource, Term,
};

#[test]
fn saved_saturatable_affine_chart_keeps_declared_layout_and_exact_application() {
    // Synthetic, explicitly uncertified transport/application fixture: no IBP
    // provenance or family-closure claim is made for this constructed rule.
    let limits = CandidateBundleLimits::default();
    let generated = family_candidates(FamilyCandidatesRequest::new(K3)).unwrap();
    let mut bundle = codec::read(generated.bundle(), limits).unwrap();
    let family = preparation::family(K3, InputFormat::Toml).unwrap();
    let prepared = preparation::prepare::<3>(family, &[true; 3], None).unwrap();
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .unwrap()
        .context()
        .clone();
    let indices = prepared.sources.index_variables();
    let equation = context
        .sub(
            &context
                .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                .unwrap(),
            &context.add(&context.one(), &context.one()).unwrap(),
        )
        .unwrap()
        .raw()
        .numerator
        .clone();
    let face = CoordinateCase::new([None, None, Some(1)]).unwrap();
    // This public declared-chart constructor must preserve the representation
    // used in existing candidate records, even though signs imply a corner.
    let AffineIntersection::Affine(affine) =
        AffineCase::from_coordinate(&face, &[equation], indices, &[true; 3]).unwrap()
    else {
        panic!("declared affine chart was rewritten")
    };
    let declared = Case::from(affine);
    assert_eq!(declared.fixed(), face.fixed());
    let target = declared.integral();
    assert!(target[0].is_symbolic() && target[1].is_symbolic());
    assert!(!target[2].is_symbolic());
    let rhs_integral = target.shifted([-1, 0, 0]).unwrap();
    let source = SeedSource {
        basis_row: 2,
        seed: Seed {
            integral: target.shifted([1, -1, 0]).unwrap(),
            shifts: [1, -1, 2],
        },
    };
    let mut solved = codec::solutions::<3>(&bundle, &context, indices, limits).unwrap();
    for (sector, solution) in &mut solved {
        solution.rules.clear();
        solution.finite_residuals.clear();
        if *sector == [true; 3] {
            solution.rules.push(SectorRule {
                candidate: RuleCandidate {
                    case: declared.clone(),
                    target,
                    rhs: vec![Term {
                        integral: rhs_integral,
                        coefficient: context.one().raw().clone(),
                    }],
                    sources: vec![source.clone()],
                    stats: SearchStats::default(),
                },
                exceptions: ExceptionalConditions {
                    branches: Vec::new(),
                },
            });
        } else if *sector == [false, true, true] {
            solution
                .finite_residuals
                .push(Integral::numeric([0, 1, 1]).unwrap());
        }
    }
    replace_solutions(&mut bundle, &solved, limits);
    let bytes = codec::write(&bundle, limits).unwrap();
    let decoded = codec::read(&bytes, limits).unwrap();
    assert_eq!(decoded.records, bundle.records);
    let reconstructed = codec::solutions::<3>(&decoded, &context, indices, limits).unwrap();
    let rule = &reconstructed
        .iter()
        .find(|(sector, _)| *sector == [true; 3])
        .unwrap()
        .1
        .rules[0];
    assert_eq!(rule.candidate.case, declared);
    assert_eq!(rule.candidate.case.fixed(), &[None, None, Some(1)]);
    assert_eq!(rule.candidate.target, target);
    assert_eq!(rule.candidate.rhs[0].integral, rhs_integral);
    assert_eq!(rule.candidate.rhs[0].coefficient, *context.one().raw());
    assert_eq!(rule.candidate.sources, [source]);

    // Search inference is explicitly requested and leaves the saved chart
    // untouched. The complete old affine equality survives until this point.
    let corner: Case<3> = CoordinateCase::new([Some(1); 3]).unwrap().into();
    assert_eq!(
        declared.intersect(&[], indices, &[true; 3]).unwrap(),
        Some(corner)
    );
    assert!(declared.affine().is_some());

    let (_, mut reducer) =
        load_generated_candidate_bundle::<3>(&bytes, limits, Default::default()).unwrap();
    let root = IntegralKey::try_new([1, 1, 1]).unwrap();
    let child = IntegralKey::try_new([0, 1, 1]).unwrap();
    let reduced = reducer.reduce_unit_mass(&root).unwrap();
    assert_eq!(reduced.terms().len(), 1);
    assert_eq!(reduced.terms().get(&child), Some(&context.base().one()));
    assert_eq!(
        reduced.terms(),
        reducer.reduce_unit_mass(&root).unwrap().terms()
    );
    let outside = IntegralKey::try_new([2, 1, 1]).unwrap();
    assert!(matches!(reducer.reduce_unit_mass(&outside),
        Err(CandidateReductionError::Uncovered { target }) if target == outside));

    // The selective immutable loader uses the same strict declared-chart
    // decoder, including target/RHS/source layout. Deliberately different
    // saved roots and priorities are carried by the two synthetic owners.
    let saved = decoded
        .sectors
        .iter()
        .map(|sector| {
            let mut shard = decoded.clone();
            shard.sectors = vec![sector.clone()];
            if sector.sector != [true; 3] {
                shard.root_sector = sector.sector.clone();
                shard.permutation = Some(vec![2, 0, 1]);
            }
            (
                rustred::sector::Mask::try_new(sector.sector.clone()).unwrap(),
                codec::write(&shard, limits).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let input = saved
        .iter()
        .map(|(mask, bytes)| CandidateOwnerBundle {
            bytes,
            owner_sector: mask,
        })
        .collect::<Vec<_>>();
    let (_, owners) =
        load_generated_candidate_owners::<3>(&input, Default::default(), Default::default())
            .unwrap();
    let routed = rustred::solver::RoutedCandidateReducer::try_new(
        std::sync::Arc::new(owners),
        [],
        Default::default(),
    )
    .unwrap();
    let trace = routed.trace_targets([root]).unwrap();
    assert!(trace.frontier().is_empty());
    assert_eq!(trace.rule_applications(), 1);
    assert_eq!(
        trace.declared_terminals(),
        &std::collections::BTreeSet::from([child])
    );
    let trace = routed.trace_targets([outside.clone()]).unwrap();
    assert_eq!(trace.frontier().len(), 1);
    assert_eq!(trace.frontier().first().unwrap().target, outside);
}
