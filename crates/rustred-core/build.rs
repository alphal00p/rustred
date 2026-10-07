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

fn render_dispatch(arities: &[usize], capacities: &[usize]) -> String {
    let arms = arities
        .iter()
        .map(|n| format!("            {n} => $function::<{n}> $arguments,\n"))
        .collect::<String>();
    let capacity_arms = capacities.iter().map(|n| format!(
        "            arity if arity > 0 && arity <= {n} && [{allowed}].contains(&arity) => $function::<{n}> $arguments,\n",
        allowed = arities.iter().map(usize::to_string).collect::<Vec<_>>().join(",")
    )).collect::<String>();
    let template = r#"
pub(super) const COMPILED_RUNTIME_ARITIES: &[usize] = &__ARITIES__;
pub(super) const COMPILED_RUNTIME_CAPACITIES: &[usize] = &__CAPACITIES__;

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
/// Dispatch to the smallest compiled solver storage capacity. Callback code
/// must distinguish the physical arity from its const-generic capacity.
#[macro_export]
macro_rules! dispatch_solver_capacity {
    ($arity:expr, $function:ident $arguments:tt, $unsupported:ident => $fallback:expr $(,)?) => {{
        match $arity {
__CAPACITY_ARMS__            $unsupported => $fallback,
        }
    }};
}
/// Invoke a caller macro with campaign storage widths (at most 16).
/// With capacity-dispatch these are only 4, 8 and 16. Callers must admit
/// physical inputs separately and select campaign_storage_arity before matching.
/// Empty registries are possible when only larger finite-reducer arities are
/// selected. The callback must accept zero or more literals.
#[macro_export]
macro_rules! with_app_runtime_arities {
    ($callback:ident) => { $callback!(__APP_ARITIES__) };
}

"#;
    template
        .replace("__ARITIES__", &format!("{arities:?}"))
        .replace("__ARMS__", &arms)
        .replace("__CAPACITIES__", &format!("{capacities:?}"))
        .replace("__CAPACITY_ARMS__", &capacity_arms)
        .replace(
            "__APP_ARITIES__",
            &capacities
                .iter()
                .filter(|&&arity| arity <= 16)
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        )
}

fn main() {
    println!("cargo:rerun-if-env-changed={VARIABLE}");
    let value = match env::var(VARIABLE) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(error) => panic!("invalid {VARIABLE}: {error}"),
    };
    if value.is_some() && env::var_os("CARGO_FEATURE_RUNTIME_ARITY_SELECTION").is_none() {
        panic!("RUSTRED_RUNTIME_ARITIES requires the runtime-arity-selection feature");
    }
    let arities = parse_arities(value.as_deref()).unwrap_or_else(|error| panic!("{error}"));
    let capacities = if env::var_os("CARGO_FEATURE_CAPACITY_DISPATCH").is_some() {
        arities
            .iter()
            .map(|&arity| match arity {
                1..=4 => 4,
                5..=8 => 8,
                9..=16 => 16,
                other => other,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        arities.clone()
    };
    let destination = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::write(
        destination.join("runtime_arities.rs"),
        render_dispatch(&arities, &capacities),
    )
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
        let source = render_dispatch(&[13, 17], &[13, 17]);
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
