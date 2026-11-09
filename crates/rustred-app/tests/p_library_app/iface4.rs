//! Generation budgets, the caller's limits in a Custom step, the LP recount of
//! Provided payloads and re-generation from a recorded LP count (owner: M3-I1
//! rr-iface4; plan M3 D-M3-3, D-M3-4; departures PSF6 and D-T7c-6).
//!
//! One test per item, on t1star05 (the 2L kite with one eps unit, arity 5,
//! LP 1, 5 lines), the smallest Custom owner of the 3L DAG. The scope 1-3
//! regeneration from Provided payloads with the recount is `#[ignore]` and
//! runs optimised.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::{Arc, OnceLock};

use rustred::family::IntegralKey;
use rustred::propagator::OwnerClassId;
use rustred::propagator::PresentationId;
use rustred::propagator::census::{PCensusOptions, p_census};
use rustred::propagator::class::{ClassLimits, classify};
use rustred::propagator::custom::{
    build_custom_step, check_custom_owner, custom_owner_support, custom_payload_dump,
    inspect_custom_payload,
};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::lane::{LaneRoles, OwnerLane};
use rustred::propagator::library::{
    CustomOwnerMeta, OwnerProgram, PLibrary, POwnerSpec, PTerminal,
};
use rustred::propagator::presentation::Presentation;
use rustred::propagator::programs::CustomPayloadRef;
use rustred::propagator::step::{PKeyView, PStepContext};
use rustred::propagator::tag::GammaTag;
use rustred::reduction::{ReductionError, ReductionLimits};
use rustred_app::propagator::assemble::{LpRecount, RecountProvidedLp, recount_provided_lp};
use rustred_app::propagator::generate::{
    CustomOwnerNode, CustomOwnerOptions, CustomOwnerPayload, CustomOwnerReport, GenerationBudget,
    custom_owner_nodes, generate_custom_owner_with_report,
};
use rustred_app::propagator::package::assemble_p_package_recounted;
use rustred_app::propagator::{AssembleOptions, CustomPayloads, equivalent_p_libraries};

use super::package::{Scratch, scope_three};

/// The t1star05 node of the 3L DAG with its EpsNode presentation.
fn kite() -> &'static CustomOwnerNode {
    static KITE: OnceLock<CustomOwnerNode> = OnceLock::new();
    KITE.get_or_init(|| {
        let nodes = custom_owner_nodes(P_FAMILIES_V1_JSON, 3).unwrap();
        nodes.into_iter().find(|node| node.eps_lines == 1).unwrap()
    })
}

fn generated(
    options: &CustomOwnerOptions,
) -> Result<(CustomOwnerPayload, CustomOwnerReport), PError> {
    generate_custom_owner_with_report(&kite().presentation, options)
}

/// t1star05 generated without a budget, once per test process.
fn unbudgeted() -> &'static (CustomOwnerPayload, CustomOwnerReport) {
    static OWNER: OnceLock<(CustomOwnerPayload, CustomOwnerReport)> = OnceLock::new();
    OWNER.get_or_init(|| generated(&CustomOwnerOptions::default()).unwrap())
}

/// Same record and same rules (canonical dump), never compared by bytes.
fn assert_same_owner(payload: &CustomOwnerPayload, expected: &CustomOwnerPayload) {
    assert_eq!(payload.meta, expected.meta);
    assert_eq!(
        custom_payload_dump(&payload.bytes).unwrap(),
        custom_payload_dump(&expected.bytes).unwrap()
    );
}

fn with_budget(max_seconds: Option<f64>, max_rss_bytes: Option<u64>) -> CustomOwnerOptions {
    CustomOwnerOptions {
        budget: GenerationBudget {
            max_seconds,
            max_rss_bytes,
        },
        ..CustomOwnerOptions::default()
    }
}

