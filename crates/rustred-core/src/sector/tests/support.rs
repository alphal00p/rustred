pub(super) fn all_indices(arity: usize, minimum: i64, maximum: i64) -> Vec<Vec<i64>> {
    fn recurse(
        arity: usize,
        minimum: i64,
        maximum: i64,
        current: &mut Vec<i64>,
        output: &mut Vec<Vec<i64>>,
    ) {
        if current.len() == arity {
            output.push(current.clone());
            return;
        }
        for value in minimum..=maximum {
            current.push(value);
            recurse(arity, minimum, maximum, current, output);
            current.pop();
        }
    }
    let mut output = Vec::new();
    recurse(arity, minimum, maximum, &mut Vec::new(), &mut output);
    output
}
