#!/usr/bin/env python3
"""fc_tool.py: helpers of the SINGLE FINAL CERTIFICATION of one node table (lane parametric, 2026-11-17).

Generic (L8): every node fact (terminals, cones, recipe position, evidence columns, keys) comes from the arguments, which
the node launcher takes from its configuration. Python here only INSPECTS tables and reference data and parses the
verdicts of the Rust harness (certificate, terminal rule, descent, liveness) and of the engine (import, first match,
one-level reduces); it decides nothing the harness or the engine decides.

Subcommands (all write JSON; a failed criterion exits 3, a usage / input error 2):
  prep        domain copies, structural scan, sources lists (domain + full), recipes placed by dispatch position
  certcheck   the region certificate's verdict per cone against the terminal set
  termcheck   the terminal rule's verdict
  b1check     the descent summary's verdict
  impcheck    an import + --check pair (rc, check line: 0 open box keys, terminal count)
  exact       exactness binding of every record (hash of target + rhs) against evidence files; changed / unproven lists
  livecompile merge liveness shard outputs (last line per position) into one list + summary
  fmkeys      deterministic drill keys: arrival sample (RRKEYS01) + explicit keys + terminals
  fmcompare   engine first match vs the core dispatcher's first match on the drill keys
  drilltable  the final table with point-rule relations inserted at the recipe position (one-level drill table)
  onejobs     one-level jobs of the acceptance keys (key + level-1 children, core first matches)
  onerun      run the one-level reduces on the engine
  manifest    sha256 + bytes of every file of a delivery directory
"""
import argparse, csv, glob, hashlib, json, os, random, re, subprocess, sys, time

FAIL = 3


def sha256_file(p, bs=1 << 24):
    h = hashlib.sha256()
    with open(p, 'rb') as f:
        for b in iter(lambda: f.read(bs), b''):
            h.update(b)
    return h.hexdigest()


def dump(obj, path):
    tmp = path + '.tmp'
    with open(tmp, 'w') as f:
        json.dump(obj, f, indent=1)
        f.write('\n')
    os.replace(tmp, path)


def key_text(k):
    return 'I(' + ','.join(map(str, k)) + ')'


def parse_key(t):
    if isinstance(t, list):
        return [int(x) for x in t]
    m = re.fullmatch(r'\s*I\(([-0-9, ]+)\)\s*', t)
    return [int(x) for x in m.group(1).split(',')]


def dr(k, mask):
    return sum(x - 1 for x, m in zip(k, mask) if m == '1') - sum(x for x, m in zip(k, mask) if m == '0')


def terminals_of(path):
    return sorted(tuple(parse_key(k)) for k in json.load(open(path))['keys'])


def is_rule(v):
    return v.get('type', 'rule') == 'rule' and 'target' in v


# ----------------------------------------------------------------------------------------------------------------- prep
def cmd_prep(a):
    """Domain copy of every table file (rhs -> [], origin.rhs_stripped_from), scan, sources lists, recipes placed."""
    os.makedirs(a.out, exist_ok=True)
    files = a.tables.split(':')
    terms = terminals_of(a.terminals)
    scan = {'tables': [], 'problems': []}
    all_rules = []  # (file index, label, dispatch position)
    residuals = []
    mask = None
    for fi, src in enumerate(files):
        dom = os.path.join(a.out, '%s.%d.domain.jsonl' % (a.node, fi))
        n = 0; labels = []; ends = []; charts = 0; hdr = False; off = 0
        ix = open(os.path.join(a.out, '%s.%d.index.jsonl' % (a.node, fi)), 'w')
        with open(src, 'rb') as f, open(dom + '.tmp', 'wb') as g:
            for raw in f:
                v = json.loads(raw)
                t = v.get('type', 'rule')
                ix.write(json.dumps({'type': t, 'index': v.get('index') if is_rule(v) else None, 'offset': off, 'length': len(raw),
                                     'sha256': hashlib.sha256(raw.rstrip(b'\n')).hexdigest()}) + '\n')
                off += len(raw)
                if is_rule(v):
                    o = v.get('origin') or {}
                    labels.append(v['index'])
                    all_rules.append((fi, v['index'], o.get('dispatch_position')))
                    v['origin'] = dict(o, rhs_stripped_from={'path': src, 'line_sha256': hashlib.sha256(raw.rstrip(b'\n')).hexdigest()})
                    v['rhs'] = []
                    g.write(json.dumps(v, separators=(',', ':')).encode() + b'\n')
                    n += 1
                    continue
                if t == 'header':
                    hdr = True; mask = mask or v.get('mask')
                elif t == 'chart':
                    charts += 1
                elif t == 'residual':
                    residuals.append(tuple(parse_key(v['key'])))
                elif t == 'end':
                    ends.append(v)
                g.write(raw)
        ix.close()
        os.replace(dom + '.tmp', dom)
        in_order = labels == list(range(len(labels)))
        if not in_order:
            # first match follows FILE order; labels stay as written. The native importer needs index = running count:
            # a file that is not in that order is reported here and refused by the import step (the engine's own message)
            scan.setdefault('warnings', []).append('%s: rule indices are not 0..n-1 in file order (first out of order at file rule %d, index %s): '
                                                   'the native importer will refuse it' % (src, next(i for i, l in enumerate(labels) if l != i), labels[next(i for i, l in enumerate(labels) if l != i)]))
        scan['tables'].append({'path': src, 'domain_copy': dom, 'rules': n, 'header': hdr, 'charts': charts, 'end': ends[-1] if ends else None,
                               'indices_in_file_order': in_order, 'labels': labels})
        if not hdr:
            scan['problems'].append('%s: no header line' % src)
        if not ends:
            scan['problems'].append('%s: no end record' % src)
    dps = [d for (_, _, d) in all_rules]
    scan['rules'] = len(all_rules)
    scan['mask'] = mask
    scan['dispatch_position_missing'] = sum(1 for d in dps if d is None)
    known = [d for d in dps if d is not None]
    scan['dispatch_position_nondecreasing'] = all(x <= y for x, y in zip(known, known[1:]))
    scan['dispatch_position_duplicates'] = len(known) - len(set(known))
    scan['residuals'] = [key_text(k) for k in residuals]
    scan['terminals'] = [key_text(k) for k in terms]
    if sorted(residuals) != terms:
        scan['problems'].append('residual records %s != terminal set %s' % (scan['residuals'], scan['terminals']))
    # recipes: placed before the first rule whose dispatch position is >= recipe_before_dp (a position of the table the
    # recipes were certified against); the count of rules before them is the new recipe position
    recipes = []
    if a.recipes:
        # file order decides: the recipes go right before the first rule (in file order) whose KNOWN dispatch position is >=
        # recipe_before_dp; rules without a dispatch position (regenerated in place, or fresh sub-table rules) keep their file place
        first = next((i for i, (_, _, d) in enumerate(all_rules) if d is not None and d >= a.recipe_before_dp), len(all_rules))
        if not any(d is not None for (_, _, d) in all_rules):
            scan['problems'].append('recipes given but no rule carries origin.dispatch_position: cannot place them')
        p = first
        scan['recipe_position'] = p   # rules before the recipes in file order (= the importer's table index when indices are in order)
        scan['rules_without_dispatch_position'] = [lab for (_, lab, d) in all_rules if d is None]
        if len(files) != 1:
            scan['problems'].append('recipes with several table files: not supported')
        with open(os.path.join(a.out, '%s.recipes.jsonl' % a.node), 'w') as g:
            for l in open(a.recipes):
                if not l.strip():
                    continue
                r = json.loads(l)
                r['position_in_certified_source'] = r.get('position')
                r['position'] = p
                recipes.append(r)
                g.write(json.dumps(r, separators=(',', ':')) + '\n')
        for r in recipes:
            for k in ('recipe_record', 'coverage_record'):
                e = r[k]
                if sha256_file(e['path']) != e['sha256']:
                    scan['problems'].append('recipe %s: %s sha256 mismatch' % (r['id'], k))
            if sha256_file(r['seal']['manifest']) != r['seal']['sha256']:
                scan['problems'].append('recipe %s: seal manifest sha256 mismatch' % r['id'])
    # sources lists: table files in dispatch order, the recipes (sealed coverage records) at their position
    for form in ('domain', 'full'):
        srcs = []
        for fi, src in enumerate(files):
            path = scan['tables'][fi]['domain_copy'] if form == 'domain' else src
            n = scan['tables'][fi]['rules']
            if recipes and fi == 0:
                p = scan['recipe_position']
                labs = scan['tables'][fi]['labels']
                srcs.append({'id': '%s.final [0,%d)' % (a.node, p), 'class': 'ship', 'path': path, 'records': labs[:p], 'witness': True})
                for r in recipes:
                    srcs.append({'id': r['id'], 'class': 'sealed_recipe', 'path': r['coverage_record']['path'], 'records': None, 'witness': False,
                                 'bound_dr': r['bound']['bound_dr'], 'seal': {'path': r['seal']['manifest'], 'sha256': r['seal']['sha256']}})
                srcs.append({'id': '%s.final [%d,%d)' % (a.node, p, n), 'class': 'ship', 'path': path, 'records': labs[p:], 'witness': True})
            else:
                srcs.append({'id': '%s.final file %d' % (a.node, fi), 'class': 'ship', 'path': path, 'records': None, 'witness': True})
        dump({'node': a.node, 'catalogue': None, 'note': 'lane parametric final certification (%s): %s' % (a.label, form), 'sources': srcs},
             os.path.join(a.out, '%s.%s.sources.json' % (a.node, form)))
    # positions of the first-match order (sources order, then records): (source id, label, file, table index)
    with open(os.path.join(a.out, '%s.positions.jsonl' % a.node), 'w') as g:
        pos = 0
        for s in json.load(open(os.path.join(a.out, '%s.domain.sources.json' % a.node)))['sources']:
            if s['class'] == 'sealed_recipe':
                labs = [json.loads(l)['index'] for l in open(s['path']) if l.strip() and is_rule(json.loads(l))]
                for lab in labs:
                    g.write(json.dumps({'position': pos, 'source': s['id'], 'label': lab, 'file': None, 'table_index': None}) + '\n'); pos += 1
            else:
                fi = [t['domain_copy'] for t in scan['tables']].index(s['path'])
                labs = s['records'] if s['records'] is not None else scan['tables'][fi]['labels']
                for lab in labs:
                    g.write(json.dumps({'position': pos, 'source': s['id'], 'label': lab, 'file': fi, 'table_index': lab}) + '\n'); pos += 1
    scan['positions'] = pos
    scan['dispatch_positions'] = [[fi, lab, d] for (fi, lab, d) in all_rules]
    dump(scan, os.path.join(a.out, '%s.scan.json' % a.node))
    print(json.dumps({k: v for k, v in scan.items() if k != 'dispatch_positions'})[:2000])
    return FAIL if scan['problems'] else 0


