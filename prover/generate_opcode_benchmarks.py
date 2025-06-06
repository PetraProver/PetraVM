#!/usr/bin/env python3
"""
Generates PetraVM opcode benchmarks JSON.
Installs cargo-criterion if needed, then writes output into docs/benches/.
Bench directory will contain benchmarks.json and index.html (sourced from repo).
"""
import subprocess
import os
import json
import argparse
from pathlib import Path

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

def extract_criterion_results(criterion_dir: Path):
    results = []
    for benchmark_path in criterion_dir.glob('**/new/estimates.json'):
        with open(benchmark_path) as f:
            data = json.load(f)

        result = {
            'benchmark': benchmark_path.parent.parent.name,  # e.g. 'generate'
            'mean_ns': data['mean']['point_estimate'],
            'ci_lower': data['mean']['confidence_interval']['lower_bound'],
            'ci_upper': data['mean']['confidence_interval']['upper_bound'],
        }
        results.append(result)

    return results

def main():
    # Set up argument parser
    parser = argparse.ArgumentParser(description='Generates PetraVM opcode benchmarks JSON.')
    parser.add_argument('--features', type=str, help='Optional features to pass to cargo criterion.')
    parser.add_argument('--generation', action='store_true', help='Run opcode_generation benchmarks.')
    args = parser.parse_args()

    # Determine project root relative to this script
    script_dir = os.path.abspath(os.path.dirname(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, '..'))

    # Prepare output directory
    out_dir = os.path.join(project_root, 'docs', 'benches')
    os.makedirs(out_dir, exist_ok=True)

    # Step 1: Ensure cargo-criterion
    install_cargo_criterion()

    # Step 2: Run benchmarks
    bench_func = 'opcode_proving' if not args.generation else 'opcode_generation'
    print(f'🛠 Running benchmarks via cargo-criterion ({bench_func})...')

    # Get the current environment and add RUSTFLAGS
    env = os.environ.copy()
    env["RUSTFLAGS"] = "-C target-cpu=native"

    cargo_command = [
        'cargo', 'criterion',
        '--package', 'petravm-prover',
        '--bench', 'opcodes',
        '--', bench_func
    ]

    # Add features if provided
    if args.features:
        cargo_command.extend(['--features', args.features])

    subprocess.check_call(cargo_command, cwd=project_root, env=env)

    # Step 3: Parse benchmark.json
    criterion_dir = Path(project_root) / 'target' / 'criterion' / bench_func
    results = extract_criterion_results(criterion_dir)

    # Step 4: Write results to docs/benches/benchmarks.json
    bench_json_path = os.path.join(out_dir, 'benchmarks.json')
    with open(bench_json_path, 'w') as f:
        json.dump(results, f, indent=2)
    print(f'✅ Wrote filtered JSON array to {bench_json_path}')

    # Step 5: Ensure dashboard HTML exists in docs/benches
    user_html = os.path.join(out_dir, 'index.html')
    if os.path.exists(user_html):
        print(f'ℹ️ Found existing dashboard HTML at {user_html}, skipping copy.')
    else:
        print(f'⚠️ No index.html found in {out_dir}. Please place your dashboard HTML there.')

    print("All done! Commit & push docs/benches; view at https://petraprover.github.io/PetraVM/benches/")

if __name__ == '__main__':
    main()
