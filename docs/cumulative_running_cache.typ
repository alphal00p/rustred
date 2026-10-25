= Cumulative completed-record caches

`SavedApplicationRunningCache` attaches optional persistence to the existing
saved Route/Apply application. It uses the existing native Pack chunks, binding,
coefficient codec and checkpoint index. It does not change rule selection,
unresolved-integral policy, admission, reduction limits or terminal conventions.
Ordinary calls without this attachment retain their existing behavior.

== Completed records under Preserve

Preserve-mode ready DAG nodes can still contain unresolved integrals. Capture
omits these nodes before optional materialization and retains independently
complete descendants and exactly cancelled completed parents. Every emitted
record passes the existing strict completed-record validation. Imported backing
records also pass that validation before admission, including the original
source nonzero conditions. Binding equality alone cannot authenticate an
arbitrary supplied reduction identity; native imports remain trusted-generated
data.

The running sink requests all eligible newly computed records. Native backing
hits are already stored and are not captured again; legacy preparation/copy
sinks retain their existing imported-hit capture behavior. Queue grants apply
backpressure and explicitly refuse a single record larger than the configured grant. They
do not impose an aggregate cache size or silently stop after a preparation
count. Required running capture propagates a materialization limit with its
original typed cause. Existing optional capture sinks keep their prior behavior
through the default `fail_on_materialization_limit() == false`.

== One delta, explicit immutable bases

`SavedApplicationRunningCache::open(binding, context, directory, bases, options)`
creates a missing standalone native Pack or resumes an existing one. All bases
must have the same exact artifact/family/arity/layer binding. The native writer
lock rejects a second writer to that directory. Separate processes use separate
deltas; a writer conflict is an error. Neither opening nor saving copies the
base payloads or replaces old chunks.

`with_io` supplies short backing/capture borrows to the normal reduction method.
Reads consult the committed delta and then the explicit bases in order.
`with_capture` supplies only capture for a productive cold baseline. `fork`
shares the existing writer lease and forks reader bookkeeping; it does not
reopen another writer. Settings reloads can move this session or use its fork.

The owned writer thread receives bounded completed-record clones and a cloned
coefficient context. Rust's ordinary thread bounds enforce ownership; there is
no unsafe Send/Sync assertion or engine reference on that thread. Queue metadata
locks do not cover coefficient encoding or filesystem I/O. The control handle
does not acquire an embedding application's prepared/cache mutex, so callers
can retain it outside a long reduction and query bounded metadata or request a
barrier without reentering the engine.

== Durability and failures

Accepted records have been enqueued; they are not yet durable. Pending counts
and bytes cover accepted records not acknowledged by a successful checkpoint.
Committed counts are session-local record publications, including repeated keys
in later immutable runs; they are not a unique mathematical-node census. A
resumed generation includes older committed data even though session counters
start at zero.

The worker batches native chunks and checkpoints at the configured count/byte
boundary or timeout. It can commit records already received while a later
coefficient operation is busy. It cannot save unfinished algebra, an in-progress
materialization, or a record not yet enqueued. The timeout is a scheduling
cadence, not a hard durability deadline; encoding, storage and resource checks
can take longer.

`with_io` and `with_capture` request a flush barrier after either callback result.
The original mathematical error retains precedence over a simultaneous writer
failure. If mathematics succeeds, a barrier failure is returned. Reports retain
writer failure and pending work; subsequent operations do not silently treat
failed persistence as success. Captured shared resource scopes keep their
original causes and deadlines: a refused flush remains an explicit failure,
not a claim that its pending records became durable. Dropping handles cannot
report errors and is not a durability acknowledgement.

`control().report()` returns primitive queue/checkpoint/fault metadata.
`flush()` is a publication barrier, and `close()` additionally releases the
writer lease. `snapshot(new_external_index_path)` first flushes and returns the
exact committed generation, full native-index digest, byte count and reference.
Callers should persist the report/descriptor and explicitly check barrier
results on normal and error exits. An outer symbolic/numerical finalization
failure cannot undo a previously acknowledged native checkpoint.

== Frozen benchmark readers

`SavedApplicationCachePack::freeze_snapshot` and the writer equivalent publish
an immutable sidecar containing the original committed native index bytes.
They copy no coefficient chunks and introduce no second persisted cache codec.
A reader snapshot captures its loaded generation even if the mutable latest
index has advanced. The sidecar path must be new and outside the source Pack.
The caller retains the source chunk directory and accounts for sidecar storage.

`SavedApplicationCachePack::open_snapshot(directory, binding, descriptor, limits)`
checks the exact digest, length, generation, native framing and binding. It never
reads a newer index, and both it and its forks reject refresh. The original
append-only chunks remain usable as the producer appends. Retained raw index
bytes are included in the reader's directory-memory allowance.

For held-out execution, construct `SavedApplicationReadOnlyCache` from frozen
Pack bases and pass only its backing to the ordinary application. This opens
no writer and performs no training. A running session snapshot describes its
delta; callers must retain the explicit immutable base configuration and pin
every base manifest too. Attaching a separate capture-only delta to a benchmark
does not change which immutable manifests supplied its reads.

Existing legacy single-file import safety limits remain unchanged. Running
Pack capacity is governed by explicit chunk/manifest/disk/queue grants and
ambient resources; there is no former 50/95 MiB aggregate publication limit.
No path deletes old committed chunks to make room.

== Portable unions of captured bases

`SavedApplicationCacheBundleWriter::copy_base_union_to_new` publishes several
captured Pack readers into a new independent bundle. Its base list is ordered
from highest to lowest priority, matching ordinary running-cache base lookup.
For each exact binding it concatenates bases in reverse order, retaining each
base's internal run order. Native lookup therefore prefers the first supplied
base and its newest matching record. Every run and shadowed record reference
is retained; the operation does not establish unique-key counts or validate
new mathematical identities.

Each output child starts at generation one as a new store. That generation
does not order the input stores or identify their provenance. Preserve the
explicit captured source descriptors when provenance matters; the output's
native index digest and directory identify its committed contents.

Payloads remain in the existing native chunk representation. Compatible
identical compressed hashes share one copied file per output namespace, while
all original run references remain in the index. Descriptor conflicts fail
before output creation. Copying checks compressed length and hash through the
existing 64 KiB buffer, creates independent destination inodes, and performs
no decompression, coefficient import, algebra or record re-encoding. Source
readers retain their captured manifests even if a producer later appends.

Before creating the output, the operation admits combined index and copy-plan
memory, encoded metadata and aggregate disk bytes under the caller's grants
and shared resource scopes. Existing conservative referenced-byte limits also
remain: repeated references can consume that allowance even when physical
chunk bytes are deduplicated. The bundle index is published last after child
chunks and indexes are durable. A failed copy retains its partial destination
and refuses overwrite; it does not modify, delete or lock the source stores.

The existing `copy_bases_to_new` continues to accept at most one base per
binding and retains its original generation-preserving behavior. Neither
operation copies active computation or preparation-job metadata. This union
requires storage for the copied payloads; metadata-only frozen snapshots remain
the appropriate operation when independent portable payloads are unnecessary.
