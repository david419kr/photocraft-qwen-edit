import copy
import importlib.util
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).parent
spec = importlib.util.spec_from_file_location('publisher', ROOT/'publish.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class SafetyTests(unittest.TestCase):
    def config(self):
        c = json.loads((ROOT/'config.json').read_text())
        for k in ('feature_sha', 'base_sha', 'feature_tree', 'build_sha', 'build_tree', 'merge_sha', 'upstream_sha', 'fork_base_sha'):
            c[k] = 'a'*40
        for k in ('artifact_sha256', 'regression_artifact_sha256', 'zip_sha256', 'executable_sha256'):
            c[k] = 'b'*64
        for k in ('pr', 'pr_ci_run', 'candidate_run', 'post_merge_ci_run', 'artifact_id', 'regression_artifact_id'):
            c[k] = 1
        c.update(tag='261110-0.21.0', title='PhotoCraft test', publisher_branch='publish/test', publish_enabled=False, publish_tag='')
        return c

    def test_pending_rejected(self):
        c = self.config()
        c['merge_sha'] = 'PENDING_MERGE_SHA'
        with self.assertRaisesRegex(RuntimeError, 'Unresolved'):
            m.validate_config(c)

    def test_valid_config(self):
        m.validate_config(self.config())

    def test_bad_digest_rejected(self):
        c = self.config()
        c['zip_sha256'] = 'x'*64
        with self.assertRaisesRegex(RuntimeError, 'zip_sha256'):
            m.validate_config(c)

    def test_extra_metadata_disallowed(self):
        c = self.config()
        c['metadata_allowlist'].append('crates/engine/src/lib.rs')
        with self.assertRaisesRegex(RuntimeError, 'allowance'):
            m.validate_config(c)

    def test_empty_regressions_disallowed(self):
        c = self.config()
        c['focused_groups'] = 0
        with self.assertRaisesRegex(RuntimeError, 'Focused'):
            m.validate_config(c)

    def test_failed_run_rejected(self):
        m.C = self.config()
        obj = dict(head_sha='a'*40, path='.github/workflows/ci.yml', repository={'full_name': m.C['repo']}, status='completed', conclusion='failure')
        with patch.object(m, 'api', return_value=obj), self.assertRaisesRegex(RuntimeError, 'not successful'):
            m.checked_run(1, 'a'*40, '.github/workflows/ci.yml')

    def test_wrong_run_source_rejected(self):
        m.C = self.config()
        obj = dict(head_sha='c'*40, path='.github/workflows/ci.yml')
        with patch.object(m, 'api', return_value=obj), self.assertRaisesRegex(RuntimeError, 'source mismatch'):
            m.checked_run(1, 'a'*40, '.github/workflows/ci.yml')

    def test_pagination(self):
        m.C = self.config()
        with patch.object(m, 'api', side_effect=[{'jobs': list(range(100))}, {'jobs': [100]}]) as api:
            self.assertEqual(len(m.pages('x', 'jobs')), 101)
            self.assertEqual(api.call_count, 2)

    def test_enabled_config_requires_exact_tag(self):
        c = self.config()
        c['publish_enabled'] = True
        with self.assertRaisesRegex(RuntimeError, 'publication tag'):
            m.validate_config(c)
        c['publish_tag'] = c['tag']
        m.validate_config(c)

    def test_publication_disabled_by_default(self):
        m.C = self.config()
        with self.assertRaisesRegex(RuntimeError, 'disabled'):
            m.publish_authorization()

    def test_only_exact_branch_push_can_publish(self):
        m.C = self.config()
        m.C.update(publish_enabled=True, publish_tag=m.C['tag'])
        env = {'RELEASE_PUBLISH_APPROVED': m.C['tag'], 'GITHUB_EVENT_NAME': 'push', 'GITHUB_REF': 'refs/heads/'+m.C['publisher_branch']}
        with patch.dict(os.environ, env, clear=True):
            m.publish_authorization()
            os.environ['GITHUB_EVENT_NAME'] = 'workflow_dispatch'
            with self.assertRaisesRegex(RuntimeError, 'branch push'):
                m.publish_authorization()
            os.environ['GITHUB_EVENT_NAME'] = 'push'
            os.environ['GITHUB_REF'] = 'refs/heads/main'
            with self.assertRaisesRegex(RuntimeError, 'Wrong publisher branch'):
                m.publish_authorization()

    def post_jobs(self):
        m.C = self.config()
        return [
            {'id': 17, 'name': f"CI provenance (PR 0, {m.C['merge_sha']})", 'status': 'completed', 'conclusion': 'success'},
            {'id': 18, 'name': 'test (windows-latest, ${{ matrix.part }})', 'status': 'completed', 'conclusion': 'skipped'},
            {'id': 19, 'name': 'gate', 'status': 'completed', 'conclusion': 'skipped'},
        ]

    def test_skipped_matrix_and_reusable_gate_accepted(self):
        jobs = self.post_jobs()
        with patch.object(m, 'run', return_value='2026-11-09T23:50:00Z Reusing successful PR validation of this exact tree.\n') as run:
            m.verify_post_merge_jobs(jobs)
            run.assert_called_once_with('gh', 'api', f"repos/{m.C['repo']}/actions/jobs/17/logs")

    def test_post_main_without_proof_log_rejected(self):
        jobs = self.post_jobs()
        with patch.object(m, 'run', return_value='Running normal CI.'), self.assertRaisesRegex(RuntimeError, 'proof missing'):
            m.verify_post_merge_jobs(jobs)

    def test_post_main_extra_success_rejected(self):
        jobs = self.post_jobs()
        jobs[1]['conclusion'] = 'success'
        with patch.object(m, 'run') as run, self.assertRaisesRegex(RuntimeError, 'provenance only'):
            m.verify_post_merge_jobs(jobs)
        run.assert_not_called()

    def test_post_main_failure_rejected(self):
        jobs = self.post_jobs()
        jobs[2]['conclusion'] = 'failure'
        with patch.object(m, 'run') as run, self.assertRaisesRegex(RuntimeError, 'clean reuse'):
            m.verify_post_merge_jobs(jobs)
        run.assert_not_called()

    def test_release_notes_have_no_preview_claim(self):
        notes = (ROOT/'release-notes.md').read_text()
        self.assertNotIn('with preview', notes)
        self.assertIn('Selected Areas', notes)
        self.assertIn('85fe92d71fbe03cdf1c7990c7b29fb5332cb63bc/docs/external-cutout-backends.md', notes)

    def test_updater_probe_covers_recent_and_same_day_releases(self):
        source = (ROOT/'check-updater-identity.py').read_text()
        self.assertIn('previous_tags = ["261107-0.21.0"]', source)
        self.assertIn('if tag == "261109-2-0.21.0":', source)
        self.assertIn('previous_tags.append("261109-0.21.0")', source)
        self.assertIn('select(fixture(tag, name, hash), tag).unwrap().is_none()', source)

    def test_public_release_never_mutated(self):
        m.C = self.config()
        obj = {'id': 1, 'tag_name': m.C['tag'], 'draft': False}
        with patch.object(m, 'pages', return_value=[obj]), patch.object(m, 'api') as api, self.assertRaisesRegex(RuntimeError, 'Public release exists'):
            m.publish([], {}, {}, None, None)
        api.assert_not_called()


if __name__ == '__main__':
    unittest.main()
