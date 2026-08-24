//! Process-isolated regression tests for Symbolica's global symbol registry.
//!
//! A registered callback, alias, or user-data payload cannot be removed again,
//! so every matrix entry runs in a fresh copy of this integration-test binary.
//! The environment dispatch happens before the parent launcher, which also
//! makes recursive child creation impossible.

use std::env;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rustred::{
    CoefficientContext, SymbolicaAffineDenominatorCompiler, SymbolicaAffineDenominatorError,
    SymbolicaAffineDenominatorLimits, SymbolicaIntegralInputCompiler, SymbolicaIntegralInputError,
    SymbolicaIntegralInputLimits,
};
use symbolica::atom::{NamespacedSymbol, SymbolBuilder, UserData};
use symbolica::prelude::AtomCore;

const CHILD_CASE_ENV: &str = "RUSTRED_SYMBOL_REGISTRY_POISON_CHILD_CASE";
const EXACT_TEST_NAME: &str = "symbol_registry_poison_matrix";
const CHILD_TIMEOUT: Duration = Duration::from_secs(60);
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);
const CHILD_CASES: &[&str] = &[
    "grammar_callback",
    "raw_input_callback",
    "raw_input_alias",
    "affine_parameter_callback",
    "affine_parameter_user_data",
    "affine_sp_callback",
    "qualified_canonical_sp_replay",
];

fn symbol_builder(name: &str) -> SymbolBuilder {
    let namespaced = NamespacedSymbol::try_parse(name)
        .unwrap_or_else(|| panic!("test symbol {name:?} must be a valid qualified name"));
    SymbolBuilder::new(namespaced)
}

fn register_hostile_callback(name: &str) -> Arc<AtomicUsize> {
    let invocations = Arc::new(AtomicUsize::new(0));
    let callback_invocations = Arc::clone(&invocations);
    symbol_builder(name)
        .with_normalization_function(move |_, out| {
            callback_invocations.fetch_add(1, Ordering::SeqCst);
            out.to_num(0);
        })
        .build()
        .unwrap_or_else(|error| panic!("register callback on {name:?}: {error}"));
    invocations
}

fn assert_callback_not_invoked(invocations: &AtomicUsize, boundary: &str) {
    assert_eq!(
        invocations.load(Ordering::SeqCst),
        0,
        "hostile normalization callback ran before {boundary} rejected its symbol"
    );
}

fn register_alias(name: &str, alias: &str) {
    symbol_builder(name)
        .with_aliases([alias])
        .build()
        .unwrap_or_else(|error| panic!("register alias on {name:?}: {error}"));
}

fn register_user_data(name: &str) {
    symbol_builder(name)
        .with_user_data(UserData::Integer(7))
        .build()
        .unwrap_or_else(|error| panic!("register user data on {name:?}: {error}"));
}

fn input_compiler() -> SymbolicaIntegralInputCompiler {
    match SymbolicaIntegralInputCompiler::new(SymbolicaIntegralInputLimits::default()) {
        Ok(compiler) => compiler,
        Err(error) => panic!("plain compact-input grammar must initialize: {error}"),
    }
}

fn expect_input_symbol_rejection(
    error: SymbolicaIntegralInputError,
    expected_symbol: &str,
    expected_reason: &'static str,
) {
    match error {
        SymbolicaIntegralInputError::UnsafeRegisteredSymbol { symbol, reason } => {
            assert_eq!(symbol, expected_symbol);
            assert_eq!(reason, expected_reason);
        }
        other => panic!("expected an unsafe registered-symbol rejection, found {other}"),
    }
}

fn affine_compiler_with_parameter(
    parameter: &str,
) -> Result<SymbolicaAffineDenominatorCompiler, SymbolicaAffineDenominatorError> {
    let coefficients = CoefficientContext::try_new([parameter])
        .unwrap_or_else(|error| panic!("construct coefficient context: {error}"));
    SymbolicaAffineDenominatorCompiler::try_new(
        coefficients,
        vec!["k".to_owned()],
        vec![],
        vec![],
        SymbolicaAffineDenominatorLimits::default(),
    )
}

fn expect_affine_symbol_rejection(
    result: Result<SymbolicaAffineDenominatorCompiler, SymbolicaAffineDenominatorError>,
    expected_label: &str,
    expected_violation: &'static str,
) {
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("an impure affine symbol must be rejected"),
    };
    match error {
        SymbolicaAffineDenominatorError::ImpureDeclaredSymbol { label, violation } => {
            assert_eq!(label, expected_label);
            assert_eq!(violation, expected_violation);
        }
        other => panic!("expected an impure declared-symbol rejection, found {other}"),
    }
}

