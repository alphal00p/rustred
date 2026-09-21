# Required native heap-power correction

`heap-pow-wide-radix.patch` applies to Symbolica revision
`953e26e2754e9a4b918404fbdea590725d8e863d` (the current submodule base).
It is a portable, vendor-relative patch for the upstream author, not a RustRed
replacement algebra implementation.

From the RustRed root after initializing that clean submodule:

```sh
git -C vendor/symbolica apply --check ../../patches/symbolica/heap-pow-wide-radix.patch
git -C vendor/symbolica apply ../../patches/symbolica/heap-pow-wide-radix.patch
```

Apply once. If `--check` fails, inspect whether the correction is already present
or the dependency revision differs; do not overwrite unrelated vendor edits.
After a compatible upstream revision incorporates this fix, that revision can
replace the patched checkout, keeping Symbolica/Numerica/Graphica coherent.

The old `heap_pow` packs exponents with a u32 intermediate, so small exponents
across enough variables can lose their exact radix encoding. The correction
uses native `Integer` Horner arithmetic and native `quot_rem`, checks radix
construction, and leaves the heap recurrence and public exponent types unchanged.
Included vendor tests cover a 22-variable square (253 terms) against native
multiplication, nonuniform radices, zero gaps, packed values beyond u128, and an
individual radix beyond u32 on 64-bit hosts. RustRed also exercises public native
power versus multiplication in its normal numerator-expansion test suite.

Run RustRed's public regressions with:

```sh
cargo test --release --locked -p rustred native_heap_pow -- --test-threads=1
```

The additional vendor-private codec tests require Symbolica's own unit-test
target; passing RustRed tests alone does not mean those vendor tests ran.
No unpatched large-power reproduction is needed or recommended. A static radix
defect is established; attribution of any earlier long-running campaign to this
defect requires separate bounded dynamic evidence.