/// Item 2: 1 ms or 1 MiB of RSS is too little for any owner, and the call
/// fails closed with the typed error of its resource; a budget the owner fits
/// in generates the same owner; an empty ceiling is refused before any work.
#[test]
fn a_generation_over_its_time_or_rss_budget_fails_closed() {
    for (options, resource) in [
        (with_budget(Some(0.001), None), "time"),
        (with_budget(None, Some(1 << 20)), "rss"),
    ] {
        match generated(&options) {
            Err(PError::BudgetExceeded {
                resource: found, ..
            }) => assert_eq!(found, resource),
            other => panic!("{resource} budget: {other:?}"),
        }
    }
    let (roomy, report) = generated(&with_budget(Some(3600.0), Some(1 << 40))).unwrap();
    assert_same_owner(&roomy, &unbudgeted().0);
    assert!(!report.lp_given);
    for options in [
        with_budget(Some(0.0), None),
        with_budget(Some(-1.0), None),
        with_budget(Some(f64::NAN), None),
        with_budget(None, Some(0)),
    ] {
        assert!(
            matches!(generated(&options), Err(PError::Unsupported(_))),
            "{options:?}"
        );
    }
}

/// t1star05 alone in a library: its full-support class owned by the
/// generated step, every other class without a program.
fn kite_library() -> &'static (PLibrary, OwnerClassId) {
    static LIBRARY: OnceLock<(PLibrary, OwnerClassId)> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let mut presentation = kite().presentation.clone();
        presentation.id = PresentationId(0);
        presentation.node = None;
        let presentation = Arc::new(presentation);
        let census = p_census(
            std::slice::from_ref(&presentation),
            None,
            &PCensusOptions::default(),
        )
        .unwrap();
        let context = presentation.family.coefficient_context().clone();
        let mut builder = PLibrary::builder(context, 2..=2);
        builder
            .add_presentation(Presentation::clone(&presentation))
            .unwrap();
        let payload = &unbudgeted().0;
        let support = custom_owner_support(&presentation);
        let mut owner = None;
        for (&class, record) in &census.classes {
            let mut spec = POwnerSpec {
                class,
                lane: OwnerLane::Custom,
                presentation: record.owner.0,
                node: None,
                roles: LaneRoles::None,
                edge_maps: Vec::new(),
                payload: None,
                custom: None,
                terminals: Vec::new(),
            };
            if record.owner.1 != support {
                let missing = OwnerProgram::Missing { ledger: 0 };
                builder.add_owner_with_program(spec, missing).unwrap();
                continue;
            }
            owner = Some(class);
            for residual in inspect_custom_payload(&payload.bytes)
                .unwrap()
                .residual_keys()
            {
                let key = IntegralKey::try_new(residual).unwrap();
                let terminal = PTerminal::Residual { owner: class, key };
                spec.terminals.push(builder.add_terminal(terminal).unwrap());
            }
            let step = build_custom_step(&presentation, &payload.bytes, &payload.meta).unwrap();
            spec.payload = Some(CustomPayloadRef::Bytes(Arc::clone(&payload.bytes)));
            spec.custom = Some(payload.meta.clone());
            builder
                .add_owner_with_program(spec, OwnerProgram::Lane(step))
                .unwrap();
        }
        builder.set_census(census).unwrap();
        (builder.build().unwrap(), owner.unwrap())
    })
}

/// Item 1 (D-T7c-6): a Custom step evaluates its rule under the request's
/// limits. Default limits give the step they always gave (and so do roomy
/// diagnostics limits); with no rule application allowed the step is refused.
#[test]
fn a_custom_step_evaluates_under_the_callers_limits() {
    let (library, class) = kite_library();
    let owner = library.owner(*class).unwrap();
    let OwnerProgram::Lane(step) = owner.program(library).unwrap() else {
        panic!("the kite owner has its step");
    };
    // The corner of the support with a dot on its first integer line: no
    // residual, one rule application.
    let presentation = library.presentation(PresentationId(0));
    let support = custom_owner_support(presentation);
    let forced = presentation.forced_axes();
    let mut powers: Vec<i64> = (0..presentation.slots.len())
        .map(|slot| i64::from(support >> slot & 1 == 1))
        .collect();
    powers[(support & !forced).trailing_zeros() as usize] = 2;
    let tag = GammaTag::one();
    let key = PKeyView {
        presentation: PresentationId(0),
        powers: &powers,
        tag: &tag,
    };
    let run = |limits: &ReductionLimits| {
        let cx = PStepContext {
            library,
            owner,
            limits,
        };
        step.step(&cx, &key)
    };
    let applied = run(&ReductionLimits::default()).unwrap();
    assert!(!applied.children.is_empty(), "{applied:?}");
    assert_eq!(
        run(&ReductionLimits::default().with_diagnostics()).unwrap(),
        applied
    );
    let no_rule = ReductionLimits {
        max_rule_applications: 0,
        ..ReductionLimits::default()
    }
    .with_diagnostics();
    assert!(
        matches!(
            run(&no_rule),
            Err(PError::Engine(ReductionError::RuleApplicationLimit {
                limit: 0,
                ..
            }))
        ),
        "{:?}",
        run(&no_rule)
    );
}

