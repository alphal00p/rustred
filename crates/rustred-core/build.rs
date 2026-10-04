//! Generate only the runtime monomorphizations explicitly selected at build time.
use std::{collections::BTreeSet, env, fs, path::PathBuf};

const VARIABLE: &str = "RUSTRED_RUNTIME_ARITIES";

fn parse_arities(value: Option<&str>) -> Result<Vec<usize>, String> {
    let Some(value) = value else {
        return Ok((1..=16).collect());
    };
    let mut arities = BTreeSet::new();
    for item in value.split(',') {
        let item = item.trim();
        if item.is_empty() || !item.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!(
                "{VARIABLE} must be a comma-separated list of positive integers"
            ));
        }
        let arity = item
            .parse::<usize>()
            .map_err(|_| format!("invalid arity {item:?} in {VARIABLE}"))?;
        if arity == 0 || !arities.insert(arity) {
            return Err(format!(
                "{VARIABLE} contains a zero or duplicate arity: {arity}"
            ));
        }
    }
    Ok(arities.into_iter().collect())
}

fn render(arities: &[usize]) -> String {
    let arms = arities
        .iter()
        .map(|n| format!("            {n} => $function::<{n}> $arguments,\n"))
        .collect::<String>();
    let template = r#"
pub(super) const COMPILED_RUNTIME_ARITIES: &[usize] = &__ARITIES__;

/// Dispatch a runtime arity to a caller's const-generic function.
///
/// The default form uses [`crate::compiled_runtime_arities`]. Supply a literal
/// list to select your own monomorphizations independently of that registry.
/// Import a function from another module with `use` before passing its name.
/// The arity is evaluated once; unsupported arities take the caller's fallback.
/// No solver policy, budget, validation, or error type is imposed by this macro.
///
/// ```
/// fn slots<const N: usize>(extra: usize) -> usize { N + extra }
/// assert_eq!(rustred::dispatch_arity!(14, [1, 14, 20], slots(2), n => n), 16);
/// let n = rustred::compiled_runtime_arities()[0];
/// assert_eq!(rustred::dispatch_arity!(n, slots(0), _n => 0), n);
/// ```
#[macro_export]
macro_rules! dispatch_arity {
    ($arity:expr, [$($n:literal),+ $(,)?], $function:ident $arguments:tt, $unsupported:ident => $fallback:expr $(,)?) => {{
        const _: () = { $(assert!($n > 0, "dispatch arities must be positive");)+ };
        match $arity {
            $($n => $function::<$n> $arguments,)+
            $unsupported => $fallback,
        }
    }};
    ($arity:expr, $function:ident $arguments:tt, $unsupported:ident => $fallback:expr $(,)?) => {{
        match $arity {
__ARMS__            $unsupported => $fallback,
        }
    }};
}
"#;
    template
        .replace("__ARITIES__", &format!("{arities:?}"))
        .replace("__ARMS__", &arms)
}

fn main() {
    println!("cargo:rerun-if-env-changed={VARIABLE}");
    let value = match env::var(VARIABLE) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(error) => panic!("invalid {VARIABLE}: {error}"),
    };
    let arities = parse_arities(value.as_deref()).unwrap_or_else(|error| panic!("{error}"));
    let destination = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::write(destination.join("runtime_arities.rs"), render(&arities))
        .expect("write runtime arity registry");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_is_explicit_and_bounded() {
        assert_eq!(parse_arities(None).unwrap(), (1..=16).collect::<Vec<_>>());
    }

    #[test]
    fn custom_registry_can_omit_defaults_and_extend_them() {
        assert_eq!(parse_arities(Some(" 20,14,1 ")).unwrap(), [1, 14, 20]);
        assert_eq!(parse_arities(Some("15")).unwrap(), [15]);
        let source = render(&[13, 17]);
        assert!(source.contains("&[13, 17]"));
        assert!(source.contains("$function::<17>"));
        assert!(!source.contains("$function::<16>"));
    }

    #[test]
    fn invalid_configuration_fails_instead_of_silently_using_defaults() {
        for value in [
            "",
            "0",
            "1,1",
            "1,,2",
            "1,",
            "-1",
            "1..16",
            "abc",
            "184467440737095516160",
        ] {
            assert!(parse_arities(Some(value)).is_err(), "{value}");
        }
    }
}
