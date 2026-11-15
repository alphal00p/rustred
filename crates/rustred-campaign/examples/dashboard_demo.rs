//! Demo host of the campaign dashboard (lane I-TUI). The implementation is shared with the pseudo-terminal test:
//! `tests/tui_support/demo.rs` (usage there; `dashboard_demo --fixture midrun`, `dashboard_demo CDIR`).

#[path = "../tests/tui_support/demo.rs"]
mod demo;

fn main() {
    std::process::exit(demo::main_with(std::env::args().skip(1).collect()));
}
