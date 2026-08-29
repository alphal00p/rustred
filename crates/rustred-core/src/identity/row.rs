use std::fmt;
use std::sync::Arc;

/// Stable source identity of a generated relation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RowId {
    OrdinaryIbp {
        /// Loops first, then external momenta.
        contraction_momentum: usize,
        differentiated_loop: usize,
    },
    LorentzInvariance {
        first_external: usize,
        second_external: usize,
    },
    Derived {
        label: Arc<str>,
    },
}

impl RowId {
    /// Version-stable identity used in user-facing output and proof payloads.
    pub fn stable_string(&self) -> String {
        let mut output = String::new();
        self.write_stable(&mut output)
            .expect("writing row identity to String cannot fail");
        output
    }

    pub(in crate::identity) fn write_stable(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::OrdinaryIbp {
                contraction_momentum,
                differentiated_loop,
            } => write!(
                writer,
                "ordinary-ibp:{contraction_momentum}:{differentiated_loop}"
            ),
            Self::LorentzInvariance {
                first_external,
                second_external,
            } => write!(
                writer,
                "lorentz-invariance:{first_external}:{second_external}"
            ),
            Self::Derived { label } => write!(writer, "derived:{}:{label}", label.len()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_strings_pin_every_row_variant() {
        assert_eq!(
            RowId::OrdinaryIbp {
                contraction_momentum: 3,
                differentiated_loop: 2,
            }
            .stable_string(),
            "ordinary-ibp:3:2"
        );
        assert_eq!(
            RowId::LorentzInvariance {
                first_external: 1,
                second_external: 4,
            }
            .stable_string(),
            "lorentz-invariance:1:4"
        );
        assert_eq!(
            RowId::Derived {
                label: Arc::from("a:b"),
            }
            .stable_string(),
            "derived:3:a:b"
        );
    }
}
