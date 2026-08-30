use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::foundry::completion::stratum::GuardBranch;

use super::super::{CoefficientIdealGuardAtom, CoefficientIdealGuardLimits};
use super::{
    CoefficientIdealGuardDag, GuardDecisionCandidate, GuardDecisionCandidateId,
    GuardDecisionDagError, GuardDecisionDagLimits, GuardDecisionOutcome,
};

fn polynomial(
    context: &IndexedCoefficientContext,
    value: &crate::algebra::IndexedCoefficient,
) -> crate::algebra::IndexedPolynomial {
    context
        .numerator_condition_with_limits(value, Default::default())
        .unwrap()
}

fn atom(
    context: &IndexedCoefficientContext,
    value: &crate::algebra::IndexedCoefficient,
) -> CoefficientIdealGuardAtom {
    CoefficientIdealGuardAtom::try_from_pulled_back(
        context,
        polynomial(context, value),
        CoefficientIdealGuardLimits::default(),
    )
    .unwrap()
}

#[test]
fn ordered_first_applicable_semantics_and_literal_units_are_exact() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "semantic-dag-priority", 2).unwrap();
    let a = atom(
        &context,
        &context
            .sub(&context.index(0).unwrap(), &context.one())
            .unwrap(),
    );
    let b = atom(
        &context,
        &context
            .sub(&context.index(1).unwrap(), &context.one())
            .unwrap(),
    );
    let unit = atom(
        &context,
        &context.lift(&base.parameter("d").unwrap()).unwrap(),
    );
    let first_atoms = [a.clone(), unit, b.clone(), a.clone()];
    let second_atoms = [b.clone()];
    let fallback_atoms = [];
    let candidates = [
        GuardDecisionCandidate::new(GuardDecisionCandidateId(7), &first_atoms),
        GuardDecisionCandidate::new(GuardDecisionCandidateId(11), &second_atoms),
        GuardDecisionCandidate::new(GuardDecisionCandidateId(13), &fallback_atoms),
    ];
    let dag =
        CoefficientIdealGuardDag::try_compile(&context, &candidates, Default::default()).unwrap();

    assert_eq!(dag.stats().atoms, 2);
    assert_eq!(dag.stats().candidate_atom_references, 3);
    assert_eq!(
        dag.try_decide(&[GuardBranch::NonZero, GuardBranch::NonZero])
            .unwrap(),
        GuardDecisionOutcome::Candidate(GuardDecisionCandidateId(7))
    );
    let a_ordinal = dag.atom_ordinal(&a).unwrap();
    let b_ordinal = dag.atom_ordinal(&b).unwrap();
    let mut branches = vec![GuardBranch::NonZero; 2];
    branches[a_ordinal] = GuardBranch::Zero;
    assert_eq!(
        dag.try_decide(&branches).unwrap(),
        GuardDecisionOutcome::Candidate(GuardDecisionCandidateId(11))
    );
    branches[a_ordinal] = GuardBranch::NonZero;
    branches[b_ordinal] = GuardBranch::Zero;
    assert_eq!(
        dag.try_decide(&branches).unwrap(),
        GuardDecisionOutcome::Candidate(GuardDecisionCandidateId(13))
    );
    assert!(dag.try_verify(Default::default()).unwrap());
}

#[test]
fn shared_wall_conjunction_is_linear_and_fails_closed() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "semantic-dag-k15", 15).unwrap();
    let common = context
        .sub(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let mut atoms = Vec::new();
    for position in 1..15 {
        let other = context
            .sub(&context.index(position).unwrap(), &context.one())
            .unwrap();
        atoms.push(atom(&context, &context.mul(&common, &other).unwrap()));
    }
    let candidates = [GuardDecisionCandidate::new(
        GuardDecisionCandidateId(3),
        &atoms,
    )];
    let dag =
        CoefficientIdealGuardDag::try_compile(&context, &candidates, Default::default()).unwrap();
    let stats = dag.stats();
    assert_eq!(stats.atoms, 14);
    assert_eq!(stats.nodes, 14);
    assert_eq!(stats.edges, 28);

    let mut branches = vec![GuardBranch::NonZero; stats.atoms];
    assert_eq!(
        dag.try_decide(&branches).unwrap(),
        GuardDecisionOutcome::Candidate(GuardDecisionCandidateId(3))
    );
    for position in 0..branches.len() {
        branches[position] = GuardBranch::Zero;
        assert_eq!(
            dag.try_decide(&branches).unwrap(),
            GuardDecisionOutcome::Incomplete
        );
        branches[position] = GuardBranch::NonZero;
    }
}

