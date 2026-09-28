#!/usr/bin/env python
"""Map W0.8 knob runs to the session that ran them.

A session is one sessions/<name>.log written by local_session.sh,
socket1_session.sh or socket1_session2.sh; each run line carries
`--family F --label L`. Runs of one arm can come from several sessions
(e.g. c6-release r1-3, r4-6 and r7-10), so a ratio against a reference arm
must use the reference runs of the same session(s) as the arm's runs, not
the pooled reference.

  session_map.py [--sessions DIR] [LABEL_GLOB ...]   # print label family session
"""
import argparse
import fnmatch
import re
from pathlib import Path

SESSIONS = Path("/common/dev/rustred/TMP/w0/knobs/sessions")
RUN_LINE = re.compile(r"^\S+Z run\b.*?--family (\S+) --label (\S+)")


def load(sessions_dir=SESSIONS):
    """{(label, family): session name}. A (label, family) seen in two
    sessions (a rerun) maps to the later log line's session."""
    out = {}
    for log in sorted(Path(sessions_dir).glob("*.log")):
        for line in log.read_text(errors="replace").splitlines():
            m = RUN_LINE.match(line)
            if m:
                out[(m.group(2), m.group(1))] = log.stem
    return out


def same_session(rows, ref_rows):
    """Reference rows from the sessions of `rows` (each row carries a
    'session' key). Falls back to all reference rows, flagged pooled, when
    no session is shared."""
    sessions = {r.get("session") for r in rows}
    matched = [r for r in ref_rows if r.get("session") in sessions]
    return (matched, False) if matched else (ref_rows, True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--sessions", default=str(SESSIONS))
    p.add_argument("labels", nargs="*", default=["*"])
    args = p.parse_args()
    for (label, family), session in sorted(load(args.sessions).items()):
        if any(fnmatch.fnmatch(label, g) for g in args.labels):
            print(label, family, session)


if __name__ == "__main__":
    main()
