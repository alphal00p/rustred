#!/usr/bin/env python3
"""Tests of the exactness binding of fc_tool.py (lane parametric, 2026-11-18, after TRICKS 67).
Run: python3 -I tools/final_cert/test_fc_tool.py   (exit 0 = all pass)."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fc_tool as t  # noqa: E402

CONE = 39


def test_vacuous_or_dead_never_whole():
    # the TRICKS-67 case: a face verdict whose exactness item is vacuous (the record is dead in the cone)
    assert t.classify_evidence('FACE-VERDICT', 'PASS (1-3 whole domain; 4-5 vacuous: dead at D+r<=39)', CONE) == ('vacuous', None)
    assert t.classify_evidence('FACE-VERDICT', 'x || proof object t162-faces.part2 Lf04c0: VACUOUS: 0 first-match keys with D + r <= 39', CONE)[0] == 'vacuous'
    assert t.classify_evidence('NOT-RUN-DEAD', '', CONE)[0] == 'vacuous'
    assert t.classify_evidence('NOT-RUN(out of scope: dead)', '', CONE)[0] == 'vacuous'
    # even an E-WHOLE class word cannot outweigh a vacuous statement on the same row
    assert t.classify_evidence('E-WHOLE', 'vacuous: dead at D+r<=39', CONE)[0] == 'vacuous'


def test_whole_only_from_a_whole_domain_proof_object():
    assert t.classify_evidence('E-WHOLE', '', CONE)[0] == 'whole'
    assert t.classify_evidence('POINT', '', CONE)[0] == 'whole'
    assert t.classify_evidence('SYM+POINT', '', CONE)[0] == 'whole'
    # a summary word alone yields nothing; its proof object's exactness item decides
    assert t.classify_evidence('FACE-VERDICT', 'PASS', CONE)[0] == 'none'
    assert t.classify_evidence('FACE-VERDICT', 'PASS || proof object t162-faces.part1 f01d: PROVED whole domain (E-WHOLE: 787 IBP rows)', CONE)[0] == 'whole'
    # "1-3 whole domain" (domain / descent / validity) is NOT a whole-domain exactness proof
    assert t.classify_evidence('FACE-VERDICT', 'PASS (1-3 whole domain; 4-5 at every FM key D+r<=39)', CONE) == ('every', None)
    assert t.classify_evidence('FACE-VERDICT', 'PASS || proof object t162-faces.part2 Lf38d0: FULL: EXACT_IN_SPAN_AT_KEY 666/666 FM keys', CONE) == ('every', None)
    assert t.classify_evidence('FACE-VERDICT', 'PASS (1-3 whole domain; 4-5 at every FM key D+r<=33)', CONE)[0] != 'every'


def test_per_key_sampled_and_face_restricted_never_whole():
    assert t.classify_evidence('E-KEYS', 'EXACT at 494 keys: ALL first-match keys with D + r <= 17 + 88 stratified-sample keys above', CONE) == ('sampled', 494)
    assert t.classify_evidence('E-KEYS', 'EXACT at ALL 741 first-match keys of D + r <= 39 (stored certificate per key)', CONE) == ('every', None)
    assert t.classify_evidence('E-KEYS', 'EXACT at ALL 741 first-match keys of D + r <= 33', CONE)[0] == 'sampled' or \
        t.classify_evidence('E-KEYS', 'EXACT at ALL 741 first-match keys of D + r <= 33', CONE)[0] == 'none'
    assert t.classify_evidence('sampled 2041 keys', '', CONE) == ('sampled', 2041)
    assert t.classify_evidence('every first-match key in cone', '', CONE) == ('every', None)
    assert t.classify_evidence('OPEN', 'prover: CRASH', CONE)[0] == 'none'
    assert t.classify_evidence('SKIPPED', '', CONE)[0] == 'none'


def _rec(dp, h, dom):
    return {'file': 0, 'table_index': 7, 'dp': dp, 'hashes': {'list': h}, 'domain': dom}


def _row(index, cls, detail='', h=None, dom=None, keyed='reference'):
    kind, n = t.classify_evidence(cls, detail, CONE)
    return {'source': 'S', 'keyed_by': keyed, 'file': 0, 'index': index, 'hash': h, 'form': 'list', 'domain': dom,
            'class': cls, 'detail': detail, 'kind': kind, 'n': n}


def test_a_twin_transfers_only_a_whole_domain_proof():
    ref = {2926: {'list': 'H', 'domain': 'D'}, 3592: {'list': 'H', 'domain': 'D'}, 3600: {'list': 'H', 'domain': 'D'}}
    own = [_row(2926, 'E-KEYS', 'EXACT at 494 keys: ALL first-match keys with D + r <= 17 + 88 stratified-sample keys above')]
    twin_vacuous = _row(3592, 'FACE-VERDICT', 'PASS (1-3 whole domain; 4-5 vacuous: dead at D+r<=39)')
    twin_every = _row(3601, 'FACE-VERDICT', 'PASS (1-3 whole domain; 4-5 at every FM key D+r<=39)', h='H', dom='D')
    bound, kind, n, best, own_vac = t.bind_record(_rec(2926, 'H', 'D'), own, [twin_vacuous, twin_every], ref)
    assert (kind, n) == ('sampled', 494), (kind, n)            # the record's OWN evidence decides; the TRICKS-67 case
    assert not own_vac
    assert all(b['status'].startswith('twin: IGNORED') for b in bound if b['index'] != 2926)
    twin_whole = _row(3600, 'E-WHOLE')
    bound, kind, n, best, _ = t.bind_record(_rec(2926, 'H', 'D'), own, [twin_whole], ref)
    assert kind == 'whole' and best['index'] == 3600          # a whole-domain proof object of the same relation + domain transfers


def test_binding_requires_target_rhs_and_domain_equality():
    ref = {10: {'list': 'H', 'domain': 'D_old'}}
    own = [_row(10, 'E-WHOLE')]
    bound, kind, _, _, _ = t.bind_record(_rec(10, 'H', 'D_new'), own, [], ref)
    assert kind == 'none' and bound[0]['status'].startswith('stale')
    bound, kind, _, _, _ = t.bind_record(_rec(10, 'H2', 'D_old'), own, [], ref)
    assert kind == 'none' and bound[0]['status'].startswith('stale')
    # a twin with a different domain does not bind at all, even a whole-domain proof
    bound, kind, _, _, _ = t.bind_record(_rec(11, 'H', 'D_new'), [], [_row(10, 'E-WHOLE')], ref)
    assert kind == 'none' and bound == []
    # candidate-keyed rows carry their own hash and domain
    crow = _row(7, 'every first-match key in cone', h='H', dom='D', keyed='candidate')
    bound, kind, _, _, _ = t.bind_record(_rec(None, 'H', 'D'), [crow], [], {})
    assert kind == 'every' and bound[0]['status'] == 'own'


def test_a_live_rule_with_a_vacuous_own_verdict_is_a_contradiction():
    ref = {5: {'list': 'H', 'domain': 'D'}}
    _, kind, _, _, own_vac = t.bind_record(_rec(5, 'H', 'D'), [_row(5, 'NOT-RUN-DEAD')], [], ref)
    assert kind == 'none' and own_vac
    assert t.decide_class(kind, None, 'live', own_vac)[2] is True        # live + own vacuous: the step must fail
    assert t.decide_class(kind, None, 'dead', own_vac) == ('dead in cone', 'dead in cone', False)
    assert t.decide_class('whole', None, 'live', False)[:2] == ('whole domain', 'whole domain')
    assert t.decide_class('sampled', 494, 'live', False)[1] == 'sampled 494 keys'


if __name__ == '__main__':
    n = 0
    for name, f in sorted(globals().items()):
        if name.startswith('test_') and callable(f):
            f(); n += 1
            print('ok', name)
    print('%d tests passed' % n)
