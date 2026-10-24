//! Standalone audit of the pinned native APIs, outside the library module tree.
//!
//! Compile against the already built Symbolica component rlibs; no project or
//! production build is required. Both components are reexported by Symbolica.
//!
//! Audit 2026-10-22: pinned Symbolica ef0db494533c87adb40356c996241680dc5a7bff,
//! Symbolica/Graphica/Numerica version 3.0.0. Read Graphica's public
//! CanonicalForm/canonize API, generator-cycle implementation and renumbering,
//! README canonization example and native canonize/iso_check_permutations
//! tests; read Numerica Matrix rank/row_reduce/inv/transpose/multiplication
//! implementations and native matrix examples/tests. Existing RustRed
//! parametric/proposal.rs confirms use of the same canonical vertex mapping.
//! The cached-library probe passed on 2026-10-22 using rustc 1.97.1 and thin
//! LTO. Its temporary executable was removed after the successful run.

use graphica::Graph;
use numerica::{
    domains::rational::{Q, Rational},
    tensors::matrix::Matrix,
};

fn main() {
    let q = Matrix::from_linear(
        vec![1, 0, 0, 1, 1, 1]
            .into_iter()
            .map(Rational::from)
            .collect(),
        3,
        2,
        Q,
    )
    .unwrap();
    assert_eq!(q.rank(), 2);
    let qt = q.transpose();
    let gram = &qt * &q;
    let projector = &(&q * &gram.inv().unwrap()) * &qt;
    assert_eq!(&projector * &projector, projector);

    let mut graph = Graph::<u8, u8>::new();
    for color in [1, 0, 1, 0] {
        graph.add_node(color);
    }
    for (left, right) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
        graph.add_edge(left, right, false, 0).unwrap();
    }
    let canonical = graph.canonize();
    assert_eq!(canonical.automorphism_group_size.to_string(), "4");
    for cycles in &canonical.orbit_generators {
        let mut action: Vec<_> = (0..4).collect();
        for cycle in cycles {
            for position in 0..cycle.len() {
                action[cycle[position]] = cycle[(position + 1) % cycle.len()];
            }
        }
        let mut mapped = Graph::<u8, u8>::new();
        for node in canonical.graph.nodes() {
            mapped.add_node(node.data);
        }
        for edge in canonical.graph.edges() {
            mapped
                .add_edge(
                    action[edge.vertices.0],
                    action[edge.vertices.1],
                    false,
                    edge.data,
                )
                .unwrap();
        }
        assert_eq!(mapped.canonize().graph, canonical.graph);
    }
    println!("native matrix rank/inverse/projector and canonical generator-cycle probe passed");
}
