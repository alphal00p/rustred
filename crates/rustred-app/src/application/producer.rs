use serde::Serialize;

/// Shared producer block for every canonical RustRed application document.
///
/// Keeping this DTO in the application layer prevents frontends from
/// inventing producer metadata or output-schema variants of their own.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ProducerOutputV1 {
    pub(super) name: &'static str,
    pub(super) rustred_version: &'static str,
    pub(super) symbolica_version: &'static str,
    pub(super) expression_format: &'static str,
}

impl ProducerOutputV1 {
    pub(super) fn current() -> Self {
        Self {
            name: "RustRed",
            rustred_version: env!("CARGO_PKG_VERSION"),
            symbolica_version: rustred::runtime::symbolica_runtime_version(),
            expression_format: "rustred.symbolica-canonical-string.v1",
        }
    }
}
