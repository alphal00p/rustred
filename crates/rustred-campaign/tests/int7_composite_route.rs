//! int7 (implementer's ruling 2026-11-15; interfaces/closeloop.md 24.9): the default second route of a composite is decided
//! by the CONFIGURATION, not by a frozen manifest's feature word: with the proof kernel configured (path + SHA-256, the
//! program symcert's adapter runs for proof-symrows jobs) it is the kernel certificate; without one it is the same-backend
//! re-check (level `imported`); a configured kernel whose file has another SHA-256 (or none stated) is no kernel. Pure (no
//! process is started).

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::adapters::symcomp::{CompositeRoute, composite_second_route_configured};

#[test]
fn the_kernel_route_is_available_exactly_when_the_spec_names_the_kernel() {
    let d = std::env::temp_dir().join(format!("int7-composite-route-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let k = d.join("rr_proof");
    std::fs::write(&k, b"a stand-in kernel binary (test data)").unwrap();
    // configured: path + its SHA-256 (as symcert's adapter configures the kernel) -> the kernel certificate route
    let prog = Program::from_file("rr_proof", &k, Invocation::Legacy).unwrap();
    match composite_second_route_configured(Some(&prog)) {
        CompositeRoute::Kernel { program, sha256 } => {
            assert_eq!(program, k);
            assert_eq!(Some(sha256), prog.sha256.clone());
        }
        r => panic!("a configured kernel must give the kernel route: {r:?}"),
    }
    // not configured -> same backend (imported), with the reason
    match composite_second_route_configured(None) {
        CompositeRoute::SameBackend { why } => {
            assert!(why.contains("no proof kernel configured"), "{why}")
        }
        r => panic!("{r:?}"),
    }
    // configured but the file is not the stated one / no sha256 stated -> no kernel (never a guess)
    let mut wrong = prog.clone();
    wrong.sha256 = Some("0".repeat(64));
    assert!(matches!(
        composite_second_route_configured(Some(&wrong)),
        CompositeRoute::SameBackend { .. }
    ));
    let mut none = prog.clone();
    none.sha256 = None;
    assert!(matches!(
        composite_second_route_configured(Some(&none)),
        CompositeRoute::SameBackend { .. }
    ));
    let _ = std::fs::remove_dir_all(&d);
}
