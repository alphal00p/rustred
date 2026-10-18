"""Small factorized K21 generation/checkpoint smoke; never runs study parents."""
import argparse
import hashlib
import json
import os
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
    environment = os.environ.copy()
    for key in ('RAYON_NUM_THREADS','OMP_NUM_THREADS','OMP_THREAD_LIMIT',
                'OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','BLIS_NUM_THREADS','SYMBOLICA_HIDE_BANNER'):
        environment[key] = '1'
    command = [str(binary), 'family-candidates', '--input', str(ROOT/'six_loop_study/tests/six_tadpoles.toml'),
        '--input-format', 'toml', '--nonpositive-indices', ','.join(map(str, range(6,21))),
        '--root-zero-census', '--n-cores', '1', '--numerical-depth', '0', '--max-numerator-rank', '0',
        '--exact-backend', 'sparse-factorized', '--checkpoint-dir', str(output/'sectors'),
        '--checkpoint-max-bytes', str(2**64-1), '--bundle-max-entries', str(2**64-1),
        '--bundle-max-bytes', str(4*1024**3), '--bundle-max-polynomial-terms', str(2**64-1),
        '--bundle-max-term-operations', str(2**64-1), '--progress']
    receipts = []
    for label, extra in [('create', []), ('resume', ['--resume'])]:
        invocation = command + ['--output', str(output/f'{label}.rrbin'),
            '--report-output', str(output/f'{label}.toml')] + extra
        started = time.monotonic()
        with (output/f'{label}.stdout').open('w') as out, (output/f'{label}.stderr').open('w') as err:
            result = subprocess.run(invocation, env=environment, stdout=out, stderr=err)
        if result.returncode:
            raise RuntimeError(f'{label} failed ({result.returncode}); see {output}/{label}.stderr')
        report = tomllib.loads((output/f'{label}.toml').read_text())
        assert report['arity'] == 21 and report['solved_sectors'] == 1
        assert report['max_numerator_rank'] == 0 and report['finite_residuals'] == 1
        receipts.append({'phase':label,'seconds':time.monotonic()-started,'report':report,'command':invocation})
    first = hashlib.sha256((output/'create.rrbin').read_bytes()).hexdigest()
    assert first == hashlib.sha256((output/'resume.rrbin').read_bytes()).hexdigest()
    inspect = [str(binary),'candidate-inspect','--input',str(output/'resume.rrbin'),
        '--options',str(ROOT/'six_loop_study/tests/inspect_summary.json'),'--output',str(output/'inspection.json')]
    subprocess.run(inspect, env=environment, check=True)
    inspection = json.loads((output/'inspection.json').read_text())
    assert inspection['arity'] == 21 and inspection['total_terminals'] == 1
    (output/'result.json').write_text(json.dumps({'status':'passed','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
        'candidate_sha256':first,'byte_identical_resume':True,'receipts':receipts},indent=2)+'\n')
    print(json.dumps({'status':'passed','rules':inspection['total_rules'],'phases':[{k:r[k] for k in ('phase','seconds')} for r in receipts]}))

if __name__ == '__main__':
    main()
