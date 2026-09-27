import sys, os, json, io, runpy
# usage: sample_wrap.py FILE NCHUNKS CHUNK_BYTES OUT
path, nchunks, chunk, out = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
size = os.path.getsize(path)
tmp = out + ".sample.jsonl"
with open(path, "rb") as fh, open(tmp, "wb") as w:
    for k in range(nchunks):
        off = int(size * k / nchunks)
        fh.seek(off)
        if off:
            fh.readline()
        data = fh.read(chunk)
        cut = data.rfind(b"\n")
        w.write(data[:cut + 1])
print(tmp)
