"""P1-P8 of the first host step (RUNBOOK 1.7), as a module (lane procfix7 round 8b: host step 1 ended without a verdict;
the grading is now testable on a recorded run: test_grade_host_step.py). Standard library only.

CLI: grade_host_step.py OUT CID EXIT VERIFY MARKED SCOPES_AFTER MODE CPU_LEDGER LEDGER_OFFSET WS
writes OUT/pass.txt and exits 0 when no criterion FAILs.
     grade_host_step.py --abort-check CDIR CID
prints the ABORT findings of a campaign directory so far (the watcher's check; empty = none) and exits 0.

procfix7 round 9 (gate13 m13-p5-lenient): every signal_sent record is checked against this campaign's OWN records
(signal_problems): a group signal must name the LEADER pid of an attempt of THE SAME job, after that attempt's start; a
process signal must carry the start time its identity check read and a marker of this campaign naming the record's job
(attempt marker / campaign marker / stop hold), or for "pinned session" an attempt of that job that has started; a
"stop hold" signal also needs an earlier SIGSTOP of ours to the same pid and start time; a cgroup kill must name a scope
unit this campaign launched. A job label alone is no longer enough. The P5 criterion and the watcher's ABORT test use
the same function.

procfix7 round 9b: (gate13b m13b-p5-slack) a group signal recorded AFTER its attempt's finished record is not accounted
for (no tolerance; it never happened in 1,728 recorded group signals); (gate13b judgement) `unproven_end` notes are also
reported as a NAMED CLASS, line "P5-unproven" (verdict INFO: it never changes the result), next to the strict P5: how many,
whether each was cleared later (unverified_end_cleared of the same attempt), and whether they are the ONLY cause of a P5
FAIL; (the supervisor as a service unit) P1 names the unit's end as systemd-run --wait reported it, P8 accounts for the
step's OWN units (the supervisor's service, the pre-flight's scope) by their exact names."""

import datetime
import glob
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import p8_units  # noqa: E402


def cpus(s):
    r = set()
    for part in re.split(r"[ ,]+", str(s).strip()):
        m = re.fullmatch(r"(\d+)-(\d+)", part)
        if m:
            r |= set(range(int(m[1]), int(m[2]) + 1))
        elif part.isdigit():
            r.add(int(part))
    return r


def norm(r):
    # a ledger line: {"seq", "utc", "prev", "event": {"kind": K, "body": {...}}, "h"}; a job event's body carries
    # {"job", "attempt", "ev": {"ev": "started", "pid": ...}}
    e = r.get("event", r)
    body = e.get("body") if isinstance(e.get("body"), dict) else {}
    d = dict(body)
    d["kind"] = e.get("kind")
    if d["kind"] == "job" and isinstance(body.get("ev"), dict):
        d["ev"] = body["ev"].get("ev")
        d["pid"] = body["ev"].get("pid")
    return d


ALLOWED_PROOFS = {"pinned group", "pinned session", "attempt marker", "campaign marker", "stop hold", "job scope",
                  "job cgroup"}
# procfix7 round 9b (gate13b m13b-p5-slack): no tolerance after the attempt's finished record (the 30 s of round 9 were
# never used: 0 of 1,728 group signals); each late record is named
FINISH_SLACK_S = 0


def slurp(path, mode="r"):
    """The whole file (closed at once)."""
    with open(path, mode, **({} if "b" in mode else {"errors": "replace"})) as f:
        return f.read()


def lines_of(path):
    return slurp(path).splitlines()


def raw_records(cdir):
    """[(seq, utc, event)] of the ledger segments of a campaign directory, in order."""
    out = []
    for f in sorted(glob.glob(cdir + "/ledger/segment-*.jsonl")):
        for line in lines_of(f):
            try:
                r = json.loads(line)
            except ValueError:
                continue
            e = r.get("event", r)
            if isinstance(e, dict):
                out.append((r.get("seq", len(out) + 1), r.get("utc"), e))
    return out


def _t(utc):
    try:
        return datetime.datetime.fromisoformat(str(utc).replace("Z", "+00:00"))
    except ValueError:
        return None


