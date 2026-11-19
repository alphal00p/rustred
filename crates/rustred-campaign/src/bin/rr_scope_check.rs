//! `rr_scope_check`: procfix7 round 11 (gate14 item 5) THE SAFE HOST CHECK of the scope belt's `cgroup.kill` path, for the
//! coordinator (`rustred_campaign::launch::host_check`: what it does, its refusals, its one write). Usage:
//!   rr_scope_check --unit gate-scopecheck-<16 hex> [--run]
//! Without `--run` it checks its preconditions and prints the plan; nothing is started. Exit 0: every step held (or the
//! dry run's preconditions); 1: a refusal or a failed step (the report says which); 2: usage.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut unit = None;
    let mut run = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--unit" => unit = it.next().cloned(),
            "--run" => run = true,
            _ => {
                eprintln!("usage: rr_scope_check --unit gate-scopecheck-<16 hex> [--run]");
                std::process::exit(2);
            }
        }
    }
    let Some(unit) = unit else {
        eprintln!("usage: rr_scope_check --unit gate-scopecheck-<16 hex> [--run]");
        std::process::exit(2);
    };
    let r = rustred_campaign::launch::host_check::host_scope_check(&unit, run, &mut |l| {
        println!("{l}");
    });
    match r {
        Ok(()) => {
            println!("RESULT: PASS{}", if run { "" } else { " (dry run)" });
            std::process::exit(0);
        }
        Err(e) => {
            println!("RESULT: FAIL: {e}");
            std::process::exit(1);
        }
    }
}
