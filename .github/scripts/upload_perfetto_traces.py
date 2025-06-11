#!/usr/bin/env python3
import os
import glob
import subprocess
import urllib.parse
import datetime
import argparse

def parse_args():
    p = argparse.ArgumentParser(
        description="Upload Perfetto traces to S3 and append links to the GitHub summary."
    )
    p.add_argument("--perfetto-host", required=True, help="Base URL of your Perfetto host")
    p.add_argument("--s3-bucket",     required=True, help="Your S3 bucket URL (e.g. s3://my-bucket)")
    p.add_argument("--repo",          required=True, help="Repository name without owner/org prefix")
    p.add_argument("--branch",        required=True, help="Branch name")
    p.add_argument("--commit-sha",    required=True, help="Commit SHA")
    p.add_argument("--results-dir",   required=True, help="Top-level directory where .perfetto-trace files live")
    p.add_argument("--summary-path",  required=True, help="Path to GitHub summary file")
    return p.parse_args()

def main():
    args = parse_args()

    # sanitize inputs
    branch = args.branch.replace("/", "-")
    commit_sha_short = args.commit_sha[:7]
    now_utc = datetime.datetime.now(datetime.timezone.utc)
    run_dir = f"{now_utc:%Y-%m-%d_%H-%M-%S}-{commit_sha_short}"

    traces_by_benchmark = {}

    pattern = os.path.join(args.results_dir, "**", "*.perfetto-trace")
    for file_path in sorted(glob.glob(pattern, recursive=True)):
        file_name = os.path.basename(file_path)
        machine = os.path.basename(os.path.dirname(file_path)).removeprefix("perfetto-traces-")
        benchmark, mode, _, run_id, _ = file_name.split("-", 4)
        thread  = f"{mode}-thread"

        s3_key = f"traces/{args.repo}/{branch}/{benchmark}/{thread}/{machine}/{run_dir}/{machine}-{file_name}"
        s3_dest = f"{args.s3_bucket}/{s3_key}"
        subprocess.run(["aws", "s3", "cp", file_path, s3_dest], check=True)

        trace_binary_url = f"{args.perfetto_host}/{s3_key}"
        perfetto_ui_url = f"{args.perfetto_host}/#!/?url={urllib.parse.quote_plus(trace_binary_url)}"

        label = f"{benchmark} ({thread}) on {machine}"
        link = f"<a href='{perfetto_ui_url}' target='_blank'>#{run_id}</a>"
        traces_by_benchmark.setdefault(benchmark, {}).setdefault(label, []).append(link)

    # append to GitHub summary
    with open(args.summary_path, "a") as summary:
        summary.write("## 📊 Perfetto Traces\n")
        for benchmark, groups in traces_by_benchmark.items():
            summary.write(f"<details><summary>{benchmark}</summary>\n<ul>\n")
            for label, links in groups.items():
                summary.write(f"<li>{label}: " + " ".join(links) + "</li>\n")
            summary.write("</ul>\n</details>\n")

if __name__ == "__main__":
    main()
