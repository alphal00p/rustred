# Forcer normalisation anchors (copy)

`p_anchors.json` is a byte-identical copy of vakint's
`crates/vakint/tests/fixtures/forcer/p_anchors.json` (alphal00p/gammaloop, branch `p_integral`, last changed in
`d48b648b4`, copied at `954865ff0`; sha256 `50ad1d173e05019921f5d0e32cf0863d017278576b4dd201b37f5029ea4add52`, blake3
`b01f541bb4f320412bf8ad6311770d9b4302200be6a5ea9d9a06a4b00659da63`),
written by M2-V2 (`fixtures::anchor_cases`, `VAKINT_FORCER_FIXTURES=write`) with FORM 5.0.0 and Forcer `68a30bf`
(B. Ruijl, T. Ueda, J. A. M. Vermaseren; used with the authors' permission, DECISIONS.md 2026-11-03).

Each case is a vakint family, its line powers, a numerator in `k(j)` and the user's `p(1)`, the canonical `p(1)` in
the user's head (`external`, `+-p(1)`) and Forcer's raw G-scheme output `C_G` (before the Wick sign). vakint's backend
contract is `reduced = (-1)^(sum a + r) C_G` (`sum a` over the positive line powers, `r` the number of scalar
products with a loop momentum); convention C3 of the M2 plan makes RustRed's `onto_masters` reduction equal to it.
The anchor tests (`basis.rs` here and in `rustred-app/tests/p_library_app/`) compare both exactly in the
(Gamma(1 + m eps) monomial x master) basis.

Refresh: copy the file again from gammaloop `p_integral` and update the commit and the hashes above; the test
`the_anchor_copy_is_the_vakint_fixture` pins the blake3.