#[test]
fn context_identity_candidate_identity_and_resource_caps_fail_typed() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "semantic-dag-limits", 2).unwrap();
    let foreign = IndexedCoefficientContext::try_new(&base, "semantic-dag-foreign", 2).unwrap();
    let local_atom = atom(&context, &context.index(0).unwrap());
    let foreign_atom = atom(&foreign, &foreign.index(0).unwrap());

    let foreign_atoms = [foreign_atom];
    let foreign_candidate = [GuardDecisionCandidate::new(
        GuardDecisionCandidateId(0),
        &foreign_atoms,
    )];
    assert_eq!(
        CoefficientIdealGuardDag::try_compile(&context, &foreign_candidate, Default::default())
            .unwrap_err(),
        GuardDecisionDagError::WrongAtomContext {
            candidate: 0,
            atom: 0,
        }
    );

    let local_atoms = [local_atom];
    let duplicate_candidates = [
        GuardDecisionCandidate::new(GuardDecisionCandidateId(2), &local_atoms),
        GuardDecisionCandidate::new(GuardDecisionCandidateId(2), &[]),
    ];
    assert_eq!(
        CoefficientIdealGuardDag::try_compile(&context, &duplicate_candidates, Default::default(),)
            .unwrap_err(),
        GuardDecisionDagError::DuplicateCandidate { candidate: 2 }
    );

    let descending_candidates = [
        GuardDecisionCandidate::new(GuardDecisionCandidateId(3), &local_atoms),
        GuardDecisionCandidate::new(GuardDecisionCandidateId(1), &[]),
    ];
    assert_eq!(
        CoefficientIdealGuardDag::try_compile(&context, &descending_candidates, Default::default())
            .unwrap_err(),
        GuardDecisionDagError::NonCanonicalCandidateOrder {
            previous: 3,
            current: 1,
        }
    );

    let one_candidate = [GuardDecisionCandidate::new(
        GuardDecisionCandidateId(1),
        &local_atoms,
    )];
    let mut node_limited = GuardDecisionDagLimits::default();
    node_limited.max_nodes = 0;
    assert!(matches!(
        CoefficientIdealGuardDag::try_compile(&context, &one_candidate, node_limited),
        Err(GuardDecisionDagError::ResourceLimit {
            resource: "semantic guard DAG nodes",
            requested: 1,
            limit: 0,
        })
    ));

    let mut edge_limited = GuardDecisionDagLimits::default();
    edge_limited.max_edges = 0;
    assert!(matches!(
        CoefficientIdealGuardDag::try_compile(&context, &one_candidate, edge_limited),
        Err(GuardDecisionDagError::ResourceLimit {
            resource: "semantic guard DAG edges",
            requested: 2,
            limit: 0,
        })
    ));

    let mut state_limited = GuardDecisionDagLimits::default();
    state_limited.max_states = 0;
    assert!(matches!(
        CoefficientIdealGuardDag::try_compile(&context, &one_candidate, state_limited),
        Err(GuardDecisionDagError::ResourceLimit {
            resource: "semantic guard DAG memo states",
            requested: 1,
            limit: 0,
        })
    ));

    let mut state_words_limited = GuardDecisionDagLimits::default();
    state_words_limited.max_state_words = 0;
    assert!(matches!(
        CoefficientIdealGuardDag::try_compile(&context, &one_candidate, state_words_limited),
        Err(GuardDecisionDagError::ResourceLimit {
            resource: "semantic guard DAG memo state words",
            requested: 1,
            limit: 0,
        })
    ));

    let mut scan_limited = GuardDecisionDagLimits::default();
    scan_limited.max_candidate_scans = 0;
    assert!(matches!(
        CoefficientIdealGuardDag::try_compile(&context, &one_candidate, scan_limited),
        Err(GuardDecisionDagError::ResourceLimit {
            resource: "semantic guard DAG candidate scans",
            requested: 1,
            limit: 0,
        })
    ));

    let mut work_limited = GuardDecisionDagLimits::default();
    work_limited.max_pending_work_items = 0;
    assert!(matches!(
        CoefficientIdealGuardDag::try_compile(&context, &one_candidate, work_limited),
        Err(GuardDecisionDagError::ResourceLimit {
            resource: "semantic guard DAG pending work items",
            requested: 1,
            limit: 0,
        })
    ));

    let mut byte_limited = GuardDecisionDagLimits::default();
    byte_limited.max_atom_identity_bytes = 0;
    assert!(matches!(
        CoefficientIdealGuardDag::try_compile(&context, &one_candidate, byte_limited),
        Err(GuardDecisionDagError::ResourceLimit {
            resource: "semantic guard DAG atom identity bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let dag = CoefficientIdealGuardDag::try_compile(&context, &one_candidate, Default::default())
        .unwrap();
    assert_eq!(
        dag.try_decide(&[]).unwrap_err(),
        GuardDecisionDagError::BranchArity {
            expected: 1,
            actual: 0,
        }
    );
}

#[test]
fn lazy_branch_oracle_queries_only_the_selected_path() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "semantic-dag-lazy", 3).unwrap();
    let mut atoms: Vec<_> = (0..3)
        .map(|position| atom(&context, &context.index(position).unwrap()))
        .collect();
    atoms.sort_by(|left, right| left.id().cmp(right.id()));
    let first = [atoms[0].clone()];
    let second = [atoms[1].clone(), atoms[2].clone()];
    let candidates = [
        GuardDecisionCandidate::new(GuardDecisionCandidateId(0), &first),
        GuardDecisionCandidate::new(GuardDecisionCandidateId(1), &second),
    ];
    let dag =
        CoefficientIdealGuardDag::try_compile(&context, &candidates, Default::default()).unwrap();

    let first_ordinal = dag.atom_ordinal(&atoms[0]).unwrap();
    let mut queried = Vec::new();
    let outcome = dag
        .try_decide_with(|ordinal| {
            queried.push(ordinal);
            GuardBranch::NonZero
        })
        .unwrap();
    assert_eq!(
        outcome,
        GuardDecisionOutcome::Candidate(GuardDecisionCandidateId(0))
    );
    assert_eq!(queried, [first_ordinal]);

    let unconditional = [GuardDecisionCandidate::new(
        GuardDecisionCandidateId(7),
        &[],
    )];
    let unconditional_dag =
        CoefficientIdealGuardDag::try_compile(&context, &unconditional, Default::default())
            .unwrap();
    let mut queries = 0usize;
    assert_eq!(
        unconditional_dag
            .try_decide_with(|_| {
                queries += 1;
                GuardBranch::Zero
            })
            .unwrap(),
        GuardDecisionOutcome::Candidate(GuardDecisionCandidateId(7))
    );
    assert_eq!(queries, 0);
}

#[test]
fn exhaustive_small_truth_table_matches_priority_conjunctions_deterministically() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "semantic-dag-truth", 4).unwrap();
    let atoms: Vec<_> = (0..4)
        .map(|position| {
            atom(
                &context,
                &context
                    .sub(&context.index(position).unwrap(), &context.one())
                    .unwrap(),
            )
        })
        .collect();
    let first = [atoms[2].clone(), atoms[0].clone(), atoms[2].clone()];
    let second = [atoms[1].clone()];
    let third = [atoms[3].clone(), atoms[2].clone()];
    let candidates = [
        GuardDecisionCandidate::new(GuardDecisionCandidateId(19), &first),
        GuardDecisionCandidate::new(GuardDecisionCandidateId(23), &second),
        GuardDecisionCandidate::new(GuardDecisionCandidateId(29), &third),
    ];
    let dag =
        CoefficientIdealGuardDag::try_compile(&context, &candidates, Default::default()).unwrap();

    let reordered_first = [atoms[0].clone(), atoms[2].clone()];
    let reordered_third = [atoms[2].clone(), atoms[3].clone()];
    let reordered = [
        GuardDecisionCandidate::new(GuardDecisionCandidateId(19), &reordered_first),
        GuardDecisionCandidate::new(GuardDecisionCandidateId(23), &second),
        GuardDecisionCandidate::new(GuardDecisionCandidateId(29), &reordered_third),
    ];
    assert_eq!(
        dag,
        CoefficientIdealGuardDag::try_compile(&context, &reordered, Default::default()).unwrap()
    );

    let ordinals: Vec<_> = atoms
        .iter()
        .map(|atom| dag.atom_ordinal(atom).unwrap())
        .collect();
    for assignment in 0_u8..16 {
        let mut branches = vec![GuardBranch::Zero; 4];
        for (logical, &ordinal) in ordinals.iter().enumerate() {
            if assignment & (1 << logical) != 0 {
                branches[ordinal] = GuardBranch::NonZero;
            }
        }
        let expected = if assignment & 0b0101 == 0b0101 {
            GuardDecisionOutcome::Candidate(GuardDecisionCandidateId(19))
        } else if assignment & 0b0010 != 0 {
            GuardDecisionOutcome::Candidate(GuardDecisionCandidateId(23))
        } else if assignment & 0b1100 == 0b1100 {
            GuardDecisionOutcome::Candidate(GuardDecisionCandidateId(29))
        } else {
            GuardDecisionOutcome::Incomplete
        };
        assert_eq!(dag.try_decide(&branches).unwrap(), expected);
    }
}
