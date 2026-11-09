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

# Four-loop anchors (copy, M3-T17)

`p4l_anchors.json` is a byte-identical copy of vakint's `crates/vakint/tests/fixtures/forcer/p4l_anchors.json`
(alphal00p/gammaloop, branch `p_integral-vk-fixtures4`, commit `0cd200618e98037a0a26e75dc6ec7f13a73cf238` = M3-V7's
'ANCHORS READY' line in `notes/vk-fixtures4.md`; sha256
`607013f2a7419942d67e400c9633071159f7a94ef08f0269260885ec3f6cb497`, equal to the artefact copy in
`artefacts/m3/vk-fixtures4/SHA256SUMS`; blake3 pinned in `basis4.rs`), written by M3-V7 (`fixtures::anchor_cases_4l`,
`VAKINT_FORCER_FIXTURES=write`, `tform -w4`, 18 s) with FORM 5.0.0 and Forcer `68a30bf`.

17 cases, same schema plus two informative fields: `master` (the Masters.prc name) and `definition` (the Forcer source
path:line of the master's definition, its topology in Forcer notation and the realisation). One case per master of
Forcer's four-loop basis not already anchored in `p_anchors.json` (bebe, cross, haha, lala, no1, no2, no6, nono,
fastar2, nostar5, nostar6, t124, t145, t155), each at its Forcer definition: a master without eps lines is its own
4L family sector at unit powers; an eps line (`*` = one eps unit) is realised as a line and a one-loop bubble in series
per eps unit, so Forcer returns `GschemeConstants(0,0)^k Master(name)` (`k` = the eps units). Three more 4L tops:
`la4_top` (finalized eps^0 = -70 zeta(7)), `no3_top` (Forcer's no3 override) and `bubu_top` (knock-out, no master of
its own). The outputs are raw `C_G` before the Wick sign: `reduced = (-1)^(sum a + r) C_G`, as for `p_anchors.json`.

Refresh: copy the file again from the gammaloop commit of a later 'ANCHORS READY' line and update the commit and the
hashes here and in `basis4.rs`. M3-T8c's final merge event compares this copy with V7's latest tip (`cmp`).
