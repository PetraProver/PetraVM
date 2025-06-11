#!/usr/bin/env python3
from typing import Any, List, Dict, Optional
from dataclasses import dataclass, field
import argparse
from pathlib import Path
import subprocess
import os
import sys
import time

#
# Benchmark Config
#


@dataclass(frozen=True)
class BenchmarkConfig:
    """
    Configuration for a single benchmark.
    """
    name: str
    display: str
    args: List[str]
    n_ops: int
    single_threaded: bool = False


# Note: Every benchmark is multi-threaded by default. On top of that it can be run in single-threaded mode.
#  ┌──── name ───┬────── display ───────┬───────────────────── args ─────────────────────┬─ n_ops ─┬─ single_threaded ─┐
_RAW_BENCH_ROWS = [
  ("fibonacci",  "fibonacci",           ["fibonacci", "--", "--n"],                          100000,   True),
]

BENCHMARKS = {
    name: BenchmarkConfig(name, display, args, n_ops, single_threaded)
    for name, display, args, n_ops, single_threaded in _RAW_BENCH_ROWS
}


#
# Parse Script Arguments
#


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run one benchmark example multiple times"
    )
    parser.add_argument(
        "--benchmark", "-b",
        choices=list(BENCHMARKS.keys()) + ["all"],
        default="all",
        help="Which benchmark to run (default: all)",
    )
    parser.add_argument(
        "--output-dir", "-o",
        type=Path,
        default=Path("benchmark_results"),
        help="Directory to write CSVs & traces",
    )
    parser.add_argument(
        "--samples", "-n",
        type=int,
        default=5,
        help="How many times to repeat the benchmark",
    )
    parser.add_argument(
        "--perfetto/--no-perfetto",
        dest="perfetto",
        default=True,
        help="Whether to generate Perfetto traces",
    )
    parser.add_argument(
        "--clean",
        action="store_true",
        help="Clean previous results for selected benchmarks before running",
    )
    return parser.parse_args()


#
# Execute a Benchmark
#


def run_single_benchmark(
    name: str,
    args: List[str],
    n_ops: int,
    outdir: Path,
    samples: int = 5,
    perfetto: bool = True,
    multi_threaded: bool = True,
) -> None:
    """
    Run one benchmark `samples` times.
    Returns a dict mapping metric keys to lists of values.
    """

    mode = 'multi-thread' if multi_threaded else 'single-thread'

    env = {
        **os.environ,
        'RAYON_NUM_THREADS': '0' if multi_threaded else '1',
        'RUSTFLAGS': '-C target-cpu=native',
    }

    cmd = ['cargo', 'run', '--release', '--example'] + args + [str(n_ops)]

    if perfetto:
        env['PERFETTO_TRACE_DIR'] = str(outdir)
        # Insert 'perfetto' feature args at index 3
        cmd[3:3] = ['--features', 'perfetto']

    # File containing the path to the last perfetto trace
    trace_pointer = Path('.last_perfetto_trace_path')

    for i in range(1, samples + 1):
        # Prepare for next iteration
        trace_pointer.unlink(missing_ok=True)

        csv_result_path = outdir / f"{name}-{mode}-{i}.csv"
        env['PROFILE_CSV_FILE'] = str(csv_result_path)
        print(f"Running {name} ({mode}) sample #{i}")
        print(f"{' '.join(cmd)}")

        # Run the benchmark
        proc = subprocess.Popen(
            cmd,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        # Print a “heartbeat” while it’s running
        last_dot = 0
        while proc.poll() is None:
            time.sleep(0.5)
            if time.monotonic() - last_dot >= 5:
                print('.', end='', flush=True)
                last_dot = time.monotonic()

        stdout, stderr = proc.communicate()

        if proc.returncode != 0:
            print(" failed")
            print(stdout, end='')
            print(stderr, file=sys.stderr, end='')
            sys.exit(proc.returncode)
        else:
            print(" done")

        # Post-run: Collect results and clean up
        if perfetto:
            # Rename Perfetto trace file
            if not trace_pointer.exists():
                raise RuntimeError("Perfetto trace file not found (missing .last_perfetto_trace_path)")
            trace_path = Path(trace_pointer.read_text().strip())
            if not trace_path.exists():
                raise RuntimeError(f"Perfetto trace file not found: {trace_path} (read from .last_perfetto_trace_path)")
            trace_pointer.unlink()

            new_path = trace_path.parent / f"{name}-{mode}-{i}-{trace_path.name}"
            trace_path.rename(new_path)

        if not csv_result_path.exists():
            raise RuntimeError(f"CSV result file not found: {csv_result_path}")

#
# Main
#


def main():
    args = parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)

    # Determine benchmarks to run
    if args.benchmark == "all":
        run_names = list(BENCHMARKS.keys())
    else:
        run_names = [args.benchmark]

    # Clean previous result files if requested
    if args.clean:
        for benchmark_name in run_names:
            prefix = f"{benchmark_name}-"
            for f in args.output_dir.glob(f"{prefix}*"):
                try:
                    f.unlink()
                except OSError:
                    pass

    # Run each benchmark and write its results
    for benchmark_name in run_names:
        cfg = BENCHMARKS[benchmark_name]

        # Always run multi-threaded
        run_single_benchmark(
            name=cfg.name,
            args=cfg.args,
            n_ops=cfg.n_ops,
            outdir=args.output_dir,
            samples=args.samples,
            perfetto=args.perfetto,
            multi_threaded=True,
        )
        # Then, if configured, run single-threaded too
        if cfg.single_threaded:
            run_single_benchmark(
                name=cfg.name,
                args=cfg.args,
                n_ops=cfg.n_ops,
                outdir=args.output_dir,
                samples=args.samples,
                perfetto=args.perfetto,
                multi_threaded=False,
            )


if __name__ == "__main__":
    main()
