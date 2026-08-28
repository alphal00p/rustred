# RustRed legacy oracles

This publish-disabled crate contains historical, topology-authored reduction
logic and concrete lower-loop fixtures. It exists only to preserve useful
regression and differential evidence while RustRed's generic Symbolica-backed
engine replaces those finite paths.

The dependency direction is intentionally one way: this crate depends on the
topology-neutral `rustred` core through its narrow, documentation-hidden
`legacy-oracle-support` facade. The core, application layer, CLI, Python
package, and Vakint integration must never depend on this crate. It is a
workspace member so its tests remain reproducible, but it is not a default
member and it is not publishable.

The code here is not a production reduction backend, a source of generic
algorithms, or a compatibility API. Each retained oracle should eventually be
replaced by a smaller fixture or a generic end-to-end test and then deleted;
Git history is the archive.
