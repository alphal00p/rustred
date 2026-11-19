#!/usr/bin/env python3
"""release.py: the delivery side of the heavy 1-5 p package (vk5l WP6). No network, no tag, no upload.

  release.py manifest <package dir> <release dir> [--selection selection.json] [--link] [--name NAME] [--reuse-verified]
      The release dir must be EMPTY (or absent): a used asset directory is refused, because an asset is never reused by
      its file name (2026-11-18 review: restaging into a used directory published old bytes under a new revision).
      --reuse-verified allows a used directory: an existing asset is reused only if its DECODED content has the
      source file's sha256, otherwise the run is refused before any manifest is written.
      Writes the RELEASE LAYOUT of a written package (`p5l_library assemble` output, `library.json` at its root):
        <release dir>/p-package.json        the package manifest (schema vakint.p-package-release.v1)
        <release dir>/assets/<asset file>   one asset per file of the package:
            core files (library.json, families/*, topology/*, census.rrbin, ordering.json, routes.rrbin, orbits.rrbin)
            are gzip-compressed (<path>.gz); owner payloads (owners/*.rrbin) are copied verbatim (a gzip-encoded payload
            is already compressed: PayloadRecord.encoding); census.json (only the VERIFIED load reads it) is an
            OPTIONAL gzip asset.
      Per asset: the installed path, installed bytes, stored bytes, the sha256 of the STORED bytes (transport
      checksum) and content_sha256 = the sha256 of the INSTALLED (decoded) bytes, i.e. of the source file (the
      install check; never a cache key, never part of an identity). Validity of the package is its version
      constants (library.json identity, P_PACKAGE_REVISION, the p_versions); the manifest repeats them.
      With --selection, the manifest also lists per 5L top the owner payloads it needs (a partial fetch of some tops),
      the open nodes and the boxed nodes. --link hard-links verbatim assets instead of copying (local staging).

  release.py fetch <release dir or file:// URL> <install dir> [--tops T1,T2|--all] [--optional]
      The LOCAL 'fetch from directory' path: installs the package from a release directory (the same layout a
      release download would give). Resumable: a file whose installed size and recorded sha256 match the install
      record is reused; an interrupted transfer continues from <file>.part; every stored asset is checked against
      its manifest sha256 while it streams, a mismatch is refused (the .part is removed) and nothing half-written
      is ever renamed into place. Writes <install dir>/p-package.json and <install dir>/INSTALLED.json (per
      installed file: bytes and the verified transport sha256), so a second fetch is offline (no read of the
      source beyond its manifest) and a load needs no re-hash.

  release.py check <install dir> [--digest]
      Sizes of every installed file against INSTALLED.json (no hashing): what a loader may do on every start;
      --digest also re-hashes every installed file against its content_sha256 and checks library.json's identity.

  release.py compare <install dir> <inventory.json>
      Every installed file against a frozen SOURCE inventory ({"files": {path: {"bytes", "sha256"}}}, the sha256 of
      the uncompressed source files): missing, extra (besides p-package.json / INSTALLED.json) and differing files.

Schema and version constants (simple versions, no digests in any identity):
  P_PACKAGE_RELEASE_SCHEMA = vakint.p-package-release.v1, P_PACKAGE_MANIFEST_VERSION = 2,
  P_INSTALL_RECORD_SCHEMA = vakint.p-package-install.v1.
"""
import gzip
import hashlib
import json
import os
import shutil
import sys
import time

P_PACKAGE_RELEASE_SCHEMA = 'vakint.p-package-release.v1'
# 2 (2026-11-18): content_sha256 per file, checked at install; a v1 manifest has none and is refused.
P_PACKAGE_MANIFEST_VERSION = 2
P_INSTALL_RECORD_SCHEMA = 'vakint.p-package-install.v1'
MANIFEST = 'p-package.json'
INSTALLED = 'INSTALLED.json'
CHUNK = 8 << 20
CORE_PREFIXES = ('families/', 'topology/', 'layer/')
CORE_FILES = ('library.json', 'census.rrbin', 'ordering.json', 'routes.rrbin', 'orbits.rrbin')
OPTIONAL_FILES = ('census.json',)


