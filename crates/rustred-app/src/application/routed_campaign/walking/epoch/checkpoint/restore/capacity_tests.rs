//! Cross-capacity CP6 runtime regressions. Synthetic inspection results exercise
//! the real merge, sealed record writer, publication and resume paths.
use super::super::{
    MergeBoundary,
    metadata::{Admission, Identity, Inputs},
    publication,
    tests::Directory,
};
use super::*;
use crate::application::routed_campaign::{
    matching::input::Query as InputQuery,
    walking::{
        descendant_closure::Tracker,
        epoch::{
            admit_initial,
            anchors::{AnchorScope, Piece},
            dispatch::{Dispatch, Refill},
            job::{BreakReason, ErrorKind, G2Part, Job, JobResult, Miss, NativeKind},
            merge::{self, MergeConfig, RecordOut},
            record_store::Sidecar,
            records::{Builder, typed::Record},
            state::EpochState,
        },
        queue::{CompactDomain, Domain, Phase},
    },
};
use crate::{
    OwnerDomainMatchRequest, OwnerDomainWalkG2ResidualAnchors, OwnerDomainWalkPublicationPolicy,
    OwnerDomainWalkRequest,
};
use rustred::{
    family::IntegralFamily,
    solver::{
        CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerScope,
        DomainPowerBounds, FiniteCasePolicy, IntegralOrder, RoutedCandidateReducer, SectorSolution,
    },
};
use serde_json::json;
use std::{fs, sync::Arc};

const SOURCE: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="cp6_capacity_restore"
loop_momenta=["q1","q2"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P1"
expression="q1^2-1"
[[family.denominators]]
id="P2"
expression="q2^2-1"
[[family.denominators]]
id="P3"
expression="(q1+q2)^2-1"
[target]
powers=[1,0,0]
"#;

fn reducer<const N: usize>() -> RoutedCandidateReducer<N> {
    let prepared =
        crate::application::input::prepare_input(SOURCE, crate::InputFormat::Toml).unwrap();
    let (_, _, _, lowered) = crate::application::lowering::lower_project(prepared)
        .unwrap()
        .into_parts();
    let family: Arc<IntegralFamily> = Arc::new(lowered.into_family());
    let owner = std::array::from_fn(|axis| axis == 0);
    let context = Arc::new(
        CandidateOwnerContext::try_new(
            family,
            CandidateOwnerScope {
                max_numerator_rank: Some(100),
                finite_case_policy: FiniteCasePolicy::SearchFinite,
            },
            vec![],
            Default::default(),
        )
        .unwrap(),
    );
    let programs = CandidateOwnerPrograms::try_new(
        context,
        [CandidateOwnerInput {
            sector: owner,
            saved_root: owner,
            ordering: rustred::sector::OrderingPolicy::SpiredUncutV1,
            solution: SectorSolution {
                order: IntegralOrder::new(owner, [false; N]),
                max_numerator_rank: Some(100),
                finite_case_policy: FiniteCasePolicy::SearchFinite,
                rules: vec![],
                finite_residuals: vec![],
                stats: Default::default(),
            },
        }],
    )
    .unwrap();
    RoutedCandidateReducer::try_new(Arc::new(programs), [], Default::default()).unwrap()
}

fn domain(lower: [u64; 3], upper: [u64; 3], min_difference: Option<i64>) -> Domain<3> {
    Domain {
        phase: Phase::Apply,
        owner: [true, false, false],
        lower: lower.to_vec(),
        upper: upper.into_iter().map(Some).collect(),
        rank: Some(100),
        powers: DomainPowerBounds {
            min_power_difference: min_difference,
            ..Default::default()
        },
    }
}

fn result<const N: usize>(job: &Job<N>, misses: &[Domain<N>]) -> JobResult<N> {
    let n = misses.len() as u64;
    JobResult {
        seq: job.seq,
        parent: job.parent,
        v0: job.v0,
        kind: NativeKind::Apply,
        error_kind: ErrorKind::None,
        break_reason: BreakReason::None,
        panic: false,
        emitted: n,
        accepted: n,
        stats_events: n,
        successors: n,
        conditional: 0,
        known_reuse: 0,
        job_duplicates: 0,
        optional: [0; 3],
        route_masks: 0,
        route_joint_pruned: 0,
        seconds: 0.0,
        stats_json: format!("{{\"events\":{n},\"successors\":{n}}}").into_bytes(),
        error: None,
        frontiers: vec![],
        refusals: vec![],
        refusals_truncated: false,
        scope: None,
        g2: None,
        finite_replay: None,
        lookup: None,
        misses: misses
            .iter()
            .enumerate()
            .map(|(ordinal, domain)| {
                let image = CompactDomain::try_from_domain(domain).unwrap();
                Miss {
                    ordinal: ordinal as u32,
                    digest: image.digest().0,
                    image,
                    target: None,
                }
            })
            .collect(),
    }
}

struct Output<'a>(&'a mut Sidecar);
impl RecordOut for Output<'_> {
    fn reserve(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn push(&mut self, record: Record) -> Result<(), String> {
        self.0.push(&record)
    }
}

