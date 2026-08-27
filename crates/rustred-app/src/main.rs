fn main() {
    // This executes before RustRed initializes Symbolica or starts worker
    // threads. Suppressing Symbolica's informational banner is required by
    // the CLI contract that successful stdout contains only TOML.
    if std::env::var_os("SYMBOLICA_HIDE_BANNER").is_none() {
        // SAFETY: `main` has not spawned threads and no RustRed/Symbolica code
        // has run, so no concurrent environment access exists here.
        unsafe { std::env::set_var("SYMBOLICA_HIDE_BANNER", "1") };
    }
    std::process::exit(rustred_app::cli_main_entry());
}
