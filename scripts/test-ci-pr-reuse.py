#!/usr/bin/env python3
"""Offline regression tests for positive proof and fail-open-to-run CI selection."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import tempfile
import os

SPEC = importlib.util.spec_from_file_location("reuse", Path(__file__).with_name("ci-pr-reuse.py"))
reuse = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reuse)
A, B, H, M, T = [c * 40 for c in "abcde"]
REPO = "owner/repo"


class ReuseTests(unittest.TestCase):
    def setUp(self):
        self.event = dict(after=A, before=B, ref="refs/heads/main", forced=False,
                          repository={"full_name": REPO})
        self.pr = dict(number=7, merged=True, merge_commit_sha=A,
                       merged_at="2026-11-07T12:00:00Z",
                       base={"ref": "main", "repo": {"full_name": REPO}},
                       head={"sha": H, "ref": "feature", "repo": {"id": 1}})
        self.run = dict(id=10, event="pull_request", head_sha=H, head_branch="feature",
                        path=".github/workflows/ci.yml", repository={"full_name": REPO},
                        head_repository={"id": 1}, pull_requests=[], run_attempt=1,
                        status="completed", conclusion="success", updated_at="2026-11-07T11:00:00Z")
        self.jobs = [{"name": n, "conclusion": "success"} for n in reuse.REQUIRED_JOBS]
        self.jobs.append({"name": f"CI provenance (PR 7, {M})", "conclusion": "success"})
        self.data = {f"git/commits/{A}": {"parents": [{"sha": B}, {"sha": H}], "tree": {"sha": T}},
                     f"git/commits/{M}": {"parents": [{"sha": B}, {"sha": H}], "tree": {"sha": T}},
                     f"commits/{A}/pulls?per_page=100": [{"number": 7}], "pulls/7": self.pr,
                     f"actions/workflows/ci.yml/runs?event=pull_request&head_sha={H}&per_page=100": {"workflow_runs": [self.run]},
                     "actions/runs/10/attempts/1/jobs?per_page=100": {"total_count": len(self.jobs), "jobs": self.jobs}}

    def result(self):
        return reuse.decide(self.event, self.data.__getitem__)

    def test_exact_merge(self):
        self.assertTrue(self.result())

    def test_squash_same_tree(self):
        self.data[f"git/commits/{A}"]["parents"].pop()
        self.assertTrue(self.result())

    def test_push_kinds(self):
        for key, value in [("forced", True), ("created", True), ("deleted", True),
                           ("ref", "refs/heads/release"), ("before", "0" * 40)]:
            with self.subTest(key=key):
                original = self.event.copy()
                self.event[key] = value
                self.assertFalse(self.result())
                self.event = original

    def test_direct_push(self):
        self.data[f"commits/{A}/pulls?per_page=100"] = []
        self.assertFalse(self.result())

    def test_pr_not_merged_or_wrong_target(self):
        for key, value in [("merged", False), ("merge_commit_sha", B)]:
            with self.subTest(key=key):
                old = self.pr[key]
                self.pr[key] = value
                self.assertFalse(self.result())
                self.pr[key] = old

    def test_tree_or_parent_mismatch(self):
        commit = self.data[f"git/commits/{M}"]
        for field, value in [("tree", {"sha": B}), ("parents", [{"sha": A}, {"sha": H}]),
                             ("parents", [{"sha": B}, {"sha": A}]), ("parents", [{"sha": H}])]:
            with self.subTest(field=field, value=value):
                old = commit[field]
                commit[field] = value
                self.assertFalse(self.result())
                commit[field] = old

    def test_non_success_stale_or_post_merge_run(self):
        for key, value in [("head_sha", B), ("status", "in_progress"), ("conclusion", "failure"),
                           ("conclusion", "cancelled"), ("event", "push"),
                           ("updated_at", "2026-11-07T13:00:00Z"),
                           ("path", ".github/workflows/other.yml"), ("head_repository", {"id": 2})]:
            with self.subTest(key=key):
                old = self.run[key]
                self.run[key] = value
                self.assertFalse(self.result())
                self.run[key] = old

    def test_newer_failed_run_overrides_green(self):
        newer = dict(self.run, id=11, conclusion="failure")
        next(d for d in self.data.values() if isinstance(d, dict) and "workflow_runs" in d)["workflow_runs"].append(newer)
        self.assertFalse(self.result())

    def test_every_required_job_must_succeed(self):
        for job in self.jobs:
            with self.subTest(job=job["name"]):
                job["conclusion"] = "skipped"
                self.assertFalse(self.result())
                job["conclusion"] = "success"

    def test_proof_wrong_pr_or_head_only(self):
        for name in [f"CI provenance (PR 8, {M})", f"CI provenance (PR 7, {H})", "CI provenance"]:
            self.jobs[-1]["name"] = name
            self.assertFalse(self.result())

    def test_missing_api_or_malformed_response(self):
        for key in list(self.data):
            with self.subTest(key=key):
                value = self.data.pop(key)
                self.assertFalse(self.result())
                self.data[key] = value

    def test_truncated_jobs(self):
        self.data["actions/runs/10/attempts/1/jobs?per_page=100"]["total_count"] = 101
        self.assertFalse(self.result())

    def test_manual_and_pr_never_skip(self):
        for event in ["workflow_dispatch", "pull_request"]:
            with tempfile.NamedTemporaryFile() as output:
                with patch.dict(os.environ, {"GITHUB_EVENT_NAME": event, "GITHUB_OUTPUT": output.name}):
                    reuse.main()
                self.assertEqual(Path(output.name).read_text(), "skip=false\n")

    def test_api_exception_runs_ci(self):
        def unavailable(_):
            raise TimeoutError("offline")
        self.assertFalse(reuse.decide(self.event, unavailable))

    def test_batched_merge_runs_ci(self):
        self.data[f"git/commits/{A}"]["parents"][0]["sha"] = H
        self.assertFalse(self.result())

    def test_missing_provenance_runs_ci(self):
        self.jobs.pop()
        self.assertFalse(self.result())

    def test_workflow_contract(self):
        text = (Path(__file__).resolve().parents[1] / ".github/workflows/ci.yml").read_text()
        self.assertIn("name: CI provenance (PR ${{ github.event.pull_request.number || 0 }}, ${{ github.sha }})", text)
        self.assertEqual(text.count("if: always() && (needs.provenance.result != 'success' || needs.provenance.outputs.skip != 'true')"), 2)
        # These jobs must use default checkout of github.sha, not a PR head/custom ref.
        for job in ["provenance", "lint", "platforms", "windows", "test-without-heif", "corpus", "web"]:
            block = text.split(f"\n  {job}:\n", 1)[1].split("\n  #", 1)[0]
            checkout = block.split("uses: actions/checkout@", 1)[1].split("\n      - ", 1)[0]
            self.assertNotIn("ref:", checkout)
            self.assertNotIn("repository:", checkout)
        self.assertIn("uses: storytold/craft-common-actions/.github/workflows/gate.yml@main", text)
        freebsd = (Path(__file__).resolve().parents[1] / ".github/workflows/freebsd.yml").read_text()
        self.assertIn("on:\n  workflow_dispatch:\n", freebsd)


if __name__ == "__main__":
    unittest.main()
