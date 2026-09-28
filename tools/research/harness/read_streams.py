#!/usr/bin/env python3
"""Decode W0.3 successor streams (format: STREAMS.md next to this file).

  read_streams.py summary STREAMS_DIR          per-file and total event counts by tag
  read_streams.py show STREAMS_DIR ID [N]      first N events of the native with parent id ID
  read_streams.py verify STREAMS_DIR           decode every block and check the index

Pure Python, no dependencies; intended as the reference decoder for Rust tools.
"""
import glob
import json
import os
import struct
import sys

TAGS = ["count", "known_reuse", "pre_admitted_orthant_reuse", "admit", "frontier", "optional"]
BLOCK_MAGIC = 0x5654414E


class Reader:
    def __init__(self, b, p=0):
        self.b, self.p = b, p

    def u8(self):
        v = self.b[self.p]
        self.p += 1
        return v

    def var(self):
        t = self.u8()
        if t <= 250:
            return t
        n = {251: 2, 252: 4, 253: 8, 254: 16}[t]
        v = int.from_bytes(self.b[self.p:self.p + n], "little")
        self.p += n
        return v

    def zig(self):
        v = self.var()
        return (v >> 1) ^ -(v & 1)

    def opt(self, f):
        t = self.u8()
        if t == 0:
            return None
        assert t == 1, t
        return f()


def decode_domain(b):
    r = Reader(b)
    phase = ["Apply", "Route"][r.var()]
    owner = [r.u8() for _ in range(r.var())]
    lower = [r.var() for _ in range(r.var())]
    upper = [r.opt(r.var) for _ in range(r.var())]
    rank = r.opt(r.var)
    powers = (r.opt(r.var), r.opt(r.zig), r.opt(r.zig))
    assert r.p == len(b), "trailing domain bytes"
    return {"phase": phase, "owner": "".join(map(str, owner)), "lower": lower, "upper": upper,
            "rank": rank, "max_positive_power": powers[0], "min_power_difference": powers[1],
            "max_power_difference": powers[2]}


def decode_event(b):
    tag, count, flags = b[0], struct.unpack_from("<I", b, 1)[0], b[5]
    event = {"tag": TAGS[tag], "count": count, "successor": bool(flags & 1), "conditional": bool(flags & 2)}
    if tag >= 2:
        (n,) = struct.unpack_from("<I", b, 6)
        payload = b[10:10 + n]
        assert 10 + n == len(b), "event length"
        if tag == 2:
            event["target"] = struct.unpack("<Q", payload)[0]
        elif tag == 3:
            event["domain"] = decode_domain(payload)
        else:
            event["json"] = json.loads(payload)
    else:
        assert len(b) == 6, "event length"
    return event


def blocks(path):
    data = open(path, "rb").read()
    assert data[:8] == b"RRSTRM01", "magic"
    arity = struct.unpack_from("<I", data, 8)[0]
    p = 16
    while p < len(data):
        start = p
        magic, pid, index, phase, events, nbytes = struct.unpack_from("<IQIBIQ", data, p)
        assert magic == BLOCK_MAGIC, f"block magic at {p}"
        p += struct.calcsize("<IQIBIQ")
        end = p + nbytes
        evs = []
        while p < end:
            (n,) = struct.unpack_from("<I", data, p)
            evs.append(data[p + 4:p + 4 + n])
            p += 4 + n
        assert p == end and len(evs) == events, "block payload"
        yield {"offset": start, "id": pid, "i": index, "phase": ["Apply", "Route"][phase],
               "arity": arity, "events": evs, "block_bytes": end - start}


def main():
    mode, root = sys.argv[1], sys.argv[2]
    files = sorted(glob.glob(os.path.join(root, "part-*.bin")))
    if mode == "summary":
        total = {t: 0 for t in TAGS}
        natives = 0
        for f in files:
            for blk in blocks(f):
                natives += 1
                for e in blk["events"]:
                    total[TAGS[e[0]]] += 1
        print(json.dumps({"files": len(files), "natives": natives, "events_by_tag": total}))
    elif mode == "show":
        want, limit = int(sys.argv[3]), int(sys.argv[4]) if len(sys.argv) > 4 else 20
        for f in files:
            for blk in blocks(f):
                if blk["id"] == want:
                    print(json.dumps({k: v for k, v in blk.items() if k != "events"}))
                    for e in blk["events"][:limit]:
                        print(json.dumps(decode_event(e)))
                    return
    elif mode == "verify":
        checked = 0
        for f in files:
            index = [json.loads(l) for l in open(f[:-4] + ".index.jsonl")]
            got = list(blocks(f))
            assert len(index) == len(got), f"{f}: index lines {len(index)} vs blocks {len(got)}"
            for entry, blk in zip(index, got):
                assert entry["offset"] == blk["offset"] and entry["id"] == blk["id"], f"{f}: index mismatch"
                assert entry["block_bytes"] == blk["block_bytes"] and entry["events"] == len(blk["events"])
                for e in blk["events"]:
                    decode_event(e)
                checked += 1
        print(json.dumps({"verified_natives": checked, "files": len(files)}))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
