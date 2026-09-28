//! census: read-only work-volume census of a RustRed CP5 walk checkpoint
//! (plan `docs/research/fable51_next_push_master_plan_2026-09-27.md` W0.7).
//!
//! Every subcommand reads a checkpoint directory (never writes into it) and
//! prints JSON lines / tables to stdout. See `tools/research/census/README.md`.
mod ckpt;
mod compose;
mod cover;
mod geom;
mod potential;
mod recs;
mod route;
mod util;

use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "usage: census <stats|compose|cost|potential|cover|saturation|pilot|route|route-saturation|route-apply|route-hits> CKPT_DIR [options]\n\
         see tools/research/census/README.md"
    );
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        usage();
    }
    let mode = args[1].as_str();
    let dir = PathBuf::from(&args[2]);
    let opts = util::Opts::parse(&args[3..]);
    let t0 = std::time::Instant::now();
    match mode {
        "stats" => compose::stats(&dir, &opts),
        "compose" => compose::compose(&dir, &opts),
        "cost" => compose::cost(&dir, &opts),
        "potential" => potential::run(&dir, &opts),
        "cover" => cover::run(&dir, &opts),
        "pilot" => cover::pilot(&dir, &opts),
        "saturation" => cover::saturation(&dir, &opts),
        "route" => route::run(&dir, &opts),
        "route-saturation" => route::saturation(&dir, &opts),
        "route-apply" => route::apply_natives(&dir, &opts),
        "route-hits" => route::hits(&dir, &opts),
        _ => usage(),
    }
    eprintln!("census {mode}: {:.1} s", t0.elapsed().as_secs_f64());
}
