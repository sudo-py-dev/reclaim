"""Project audit: python3 scripts/check_project.py [--full] [--previews DIR].

Uses only Python 3.11+ standard-library modules. Full mode runs the compiler's
checks and regression tests; static checks do not attempt to parse Rust.
"""
import argparse
import ast
from collections import Counter
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def check_catalogs(catalogs):
    base = catalogs["en"]
    for language, catalog in catalogs.items():
        if catalog.keys() != base.keys():
            raise ValueError(f"{language}: missing {base.keys() - catalog.keys()}, "
                             f"extra {catalog.keys() - base.keys()}")
        for key, value in catalog.items():
            if not isinstance(value, str) or not value.strip():
                raise ValueError(f"{language}: empty/non-string translation: {key}")
            # Reclaim's catalogs use numbered positional placeholders.
            if Counter(re.findall(r"\{\d+\}", value)) != Counter(re.findall(r"\{\d+\}", key)):
                raise ValueError(f"{language}: changed placeholders: {key}")


def static_checks(root):
    for path in sorted((root / "scripts").glob("*.py")):
        ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    catalogs = {
        path.stem: json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
        for path in sorted((root / "locales").glob("*.json"))
    }
    check_catalogs(catalogs)
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    package = manifest["package"]
    if not any(p["name"] == package["name"] and p["version"] == package["version"]
               for p in lock["package"]):
        raise ValueError("Cargo.lock does not match the application version")
    print("PASS: Python syntax, unique translation keys, catalog parity, placeholders, lock version", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--full", action="store_true", help="also run formatting, tests and Clippy")
    parser.add_argument("--previews", type=Path, help="render headless UI previews into this directory")
    args = parser.parse_args()
    static_checks(ROOT)
    commands = []
    if args.full:
        commands += [
            [sys.executable, "-m", "unittest", "discover", "-s", "scripts", "-p", "test_*.py"],
            ["cargo", "fmt", "--check"],
            ["cargo", "test", "--locked"],
            ["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"],
        ]
    environment = os.environ.copy()
    if args.previews:
        directory = args.previews.resolve()
        directory.mkdir(parents=True, exist_ok=True)
        environment["RECLAIM_PREVIEW_DIR"] = str(directory)
        # In full mode the entire suite already includes these render tests.
        if not args.full:
            commands.append(["cargo", "test", "--locked", "--bin", "reclaim", "slint_workspace"])
    for command in commands:
        print("RUN:", " ".join(command), flush=True)
        subprocess.run(command, cwd=ROOT, env=environment, check=True)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        sys.exit(1)
