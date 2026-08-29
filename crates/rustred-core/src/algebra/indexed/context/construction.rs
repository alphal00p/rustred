use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::prelude::*;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext, is_exact_plain_symbol};

use super::super::error::IndexedAlgebraError;
use super::super::limits::{IndexedContextLimits, check_limit};
use super::super::scope::{
    base_context_fingerprint_with_limit, indexed_context_fingerprint_segments_with_limit,
    preflight_qualified_index_symbols, qualified_index_symbol,
};

fn register_index_symbol(qualified: &str, position: usize) -> Result<Symbol, IndexedAlgebraError> {
    let namespaced = NamespacedSymbol::try_parse(qualified)
        .ok_or(IndexedAlgebraError::IndexSymbolRegistrationFailure { position })?;
    let symbol = SymbolBuilder::new(namespaced)
        .build()
        .map_err(|_| IndexedAlgebraError::IndexSymbolRegistrationFailure { position })?;
    super::authenticate_index_symbol(symbol, qualified, position)?;
    Ok(symbol)
}

/// Reject an existing process-global registration that could change the
/// algebraic or printed meaning of RustRed's private positional symbols.
/// Symbolica registrations are immutable, so this post-registration check is
/// also safe when contexts are constructed concurrently.
pub(in crate::algebra::indexed) fn authenticate_index_symbol(
    symbol: Symbol,
    qualified: &str,
    position: usize,
) -> Result<(), IndexedAlgebraError> {
    if !is_exact_plain_symbol(symbol, qualified) {
        Err(IndexedAlgebraError::IndexSymbolCollision { position })
    } else {
        Ok(())
    }
}

impl IndexedCoefficientContext {
    /// Extend `base` by `index_count` private index variables.
    ///
    /// `scope` is persisted exactly as part of the authenticated context
    /// identity. Native Symbolica names use a stable process-shared positional
    /// pool; the full scope remains the sole compatibility authority.
    pub fn try_new(
        base: &CoefficientContext,
        scope: &str,
        index_count: usize,
    ) -> Result<Self, IndexedAlgebraError> {
        Self::try_new_with_limits(base, scope, index_count, IndexedContextLimits::default())
    }

    /// Extend `base` under explicit construction resource limits.
    pub fn try_new_with_limits(
        base: &CoefficientContext,
        scope: &str,
        index_count: usize,
        limits: IndexedContextLimits,
    ) -> Result<Self, IndexedAlgebraError> {
        Self::try_new_with_scope_segments_and_limits(base, &[scope], index_count, limits)
    }

    /// Private zero-copy scope assembly for callers that already retain large
    /// semantic identity segments. The durable fingerprint stores their exact
    /// concatenation with the same length-delimited grammar as [`Self::try_new`].
    #[cfg(test)]
    pub(crate) fn try_new_with_scope_segments(
        base: &CoefficientContext,
        scope: &[&str],
        index_count: usize,
    ) -> Result<Self, IndexedAlgebraError> {
        Self::try_new_with_scope_segments_and_limits(
            base,
            scope,
            index_count,
            IndexedContextLimits::default(),
        )
    }

    pub(crate) fn try_new_with_scope_segments_and_limits(
        base: &CoefficientContext,
        scope: &[&str],
        index_count: usize,
        limits: IndexedContextLimits,
    ) -> Result<Self, IndexedAlgebraError> {
        if index_count == 0 {
            return Err(IndexedAlgebraError::EmptyIndexSpace);
        }
        if scope.iter().all(|segment| segment.is_empty()) {
            return Err(IndexedAlgebraError::InvalidScope);
        }

        let variable_count = base.variables().len().checked_add(index_count).ok_or(
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "indexed coefficient variables",
            },
        )?;
        check_limit(
            "indexed coefficient index variables",
            index_count,
            limits.max_index_variables,
        )?;
        let mut index_variables = Vec::new();
        index_variables
            .try_reserve_exact(index_count)
            .map_err(|_| IndexedAlgebraError::AllocationFailure {
                resource: "indexed coefficient index variables",
                requested: index_count,
            })?;
        let mut variables = Vec::new();
        variables.try_reserve_exact(variable_count).map_err(|_| {
            IndexedAlgebraError::AllocationFailure {
                resource: "indexed coefficient variables",
                requested: variable_count,
            }
        })?;
        preflight_qualified_index_symbols(index_count, limits.max_native_symbol_name_bytes)?;
        let base_fingerprint =
            base_context_fingerprint_with_limit(base, limits.max_fingerprint_bytes)?;
        let fingerprint = Arc::new(indexed_context_fingerprint_segments_with_limit(
            &base_fingerprint,
            scope,
            index_count,
            limits.max_fingerprint_bytes,
        )?);

        // RustRed has overflow-checked the complete private-name workload and
        // fallibly reserved every Rust-owned vector/string used here.
        // Symbolica's public API does not expose a capacity preflight for
        // NamespacedSymbol parsing or its global symbol interner; retain its
        // Option/Result errors, but do not claim that an unrelated probe or
        // catch_unwind can make those internal allocations fallible.
        for position in 0..index_count {
            let qualified = qualified_index_symbol(position)?;
            let symbol = register_index_symbol(&qualified, position)?;
            let variable = PolyVariable::Symbol(symbol);
            if base.variables().contains(&variable) {
                return Err(IndexedAlgebraError::IndexSymbolCollision { position });
            }
            index_variables.push(variable);
        }

        variables.extend(base.variables().iter().cloned());
        variables.extend(index_variables.iter().cloned());
        let variables = Arc::new(variables);
        // RationalPolynomial::new is likewise infallible in Symbolica's
        // public API and may initialize internal template state. All sizes
        // RustRed can truthfully preflight (the variable count and retained
        // Rust-owned containers) have already been checked and reserved.
        let template = RationalPolynomial::new(&Z, variables.clone());

        Ok(Self {
            base: base.clone(),
            fingerprint,
            variables,
            index_variables: Arc::new(index_variables),
            template,
            #[cfg(test)]
            authentication_counters: Arc::new(Default::default()),
        })
    }

    pub fn base(&self) -> &CoefficientContext {
        &self.base
    }

    pub fn fingerprint(&self) -> &str {
        self.fingerprint.as_str()
    }

    pub(crate) fn fingerprint_owner(&self) -> Arc<String> {
        self.fingerprint.clone()
    }

    pub fn index_count(&self) -> usize {
        self.index_variables.len()
    }
}
