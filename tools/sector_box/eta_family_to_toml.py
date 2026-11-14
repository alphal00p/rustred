#!/usr/bin/env python3
"""Convert an affine vacuum family in the JSON layout of the sector-box campaign driver into a
`rustred.project.toml.v1` file for `rustred sector-box-reduce`.

Input JSON: `name`, `loop_momenta` [k_1..k_L], `denominators` [{`coefficients`: [c_ij as strings, over the loop-loop
scalar products k_i.k_j, i <= j, row-major upper triangle], `constant`: "<rational>" or "<rational>-<param>" or
"<rational>+<param>"}], optional `power_shifts` (must be all zero), optional `parameters` (default ["d", ...the
parameters named in the constants]).

Each denominator becomes the expanded quadratic form `sum c_ij k_i.k_j + constant`, written with products of momenta
(`k1*k3` is the scalar product). Usage: eta_family_to_toml.py FAMILY.json > family.toml   (run with python3 -I)
"""
import json
import re
import sys
from fractions import Fraction


def term(coefficient, monomial):
    if coefficient == 0:
        return ''
    sign = '-' if coefficient < 0 else '+'
    magnitude = abs(coefficient)
    if monomial == '':
        return f'{sign}{magnitude}'
    return f'{sign}{monomial}' if magnitude == 1 else f'{sign}{magnitude}*{monomial}'


def main():
    data = json.load(open(sys.argv[1]))
    loops = data['loop_momenta']
    pairs = [(i, j) for i in range(len(loops)) for j in range(i, len(loops))]
    if any(Fraction(x) != 0 for x in data.get('power_shifts', [])):
        sys.exit('power shifts are not supported by this converter')
    parameters = list(data.get('parameters', []))
    if not parameters:
        parameters = ['d']
    lines = ['schema = "rustred.project.toml.v1"', f'parameters = {json.dumps(parameters)}', '',
             '[family]', f'name = {json.dumps(data["name"])}', f'loop_momenta = {json.dumps(loops)}',
             'external_momenta = []', 'dimension = "d"']
    for index, denominator in enumerate(data['denominators']):
        coefficients = denominator['coefficients']
        if len(coefficients) != len(pairs):
            sys.exit(f'denominator {index}: {len(coefficients)} coefficients, expected {len(pairs)}')
        expression = ''
        for (i, j), value in zip(pairs, coefficients):
            monomial = f'{loops[i]}^2' if i == j else f'{loops[i]}*{loops[j]}'
            expression += term(Fraction(value), monomial)
        match = re.fullmatch(r'\s*([-+]?\d+(?:/\d+)?)\s*(?:([-+])\s*([A-Za-z_]\w*))?\s*', denominator['constant'])
        if not match:
            sys.exit(f'denominator {index}: unsupported constant {denominator["constant"]!r}')
        expression += term(Fraction(match[1]), '')
        if match[3]:
            if match[3] not in parameters:
                sys.exit(f'denominator {index}: parameter {match[3]} is not in {parameters}')
            expression += f'{match[2]}{match[3]}'
        expression = expression.lstrip('+') or '0'
        lines += ['', '[[family.denominators]]', f'id = "D{index + 1}"', f'expression = "{expression}"']
    print('\n'.join(lines))


main()
