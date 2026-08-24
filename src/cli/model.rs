use std::collections::BTreeMap;

use rustred::LoweredSymbolicaProjectV1;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum MetadataValue {
    String(String),
    StringArray(Vec<String>),
}

/// CLI-owned view of the parser/lowering boundary.
///
/// `backend.rs` is the only module allowed to translate the public
/// `symbolica_integral_input` API into this view.  Keeping the rest of the CLI
/// on this small DTO prevents it from growing a second expression grammar or
/// duplicating affine lowering.
#[derive(Clone, Debug)]
pub(crate) struct LoweredCliProject {
    input_form: &'static str,
    input_schema: String,
    metadata: BTreeMap<String, MetadataValue>,
    lowered: LoweredSymbolicaProjectV1,
}

impl LoweredCliProject {
    pub(crate) fn new(
        input_form: &'static str,
        input_schema: String,
        metadata: BTreeMap<String, MetadataValue>,
        lowered: LoweredSymbolicaProjectV1,
    ) -> Self {
        Self {
            input_form,
            input_schema,
            metadata,
            lowered,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        &'static str,
        String,
        BTreeMap<String, MetadataValue>,
        LoweredSymbolicaProjectV1,
    ) {
        (
            self.input_form,
            self.input_schema,
            self.metadata,
            self.lowered,
        )
    }
}
