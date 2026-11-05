#!/usr/bin/env python3
"""Reject candidate artifacts whose upstream identity differs from recorded source."""
import argparse
import json
from pathlib import Path


def verify(metadata, state):
    for field in ("upstream_tag", "upstream_release_id", "upstream_sha", "fork_base_sha"):
        expected = state.get(field)
        if not expected or metadata.get(field) != expected:
            raise ValueError(f"Candidate {field} does not match upstream update state")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--state", type=Path, required=True)
    args = parser.parse_args()
    verify(json.loads(args.metadata.read_text(encoding="utf-8-sig")),
           json.loads(args.state.read_text(encoding="utf-8-sig")))
    print("Verified candidate upstream release, tag, source and fork base")


if __name__ == "__main__":
    main()
