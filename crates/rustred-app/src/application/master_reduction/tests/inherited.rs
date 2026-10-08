use super::*;
use rustred::reduction::terminal_relations::TerminalEquation;
use std::collections::BTreeMap;

#[test]
fn publication_preserves_assisted_rows_guards_and_pending_work_without_providers() {
    for (saved, circuit) in [(true, false), (false, true), (true, true)] {
        let source = Scratch::new();
        let sample = session();
        let raw = BTreeSet::from([
            IntegralKey::try_new([1]).unwrap(),
            IntegralKey::try_new([3]).unwrap(),
        ]);
        let selected = MasterNormalizationProfile::StandardV1;
        let mut native = TerminalRelationSession::new(
            sample.family_owner().clone(),
            raw,
            0,
            selected.relation_limits(),
        )
        .unwrap();
        let mut options = MasterReductionOptions::new("unused", &source.0);
        options.saved_rule_assistance = saved;
        options.circuit_symmetry_assistance = circuit;
        let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0},"inputs":portable_inputs(&source.0)});
        profile::record(&mut report, selected);
        assistance::configure(&options, &mut native, &mut report).unwrap();
        let c = native.family_owner().coefficient_context();
        let d = c.parameter("d").unwrap();
        let equation = TerminalEquation {
            terms: BTreeMap::from([
                (
                    IntegralKey::try_new([2]).unwrap(),
                    c.try_sub(&d, &c.integer(4), Default::default()).unwrap(),
                ),
                (IntegralKey::try_new([3]).unwrap(), c.integer(-4)),
            ]),
            nonzero_conditions: vec![d],
        };
        while !native.is_complete() {
            native
                .step_with_provider(&AtomicBool::new(false), |key| {
                    Ok(if key.powers() == [2] {
                        vec![equation.clone()]
                    } else {
                        vec![]
                    })
                })
                .unwrap();
        }
        assert_eq!(native.statistics().completed_assistance_rows, 1);
        assert_eq!(native.statistics().remaining_terminals, 1);
        save(
            &options,
            &native,
            &mut report,
            "completed_nonminimal",
            Instant::now(),
            &|_| {},
        )
        .unwrap();
        publish_final(&options, &mut report).unwrap();
        let source_bytes = native.to_native_bytes(Default::default()).unwrap();
        let old_application = native
            .apply_terminal(&IntegralKey::try_new([3]).unwrap())
            .unwrap();
        let guards = native.nonzero_conditions().to_vec();
        let before = native.statistics();

        let output = Scratch::new();
        let mut published =
            json!({"schema":SCHEMA,"checkpoint":{"generation":0},"inputs":report["inputs"]});
        profile::record(&mut published, selected);
        inherited::capture(&native, &report, &mut published).unwrap();
        native
            .extend(&BTreeSet::from([IntegralKey::try_new([5]).unwrap()]), 0)
            .unwrap();
        let pending = native.statistics().pending_assistance_keys;
        assert!(pending > 0);
        assert!(native.statistics().pending_rebuild_rows > 0);
        let mut publishing = MasterReductionOptions::new("unused", &output.0);
        publishing.operation = MasterReductionOperation::Publish;
        assistance::configure(&publishing, &mut native, &mut published).unwrap();
        assert_eq!(published["finite_search_restarted_for_policy"], false);
        assert_eq!(published["saved_rule_assistance"], false);
        assert_eq!(
            published["inherited_equation_authority"]["saved_rule_assistance"],
            saved
        );
        assert_eq!(
            published["relation_authority"]
                .as_str()
                .unwrap()
                .contains("candidate source provenance"),
            saved
        );
        assert!(
            assistance::prepare(
                &publishing,
                &native,
                &mut published,
                &AtomicBool::new(false),
                &|_| {},
                Instant::now()
            )
            .unwrap()
            .is_none()
        );
        // Pause with reindexing still pending, cold-load, and publish. No owner
        // payloads were copied, so saved provider preparation would also fail.
        save(
            &publishing,
            &native,
            &mut published,
            "paused",
            Instant::now(),
            &|_| {},
        )
        .unwrap();
        let mut loaded = load_master_relation_session(&output.0).unwrap();
        publishing.resume = true;
        assistance::validate(&publishing, &loaded, &published).unwrap();
        execute_session(
            &publishing,
            &mut loaded,
            &mut published,
            &AtomicBool::new(false),
            &|_| {},
            Instant::now(),
        )
        .unwrap();
        let after = loaded.statistics();
        assert_eq!(after.completed_source_rows, before.completed_source_rows);
        assert_eq!(
            after.completed_assistance_keys,
            before.completed_assistance_keys
        );
        assert_eq!(
            after.completed_assistance_rows,
            before.completed_assistance_rows
        );
        assert_eq!(after.pending_assistance_keys, pending);
        assert_eq!(after.pending_rebuild_rows, 0);
        assert_eq!(loaded.nonzero_conditions(), guards);
        assert_eq!(
            loaded
                .apply_terminal(&IntegralKey::try_new([3]).unwrap())
                .unwrap(),
            old_application
        );
        assert_eq!(published["status"], "published_unrefined");
        assert_eq!(profile::from_report(&published).unwrap(), selected);
        assert_eq!(
            load_master_relation_session(&source.0)
                .unwrap()
                .to_native_bytes(Default::default())
                .unwrap(),
            source_bytes
        );

        let mut next = published.clone();
        inherited::capture(&loaded, &published, &mut next).unwrap();
        assert_eq!(
            next["inherited_equation_authority"],
            published["inherited_equation_authority"]
        );
        let stable = loaded.to_native_bytes(Default::default()).unwrap();
        next["inherited_equation_authority"]["saved_rule_assistance"] = json!(!saved);
        assert!(assistance::validate(&publishing, &loaded, &next).is_err());
        assert_eq!(loaded.to_native_bytes(Default::default()).unwrap(), stable);
        let stats = loaded.statistics();
        loaded.step_rebuild_only(&AtomicBool::new(false)).unwrap();
        assert_eq!(loaded.statistics(), stats);

        // A second publication extends the same frozen authority, rather than
        // claiming that its original finite circuit provider covered new keys.
        let repeated = Scratch::new();
        let mut repeated_options = MasterReductionOptions::new("unused", &repeated.0);
        repeated_options.operation = MasterReductionOperation::Publish;
        next = published.clone();
        next["checkpoint"] = json!({"generation":0});
        next["native_state"] = Value::Null;
        inherited::capture(&loaded, &published, &mut next).unwrap();
        collection::inherit(&output.0, &repeated.0, &mut next).unwrap();
        loaded
            .extend(&BTreeSet::from([IntegralKey::try_new([7]).unwrap()]), 0)
            .unwrap();
        assistance::configure(&repeated_options, &mut loaded, &mut next).unwrap();
        execute_session(
            &repeated_options,
            &mut loaded,
            &mut next,
            &AtomicBool::new(false),
            &|_| {},
            Instant::now(),
        )
        .unwrap();
        assert_eq!(
            loaded.statistics().completed_source_rows,
            before.completed_source_rows
        );
        assert_eq!(
            loaded.statistics().completed_assistance_rows,
            before.completed_assistance_rows
        );
        assert_eq!(
            next["inherited_equation_authority"],
            published["inherited_equation_authority"]
        );
        storage::clone_inputs(&source.0, &repeated.0, &report).unwrap();

        // Explicit refinement reuses the global saved binding, but an enlarged
        // circuit inventory deliberately starts a newly bound finite search.
        let refined = Scratch::new();
        let mut refining = MasterReductionOptions::new("unused", &refined.0);
        refining.saved_rule_assistance = saved;
        refining.circuit_symmetry_assistance = circuit;
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            master_refine_published_artifact(
                &repeated.0,
                &refining,
                &AtomicBool::new(false),
                |event| {
                    if event["event"] == "master_reduction_checkpoint" {
                        panic!("first durable refinement boundary");
                    }
                },
            )
        }));
        assert!(interrupted.is_err());
        let refined_report = master_reduction_inspect(&refined.0).unwrap();
        assert!(refined_report.get("inherited_equation_authority").is_none());
        assert_eq!(
            refined_report["finite_search_restarted_for_policy"],
            circuit
        );
        let refined_native = load_master_relation_session(&refined.0).unwrap();
        assert_eq!(refined_native.raw_terminals(), loaded.raw_terminals());
        assert_eq!(
            refined_native.statistics().completed_source_rows,
            if circuit {
                0
            } else {
                before.completed_source_rows
            }
        );
    }
}
