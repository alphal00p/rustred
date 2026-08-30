use super::{StratumRegistryError, check_limit, checked_add};

/// Fallible byte-bounded builder for diagnostic identities retained by the
/// completion prototype. Mathematical authority remains in the sealed values
/// whose payload these identities commit to.
pub(super) struct BoundedIdentityBuilder {
    value: String,
    limit: usize,
    resource: &'static str,
}

impl BoundedIdentityBuilder {
    pub(super) fn new(limit: usize, resource: &'static str) -> Self {
        Self {
            value: String::new(),
            limit,
            resource,
        }
    }

    pub(super) fn push(&mut self, value: &str) -> Result<(), StratumRegistryError> {
        let requested = checked_add(self.resource, self.value.len(), value.len())?;
        check_limit(self.resource, requested, self.limit)?;
        self.value.try_reserve_exact(value.len()).map_err(|_| {
            StratumRegistryError::AllocationFailure {
                resource: self.resource,
                requested,
            }
        })?;
        self.value.push_str(value);
        Ok(())
    }

    pub(super) fn push_usize(&mut self, value: usize) -> Result<(), StratumRegistryError> {
        self.push(&value.to_string())
    }

    pub(super) fn push_i64(&mut self, value: i64) -> Result<(), StratumRegistryError> {
        self.push(&value.to_string())
    }

    pub(super) fn finish(self) -> String {
        self.value
    }
}

pub(super) fn try_copy_identity(
    value: &str,
    resource: &'static str,
    limit: usize,
) -> Result<String, StratumRegistryError> {
    check_limit(resource, value.len(), limit)?;
    let mut retained = String::new();
    retained.try_reserve_exact(value.len()).map_err(|_| {
        StratumRegistryError::AllocationFailure {
            resource,
            requested: value.len(),
        }
    })?;
    retained.push_str(value);
    Ok(retained)
}
