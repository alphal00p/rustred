# CLI example

[`run.sh`](run.sh) generates the complete `unit-mass-vacuum-k3` artifact into
a temporary file, authenticates and inspects those bytes, and applies them to
`I(2,2,1)`:

```bash
sh examples/cli/run.sh
```

The inspection output reports algorithm
`rustred.generated.two-loop-unit-mass-sunset.v1`, arity 3, four source rows,
five guarded rules, two masters, and four zero sectors. The reduction output
contains master keys `[0,1,1]` and `[1,1,1]`, with common-mass-squared powers
`-3` and `-2`, respectively. The temporary artifact is removed on exit.
