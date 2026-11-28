#!/usr/bin/env python3
"""Reproducible Matrix capacity research; production bounds and weights stay unchanged.

Example:
  python3 .maintain/benchmark-matrix-capacity.py /tmp/matrix-capacity \
    --node /path/to/pinned-sdk/hypertensor-node --prepare
  python3 .maintain/benchmark-matrix-capacity.py /tmp/matrix-capacity \
    --node /path/to/pinned-sdk/hypertensor-node --sizes 32,64,128,256,384,512

The isolated runtime lifts Matrix's bound to 512. It uses optimized compiled Wasm,
proof recording and verification. Equivalent parameter pairs at each size avoid fitting the
quadratic settlement path to one global linear model. Partial weight files are
research artifacts only and must not be installed as runtime weights. Finish with
--restore to rebuild the ordinary runtime in the shared target directory.
"""
import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
METHODS = ','.join('capacity_matrix_' + x for x in (
    'open', 'commit', 'reveal', 'commit_loaded', 'reveal_loaded', 'slot', 'failed_credits', 'promotions', 'sparse_stake', 'no_scores'))


def command(args, directory, name, env=None):
    with (directory / 'commands.jsonl').open('a') as f:
        f.write(json.dumps([str(a) for a in args]) + '\n')
    with (directory / name).open('w') as log:
        subprocess.run(args, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--restore', action='store_true')
    parser.add_argument('--sizes', default='32,64,128,256,384,512')
    parser.add_argument('--repeat', type=int, default=20)
    parser.add_argument('--methods', default=METHODS)
    args = parser.parse_args()
    out = args.directory.resolve()
    out.mkdir(parents=True, exist_ok=True)
    node = args.node.resolve()
    if args.restore:
        env = os.environ.copy()
        env.pop('SKIP_WASM_BUILD', None)
        env.pop('WASM_TARGET_DIRECTORY', None)
        env.update(CARGO_TARGET_DIR=str(ROOT / 'target'), WASM_BUILD_TYPE='release',
                   SKIP_FRAME_STORAGE_ACCESS_TEST_RUNTIME_WASM_BUILD='1',
                   SKIP_PALLET_REVIVE_FIXTURES='1')
        command(['cargo', 'build', '--locked', '--offline', '-p', 'hypertensor-runtime',
                 '--features', 'runtime-benchmarks'], out, 'restore-build.log', env)
        artifact = ROOT / 'target/debug/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm'
        (out / 'restored-runtime.sha256').write_text(hashlib.sha256(artifact.read_bytes()).hexdigest() + '\n')
        return
    if args.prepare:
        source = out / 'source'
        source.mkdir(exist_ok=False)
        names = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=ROOT).decode().split('\0')
        hashes = {}
        for name in sorted(set(names) - {''}):
            original = ROOT / name
            if not original.is_file():
                continue
            dest = source / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original, dest)
            hashes[name] = hashlib.sha256(original.read_bytes()).hexdigest()
        types = source / 'pallets/network/src/consensus/matrix/types.rs'
        old = 'pub const MATRIX_MAX_NODES: u32 = 64;'
        text = types.read_text()
        assert text.count(old) == 1
        types.write_text(text.replace(old, old.replace('64', '512')))
        (out / 'manifest.json').write_text(json.dumps({
            'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
            'source_sha256': hashes, 'research_matrix_bound': 512,
            'production_matrix_bound': 64, 'production_weights_regenerated': False,
            'research_overrides': {'pallets/network/src/consensus/matrix/types.rs': {
                'original_sha256': hashes['pallets/network/src/consensus/matrix/types.rs'],
                'research_sha256': hashlib.sha256(types.read_bytes()).hexdigest(),
                'replacement': 'MATRIX_MAX_NODES: 64 -> 512'}},
            'host': platform.platform(), 'node': str(node),
            'node_sha256': hashlib.sha256(node.read_bytes()).hexdigest(),
            'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
            'wasm_build_type': 'release',
        }, indent=2) + '\n')
        command(['lscpu'], out, 'cpu.txt')
        command([node, '--version'], out, 'node-version.txt')
        env = os.environ.copy()
        env.pop('SKIP_WASM_BUILD', None)
        env.update(CARGO_TARGET_DIR=str(ROOT / 'target'), WASM_BUILD_TYPE='release',
                   WASM_TARGET_DIRECTORY=str(out),
                   SKIP_FRAME_STORAGE_ACCESS_TEST_RUNTIME_WASM_BUILD='1',
                   SKIP_PALLET_REVIVE_FIXTURES='1')
        # Host optimization is irrelevant; the measured Wasm is always a release build.
        command(['cargo', 'build', '--locked', '--offline', '--manifest-path',
                 str(source / 'runtime/Cargo.toml'), '--features', 'runtime-benchmarks'], out, 'build.log', env)
        runtime = ROOT / 'target/debug/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm'
        shutil.copy2(runtime, out / 'runtime.wasm')
        (out / 'runtime.sha256').write_text(hashlib.sha256((out / 'runtime.wasm').read_bytes()).hexdigest() + '\n')
        return
    for size in map(int, args.sizes.split(',')):
        assert 4 <= size <= 512
        directory = out / f'n-{size}'
        directory.mkdir(exist_ok=True)
        command([node, 'benchmark', 'pallet', '--runtime', out / 'runtime.wasm',
                 '--genesis-builder', 'runtime', '--pallets', 'pallet_network',
                 '--extrinsic', args.methods, '--extra', '--low', str(2 * size), '--high', str(2 * size + 1),
                 '--steps', '2', '--repeat', str(args.repeat), '--min-duration', '0',
                 '--wasm-execution', 'compiled', '--output-analysis', 'max', '--output-pov-analysis', 'max',
                 '--template', ROOT / '.maintain/frame-weight-template.hbs',
                 '--output', directory / 'weights.rs', '--json-file', directory / 'results.json'], directory, 'run.log')
        print(f'Completed {size} nodes', flush=True)


if __name__ == '__main__':
    main()
