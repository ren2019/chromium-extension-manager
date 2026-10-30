#!/usr/bin/env python3
"""Validate the app version and an optional release tag before building."""
import os
import re
import subprocess
from pathlib import Path


def main():
    import json

    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], text=True
    ))
    version = next(p["version"] for p in metadata["packages"] if p["name"] == "cepm-app")
    # Stable versions and ordinary SemVer prereleases; no ambiguous leading zeros.
    numeric = r"(?:0|[1-9][0-9]*)"
    identifier = rf"(?:{numeric}|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)"
    if not re.fullmatch(rf"{numeric}\.{numeric}\.{numeric}(?:-{identifier}(?:\.{identifier})*)?", version):
        raise SystemExit(f"Unsupported release version: {version}")
    tag = f"v{version}"
    ref = os.environ.get("GITHUB_REF", "")
    if ref.startswith("refs/tags/"):
        if ref != f"refs/tags/{tag}":
            raise SystemExit(f"Tag {ref} does not match cepm-app version {version}")
        subprocess.run(["git", "merge-base", "--is-ancestor", "HEAD", "origin/main"], check=True)
    lines = f"version={version}\ntag={tag}\nprerelease={str('-' in version).lower()}\n"
    print(lines, end="")
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8") as handle:
            handle.write(lines)


if __name__ == "__main__":
    main()