fn run_child_case(case: &str) {
    match case {
        "grammar_callback" => {
            let invocations = register_hostile_callback("rustred::I");
            let error = match SymbolicaIntegralInputCompiler::new(
                SymbolicaIntegralInputLimits::default(),
            ) {
                Err(error) => error,
                Ok(_) => panic!("a callback-poisoned grammar head must be rejected"),
            };
            expect_input_symbol_rejection(error, "rustred::I", "a custom callback is registered");
            assert_callback_not_invoked(&invocations, "compact grammar initialization");
        }
        "raw_input_callback" => {
            let invocations = register_hostile_callback("rustred::m2");
            let error = input_compiler()
                .compile_str("I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1))")
                .expect_err("a callback-poisoned raw scalar must be rejected");
            expect_input_symbol_rejection(error, "rustred::m2", "a custom callback is registered");
            assert_callback_not_invoked(&invocations, "raw compact-input authentication");
        }
        "raw_input_alias" => {
            register_alias("rustred::m2", "m2_poison_alias");
            let error = input_compiler()
                .compile_str("I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1))")
                .expect_err("an aliased raw scalar must be rejected");
            expect_input_symbol_rejection(error, "rustred::m2", "aliases are registered");
        }
        "affine_parameter_callback" => {
            let invocations = register_hostile_callback("rustred::a");
            expect_affine_symbol_rejection(
                affine_compiler_with_parameter("a"),
                "a",
                "a callback or custom function is registered",
            );
            assert_callback_not_invoked(&invocations, "affine parameter authentication");
        }
        "affine_parameter_user_data" => {
            register_user_data("rustred::a");
            expect_affine_symbol_rejection(
                affine_compiler_with_parameter("a"),
                "a",
                "custom user data is registered",
            );
        }
        "affine_sp_callback" => {
            let invocations = register_hostile_callback("rustred::sp");
            expect_affine_symbol_rejection(
                affine_compiler_with_parameter("a"),
                "sp",
                "a callback or custom function is registered",
            );
            assert_callback_not_invoked(&invocations, "affine sp-head authentication");
        }
        "qualified_canonical_sp_replay" => {
            let compiler = affine_compiler_with_parameter("a")
                .unwrap_or_else(|error| panic!("construct plain affine compiler: {error}"));
            let unqualified = compiler
                .compile_str("sp(k,k)+a")
                .unwrap_or_else(|error| panic!("compile unqualified sp expression: {error}"));
            let qualified = compiler
                .compile_str("rustred::sp(rustred::k,rustred::k)+rustred::a")
                .unwrap_or_else(|error| panic!("compile qualified sp expression: {error}"));
            let canonical_source = unqualified.source().to_canonical_string();
            assert!(
                canonical_source.contains("sp"),
                "canonical replay control must retain the scalar-product head"
            );
            let canonical = compiler
                .compile_str(&canonical_source)
                .unwrap_or_else(|error| panic!("compile canonical sp expression: {error}"));
            assert_eq!(
                qualified.affine_denominator(),
                unqualified.affine_denominator()
            );
            assert_eq!(
                canonical.affine_denominator(),
                unqualified.affine_denominator()
            );
            unqualified
                .verify_replay(&compiler)
                .unwrap_or_else(|error| panic!("verify canonical affine replay: {error}"));
        }
        other => panic!("unknown isolated symbol-registry case {other:?}"),
    }
}

fn spawn_child(executable: &std::path::Path, case: &'static str) -> Child {
    Command::new(executable)
        .args([
            "--exact",
            EXACT_TEST_NAME,
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_CASE_ENV, case)
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn isolated case {case:?}: {error}"))
}

fn wait_for_child(mut child: Child, case: &str, deadline: Instant) -> std::process::Output {
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .unwrap_or_else(|error| panic!("collect isolated case {case:?}: {error}"));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(CHILD_POLL_INTERVAL),
            Ok(None) => {
                let kill_error = child.kill().err();
                let output = child.wait_with_output().unwrap_or_else(|error| {
                    panic!("collect timed-out isolated case {case:?}: {error}")
                });
                panic!(
                    "isolated symbol-registry case {case:?} exceeded {CHILD_TIMEOUT:?}; kill error: {kill_error:?}\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                );
            }
            Err(error) => panic!("poll isolated case {case:?}: {error}"),
        }
    }
}

#[test]
fn symbol_registry_poison_matrix() {
    if let Some(case) = env::var_os(CHILD_CASE_ENV) {
        let case = case
            .to_str()
            .expect("the parent writes an ASCII child-case name");
        run_child_case(case);
        return;
    }

    let executable = env::current_exe().expect("locate this integration-test executable");
    // Start every isolated case before waiting, so the matrix remains parallel
    // under both Cargo's libtest harness and cargo-nextest.
    let children = CHILD_CASES
        .iter()
        .map(|&case| (case, spawn_child(&executable, case)))
        .collect::<Vec<_>>();
    let deadline = Instant::now()
        .checked_add(CHILD_TIMEOUT)
        .expect("the bounded child deadline must fit in Instant");

    for (case, child) in children {
        let output = wait_for_child(child, case, deadline);
        assert!(
            output.status.success(),
            "isolated symbol-registry case {case:?} failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
