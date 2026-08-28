use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IbpSourceLayout {
    CompleteOrdinary,
    ExternalOnly,
}

impl IbpSourceLayout {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::CompleteOrdinary => "ordinary IBP source",
            Self::ExternalOnly => "external-contraction IBP source",
        }
    }

    pub(super) const fn source_offset(self, loops: usize) -> Option<usize> {
        match self {
            Self::CompleteOrdinary => loops.checked_mul(loops),
            Self::ExternalOnly => Some(0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IbpSourceScope {
    pub(super) family_fingerprint: Arc<str>,
    pub(super) context_fingerprint: Arc<str>,
}
