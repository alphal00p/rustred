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

/// lane closeloop2: proof (span) and fact items of round `k`; lane closeloop3: `exact` (coefficient-facts) items `x`.
pub fn proof_item(n: &str, k: u32, j: usize, kind: &str) -> String {
    let tag = match kind {
        "facts" => "f",
        "exact" => "x",
        _ => "p",
    };
    format!("{n}.{tag}{k}.{j}")
}

/// (round, batch index) of a proof / fact item of node `n`.
pub fn parse_proof_item(n: &str, item: &str) -> Option<(u32, usize)> {
    let rest = item.strip_prefix(n)?.strip_prefix('.')?;
    let rest = rest
        .strip_prefix('p')
        .or_else(|| rest.strip_prefix('f'))
        .or_else(|| rest.strip_prefix('x'))?;
    let (k, j) = rest.split_once('.')?;
    Some((k.parse().ok()?, j.parse().ok()?))
}

pub fn input_pbatch(item: &str) -> String {
    format!("closeloop:{item}:pbatch")
}
pub fn input_proofs(item: &str, job: &str) -> String {
    format!("closeloop:{item}:proofs:{job}")
}
pub fn input_facts(item: &str, job: &str) -> String {
    format!("closeloop:{item}:facts:{job}")
}
/// lane closeloop3: the ERROR / IRREGULAR keys of a solve job (R48-3b records; retried, then attention).
pub fn input_retry(item: &str, job: &str) -> String {
    format!("closeloop:{item}:retry:{job}")
}
pub fn input_relations(item: &str, job: &str) -> String {
    format!("closeloop:{item}:relations:{job}")
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
/// lane closeloop4 (closeloop.md 18): the second route's fact answers of a fact item (kept apart from the first's).
pub fn input_facts2(item: &str, job: &str) -> String {
    format!("closeloop:{item}:facts2:{job}")
}
/// lane closeloop4 (closeloop.md 19): the terminal proposals of node `n` at check round `k` (awaiting a decision).
pub fn input_tproposals(n: &str, k: u32) -> String {
    format!("closeloop:{n}:tproposals:r{k}")
}
/// lane closeloop4: prefix of the terminal decisions of node `n` (operator or automatic).
pub fn input_tdecision_prefix(n: &str) -> String {
    format!("closeloop:{n}:tdecision:")
}
pub fn input_tdecision(n: &str, id: &str) -> String {
    format!("{}{}", input_tdecision_prefix(n), safe(id))
}

pub const ROLE_SPEC: &str = "table_set_spec";
pub const ROLE_BOX: &str = "box";
pub const ROLE_CHECK: &str = "closeloop_check";
pub const ROLE_BATCH: &str = "closeloop_batch";
pub const ROLE_ANSWERS: &str = "closeloop_answers";
pub const ROLE_RESULT: &str = "closeloop_result";
pub const ROLE_PBATCH: &str = "closeloop_proof_batch";
pub const ROLE_PROOFS: &str = "closeloop_proofs";
pub const ROLE_FACTS: &str = "closeloop_facts";
pub const ROLE_RELATIONS: &str = "closeloop_relations";
pub const ROLE_RETRY: &str = "closeloop_retry";
pub const ROLE_FACTS2: &str = "closeloop_facts2";
pub const ROLE_TPROPOSALS: &str = "closeloop_terminal_proposals";
pub const ROLE_TDECISION: &str = "closeloop_terminal_decision";

pub const KIND_IMPORT: &str = "import";
pub const KIND_SOLVE: &str = "recipe-solve";
pub const KIND_PROOF: &str = "proof-span";
pub const KIND_FACTS: &str = "proof-facts";
/// lane closeloop3: items of `coefficient-facts` jobs (exact evaluation of elided records' coefficients).
pub const KIND_EXACT: &str = "proof-exact-eval";

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
        assert_eq!(
            parse_proof_item("N0", &proof_item("N0", 2, 5, "span")),
            Some((2, 5))
        );
        assert_eq!(
            parse_proof_item("N0", &proof_item("N0", 2, 5, "facts")),
            Some((2, 5))
        );
        assert_eq!(
            parse_proof_item("N0", &proof_item("N0", 2, 5, "exact")),
            Some((2, 5))
        );
        assert_eq!(proof_item("N0", 2, 5, "exact"), "N0.x2.5");
        assert_eq!(parse_proof_item("N0", "N0.s2.5"), None);
    }
}