def signal_problems(raw, cid):
    """The signal_sent records of `raw` (raw_records) that this campaign's own records do not account for."""
    att = {}  # (job, attempt) -> {launch, unit, pid, start, start_ticks, end, end_utc}
    launches = {}  # job -> sorted launch seqs
    stops = set()  # (pid, start_ticks) of SIGSTOPs of ours to a process
    bad = []
    for seq, utc, e in raw:
        body = e.get("body") if isinstance(e.get("body"), dict) else {}
        if e.get("kind") == "job" and isinstance(body.get("ev"), dict):
            ev = body["ev"]
            k = (body.get("job"), body.get("attempt"))
            a = att.setdefault(k, {})
            if ev.get("ev") == "launch":
                a["launch"] = seq
                a["unit"] = ev.get("scope_unit")
                launches.setdefault(k[0], []).append(seq)
            elif ev.get("ev") == "started":
                a["pid"], a["start"], a["start_ticks"] = ev.get("pid"), seq, ev.get("start_ticks")
            elif ev.get("ev") == "finished":
                a["end"], a["end_utc"] = seq, _t(utc)
            continue
        if e.get("kind") != "signal_sent":
            continue
        s = body
        proof, target, job, pid = s.get("proof"), s.get("target"), s.get("job"), s.get("pid")
        marker, by, st = s.get("marker") or "", s.get("by") or "", s.get("start_ticks")
        why = None
        if proof not in ALLOWED_PROOFS:
            why = "unexpected proof"
        elif target == "group":
            def fits(k, a):
                if k[0] != job or a.get("pid") != pid or a.get("start") is None or seq <= a["start"]:
                    return False
                later = [x for x in launches.get(job, []) if x > a.get("launch", 0)]
                if later and seq > later[0]:
                    return False  # the job's next attempt was launched: this leader was reaped long before
                if a.get("end") is not None and seq > a["end"]:
                    if FINISH_SLACK_S <= 0:
                        return False
                    te, ts = a.get("end_utc"), _t(utc)
                    return te is not None and ts is not None and (ts - te).total_seconds() <= FINISH_SLACK_S
                return True
            if proof != "pinned group" or not any(fits(k, a) for k, a in att.items()):
                late = [k for k, a in att.items() if k[0] == job and a.get("pid") == pid and a.get("end") is not None
                        and seq > a["end"]]
                why = ("group signal: recorded after its attempt's finished record (attempt "
                       f"{late[0][0]}:a{late[0][1]})" if late else
                       "group signal: not the leader of a started attempt of that job")
        elif target == "process":
            mine = marker.startswith(cid + ":")
            mjob = marker[len(cid) + 1:].rsplit(":a", 1)[0] if mine else None
            if st is None:
                why = "process signal without the start time its identity check read"
            elif proof in ("attempt marker", "campaign marker"):
                if not mine or (job is not None and job != mjob):
                    why = "process signal: the marker read is not this campaign's or names another job"
                elif proof == "attempt marker" and not any(k[0] == mjob for k in att):
                    why = "process signal: the marker names no job of this campaign"
            elif proof == "pinned session":
                if marker and not mine:
                    why = "pinned-session signal: a marker of another campaign"
                elif not any(k[0] == job and a.get("start") is not None and seq > a["start"] for k, a in att.items()):
                    why = "pinned-session signal: no started attempt of that job"
            elif proof == "stop hold":
                if (pid, st) not in stops or (marker and not mine):
                    why = "stop-hold signal: no earlier SIGSTOP of ours to that pid and start time"
            else:
                why = "unexpected proof for a process"
            if why is None and s.get("signal") == 19:
                stops.add((pid, st))
        elif target == "cgroup":
            units = {a.get("unit") for a in att.values() if a.get("unit")}
            if proof == "job scope":
                if not any(u + ".scope" in by for u in units):
                    why = "scope kill: names no scope unit this campaign launched"
            elif job is None or job not in by and cid not in by:
                why = "cgroup kill: names no job cgroup of this campaign"
        else:
            why = "unknown target"
        if why:
            bad.append(f"signal_sent seq {seq} pid {pid} proof {proof!r} target {target!r} job {job!r}: {why}")
    return bad


def note_marker(text):
    """The "marker" of a note body `<word> {json}` ("" when absent)."""
    try:
        return str(json.loads(text.split(" ", 1)[1]).get("marker", ""))
    except (IndexError, ValueError, AttributeError):
        return ""


def abort_findings(cdir, cid):
    """The watcher's ABORT test (RUNBOOK 1.7): signals not accounted for, and scope_killed notes naming no scope of
    this campaign."""
    raw = raw_records(cdir)
    bad = signal_problems(raw, cid)
    for _, _, e in raw:
        t = (e.get("body") or {}).get("text", "") if e.get("kind") == "note" else ""
        if t.startswith("scope_killed") and ("rr-" + cid) not in t:
            bad.append("scope_killed " + t[:120])
    return bad


