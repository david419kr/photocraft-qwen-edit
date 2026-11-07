#!/usr/bin/env python3
"""Skip duplicate main CI only with server-side proof of the tested merge tree."""
import json
import os
import re
import urllib.request
from pathlib import Path

REQUIRED_JOBS = {
    "early Clippy", "select platforms", "gate / gate (ubuntu-24.04)",
    "test (without heif)", "corpus tests", "web build and wasm size gate",
    "test (windows-latest, unit)", "test (windows-latest, integration)",
}

PROVENANCE = re.compile(r"CI provenance \(PR ([0-9]+), ([0-9a-f]{40})\)")


def reusable(event, get):
    """All uncertainty means run CI. `get` reads repository-relative GitHub REST URLs."""
    after, before = event.get("after"), event.get("before")
    if (event.get("ref") != "refs/heads/main" or event.get("forced") is not False
            or event.get("deleted") or event.get("created")
            or not re.fullmatch(r"[0-9a-f]{40}", after or "")
            or not re.fullmatch(r"[0-9a-f]{40}", before or "")
            or before == "0" * 40):
        return False
    repository = event["repository"]["full_name"]
    pushed = get(f"git/commits/{after}")
    # A single normal merge or squash; never infer approval from a commit message.
    if pushed["parents"][0]["sha"] != before:
        return False
    for summary in get(f"commits/{after}/pulls?per_page=100"):
        pr = get(f"pulls/{summary['number']}")
        if not (pr.get("merged") is True and pr.get("merge_commit_sha") == after
                and pr["base"]["ref"] == "main"
                and pr["base"]["repo"]["full_name"] == repository):
            continue
        head = pr["head"]["sha"]
        runs = get(f"actions/workflows/ci.yml/runs?event=pull_request&head_sha={head}&per_page=100")["workflow_runs"]
        # Do not reuse an earlier green run after a failing/in-progress rerun.
        matching = [r for r in runs if r.get("event") == "pull_request"
                    and r.get("head_sha") == head
                    and r.get("path") == ".github/workflows/ci.yml"
                    and r["repository"]["full_name"] == repository
                    and r["head_branch"] == pr["head"]["ref"]
                    and r["head_repository"]["id"] == pr["head"]["repo"]["id"]]
        if not matching:
            continue
        run = max(matching, key=lambda r: r["id"])
        if (run.get("status") != "completed" or run.get("conclusion") != "success"
                or run["updated_at"] > pr["merged_at"]):
            continue
        jobs = get(f"actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs?per_page=100")
        if jobs["total_count"] > len(jobs["jobs"]):
            continue
        successful = {j["name"] for j in jobs["jobs"] if j.get("conclusion") == "success"}
        if not REQUIRED_JOBS <= successful:
            continue
        proofs = [PROVENANCE.fullmatch(j["name"]) for j in jobs["jobs"]
                  if j.get("conclusion") == "success"]
        proofs = [p for p in proofs if p]
        if len(proofs) != 1 or int(proofs[0].group(1)) != pr["number"]:
            continue
        tested = get(f"git/commits/{proofs[0].group(2)}")
        # pull_request GITHUB_SHA is GitHub's test merge, not workflow_run.head_sha.
        # Check both parents to reject stale-base validation, then the complete tree.
        if ([p["sha"] for p in tested["parents"]] == [before, head]
                and tested["tree"]["sha"] == pushed["tree"]["sha"]):
            return True
    return False


def decide(event, get):
    try:
        return reusable(event, get)
    except Exception as error:  # API failures, schema drift and missing proof must run CI.
        print(f"PR validation could not be proven ({type(error).__name__}); running CI.")
        return False


def main():
    skip = False
    if os.environ.get("GITHUB_EVENT_NAME") == "push":
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
        repo = os.environ["GITHUB_REPOSITORY"]

        requests_left = 20

        def get(path):
            nonlocal requests_left
            requests_left -= 1
            if requests_left < 0:
                raise RuntimeError("proof request budget exceeded")
            request = urllib.request.Request(
                f"https://api.github.com/repos/{repo}/{path}",
                headers={"Authorization": f"Bearer {os.environ['GH_TOKEN']}",
                         "Accept": "application/vnd.github+json",
                         "X-GitHub-Api-Version": "2022-11-28"})
            with urllib.request.urlopen(request, timeout=15) as response:
                return json.load(response)

        skip = decide(event, get)
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"skip={'true' if skip else 'false'}\n")
    print("Reusing successful PR validation of this exact tree." if skip else "Running normal CI.")


if __name__ == "__main__":
    main()
