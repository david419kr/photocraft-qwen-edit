# Reusing PR CI after merge

The main-push `CI` workflow starts a small read-only provenance job. It skips the
expensive duplicate jobs only after proving all of the following from GitHub:

- The push is an ordinary main update, and its first parent is the previous main.
- GitHub associates the pushed SHA with a merged PR into this repository's main.
- The latest CI run for the PR's current head/repository/branch succeeded before
  merge, including Ubuntu, both Windows legs, without-HEIF, corpus and web jobs.
- The successful provenance job's server-expanded name records that PR number
  and its `github.sha`. For `pull_request`, this is the synthetic test merge,
  whereas the workflow-run API's `head_sha` is the branch head.
- The tested merge's parents are exactly the previous main and current PR head,
  and its complete Git tree equals the pushed commit's tree.

The provenance job uses the same default `actions/checkout` target as the test
jobs. Regression tests guard that checkout contract. Changes to checkout refs,
job names, gate layout or job coverage must update and review this policy too.
No commit-message markers, PR artifacts, user text, or successful head checks
alone can authorize a skip. Tokens have read-only permissions; no privileged
`pull_request_target` workflow is introduced. GitHub can omit `pull_requests`
from historical workflow responses, so the proof uses the server-side PR number
in the job name as well as repository, branch and exact parents.

Missing proof, a stale base, a failed/running newer run, partial platform coverage,
API failures, inaccessible/deleted data, or a failed/timed-out provenance job
runs normal CI. Queries use at most 100 results per response and no pagination;
missing/truncated job evidence cannot justify a skip. A normal merge or squash
can qualify; multi-commit rebase/batched/forced pushes conservatively run again.
PR and manual triggers keep their existing checks. Narrow PRs without Windows
or web validation do not qualify. Old runs without provenance do not qualify.

Only `.github/workflows/ci.yml` is deduplicated. Other workflows, release tags and
assets are unchanged. FreeBSD remains manual-only. Manual CI can always rerun
all validation. Reusing validation does not warm main-only build caches; an
occasional manual main run can do that. Shared actions still track their existing
refs, and dependency/toolchain updates over time are not a reason to reinterpret
an already completed run; manual CI is available for fresh validation.

Run the offline policy/checkout-contract suite:

```sh
python3 scripts/test-ci-pr-reuse.py
```
