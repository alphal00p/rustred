#!/usr/bin/env python3
"""test_release.py: the manifest writer and the local fetch-from-directory path of release.py on a small fake
package (no engine needed): full fetch, offline re-fetch, an interrupted transfer resumed, a corrupt asset refused,
a partial (per top) fetch, another package refused in the same directory, the size check."""
import json
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import release  # noqa: E402


def fake_package(root):
    os.makedirs(os.path.join(root, 'owners'))
    os.makedirs(os.path.join(root, 'families'))
    os.makedirs(os.path.join(root, 'topology'))
    lib = {'schema': 'rustred.p-library.v1', 'identity': 'scope=1-5,X=1,P_PACKAGE_REVISION=wp6-test',
           'scope': [1, 5], 'versions': {'X': '1'},
           'presentations': [{'index': 0, 'node': {'index': 0, 'key': 'aa'}}, {'index': 1, 'node': {'index': 1, 'key': 'bb'}}],
           'owners': [{'class': 'L5T10-0001', 'lane': 'custom', 'presentation': 0,
                       'payload': {'file': 'owners/L5T10-0001.rrbin', 'bytes': 300000, 'encoding': 'gzip'}},
                      {'class': 'L5T11-0002', 'lane': 'custom', 'presentation': 1,
                       'payload': {'file': 'owners/L5T11-0002.rrbin', 'bytes': 1000, 'encoding': 'gzip'}}],
           'open_nodes': [{'version': 1, 'class': 'L5T12-0003', 'node': 'T266', 'status': 'stuck: not closed'}]}
    json.dump(lib, open(os.path.join(root, 'library.json'), 'w'))
    open(os.path.join(root, 'owners', 'L5T10-0001.rrbin'), 'wb').write(os.urandom(300000))
    open(os.path.join(root, 'owners', 'L5T11-0002.rrbin'), 'wb').write(os.urandom(1000))
    json.dump({'families': [{'name': 'PA_e1', 'loops': 5}, {'name': 'PB_e1', 'loops': 5}]},
              open(os.path.join(root, 'families', 'p-families.json'), 'w'))
    open(os.path.join(root, 'topology', 'p-topologies.json'), 'w').write('{}' * 1000)
    open(os.path.join(root, 'census.rrbin'), 'wb').write(b'\0' * 50000)
    open(os.path.join(root, 'census.json'), 'w').write('[' + '0,' * 50000 + '0]')
    sel = {'tops': [{'top': 'PA_e1', 'custom': ['T001']}, {'top': 'PB_e1', 'custom': ['T001', 'T002']}],
           'nodes': {'T001': {'kind': 'min'}, 'T002': {'kind': 'min'}}}
    return lib, sel


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix='wp6-release-')
        self.pkg = os.path.join(self.tmp, 'pkg')
        self.rel = os.path.join(self.tmp, 'release')
        self.lib, sel = fake_package(self.pkg)
        self.sel = os.path.join(self.tmp, 'selection.json')
        json.dump(sel, open(self.sel, 'w'))
        # node_map: T001 -> aa, T002 -> bb
        self.nm = os.path.join(self.tmp, 'node_map.tsv')
        open(self.nm, 'w').write('id\tnode\tgraph_key\nT001\t0\taa\nT002\t1\tbb\n')
        release_node_map = release.node_payload_files

        def patched(package, lib):
            import csv
            gk = {r['graph_key']: r['id'] for r in csv.DictReader(open(self.nm), delimiter='\t')}
            pres = {p['index']: p for p in lib['presentations']}
            return {gk[pres[o['presentation']]['node']['key']]: o['payload']['file'] for o in lib['owners']}
        release.node_payload_files = patched
        self.addCleanup(lambda: setattr(release, 'node_payload_files', release_node_map))
        self.manifest = release.write_manifest(self.pkg, self.rel, self.sel)

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def test_manifest(self):
        m = self.manifest
        self.assertEqual(m['schema'], release.P_PACKAGE_RELEASE_SCHEMA)
        self.assertEqual(m['revision'], 'wp6-test')
        self.assertEqual(m['files']['census.json']['kind'], 'optional')
        self.assertEqual(m['files']['owners/L5T10-0001.rrbin']['encoding'], None)
        self.assertEqual(m['files']['library.json']['encoding'], 'gzip')
        self.assertEqual(m['tops']['PA_e1']['payload_files'], ['owners/L5T10-0001.rrbin'])
        self.assertEqual(len(m['tops']['PB_e1']['payload_files']), 2)
        self.assertEqual(m['open_nodes'][0]['node'], 'T266')

    def test_full_fetch_then_offline(self):
        dest = os.path.join(self.tmp, 'inst')
        s = release.fetch(self.rel, dest)
        self.assertEqual((s['fetched'], s['reused']), (6, 0))
        for p in ('library.json', 'owners/L5T10-0001.rrbin', 'census.rrbin'):
            self.assertEqual(open(os.path.join(dest, p), 'rb').read(), open(os.path.join(self.pkg, p), 'rb').read())
        self.assertFalse(os.path.exists(os.path.join(dest, 'census.json')))
        # Offline: the assets are gone, the install record serves.
        shutil.rmtree(os.path.join(self.rel, 'assets'))
        s = release.fetch(self.rel, dest)
        self.assertEqual((s['fetched'], s['reused']), (0, 6))
        rec, bad = release.check(dest)
        self.assertEqual(bad, [])

    def test_interrupted_transfer_resumes(self):
        dest = os.path.join(self.tmp, 'inst')
        with self.assertRaises(KeyboardInterrupt):
            release.fetch(self.rel, dest, interrupt_after=1000)
        parts = [p for p in release.walk(dest) if p.endswith('.part')]
        self.assertEqual(len(parts), 1)
        s = release.fetch(self.rel, dest, optional=True)
        self.assertEqual(s['fetched'], 7)
        self.assertEqual([p for p in release.walk(dest) if p.endswith('.part')], [])
        self.assertEqual(open(os.path.join(dest, 'census.json')).read(), open(os.path.join(self.pkg, 'census.json')).read())

    def test_corrupt_asset_is_refused(self):
        dest = os.path.join(self.tmp, 'inst')
        a = os.path.join(self.rel, 'assets', self.manifest['files']['owners/L5T10-0001.rrbin']['asset'])
        data = bytearray(open(a, 'rb').read())
        data[100] ^= 1
        open(a, 'wb').write(bytes(data))
        with self.assertRaises(release.Refused):
            release.fetch(self.rel, dest)
        self.assertFalse(os.path.exists(os.path.join(dest, 'owners', 'L5T10-0001.rrbin')))
        self.assertFalse(os.path.exists(os.path.join(dest, 'owners', 'L5T10-0001.rrbin.part')))

    def test_partial_fetch_by_top(self):
        dest = os.path.join(self.tmp, 'inst')
        s = release.fetch(self.rel, dest, tops=['PA_e1'])
        self.assertTrue(os.path.exists(os.path.join(dest, 'owners', 'L5T10-0001.rrbin')))
        self.assertFalse(os.path.exists(os.path.join(dest, 'owners', 'L5T11-0002.rrbin')))
        s = release.fetch(self.rel, dest, tops=['PB_e1'])
        self.assertEqual(s['fetched'], 1)
        with self.assertRaises(release.Refused):
            release.fetch(self.rel, dest, tops=['PZ_e1'])

    def test_another_package_is_refused_in_place(self):
        dest = os.path.join(self.tmp, 'inst')
        release.fetch(self.rel, dest)
        m = json.load(open(os.path.join(self.rel, release.MANIFEST)))
        m['identity'] = 'scope=1-5,X=2'
        json.dump(m, open(os.path.join(self.rel, release.MANIFEST), 'w'))
        with self.assertRaises(release.Refused):
            release.fetch(self.rel, dest)

    def test_size_check_finds_a_truncated_file(self):
        dest = os.path.join(self.tmp, 'inst')
        release.fetch(self.rel, dest)
        p = os.path.join(dest, 'owners', 'L5T11-0002.rrbin')
        open(p, 'wb').write(b'x')
        rec, bad = release.check(dest)
        self.assertEqual(bad, ['owners/L5T11-0002.rrbin'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
