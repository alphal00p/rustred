# RustRed handoff — simple overview

## The big picture

RustRed is being built to find reusable equations that reduce complicated loop
integrals to a finite list of standard integrals. It is written in Rust and
uses Symbolica for symbolic mathematics. The long-term target is six-loop
single-scale vacuum calculations; the immediate target is to finish every
family through three loops and use those equations inside Vakint without FORM.

The one-loop and two-loop parts already work. RustRed can generate, save,
reload, inspect, and apply those equation sets from Rust, the command line, and
Python (`import rustred`). The three-loop family has six integral coordinates,
so the project calls it `K=6`. That family is not closed yet, and therefore
Vakint's new RustRed mode is not yet validated for every three-loop test.

## Why K=6 has not finished

The older completion attempt was mathematically systematic, but its symbolic
coefficients were expanded too early. After roughly 60–110 equations, some
coefficients contained tens of millions of terms and the run became too slow
and memory-hungry to reach an answer. This does not show that the method is
wrong; it shows that the old representation could not finish the calculation.

Separate bounded studies reduced the unexplained infinite regions to 10 boxes
for one routing, 4 for another, and 3 five-dimensional boxes in the strongest
focused study. Those numbers are useful diagnostics, but they are not “rays
left after a finished run,” because the full completion queue never exhausted.
Some missing directions appear to need a loop-routing or factorization move
involving numerator variables before ordinary integration-by-parts equations
can make progress.

## What was just completed

The latest pushed milestone is commit `759ab1c` on RustRed `main`. It replaces
eagerly expanded intermediate coefficients with exact, shared calculation
circuits. It also provides immutable snapshots of the current equation basis,
exact tests for whether terms are really zero, safe handling of exceptional
parameter values, and an exact replay step that checks results with Symbolica.

This foundation was reviewed independently by three agents. The focused tests
passed 72/72, the full completion subsystem passed 196/196, and the entire
RustRed core passed 1,237 tests with no failures. The complete workspace suite
also passed. The repository was clean at that milestone; the only subsequent
changes are the two handoff files requested by the user.

## What must happen next

The new fast representation still needs to be connected into a complete run:

1. Let the reducer use the new shared equation snapshots directly.
2. Generate each required next equation without expanding its coefficients.
3. Add new equations safely, simplify the whole basis, and handle equations
   that acquire the same leading term.
4. Keep processing every required equation until none remain.
5. Check separately that only a finite list of standard integrals is left.
6. Expand and verify the final equations in small memory-bounded batches, then
   save and reload the artifact.

Only after all six steps succeed may anyone say that K=6 closes. Short release
runs should be made after each coherent implementation step so the work stays
grounded in the real three-loop target. Different variable orderings and
generic routing/factorization seeds should be compared, but they must never be
used to skip required equations.

Once the K=6 artifact exists, Vakint should ship it alongside the existing
one- and two-loop artifacts. Vakint will use FeynKit for tensor reduction and
RustRed for the scalar equations, giving a FORM-less path. Its existing test
harness should compare this path against MATAD and AlphaLoop for every
applicable one-, two-, and three-loop case. A finite but nonminimal list of
standard integrals is acceptable if final numerical answers agree.

## How to resume safely

Start by reading `GOAL.md` and the detailed `HANDOFF.md`; `GOAL.md` is the
authority. Verify that RustRed `main` is clean and at the pushed handoff commit,
use the current Symbolica license supplied by the user, and run the focused
exact-lazy tests. Then delegate the next implementation pieces and an
independent audit. Use release builds for timings, keep RustRed generic, use
Symbolica rather than rebuilding symbolic algebra, and commit/push clean
milestones with the required ValentinHirschi Git identity.

Do not clean or overwrite the separate GammaLoop/Vakint worktree blindly: its
`vakint_rustred` branch currently contains uncommitted work and has diverged
from its remote. Audit that state before continuing Vakint integration.