# ----------------------------------------------------------------------------------------------------------- certcheck
def cmd_certcheck(a):
    terms = terminals_of(a.terminals)
    summ = json.load(open(a.summary))
    exact = [c for c in a.exact_cones.split(';') if c]
    report = {'summary': a.summary, 'terminals': [key_text(k) for k in terms], 'cones': {}, 'problems': []}
    failed = summ['header'].get('pieces', {}).get('failed')
    if failed:
        report['problems'].append('pieces failed: %s' % failed)
    by = {c['cone']: c for c in summ['cones']}
    pdir = re.sub(r'\.summary\.json$', '.pieces', a.summary)
    for cone in [c for c in a.cones.split(';') if c]:
        tag = cone.replace(':', '_')
        c = by.get(tag)
        files = sorted(glob.glob(os.path.join(pdir, '*.%s.certificate.json' % tag)))
        if c is None or not files:
            report['problems'].append('%s: no summary / certificate file' % cone); continue
        cert = json.load(open(files[0]))
        unc = sorted(tuple(x['v']['witness']) for x in cert['listed'] if x.get('v', {}).get('verdict') == 'uncovered' and x['v'].get('witness'))
        row = {'certificate': files[0], 'certificate_sha256': sha256_file(files[0]), 'certificate_status': cert.get('certificate_status'),
               'verdict': c['verdict'], 'uncovered_elements': c['uncovered_elements'], 'undecided_elements': c['undecided_elements'],
               'residual_open_keys': c['residual_open_keys'], 'residual_cell_keys': c['residual_cell_keys'], 'checker_not_ok': c['checker_not_ok'],
               'pieces_with_flags': c['pieces_with_flags'], 'uncovered_witnesses': [key_text(k) for k in unc], 'engine': cert.get('engine'),
               'terminals_bound': cert.get('terminals'), 'tables_digest': cert.get('tables_digest'), 'probes': len(cert.get('probes', []))}
        ok = None
        if cone in exact:
            ok = (cert.get('certificate_status') == 'complete' and c['uncovered_elements'] == len(terms) and c['undecided_elements'] == 0
                  and c['residual_open_keys'] == len(terms) and c['checker_not_ok'] == 0 and not c['pieces_with_flags'] and unc == terms)
            if not ok:
                report['problems'].append('%s: open set is not exactly the terminal set (see row)' % cone)
        elif cone == 'orthant' and a.orthant_undecided is not None:
            ok = (cert.get('certificate_status') == 'complete' and unc == terms and c['uncovered_elements'] == len(terms)
                  and c['undecided_elements'] <= a.orthant_undecided and c['checker_not_ok'] == 0)
            if not ok:
                report['problems'].append('orthant: expected uncovered = terminals and <= %d undecided elements' % a.orthant_undecided)
            row['undecided_listed'] = [x for x in cert['listed'] if x.get('kind') == 'unresolved'][:20]
        row['pass'] = ok
        report['cones'][cone] = row
    report['pass'] = not report['problems']
    dump(report, a.out)
    print(json.dumps({'pass': report['pass'], 'problems': report['problems'],
                      'cones': {k: [v['uncovered_elements'], v['undecided_elements'], v['residual_open_keys'], v['pass']] for k, v in report['cones'].items()}}))
    return 0 if report['pass'] else FAIL


def cmd_termcheck(a):
    t = json.load(open(a.file))
    probs = []
    if t.get('coverage_stubs', 0) != 0:
        probs.append('coverage stubs: %s' % t['coverage_stubs'])
    if t.get('records_containing_a_terminal'):
        probs.append('records containing a terminal: %s' % t['records_containing_a_terminal'][:10])
    if t.get('verdict') not in (None, 'ok'):
        probs.append('verdict %s' % t.get('verdict'))
    out = {'file': a.file, 'sha256': sha256_file(a.file), 'pass': not probs, 'problems': probs,
           'bounded_recipes_not_stubs': t.get('bounded_recipes_not_stubs')}
    dump(out, a.out); print(json.dumps(out))
    return 0 if not probs else FAIL


def cmd_b1check(a):
    s = json.load(open(a.summary))
    flat = s.get('summary', s)
    dnd = flat.get('does_not_descend_listed', s.get('does_not_descend_listed'))
    err = flat.get('record_errors', s.get('record_errors'))
    probs = []
    if dnd:
        probs.append('does not descend: %s' % (dnd[:10] if isinstance(dnd, list) else dnd))
    if err not in (0, None, []):
        probs.append('record errors: %s' % err)
    if dnd is None and err is None:
        probs.append('summary without does_not_descend_listed / record_errors')
    out = {'summary': a.summary, 'sha256': sha256_file(a.summary), 'records_by_worst': flat.get('records_by_worst', s.get('records_by_worst')),
           'undecided_listed': flat.get('undecided_listed', s.get('undecided_listed')), 'pass': not probs, 'problems': probs}
    dump(out, a.out); print(json.dumps(out)[:1500])
    return 0 if not probs else FAIL


def time_v(path):
    txt = open(path, errors='replace').read() if os.path.exists(path) else ''
    w = re.search(r'Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): (\S+)', txt)
    m = re.search(r'Maximum resident set size \(kbytes\): (\d+)', txt)
    return {'wall': w.group(1) if w else None, 'peak_rss_gb': round(int(m.group(1)) / 1e6, 2) if m else None}