class Refused(Exception):
    pass


def utc():
    return time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(CHUNK), b''):
            h.update(block)
    return h.hexdigest()


def sha256_gzip_content(path):
    h = hashlib.sha256()
    with gzip.open(path, 'rb') as f:
        for block in iter(lambda: f.read(CHUNK), b''):
            h.update(block)
    return h.hexdigest()


def walk(root):
    out = []
    for d, _, files in os.walk(root):
        for f in files:
            p = os.path.join(d, f)
            out.append(os.path.relpath(p, root).replace(os.sep, '/'))
    return sorted(out)


def kind_of(path):
    if path in OPTIONAL_FILES:
        return 'optional'
    if path.startswith('owners/'):
        return 'payload'
    if path in CORE_FILES or path.startswith(CORE_PREFIXES):
        return 'core'
    return 'other'


def write_manifest(package, release, selection=None, link=False, name=None, reuse_verified=False):
    lib = json.load(open(os.path.join(package, 'library.json')))
    assets = os.path.join(release, 'assets')
    used = (os.path.exists(os.path.join(release, MANIFEST))
            or (os.path.isdir(assets) and any(os.scandir(assets))))
    if used and not reuse_verified:
        raise Refused(f'{release} is not empty: a release layout is written into an EMPTY directory '
                      '(or pass --reuse-verified to reuse only assets whose decoded content matches the source)')
    os.makedirs(assets, exist_ok=True)
    files = {}
    for path in walk(package):
        if path in ('ASSEMBLED.ok',) or path.endswith('.partial'):
            continue
        kind = kind_of(path)
        src = os.path.join(package, path)
        compressed = kind in ('core', 'optional', 'other')
        stored = path.replace('/', '__') + ('.gz' if compressed else '')
        dst = os.path.join(release, 'assets', stored)
        content = sha256_file(src)
        if os.path.exists(dst):
            # only reachable with reuse_verified: reuse only if the asset decodes to the source bytes
            have = sha256_gzip_content(dst) if compressed else sha256_file(dst)
            if have != content:
                raise Refused(f'{dst}: existing asset decodes to sha256 {have}, the source {src} has {content}: '
                              'refused (write the release into an empty directory)')
        if compressed:
            if not os.path.exists(dst):
                with open(src, 'rb') as fi, gzip.open(dst + '.tmp', 'wb', compresslevel=6) as fo:
                    shutil.copyfileobj(fi, fo, CHUNK)
                os.replace(dst + '.tmp', dst)
        elif not os.path.exists(dst):
            if link:
                os.link(src, dst)
            else:
                shutil.copyfile(src, dst + '.tmp')
                os.replace(dst + '.tmp', dst)
        files[path] = {'kind': kind, 'asset': stored, 'encoding': 'gzip' if compressed else None,
                       'bytes': os.path.getsize(src), 'stored_bytes': os.path.getsize(dst),
                       'sha256': sha256_file(dst), 'content_sha256': content}
    owners = {o['class']: o for o in lib['owners']}
    manifest = {
        'schema': P_PACKAGE_RELEASE_SCHEMA,
        'manifest_version': P_PACKAGE_MANIFEST_VERSION,
        'name': name or f"p-library-{lib['scope'][0]}-{lib['scope'][1]}",
        'written_utc': utc(),
        'scope': lib['scope'],
        'library_schema': lib['schema'],
        'identity': lib['identity'],
        'revision': lib['identity'].rsplit(',P_PACKAGE_REVISION=', 1)[1] if ',P_PACKAGE_REVISION=' in lib['identity'] else None,
        'versions': lib['versions'],
        'payload_encoding': sorted({(o.get('payload') or {}).get('encoding') or 'raw'
                                    for o in lib['owners'] if o.get('payload')}),
        'open_nodes': lib.get('open_nodes', []),
        'totals': {k: {'files': sum(1 for f in files.values() if f['kind'] == k),
                       'bytes': sum(f['bytes'] for f in files.values() if f['kind'] == k),
                       'stored_bytes': sum(f['stored_bytes'] for f in files.values() if f['kind'] == k)}
                   for k in sorted({f['kind'] for f in files.values()})},
        'files': files,
        # (5) The optional pre-filled recipe cache (WP2 `p-materialise-recipes`, later): layout
        # vakint.p-recipe-cache.v1 = the shipped p step-cache layout; null until one is staged next to the package.
        'recipe_cache': recipe_cache_record(package, release),
    }
    if selection:
        sel = json.load(open(selection))
        # Owners of a top: the classes whose owner presentation belongs to the top's DAG nodes is not recorded in
        # library.json; the selection names the Custom nodes per top and the import records name their labels.
        tops = {}
        node_files = node_payload_files(package, lib)
        names = {f['name'] for f in json.load(open(os.path.join(package, 'families', 'p-families.json')))['families']}
        for tp in sel['tops']:
            if tp['top'] not in names:
                continue
            need = sorted({node_files[t] for t in tp['custom'] if t in node_files})
            tops[tp['top']] = {'custom_nodes': tp['custom'], 'payload_files': need,
                               'bytes': sum(files[p]['stored_bytes'] for p in need if p in files)}
        manifest['tops'] = tops
        manifest['boxed_nodes'] = [{'node': t, 'statement': n.get('statement'), 'certified_keys_version': 1,
                                    'recipes': []}
                                   for t, n in sel['nodes'].items() if n['kind'] == 'boxed' and t in node_files]
    tmp = os.path.join(release, MANIFEST + '.tmp')
    json.dump(manifest, open(tmp, 'w'), indent=1, sort_keys=True)
    os.replace(tmp, os.path.join(release, MANIFEST))
    return manifest


