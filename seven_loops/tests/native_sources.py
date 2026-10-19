"""Validate all prepared K28 inputs and their 49 ordinary source identities.

This derives ordinary identities only. It does not solve any seven-loop sector
or run the parametric generation campaign.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    topology = ROOT/'seven_loops/topology'
    manifest = json.loads((topology/'manifest.json').read_text())
    records = []
    started = time.monotonic()
    for entry in manifest['entries']:
        destination = output/(entry['id']+'.toml')
        before = time.monotonic()
        subprocess.run([str(binary), 'derive', '--input', str(topology/entry['input']),
                        '--relations', 'ordinary', '--n-cores', '1', '--output', str(destination)], check=True)
        result = tomllib.loads(destination.read_text())
        if (result['status'] != 'ok' or result['family']['denominator_count'] != 28
                or len(result['family']['loop_momenta']) != 7 or len(result['relations']) != 49):
            raise ValueError('native source family/count disagrees with the input manifest')
        records.append(dict(parent=entry['id'], ordinary_sources=len(result['relations']),
                            wall_seconds=time.monotonic()-before))
    report = dict(status='passed', parents=len(records), ordinary_sources=sum(r['ordinary_sources'] for r in records),
        wall_seconds=time.monotonic()-started, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        rows=records, parametric_ibp_generation_performed=False, closure_claim=False)
    (output/'result.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({key: value for key, value in report.items() if key != 'rows'}, indent=2))


if __name__ == '__main__':
    main()
