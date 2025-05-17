#!/usr/bin/env python3
"""
Generates PetraVM opcode benchmarks JSON.
Installs cargo-criterion if needed, then writes output into docs/benches/.
Bench directory will contain benchmarks.json and index.html (sourced from repo).
"""
import subprocess
import os
import json

INDEX_SRC = 'index.html'


def install_cargo_criterion():
    """
    Ensures cargo-criterion is installed. Installs via `cargo install` if missing.
    """
    try:
        subprocess.run([
            'cargo', 'criterion', '--version'
        ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
        print('✅ cargo-criterion is already installed')
    except subprocess.CalledProcessError:
        print('📦 Installing cargo-criterion...')
        subprocess.run([
            'cargo', 'install', '--force', 'cargo-criterion'
        ], check=True)
        print('✅ Installed cargo-criterion')


def main():
    # Determine project root relative to this script
    script_dir = os.path.abspath(os.path.dirname(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, '..'))

    # Prepare output directory
    out_dir = os.path.join(project_root, 'docs', 'benches')
    os.makedirs(out_dir, exist_ok=True)

    # Step 1: Ensure cargo-criterion
    install_cargo_criterion()

    # Step 2: Run benchmarks and collect JSON lines
    print('🛠 Running benchmarks via cargo-criterion (JSON lines)...')
    raw = subprocess.check_output([
        'cargo', 'criterion',
        '--package', 'petravm-prover',
        '--bench', 'opcodes',
        '--message-format=json'
    ], cwd=project_root)

    # Step 3: Parse lines into a JSON array
    lines = raw.decode('utf-8').splitlines()
    objs = [json.loads(line) for line in lines if line.strip()]
    bench_json_path = os.path.join(out_dir, 'benchmarks.json')
    with open(bench_json_path, 'w') as f:
        json.dump(objs, f, indent=2)
    print(f'✅ Wrote JSON array to {bench_json_path}')

    # Step 4: Ensure dashboard HTML exists in docs/benches
    # We assume the user places index.html directly in docs/benches/
    user_html = os.path.join(out_dir, 'index.html')
    if os.path.exists(user_html):
        print(f'ℹ️ Found existing dashboard HTML at {user_html}, skipping copy.')
    else:
        print(f'⚠️ No index.html found in {out_dir}. Please place your dashboard HTML there.')

    print("All done! Commit & push docs/benches; view at https://petraprover.github.io/PetraVM/benches/")


if __name__ == '__main__':
    main()
