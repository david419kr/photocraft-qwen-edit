import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "provenance", Path(__file__).resolve().parents[1] / "verify-candidate-provenance.py")
provenance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provenance)


class CandidateProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.state = dict(upstream_tag="v0.20.2", upstream_release_id=434651642,
                          upstream_sha="a" * 40, fork_base_sha="b" * 40)

    def test_matching_identity(self):
        provenance.verify(dict(self.state, source_sha="c" * 40), self.state)

    def test_stale_or_missing_identity_fields_are_rejected(self):
        for field in self.state:
            for value in (None, "stale"):
                with self.subTest(field=field, value=value):
                    metadata = dict(self.state)
                    metadata[field] = value
                    with self.assertRaisesRegex(ValueError, field):
                        provenance.verify(metadata, self.state)

    def test_missing_source_state_is_rejected(self):
        for field in self.state:
            with self.subTest(field=field):
                state = dict(self.state)
                del state[field]
                with self.assertRaisesRegex(ValueError, field):
                    provenance.verify(self.state, state)


if __name__ == "__main__":
    unittest.main()
