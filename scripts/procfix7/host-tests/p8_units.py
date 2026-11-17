"""P8 of the first host step (RUNBOOK 1.7; lane procfix7 round 8, gate12 M12-2): the systemd units OF THIS CAMPAIGN.

The uid is SHARED: other sessions create units during the hour (light builds' run-p*-i*.scope, vk5l-*, t3code-*, other
campaigns' rr-<other id>-*). P8 grades only this campaign's own units, by name: `rr-<campaign id>-*` (the deterministic
job scope names, RUNBOOK A6) and the scope names the ledger recorded (job_containment notes). It FAILS when
  (a) a unit of this campaign seen during or after the run is not a scope the ledger recorded (a scope nothing accounts
      for), or
  (b) a unit of this campaign is still loaded after the run (active, failed or inactive: list-units --all).
Every other new unit is FOREIGN: listed (units.foreign.txt), never graded. Standard library only."""

import json
import re

SCOPE_IN_NOTE = re.compile(r'"containment":"scope:([^"]+)"')
# the unit as a note's "why" names it (with or without ".scope"): requested but not verified (until round 8a), or the
# round-8b words for a scope that was not verified
UNIT_IN_WHY = re.compile(r'scope (rr-[^ ]+?)(?:\.scope)? (?:requested|existed|not verified)')


def unit_name(u):
    return u if u.endswith(".scope") else u + ".scope"


def recorded_scopes(notes):
    """The unit names of the job scopes the ledger recorded: `job_containment` notes with "containment":"scope:<unit>",
    and the requested unit of a scope recorded as not verified ("scope <unit> requested but not verified")."""
    out = set()
    for n in notes:
        if not n.startswith("job_containment "):
            continue
        out.update(unit_name(u) for u in SCOPE_IN_NOTE.findall(n))
        out.update(unit_name(u) for u in UNIT_IN_WHY.findall(n))
    return out


def grade_p8(cid, before, after, seen, notes, own_unit_prefix="hoststep-fake1-", launched=()):
    """(ok, why, foreign): `before` / `after` the unit names of `systemctl --user list-units --all` before and after the
    run, `seen` the rr-<cid>-* names the watcher saw during the run, `notes` the ledger's note texts, `launched` the
    scope units the ledger's launch records name (`scope_unit`)."""
    before, after, seen = set(before), set(after), set(seen)
    mine = f"rr-{cid}-"
    ours = {u for u in seen | (after - before) if u.startswith(mine)}
    recorded = recorded_scopes(notes) | {unit_name(u) for u in launched if u}
    unrecorded = sorted(ours - recorded)
    left = sorted(u for u in after if u.startswith(mine))
    foreign = sorted(u for u in after - before if not u.startswith(mine) and not u.startswith(own_unit_prefix))
    ok = bool(cid) and not unrecorded and not left
    why = (f"units of this campaign (rr-{cid}-*): {len(ours)} seen, {len(recorded)} recorded scopes; "
           f"not recorded {unrecorded or 'none'}; still loaded after the run {left or 'none'}; "
           f"{len(foreign)} foreign unit(s) of the shared uid listed in units.foreign.txt (not graded)")
    if not cid:
        why = "no campaign id in run.log: nothing can be attributed; " + why
    return ok, why, foreign


if __name__ == "__main__":
    import sys
    # p8_units.py <cid> <units.before> <units.after> <units.seen> <ledger segment>... : prints the verdict
    cid, b, a, s = sys.argv[1:5]
    notes = []
    for f in sys.argv[5:]:
        for line in open(f, errors="replace"):
            try:
                e = json.loads(line).get("event", {})
            except ValueError:
                continue
            if e.get("kind") == "note":
                notes.append((e.get("body") or {}).get("text", ""))
    rd = lambda p: open(p).read().split()
    ok, why, _ = grade_p8(cid, rd(b), rd(a), rd(s), notes)
    print(("PASS" if ok else "FAIL") + ": " + why)
    sys.exit(0 if ok else 1)
