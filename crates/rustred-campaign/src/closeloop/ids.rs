//! Identifiers and file locations of the close-from-material source (interfaces/closeloop.md 3). Item and job ids use
//! the contract charset `[A-Za-z0-9._-]`; input ids may also use `:`.

/// A name component in the contract charset: `[A-Za-z0-9.-]` kept, everything else `_`.
pub fn safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn import_item(n: &str) -> String {
    format!("{n}.import")
}

/// Prefix of the solve items of round `k`.
pub fn round_prefix(n: &str, k: u32) -> String {
    format!("{n}.s{k}.")
}

pub fn solve_item(n: &str, k: u32, j: usize) -> String {
    format!("{n}.s{k}.{j}")
}

/// (round, batch index) of a solve item of node `n`.
pub fn parse_solve_item(n: &str, item: &str) -> Option<(u32, usize)> {
    let rest = item.strip_prefix(n)?.strip_prefix(".s")?;
    let (k, j) = rest.split_once('.')?;
    Some((k.parse().ok()?, j.parse().ok()?))
}

pub fn input_import_spec(n: &str) -> String {
    format!("closeloop:{n}:import_spec")
}
pub fn input_box(n: &str) -> String {
    format!("closeloop:{n}:box")
}
pub fn input_check(n: &str, k: u32) -> String {
    format!("closeloop:{n}:check:r{k}")
}
pub fn input_check_prefix(n: &str) -> String {
    format!("closeloop:{n}:check:r")
}
pub fn input_batch(item: &str) -> String {
    format!("closeloop:{item}:batch")
}
pub fn input_answers(item: &str, job: &str) -> String {
    format!("closeloop:{item}:answers:{job}")
}
pub fn input_answers_prefix(n: &str) -> String {
    format!("closeloop:{n}.s")
}
pub fn input_final(n: &str) -> String {
    format!("closeloop:{n}:final")
}

pub const ROLE_SPEC: &str = "table_set_spec";
pub const ROLE_BOX: &str = "box";
pub const ROLE_CHECK: &str = "closeloop_check";
pub const ROLE_BATCH: &str = "closeloop_batch";
pub const ROLE_ANSWERS: &str = "closeloop_answers";
pub const ROLE_RESULT: &str = "closeloop_result";

pub const KIND_IMPORT: &str = "import";
pub const KIND_SOLVE: &str = "recipe-solve";

/// CDIR-relative directory of a node's files.
pub fn node_dir(n: &str) -> String {
    format!("closeloop/{n}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        assert_eq!(safe("T 1/x"), "T_1_x");
        let it = solve_item("N0", 3, 12);
        assert_eq!(parse_solve_item("N0", &it), Some((3, 12)));
        assert_eq!(parse_solve_item("N0", "N0.import"), None);
        assert_eq!(parse_solve_item("N", "N0.s1.2"), None);
    }
}
