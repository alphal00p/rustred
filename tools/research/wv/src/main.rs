//! wv: read-only W0.11 work-volume analyses of RustRed walk checkpoints
//! (plan `docs/research/fable51_next_push_master_plan_2026-09-27.md` W0.11).
//!
//! `ckpt.rs`, `geom.rs`, `recs.rs` and `util.rs` are the CP5 decoders of the
//! W0.7 census crate (`tools/research/census`, branch fable_5_1-v3-census,
//! commit 7d6dd2f1), copied unchanged. Nothing is ever written into a
//! checkpoint. See `tools/research/wv/README.md`.
#![allow(dead_code)]
mod ckpt;
mod geom;
mod kd;
mod pieces;
mod recs;
mod sym;
mod tight;
mod util;
mod volume;

use std::path::PathBuf;

fn usage() -> ! {
    eprintln!("usage: wv <sym|volume|pieces> CKPT_DIR [options]; see tools/research/wv/README.md");
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
        "sym" => sym::run(&dir, &opts),
        "pieces" => pieces::run(&dir, &opts),
        "volume" => volume::run(&dir, &opts),
        _ => usage(),
    }
    eprintln!("wv {mode}: {:.1} s", t0.elapsed().as_secs_f64());
}
