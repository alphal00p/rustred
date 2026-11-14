#!/usr/bin/env python3
"""Validator of the campaign JSON contract (WP-0a; AUTOMATION_PLAN 3.3, 3.P, 4.1).

Dependency-free standard-library Python 3. The checks are HAND-WRITTEN (no `jsonschema` package): a small interpreter
of the subset of JSON Schema draft 2020-12 that the `*.schema.json` files of this directory use, plus semantic checks
(digest forms, state combinations, the ledger hash chain with a pure-Python BLAKE3, cross-file consistency of a
campaign directory). The same module is imported by the website's loader and run as a subprocess by the Rust tests.

Usage:
  validate.py FILE...                 validate each file (schema chosen by its `schema` field, or --schema NAME)
  validate.py --schema NAME FILE...   force a schema (NAME = a schema file stem, e.g. `events-line`)
  validate.py --cdir DIR [--now UTC]  validate a whole campaign directory (spec, ledger, progress, records)
  validate.py --canon FILE            print the canonical JSON (plan 3.3.0) of a JSON document
  validate.py --blake3 FILE           print the BLAKE3 of a file's bytes
  validate.py --list                  list the known schemas
Exit codes: 0 valid, 1 invalid (schema or consistency error), 2 missing / unreadable input, 3 usage error.
Warnings (e.g. a STALE campaign, a chain too large to check here) never change the exit code.

Library API (the website imports these): `load_schemas()`, `validate_document(doc, schema=None) -> [errors]`,
`validate_file(path, schema=None) -> Report`, `validate_cdir(path, now=None) -> Report`, `canonical_json(value)`,
`parse_json_strict(text)`, `blake3_hex(bytes)`, `reader_status(doc)` (tolerant-reader verdict for one document).
"""

import datetime
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
VERSION = "rustred-campaign-schemas validate.py 1"
# Pure-Python BLAKE3 is slow (about 1 MB/s): the ledger chain is checked in full only below this many bytes.
CHAIN_CHECK_MAX_BYTES = int(os.environ.get("RR_VALIDATE_CHAIN_MAX", str(8 << 20)))
STALE_AFTER_S = 60

# ------------------------------------------------------------------------------------------------------------------
# BLAKE3 (pure Python, from the specification: 7-round compression, 1024-byte chunks, binary tree; hash mode only)
# ------------------------------------------------------------------------------------------------------------------

_IV = (0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19)
_PERM = (2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8)
_CHUNK_START, _CHUNK_END, _PARENT, _ROOT = 1, 2, 4, 8
_M32 = 0xFFFFFFFF


def _compress(cv, m, counter, block_len, flags):
    s = list(cv) + list(_IV[:4]) + [counter & _M32, (counter >> 32) & _M32, block_len, flags]
    m = list(m)

    def g(a, b, c, d, x, y):
        sa = (s[a] + s[b] + x) & _M32
        sd = s[d] ^ sa
        sd = ((sd >> 16) | (sd << 16)) & _M32
        sc = (s[c] + sd) & _M32
        sb = s[b] ^ sc
        sb = ((sb >> 12) | (sb << 20)) & _M32
        sa = (sa + sb + y) & _M32
        sd = sd ^ sa
        sd = ((sd >> 8) | (sd << 24)) & _M32
        sc = (sc + sd) & _M32
        sb = sb ^ sc
        sb = ((sb >> 7) | (sb << 25)) & _M32
        s[a], s[b], s[c], s[d] = sa, sb, sc, sd

    for r in range(7):
        g(0, 4, 8, 12, m[0], m[1])
        g(1, 5, 9, 13, m[2], m[3])
        g(2, 6, 10, 14, m[4], m[5])
        g(3, 7, 11, 15, m[6], m[7])
        g(0, 5, 10, 15, m[8], m[9])
        g(1, 6, 11, 12, m[10], m[11])
        g(2, 7, 8, 13, m[12], m[13])
        g(3, 4, 9, 14, m[14], m[15])
        if r < 6:
            m = [m[p] for p in _PERM]
    for i in range(8):
        s[i] ^= s[i + 8]
        s[i + 8] ^= cv[i]
    return s


def _words(block):
    block = block + b"\0" * (64 - len(block))
    return [int.from_bytes(block[i:i + 4], "little") for i in range(0, 64, 4)]


