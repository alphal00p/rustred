"""Opt-in route geometry plumbing; no native algebra or campaign launch."""
import argparse
import copy
import importlib.util
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


def sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


MATCH = sibling('match_shared_owner_domains')
PRODUCTION = sibling('production_saved_owner_campaign')
FLAG = '--' + MATCH.PRECISE_IMAGE_BOUNDS


class PreciseRouteSteeringTests(unittest.TestCase):
    def test_native_match_flag_is_explicit_unique_and_requires_routing(self):
        base = ['match', '--executable', 'native', '--manifest', 'm', '--queries', 'q', '--output', 'o']
        route = ['--follow-successors', '--route-domain-overcover']
        for suffix in (route, [*route, FLAG]):
            with patch('sys.argv', base + suffix), patch.object(MATCH.os, 'execve') as execute:
                MATCH.main()
            self.assertEqual(execute.call_args.args[1].count(FLAG), int(FLAG in suffix))
        for suffix in ([FLAG], ['--follow-successors', FLAG], [*route, FLAG, FLAG]):
            with patch('sys.argv', base + suffix), patch.object(MATCH.os, 'execve') as execute, \
                    patch('sys.stderr', new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def policy(self, campaign, enabled):
        args = argparse.Namespace(workers=1, cpus=str(min(os.sched_getaffinity(0))),
                                  route_precise_image_bounds=enabled, resume=False,
                                  auto_rescue=False)
        return args, PRODUCTION.frozen_policy(campaign, args, campaign / 'bin/rustred',
                                              campaign / 'inputs', 2, 1024)

    def test_geometry_is_frozen_and_legacy_policy_is_default_off(self):
        workspace_tmp = Path(__file__).resolve().parents[2] / 'TMP'
        for enabled in (False, True):
            with self.subTest(enabled=enabled), tempfile.TemporaryDirectory(dir=workspace_tmp) as tmp:
                campaign = Path(tmp)
                (campaign / 'bin').mkdir()
                args, policy = self.policy(campaign, enabled)
                self.assertEqual(policy['command_arguments'].count(FLAG), int(enabled))
                if not enabled:
                    del policy['options']['route_precise_image_bounds']
                    (campaign / 'bin/steering.json').chmod(0o600)
                    PRODUCTION.write_json(campaign / 'bin/steering.json', policy)
                before = (campaign / 'bin/steering.json').read_bytes()
                self.assertEqual(PRODUCTION.frozen_options(policy)['route_precise_image_bounds'], enabled)
                args.resume = True
                args.route_precise_image_bounds = None
                self.assertEqual(PRODUCTION.frozen_policy(campaign, args, campaign, campaign, 9, 99), policy)
                args.route_precise_image_bounds = not enabled
                with self.assertRaisesRegex(ValueError, 'differs from frozen policy'):
                    PRODUCTION.frozen_policy(campaign, args, campaign, campaign, 9, 99)
                self.assertEqual((campaign / 'bin/steering.json').read_bytes(), before)

    def test_policy_and_command_disagreement_is_not_silently_repaired(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[2] / 'TMP') as tmp:
            campaign = Path(tmp)
            (campaign / 'bin').mkdir()
            _, policy = self.policy(campaign, True)
            for kind in ('missing', 'duplicate', 'not-boolean', 'equal-spelling'):
                bad = copy.deepcopy(policy)
                if kind == 'missing':
                    bad['command_arguments'].remove(FLAG)
                elif kind == 'duplicate':
                    bad['command_arguments'].append(FLAG)
                elif kind == 'not-boolean':
                    bad['options']['route_precise_image_bounds'] = 1
                else:
                    bad['command_arguments'].remove(FLAG)
                    bad['command_arguments'].append(FLAG + '=true')
                with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, 'precise route-image'):
                    PRODUCTION.frozen_options(bad)


if __name__ == '__main__':
    unittest.main()
