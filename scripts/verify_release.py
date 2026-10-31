"""Release-gate helpers for the release workflow; identical under PowerShell and bash.

fetch      download the draft candidate, check SHA256SUMS, fetch the previous release's host archive
run        run artifacts-mode acceptance, keep its report and write the job summary
summarize  write the revoke summary from every uploaded report and the failed jobs

Environment: RELEASE_TAG, GITHUB_REPOSITORY, RELEASE_TARGET (fetch/run),
VERIFY_NAME (run), NEEDS_JSON and REVOKED_TAG (summarize). Requires the gh CLI.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

UNVERIFIED_NOTE = "packaged but not verified on a native runner"


def gh(*args: str, capture: bool = False) -> str:
    result = subprocess.run(["gh", *args], check=True, text=True, capture_output=capture)
    return result.stdout if capture else ""


def checksums(directory: Path) -> dict[str, str]:
    """Verify every listed asset that is present; return the listing."""
    listed = {}
    for line in (directory / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
        digest, name = line.split("  ", 1)
        listed[name] = digest
    for path in directory.iterdir():
        if path.name == "SHA256SUMS":
            continue
        if path.name not in listed:
            raise SystemExit(f"{path.name} is not listed in SHA256SUMS")
        with path.open("rb") as stream:
            actual = hashlib.file_digest(stream, "sha256").hexdigest()
        if actual != listed[path.name]:
            raise SystemExit(f"{path.name}: digest {actual} does not match SHA256SUMS")
    return listed


def previous_release(repo: str, tag: str) -> str | None:
    """The newest published, non-prerelease release other than the candidate."""
    releases = json.loads(gh("api", f"repos/{repo}/releases?per_page=50", capture=True))
    published = [r for r in releases if not r["draft"] and not r["prerelease"] and r["tag_name"] != tag]
    published.sort(key=lambda r: r["published_at"] or "", reverse=True)
    return published[0]["tag_name"] if published else None


def fetch(args) -> None:
    tag, repo, target = os.environ["RELEASE_TAG"], os.environ["GITHUB_REPOSITORY"], os.environ["RELEASE_TARGET"]
    dist, previous = Path(args.dist), Path(args.previous)
    dist.mkdir(parents=True, exist_ok=True)
    previous.mkdir(parents=True, exist_ok=True)
    gh("release", "download", tag, "--repo", repo, "--dir", str(dist))
    listed = checksums(dist)
    host = f"saucepan-{tag}-{target}.zip"
    if host not in listed or not (dist / host).is_file():
        raise SystemExit(f"candidate lacks {host}")
    print(f"candidate {tag}: {len(listed)} assets match SHA256SUMS")
    prior = previous_release(repo, tag)
    if prior is None:
        print("no previous published release; release_upgrade will be skipped")
        return
    try:
        gh("release", "download", prior, "--repo", repo, "--dir", str(previous),
           "--pattern", f"saucepan-{prior}-{target}.zip")
        print(f"previous release {prior}")
    except subprocess.CalledProcessError:
        print(f"previous release {prior} has no {target} archive; release_upgrade will be skipped")


def summary(text: str) -> None:
    path = os.environ.get("GITHUB_STEP_SUMMARY")
    if path:
        with open(path, "a", encoding="utf-8") as output:
            output.write(text + "\n")
    print(text)


def reproduction(report: dict, run_id: str, scenario: str | None) -> str:
    """Commands that rerun a failed scenario against the same packaged artifacts."""
    target = report.get("target", "<target>")
    previous = ((report.get("artifacts") or {}).get("previous") or {}).get("tag")
    steps = [f"gh run download {run_id} -n staged-release -D dist",
             f"gh run download {run_id} -n verify-twin-{target} -D twin"]
    command = "python -m acceptance --artifacts dist --twin twin"
    if previous:
        steps.append(f"gh release download {previous} --repo ZackaryW/saucepan "
                     f"--pattern saucepan-{previous}-{target}.zip --dir previous")
        command += " --previous previous"
    if scenario:
        command += f" --name {scenario}"
    return "; ".join([*steps, command])


def run(args) -> None:
    name, target = os.environ["VERIFY_NAME"], os.environ["RELEASE_TARGET"]
    command = [sys.executable, "-B", "-m", "acceptance", "--artifacts", args.dist, "--twin", args.twin]
    if any(Path(args.previous).glob("saucepan-*.zip")):
        command += ["--previous", args.previous]
    code = subprocess.run(command).returncode
    latest = Path("target/acceptance/latest.json")
    reports = Path(args.reports)
    reports.mkdir(parents=True, exist_ok=True)
    if not latest.is_file():
        summary(f"### {name}: acceptance produced no report (exit {code})")
        raise SystemExit(code or 1)
    report = json.loads(latest.read_text(encoding="utf-8"))
    report["verify_name"], report["target"] = name, target
    (reports / f"acceptance-{name}.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    failed = [k for k, v in report["scenarios"].items() if v["outcome"] == "failed"]
    artifacts = report.get("artifacts") or {}
    lines = [f"### {name} ({report['environment']['launch_environment']}, {target}): "
             f"{'FAILED' if failed or code else 'passed'}",
             f"summary: {report['summary']}"]
    lines += [f"- failed: `{scenario}`" for scenario in failed]
    if artifacts.get("unverified_targets"):
        lines.append(f"- {UNVERIFIED_NOTE}: {', '.join(artifacts['unverified_targets'])}")
    if failed:
        lines.append(f"- reproduce: `{reproduction(report, os.environ.get('GITHUB_RUN_ID', '<run>'), failed[0].rsplit('.', 1)[-1])}`")
    summary("\n".join(lines))
    raise SystemExit(code)


def summarize(args) -> None:
    tag = os.environ.get("RELEASE_TAG", "")
    needs = json.loads(os.environ.get("NEEDS_JSON") or "{}")
    failed_jobs = [job for job, result in needs.items() if result.get("result") in ("failure", "cancelled")]
    lines = [f"## Release candidate {tag} revoked",
             f"- tag moved to `{os.environ.get('REVOKED_TAG', '(not moved)')}`",
             f"- failed or cancelled jobs: {', '.join(failed_jobs) or 'none recorded'}"]
    run_id = os.environ.get("GITHUB_RUN_ID", "<run>")
    for path in sorted(Path(args.reports).rglob("acceptance-*.json")):
        report = json.loads(path.read_text(encoding="utf-8"))
        failed = [k for k, v in report["scenarios"].items() if v["outcome"] == "failed"]
        state = "failed" if failed else "passed"
        lines.append(f"### {report.get('verify_name', path.stem)}: {state}")
        lines += [f"- `{scenario}`" for scenario in failed]
        if failed:
            lines.append(f"- reproduce: `{reproduction(report, run_id, failed[0].rsplit('.', 1)[-1])}`")
    lines.append("Fix the cause, commit, and push the same version tag again to retry.")
    summary("\n".join(lines))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("fetch", "run"):
        command = commands.add_parser(name)
        command.add_argument("--dist", default="dist")
        command.add_argument("--twin", default="twin")
        command.add_argument("--previous", default="previous")
        command.add_argument("--reports", default="reports")
    commands.add_parser("summarize").add_argument("--reports", default="reports")
    args = parser.parse_args()
    {"fetch": fetch, "run": run, "summarize": summarize}[args.command](args)


if __name__ == "__main__":
    main()
