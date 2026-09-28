# W0.3 successor streams: format

Produced by the native re-inspection harness
(`crates/rustred-app/src/application/routed_campaign/walking/reinspection.rs`,
`RUSTRED_HARNESS_SINK=dump`). Each stream is the exact sequence of `Event`s that
`inspection::inspect` (the walk's own inspection entry point) emitted for one
domain, after the inspection-side job-local reuse cache (`reuse.rs`,
`MAX_KEYS = 4096`), i.e. what the walk's coordinator/admission side receives.
The harness sink accepts every event, so every stream is complete (a stream
whose `error_kind` is not `none` is a native failure or cancellation and is
flagged in the index).

## Files

A run directory holds `streams/part-TTT.bin` (one per harness thread `TTT`)
and `streams/part-TTT.index.jsonl` (one line per native in that file).

Index line fields: `i` (fixture index), `id` (checkpoint domain id of the
inspected parent), `file`, `offset` (byte offset of the block), `block_bytes`,
`events`, `phase`, `owner` (mask string, axis 0 first), `stratum`,
`error_kind`, `pass`.

## Binary layout (all integers little-endian)

File header (16 bytes): `b"RRSTRM01"`, `u32 arity N`, `u32 0`.

Per native, one block:

| field | type |
|---|---|
| block magic | `u32` = `0x5654414E` (bytes `N A T V`) |
| parent id | `u64` |
| fixture index `i` | `u32` |
| parent phase | `u8` (0 Apply, 1 Route) |
| event count E | `u32` |
| payload bytes B | `u64` |
| payload | E events |

Each event: `u32 L`, then L bytes of the canonical event encoding
(`encode_event` in `reinspection.rs`, also used for the digests of the
differential proof):

| field | type |
|---|---|
| tag | `u8`: 0 Count, 1 KnownReuse, 2 PreAdmittedOrthantReuse, 3 Admit, 4 Frontier, 5 Optional |
| count | `u32`: callback charge of this event (adjacent homogeneous runs may be compacted; usually 1) |
| flags | `u8`: bit 0 successor (a rule-application successor; Route covers have 0), bit 1 conditional (coefficient nonzero only conditionally) |
| payload | tags 2-5 only: `u32 P` then P bytes |

Payloads:
- tag 2 `PreAdmittedOrthantReuse`: `u64` target id (an initial full-orthant
  domain of the checkpoint; contained, so no admission request).
- tag 3 `Admit`: the successor domain as a CP5 domain-segment record, i.e.
  bincode 2 `config::standard()` (varint integers, zigzag for signed) of
  `Domain<N>`: `phase` (varint 0 Apply / 1 Route), `owner` (varint N, then N
  bytes 0/1), `lower` (varint N, then N varint u64), `upper` (varint N, then N
  `Option<u64>`: byte 0 = None/+infinity, byte 1 + varint), `rank`
  (`Option<u32>`), `powers` = (`max_positive_power: Option<u64>`,
  `min_power_difference: Option<i64>`, `max_power_difference: Option<i64>`).
  The same decoder as `indexscan` (varint tags 251-254 = u16/u32/u64/u128).
  This is the admission request the walk would resolve (exact, orthant,
  containment or new domain).
- tag 4 `Frontier`: JSON of the frontier record as the walk would persist it.
- tag 5 `Optional`: JSON of the optional-coefficient refusal diagnostic.

Tags 0 (`Count`: a classified terminal/selected-rule/zero-sector piece or a
rule completion) and 1 (`KnownReuse`: an `Admit` identical to an earlier
`Admit` of the same stream, suppressed by the job-local cache; it carries no
domain) have no payload.

## Reading

`read_streams.py` (next to this file) decodes blocks and domains; see its
`--help`. Offsets in the index allow random access to single natives.