/// Item 3 (PSF6): the recount of a Provided payload takes the LP count again
/// and refuses a record that states another one, even one every guard
/// passes (LP 0 over a residual); an owner outside the line filter keeps its
/// record's word, as at load. The filter: at most 9 lines, every owner only
/// for `P_LP_RECOUNT_ALL=1`.
#[test]
fn the_lp_recount_refuses_a_tampered_record_inside_its_line_filter() {
    let presentation = &kite().presentation;
    let payload = &unbudgeted().0;
    let recount = |payload: &CustomOwnerPayload, filter: RecountProvidedLp| {
        recount_provided_lp("t1star05", presentation, payload, &filter)
    };
    let default = RecountProvidedLp::for_variable(None);
    assert_eq!(
        recount(payload, default).unwrap(),
        LpRecount {
            label: "t1star05".to_owned(),
            lines: 5,
            stored: 1,
            recounted: Some(1),
        }
    );
    let tampered = CustomOwnerPayload {
        meta: CustomOwnerMeta {
            lp_count: 0,
            ..payload.meta.clone()
        },
        ..payload.clone()
    };
    check_custom_owner(presentation, &tampered.meta).unwrap();
    assert_eq!(
        recount(&tampered, default),
        Err(PError::MalformedLibrary(
            "the Provided Custom owner t1star05 records LP 0, its support 0x1f (5 lines) counts LP 1"
                .to_owned()
        ))
    );
    let outside = recount(&tampered, RecountProvidedLp::up_to_lines(4)).unwrap();
    assert_eq!((outside.stored, outside.recounted), (0, None));
    for (value, max_lines) in [
        (None, Some(9)),
        (Some(""), Some(9)),
        (Some("0"), Some(9)),
        (Some("1"), None),
    ] {
        assert_eq!(
            RecountProvidedLp::for_variable(value).max_lines(),
            max_lines,
            "{value:?}"
        );
    }
}

fn p_generate(arguments: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rustred"));
    command.arg("p-generate").args(arguments).output().unwrap()
}

fn path(directory: &Path) -> &str {
    directory.to_str().unwrap()
}

fn record(directory: &Path, label: &str) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(directory.join(format!("{label}.json"))).unwrap())
        .unwrap()
}