P_RECIPE_CACHE_SCHEMA = 'vakint.p-recipe-cache.v1'
P_RECIPE_CACHE_VERSION = 1


def recipe_cache_record(package, release):
    """The optional recipe cache staged as <package>/../recipe-cache/ (recipe-cache.json + the step-cache directory):
    one gzip tar asset `recipe-cache.tar.gz`, or None."""
    root = os.path.join(os.path.dirname(os.path.abspath(package)), 'recipe-cache')
    meta = os.path.join(root, 'recipe-cache.json')
    if not os.path.exists(meta):
        return None
    m = json.load(open(meta))
    if m.get('schema') != P_RECIPE_CACHE_SCHEMA or m.get('version') != P_RECIPE_CACHE_VERSION:
        raise Refused(f'{meta}: schema {m.get("schema")} v{m.get("version")}')
    import tarfile
    dst = os.path.join(release, 'assets', 'recipe-cache.tar.gz')
    with tarfile.open(dst + '.tmp', 'w:gz') as tar:
        tar.add(root, arcname='recipe-cache')
    os.replace(dst + '.tmp', dst)
    return {'schema': P_RECIPE_CACHE_SCHEMA, 'version': P_RECIPE_CACHE_VERSION, 'identity': m.get('identity'),
            'asset': 'recipe-cache.tar.gz', 'stored_bytes': os.path.getsize(dst), 'sha256': sha256_file(dst),
            'optional': True}


def node_payload_files(package, lib):
    """T-id -> owners/<label>.rrbin via the presentations' DAG graph keys and node_map.tsv (when available)."""
    import csv
    nm = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/p5l-converter/node_map.tsv'
    out = {}
    if not os.path.exists(nm):
        return out
    gk = {r['graph_key']: r['id'] for r in csv.DictReader(open(nm), delimiter='\t')}
    pres = {p['index']: p for p in lib['presentations']}
    for o in lib['owners']:
        if o['lane'] != 'custom' or not o.get('payload'):
            continue
        node = (pres.get(o['presentation']) or {}).get('node')
        if node and node['key'] in gk:
            out[gk[node['key']]] = o['payload']['file']
    return out


def source_path(source):
    return source[len('file://'):] if source.startswith('file://') else source


