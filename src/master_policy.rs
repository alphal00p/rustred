//! Explicit user-selected and externally certified master terminals.
//!
//! Rule discovery is not allowed to infer that an uncovered integral is a
//! master.  This provider wrapper is the policy boundary at which a caller may
//! deliberately stop reduction at selected keys or bind a key to an external
//! master certificate.  Every other request is delegated unchanged to the
//! wrapped generic rule provider.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::ConcreteIntegralKey;
use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MasterPolicyLimits {
    pub max_terminals: usize,
    pub max_certificate_fingerprint_bytes: usize,
    pub max_total_certificate_fingerprint_bytes: usize,
}

impl Default for MasterPolicyLimits {
    fn default() -> Self {
        Self {
            max_terminals: 10_000_000,
            max_certificate_fingerprint_bytes: 64 * 1024,
            max_total_certificate_fingerprint_bytes: 64 * 1024 * 1024,
        }
    }
}

/// A policy terminal accepted by [`MasterPolicyProvider`].
///
/// There is intentionally no `Uncovered` variant: uncovered is a discovery
/// outcome, not a caller assertion about a master basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MasterPolicyTerminal {
    Selected,
    Certified { certificate_fingerprint: Arc<str> },
}

impl MasterPolicyTerminal {
    fn as_status(&self) -> ConcreteTerminalStatus {
        match self {
            Self::Selected => ConcreteTerminalStatus::SelectedMaster,
            Self::Certified {
                certificate_fingerprint,
            } => ConcreteTerminalStatus::CertifiedMaster {
                certificate_fingerprint: certificate_fingerprint.clone(),
            },
        }
    }
}

/// A topology-independent master policy layered over any concrete rule
/// provider.
pub struct MasterPolicyProvider<Provider> {
    inner: Provider,
    index_arity: usize,
    terminals: BTreeMap<ConcreteIntegralKey, MasterPolicyTerminal>,
    limits: MasterPolicyLimits,
    total_certificate_fingerprint_bytes: usize,
}

