#!/usr/bin/env python3
"""Read-only per-owner / per-rank / per-A breakdown of a checkpoint records sidecar (JSONL).

usage: records_breakdown.py RECORDS.jsonl QUERIES.json SELECTION.json OUT.json [--procs N] [--bins N]

Splits the file into byte chunks (line aligned) and parses them in parallel (stdlib only).
Per record: kind, phase, owner, rank, seconds, id and A_lo = the sum of the lower-corner local
coordinates (in owner-local coordinates: dots on the active lines plus numerator powers on the
inactive lines), a lower bound on how far the domain sits from the owner's corner.  Route records are also attributed to the class owner
they route to (selection.json initial_frontier_routes source_mask -> owner_mask).
"""
import collections
import json
import multiprocessing as mp
import os
import sys


def a_lo(rec):
    lo = rec.get('lower') or []
    return sum(x for x in lo if isinstance(x, int) and x > 0)


def a_bucket(a):
    if a <= 16:
        return str(a)
    b = 32
    while a > b:
        b *= 2
    return f'<={b}'


def chunk_bounds(path, n):
    size = os.path.getsize(path)
    bounds = [0]
    with open(path, 'rb') as f:
        for i in range(1, n):
            f.seek(size * i // n)
            f.readline()
            bounds.append(f.tell())
    bounds.append(size)
    return [(bounds[i], bounds[i + 1]) for i in range(n) if bounds[i + 1] > bounds[i]]


def work(args):
    path, start, end, route_owner, max_id, nbins = args
    kinds = collections.Counter()
    cnt = collections.Counter()      # (kind, phase, owner)
    secs = collections.Counter()     # (kind, phase, owner)
    rank = collections.Counter()     # (kind, phase, owner, rank)
    abkt = collections.Counter()     # (kind, phase, owner, a_bucket)
    route_to = collections.Counter()  # (kind, class_owner) for Route records
    route_to_rank = collections.Counter()  # (kind, class_owner, rank)
    binned = collections.Counter()   # (bin, kind, phase, owner)
    bin_secs = collections.Counter()  # (bin, owner) native seconds
    amax = {}                        # (phase, owner) -> max A_lo over natives
    rmax = {}
    ids = [None, None]
    n = 0
    with open(path, 'rb') as f:
        f.seek(start)
        pos = start
        while pos < end:
            line = f.readline()
            if not line:
                break
            pos += len(line)
            n += 1
            d = json.loads(line)
            k = d['record_kind']
            ph = d.get('phase')
            ow = d.get('owner')
            r = d.get('rank')
            i = d.get('id')
            kinds[k] += 1
            key = (k, ph, ow)
            cnt[key] += 1
            s = d.get('seconds') or 0.0
            secs[key] += s
            rank[(k, ph, ow, r)] += 1
            a = a_lo(d)
            abkt[(k, ph, ow, a_bucket(a))] += 1
            if ph == 'Route':
                co = route_owner.get(ow, '?')
                route_to[(k, co)] += 1
                route_to_rank[(k, co, r)] += 1
            if i is not None:
                b = min(nbins - 1, i * nbins // (max_id + 1))
                binned[(b, k, ph, ow)] += 1
                if k != 'delegated_not_inspected':
                    bin_secs[(b, ow)] += s
                if ids[0] is None or i < ids[0]:
                    ids[0] = i
                if ids[1] is None or i > ids[1]:
                    ids[1] = i
            if k != 'delegated_not_inspected':
                amax[(ph, ow)] = max(amax.get((ph, ow), 0), a)
                if r is not None:
                    rmax[(ph, ow)] = max(rmax.get((ph, ow), -1), r)
    return dict(n=n, kinds=kinds, cnt=cnt, secs=secs, rank=rank, abkt=abkt, route_to=route_to,
                route_to_rank=route_to_rank, binned=binned, bin_secs=bin_secs, amax=amax, rmax=rmax, ids=ids)


def main():
    path, qpath, spath, out = sys.argv[1:5]
    procs = 12
    nbins = 20
    if '--procs' in sys.argv:
        procs = int(sys.argv[sys.argv.index('--procs') + 1])
    if '--bins' in sys.argv:
        nbins = int(sys.argv[sys.argv.index('--bins') + 1])
    queries = json.load(open(qpath))['queries']
    helper = {}
    for q in queries:
        if q['id'].startswith('owner-anchor-') or q['id'].startswith('helper'):
            helper[q['owner']] = max(helper.get(q['owner'], -1), q.get('max_numerator_rank') if q.get('max_numerator_rank') is not None else 99)
    sel = json.load(open(spath))
    route_owner = {r['source_mask']: r['owner_mask'] for r in sel['initial_frontier_routes']}
    # max id from the tail
    with open(path, 'rb') as f:
        f.seek(max(0, os.path.getsize(path) - 65536))
        tail = f.read().splitlines()
    max_id = max(json.loads(l)['id'] for l in tail[1:] if l.strip())
    chunks = chunk_bounds(path, procs * 4)
    with mp.Pool(procs) as pool:
        parts = pool.map(work, [(path, a, b, route_owner, max_id, nbins) for a, b in chunks])
    tot = collections.defaultdict(collections.Counter)
    amax, rmax = {}, {}
    n = 0
    lo_id, hi_id = None, None
    for p in parts:
        n += p['n']
        for name in ('kinds', 'cnt', 'secs', 'rank', 'abkt', 'route_to', 'route_to_rank', 'binned', 'bin_secs'):
            tot[name].update(p[name])
        for kk, v in p['amax'].items():
            amax[kk] = max(amax.get(kk, 0), v)
        for kk, v in p['rmax'].items():
            rmax[kk] = max(rmax.get(kk, -1), v)
        if p['ids'][0] is not None:
            lo_id = p['ids'][0] if lo_id is None else min(lo_id, p['ids'][0])
            hi_id = p['ids'][1] if hi_id is None else max(hi_id, p['ids'][1])
    NAT = ('native_inspection', 'partial_initial_overlap_inspection')
    nat_total = sum(v for (k, ph, ow), v in tot['cnt'].items() if k in NAT)
    nat_secs = sum(v for (k, ph, ow), v in tot['secs'].items() if k in NAT)
    owners = sorted({ow for (k, ph, ow) in tot['cnt'] if ph == 'Apply'})
    per_owner = []
    for ow in owners:
        row = {'owner': ow, 'helper_rank': helper.get(ow)}
        for ph in ('Apply',):
            row['apply_natives'] = sum(tot['cnt'][(k, ph, ow)] for k in NAT)
            row['apply_native_seconds'] = round(sum(tot['secs'][(k, ph, ow)] for k in NAT), 3)
            row['apply_aliases'] = tot['cnt'][('delegated_not_inspected', ph, ow)]
            row['apply_native_rank_hist'] = {str(r): sum(tot['rank'][(k, ph, ow, r)] for k in NAT)
                                            for r in sorted({r for (k2, p2, o2, r) in tot['rank'] if o2 == ow and p2 == ph and k2 in NAT}, key=lambda x: (x is None, x))}
            row['apply_alias_rank_hist'] = {str(r): tot['rank'][('delegated_not_inspected', ph, ow, r)]
                                           for r in sorted({r for (k2, p2, o2, r) in tot['rank'] if o2 == ow and p2 == ph and k2 == 'delegated_not_inspected'}, key=lambda x: (x is None, x))}
            row['apply_native_A_lo_hist'] = {b: sum(tot['abkt'][(k, ph, ow, b)] for k in NAT)
                                            for b in sorted({b for (k2, p2, o2, b) in tot['abkt'] if o2 == ow and p2 == ph and k2 in NAT})}
            row['apply_native_max_A_lo'] = amax.get((ph, ow))
            row['apply_native_max_rank'] = rmax.get((ph, ow))
        row['route_natives_into'] = sum(tot['route_to'][(k, ow)] for k in NAT)
        row['route_aliases_into'] = tot['route_to'][('delegated_not_inspected', ow)]
        row['route_native_rank_hist_into'] = {str(r): sum(tot['route_to_rank'][(k, ow, r)] for k in NAT)
                                              for r in sorted({r for (k2, o2, r) in tot['route_to_rank'] if o2 == ow and k2 in NAT}, key=lambda x: (x is None, x))}
        row['apply_share_of_native_seconds'] = round(row['apply_native_seconds'] / nat_secs, 4) if nat_secs else None
        per_owner.append(row)
    per_owner.sort(key=lambda r: -r['apply_native_seconds'])
    route_sources = collections.Counter()
    for (k, ph, ow), v in tot['cnt'].items():
        if ph == 'Route' and k in NAT:
            route_sources[ow] += v
    # time bins: natives per owner (Apply) and Route-by-class
    bins = []
    for b in range(nbins):
        row = {'bin': b, 'id_range': [b * (max_id + 1) // nbins, (b + 1) * (max_id + 1) // nbins - 1]}
        row['natives'] = sum(v for (bb, k, ph, ow), v in tot['binned'].items() if bb == b and k in NAT)
        row['aliases'] = sum(v for (bb, k, ph, ow), v in tot['binned'].items() if bb == b and k == 'delegated_not_inspected')
        ap = collections.Counter()
        for (bb, k, ph, ow), v in tot['binned'].items():
            if bb == b and k in NAT and ph == 'Apply':
                ap[ow] += v
        row['apply_natives_top'] = ap.most_common(4)
        sc = collections.Counter({ow: v for (bb, ow), v in tot['bin_secs'].items() if bb == b})
        row['native_seconds'] = round(sum(sc.values()), 2)
        row['native_seconds_top'] = [(o, round(v, 2)) for o, v in sc.most_common(3)]
        bins.append(row)
    phase_tot = {}
    for ph in ('Apply', 'Route'):
        phase_tot[ph] = {'natives': sum(v for (k, p2, ow), v in tot['cnt'].items() if p2 == ph and k in NAT),
                         'native_seconds': round(sum(v for (k, p2, ow), v in tot['secs'].items() if p2 == ph and k in NAT), 2),
                         'aliases': sum(v for (k, p2, ow), v in tot['cnt'].items() if p2 == ph and k == 'delegated_not_inspected')}
    rank_all = collections.Counter()
    for (k, ph, ow, r), v in tot['rank'].items():
        if k in NAT:
            rank_all[(ph, r)] += v
    report = {
        'records_path': os.path.abspath(path), 'records_bytes': os.path.getsize(path), 'queries_path': os.path.abspath(qpath),
        'records': n, 'id_range': [lo_id, hi_id], 'kinds': dict(tot['kinds']), 'natives': nat_total,
        'native_seconds': round(nat_secs, 2), 'phase_totals': phase_tot,
        'native_rank_hist_by_phase': {ph: {str(r): rank_all[(ph, r)] for r in sorted({r for (p2, r) in rank_all if p2 == ph}, key=lambda x: (x is None, x))} for ph in ('Apply', 'Route')},
        'helper_ranks': helper, 'per_owner_apply': per_owner,
        'route_native_sources_top': route_sources.most_common(25), 'time_bins': bins,
        'scope': 'committed records sidecar only (records admitted and committed before the save); read-only',
    }
    json.dump(report, open(out, 'w'), indent=1)
    print(json.dumps({k: report[k] for k in ('records', 'kinds', 'natives', 'native_seconds', 'phase_totals')}, indent=1))
    for r in per_owner[:8]:
        print(r['owner'], 'helperR', r['helper_rank'], 'apply n', r['apply_natives'], 'sec', r['apply_native_seconds'],
              'share', r['apply_share_of_native_seconds'], 'ranks', r['apply_native_rank_hist'], 'maxA', r['apply_native_max_A_lo'],
              'route_in', r['route_natives_into'])


if __name__ == '__main__':
    main()
