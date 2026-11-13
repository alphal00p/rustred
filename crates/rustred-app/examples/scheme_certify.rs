//! scheme-certs: search stored certificates for replay-only Scheme rules with RustRed's own
//! source-weight reconstruction (rustred::propagator::scheme::certify), one JSON line per rule.
//! usage: scheme_certify <p-library dir> <scheme> <out.jsonl> [all | rule,rule,..]
//! Environment: CERT_LEVELS (default 0,1,2,3), CERT_MAX_ROWS (150000), CERT_SEEDS (2), CERT_MAX_DEGREE (64),
//! CERT_REDO=1 (also search rules with a stored certificate: validation), CERT_CHAIN=0 (no rule generators), CERT_VERBOSE=1, CERT_EQ_FREE=1.
//! The records are merged into the scheme file by SCRATCHPAD/scheme-certs/tools/merge.py and verified
//! again, independently, by examples/scheme_check (the program build's exact certificate check).
use std::io::Write;
use std::path::PathBuf;

use rustred::propagator::lane::OwnerLane;
use rustred::propagator::scheme::SchemeStep;
use rustred::propagator::scheme::certify::{CertifyOptions, certify_rules, uncertified_rules};
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("library dir"));
    let scheme = args.next().expect("scheme name");
    let out = PathBuf::from(args.next().expect("out.jsonl"));
    let which = args.next().unwrap_or_else(|| "all".to_owned());
    let library = load_p_library(
        PLibrarySource::Directory(dir),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .unwrap();
    let options = CertifyOptions {
        levels: std::env::var("CERT_LEVELS")
            .map(|v| v.split(',').map(|x| x.parse().unwrap()).collect())
            .unwrap_or_else(|_| vec![0, 1, 2, 3]),
        max_rows: env("CERT_MAX_ROWS", 150_000),
        seeds: env("CERT_SEEDS", 2),
        max_degree: env("CERT_MAX_DEGREE", 64),
        redo: env("CERT_REDO", 0u8) == 1,
        chain: env("CERT_CHAIN", 1u8) == 1,
        verbose: env("CERT_VERBOSE", 0u8) == 1,
        eq_free: env("CERT_EQ_FREE", 0u8) == 1,
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out)
        .unwrap();
    for owner in library.owners() {
        if owner.spec.lane != OwnerLane::Scheme {
            continue;
        }
        let step = SchemeStep::from_owner(&library, &owner.spec).unwrap();
        if step.template().name() != scheme {
            continue;
        }
        // CERT_DUMP_ROWS=n1,..,n14: the generator's ordinary IBP rows at that index point (presentation
        // slots), every coefficient as a polynomial in d, for an independent check of the row generator.
        if let Ok(point) = std::env::var("CERT_DUMP_ROWS") {
            let point: Vec<i64> = point.split(',').map(|x| x.parse().unwrap()).collect();
            let p = library.presentation(owner.spec.presentation);
            let generator = rustred::identity::ParametricIbpGenerator::try_new(&p.family).unwrap();
            let batch = generator.prepare_ordinary_ibp().unwrap();
            let rows = (0..batch.len()).map(|row| batch.generate(row)).collect();
            let relations = batch.complete(rows).unwrap().into_relations();
            let context = generator.context();
            let fixed: Vec<(usize, i64)> = point.iter().copied().enumerate().collect();
            let mut dump = Vec::new();
            for relation in &relations {
                let mut terms = Vec::new();
                for (shift, value) in relation.terms() {
                    let at = context
                        .specialize_fixed_indices(
                            value,
                            &fixed,
                            rustred::algebra::IndexedAlgebraLimits::unbounded_resources(),
                        )
                        .unwrap()
                        .0;
                    terms.push(serde_json::json!([
                        shift.values(),
                        at.to_expression().to_string()
                    ]));
                }
                dump.push(
                    serde_json::json!({"row": format!("{:?}", relation.row_id()), "terms": terms}),
                );
            }
            let slots: Vec<String> = p.slots.iter().map(|slot| format!("{slot:?}")).collect();
            let json = serde_json::json!({"scheme": scheme, "point": point, "forcer_slots": step.template().forcer_slots(), "slots": slots, "rows": dump});
            std::fs::write(&out, serde_json::to_string_pretty(&json).unwrap()).unwrap();
            return;
        }
        let names: Vec<String> = if which == "all" {
            uncertified_rules(&library, &owner.spec, options.redo).unwrap()
        } else {
            which.split(',').map(str::to_owned).collect()
        };
        eprintln!(
            "scheme {scheme}: {} rules, options {options:?}",
            names.len()
        );
        certify_rules(&library, &owner.spec, &names, &options, |outcome| {
            eprintln!(
                "  {:<14} {:<15} level {:?} frame {} x {} circuit {} weights deg n {} d {} multiplier {} poles {:?} {:.1} s {}",
                outcome.rule,
                outcome.status,
                outcome.level,
                outcome.frame_rows,
                outcome.frame_columns,
                outcome.circuit_rows,
                outcome.weight_degree_n,
                outcome.weight_degree_d,
                outcome.multiplier,
                outcome.pole_layers,
                outcome.seconds,
                outcome.detail
            );
            let mut line = serde_json::to_value(outcome).unwrap();
            line["scheme"] = scheme.clone().into();
            writeln!(file, "{}", serde_json::to_string(&line).unwrap()).unwrap();
            file.flush().unwrap();
        })
        .unwrap();
    }
}