def copy_verified(src, dst, expected_sha, expected_bytes, encoding, installed_bytes, content_sha):
    """Stream src into dst.part (resuming), check the stored sha256, then decode (gzip) or rename; the installed
    bytes must have content_sha (the source file's sha256) before they are renamed into place."""
    part = dst + '.part'
    os.makedirs(os.path.dirname(dst) or '.', exist_ok=True)
    h = hashlib.sha256()
    offset = os.path.getsize(part) if os.path.exists(part) else 0
    if offset > expected_bytes:
        os.remove(part)
        offset = 0
    if offset:
        with open(part, 'rb') as f:
            for block in iter(lambda: f.read(CHUNK), b''):
                h.update(block)
    with open(src, 'rb') as fi, open(part, 'ab') as fo:
        fi.seek(offset)
        for block in iter(lambda: fi.read(CHUNK), b''):
            h.update(block)
            fo.write(block)
    if os.path.getsize(part) != expected_bytes or h.hexdigest() != expected_sha:
        os.remove(part)
        raise Refused(f'{src}: stored bytes/sha256 differ from the manifest ({os.path.getsize(src)} B); refused')
    if encoding == 'gzip':
        tmp = dst + '.decoded'
        hc = hashlib.sha256()
        with gzip.open(part, 'rb') as fi, open(tmp, 'wb') as fo:
            for block in iter(lambda: fi.read(CHUNK), b''):
                hc.update(block)
                fo.write(block)
        if os.path.getsize(tmp) != installed_bytes or hc.hexdigest() != content_sha:
            os.remove(tmp)
            raise Refused(f'{src}: decoded {os.path.getsize(tmp)} B sha256 {hc.hexdigest()} != {installed_bytes} B '
                          f'{content_sha} (manifest content digest); refused')
        os.replace(tmp, dst)
        os.remove(part)
    else:
        if h.hexdigest() != content_sha or os.path.getsize(part) != installed_bytes:
            os.remove(part)
            raise Refused(f'{src}: verbatim asset sha256 {h.hexdigest()} != content digest {content_sha}; refused')
        os.replace(part, dst)


def fetch(source, dest, tops=None, optional=False, interrupt_after=None):
    """Install from a release directory. `interrupt_after` (tests): stop after that many bytes of one file."""
    src = source_path(source)
    manifest = json.load(open(os.path.join(src, MANIFEST)))
    if manifest.get('schema') != P_PACKAGE_RELEASE_SCHEMA or manifest.get('manifest_version') != P_PACKAGE_MANIFEST_VERSION:
        raise Refused(f"manifest schema {manifest.get('schema')} v{manifest.get('manifest_version')}: not "
                      f'{P_PACKAGE_RELEASE_SCHEMA} v{P_PACKAGE_MANIFEST_VERSION}')
    os.makedirs(dest, exist_ok=True)
    rec_path = os.path.join(dest, INSTALLED)
    record = json.load(open(rec_path)) if os.path.exists(rec_path) else {}
    if record and record.get('identity') != manifest['identity']:
        raise Refused(f"{dest} holds the package {record.get('identity')}, not {manifest['identity']}: use another "
                      'directory (an installed package is never overwritten by another one)')
    wanted = set()
    for path, f in manifest['files'].items():
        if f['kind'] == 'payload':
            continue
        if f['kind'] == 'optional' and not optional:
            continue
        wanted.add(path)
    if tops is None:
        wanted |= {p for p, f in manifest['files'].items() if f['kind'] == 'payload'}
    else:
        for t in tops:
            if t not in manifest.get('tops', {}):
                raise Refused(f'top {t} is not in the package')
            wanted |= set(manifest['tops'][t]['payload_files'])
    installed = record.get('files', {})
    stats = {'reused': 0, 'fetched': 0, 'bytes': 0}
    for path in sorted(wanted):
        f = manifest['files'][path]
        dst = os.path.join(dest, path)
        have = installed.get(path)
        if (have and have.get('sha256') == f['sha256'] and have.get('content_sha256') == f['content_sha256']
                and os.path.exists(dst) and os.path.getsize(dst) == f['bytes']):
            stats['reused'] += 1
            continue
        asset = os.path.join(src, 'assets', f['asset'])
        if interrupt_after is not None:
            # Simulated interruption: a partial transfer left behind.
            os.makedirs(os.path.dirname(dst) or '.', exist_ok=True)
            with open(asset, 'rb') as fi, open(dst + '.part', 'ab') as fo:
                fo.write(fi.read(interrupt_after))
            raise KeyboardInterrupt('simulated interruption')
        copy_verified(asset, dst, f['sha256'], f['stored_bytes'], f['encoding'], f['bytes'], f['content_sha256'])
        installed[path] = {'bytes': f['bytes'], 'sha256': f['sha256'], 'content_sha256': f['content_sha256'],
                           'utc': utc()}
        stats['fetched'] += 1
        stats['bytes'] += f['stored_bytes']
        # The record is rewritten after every file: an interrupted fetch keeps what it verified.
        write_record(rec_path, manifest, installed, tops)
    write_record(rec_path, manifest, installed, tops)
    lib_identity = json.load(open(os.path.join(dest, 'library.json'))).get('identity')
    if lib_identity != manifest['identity']:
        raise Refused(f'installed library.json identity {lib_identity} != manifest identity {manifest["identity"]}')
    shutil.copyfile(os.path.join(src, MANIFEST), os.path.join(dest, MANIFEST))
    return stats


