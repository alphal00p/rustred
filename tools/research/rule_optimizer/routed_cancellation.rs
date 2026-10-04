//! Research weighted routing; optional bounded same-owner plateau substitutions.
mod applied_observer {
    pub mod input;
    pub mod record;
}
#[path = "routed_cancellation/mod.rs"]
mod routed_cancellation;

fn main() {
    if let Err(error) = routed_cancellation::run() {
        eprintln!("{}", serde_json::json!({"complete":false,"error":error}));
        std::process::exit(2);
    }
}