impl<Provider> MasterPolicyProvider<Provider>
where
    Provider: ConcreteRuleProvider,
{
    pub fn try_new(
        inner: Provider,
        terminals: impl IntoIterator<Item = (ConcreteIntegralKey, MasterPolicyTerminal)>,
        limits: MasterPolicyLimits,
    ) -> Result<Self, MasterPolicyError<Provider::Error>> {
        let index_arity = inner.index_arity();
        let mut result = Self {
            inner,
            index_arity,
            terminals: BTreeMap::new(),
            limits,
            total_certificate_fingerprint_bytes: 0,
        };
        for (integral, terminal) in terminals {
            result.insert_terminal(integral, terminal)?;
        }
        Ok(result)
    }

    pub fn with_selected(
        inner: Provider,
        selected: impl IntoIterator<Item = ConcreteIntegralKey>,
    ) -> Result<Self, MasterPolicyError<Provider::Error>> {
        Self::try_new(
            inner,
            selected
                .into_iter()
                .map(|key| (key, MasterPolicyTerminal::Selected)),
            MasterPolicyLimits::default(),
        )
    }

    pub const fn limits(&self) -> MasterPolicyLimits {
        self.limits
    }

    pub fn terminals(&self) -> &BTreeMap<ConcreteIntegralKey, MasterPolicyTerminal> {
        &self.terminals
    }

    pub const fn total_certificate_fingerprint_bytes(&self) -> usize {
        self.total_certificate_fingerprint_bytes
    }

    pub const fn inner(&self) -> &Provider {
        &self.inner
    }

    /// Mutating a wrapped provider through an engine must be done through
    /// `ParametricReductionEngine::provider_mut`, which invalidates its cache.
    pub fn inner_mut(&mut self) -> &mut Provider {
        &mut self.inner
    }

    pub fn into_inner(self) -> Provider {
        self.inner
    }

    pub fn insert_terminal(
        &mut self,
        integral: ConcreteIntegralKey,
        terminal: MasterPolicyTerminal,
    ) -> Result<(), MasterPolicyError<Provider::Error>> {
        self.validate_inner_arity()?;
        self.validate_key(&integral)?;
        validate_terminal::<Provider::Error>(&terminal, self.limits)?;

        if let Some(existing) = self.terminals.get(&integral) {
            return if existing == &terminal {
                Ok(())
            } else {
                Err(MasterPolicyError::ConflictingTerminal { integral })
            };
        }

        let requested = self.terminals.len().checked_add(1).ok_or(
            MasterPolicyError::ResourceCountOverflow {
                resource: "master policy terminals",
            },
        )?;
        check_limit::<Provider::Error>(
            "master policy terminals",
            requested,
            self.limits.max_terminals,
        )?;
        let additional_bytes = certificate_bytes(&terminal);
        let total_bytes = self
            .total_certificate_fingerprint_bytes
            .checked_add(additional_bytes)
            .ok_or(MasterPolicyError::ResourceCountOverflow {
                resource: "master certificate fingerprint bytes",
            })?;
        check_limit::<Provider::Error>(
            "master certificate fingerprint bytes",
            total_bytes,
            self.limits.max_total_certificate_fingerprint_bytes,
        )?;
        self.terminals.insert(integral, terminal);
        self.total_certificate_fingerprint_bytes = total_bytes;
        Ok(())
    }

    pub fn remove_terminal(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<bool, MasterPolicyError<Provider::Error>> {
        let Some(removed) = self.terminals.remove(integral) else {
            return Ok(false);
        };
        self.total_certificate_fingerprint_bytes = self
            .total_certificate_fingerprint_bytes
            .checked_sub(certificate_bytes(&removed))
            .ok_or(MasterPolicyError::ResourceCountOverflow {
                resource: "master certificate fingerprint byte accounting",
            })?;
        Ok(true)
    }

    fn validate_inner_arity(&self) -> Result<(), MasterPolicyError<Provider::Error>> {
        let actual = self.inner.index_arity();
        if actual == self.index_arity {
            Ok(())
        } else {
            Err(MasterPolicyError::ProviderArityChanged {
                expected: self.index_arity,
                actual,
            })
        }
    }

    fn validate_key(
        &self,
        integral: &ConcreteIntegralKey,
    ) -> Result<(), MasterPolicyError<Provider::Error>> {
        if integral.powers().len() == self.index_arity {
            Ok(())
        } else {
            Err(MasterPolicyError::WrongArity {
                expected: self.index_arity,
                actual: integral.powers().len(),
            })
        }
    }
}

impl<Provider> ConcreteRuleProvider for MasterPolicyProvider<Provider>
where
    Provider: ConcreteRuleProvider,
{
    type Error = MasterPolicyError<Provider::Error>;

    fn index_arity(&self) -> usize {
        self.index_arity
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.validate_inner_arity()?;
        self.validate_key(integral)?;
        if let Some(terminal) = self.terminals.get(integral) {
            return Ok(ConcreteRuleDecision::Terminal(terminal.as_status()));
        }
        self.inner
            .decision_for(integral)
            .map_err(MasterPolicyError::Inner)
    }
}

fn certificate_bytes(terminal: &MasterPolicyTerminal) -> usize {
    match terminal {
        MasterPolicyTerminal::Selected => 0,
        MasterPolicyTerminal::Certified {
            certificate_fingerprint,
        } => certificate_fingerprint.len(),
    }
}

fn validate_terminal<ProviderError>(
    terminal: &MasterPolicyTerminal,
    limits: MasterPolicyLimits,
) -> Result<(), MasterPolicyError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    let MasterPolicyTerminal::Certified {
        certificate_fingerprint,
    } = terminal
    else {
        return Ok(());
    };
    if certificate_fingerprint.is_empty() {
        return Err(MasterPolicyError::EmptyCertificateFingerprint);
    }
    check_limit(
        "one master certificate fingerprint bytes",
        certificate_fingerprint.len(),
        limits.max_certificate_fingerprint_bytes,
    )
}

fn check_limit<ProviderError>(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), MasterPolicyError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    if requested > limit {
        Err(MasterPolicyError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub enum MasterPolicyError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    Inner(ProviderError),
    WrongArity {
        expected: usize,
        actual: usize,
    },
    ProviderArityChanged {
        expected: usize,
        actual: usize,
    },
    EmptyCertificateFingerprint,
    ConflictingTerminal {
        integral: ConcreteIntegralKey,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}

impl<ProviderError> fmt::Display for MasterPolicyError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inner(error) => write!(formatter, "wrapped rule provider failed: {error}"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "master policy key has arity {actual}; expected {expected}"
            ),
            Self::ProviderArityChanged { expected, actual } => write!(
                formatter,
                "wrapped rule provider arity changed from {expected} to {actual}"
            ),
            Self::EmptyCertificateFingerprint => {
                formatter.write_str("a certified master fingerprint cannot be empty")
            }
            Self::ConflictingTerminal { integral } => write!(
                formatter,
                "master policy has conflicting classifications for {integral:?}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
        }
    }
}

impl<ProviderError> std::error::Error for MasterPolicyError<ProviderError> where
    ProviderError: std::error::Error + Send + Sync + 'static
{
}