def write_record(path, manifest, installed, tops):
    rec = {'schema': P_INSTALL_RECORD_SCHEMA, 'manifest_version': manifest['manifest_version'],
           'identity': manifest['identity'], 'name': manifest['name'], 'updated_utc': utc(),
           'tops': 'all' if tops is None else sorted(tops), 'files': installed}
    json.dump(rec, open(path + '.tmp', 'w'), indent=1, sort_keys=True)
    os.replace(path + '.tmp', path)


def check(dest, digest=False):
    rec = json.load(open(os.path.join(dest, INSTALLED)))
    bad = [p for p, f in rec['files'].items()
           if not os.path.exists(os.path.join(dest, p)) or os.path.getsize(os.path.join(dest, p)) != f['bytes']
           or (digest and sha256_file(os.path.join(dest, p)) != f.get('content_sha256'))]
    if digest and json.load(open(os.path.join(dest, 'library.json'))).get('identity') != rec['identity']:
        bad.append('library.json: identity differs from the install record')
    return rec, bad


def compare(dest, inventory):
    """Installed files vs a frozen source inventory (sha256 of the uncompressed source files)."""
    inv = json.load(open(inventory))['files']
    got = {p for p in walk(dest) if p not in (MANIFEST, INSTALLED)}
    missing = sorted(set(inv) - got)
    extra = sorted(got - set(inv))
    differ = sorted(p for p in set(inv) & got
                    if os.path.getsize(os.path.join(dest, p)) != inv[p]['bytes']
                    or sha256_file(os.path.join(dest, p)) != inv[p]['sha256'])
    return {'inventory_files': len(inv), 'installed_files': len(got), 'missing': missing, 'extra': extra,
            'differ': differ}


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    cmd = argv[0]
    if cmd == 'manifest':
        sel = argv[argv.index('--selection') + 1] if '--selection' in argv else None
        name = argv[argv.index('--name') + 1] if '--name' in argv else None
        try:
            m = write_manifest(argv[1], argv[2], sel, '--link' in argv, name, '--reuse-verified' in argv)
        except Refused as error:
            print(f'refused: {error}', file=sys.stderr)
            return 3
        print(json.dumps(m['totals'], indent=1))
        return 0
    if cmd == 'fetch':
        tops = None
        if '--tops' in argv:
            tops = argv[argv.index('--tops') + 1].split(',')
        try:
            stats = fetch(argv[1], argv[2], tops, '--optional' in argv)
        except Refused as error:
            print(f'refused: {error}', file=sys.stderr)
            return 3
        print(json.dumps(stats))
        return 0
    if cmd == 'check':
        rec, bad = check(argv[1], '--digest' in argv)
        print(json.dumps({'files': len(rec['files']), 'bad': bad}))
        return 0 if not bad else 3
    if cmd == 'compare':
        r = compare(argv[1], argv[2])
        print(json.dumps(r))
        return 0 if not (r['missing'] or r['extra'] or r['differ']) else 3
    print(__doc__)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