def cmd_impcheck(a):
    d = a.dir; n_term = len(terminals_of(a.terminals))
    rc_i = open(d + '/import.rc').read().strip() if os.path.exists(d + '/import.rc') else None
    rc_c = open(d + '/check.rc').read().strip() if os.path.exists(d + '/check.rc') else None
    imp = open(d + '/import.stdout', errors='replace').read() if os.path.exists(d + '/import.stdout') else ''
    chk = open(d + '/check.stdout', errors='replace').read() if os.path.exists(d + '/check.stdout') else ''
    line_i = next((l for l in imp.splitlines() if 'rules over' in l), None)
    line_c = next((l for l in chk.splitlines() if 'check of' in l), None)
    probs = []
    if rc_i != '0':
        probs.append('import rc %s' % rc_i)
    if rc_c != '0':
        probs.append('check rc %s' % rc_c)
    if not line_c or not re.search(r'\b0 open box keys', line_c):
        probs.append('check line without "0 open box keys": %s' % line_c)
    elif not re.search(r'\b%d terminals' % n_term, line_c):
        probs.append('check line: terminal count != %d: %s' % (n_term, line_c))
    payload = sorted(glob.glob(d + '/out/*'))
    out = {'dir': d, 'import_rc': rc_i, 'check_rc': rc_c, 'import_line': line_i, 'check_line': line_c, 'import_time': time_v(d + '/import.stderr'),
           'check_time': time_v(d + '/check.stderr'), 'payload': {os.path.basename(p): os.path.getsize(p) for p in payload},
           'schema_features': sorted(set(re.findall(r'binary\.v[0-9]+|required_features[^"\\]{0,80}', ''.join(open(p, errors='replace').read(4000) for p in payload if p.endswith('.json'))))),
           'pass': not probs, 'problems': probs}
    dump(out, d + '/impcheck.json'); print(json.dumps(out)[:1500])
    return 0 if not probs else FAIL


# --------------------------------------------------------------------------------------------------------------- exact
def load_evidence(spec):
    """spec = JSON list of {path, comment?, index_col, hash_col?, class_col, name}: rows -> (index, hash, class)."""
    out = []
    for e in spec:
        lines = [l for l in open(e['path']) if not (e.get('comment') and l.startswith(e['comment']))]
        rows = list(csv.DictReader(lines, delimiter='\t'))
        for r in rows:
            try:
                idx = int(r[e['index_col']])
            except (KeyError, ValueError):
                continue
            out.append({'source': e['name'], 'index': idx, 'hash': r.get(e['hash_col']) if e.get('hash_col') else None, 'form': e.get('hash_form', 'list'),
                        'class': (r.get(e['class_col']) or '').strip()})
    return out