fn merge<const N: usize>(
    state: &mut EpochState<N>,
    dispatch: &mut Dispatch,
    sidecar: &mut Sidecar,
    results: Vec<JobResult<N>>,
) {
    let config = MergeConfig {
        frontier_stop: true,
        lockstep: true,
        g2: true,
        finite_replay: None,
    };
    let checked = merge::p1_check(
        state,
        results.iter().map(JobResult::encode).collect(),
        config,
    )
    .unwrap();
    let plan = merge::p2(state, &checked).unwrap();
    let mut output = Output(sidecar);
    merge::p3_preflight(state, &checked, &plan, &mut output).unwrap();
    merge::p3_apply(
        state,
        checked,
        plan,
        config,
        &Builder,
        &mut output,
        &mut |id, attempts| dispatch.requeue(id, attempts),
    )
    .unwrap();
}

#[test]
fn physical_cp6_g2_resume_resave_and_capacity_resume_preserve_sealed_records() {
    let directory = Directory::new();
    let initial = [
        domain([0, 0, 0], [20, 20, 0], Some(6)),
        domain([30, 30, 0], [30, 30, 0], None),
    ];
    let queries: Vec<_> = initial
        .iter()
        .enumerate()
        .map(|(id, d)| InputQuery {
            id: format!("q-{id}"),
            auxiliary: false,
            role_declared: true,
            owner: d.owner.to_vec(),
            lower: d.lower.clone(),
            upper: d.upper.clone(),
            rank: d.rank,
            powers: d.powers,
        })
        .collect();
    let document = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":queries.iter().map(|q| json!({
        "id":q.id,"owner":"100","lower":q.lower,"upper":q.upper,"max_numerator_rank":q.rank,
        "power_bounds":crate::application::routed_campaign::walking::power_bounds_json(q.powers)
    })).collect::<Vec<_>>(), "query_roles":{"required":["q-0","q-1"],"auxiliary":[]}});
    let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
        "{}".into(),
        document.to_string(),
    ));
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.g2_residual_anchors = OwnerDomainWalkG2ResidualAnchors::Union;
    let owners = ["0".repeat(64)];
    let identity = Identity::new(&request, &owners, &queries).unwrap();
    let mut state = EpochState::<3>::new(
        request.max_domains,
        request.max_events,
        request.max_frontiers,
    );
    for d in &initial {
        admit_initial(&mut state, d).unwrap();
    }
    state.p0 = 2;
    state.tracker = Tracker::new(2);
    let mut dispatch = Dispatch::new();
    let mut publisher = publication::Store::fresh(directory.0.clone()).unwrap();
    let mut records = Sidecar::new(directory.0.clone(), 1);
    let Refill::Jobs(jobs) = dispatch.refill(&mut state, 2) else {
        panic!("initial jobs")
    };
    let descendant = domain([0, 0, 0], [12, 3, 0], None);
    merge(
        &mut state,
        &mut dispatch,
        &mut records,
        vec![
            result(&jobs[0], &[]),
            result(
                &jobs[1],
                &[descendant.clone(), domain([40, 40, 0], [40, 40, 0], None)],
            ),
        ],
    );
    let Refill::Jobs(jobs) = dispatch.refill(&mut state, 1) else {
        panic!("residual job")
    };
    assert_eq!(jobs[0].parent, 2);
    let mut residual = result(&jobs[0], &[]);
    residual.kind = NativeKind::G2Residual;
    residual.g2 = Some(G2Part {
        kind: 2,
        anchors: vec![(0, 1, 0)],
        pieces: vec![Piece {
            d_lo: Some(-2),
            d_hi: Some(5),
            lower: vec![0, 0, 0],
            upper: vec![12, 3, 0],
        }],
    });
    merge(&mut state, &mut dispatch, &mut records, vec![residual]);
    assert!(matches!(dispatch.refill(&mut state, 1), Refill::Jobs(_)));
    let rows: Vec<_> = queries
        .iter()
        .enumerate()
        .map(|(id, q)| json!({"id":q.id,"domain":id,"role":"required","role_declared":true}))
        .collect();
    let inputs = Inputs {
        identity: &identity,
        admission: Admission::Complete,
        rows: &rows,
        frontiers: &[],
        stop: None,
        operational_stop: None,
        admission_failure: None,
    };
    publisher
        .save(
            &MergeBoundary::borrow(&state, &dispatch, 16).unwrap(),
            &inputs,
            &mut records,
        )
        .unwrap();
    drop(publisher);
    drop(records);

    let physical = assembly::read::<3>(&directory.0, &identity, 16).unwrap();
    let segments = physical.record_segments.clone();
    let original_bytes: Vec<_> = segments
        .iter()
        .map(|segment| fs::read(directory.0.join(&segment.file)).unwrap())
        .collect();
    let digest = physical.scalars.records_digest.clone();
    drop(physical);
    let reducer = reducer::<4>();
    let mut restored = runtime::open(directory.0.clone(), &identity, &reducer, 16).unwrap();
    assert_eq!(restored.state.edges.records_digest(), digest);
    assert_eq!(restored.replay.len(), 1);
    assert_eq!(restored.replay[0].parent, 3);
    assert_eq!(
        restored.state.store.domains[2].raw_bounds(),
        (&[0, 0, 0, 0], &[12, 3, 0, 0])
    );
    let AnchorScope::Residual(pieces) = &restored.state.anchors.get(2).unwrap().scope else {
        panic!("residual scope")
    };
    assert_eq!(pieces[0].lower, [0, 0, 0, 0]);
    assert_eq!(pieces[0].upper, [12, 3, 0, 0]);
    let replay = std::mem::take(&mut restored.replay);
    merge(
        &mut restored.state,
        &mut restored.dispatch,
        &mut restored.records,
        vec![result(&replay[0], &[])],
    );
    let inputs = Inputs {
        identity: &identity,
        admission: restored.admission,
        rows: &restored.roots.rows,
        frontiers: &restored.roots.frontiers,
        stop: None,
        operational_stop: None,
        admission_failure: None,
    };
    restored
        .publisher
        .save(
            &MergeBoundary::borrow(&restored.state, &restored.dispatch, 16).unwrap(),
            &inputs,
            &mut restored.records,
        )
        .unwrap();
    drop(restored);

    let saved = assembly::read::<4>(&directory.0, &identity, 16).unwrap();
    assert_eq!(saved.manifest.arity, 4);
    assert_eq!(
        &saved.record_segments[..segments.len()],
        segments.as_slice()
    );
    assert_ne!(
        saved.scalars.records_digest, digest,
        "the new merge extends the authority digest"
    );
    for (segment, bytes) in segments.iter().zip(&original_bytes) {
        assert_eq!(&fs::read(directory.0.join(&segment.file)).unwrap(), bytes);
    }
    let mut widths = Vec::new();
    for segment in &saved.record_segments {
        let mut reader = fs::File::open(directory.0.join(&segment.file)).unwrap();
        for _ in 0..segment.count {
            widths.push(
                crate::application::routed_campaign::walking::epoch::records::wire::read(
                    &mut reader,
                )
                .unwrap()
                .authority
                .image
                .owner
                .len(),
            );
        }
    }
    assert_eq!(widths, [3, 3, 3, 4]);
    drop(saved);
    let resumed = runtime::open(directory.0.clone(), &identity, &reducer, 16).unwrap();
    assert!(resumed.replay.is_empty());
    assert_eq!(resumed.state.k, 3);
    assert_eq!(resumed.state.anchors.len(), 1);
    assert!(
        resumed.warnings.is_empty(),
        "latest must validate without fallback"
    );
}
