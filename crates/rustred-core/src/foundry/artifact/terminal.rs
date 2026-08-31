use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::symmetry::Canonicalizer;

use super::{ClosedArtifact, FactorizationRule, ZeroSectorTerminal};

/// Immutable proof authority for terminalizing regions of one family.
///
/// Unlike a [`ClosedArtifact`], this owner makes no claim that rules cover the
/// surrounding integral lattice. Construction is nevertheless a production
/// seal: every zero proof and factorization is replayed once by the same
/// generic installers used for complete artifacts. Callers retain this value
/// through an [`Arc`] and perform only domain lookup after installation.
#[derive(Debug)]
pub(crate) struct ClosedTerminalAuthority {
    // Human-reviewable revision label. It is never treated as a content hash:
    // immutable snapshots bind this label together with the complete ordered
    // terminal payload, family, and coefficient-context fingerprints.
    authority_id: &'static str,
    arity: usize,
    family: IntegralFamily,
    family_fingerprint: Arc<String>,
    context: IndexedCoefficientContext,
    canonicalizer: Option<Canonicalizer>,
    dependencies: Vec<Box<ClosedArtifact>>,
    factorization_rules: Vec<FactorizationRule>,
    parent_terminals: BTreeSet<IntegralKey>,
    zero_sectors: Vec<ZeroSectorTerminal>,
}

impl ClosedTerminalAuthority {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_validated_parts(
        authority_id: &'static str,
        arity: usize,
        family: IntegralFamily,
        context: IndexedCoefficientContext,
        canonicalizer: Option<Canonicalizer>,
        dependencies: Vec<Box<ClosedArtifact>>,
        factorization_rules: Vec<FactorizationRule>,
        parent_terminals: BTreeSet<IntegralKey>,
        zero_sectors: Vec<ZeroSectorTerminal>,
    ) -> Self {
        let family_fingerprint = family.fingerprint_owner();
        Self {
            authority_id,
            arity,
            family,
            family_fingerprint,
            context,
            canonicalizer,
            dependencies,
            factorization_rules,
            parent_terminals,
            zero_sectors,
        }
    }

    pub(crate) const fn authority_id(&self) -> &'static str {
        self.authority_id
    }

    pub(crate) const fn arity(&self) -> usize {
        self.arity
    }

    pub(crate) fn family(&self) -> &IntegralFamily {
        &self.family
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(crate) fn context(&self) -> &IndexedCoefficientContext {
        &self.context
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context.fingerprint()
    }

    pub(crate) fn canonicalizer(&self) -> Option<&Canonicalizer> {
        self.canonicalizer.as_ref()
    }

    pub(crate) fn dependencies(&self) -> &[Box<ClosedArtifact>] {
        &self.dependencies
    }

    pub(crate) fn factorization_rules(&self) -> &[FactorizationRule] {
        &self.factorization_rules
    }

    pub(crate) fn parent_terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.parent_terminals
    }

    pub(crate) fn zero_sectors(&self) -> &[ZeroSectorTerminal] {
        &self.zero_sectors
    }

    pub(crate) fn is_zero_terminal(&self, key: &IntegralKey) -> bool {
        key.powers().len() == self.arity
            && self.zero_sectors.iter().any(|terminal| {
                terminal
                    .sector()
                    .active_bits()
                    .iter()
                    .zip(key.powers())
                    .all(|(&active, &power)| active == (power >= 1))
            })
    }
}