def target_rhs_hash(v, form):
    """The two re-binding hash forms in use: 'list' = sha256(json.dumps([target, rhs], separators=(',', ':'))) (helper VH T162 lanes,
    lane sizegen); 'dict_sorted' = sha256(json.dumps({'target', 'rhs'}, sort_keys=True, separators=(',', ':'))) (helper VH T178 lanes)."""
    if form == 'list':
        return hashlib.sha256(json.dumps([v['target'], v.get('rhs')], separators=(',', ':')).encode()).hexdigest()
    if form == 'dict_sorted':
        return hashlib.sha256(json.dumps({'target': v['target'], 'rhs': v.get('rhs')}, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    raise SystemExit('unknown hash form %s' % form)


def category(cls, cats):
    for name in ('whole', 'per_key'):
        if any(re.search(p, cls) for p in cats[name]):
            return name
    return 'not_proven'


def cmd_exact(a):
    """Per record of the final table(s): target_rhs_sha256 = sha256(json.dumps([target, rhs], separators=(',', ':'))) (helper VH's
    re-binding hash) and domain_sha256; evidence rows bind by HASH (hash column) or by INDEX when the evidence's own anchor hash at
    that index equals the record's hash; the best category wins (whole > per_key > not_proven); new records (no evidence) and the
    watch list are reported; side files of the size lane (weight / derivation data) are matched by hash when given."""
    cfg = json.load(open(a.config))
    cats = cfg['categories']
    ev = load_evidence(cfg['evidence'])
    by_hash, anchor = {}, {}
    forms = sorted({e.get('hash_form', 'list') for e in cfg['evidence']} | {'list'})
    for r in ev:
        if r['hash']:
            by_hash.setdefault((r['form'], r['hash']), []).append(r)
            anchor.setdefault(r['index'], set()).add((r['form'], r['hash']))
    by_index = {}
    for r in ev:
        by_index.setdefault(r['index'], []).append(r)
    side = {}
    for d in (a.side_dirs or '').split(':'):
        if not d or not os.path.isdir(d):
            continue
        for p in sorted(glob.glob(os.path.join(d, '**', '*.json*'), recursive=True)):
            try:
                txt = open(p, errors='replace').read(1 << 22)
            except OSError:
                continue
            for h in set(re.findall(r'[0-9a-f]{64}', txt)):
                side.setdefault(h, set()).add(p)
    rows = []
    for fi, path in enumerate(a.tables.split(':')):
        with open(path, 'rb') as f:
            for raw in f:
                if not raw.strip():
                    continue
                v = json.loads(raw)
                if not is_rule(v):
                    continue
                o = v.get('origin') or {}
                hs = {f: target_rhs_hash(v, f) for f in forms}
                th = hs['list']
                dh = hashlib.sha256(json.dumps([v.get('fixed'), v.get('equations'), v.get('exceptions'), v.get('domain_bounds')], separators=(',', ':')).encode()).hexdigest()
                dp = o.get('dispatch_position')
                mine = {(f, h) for f, h in hs.items()}
                hits = [dict(r, bound='hash') for f, h in hs.items() for r in by_hash.get((f, h), [])]
                if dp is not None:
                    hits += [dict(r, bound='index') for r in by_index.get(dp, []) if not r['hash'] and mine & anchor.get(dp, set())]
                best = max(hits, key=lambda r: ('not_proven', 'per_key', 'whole').index(category(r['class'], cats)), default=None)
                changed = not any(by_hash.get(x) for x in mine)
                rows.append({'file': fi, 'table_index': v['index'], 'dispatch_position': dp, 'bytes': len(raw), 'target': v['target'],
                             'target_rhs_sha256': th, 'target_rhs_sha256_forms': hs, 'domain_sha256': dh, 'changed_vs_evidence': changed,
                             'class': best['class'] if best else None, 'class_source': best['source'] if best else None,
                             'bound_by': best['bound'] if best else None, 'category': category(best['class'], cats) if best else 'not_proven',
                             'evidence_rows': len(hits), 'side_files': sorted(side.get(th, []))[:5],
                             'witness_loci': None if 'witness_loci' not in v else len(v['witness_loci'] or []),
                             'derivation': (o.get('derivation') or {}).get('kind') if isinstance(o.get('derivation'), dict) else o.get('derivation')})
    watch = set(cfg.get('watch_dispatch_positions', []))
    with open(a.out + '.jsonl', 'w') as g:
        for r in rows:
            g.write(json.dumps(r) + '\n')
    cnt = {}
    for r in rows:
        cnt[r['category']] = cnt.get(r['category'], 0) + 1
    summ = {'records': len(rows), 'by_category': cnt,
            'bytes_by_category': {c: sum(r['bytes'] for r in rows if r['category'] == c) for c in cnt},
            'changed_vs_evidence': [{k: r[k] for k in ('file', 'table_index', 'dispatch_position', 'bytes', 'category', 'side_files')} for r in rows if r['changed_vs_evidence']],
            'not_proven': [{k: r[k] for k in ('file', 'table_index', 'dispatch_position', 'bytes', 'class', 'changed_vs_evidence', 'side_files')} for r in rows if r['category'] == 'not_proven'],
            'per_key': [{k: r[k] for k in ('file', 'table_index', 'dispatch_position', 'bytes', 'class')} for r in rows if r['category'] == 'per_key'],
            'watch_list': [{k: r[k] for k in ('file', 'table_index', 'dispatch_position', 'bytes', 'class', 'category', 'changed_vs_evidence')} for r in rows if r['dispatch_position'] in watch],
            'watch_missing': sorted(watch - {r['dispatch_position'] for r in rows}),
            'evidence': [{'name': e['name'], 'path': e['path'], 'sha256': sha256_file(e['path'])} for e in cfg['evidence']],
            'side_dirs': a.side_dirs, 'side_files_seen': len({p for s in side.values() for p in s}),
            'categories': cats, 'hash_rule': 'target_rhs_sha256 = sha256(json.dumps([target, rhs], separators=(",", ":"))); per evidence source also its own form (target_rhs_sha256_forms)'}
    dump(summ, a.out + '.summary.json')
    print(json.dumps({'records': len(rows), 'by_category': cnt, 'changed': len(summ['changed_vs_evidence']), 'not_proven': len(summ['not_proven']),
                      'watch': [(w['dispatch_position'], w['category']) for w in summ['watch_list']]}))
    # the positions the targeted liveness must decide: changed records and records without a whole-domain / per-key proof
    pos = {}
    for l in open(a.positions):
        p = json.loads(l)
        if p['file'] is not None:
            pos[(p['file'], p['table_index'])] = p['position']
    sel = sorted(pos[(r['file'], r['table_index'])] for r in rows if r['changed_vs_evidence'] or r['category'] != 'whole')
    dump(sel, a.out + '.live_positions.json')
    return 0


# --------------------------------------------------------------------------------------------------------- livecompile
def cmd_livecompile(a):
    pos = [json.loads(l) for l in open(a.positions)]
    want = set(json.load(open(a.selected))) if a.selected else {p['position'] for p in pos}
    got = {}
    for f in sorted(glob.glob(a.shards)):
        for l in open(f):
            try:
                x = json.loads(l)
            except ValueError:
                continue
            got[x['position']] = x
    rows, probs, cnt = [], [], {}
    for p in pos:
        if p['position'] not in want:
            continue
        x = got.get(p['position'])
        if x and (x['table'] != p['source'] or x['label'] != p['label']):
            probs.append('position %d: harness says %s#%s, sources list %s#%s' % (p['position'], x['table'], x['label'], p['source'], p['label']))
        st = x['live'] if x else 'not run'
        cnt[st] = cnt.get(st, 0) + 1
        rows.append(dict(p, live=st, witness=x.get('witness') if x else None, why=x.get('why') if x else None, seconds=x.get('seconds') if x else None))
    with open(a.out + '.jsonl', 'w') as g:
        for r in rows:
            g.write(json.dumps(r) + '\n')
    summ = {'selected': len(want), 'by_verdict': cnt, 'problems': probs, 'region': a.region,
            'undecided_positions': [r['position'] for r in rows if r['live'] in ('undecided', 'not run')]}
    dump(summ, a.out + '.summary.json'); print(json.dumps({k: v for k, v in summ.items() if k != 'undecided_positions'}))
    return FAIL if probs else 0


# -------------------------------------------------------------------------------------------------------------- fmkeys
def read_rrkeys(path, n, seed, mask_check=None):
    with open(path, 'rb') as f:
        magic = f.read(8)
        if magic != b'RRKEYS01':
            raise SystemExit('%s: not RRKEYS01' % path)
        hl = int.from_bytes(f.read(4), 'little')
        hdr = json.loads(f.read(hl))
        ar = hdr['arity']; rec = ar + 2; off = 12 + hl
        total = (os.path.getsize(path) - off) // rec
        rnd = random.Random(seed)
        idx = sorted(rnd.sample(range(total), min(n, total)))
        keys = []
        for i in idx:
            f.seek(off + i * rec)
            b = f.read(rec)
            keys.append([x - 256 if x > 127 else x for x in b[:ar]])
        return hdr, total, keys


def cmd_fmkeys(a):
    keys, meta = [], {}
    if a.arrivals:
        hdr, total, ks = read_rrkeys(a.arrivals, a.n, a.seed)
        keys += [('arrival', k) for k in ks]
        meta = {'arrivals': a.arrivals, 'arrivals_total': total, 'sample': len(ks), 'seed': a.seed, 'frame': hdr.get('frame')}
    for t in terminals_of(a.terminals):
        keys.append(('terminal', list(t)))
    if a.explicit:
        for l in open(a.explicit):
            if l.strip():
                v = json.loads(l)
                keys.append((v.get('name', 'explicit'), parse_key(v['key'])))
    seen, out = set(), []
    for kind, k in keys:
        if tuple(k) in seen:
            continue
        seen.add(tuple(k)); out.append((kind, k))
    with open(a.out + '.jsonl', 'w') as g, open(a.out + '.plain.jsonl', 'w') as h:
        for kind, k in out:
            g.write(json.dumps({'kind': kind, 'key': k}) + '\n'); h.write(json.dumps(k) + '\n')
    meta['keys'] = len(out)
    dump(meta, a.out + '.meta.json'); print(json.dumps(meta))
    return 0


def cmd_fmcompare(a):
    """Engine label (rule index of the file it loaded: the final table, or the concatenation core-then-beyond) vs the core
    dispatcher's (source table, label) mapped through the positions list; keys the core serves by a recipe source are counted
    apart (the engine binary's rr_node does not dispatch recipes)."""
    terms = {tuple(k) for k in terminals_of(a.terminals)}
    scan = json.load(open(a.scan))
    offs, acc = [], 0
    for t in scan['tables']:
        offs.append(acc); acc += t['rules']
    kinds = {}
    for l in open(a.keys):
        if l.strip():
            v = json.loads(l); kinds[tuple(v['key'])] = v['kind']
    pos = {}
    for l in open(a.positions):
        p = json.loads(l); pos[(p['source'], p['label'])] = p
    core = {tuple(json.loads(l)['key']): json.loads(l) for l in open(a.core) if l.strip()}
    eng = {tuple(json.loads(l)['key']): json.loads(l) for l in open(a.engine) if l.strip()}
    agree = recipe = 0
    mism, open_core, open_eng, missing, recipe_keys = [], [], [], [], []
    for k, kind in kinds.items():
        c = core.get(k); e = eng.get(k)
        if c is None or e is None:
            missing.append(key_text(k)); continue
        el = e.get('label')
        if c.get('label') is None:
            open_core.append(key_text(k))
            if el is None:
                agree += 1
            else:
                mism.append({'key': key_text(k), 'kind': kind, 'core': None, 'engine': el, 'core_dispatch': c.get('dispatch')})
            continue
        p = pos.get((c.get('table'), c['label']))
        if p is None:
            mism.append({'key': key_text(k), 'kind': kind, 'core': [c.get('table'), c['label']], 'engine': el, 'note': 'core source not in the positions list'}); continue
        if p['file'] is None:
            recipe += 1; recipe_keys.append({'key': key_text(k), 'kind': kind, 'recipe': p['source'], 'engine': el}); continue
        want = offs[p['file']] + p['table_index']
        if el is None:
            open_eng.append(key_text(k)); mism.append({'key': key_text(k), 'kind': kind, 'core': want, 'engine': None})
        elif el == want:
            agree += 1
        else:
            mism.append({'key': key_text(k), 'kind': kind, 'core': want, 'engine': el})
    open_not_terminal = sorted({x for x in open_core + open_eng if tuple(parse_key(x)) not in terms})
    out = {'keys': len(kinds), 'agree': agree, 'recipe_served_by_core': recipe, 'recipe_keys': recipe_keys[:50], 'mismatch_count': len(mism),
           'mismatches': mism[:50], 'open_core': open_core, 'open_engine_only': open_eng, 'open_not_terminal': open_not_terminal,
           'missing_count': len(missing), 'missing': missing[:20], 'engine_time': time_v(a.engine_err) if a.engine_err else None,
           'acceptance': {key_text(k): {'core': [core[k].get('table'), core[k].get('label')] if k in core else None, 'engine': eng[k].get('label') if k in eng else None}
                          for k, kind in kinds.items() if kind not in ('arrival', 'terminal')}}
    out['pass'] = not mism and not open_not_terminal and not missing
    dump(out, a.out); print(json.dumps({k: v for k, v in out.items() if k not in ('mismatches', 'recipe_keys')})[:1500])
    return 0 if out['pass'] else FAIL


def cmd_concat(a):
    """The two-file (or n-file) ship as ONE table in dispatch order (file 0's header / zero_sectors / chart, the rules of every file
    renumbered, the residuals, one end record): the engine dispatch "core then beyond" as the importer and rr_node read one --table."""
    files = a.tables.split(':')
    n = 0; heads = []; tail = []; rules_out = open(a.out + '.tmp', 'wb')
    for fi, path in enumerate(files):
        for raw in open(path, 'rb'):
            v = json.loads(raw)
            if is_rule(v):
                v.setdefault('origin', {})['concat_source'] = {'file': fi, 'index': v['index']}
                v['index'] = n; n += 1
                rules_out.write(json.dumps(v, separators=(',', ':')).encode() + b'\n')
            elif v.get('type') == 'residual':
                tail.append(raw)
            elif v.get('type') == 'end':
                continue
            elif fi == 0:
                (heads if not n else tail).append(raw)
    rules_out.close()
    with open(a.out, 'wb') as g:
        for h in heads:
            g.write(h)
        with open(a.out + '.tmp', 'rb') as r:
            for raw in r:
                g.write(raw)
        for t in tail:
            g.write(t)
        g.write(json.dumps({'type': 'end', 'charts': 1, 'rules': n, 'residuals': sum(1 for t in tail if b'"residual"' in t)}).encode() + b'\n')
    os.remove(a.out + '.tmp')
    out = {'files': files, 'rules': n, 'out': a.out, 'sha256': sha256_file(a.out)}
    dump(out, a.out + '.json'); print(json.dumps(out))
    return 0


# ---------------------------------------------------------------------------------------------- one-level drill (engine)
def cmd_drilltable(a):
    """The final table with the point-rule relations of a table_dump.v1 file inserted before rule `position` (indices renumbered)."""
    pts = [json.loads(l) for l in open(a.points) if l.strip()]
    pts = [v for v in pts if is_rule(v)]
    n = 0
    with open(a.table, 'rb') as f, open(a.out + '.tmp', 'wb') as g:
        tail = []
        for raw in f:
            v = json.loads(raw)
            if is_rule(v):
                if v['index'] == a.position:
                    for p in pts:
                        p = dict(p, index=n); g.write(json.dumps(p, separators=(',', ':')).encode() + b'\n'); n += 1
                v['index'] = n; n += 1
                g.write(json.dumps(v, separators=(',', ':')).encode() + b'\n')
            else:
                if v.get('type') == 'end':
                    v['rules'] = n; tail.append(json.dumps(v).encode() + b'\n')
                elif v.get('type') in ('residual', 'chart') and n > 0:
                    tail.append(raw)
                else:
                    g.write(raw)
        for t in tail:
            g.write(t)
    os.replace(a.out + '.tmp', a.out)
    out = {'table': a.table, 'points': a.points, 'points_sha256': sha256_file(a.points), 'inserted': len(pts), 'before_rule': a.position,
           'rules': n, 'drill_table': a.out, 'drill_table_sha256': sha256_file(a.out)}
    dump(out, a.out + '.json'); print(json.dumps(out))
    return 0


def rule_children(v, k, mask):
    kids = set()
    for t in v['rhs']:
        pat = t[0]
        slots = pat[pat.index('(') + 1:-1].split(',')
        c = []
        for s in slots:
            m = re.fullmatch(r'\s*n(\d+)([+-]\d+)?\s*', s)
            c.append(k[int(m.group(1))] + int(m.group(2) or 0) if m else int(s))
        if all(c[i] >= 1 for i, m in enumerate(mask) if m == '1') and c != k:
            kids.add(tuple(c))
    return sorted(kids)


def read_record(path, offsets, index):
    off, ln = offsets[index]
    with open(path, 'rb') as f:
        f.seek(off)
        return json.loads(f.read(ln))


def line_offsets(index_path):
    offs = {}
    for l in open(index_path):
        e = json.loads(l)
        if e['index'] is not None:
            offs[e['index']] = (e['offset'], e['length'])
    return offs


def cmd_onejobs(a):
    """Jobs: per acceptance key its first-match rule (core, given) and its in-sector children; level 1 = the children (capped)."""
    terms = [tuple(k) for k in terminals_of(a.terminals)]
    mask = json.load(open(a.scan))['mask']
    offs = line_offsets(a.index)
    core = {}
    for f in a.core.split(':'):
        for l in open(f):
            v = json.loads(l); core[tuple(v['key'])] = v
    pos = {}
    for l in open(a.positions):
        p = json.loads(l); pos[(p['source'], p['label'])] = p
    jobs, need = [], []
    for l in open(a.keys):
        if not l.strip():
            continue
        x = json.loads(l); k = parse_key(x['key']); name = x.get('name', key_text(k))
        c = core.get(tuple(k))
        if c is None:
            need.append(k); continue
        lab = c.get('label')
        if lab is None or lab not in offs or (pos.get((c.get('table'), lab)) or {}).get('file') is None:
            jobs.append({'acceptance': name, 'level': 0, 'key': k, 'rule': None, 'why': c.get('dispatch')}); continue
        kids = rule_children(read_record(a.table, offs, lab), k, mask)
        jobs.append({'acceptance': name, 'level': 0, 'key': k, 'rule': lab, 'children': [list(t) for t in kids]})
        l1 = [t for t in kids if t not in terms]
        if len(l1) > a.cap1:
            l1 = [l1[int(i * len(l1) / a.cap1)] for i in range(a.cap1)]
        for t in l1:
            cc = core.get(t)
            if cc is None:
                need.append(list(t)); continue
            cl = cc.get('label')
            if cl is None or cl not in offs or (pos.get((cc.get('table'), cl)) or {}).get('file') is None:
                jobs.append({'acceptance': name, 'level': 1, 'key': list(t), 'rule': None, 'why': cc.get('dispatch') or 'served outside the table (recipe)'}); continue
            jobs.append({'acceptance': name, 'level': 1, 'key': list(t), 'rule': cl, 'children': [list(z) for z in rule_children(read_record(a.table, offs, cl), list(t), mask)]})
    with open(a.out, 'w') as g:
        for j in jobs:
            g.write(json.dumps(j) + '\n')
    with open(a.out + '.need.jsonl', 'w') as g:
        for k in need:
            g.write(json.dumps(k) + '\n')
    print(json.dumps({'jobs': len(jobs), 'need_core_first_match': len(need)}))
    return 0


def cmd_onerun(a):
    import concurrent.futures as cf
    terms = [key_text(k) for k in terminals_of(a.terminals)]
    jobs = [json.loads(l) for l in open(a.jobs) if l.strip()]
    os.makedirs(a.work, exist_ok=True)

    def run(j):
        r = dict(j); r.pop('children', None); r['n_children'] = len(j.get('children', []))
        if j.get('rule') is None:
            r['engine'] = {'skipped': j.get('why')}; return r
        key = key_text(j['key'])
        masters = terms + [key_text(c) for c in j['children']]
        out = os.path.join(a.work, '%s.json' % '_'.join(map(str, j['key'])))
        t0 = time.time()
        pp = subprocess.Popen(['prlimit', '--core=0', '--as=%d' % (a.as_gb * 10**9), '/run/current-system/sw/bin/time', '-v', a.rr_node, 'reduce', '--table', a.table,
                               '--keys', key, '--masters', ';'.join(masters), '--out', out], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
        try:
            so, se = pp.communicate(timeout=a.wall)
        except subprocess.TimeoutExpired:
            import signal
            os.killpg(pp.pid, signal.SIGKILL); so, se = pp.communicate()
            r['engine'] = {'rc': 124, 'timeout': True, 'wall_s': round(time.time() - t0, 1)}; return r
        m = re.search(r'Maximum resident set size \(kbytes\): (\d+)', se)
        r['engine'] = {'rc': pp.returncode, 'message': ' | '.join(l for l in se.splitlines() if l.startswith('rr_node:'))[:400],
                       'wall_s': round(time.time() - t0, 1), 'peak_rss_gb': round(int(m.group(1)) / 1e6, 2) if m else None}
        if os.path.exists(out):
            os.remove(out)
        return r
    res = []
    with open(a.out, 'w') as g, cf.ThreadPoolExecutor(a.parallel) as ex:
        for r in ex.map(run, jobs):
            res.append(r); g.write(json.dumps(r) + '\n'); g.flush()
    ran = [r for r in res if 'rc' in r['engine']]
    bad = [r for r in ran if r['engine']['rc'] != 0]
    summ = {'jobs': len(res), 'ran': len(ran), 'clean': len(ran) - len(bad), 'not_clean': [{k: r[k] for k in ('acceptance', 'level', 'key', 'rule', 'engine')} for r in bad],
            'skipped': [{k: r.get(k) for k in ('acceptance', 'level', 'key', 'why')} for r in res if 'skipped' in r['engine']],
            'by_acceptance': {n: sum(1 for r in ran if r['acceptance'] == n and r['engine']['rc'] == 0) for n in sorted({r['acceptance'] for r in res})},
            'peak_rss_gb_max': max((r['engine'].get('peak_rss_gb') or 0 for r in ran), default=None), 'wall_s_max': max((r['engine']['wall_s'] for r in ran), default=None)}
    summ['pass'] = not bad
    dump(summ, a.out + '.summary.json'); print(json.dumps(summ)[:1500])
    return 0 if summ['pass'] else FAIL


# ------------------------------------------------------------------------------------------------------------ candcheck
def rule_facts(path):
    """Rules of a table in FILE order: (index, target_rhs_sha256 'list', domain_sha256, dispatch_position); residual keys; end."""
    rules, res, end = [], [], None
    with open(path, 'rb') as f:
        for raw in f:
            if not raw.strip():
                continue
            v = json.loads(raw)
            if is_rule(v):
                rules.append((v.get('index'), target_rhs_hash(v, 'list'),
                              hashlib.sha256(json.dumps([v.get('fixed'), v.get('equations'), v.get('exceptions'), v.get('domain_bounds')], separators=(',', ':')).encode()).hexdigest(),
                              (v.get('origin') or {}).get('dispatch_position')))
            elif v.get('type') == 'residual':
                res.append(tuple(parse_key(v['key'])))
            elif v.get('type') == 'end':
                end = v
    return rules, res, end


def cmd_candcheck(a):
    """A candidate table against its stated composition: the reference table's rules in file order, with the rule of index
    `replace_index` replaced by the sub-table's rules (in their order). Checks: rule index = file position (the importer's
    order); per position target + rhs hash and domain hash equal to the expected rule; origin.dispatch_position equal to the
    reference's (sub-table rules: reported); residuals = terminal set; end.rules = rule count."""
    ref, _, _ = rule_facts(a.reference)
    sub, _, _ = rule_facts(a.sub) if a.sub else ([], None, None)
    exp = []
    for r in ref:
        if a.replace_index is not None and r[0] == a.replace_index and sub:
            exp += [('sub', k, x[1], x[2], None) for k, x in enumerate(sub)]
        else:
            exp.append(('ref', r[0], r[1], r[2], r[3]))
    got, res, end = rule_facts(a.candidate)
    probs, mism = [], []
    if len(got) != len(exp):
        probs.append('rules: candidate %d, expected %d' % (len(got), len(exp)))
    bad_index = [i for i, g in enumerate(got) if g[0] != i]
    if bad_index:
        probs.append('%d rule(s) whose index != file position (first at %d: index %s)' % (len(bad_index), bad_index[0], got[bad_index[0]][0]))
    sub_dp = []
    for i, (g, e) in enumerate(zip(got, exp)):
        if g[1] != e[2] or g[2] != e[3]:
            mism.append({'position': i, 'expected': [e[0], e[1]], 'target_rhs_equal': g[1] == e[2], 'domain_equal': g[2] == e[3]})
        if e[0] == 'ref' and g[3] != e[4]:
            mism.append({'position': i, 'expected': [e[0], e[1]], 'dispatch_position': [g[3], e[4]]})
        if e[0] == 'sub':
            sub_dp.append([i, g[3]])
    if mism:
        probs.append('%d per-rule mismatch(es)' % len(mism))
    terms = terminals_of(a.terminals)
    if sorted(res) != terms:
        probs.append('residuals %s != terminals' % [key_text(k) for k in res])
    if not end or end.get('rules') != len(got):
        probs.append('end record %s' % end)
    out = {'candidate': a.candidate, 'candidate_sha256': sha256_file(a.candidate), 'reference': a.reference, 'reference_sha256': sha256_file(a.reference),
           'sub': a.sub, 'sub_sha256': sha256_file(a.sub) if a.sub else None, 'replace_index': a.replace_index, 'rules': len(got), 'expected_rules': len(exp),
           'sub_rules_at': [i for i, e in enumerate(exp) if e[0] == 'sub'], 'sub_rules_dispatch_position': sub_dp,
           'mismatches': mism[:50], 'pass': not probs, 'problems': probs}
    dump(out, a.out); print(json.dumps({k: v for k, v in out.items() if k not in ('mismatches',)})[:1500])
    return 0 if not probs else FAIL


# ------------------------------------------------------------------------------------------------------ summary / deliver
STEPS = ['prep', 'cert', 'term', 'b1', 'exact', 'imp_plain', 'imp_shared', 'live', 'fm', 'one', 'deliver']


def run_env(run):
    """The cfg_* / FC_* scalars of run.env (declare -p lines); arrays as lists."""
    env = {}
    for l in open(os.path.join(run, 'run.env')):
        m = re.match(r'declare -(\S+) (\w+)=(.*)$', l.rstrip('\n'))
        if not m:
            m2 = re.match(r'(\w+)=(.*)$', l.rstrip('\n'))
            if m2:
                env[m2.group(1)] = m2.group(2)
            continue
        flags, name, val = m.groups()
        if 'a' in flags:
            env[name] = re.findall(r'\[\d+\]="((?:[^"\\]|\\.)*)"', val)
        else:
            env[name] = val[1:-1] if val.startswith('"') and val.endswith('"') else val
    return env


def step_verdicts(run, env):
    N = env['cfg_node']
    v = {}
    def jl(p):
        try:
            return json.load(open(p))
        except (OSError, ValueError):
            return None
    for st in STEPS:
        rc = open(os.path.join(run, 'steps', st + '.rc')).read().strip() if os.path.exists(os.path.join(run, 'steps', st + '.rc')) else None
        v[st] = {'rc': rc, 'done': os.path.exists(os.path.join(run, 'steps', st + '.done')), 'failed': os.path.exists(os.path.join(run, 'steps', st + '.fail'))}
    v['cert']['verdict'] = jl(os.path.join(run, 'cert', 'cert.verdict.json'))
    v['term']['verdict'] = jl(os.path.join(run, 'term', 'term.verdict.json'))
    v['b1']['verdict'] = jl(os.path.join(run, 'b1', 'b1.verdict.json'))
    v['exact']['summary'] = jl(os.path.join(run, 'exact', N + '.exact.summary.json'))
    v['live']['summary'] = jl(os.path.join(run, 'live', N + '.liveness.summary.json'))
    v['fm']['verdict'] = jl(os.path.join(run, 'fm', 'fm.verdict.json'))
    v['one']['summary'] = jl(os.path.join(run, 'one', 'onelevel.results.jsonl.summary.json'))
    v['prep']['scan'] = jl(os.path.join(run, 'prep', N + '.scan.json'))
    if v['prep']['scan']:
        v['prep']['scan'].pop('dispatch_positions', None)
    imps = {}
    for d in sorted(glob.glob(os.path.join(run, 'imp', '*'))):
        imps[os.path.basename(d)] = jl(os.path.join(d, 'impcheck.json'))
    v['imports'] = imps
    return v


def cmd_summary(a):
    env = run_env(a.run)
    v = step_verdicts(a.run, env)
    out = {'schema': 'lane-parametric.final-certification.summary.v1', 'label': env.get('FC_LABEL'), 'node': env['cfg_node'], 'state': a.state,
           'utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), 'candidate_files': env['FC_CAND_FILES'].split(':'),
           'candidate_sha256': env['FC_CAND_SHAS'].split(':'), 'engine_model': env.get('cfg_engine_model'), 'rustred': env.get('cfg_rustred'),
           'rr_node': env.get('cfg_rr_node'), 'harness': env.get('cfg_harness'), 'cones': env.get('cfg_cones'), 'exact_cones': env.get('cfg_exact_cones'),
           'terminals': env.get('cfg_terminals'), 'steps': v}
    dump(out, os.path.join(a.run, 'summary.json'))
    print(json.dumps({'state': a.state, 'steps': {k: (x.get('done'), x.get('failed')) for k, x in v.items() if isinstance(x, dict) and 'done' in x}}))
    return 0


def cmd_deliver(a):
    import shutil
    run = a.run; env = run_env(run); N = env['cfg_node']; label = env.get('FC_LABEL', 'FINAL')
    if os.path.exists(a.out) and os.listdir(a.out):
        print('delivery directory %s exists and is not empty: refused' % a.out); return FAIL
    os.makedirs(a.out, exist_ok=True)
    def cp(src, dst):
        if os.path.exists(src):
            os.makedirs(os.path.dirname(dst), exist_ok=True); shutil.copy2(src, dst)
    refs = {}
    # tables: hard links (same dataset), else referenced by path + sha256
    for fi, (path, sha) in enumerate(zip(env['FC_CAND_FILES'].split(':'), env['FC_CAND_SHAS'].split(':'))):
        dst = os.path.join(a.out, 'table', os.path.basename(path))
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        try:
            os.link(path, dst)
        except OSError:
            refs['table/' + os.path.basename(path)] = {'path': path, 'sha256': sha}
    for f in glob.glob(os.path.join(run, 'prep', '*')):
        if os.path.isfile(f) and not f.endswith(('.domain.jsonl', '.ship.concat.jsonl')):
            cp(f, os.path.join(a.out, 'prep', os.path.basename(f)))
    for f in glob.glob(os.path.join(run, 'prep', '*.domain.jsonl')) + glob.glob(os.path.join(run, 'prep', '*.ship.concat.jsonl')):
        refs['prep/' + os.path.basename(f)] = {'path': f, 'sha256': sha256_file(f), 'bytes': os.path.getsize(f)}
    if os.path.exists(os.path.join(run, 'prep', N + '.recipes.jsonl')):
        cp(os.path.join(run, 'prep', N + '.recipes.jsonl'), os.path.join(a.out, 'table', N + '.recipes.jsonl'))
    for sub in ('cert', 'term', 'b1', 'exact', 'live', 'fm'):
        for root, _, names in os.walk(os.path.join(run, sub)):
            for nm in names:
                src = os.path.join(root, nm)
                cp(src, os.path.join(a.out, {'cert': 'certificate'}.get(sub, sub), os.path.relpath(src, os.path.join(run, sub))))
    for f in glob.glob(os.path.join(run, 'one', '*')):
        if os.path.isfile(f) and not f.endswith('.drill.table.jsonl'):
            cp(f, os.path.join(a.out, 'one', os.path.basename(f)))
        elif f.endswith('.drill.table.jsonl'):
            refs['one/' + os.path.basename(f)] = {'path': f, 'sha256': sha256_file(f), 'bytes': os.path.getsize(f)}
    for d in glob.glob(os.path.join(run, 'imp', '*')):
        for f in glob.glob(os.path.join(d, '*')):
            if os.path.isfile(f):
                cp(f, os.path.join(a.out, 'import', os.path.basename(d), os.path.basename(f)))
        for f in glob.glob(os.path.join(d, 'out', '*')):
            refs['import/%s/out/%s' % (os.path.basename(d), os.path.basename(f))] = {'path': f, 'sha256': sha256_file(f), 'bytes': os.path.getsize(f)}
    for f in ('run.env', 'LABEL'):
        cp(os.path.join(run, f), os.path.join(a.out, 'run', f))
    for f in (env.get('cfg_delivery_extra') or '').split(':'):
        if f and os.path.isfile(f):
            cp(f, os.path.join(a.out, 'extra', os.path.basename(f)))
    for f in glob.glob(os.path.join(run, 'log', '*')):
        cp(f, os.path.join(a.out, 'run', 'log', os.path.basename(f)))
    for f in glob.glob(os.path.join(run, 'steps', '*')):
        cp(f, os.path.join(a.out, 'run', 'steps', os.path.basename(f)))
    kit = env.get('FC_KIT_FROZEN')
    for f in ('fc_lib.sh', 'fc_tool.py', 'final_cert_%s.sh' % N):
        cp(os.path.join(kit, f), os.path.join(a.out, 'run', 'kit', f))
    # binaries named by the run (sha256)
    bins = {}
    for k in ('cfg_harness', 'cfg_rustred', 'cfg_rr_node'):
        if env.get(k) and os.path.exists(env[k]):
            bins[k[4:]] = {'path': env[k], 'sha256': sha256_file(env[k])}
    refs['binaries'] = bins
    for e in env.get('cfg_inputs', []):
        pth, _, want = e.partition('=')
        refs.setdefault('inputs', {})[pth] = want
    dump(refs, os.path.join(a.out, 'referenced_outside.json'))
    v = step_verdicts(run, env)
    dump({'label': label, 'node': N, 'candidate_files': env['FC_CAND_FILES'].split(':'), 'candidate_sha256': env['FC_CAND_SHAS'].split(':'), 'steps': v},
         os.path.join(a.out, 'summary.json'))
    write_verifier_spec(a.out, env, v)
    cmd_manifest(argparse.Namespace(dir=a.out, label=label, extra=os.path.join(a.out, 'referenced_outside.json')))
    return 0


def write_verifier_spec(out, env, v):
    N = env['cfg_node']; label = env.get('FC_LABEL', 'FINAL')
    terms = [key_text(k) for k in terminals_of(env['cfg_terminals'])]
    files = env['FC_CAND_FILES'].split(':'); shas = env['FC_CAND_SHAS'].split(':')
    cert = (v['cert'].get('verdict') or {}).get('cones', {})
    ex = v['exact'].get('summary') or {}
    L = []
    L.append('# %s final certification (%s): what the INDEPENDENT verifier must check (lane parametric, %s)' % (N, label, time.strftime('%Y-%m-%d')))
    L.append('')
    if label != 'FINAL':
        L.append('**%s: NOT a certificate of the real table (launcher rehearsal on a cut region).**' % label); L.append('')
    L.append('Write the verifier without reading lane parametric\'s code path (rustred-campaign-core/src/parametric/*, partition/*, loci/*).')
    L.append('The engine sources (rustred-core) and the core table reader may be used. Every input is bound by sha256 in MANIFEST.json')
    L.append('(files of this directory) and referenced_outside.json (large or external files: path + sha256).')
    L.append('')
    L.append('## Inputs')
    for f, h in zip(files, shas):
        L.append('- table file (dispatch order: files in this order, rules in table order): `table/%s` sha256 `%s`' % (os.path.basename(f), h))
    if env.get('cfg_recipes'):
        L.append('- bounded recipes: `table/%s.recipes.jsonl` (rustred.campaign.recipe-record.v1; `position` = the number of table rules dispatched before them,'
                 ' derived from origin.dispatch_position < %s; seal manifests and coverage records bound inside each record)' % (N, env.get('cfg_recipe_before_dp')))
    L.append('- terminal set (explicit input): `%s` = %s' % (env['cfg_terminals'], ', '.join(terms)))
    L.append('- engine model of the certificate: %s (exact zero test, slot-permuted aliases in table order); engine binaries of the import and drills:'
             ' see referenced_outside.json "binaries"' % env.get('cfg_engine_model'))
    L.append('')
    L.append('## The statement to check')
    L.append('1. COVERAGE at first match on each cone of [%s]: the engine\'s first match (table order; recipes at their position, inside'
             ' their bound) selects a record for every top-sector key of the cone, except exactly the terminal set.' % env.get('cfg_exact_cones'))
    for c, r in cert.items():
        L.append('   - %s: uncovered elements %s, undecided %s, open residual keys %s, certificate %s (`%s`)' % (c, r.get('uncovered_elements'), r.get('undecided_elements'),
                 r.get('residual_open_keys'), r.get('certificate_status'), os.path.basename(r.get('certificate', ''))))
    L.append('2. VALIDITY: every witness locus of a record lies inside its exception branches on its domain (records carry `witness_loci`;'
             ' `[]` only with an explicit exact proof in origin.derivation).')
    L.append('3. STRICT DESCENT of every rhs term on the record\'s whole domain (b1/ has lane parametric\'s result: does-not-descend 0;'
             ' "descends except" = on numerator planes where the engine fails closed).')
    L.append('4. TERMINAL RULE: no record contains a terminal key; no coverage stub.')
    cfgx = json.load(open(env['cfg_exact_config']))
    forms = ', '.join('%s: %s' % (e['name'], e.get('hash_form', 'list')) for e in cfgx['evidence'])
    L.append('5. EXACTNESS, keyed by the hash of target + rhs per record: form "list" = sha256(json.dumps([target, rhs], separators=(",", ":"))),'
             ' form "dict_sorted" = sha256(json.dumps({"target": .., "rhs": ..}, sort_keys=True, separators=(",", ":"))); evidence sources and'
             ' their forms: %s. `exact/%s.exact.jsonl` lists every record with its hashes (`target_rhs_sha256_forms`), dispatch position,'
             ' the evidence class it re-binds to and the category (whole domain / per key / NOT proven). A record whose hash is in no'
             ' evidence ("changed") needs a new proof.' % (forms, N))
    if ex:
        L.append('   - by category: %s; changed vs evidence: %d; not proven: %d; watch list: %s' % (ex.get('by_category'), len(ex.get('changed_vs_evidence', [])),
                 len(ex.get('not_proven', [])), [(w['dispatch_position'], w['category']) for w in ex.get('watch_list', [])]))
    L.append('6. ENGINE: strict native import + --check (import/*/impcheck.json: rc 0, "0 open box keys", the terminal count);'
             ' first-match agreement engine vs core on the drill keys (fm/fm.verdict.json).')
    L.append('')
    L.append('## Not proven / open (never silent)')
    L.append('Source: exact/%s.exact.summary.json (`not_proven`, `per_key`, `changed_vs_evidence`, `watch_list`); first-match liveness in %s'
             ' of each from live/%s.liveness.jsonl.' % (N, env.get('cfg_region'), N))
    live = {}
    lp = os.path.join(out, 'live', N + '.liveness.jsonl')
    if os.path.exists(lp):
        for l in open(lp):
            r = json.loads(l)
            if r.get('file') is not None:
                live[(r['file'], r['table_index'])] = r['live']
    def row(r):
        return '| %s | %s | %s | %s | %s | %s |' % (r.get('file'), r.get('table_index'), r.get('dispatch_position'), r.get('bytes'),
                                                   (r.get('class') or 'no evidence (changed)')[:90], live.get((r.get('file'), r.get('table_index')), 'not run'))
    for title, key in (('NOT proven', 'not_proven'), ('per key only', 'per_key'), ('changed vs evidence (hash in no evidence)', 'changed_vs_evidence'),
                       ('watch list (%s)' % cfgx.get('watch_note', ''), 'watch_list')):
        rows = ex.get(key, [])
        L.append(''); L.append('### %s: %d record(s)' % (title, len(rows)))
        if rows:
            L.append('| file | table index | dispatch position | bytes | class | first match in %s |' % env.get('cfg_region'))
            L.append('|---|---|---|---|---|---|')
            L.extend(row(r) for r in rows[:400])
    notes = env.get('cfg_delivery_notes')
    if notes and os.path.exists(notes):
        L.append(''); L.append(open(notes).read().strip())
    open(os.path.join(out, 'VERIFIER_SPEC.md'), 'w').write('\n'.join(L) + '\n')


# ------------------------------------------------------------------------------------------------------------ manifest
def cmd_manifest(a):
    files = {}
    for root, _, names in os.walk(a.dir):
        for nm in sorted(names):
            p = os.path.join(root, nm)
            rel = os.path.relpath(p, a.dir)
            if rel == 'MANIFEST.json':
                continue
            files[rel] = {'bytes': os.path.getsize(p), 'sha256': sha256_file(p)}
    out = {'schema': 'lane-parametric.final-certification.manifest.v1', 'dir': a.dir, 'label': a.label, 'utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
           'files': files}
    if a.extra:
        out['referenced_outside'] = json.load(open(a.extra))
    dump(out, os.path.join(a.dir, 'MANIFEST.json')); print('%d files' % len(files))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sp = ap.add_subparsers(dest='cmd', required=True)
    p = sp.add_parser('prep'); p.add_argument('--node', required=True); p.add_argument('--label', required=True); p.add_argument('--tables', required=True)
    p.add_argument('--terminals', required=True); p.add_argument('--recipes', default=''); p.add_argument('--recipe-before-dp', type=int, default=-1); p.add_argument('--out', required=True)
    p = sp.add_parser('certcheck'); p.add_argument('--summary', required=True); p.add_argument('--terminals', required=True); p.add_argument('--cones', required=True)
    p.add_argument('--exact-cones', required=True); p.add_argument('--orthant-undecided', type=int, default=None); p.add_argument('--out', required=True)
    p = sp.add_parser('termcheck'); p.add_argument('--file', required=True); p.add_argument('--out', required=True)
    p = sp.add_parser('b1check'); p.add_argument('--summary', required=True); p.add_argument('--out', required=True)
    p = sp.add_parser('impcheck'); p.add_argument('--dir', required=True); p.add_argument('--terminals', required=True)
    p = sp.add_parser('exact'); p.add_argument('--tables', required=True); p.add_argument('--config', required=True); p.add_argument('--positions', required=True)
    p.add_argument('--side-dirs', default=''); p.add_argument('--out', required=True)
    p = sp.add_parser('livecompile'); p.add_argument('--positions', required=True); p.add_argument('--shards', required=True); p.add_argument('--selected', default='')
    p.add_argument('--region', required=True); p.add_argument('--out', required=True)
    p = sp.add_parser('fmkeys'); p.add_argument('--arrivals', default=''); p.add_argument('--n', type=int, default=20000); p.add_argument('--seed', type=int, default=20261117)
    p.add_argument('--terminals', required=True); p.add_argument('--explicit', default=''); p.add_argument('--out', required=True)
    p = sp.add_parser('fmcompare'); p.add_argument('--keys', required=True); p.add_argument('--core', required=True); p.add_argument('--engine', required=True)
    p.add_argument('--engine-err', default=''); p.add_argument('--positions', required=True); p.add_argument('--terminals', required=True); p.add_argument('--scan', required=True)
    p.add_argument('--out', required=True)
    p = sp.add_parser('drilltable'); p.add_argument('--table', required=True); p.add_argument('--points', required=True); p.add_argument('--position', type=int, required=True)
    p.add_argument('--out', required=True)
    p = sp.add_parser('onejobs'); p.add_argument('--table', required=True); p.add_argument('--index', required=True); p.add_argument('--scan', required=True); p.add_argument('--keys', required=True)
    p.add_argument('--core', required=True); p.add_argument('--positions', required=True); p.add_argument('--terminals', required=True); p.add_argument('--cap1', type=int, default=40)
    p.add_argument('--out', required=True)
    p = sp.add_parser('onerun'); p.add_argument('--jobs', required=True); p.add_argument('--table', required=True); p.add_argument('--rr-node', required=True)
    p.add_argument('--terminals', required=True); p.add_argument('--parallel', type=int, default=3); p.add_argument('--wall', type=int, default=1800); p.add_argument('--as-gb', type=int, default=30)
    p.add_argument('--work', required=True); p.add_argument('--out', required=True)
    p = sp.add_parser('candcheck'); p.add_argument('--candidate', required=True); p.add_argument('--reference', required=True); p.add_argument('--sub', default='')
    p.add_argument('--replace-index', type=int, default=None); p.add_argument('--terminals', required=True); p.add_argument('--out', required=True)
    p = sp.add_parser('concat'); p.add_argument('--tables', required=True); p.add_argument('--out', required=True)
    p = sp.add_parser('summary'); p.add_argument('--run', required=True); p.add_argument('--state', required=True)
    p = sp.add_parser('deliver'); p.add_argument('--run', required=True); p.add_argument('--out', required=True)
    p = sp.add_parser('manifest'); p.add_argument('--dir', required=True); p.add_argument('--label', required=True); p.add_argument('--extra', default='')
    a = ap.parse_args()
    sys.exit(globals()['cmd_' + a.cmd](a))


if __name__ == '__main__':
    main()