/// Item 4: a re-generation from a recorded LP count takes it as given and
/// says so, in the report and in the owner record of `p-generate --lp-from`
/// (`lp_count_source`, `lp_given_from`); the owner is the counted one. The
/// budget flags of `p-generate` fail an owner over budget, nothing written.
#[test]
fn a_regeneration_from_a_recorded_lp_count_records_it() {
    let (given, report) = generated(&CustomOwnerOptions {
        lp_given: Some(1),
        ..CustomOwnerOptions::default()
    })
    .unwrap();
    assert_same_owner(&given, &unbudgeted().0);
    assert!(report.lp_given && report.lp_seconds == 0.0);
    assert!(!unbudgeted().1.lp_given);

    let scratch = Scratch::new("iface4-generate");
    let families = scratch.join("p-families.json");
    std::fs::write(&families, P_FAMILIES_V1_JSON).unwrap();
    let presentation = &kite().presentation;
    let label = classify(
        presentation,
        custom_owner_support(presentation),
        &ClassLimits::default(),
    )
    .unwrap()
    .unwrap()
    .key
    .label();
    let run = |out: &Path, more: &[&str]| {
        let common = [
            "--families",
            path(&families),
            "--max-loops",
            "3",
            "--owner",
            label.as_str(),
            "--out",
            path(out),
        ];
        p_generate(&[&common[..], more].concat())
    };
    let (counted, regenerated) = (scratch.join("counted"), scratch.join("given"));
    let output = run(&counted, &["--max-seconds", "3600", "--max-rss", "64GiB"]);
    assert!(output.status.success(), "{output:?}");
    let first = record(&counted, &label);
    assert_eq!(
        (&first["lp_count"], &first["lp_count_source"]),
        (&serde_json::json!(1), &serde_json::json!("counted"))
    );
    assert_eq!(
        first["budget"]["max_rss_bytes"],
        serde_json::json!(64_u64 << 30)
    );
    let output = run(&regenerated, &["--lp-from", path(&counted)]);
    assert!(output.status.success(), "{output:?}");
    let second = record(&regenerated, &label);
    assert_eq!(
        (&second["lp_count"], &second["lp_count_source"]),
        (&serde_json::json!(1), &serde_json::json!("given"))
    );
    let from = counted.join(format!("{label}.json"));
    assert_eq!(second["lp_given_from"], serde_json::json!(path(&from)));
    assert_eq!(
        (&second["rules"], &second["residual_keys"]),
        (&first["rules"], &first["residual_keys"])
    );

    for (flag, value, resource) in [
        ("--max-seconds", "0.001", "time"),
        ("--max-rss", "1MiB", "rss"),
    ] {
        let refused = scratch.join(resource);
        let output = run(&refused, &[flag, value]);
        assert_eq!(output.status.code(), Some(5), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("{resource} budget exceeded")),
            "{stderr}"
        );
        assert!(!refused.join(format!("{label}.json")).exists());
    }
}

/// Item 3 at scope 1-3 (M3-T8c runs it at 1-4): the program of the built-in
/// library again, from its Provided payloads (NO and t1star05, both at most 9
/// lines, so both recounted under the default filter) with the descent
/// certificate; a Provided record with another LP count is refused.
#[test]
#[ignore = "slow: scope 1-3 generated once, then assembled twice from Provided payloads, run optimised"]
fn scope_one_to_three_reassembles_from_provided_payloads_with_the_lp_recount() {
    let assembled = scope_three();
    let families: PFamiliesFile = serde_json::from_str(P_FAMILIES_V1_JSON).unwrap();
    let options = AssembleOptions {
        threads: 4,
        verify_descent: true,
    };
    let filter = RecountProvidedLp::from_env();
    let provided = assembled.custom.clone();
    assert_eq!(provided.len(), 2, "NO and t1star05");
    let again = |payloads: BTreeMap<String, CustomOwnerPayload>| {
        assemble_p_package_recounted(
            &families,
            3,
            CustomPayloads::Provided(payloads),
            &options,
            &filter,
        )
    };
    let (reassembled, recounts) = again(provided.clone()).unwrap();
    assert!(reassembled.descent.is_some());
    assert!(equivalent_p_libraries(&assembled.library, &reassembled.library).unwrap());
    let recounted: BTreeMap<&str, (u32, u32, Option<u32>)> = (recounts.iter())
        .map(|r| (r.label.as_str(), (r.lines, r.stored, r.recounted)))
        .collect();
    let expected: BTreeMap<&str, (u32, u32, Option<u32>)> = (provided.iter())
        .map(|(label, payload)| {
            let lines = inspect_custom_payload(&payload.bytes)
                .unwrap()
                .support
                .count_ones();
            (label.as_str(), (lines, 1, Some(1)))
        })
        .collect();
    assert_eq!(recounted, expected);
    assert!(expected.values().all(|&(lines, _, _)| lines <= 9));

    let mut tampered = provided.clone();
    let (label, payload) = tampered.iter_mut().next().unwrap();
    let label = label.clone();
    payload.meta.lp_count = 0;
    match again(tampered) {
        Err(PError::MalformedLibrary(message)) => {
            assert!(
                message.contains(&format!("{label} records LP 0")),
                "{message}"
            );
        }
        other => panic!(
            "a tampered record: {:?}",
            other.map(|(_, recounts)| recounts)
        ),
    }
}
