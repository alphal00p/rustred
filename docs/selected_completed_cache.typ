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
let limits = SavedApplicationCacheFileLimits {
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

The compressed allowance above is decimal bytes; the historical file-format
ceiling is 95 MiB. The existing decoded-file ceiling of 1 GiB remains separate.
This bounded single-table transport is intended for selected caches. An
unbounded accumulated cache retains its native bundle and existing archive
transport instead.

Selection does not prove an IBP identity or broader campaign coverage. Record
validation checks the declared terminals, native source conditions and algebra
in the current context. Original producer authority and trustworthy checksums
remain necessary. Usage profiles may rank existing shortcuts, but hit frequency
alone is not measured work saved; actual encoded size and independent matched
reductions establish size and performance.
