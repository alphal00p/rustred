# T075 test data (owner: M3-T13 rr-portfolio)

Copies, byte for byte, read by `../portfolio.rs`:

- `T075c.sym`: the M2-T10b regression file of T075c,
  `crates/rustred-core/tests/support/eps_families/T075c.sym`
  (sha256 dcf537d057d958be321f81bab885235d0bcfd24e780c02ae22ad546ce5ce1abc; = rules_study
  `SCRATCHPAD/m2-canary/families/T075e.sym` with another name: `classify/families/T075.sym` plus
  `power_shift(D6,2-d/2)`).
- `T075p1.sym`: the family of record of the KO VERIFIED_RS T075p1 set, `SCRATCHPAD/m2-canary/families/T075p1.sym`
  (sha256 5e4a2e1061f49e20181b78228bc44964eb081418ecc9d826474e363871ba1e8d; equal to the `family_source` of the
  set's header and to Astra's `autonomous_rs_four_b_20261108/T075/family.sym`): T075c with lines and ISPs
  permuted (m2-canary set P1), slot map onto T075c `[3,5,1,4,0,8,2,6,7,12,13,11,9,10]`
  (`t075-rs-promotion.json` RS6).

The RS set itself (table_dump.v1, 4,212,263 bytes, sha256
67cb228e71da01f854fd88533fbbe2b4c93ffd0f20580d28761a957f9d9eaa5f, 360 rules, 0 residuals) stays outside the
repository: `rules_study/ko4/T075/ko4-dump/OUT.table.jsonl` of the p-integral workspace (= Astra's
`autonomous_rs_four_b_20261108/T075/OUT.table.jsonl`, VERIFIED_RS 2026-11-08T22:09Z). The ignored test reads it
from `RUSTRED_T075P1_RS_TABLE`.
