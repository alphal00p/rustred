use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::symmetry::Canonicalizer;

use super::{ClosedArtifact, FactorizationRule, ZeroSectorTerminal};

/// Immutable intentional-terminal policy for same-family numerical masters.
///
/// These points are not claimed to follow from a factorization rule. They are
/// explicitly selected as finite terminal values to be supplied by a later
/// evaluation layer. Installation binds the sorted canonical representatives
/// to one family and indexed coefficient context; symmetry aliases are routed
/// by the authority's authenticated canonicalizer.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DeclaredMasterManifest {
    arity: usize,
    family_fingerprint: Arc<String>,
    context_fingerprint: Arc<String>,
    terminals: BTreeSet<IntegralKey>,
}

impl DeclaredMasterManifest {
    pub(super) fn from_validated_parts(
        arity: usize,
        family_fingerprint: Arc<String>,
        context_fingerprint: Arc<String>,
        terminals: BTreeSet<IntegralKey>,
    ) -> Self {
        Self {
            arity,
            family_fingerprint,
            context_fingerprint,
            terminals,
        }
    }

    pub(crate) const fn arity(&self) -> usize {
        self.arity
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) fn terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.terminals
    }
}

/// Immutable proof authority for terminalizing regions of one family.
///
/// Unlike a [`ClosedArtifact`], this owner makes no claim that rules cover the
/// surrounding integral lattice. Construction is nevertheless a production
/// seal: every zero proof and factorization is replayed once by the same
/// generic installers used for complete artifacts, while intentional master
/// points are symmetry-canonicalized and checked against every zero and
/// factorization terminal. Callers retain this value through an [`Arc`] and
/// perform only domain lookup after installation.
#[derive(Debug)]
#[allow(dead_code)] // K = 6 consumes the complete authority during Stage 1 publication.
pub(crate) struct ClosedTerminalAuthority {
    // Human-reviewable revision label. It is never treated as a content hash:
    // immutable snapshots bind this label together with the complete ordered
    // terminal payload, family, and coefficient-context fingerprints.
    authority_id: &'static str,
    pub(super) arity: usize,
    pub(super) family: IntegralFamily,
    family_fingerprint: Arc<String>,
    pub(super) context: IndexedCoefficientContext,
    pub(super) canonicalizer: Option<Canonicalizer>,
    pub(super) dependencies: Vec<Box<ClosedArtifact>>,
    pub(super) factorization_rules: Vec<FactorizationRule>,
    pub(super) parent_terminals: BTreeSet<IntegralKey>,
    pub(super) declared_masters: DeclaredMasterManifest,
    pub(super) zero_sectors: Vec<ZeroSectorTerminal>,
}

/// Owned, already authenticated terminal payload transferred into a complete
/// closing artifact installer. Search configuration and campaign provenance
/// are intentionally absent from this type.
pub(super) struct TerminalArtifactParts {
    pub(super) arity: usize,
    pub(super) family: IntegralFamily,
    pub(super) context: IndexedCoefficientContext,
    pub(super) canonicalizer: Option<Canonicalizer>,
    pub(super) dependencies: Vec<Box<ClosedArtifact>>,
    pub(super) factorization_rules: Vec<FactorizationRule>,
    pub(super) masters: BTreeSet<IntegralKey>,
    pub(super) zero_sectors: Vec<ZeroSectorTerminal>,
}

#[allow(dead_code)] // K = 6 consumes the complete authority during Stage 1 publication.
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
        declared_masters: DeclaredMasterManifest,
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
            declared_masters,
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

    pub(crate) const fn declared_master_manifest(&self) -> &DeclaredMasterManifest {
        &self.declared_masters
    }

    pub(crate) fn master_terminal_count(&self) -> usize {
        self.parent_terminals.len() + self.declared_masters.terminals().len()
    }

    /// Stable factorization-image terminals followed by stable intentional
    /// master representatives. Installation proves the two sets disjoint.
    pub(crate) fn master_terminals(
        &self,
    ) -> impl Iterator<Item = &IntegralKey> + DoubleEndedIterator {
        self.parent_terminals
            .iter()
            .chain(self.declared_masters.terminals().iter())
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

    /// Consume the terminal-only seal into the generic artifact payload after
    /// a separate ordinary-rule closure proof has been retained. Both the
    /// factorization-image terminals and intentional numerical masters become
    /// exact terminal keys of the resulting reducer artifact.
    pub(super) fn into_artifact_parts(self) -> TerminalArtifactParts {
        let mut masters = self.parent_terminals;
        masters.extend(self.declared_masters.terminals);
        TerminalArtifactParts {
            arity: self.arity,
            family: self.family,
            context: self.context,
            canonicalizer: self.canonicalizer,
            dependencies: self.dependencies,
            factorization_rules: self.factorization_rules,
            masters,
            zero_sectors: self.zero_sectors,
        }
    }
}
