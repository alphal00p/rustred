= Selected completed saved-application cache records

A large immutable cache can supply a smaller runtime cache without repeating a
reduction. Selection uses exact #raw("ApplicationCacheKey") identities, including
the Route/Apply stage and the full Apply owner. The caller supplies an
authenticated campaign/catalogue binding and a captured immutable pack.

#raw("visit_selected_completed_records", lang: "rust") visits each requested key
once, ignoring duplicate requests. It preserves newest-run precedence, including
empty zero records. Range and negative-key filters skip unrelated chunks. Each
candidate physical chunk is authenticated and imported at most once, even when
retained runs reference the same chunk; only requested coefficients are decoded.
Missing keys are errors. Callback order follows the native scan, and a callback
failure stops immediately; previously delivered callbacks are not rolled back.

#raw("PreparedSavedApplication::encode_selected_completed_cache", lang: "rust")
adds the current owner's validation of every selected record, sorts the exact
keys and uses the existing deterministic single-file encoder once. Its one gzip
member contains one native coefficient table shared by the selected records.
It preserves terminal coefficients and ordered nonzero conditions and performs
no reduction, resident export or source mutation.

```rust
let mut limits = SavedApplicationCacheFileLimits {
    max_compressed_bytes: 95_000_000,
    ..Default::default()
};
let (bytes, identity) = prepared.encode_selected_completed_cache(
    &authenticated_binding,
    &mut frozen_pack,
    &selected_keys,
    limits,
)?;
```

Here #raw("selected_keys") is a slice of existing native keys, and the prepared
owner must belong to the same authenticated application. The caller atomically
writes the fully encoded bytes and records the returned identity; an error does
not return a partial file. A selection from several sources must first use their
explicit native union or otherwise resolve their precedence. The strict
single-pack visitor does not silently fall back to another store for missing
keys.

The compressed allowance above is decimal bytes. The ordinary file limits
default to 95 MiB compressed and 1 GiB decoded; these are caller-owned grants,
not absolute format ceilings. Published light caches must meet both their
95,000,000-byte compressed policy and the default decoded allowance. Record the
measured compressed and decoded identity before publishing a selection.

== Larger native files and shared lazy reads

An independently hosted heavy cache uses the same native format: exactly one
gzip member, one sorted record directory and one shared coefficient table.
Explicit larger grants are accepted without changing the codec or reduction
limits. #raw("with_decoded_bytes") includes the native file header in its
allowance. An authenticated producer identity can supply exact load grants:

```rust
let limits = SavedApplicationCacheFileLimits {
    max_compressed_bytes: trusted_identity.compressed_bytes.try_into()?,
    ..Default::default()
}.with_decoded_bytes(trusted_identity.decoded_bytes.try_into()?)?;
limits.chunk.max_records = trusted_statistics.records.try_into()?;
let options = SavedApplicationCacheFileOptions {
    limits,
    decoded_directory: Some(private_decoded_directory.to_owned()),
};
let cache = SavedApplicationCacheFile::open_with_options(
    path, &authenticated_binding, Some(&trusted_identity), &options,
)?;
```

Record-count and index grants remain independent admission checks; set them
from the measured inventory and available RAM rather than assuming a larger
decoded allowance raises every limit. The decoded directory must already
exist. Loading streams decompression once
into a private owned file. A bounded scan authenticates its native digest and
retains one BLAKE3 digest per 64 KiB block; subsequent selected-range reads check
the loaded blocks. The record/frame index, a fixed verification buffer and
selected row or atom buffers remain in RAM. Native symbolic state imports once
on first use, and accessed coefficients remain in the shared lazy table across
reader forks. Its admitted coefficient storage and the index still need real
RAM; disk backing does not make either unbounded resource free.

== Streaming complete snapshots

#raw("SavedApplicationCacheFileWriter") streams validated completed records
through buffered private spools. Its unordered mode accepts the deterministic
scan order of #raw("visit_all_completed_records"), indexes exact keys in memory,
and writes the spooled rows in sorted order with one shared coefficient table.
It deduplicates only identical full
records. Different coefficients, guard order or guard multiplicity for one
exact key cause an error before a complete file is returned. The caller owns
atomic publication after successful finalization and synchronization.

Pack and read-only visitors expose every committed version, including shadowed
keys. #raw("visit_closed_completed_records") additionally requires an explicitly
closed, drained running writer with no writer error. It refreshes a detached
delta reader and verifies the acknowledged final generation before scanning;
it never flushes or closes a live producer implicitly.

The writer retains the complete unique compact-coefficient interner and the
key/range index. Its payload and index statistics are memory floors rather than
RSS guarantees. Unordered finalization can use two decoded-size disk spools in
addition to compressed output. Establish an actual census and resource budget
before a large conversion. A small pilot with no conflicts does not establish
that the complete accumulated cache has no guard variants. Preserve every raw
source prefix; if a complete census finds conflicting valid versions, retain
their concrete identities and conditions for an owner-level resolution rather
than silently applying newest-run precedence.

Native directory packs and bundles remain useful for incremental capture,
frozen source snapshots and recovery. Their existing directory/archive tooling
can preserve or transport those original sources. Such an archive is a source
backup; the published single-file runtime cache is the native gzip above.

Selection does not prove an IBP identity or broader campaign coverage. Record
validation checks the declared terminals, native source conditions and algebra
in the current context. Original producer authority and trustworthy checksums
remain necessary. Usage profiles may rank existing shortcuts, but hit frequency
alone is not measured work saved; actual encoded size and independent matched
reductions establish size and performance.