def blake3_hex(data):
    """Lower-case hex BLAKE3-256 of `data` (bytes)."""
    data = bytes(data)
    n_chunks = max(1, (len(data) + 1023) // 1024)
    stack = []
    out = None
    for ci in range(n_chunks):
        chunk = data[ci * 1024:(ci + 1) * 1024]
        blocks = [chunk[i:i + 64] for i in range(0, len(chunk), 64)] or [b""]
        cv = _IV
        for bi, blk in enumerate(blocks):
            flags = (_CHUNK_START if bi == 0 else 0) | (_CHUNK_END if bi == len(blocks) - 1 else 0)
            if bi < len(blocks) - 1:
                cv = _compress(cv, _words(blk), ci, 64, flags)[:8]
            else:
                out = (cv, _words(blk), ci, len(blk), flags)
        if ci < n_chunks - 1:
            ccv = _compress(*out)[:8]
            total = ci + 1
            while total & 1 == 0:
                ccv = _compress(_IV, stack.pop() + ccv, 0, 64, _PARENT)[:8]
                total >>= 1
            stack.append(ccv)
    while stack:
        right = _compress(*out)[:8]
        out = (_IV, stack.pop() + right, 0, 64, _PARENT)
    cv, m, counter, block_len, flags = out
    words = _compress(cv, m, counter, block_len, flags | _ROOT)[:8]
    return b"".join(w.to_bytes(4, "little") for w in words).hex()


# ------------------------------------------------------------------------------------------------------------------
# Strict JSON parsing and the canonical form of plan 3.3.0
# ------------------------------------------------------------------------------------------------------------------

class DuplicateKey(ValueError):
    pass


def _no_dups(pairs):
    d = {}
    for k, v in pairs:
        if k in d:
            raise DuplicateKey("duplicate JSON key %r" % k)
        d[k] = v
    return d


def _reject_constant(name):
    raise ValueError("non-standard JSON constant %s" % name)


def parse_json_strict(text):
    """json.loads that refuses duplicate keys (plan 3.3) and NaN / Infinity."""
    if isinstance(text, (bytes, bytearray)):
        text = bytes(text).decode("utf-8")
    return json.loads(text, object_pairs_hook=_no_dups, parse_constant=_reject_constant)


def _ryu_like(x):
    """A float the way serde_json (ryu) prints it: shortest round-trip digits, `e` without `+` or zero padding,
    integral values with `.0`. Only used for non-identifying documents (progress files)."""
    if x != x or x in (float("inf"), float("-inf")):
        raise ValueError("NaN / infinity is not JSON")
    r = repr(x)
    if "e" in r or "E" in r:
        mant, exp = r.lower().split("e")
        exp_i = int(exp)
        digits = mant.replace(".", "").lstrip("-")
        neg = mant.startswith("-")
        # ryu: decimal notation for exponents -5 <= e10 < 16 (e10 = exponent of the first digit)
        if -5 <= exp_i < 16:
            dot = mant.find(".")
            intlen = (dot if dot >= 0 else len(mant)) - (1 if neg else 0)
            point = intlen + exp_i
            digits = digits.rstrip("0") or "0"
            if point <= 0:
                s = "0." + "0" * (-point) + digits
            elif point >= len(digits):
                s = digits + "0" * (point - len(digits)) + ".0"
            else:
                s = digits[:point] + "." + digits[point:]
            return ("-" if neg else "") + s
        if mant.endswith(".0"):
            mant = mant[:-2]
        return "%se%d" % (mant, exp_i)
    if "." not in r:
        r += ".0"
    return r


def canonical_json(value, allow_floats=False, final_newline=False):
    """CANONICAL JSON of plan 3.3.0: UTF-8, object keys sorted by byte order, no whitespace, integers only (floats are
    refused unless `allow_floats`), strings escaped as serde_json does, arrays in their given order, a final newline
    only for files. Returns `str`."""
    out = []

    def esc(s):
        b = ['"']
        for ch in s:
            o = ord(ch)
            if ch == '"':
                b.append('\\"')
            elif ch == "\\":
                b.append("\\\\")
            elif o < 0x20:
                b.append({8: "\\b", 9: "\\t", 10: "\\n", 12: "\\f", 13: "\\r"}.get(o, "\\u%04x" % o))
            else:
                b.append(ch)
        b.append('"')
        return "".join(b)

    def emit(v):
        if v is None:
            out.append("null")
        elif v is True:
            out.append("true")
        elif v is False:
            out.append("false")
        elif isinstance(v, int):
            out.append(str(v))
        elif isinstance(v, float):
            if not allow_floats:
                raise ValueError("float %r in a canonical (identifying) document" % v)
            out.append(_ryu_like(v))
        elif isinstance(v, str):
            out.append(esc(v))
        elif isinstance(v, list):
            out.append("[")
            for i, e in enumerate(v):
                if i:
                    out.append(",")
                emit(e)
            out.append("]")
        elif isinstance(v, dict):
            out.append("{")
            for i, k in enumerate(sorted(v, key=lambda s: s.encode("utf-8"))):
                if i:
                    out.append(",")
                out.append(esc(k))
                out.append(":")
                emit(v[k])
            out.append("}")
        else:
            raise ValueError("not a JSON value: %r" % (v,))

    emit(value)
    if final_newline:
        out.append("\n")
    return "".join(out)


# ------------------------------------------------------------------------------------------------------------------
# Schema registry and the hand-written interpreter of the JSON Schema subset
# ------------------------------------------------------------------------------------------------------------------

_SCHEMAS = None  # stem -> schema object
_BY_NAME = None  # contract name (e.g. rustred.campaign.spec.v1) -> [(stem, selector dict)]


def load_schemas(directory=HERE):
    global _SCHEMAS, _BY_NAME
    if _SCHEMAS is not None and directory == HERE:
        return _SCHEMAS
    schemas, by_name = {}, {}
    for fn in sorted(os.listdir(directory)):
        if not fn.endswith(".schema.json"):
            continue
        with open(os.path.join(directory, fn), "rb") as f:
            s = parse_json_strict(f.read())
        stem = fn[: -len(".schema.json")]
        schemas[stem] = s
        for name in s.get("x-rr-names", []):
            by_name.setdefault(name, []).append((stem, s.get("x-rr-select", {})))
    if directory == HERE:
        _SCHEMAS, _BY_NAME = schemas, by_name
    return schemas


def _by_name():
    load_schemas()
    return _BY_NAME


_TYPES = {
    "object": lambda v: isinstance(v, dict),
    "array": lambda v: isinstance(v, list),
    "string": lambda v: isinstance(v, str),
    "integer": lambda v: isinstance(v, int) and not isinstance(v, bool),
    "number": lambda v: isinstance(v, (int, float)) and not isinstance(v, bool),
    "boolean": lambda v: isinstance(v, bool),
    "null": lambda v: v is None,
}


def _type_name(v):
    for t in ("null", "boolean", "integer", "number", "string", "array", "object"):
        if _TYPES[t](v):
            return t
    return type(v).__name__


def _json_eq(a, b):
    if isinstance(a, bool) or isinstance(b, bool):
        return isinstance(a, bool) and isinstance(b, bool) and a == b
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(_json_eq(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(_json_eq(x, y) for x, y in zip(a, b))
    return type(a) is type(b) and a == b or (_TYPES["number"](a) and _TYPES["number"](b) and a == b)


def _resolve(ref, base_stem):
    schemas = load_schemas()
    if ref.startswith("#"):
        stem, frag = base_stem, ref[1:]
    else:
        f, _, frag = ref.partition("#")
        stem = f[: -len(".schema.json")] if f.endswith(".schema.json") else f
    if stem not in schemas:
        raise KeyError("unresolvable $ref %r (schema %r unknown)" % (ref, stem))
    node = schemas[stem]
    for part in [p for p in frag.split("/") if p]:
        node = node[part.replace("~1", "/").replace("~0", "~")]
    return node, stem


def _ptr(path):
    return "/" + "/".join(str(p).replace("~", "~0").replace("/", "~1") for p in path) if path else "(root)"


def _check(inst, sch, path, stem, errs):
    """Appends (pointer, message) errors of `inst` against schema node `sch`."""
    if sch is True or sch == {}:
        return
    if sch is False:
        errs.append((_ptr(path), "no value allowed here"))
        return
    if "$ref" in sch:
        target, tstem = _resolve(sch["$ref"], stem)
        _check(inst, target, path, tstem, errs)
    t = sch.get("type")
    if t is not None:
        ts = t if isinstance(t, list) else [t]
        if not any(_TYPES[x](inst) for x in ts):
            errs.append((_ptr(path), "wrong type: %s, expected %s" % (_type_name(inst), " or ".join(ts))))
            return
    if "const" in sch and not _json_eq(inst, sch["const"]):
        errs.append((_ptr(path), "must be %s, found %s" % (json.dumps(sch["const"]), json.dumps(inst)[:80])))
    if "enum" in sch and not any(_json_eq(inst, e) for e in sch["enum"]):
        errs.append((_ptr(path), "%s is not one of %s" % (json.dumps(inst)[:80], json.dumps(sch["enum"])[:300])))
    if isinstance(inst, str):
        if "minLength" in sch and len(inst) < sch["minLength"]:
            errs.append((_ptr(path), "string shorter than %d" % sch["minLength"]))
        if "maxLength" in sch and len(inst) > sch["maxLength"]:
            errs.append((_ptr(path), "string longer than %d" % sch["maxLength"]))
        if "pattern" in sch and not re.search(sch["pattern"], inst):
            errs.append((_ptr(path), "%s does not match %s" % (json.dumps(inst)[:80], sch["pattern"])))
    if _TYPES["number"](inst):
        if "minimum" in sch and inst < sch["minimum"]:
            errs.append((_ptr(path), "%s is below the minimum %s" % (inst, sch["minimum"])))
        if "maximum" in sch and inst > sch["maximum"]:
            errs.append((_ptr(path), "%s is above the maximum %s" % (inst, sch["maximum"])))
        if "exclusiveMinimum" in sch and inst <= sch["exclusiveMinimum"]:
            errs.append((_ptr(path), "%s must be above %s" % (inst, sch["exclusiveMinimum"])))
        if "exclusiveMaximum" in sch and inst >= sch["exclusiveMaximum"]:
            errs.append((_ptr(path), "%s must be below %s" % (inst, sch["exclusiveMaximum"])))
    if isinstance(inst, list):
        pre = sch.get("prefixItems", [])
        for i, s in enumerate(pre[: len(inst)]):
            _check(inst[i], s, path + [i], stem, errs)
        if "items" in sch:
            for i in range(len(pre), len(inst)):
                _check(inst[i], sch["items"], path + [i], stem, errs)
        if "minItems" in sch and len(inst) < sch["minItems"]:
            errs.append((_ptr(path), "fewer than %d items" % sch["minItems"]))
        if "maxItems" in sch and len(inst) > sch["maxItems"]:
            errs.append((_ptr(path), "more than %d items" % sch["maxItems"]))
        if sch.get("uniqueItems"):
            seen = set()
            for e in inst:
                c = canonical_json(e, allow_floats=True)
                if c in seen:
                    errs.append((_ptr(path), "duplicate item %s" % c[:80]))
                seen.add(c)
    if isinstance(inst, dict):
        for k in sch.get("required", []):
            if k not in inst:
                errs.append((_ptr(path), "missing required field %r" % k))
        props = sch.get("properties", {})
        pats = sch.get("patternProperties", {})
        for k, v in inst.items():
            matched = False
            if k in props:
                matched = True
                _check(v, props[k], path + [k], stem, errs)
            for p, s in pats.items():
                if re.search(p, k):
                    matched = True
                    _check(v, s, path + [k], stem, errs)
            if not matched and "additionalProperties" in sch:
                ap = sch["additionalProperties"]
                if ap is False:
                    errs.append((_ptr(path + [k]), "unknown field %r (this object is closed)" % k))
                else:
                    _check(v, ap, path + [k], stem, errs)
        if "propertyNames" in sch:
            for k in inst:
                _check(k, sch["propertyNames"], path + [k], stem, errs)
        if "minProperties" in sch and len(inst) < sch["minProperties"]:
            errs.append((_ptr(path), "fewer than %d fields" % sch["minProperties"]))
        for k, deps in sch.get("dependentRequired", {}).items():
            if k in inst:
                for d in deps:
                    if d not in inst:
                        errs.append((_ptr(path), "field %r requires %r" % (k, d)))
    for s in sch.get("allOf", []):
        _check(inst, s, path, stem, errs)
    for key in ("anyOf", "oneOf"):
        if key in sch:
            branch_errs = []
            ok = 0
            for s in sch[key]:
                e = []
                _check(inst, s, path, stem, e)
                if not e:
                    ok += 1
                branch_errs.append(e)
            if ok == 0:
                best = min(branch_errs, key=len)
                errs.append((_ptr(path), "matches none of the %d alternatives (%s); closest:" % (len(sch[key]), key)))
                errs.extend(best[:6])
            elif key == "oneOf" and ok > 1:
                errs.append((_ptr(path), "matches %d alternatives of oneOf (exactly one required)" % ok))
    if "not" in sch:
        e = []
        _check(inst, sch["not"], path, stem, e)
        if not e:
            errs.append((_ptr(path), "matches a forbidden form"))
    if "if" in sch:
        e = []
        _check(inst, sch["if"], path, stem, e)
        branch = sch.get("then") if not e else sch.get("else")
        if branch is not None:
            _check(inst, branch, path, stem, errs)


# ------------------------------------------------------------------------------------------------------------------
# Schema selection and semantic checks
# ------------------------------------------------------------------------------------------------------------------

_NAME_RE = re.compile(r"^(?P<base>[a-z][a-z0-9.-]*?)\.v(?P<major>[0-9]+)$")


def _detect(doc):
    """Schema stem for documents without a `schema` field (contract files whose as-built form has none)."""
    if isinstance(doc, dict):
        if doc.get("v") is not None and "kind" in doc and "seq" in doc and "t" in doc:
            return "events-line", None
        job = doc.get("job")
        if isinstance(job, dict) and isinstance(job.get("schema"), str) and job["schema"].startswith("rustred.child-job.outcome."):
            return "outcome", None
        if {"program", "package_version", "contract"} <= doc.keys():
            return "version", None
        if "script" in doc and isinstance(doc.get("script"), list):
            return "fake-script", None
        if {"item", "record", "jobs", "why"} <= doc.keys():
            return "progress-item-detail", None
    return None, "no `schema` field and the document's form is not recognised (use --schema NAME)"


def select_schema(doc):
    """(stem, None) or (None, error message). Refuses unknown MAJOR versions of known contracts."""
    if not isinstance(doc, dict):
        return None, "a document must be a JSON object"
    name = doc.get("schema")
    if name is None:
        return _detect(doc)
    if not isinstance(name, str):
        return None, "`schema` is not a string"
    by = _by_name()
    cands = by.get(name)
    if cands:
        for stem, sel in cands:
            if all(doc.get(k) == v for k, v in sel.items()):
                return stem, None
        return None, "schema %s: no variant for %s" % (name, json.dumps({k: doc.get(k) for k in cands[0][1]}))
    m = _NAME_RE.match(name)
    if m:
        known = sorted({n for n in by if _NAME_RE.match(n) and _NAME_RE.match(n).group("base") == m.group("base")})
        if known:
            return None, "schema %s: major version %s not understood (this validator reads %s)" % (
                name, m.group("major"), ", ".join(known))
    return None, "unknown schema %r" % name


def _semantic(stem, doc, errs):
    f = _SEMANTIC.get(stem)
    if f:
        f(doc, errs)


def _sem_spec(doc, errs):
    ids = [n.get("id") for n in doc.get("nodes", [])]
    if len(ids) != len(set(ids)):
        errs.append(("/nodes", "node ids are not unique"))
    input_ids = []
    for i, n in enumerate(doc.get("nodes", [])):
        input_ids.append(n["family"]["id"])
        input_ids.extend(t["id"] for t in n.get("tables", []))
        if n.get("box") is not None:
            input_ids.append(n["box"]["id"])
    input_ids.extend("program:" + p for p in doc.get("programs", {}))
    dup = sorted({x for x in input_ids if input_ids.count(x) > 1})
    if dup:
        errs.append(("/nodes", "input ids used twice: %s" % ", ".join(dup)))
    prio = doc.get("resources", {}).get("node_priority", [])
    unknown = [p for p in prio if p not in ids]
    if unknown:
        errs.append(("/resources/node_priority", "names nodes not in the spec: %s" % ", ".join(unknown)))


def _sem_accept(doc, errs):
    lvl = doc.get("level_after")
    ev = doc.get("evidence_level")
    want = {"A0": None, "A1": "geometry_accepted", "A2": "identity_tested", "A3": "identity_proved",
            "A4": "identity_proved"}.get(lvl)
    if ev != want:
        errs.append(("/evidence_level", "level_after %s maps to ledger word %s, found %s" % (lvl, want, ev)))
    v = doc.get("verdict", {})
    if v.get("normalised") == "FAIL" and not v.get("disproof"):
        errs.append(("/verdict/disproof", "FAIL only with a stored disproof (plan 2.5.3)"))
    if v.get("normalised") != "PASS" and lvl in ("A2", "A3", "A4") and doc.get("imported_from") is None:
        errs.append(("/level_after", "a %s verdict cannot raise the level to %s" % (v.get("normalised"), lvl)))
    chk = doc.get("check", {})
    prod = doc.get("rule", {}).get("producer", {})
    # an accept line whose producer digest is unknown (historical import) can never BY ITSELF yield a level above A0
    # (imported, provenance only); only a check of our own (not kind `imported`, no `imported_from`) can raise it
    if prod.get("bin_blake3", "") is None and (chk.get("kind") == "imported" or doc.get("imported_from") is not None) \
            and (lvl != "A0" or ev is not None):
        errs.append(("/level_after", "producer digest unknown (%s): an imported line cannot raise the level above A0, found %s"
                     % (prod.get("provenance"), lvl)))
    lift_v = prod.get("lift_primes")
    lift = set(lift_v if isinstance(lift_v, list) else [])
    if lift & set(chk.get("primes", [])):
        errs.append(("/check/primes", "verification prime inside the rule's lift-prime set (plan 6.7)"))


def _sem_progress_node(doc, errs):
    ck, phase, target, box = doc.get("closure_kind"), doc.get("phase"), doc.get("target") or {}, doc.get("spec_box")
    closed = phase in ("closed", "certified")
    if ck == "boxed":
        if target.get("target") != "boxed" or not closed:
            errs.append(("/closure_kind", "BOXED needs target `boxed` and phase closed/certified"))
        if box is None:
            errs.append(("/spec_box", "BOXED without a box id (only the user-provided spec box may be used, R35)"))
    if target.get("target") == "boxed" and target.get("box_id") != box:
        errs.append(("/target/box_id", "box target %r differs from the spec box %r" % (target.get("box_id"), box)))
    if ck == "full" and (target.get("target") != "full" or not closed):
        errs.append(("/closure_kind", "FULL needs target `full` and phase closed/certified"))
    if ck == "open" and closed:
        errs.append(("/closure_kind", "phase %s is closed: closure_kind must be full or boxed" % phase))
    if doc.get("certified") != (phase == "certified"):
        errs.append(("/certified", "certified must equal (phase == certified)"))
    t = doc.get("terminals", {})
    if isinstance(t.get("declared"), int) and isinstance(t.get("bound"), int) and t["declared"] > t["bound"]:
        errs.append(("/terminals/declared", "more terminals than the node's bound"))


def _sem_progress_campaign(doc, errs):
    n = doc.get("nodes", {})
    if isinstance(n.get("by_closure"), dict) and isinstance(n.get("total"), int):
        if sum(n["by_closure"].values()) != n["total"]:
            errs.append(("/nodes/by_closure", "full + boxed + open != nodes.total"))
        if sum(n.get("by_phase", {}).values()) != n["total"]:
            errs.append(("/nodes/by_phase", "sum of node phases != nodes.total"))
    it = doc.get("items", {})
    if isinstance(it.get("by_phase"), dict) and sum(it["by_phase"].values()) != it.get("total"):
        errs.append(("/items/by_phase", "sum of item phases != items.total"))
    j = doc.get("jobs", {})
    if isinstance(j.get("by_state"), dict) and sum(j["by_state"].values()) != j.get("total"):
        errs.append(("/jobs/by_state", "sum of job states != jobs.total"))


def _sem_terminals(doc, errs):
    for i, n in enumerate(doc.get("nodes", [])):
        ids = [t.get("id") for t in n.get("terminals", [])]
        if len(ids) != len(set(ids)):
            errs.append(("/nodes/%d/terminals" % i, "terminal ids not unique"))
        if n.get("provisional") is False and n.get("certification") != "certified":
            errs.append(("/nodes/%d/provisional" % i, "provisional: false only with certification `certified` (3.P.6)"))


def _sem_outcome(doc, errs):
    job = doc.get("job", {})
    if job.get("time_decided") != (bool(job.get("time_decisions")) or (job.get("stop") or {}).get("code") == "budget_wall"):
        errs.append(("/job/time_decided", "time_decided iff time_decisions non-empty or stop budget_wall (child-job 3.7)"))
    table = (job.get("outputs") or {}).get("table")
    if table and table.get("sealed"):
        for k in ("bytes", "blake3", "end"):
            if table.get(k) is None:
                errs.append(("/job/outputs/table/" + k, "a sealed table names bytes, blake3 and end"))
        if (table.get("end") == "end") != ((job.get("stop") or {}).get("code") is None):
            errs.append(("/job/outputs/table/end", "end record `end` iff the stop is null"))


def _sem_ledger_record(doc, errs):
    if doc.get("seq") == 1 and doc.get("prev") != "0" * 64:
        errs.append(("/prev", "record 1 must have prev = 64 zeros"))
    ev = doc.get("event", {})
    if ev.get("kind") == "campaign_created" and doc.get("seq") != 1:
        errs.append(("/event/kind", "campaign_created only as record 1"))
    body = ev.get("body") if isinstance(ev, dict) else None
    if isinstance(body, dict) and ev.get("kind") == "item":
        e = body.get("ev", {})
        if e.get("ev") == "accept" and e.get("level") != "geometry_accepted" and not e.get("evidence"):
            errs.append(("/event/body/ev/evidence", "a level above geometry_accepted cites its evidence artefacts (audit A M3)"))
        if e.get("ev") == "accept" and e.get("elided") is True:
            errs.append(("/event/body/ev/elided", "a record with elided witness loci is unusable (ruling R-f)"))
        if e.get("ev") == "retire" and (e.get("proof") or {}).get("kind") == "replaced_by" and not e.get("superseded_by"):
            errs.append(("/event/body/ev/superseded_by", "replaced_by needs superseded_by (ledger 5.6)"))


_SEMANTIC = {
    "campaign-spec": _sem_spec,
    "accept": _sem_accept,
    "progress-node": _sem_progress_node,
    "progress-campaign": _sem_progress_campaign,
    "terminals": _sem_terminals,
    "outcome": _sem_outcome,
    "ledger-record": _sem_ledger_record,
}


def validate_document(doc, schema=None):
    """List of error strings (empty = valid). `schema` = a stem to force; else chosen by the `schema` field."""
    load_schemas()
    if schema is None:
        stem, why = select_schema(doc)
        if stem is None:
            return [why]
    else:
        stem = schema
        if stem not in load_schemas():
            return ["unknown schema stem %r (see --list)" % stem]
    errs = []
    try:
        _check(doc, load_schemas()[stem], [], stem, errs)
    except (KeyError, IndexError, TypeError, re.error) as e:
        return ["schema %s is broken: %s" % (stem, e)]
    if not errs:
        _semantic(stem, doc, errs)
    return ["%s: %s" % (p, m) for p, m in errs]


def reader_status(doc):
    """Tolerant-reader verdict for the website: ('ok' | 'invalid' | 'unknown_major' | 'unknown', message)."""
    stem, why = select_schema(doc)
    if stem is None:
        if why and "major version" in why:
            return "unknown_major", why
        return "unknown", why
    errs = validate_document(doc, stem)
    return ("ok", stem) if not errs else ("invalid", "; ".join(errs[:5]))


# ------------------------------------------------------------------------------------------------------------------
# Files and campaign directories
# ------------------------------------------------------------------------------------------------------------------

class Report:
    def __init__(self):
        self.errors = []
        self.warnings = []
        self.missing = []
        self.checked = 0

    def err(self, where, msg):
        self.errors.append("%s: %s" % (where, msg))

    def warn(self, where, msg):
        self.warnings.append("%s: %s" % (where, msg))

    @property
    def exit_code(self):
        if self.missing:
            return 2
        return 1 if self.errors else 0

    def merge(self, other):
        self.errors += other.errors
        self.warnings += other.warnings
        self.missing += other.missing
        self.checked += other.checked


def _read(path, rep):
    try:
        with open(path, "rb") as f:
            return f.read()
    except OSError as e:
        rep.missing.append("%s: %s" % (path, e.strerror or e))
        return None


def _jsonl_lines(data, where, rep, torn_ok):
    """[(line_number, bytes)] of complete lines; a final line without newline is torn (ignored by readers)."""
    lines = data.split(b"\n")
    tail = lines.pop()
    if tail:
        (rep.warn if torn_ok else rep.err)(where, "torn last line (%d bytes without newline) ignored" % len(tail))
    return [(i + 1, l) for i, l in enumerate(lines)]


def validate_file(path, schema=None):
    rep = Report()
    data = _read(path, rep)
    if data is None:
        return rep
    name = os.path.basename(path)
    is_jsonl = name.endswith(".jsonl")
    if is_jsonl:
        forced = schema
        if forced is None and (name.startswith("segment-") or name == "LEDGER.jsonl"):
            forced = "ledger-record"
        if forced is None and name.upper().startswith("EVENTS"):
            forced = "events-line"
        for n, line in _jsonl_lines(data, path, rep, torn_ok=True):
            if not line.strip():
                rep.err("%s:%d" % (path, n), "empty line")
                continue
            try:
                doc = parse_json_strict(line)
            except ValueError as e:
                rep.err("%s:%d" % (path, n), "not JSON: %s" % e)
                continue
            rep.checked += 1
            for e in validate_document(doc, forced):
                rep.err("%s:%d" % (path, n), e)
        return rep
    try:
        doc = parse_json_strict(data)
    except ValueError as e:
        rep.err(path, "not JSON: %s" % e)
        return rep
    rep.checked += 1
    for e in validate_document(doc, schema):
        rep.err(path, e)
    return rep


def _parse_utc(s):
    m = re.match(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?Z$", s or "")
    if not m:
        return None
    y, mo, d, h, mi, se = (int(x) for x in m.groups()[:6])
    frac = m.group(7) or "0"
    return datetime.datetime(y, mo, d, h, mi, se, tzinfo=datetime.timezone.utc) + datetime.timedelta(
        microseconds=int(frac.ljust(6, "0")[:6]))


def _check_fileref(cdir, ref, where, rep):
    """A FileRef {path, bytes, blake3} relative to CDIR must exist with that size and digest; `large:` paths live on
    the large root (not checked here; listed as a warning once)."""
    p = ref.get("path", "")
    if p.startswith("large:") or p.startswith("wdir:"):
        return "large"
    if os.path.isabs(p) or ".." in p.split("/"):
        rep.err(where, "FileRef path %r must be relative to CDIR without `..`" % p)
        return "bad"
    full = os.path.join(cdir, p)
    try:
        with open(full, "rb") as f:
            data = f.read()
    except OSError:
        rep.err(where, "referenced file %s is missing" % p)
        return "missing"
    if len(data) != ref.get("bytes"):
        rep.err(where, "referenced file %s has %d bytes, the record says %s" % (p, len(data), ref.get("bytes")))
    if len(data) <= CHAIN_CHECK_MAX_BYTES and blake3_hex(data) != ref.get("blake3"):
        rep.err(where, "referenced file %s: BLAKE3 differs from the record (bad digest)" % p)
    return "ok"


def _walk_filerefs(v, out):
    if isinstance(v, dict):
        if set(v.keys()) == {"path", "bytes", "blake3"}:
            out.append(v)
        for x in v.values():
            _walk_filerefs(x, out)
    elif isinstance(v, list):
        for x in v:
            _walk_filerefs(x, out)


def _validate_ledger(cdir, rep):
    """Ledger of record (as built: ledger/MANIFEST.json + segments): schema of every line, dense seq, the `prev` hash
    chain (pure-Python BLAKE3 below CHAIN_CHECK_MAX_BYTES), sealed-segment digests, manifest links."""
    ldir = os.path.join(cdir, "ledger")
    mpath = os.path.join(ldir, "MANIFEST.json")
    mdata = _read(mpath, rep)
    if mdata is None:
        return []
    try:
        man = parse_json_strict(mdata)
    except ValueError as e:
        rep.err(mpath, "not JSON: %s" % e)
        return []
    for e in validate_document(man, "ledger-manifest"):
        rep.err(mpath, e)
    rep.checked += 1
    if rep.errors:
        return []
    segs = [(s, True) for s in man["sealed"]] + [(man["active"], False)]
    total = 0
    for s, _ in segs:
        try:
            total += os.path.getsize(os.path.join(ldir, s["file"]))
        except OSError:
            pass
    chain = total <= CHAIN_CHECK_MAX_BYTES
    if not chain:
        rep.warn(ldir, "ledger has %d bytes: hash chain not recomputed here (pure-Python BLAKE3); use `rr_campaign "
                       "verify`; seq density and links still checked" % total)
    records = []
    prev_hash = "0" * 64
    expect_seq = 1
    for s, sealed in segs:
        spath = os.path.join(ldir, s["file"])
        data = _read(spath, rep)
        if data is None:
            return records
        if sealed:
            if len(data) != s["bytes"]:
                rep.err(spath, "sealed segment has %d bytes, manifest says %d" % (len(data), s["bytes"]))
            if chain and blake3_hex(data) != s["blake3"]:
                rep.err(spath, "sealed segment BLAKE3 differs from the manifest (tampered or truncated)")
        else:
            if s["first_seq"] != expect_seq:
                rep.err(mpath, "active segment first_seq %d, expected %d" % (s["first_seq"], expect_seq))
            if s["prev"] != prev_hash and chain:
                rep.err(mpath, "active segment prev does not continue the chain")
        first = True
        for n, line in _jsonl_lines(data, spath, rep, torn_ok=not sealed):
            where = "%s:%d" % (spath, n)
            try:
                rec = parse_json_strict(line)
            except ValueError as e:
                rep.err(where, "not JSON: %s" % e)
                return records
            rep.checked += 1
            for e in validate_document(rec, "ledger-record"):
                rep.err(where, e)
            if rec.get("seq") != expect_seq:
                rep.err(where, "seq %s, expected %d (seq must be dense from 1)" % (rec.get("seq"), expect_seq))
            if sealed and first and rec.get("seq") != s["first_seq"]:
                rep.err(where, "first seq differs from the manifest")
            first = False
            # line framing: `h` = BLAKE3 of the line without its `,"h":"..."` suffix (= the canonical record bytes)
            body = line[:-72] + b"}" if isinstance(line, bytes) else (line[:-72] + "}").encode()
            raw = line if isinstance(line, bytes) else line.encode()
            if not raw.endswith(b'"}') or raw[-72:-66] != b',"h":"':
                rep.err(where, "record lacks its digest suffix")
            if chain:
                if blake3_hex(body) != rec.get("h"):
                    rep.err(where, "record digest h does not match the line (altered record: broken hash chain)")
                if rec.get("prev") != prev_hash:
                    rep.err(where, "broken hash chain: prev %s..., expected %s..." % (str(rec.get("prev"))[:12], prev_hash[:12]))
            prev_hash = rec.get("h") if chain else rec.get("prev")
            expect_seq += 1
            records.append(rec)
        if sealed:
            if records and records[-1]["seq"] != s["last_seq"]:
                rep.err(spath, "last seq differs from the manifest")
            if chain and prev_hash != s["last_hash"]:
                rep.err(spath, "manifest last_hash differs from the hash of the segment's last record")
    if chain:
        _LAST_HASH[cdir] = prev_hash
    return records


_LAST_HASH = {}


def _derive_counts(records):
    """Small re-derivation from the ledger for cross-checks of the progress files."""
    c = {"items": set(), "nodes": {}, "jobs": set(), "campaign_id": None, "spec_blake3": None, "phase": None,
         "last_seq": 0, "last_utc": None, "accepted": 0, "rejected": 0, "retired": 0, "terminals": 0}
    for r in records:
        ev, body = r["event"]["kind"], r["event"].get("body", {})
        c["last_seq"], c["last_utc"] = r["seq"], r["utc"]
        if ev == "campaign_created":
            c["campaign_id"], c["spec_blake3"] = body["campaign_id"], body["spec_blake3"]
        elif ev == "campaign":
            c["phase"] = {"start": "starting", "run": "running", "drain": "draining", "stop": "stopped",
                          "finish": "finished"}[body["ev"]]
        elif ev == "node_registered":
            c["nodes"][body["node"]] = body
        elif ev == "item_created":
            c["items"].add(body["item"])
        elif ev == "job_queued":
            c["jobs"].add(body["job"])
        elif ev == "item":
            e = body["ev"]["ev"]
            if e == "accept":
                c["accepted"] += 1
            elif e == "reject":
                c["rejected"] += 1
            elif e == "retire":
                c["retired"] += 1
            elif e == "declare_terminal":
                c["terminals"] += body["ev"]["count"]
    return c


def validate_cdir(cdir, now=None):
    """Whole campaign directory: spec.json, the ledger, every JSON file under progress/, accepted/, terminals/,
    control/, jobs/, LOCK.json; FileRefs of the ledger resolved; progress counts against the ledger; staleness."""
    rep = Report()
    if not os.path.isdir(cdir):
        rep.missing.append("%s: not a directory" % cdir)
        return rep
    spec_path = os.path.join(cdir, "spec.json")
    spec_bytes = _read(spec_path, rep)
    if spec_bytes is not None:
        rep.merge(validate_file(spec_path, "campaign-spec"))
    records = _validate_ledger(cdir, rep)
    counts = _derive_counts(records) if records else None
    if counts and spec_bytes is not None and counts["spec_blake3"] != blake3_hex(spec_bytes):
        rep.err(spec_path, "BLAKE3 of spec.json differs from campaign_created.spec_blake3 (bad digest)")
    # FileRefs named by the ledger
    large = 0
    for r in records:
        refs = []
        _walk_filerefs(r["event"].get("body", {}), refs)
        for ref in refs:
            if _check_fileref(cdir, ref, "ledger seq %d" % r["seq"], rep) == "large":
                large += 1
    if large:
        rep.warn(cdir, "%d FileRefs on the large root (`large:`) not checked here" % large)
    # every other JSON document
    for sub in ("progress", "accepted", "terminals", "control", "jobs", "reports"):
        d = os.path.join(cdir, sub)
        for root, _dirs, files in os.walk(d):
            for fn in sorted(files):
                if fn.startswith("."):
                    continue
                p = os.path.join(root, fn)
                if fn.endswith(".json") or fn.endswith(".jsonl"):
                    rep.merge(validate_file(p))
    lock = os.path.join(cdir, "LOCK.json")
    if os.path.exists(lock):
        rep.merge(validate_file(lock, "lock"))
    # progress against the ledger
    pdir = os.path.join(cdir, "progress")
    camp_path = os.path.join(pdir, "campaign.json")
    if os.path.exists(camp_path) and counts:
        try:
            with open(camp_path, "rb") as f:
                camp = parse_json_strict(f.read())
        except ValueError:
            camp = None
        if isinstance(camp, dict):
            led = camp.get("ledger", {})
            if led.get("seq", 0) > counts["last_seq"]:
                rep.err(camp_path, "progress ahead of the ledger: seq %s > ledger end %d" % (led.get("seq"), counts["last_seq"]))
            elif led.get("seq") == counts["last_seq"]:
                for key, got, want in (("items.total", camp.get("items", {}).get("total"), len(counts["items"])),
                                       ("nodes.total", camp.get("nodes", {}).get("total"), len(counts["nodes"])),
                                       ("jobs.total", camp.get("jobs", {}).get("total"), len(counts["jobs"])),
                                       ("rules.accepted", camp.get("rules", {}).get("accepted"), counts["accepted"]),
                                       ("rules.rejected", camp.get("rules", {}).get("rejected"), counts["rejected"]),
                                       ("economy.retired", camp.get("economy", {}).get("retired"), counts["retired"]),
                                       ("terminals.declared", camp.get("terminals", {}).get("declared"), counts["terminals"]),
                                       ("campaign_id", camp.get("campaign_id"), counts["campaign_id"]),
                                       ("campaign.phase", camp.get("campaign", {}).get("phase"), counts["phase"]),
                                       ("ledger.last_utc", led.get("last_utc"), counts["last_utc"])):
                    if got != want:
                        rep.err(camp_path, "%s = %s, the ledger gives %s" % (key, json.dumps(got), json.dumps(want)))
                if _LAST_HASH.get(cdir) and led.get("head") != _LAST_HASH[cdir]:
                    rep.err(camp_path, "ledger.head is not the hash of the ledger's last record")
            nodes_files = (camp.get("files") or {}).get("nodes") or {}
            for node in counts["nodes"]:
                rel = nodes_files.get(node)
                if rel is None:
                    rep.err(camp_path, "files.nodes lacks node %s" % node)
                elif not os.path.exists(os.path.join(pdir, rel)):
                    rep.err(camp_path, "node file %s missing" % rel)
            # staleness (plan 3.P.2: STALE when the heartbeat is older than 60 s and the state is running)
            hb = (camp.get("supervisor") or {}).get("heartbeat_utc") or (camp.get("resources") or {}).get("sampled_utc") \
                or camp.get("written_utc")
            t_hb = _parse_utc(hb)
            t_now = _parse_utc(now) if now else datetime.datetime.now(datetime.timezone.utc)
            phase = (camp.get("campaign") or {}).get("phase")
            if t_hb and t_now and phase in ("starting", "running", "draining") and (t_now - t_hb).total_seconds() > STALE_AFTER_S:
                rep.warn(camp_path, "STALE: campaign %s but its heartbeat %s is %d s old" % (
                    phase, hb, int((t_now - t_hb).total_seconds())))
    return rep


# ------------------------------------------------------------------------------------------------------------------
# Command line
# ------------------------------------------------------------------------------------------------------------------

def _print_report(rep, quiet=False):
    for w in rep.warnings:
        print("warning: " + w)
    for m in rep.missing:
        print("missing: " + m)
    for e in rep.errors:
        print("error: " + e)
    if not quiet:
        state = {0: "VALID", 1: "INVALID", 2: "MISSING"}[rep.exit_code]
        print("%s (%d documents checked, %d errors, %d warnings)" % (state, rep.checked, len(rep.errors), len(rep.warnings)))


def main(argv):
    args = list(argv)
    schema = None
    now = None
    quiet = False
    if "--quiet" in args:
        args.remove("--quiet")
        quiet = True
    if "--now" in args:
        i = args.index("--now")
        now = args[i + 1]
        del args[i:i + 2]
    if "--schema" in args:
        i = args.index("--schema")
        schema = args[i + 1]
        del args[i:i + 2]
    if not args or args[0] in ("-h", "--help"):
        print(__doc__)
        return 3 if not args else 0
    if args[0] == "--version":
        print(VERSION)
        return 0
    if args[0] == "--list":
        for stem, s in sorted(load_schemas().items()):
            print("%-24s %s" % (stem, ", ".join(s.get("x-rr-names", [])) or "(detected by form)"))
        return 0
    if args[0] == "--canon":
        rep = Report()
        data = _read(args[1], rep)
        if data is None:
            _print_report(rep)
            return 2
        sys.stdout.write(canonical_json(parse_json_strict(data), allow_floats="--floats" in args, final_newline=True))
        return 0
    if args[0] == "--blake3":
        rep = Report()
        data = _read(args[1], rep)
        if data is None:
            return 2
        print(blake3_hex(data))
        return 0
    if args[0] == "--cdir":
        if len(args) < 2:
            print("usage: validate.py --cdir DIR [--now UTC]")
            return 3
        rep = Report()
        for d in args[1:]:
            rep.merge(validate_cdir(d, now))
        _print_report(rep, quiet)
        return rep.exit_code
    rep = Report()
    for p in args:
        rep.merge(validate_file(p, schema))
    _print_report(rep, quiet)
    return rep.exit_code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