def grade(out, cid, ex, ver, marked, scopes_after, mode, ledger, off, ws):
    """[(criterion, PASS / FAIL / n.a., why)]"""
    host = mode == "--run"
    recs = []
    for f in sorted(glob.glob(out + "/c/ledger/segment-*.jsonl")):
        for line in lines_of(f):
            try:
                r = json.loads(line)
            except ValueError:
                continue
            recs.append(norm(r))
    started = [e for e in recs if e.get("kind") == "job" and e.get("ev") == "started"]
    launched = []
    for f in sorted(glob.glob(out + "/c/ledger/segment-*.jsonl")):
        for line in lines_of(f):
            try:
                e = json.loads(line).get("event", {})
            except ValueError:
                continue
            ev = (e.get("body") or {}).get("ev") if e.get("kind") == "job" else None
            if isinstance(ev, dict) and ev.get("ev") == "launch" and ev.get("scope_unit"):
                launched.append(ev["scope_unit"])
    notes = [e.get("text", "") for e in recs if e.get("kind") == "note"]
    sigs = [e for e in recs if e.get("kind") == "signal_sent"]
    log = slurp(out + "/run.log") if os.path.exists(out + "/run.log") else ""
    res = []

    def put(k, ok, why):
        res.append((k, ok if ok == "INFO" else ("PASS" if ok is True else ("n.a." if ok is None else "FAIL")), why))

    sup = slurp(out + "/supervisor.unit").strip() if os.path.exists(out + "/supervisor.unit") else ""
    end = " ".join(l.strip() for l in slurp(out + "/launcher.log").splitlines()
                   if l.startswith(("Finished with result", "Main processes terminated"))) \
        if os.path.exists(out + "/launcher.log") else ""
    # procfix7 round 10 (gate13c m13c-p1-clean-signal): with the service form, exit 0 also needs systemd's report of a
    # NORMAL exit with status 0: systemd-run --wait returns 0 for a death by SIGHUP / SIGINT / SIGTERM / SIGPIPE too
    # (possible only before rr_campaign installs its handlers)
    clean = not sup or "Main processes terminated with: code=exited, status=0" in (end + " ")
    put("P1", ex == 0 and clean,
        f"exit {ex}" + (f" (service {sup}: {end or 'no end reported by systemd-run --wait'})" if sup else "")
        + ("" if clean else "; systemd does not report a normal exit with status 0"))
    jc = [n for n in notes if n.startswith("job_containment ")]
    if host:
        sc = sum('"containment":"scope:rr-' in n for n in jc)
        # procfix7 round 8b: a job that ended before its scope was checked counts with POSITIVE evidence only
        # (systemd-run's start acknowledgment); scope_unverified_ended never counts
        ack = sum('"containment":"scope_acknowledged_ended"' in n for n in jc)
        unv = sum('"containment":"scope_unverified_ended"' in n for n in jc)
        line = "containment: one transient systemd scope per job" in log
        put("P2", line and sc + ack == len(started),
            f"start line {'present' if line else 'MISSING'}; job_containment scope:rr- {sc} + scope_acknowledged_ended {ack} == started {len(started)} (scope_unverified_ended {unv})")
    else:
        mo = sum('"containment":"marker_only"' in n for n in jc)
        put("P2", ("containment: marker only" in log and mo == len(started)) or None,
            f"(namespace: no scope) marker_only notes {mo} == started {len(started)}")
    peak = int(slurp(out + "/scopes.peak")) if os.path.exists(out + "/scopes.peak") else 0
    put("P3", (peak <= 8 and scopes_after == 0 and marked == 0) if host else (marked == 0),
        f"peak scopes {peak if host else 'n.a.'} (<= 8), scopes after {scopes_after if host else 'n.a.'}, A12 MARKED {marked}")
    put("P4", ver == 0, f"verify exit {ver}")
    badsig = signal_problems(raw_records(out + "/c"), cid)
    ledger_text = "\n".join(slurp(f) for f in glob.glob(out + "/c/ledger/*.jsonl"))
    possibly = ledger_text.count("POSSIBLY SENT")
    badnotes = [n[:60] for n in notes if re.match(r"(scope_backstop_|final_sweep_vanished|final_sweep_unproven|end_unproven|unproven_end|unproven_at_exit|marked_survivor|descendants_alive|hidden_descendants|scope_unverified_limit)", n)]
    put("P5", not badsig and possibly == 0 and not badnotes,
        f"{len(sigs)} signal_sent ({len(badsig)} not accounted for by this campaign's records{': ' + '; '.join(badsig[:3]) if badsig else ''}), POSSIBLY SENT {possibly}, notes {badnotes or 'none'}")
    # procfix7 round 9b (gate13b): the unproven_end notes as a NAMED CLASS (INFO: the strict P5 above is unchanged)
    unproven, cleared_after = [], {}
    for i, n in enumerate(notes):
        if n.startswith("unproven_end "):
            unproven.append((i, note_marker(n)))
        elif n.startswith("unverified_end_cleared "):
            cleared_after.setdefault(note_marker(n), []).append(i)
    if unproven:
        cleared = [m for i, m in unproven if any(j > i for j in cleared_after.get(m, []))]
        others = [b for b in badnotes if not b.startswith("unproven_end")]
        sole = not badsig and possibly == 0 and not others
        put("P5-unproven", "INFO",
            f"{len(unproven)} unproven_end note(s) ({', '.join(m for _, m in unproven)}): {len(cleared)} of {len(unproven)} "
            f"cleared later (unverified_end_cleared of the same attempt); signals not accounted for {len(badsig)}, POSSIBLY "
            f"SENT {possibly}, other P5 notes {others or 'none'}: "
            + ("the strict P5 FAILs ONLY on these notes" if sole else "the strict P5 FAILs ALSO for other reasons"))
    with open(ledger, errors="replace") as f:
        f.seek(off)
        lines = f.read().splitlines()
    mine = [l for l in lines if "(rr-campaign)" in l and cid in l]
    held_now, peak_cpu, claimed = set(), 0, set()
    for l in mine:
        m = re.search(r"\): \+ CPUs? ([0-9,]+) ", l)
        if m:
            s = cpus(m[1]); claimed |= s; held_now |= s; peak_cpu = max(peak_cpu, len(held_now)); continue
        m = re.search(r"\): RELEASED ([0-9,]+) ", l)
        if m:
            held_now -= cpus(m[1])
    reserved = cpus("168-175,288-319,192-215,258-260,262,271,335,128-162,164-166,342,362,363,378")
    held = set()
    try:
        v = json.loads(slurp(ws + "/ops/FOREIGN_5L_LIFT.held"))
        if isinstance(v.get("held"), list):
            held |= cpus(",".join(map(str, v["held"])))
    except Exception:
        pass
    put("P6", not held_now and not (claimed & reserved) and peak_cpu <= 8,
        f"claims of this campaign {sum(' + CPU' in l for l in mine)}, unreleased {sorted(held_now)}, reserved CPUs claimed {sorted(claimed & reserved)}, peak {peak_cpu} at once (<= 8); CPUs used {sorted(claimed)}; held-list overlap now {sorted(claimed & held)}")
    stopped = [p for p in slurp(out + "/stopped.txt").split() if p] if os.path.exists(out + "/stopped.txt") else []
    ours_stopped = []
    for p in stopped:
        try:
            if f"RR_CAMPAIGN_JOB={cid}:".encode() in slurp(f"/proc/{p}/environ", "rb"):
                ours_stopped.append(p)
        except OSError:
            pass
    put("P7", not ours_stopped, f"STOPPED processes of the user {len(stopped)} (of this campaign {ours_stopped or 'none'}; the uid is shared: others are listed in stopped.txt)")
    if host:
        rd = lambda n: slurp(out + "/" + n).split() if os.path.exists(out + "/" + n) else []
        own = [x for x in (rd("supervisor.unit") + rd("preflight.unit")) if x]
        ok8, why8, foreign = p8_units.grade_p8(cid, rd("units.before"), rd("units.after"), rd("units.seen"), notes,
                                               launched=launched, own_units=own)
        with open(out + "/units.foreign.txt", "w") as f:
            f.write("".join(u + "\n" for u in foreign))
        put("P8", ok8, why8)
    else:
        put("P8", None, "(namespace: no systemd units)")
    if os.path.exists(out + "/abort.txt") and slurp(out + "/abort.txt").strip():
        put("ABORT", False, slurp(out + "/abort.txt").strip())
    return res


if __name__ == "__main__":
    if sys.argv[1:2] == ["--abort-check"]:
        print("; ".join(abort_findings(sys.argv[2], sys.argv[3])))
        sys.exit(0)
    out, cid, ex, ver, marked, scopes_after, mode, ledger, off, ws = sys.argv[1:11]
    res = grade(out, cid, int(ex), int(ver), int(marked), int(scopes_after), mode, ledger, int(off), ws)
    with open(out + "/pass.txt", "w") as f:
        for k, v, why in res:
            line = f"{k} {v}: {why}"
            print(line)
            f.write(line + "\n")
    sys.exit(0 if all(v != "FAIL" for _, v, _ in res) else 1)
