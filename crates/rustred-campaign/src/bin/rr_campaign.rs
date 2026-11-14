//! `rr_campaign`: the standalone campaign command (interfaces/supervisor.md section 12). Thin: everything is in
//! `rustred_campaign::cli`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(rustred_campaign::cli::main_with(&args));
}
